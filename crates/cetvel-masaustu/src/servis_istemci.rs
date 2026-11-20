//! Cetvel arka plan servisiyle (`cetvel servis`) HTTP / Named Pipe + jeton bağlantısı.
//!
//! Masaüstü uygulaması `anlik.json` veya platform sırlarını kendisi okumaz; yalnız
//! `baglanti.json` / `jeton` bilgisini okuyarak servisten anlık veri alır ve servis
//! çalışmıyorsa kullanıcıya `cetvel servis` sürecini başlatmayı önerir.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use cetvel_depo::{BaglantiBilgisi, CetvelYollar, baglanti_bilgisi_oku};
use cetvel_model::AnlikV2;
use cetvel_servis::bagla::{TaramaSonucu, TestSonucu};
use cetvel_servis::durum::YenileYaniti;
use cetvel_servis::pipe_sunucu::{pipe_adaylari, pipe_istek_kesifle};
use cetvel_servis::servis_calisiyor_mu;
use reqwest::StatusCode;
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};

use crate::durum::{MasaustuDurum, tepsi_gorunum_guncelle};

/// Ön yüz ve tepsi için servis bağlantı durumu özeti.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServisDurumuBilgisi {
    pub calisiyor: bool,
    pub port: Option<u16>,
    pub pipe: Option<String>,
    pub son_sira: Option<u64>,
    pub hata: Option<String>,
}

/// `anlik_al` / `yenile_iste` / `servis_baslat` komutlarının dönüş yapısı.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnlikServisYaniti {
    pub servis_calisiyor: bool,
    pub baslatma_onerisi: bool,
    pub anlik: Option<AnlikV2>,
    pub hata: Option<String>,
}

/// Çalışan `cetvel servis` örneğine HTTP / Named Pipe üzerinden bağlanan istemci.
#[derive(Clone)]
pub struct ServisIstemci {
    http: reqwest::Client,
    yollar: CetvelYollar,
}

impl ServisIstemci {
    pub fn ortamdan() -> Result<Self, String> {
        let yollar = CetvelYollar::ortamdan().map_err(|e| e.to_string())?;
        Ok(Self::yollarla(yollar))
    }

    pub fn yollarla(yollar: CetvelYollar) -> Self {
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(32))
            .build()
            .unwrap_or_default();
        Self { http, yollar }
    }

    fn baglanti_oku(&self) -> Result<BaglantiBilgisi, String> {
        let mut bilgi = baglanti_bilgisi_oku(&self.yollar).map_err(|e| e.to_string())?;
        if bilgi.jeton.trim().is_empty()
            && let Ok(ham) = std::fs::read_to_string(self.yollar.jeton_dosyasi())
        {
            bilgi.jeton = ham.trim().to_string();
        }
        Ok(bilgi)
    }

    /// Çalışan servisten güncel sağlık özetini kontrol eder.
    pub async fn saglik_kontrol(&self) -> ServisDurumuBilgisi {
        let Ok(bilgi) = self.baglanti_oku() else {
            // `baglanti.json` yok (silinmiş, servis ≤ 15 sn'de yeniden yazar): pipe keşfiyle dene.
            if let Ok(val) = pipe_istek_gonder("", "saglik", None).await {
                return ServisDurumuBilgisi {
                    calisiyor: true,
                    port: None,
                    pipe: None,
                    son_sira: val.get("sira").and_then(Value::as_u64),
                    hata: None,
                };
            }
            let surec_var = servis_calisiyor_mu();
            return ServisDurumuBilgisi {
                calisiyor: surec_var,
                port: None,
                pipe: None,
                son_sira: None,
                hata: (!surec_var).then(|| "servis_calismiyor".to_string()),
            };
        };

        if bilgi.port > 0 {
            let url = format!("http://127.0.0.1:{}/v1/saglik", bilgi.port);
            if let Ok(yanit) = self
                .http
                .get(&url)
                .header(AUTHORIZATION, format!("Bearer {}", bilgi.jeton))
                .timeout(Duration::from_millis(1500))
                .send()
                .await
                && yanit.status().is_success()
                && let Ok(baytlar) = yanit.bytes().await
                && let Ok(val) = serde_json::from_slice::<Value>(&baytlar)
            {
                return ServisDurumuBilgisi {
                    calisiyor: true,
                    port: Some(bilgi.port),
                    pipe: Some(bilgi.pipe),
                    son_sira: val.get("sira").and_then(Value::as_u64),
                    hata: None,
                };
            }
        }

        if let Ok(val) = pipe_istek_gonder(&bilgi.pipe, "saglik", None).await {
            return ServisDurumuBilgisi {
                calisiyor: true,
                port: (bilgi.port > 0).then_some(bilgi.port),
                pipe: Some(bilgi.pipe),
                son_sira: val.get("sira").and_then(Value::as_u64),
                hata: None,
            };
        }

        let surec_var = servis_calisiyor_mu();
        ServisDurumuBilgisi {
            calisiyor: surec_var,
            port: (bilgi.port > 0).then_some(bilgi.port),
            pipe: Some(bilgi.pipe),
            son_sira: None,
            hata: (!surec_var).then(|| "servis_calismiyor".to_string()),
        }
    }

    /// Servisten `AnlikV2` verisini getirir. `bekle_sira` verilirse uzun-yoklama (`long-poll`) yapar;
    /// değişiklik yoksa (`304 Not Modified`) `Ok(None)` döner.
    pub async fn anlik_getir(
        &self,
        bekle_sira: Option<u64>,
        sure_sn: u64,
    ) -> Result<Option<AnlikV2>, String> {
        let bilgi = self.baglanti_oku()?;

        if bilgi.port > 0 {
            let mut url = format!("http://127.0.0.1:{}/v1/anlik", bilgi.port);
            if let Some(sira) = bekle_sira {
                url.push_str(&format!("?bekle={sira}&sure={}", sure_sn.clamp(1, 60)));
            }
            let mut istek = self
                .http
                .get(&url)
                .header(AUTHORIZATION, format!("Bearer {}", bilgi.jeton))
                .timeout(Duration::from_secs(sure_sn.clamp(1, 60) + 4));
            if let Some(sira) = bekle_sira {
                istek = istek.header(reqwest::header::IF_NONE_MATCH, format!("\"{sira}\""));
            }

            if let Ok(yanit) = istek.send().await {
                let durum = yanit.status();
                if durum == StatusCode::NOT_MODIFIED {
                    return Ok(None);
                }
                let http_sonuc = if durum.is_success() {
                    match yanit.bytes().await {
                        Ok(b) => serde_json::from_slice::<AnlikV2>(&b).map_err(|e| e.to_string()),
                        Err(e) => Err(e.to_string()),
                    }
                } else {
                    Err(format!("http_durum_{}", durum.as_u16()))
                };
                match http_sonuc {
                    Ok(anlik) => return Ok(Some(anlik)),
                    // 4xx/5xx ya da ayrıştırılamayan gövde (ör. sürüm farkı): pipe yedeği denenir.
                    Err(http_hata) => {
                        return self
                            .anlik_pipe_ile(&bilgi, bekle_sira)
                            .await
                            .map_err(|pipe_hata| format!("{http_hata}; pipe: {pipe_hata}"));
                    }
                }
            }
        }

        // HTTP yanıt vermediyse Named Pipe üzerinden dene
        self.anlik_pipe_ile(&bilgi, bekle_sira).await
    }

    async fn anlik_pipe_ile(
        &self,
        bilgi: &BaglantiBilgisi,
        bekle_sira: Option<u64>,
    ) -> Result<Option<AnlikV2>, String> {
        let val = pipe_istek_gonder(&bilgi.pipe, "anlik.al", None).await?;
        let anlik: AnlikV2 = serde_json::from_value(val).map_err(|e| e.to_string())?;
        // Yalnız aynı `sira` "değişmedi" sayılır; servis yeniden başlayıp sira küçüldüyse yeni veri alınır.
        if bekle_sira.is_some_and(|sira| !sira_degisti_mi(sira, anlik.sira)) {
            return Ok(None);
        }
        Ok(Some(anlik))
    }

    /// Servisin HTTP API'sine JSON gövdeli istek atar; HTTP yoksa `None` döner (pipe denenmeli).
    async fn http_json(
        &self,
        bilgi: &BaglantiBilgisi,
        yontem: reqwest::Method,
        yol: &str,
        govde: Option<Value>,
        zaman_asimi: Duration,
    ) -> Option<Result<Value, String>> {
        if bilgi.port == 0 {
            return None;
        }
        let url = format!("http://127.0.0.1:{}{yol}", bilgi.port);
        let mut istek = self
            .http
            .request(yontem, &url)
            .header(AUTHORIZATION, format!("Bearer {}", bilgi.jeton))
            .timeout(zaman_asimi);
        if let Some(g) = govde {
            istek = istek
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(serde_json::to_vec(&g).unwrap_or_default());
        }
        let yanit = istek.send().await.ok()?;
        let durum = yanit.status();
        let baytlar = match yanit.bytes().await {
            Ok(b) => b,
            Err(e) => return Some(Err(e.to_string())),
        };
        let deger = serde_json::from_slice::<Value>(&baytlar).unwrap_or(Value::Null);
        if durum.is_success() || durum == StatusCode::TOO_MANY_REQUESTS {
            Some(Ok(deger))
        } else {
            let detay = deger
                .get("detay")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("http_durum_{}", durum.as_u16()));
            Some(Err(detay))
        }
    }

    /// Servisin salt-okunur Bağla taramasını alır (`GET /v1/bagla/tara`, yedek pipe `bagla.tara`).
    pub async fn bagla_tara(&self) -> Result<TaramaSonucu, String> {
        let bilgi = self.baglanti_oku()?;
        let deger = match self
            .http_json(
                &bilgi,
                reqwest::Method::GET,
                "/v1/bagla/tara",
                None,
                Duration::from_secs(5),
            )
            .await
        {
            Some(sonuc) => sonuc?,
            None => pipe_istek_gonder(&bilgi.pipe, "bagla.tara", None).await?,
        };
        serde_json::from_value(deger).map_err(|e| e.to_string())
    }

    /// "Test et": bağlayıcıyı servis döngüsünde çalıştırır (masaüstü kendisi okumaz).
    pub async fn bagla_test(&self, platform: &str) -> Result<TestSonucu, String> {
        let bilgi = self.baglanti_oku()?;
        let govde = json!({ "platform": platform });
        let deger = match self
            .http_json(
                &bilgi,
                reqwest::Method::POST,
                "/v1/bagla/test",
                Some(govde.clone()),
                Duration::from_secs(95),
            )
            .await
        {
            Some(sonuc) => sonuc?,
            None => pipe_istek_gonder(&bilgi.pipe, "bagla.test", Some(govde)).await?,
        };
        serde_json::from_value(deger).map_err(|e| e.to_string())
    }

    /// Ayar veya anahtar değişince servisin ayarları yeniden yüklemesini ister.
    pub async fn ayar_yenile(&self) -> Result<(), String> {
        let bilgi = self.baglanti_oku()?;
        match self
            .http_json(
                &bilgi,
                reqwest::Method::POST,
                "/v1/ayar/yenile",
                None,
                Duration::from_secs(4),
            )
            .await
        {
            Some(sonuc) => sonuc.map(|_| ()),
            None => pipe_istek_gonder(&bilgi.pipe, "ayar.yenile", None)
                .await
                .map(|_| ()),
        }
    }

    /// Çalışan servise yenileme (`POST /v1/yenile` veya pipe `yenile`) isteği gönderir.
    /// Hız sınırına takılırsa `istendi: false` ve `bekle_sn` döner (hata değildir).
    pub async fn yenile_iste(&self, havuz: Option<&str>) -> Result<YenileYaniti, String> {
        let bilgi = self.baglanti_oku()?;
        let temiz = havuz.map(str::trim).filter(|s| !s.is_empty());

        let mut yol = "/v1/yenile".to_string();
        if let Some(h) = temiz {
            yol.push_str(&format!("?havuz={}", url_parca_kodla(h)));
        }
        let deger = match self
            .http_json(
                &bilgi,
                reqwest::Method::POST,
                &yol,
                None,
                Duration::from_secs(4),
            )
            .await
        {
            Some(sonuc) => sonuc?,
            None => {
                let params = temiz.map(|h| json!({ "havuz": h }));
                pipe_istek_gonder(&bilgi.pipe, "yenile", params).await?
            }
        };
        serde_json::from_value(deger).map_err(|e| e.to_string())
    }

    /// Çalışan servisten geçmiş zaman serisini (`GET /v1/gecmis/{havuz}/{pencere}?gun=N`) alır.
    pub async fn gecmis_getir(
        &self,
        havuz: &str,
        pencere: &str,
        gun: u64,
    ) -> Result<Value, String> {
        if !slug_gecerli_mi(havuz) || !slug_gecerli_mi(pencere) {
            return Err("gecersiz_havuz_veya_pencere".to_string());
        }
        let bilgi = self.baglanti_oku()?;
        if bilgi.port == 0 {
            return Err("http_api_kapali".to_string());
        }
        let url = format!(
            "http://127.0.0.1:{}/v1/gecmis/{havuz}/{pencere}?gun={}",
            bilgi.port,
            gun.clamp(1, 90)
        );
        let yanit = self
            .http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", bilgi.jeton))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !yanit.status().is_success() {
            return Err(format!("http_durum_{}", yanit.status().as_u16()));
        }
        let baytlar = yanit.bytes().await.map_err(|e| e.to_string())?;
        serde_json::from_slice(&baytlar).map_err(|e| e.to_string())
    }

    /// Servis çalışmıyorsa arka planda `cetvel servis` sürecini başlatır ve bağlanmasını bekler.
    pub async fn servis_baslat(&self) -> Result<ServisDurumuBilgisi, String> {
        let mevcut = self.saglik_kontrol().await;
        if mevcut.calisiyor && mevcut.port.is_some() {
            return Ok(mevcut);
        }

        if !servis_calisiyor_mu() {
            let exe_yolu = cetvel_ikilisi_bul();
            let mut cmd = std::process::Command::new(&exe_yolu);
            cmd.arg("servis")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            cmd.spawn()
                .map_err(|e| format!("cetvel servis başlatılamadı: {e}"))?;
        }

        // Bağlantı dosyası ve ilk HTTP/pipe yanıtı için en fazla 3.2 sn bekle
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(160)).await;
            let durum = self.saglik_kontrol().await;
            if durum.calisiyor && (durum.port.is_some() || durum.pipe.is_some()) {
                return Ok(durum);
            }
        }

        Ok(self.saglik_kontrol().await)
    }
}

/// Uzun yoklamada yeni veri sayılıp sayılmayacağı: yalnız aynı `sira` "değişmedi" demektir.
pub fn sira_degisti_mi(bilinen: u64, gelen: u64) -> bool {
    bilinen != gelen
}

/// Servis/CLI ikilisinin (`cetvel.exe`) yolu. Kurulumda (NSIS) ve taşınabilir ZIP'te masaüstü
/// uygulamasıyla aynı dizindedir; geliştirmede (`cargo tauri dev`, `cargo run`) ikisi de
/// `target/<profil>/` altındadır. Bulunamazsa `PATH` araması için yalın ad döner.
pub fn cetvel_ikilisi_bul() -> PathBuf {
    if let Ok(mevcut_exe) = std::env::current_exe() {
        for aday in cetvel_ikilisi_adaylari(&mevcut_exe, cfg!(debug_assertions)) {
            if aday.is_file() {
                return aday;
            }
        }
    }
    PathBuf::from("cetvel.exe")
}

/// Aday sırası (saf; test edilir): önce aynı dizin, geliştirme derlemesinde ardından kardeş
/// profil dizinleri (`target/debug` ↔ `target/release`). Masaüstü ikilisinin kendisi asla aday
/// olmaz (NTFS büyük/küçük harf duyarsız: `Cetvel.exe` ile `cetvel.exe` aynı dosyadır).
pub fn cetvel_ikilisi_adaylari(mevcut_exe: &std::path::Path, gelistirme: bool) -> Vec<PathBuf> {
    let ayni_dosya = |p: &std::path::Path| {
        p.file_name()
            .zip(mevcut_exe.file_name())
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            && p.parent() == mevcut_exe.parent()
    };
    let mut adaylar = vec![mevcut_exe.with_file_name("cetvel.exe")];
    if gelistirme
        && let Some(profil) = mevcut_exe.parent()
        && let Some(target) = profil.parent()
    {
        for diger in ["debug", "release"] {
            let aday = target.join(diger).join("cetvel.exe");
            if !adaylar.contains(&aday) {
                adaylar.push(aday);
            }
        }
    }
    adaylar.retain(|p| !ayni_dosya(p));
    adaylar
}

fn slug_gecerli_mi(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn url_parca_kodla(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Servis canlıyken bu kadar ardışık okuma hatasından sonra arayüze hata yayılır.
const ARDISIK_HATA_ESIGI: u32 = 3;

/// Servis değişikliklerini uzun-yoklama / pipe ile izleyip `anlik_degisti` ve `servis_durumu_degisti`
/// Tauri olaylarını yayan arka plan görevi.
pub fn servis_izleyici_baslat(app: AppHandle, durum: MasaustuDurum) {
    tauri::async_runtime::spawn(async move {
        let mut son_bildirilen_calisiyor: Option<bool> = None;
        // Servis süreci canlıyken art arda hata (HTTP 4xx/5xx + pipe başarısız) sessiz kalmasın.
        let mut ardisik_hata: u32 = 0;

        loop {
            let son_sira = {
                let g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
                g.son_anlik.as_ref().map(|a| a.sira)
            };

            match durum.istemci.anlik_getir(son_sira, 20).await {
                Ok(Some(yeni_anlik)) => {
                    ardisik_hata = 0;
                    let sira_degisti = son_sira.is_none_or(|eski| eski != yeni_anlik.sira);
                    let durum_degisti = son_bildirilen_calisiyor != Some(true);
                    son_bildirilen_calisiyor = Some(true);

                    {
                        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
                        g.servis_calisiyor = true;
                        g.son_anlik = Some(yeni_anlik.clone());
                    }

                    if sira_degisti || durum_degisti {
                        tepsi_gorunum_guncelle(&app, &durum);
                        let _ = app.emit(
                            "anlik_degisti",
                            AnlikServisYaniti {
                                servis_calisiyor: true,
                                baslatma_onerisi: false,
                                anlik: Some(yeni_anlik),
                                hata: None,
                            },
                        );
                    }
                    if durum_degisti {
                        let _ = app.emit(
                            "servis_durumu_degisti",
                            ServisDurumuBilgisi {
                                calisiyor: true,
                                port: None,
                                pipe: None,
                                son_sira: son_sira.or(Some(0)),
                                hata: None,
                            },
                        );
                    }
                    // Named pipe fallback döngüsü ise CPU döndürmemek için kısa nefes
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Ok(None) => {
                    // 304 Not Modified veya aynı sıra: servis hayatta
                    ardisik_hata = 0;
                    let durum_degisti = son_bildirilen_calisiyor != Some(true);
                    son_bildirilen_calisiyor = Some(true);
                    {
                        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
                        g.servis_calisiyor = true;
                    }
                    if durum_degisti {
                        tepsi_gorunum_guncelle(&app, &durum);
                        let _ = app.emit(
                            "servis_durumu_degisti",
                            ServisDurumuBilgisi {
                                calisiyor: true,
                                port: None,
                                pipe: None,
                                son_sira,
                                hata: None,
                            },
                        );
                    }
                    tokio::time::sleep(Duration::from_millis(800)).await;
                }
                Err(hata) => {
                    let calisiyor_mu = servis_calisiyor_mu();
                    ardisik_hata = ardisik_hata.saturating_add(1);
                    let hata_bildir = calisiyor_mu && ardisik_hata == ARDISIK_HATA_ESIGI;
                    let durum_degisti =
                        son_bildirilen_calisiyor != Some(calisiyor_mu) || hata_bildir;
                    // Hata bildirildiyse toparlanınca "çalışıyor" yeniden yayılsın.
                    son_bildirilen_calisiyor = if hata_bildir {
                        None
                    } else {
                        Some(calisiyor_mu)
                    };

                    {
                        let mut g = durum.ic.lock().unwrap_or_else(|e| e.into_inner());
                        g.servis_calisiyor = calisiyor_mu;
                    }
                    if durum_degisti {
                        tepsi_gorunum_guncelle(&app, &durum);
                        let _ = app.emit(
                            "servis_durumu_degisti",
                            ServisDurumuBilgisi {
                                calisiyor: calisiyor_mu,
                                port: None,
                                pipe: None,
                                son_sira,
                                hata: Some(hata.clone()),
                            },
                        );
                        let _ = app.emit(
                            "anlik_degisti",
                            AnlikServisYaniti {
                                servis_calisiyor: calisiyor_mu,
                                baslatma_onerisi: !calisiyor_mu,
                                anlik: None,
                                hata: Some(hata),
                            },
                        );
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
}

/// Pipe isteği keşifle: önce `baglanti.json`'daki ad, yoksa oturum SID'li ad, en son eski
/// sabit ad (eski sürüm servis). Yalnız var olmayan pipe'ta sonrakine geçilir.
async fn pipe_istek_gonder(
    birincil: &str,
    yontem: &str,
    params: Option<Value>,
) -> Result<Value, String> {
    pipe_istek_kesifle(&pipe_adaylari(Some(birincil)), yontem, params)
        .await
        .map(|(_, deger)| deger)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    #[test]
    fn cetvel_ikilisi_kurulumda_yalniz_ayni_dizinde_aranir() {
        let kurulu = Path::new(r"C:\Kullanici\AppData\Local\Programs\Cetvel\cetvel-masaustu.exe");
        assert_eq!(
            super::cetvel_ikilisi_adaylari(kurulu, false),
            vec![PathBuf::from(
                r"C:\Kullanici\AppData\Local\Programs\Cetvel\cetvel.exe"
            )]
        );
    }

    #[test]
    fn cetvel_ikilisi_gelistirmede_kardes_profile_de_bakar() {
        let gelistirme = Path::new(r"C:\Depo\target\debug\cetvel-masaustu.exe");
        assert_eq!(
            super::cetvel_ikilisi_adaylari(gelistirme, true),
            vec![
                PathBuf::from(r"C:\Depo\target\debug\cetvel.exe"),
                PathBuf::from(r"C:\Depo\target\release\cetvel.exe"),
            ]
        );
    }

    #[test]
    fn masaustu_ikilisi_kendini_servis_sanmaz() {
        // NTFS büyük/küçük harf duyarsız: ürün adlı `Cetvel.exe` kendini başlatmamalı.
        let ayni_ad = Path::new(r"C:\Kur\Cetvel.exe");
        assert!(super::cetvel_ikilisi_adaylari(ayni_ad, false).is_empty());
    }

    #[test]
    fn sira_geri_gidince_de_yeni_veri_sayilir() {
        assert!(!super::sira_degisti_mi(42, 42));
        assert!(super::sira_degisti_mi(42, 43));
        assert!(
            super::sira_degisti_mi(42, 1),
            "servis yeniden başlayıp sıfırlandıysa"
        );
    }
    use std::sync::Arc;

    use cetvel_depo::{Ayarlar, BaglantiBilgisi, baglanti_bilgisi_yaz};
    use cetvel_model::{Durum, HavuzAnlik, PencereOlcum, UtcZaman};
    use cetvel_servis::durum::PaylasilanServisDurumu;
    use cetvel_servis::http_sunucu::http_router_olustur;

    use super::*;

    #[tokio::test]
    async fn servis_istemci_http_ve_jetonla_anlik_ve_yenile_calisir() {
        let gecici =
            std::env::temp_dir().join(format!("cetvel-masaustu-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&gecici);
        let yollar = CetvelYollar::ozel_koklerle(gecici.join("local"), gecici.join("roaming"));
        let jeton = "test-masaustu-jeton-12345";
        let pipe_adi = format!(r"\\.\pipe\cetvel-masaustu-test-{}", std::process::id());

        let paylasilan = Arc::new(PaylasilanServisDurumu::yeni(
            Ayarlar::default(),
            None,
            jeton,
            &pipe_adi,
            None,
        ));

        let simdi = UtcZaman::simdi();
        let mut codex = HavuzAnlik {
            platform: "codex".to_string(),
            etiket: "Codex Plus".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
            pencereler: BTreeMap::from([(
                "5s".to_string(),
                PencereOlcum {
                    uzunluk_dk: 300,
                    kullanilan: 0.25,
                    kalan: 0.75,
                    sifirlama: Some(simdi.ekle_sn(3600)),
                    etiket: None,
                },
            )]),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
            kesif: None,
            etkin_kalan_zincir: None,
        };
        codex.hesapla_etkin_durum();
        let mut havuzlar = BTreeMap::new();
        havuzlar.insert("codex_plus".to_string(), codex);
        paylasilan
            .anlik_yayinla(AnlikV2::yeni(simdi, 42, havuzlar), simdi)
            .await;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("tcp port acilmali");
        let port = listener.local_addr().unwrap().port();
        let router = http_router_olustur(Arc::clone(&paylasilan));
        let sunucu_gorev = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        baglanti_bilgisi_yaz(
            &yollar,
            &BaglantiBilgisi {
                port,
                adresler: vec![format!("http://127.0.0.1:{port}")],
                jeton: jeton.to_string(),
                pipe: pipe_adi,
                surum: "0.1.0".to_string(),
                pid: None,
            },
        )
        .expect("baglanti json yazilmali");

        let istemci = ServisIstemci::yollarla(yollar);
        let saglik = istemci.saglik_kontrol().await;
        assert!(saglik.calisiyor);
        assert_eq!(saglik.son_sira, Some(42));

        let anlik = istemci
            .anlik_getir(None, 2)
            .await
            .expect("anlik istegi basarili olmali")
            .expect("ilk istekte anlik donmeli");
        assert_eq!(anlik.sira, 42);
        assert!(anlik.havuzlar.contains_key("codex_plus"));

        // Aynı sıra ile uzun-yoklama 304 -> None dönmeli
        let degismedi = istemci
            .anlik_getir(Some(42), 1)
            .await
            .expect("304 hata olmamali");
        assert!(degismedi.is_none());

        let yenile = istemci.yenile_iste(None).await.expect("yenile calismali");
        assert!(yenile.istendi);
        assert_eq!(yenile.sira, 42);
        // Aynı dakika içinde ikinci istek hız sınırına takılır (hata değil, bekle_sn)
        let ikinci = istemci.yenile_iste(None).await.expect("ikinci yenile");
        assert!(!ikinci.istendi);
        assert!(ikinci.bekle_sn.is_some());

        // Tarama servis üzerinden gelir (masaüstü kendisi taramaz)
        let tarama = istemci.bagla_tara().await.expect("tarama");
        assert!(!tarama.codex_durum.is_empty());
        // Desteklenmeyen platform testi servis tarafından reddedilir
        assert!(istemci.bagla_test("gemini").await.is_err());
        istemci.ayar_yenile().await.expect("ayar yenile");

        sunucu_gorev.abort();
        let _ = std::fs::remove_dir_all(&gecici);
    }
}
