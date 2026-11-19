use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use cetvel_depo::CozulmusDeepSeek;
use cetvel_model::{
    BakiyeBilgisi, Durum, GuvenEtiketi, HataBilgisi, HavuzAnlik, KaynakBilgisi, KaynakTuru,
    UtcZaman,
};
use reqwest::header::{AUTHORIZATION, HeaderMap, RETRY_AFTER};
use reqwest::{Client, StatusCode, Url, redirect};
use serde::Deserialize;
use serde_json::Value;

use crate::{Baglayici, BaglayiciSonucu};

const MAKS_GOVDE_BAYT: usize = 64 * 1024;

/// DeepSeek `/user/balance` bakiye bağlayıcısı (`plan.md §6.5`).
/// Anahtar yoksa ağ isteği atmadan `Durum::Baglanmadi` (`baglanmadi`) döndürür.
pub struct DeepSeekBaglayicisi {
    ayar: CozulmusDeepSeek,
    loopback_http_izin: bool,
    istemci: Option<Client>,
}

impl DeepSeekBaglayicisi {
    pub fn yeni(ayar: CozulmusDeepSeek) -> Self {
        Self::ic_olustur(ayar, false)
    }

    pub fn test_icin_loopback(ayar: CozulmusDeepSeek) -> Self {
        Self::ic_olustur(ayar, true)
    }

    fn ic_olustur(ayar: CozulmusDeepSeek, loopback_http_izin: bool) -> Self {
        let mut builder = Client::builder()
            .redirect(redirect::Policy::none())
            .timeout(Duration::from_secs(ayar.zaman_asimi_sn.max(1)));
        if loopback_http_izin {
            builder = builder.no_proxy();
        }
        let istemci = builder.build().ok();
        Self {
            ayar,
            loopback_http_izin,
            istemci,
        }
    }

    pub fn uc_url_dogrula(&self) -> Result<Url, &'static str> {
        let taban = self.ayar.taban_url.trim().trim_end_matches('/');
        let url = Url::parse(&format!("{taban}/user/balance")).map_err(|_| "gecersiz_url")?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err("url_kimlik_yasak");
        }
        let host = url.host_str().ok_or("gecersiz_url")?;
        let loopback_mi = host.eq_ignore_ascii_case("127.0.0.1")
            || host.eq_ignore_ascii_case("localhost")
            || host == "[::1]"
            || host == "::1";

        match url.scheme() {
            "https" => {}
            "http" if self.loopback_http_izin && loopback_mi => {}
            _ => return Err("https_zorunlu"),
        }

        if !self.loopback_http_izin && !host.eq_ignore_ascii_case("api.deepseek.com") {
            return Err("izinli_host_degil");
        }

        Ok(url)
    }
}

impl Baglayici for DeepSeekBaglayicisi {
    fn havuz_id(&self) -> &str {
        "deepseek"
    }

    fn oku<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = BaglayiciSonucu> + Send + 'a>> {
        Box::pin(async move {
            if !self.ayar.etkin {
                return BaglayiciSonucu {
                    havuz_id: "deepseek".to_string(),
                    havuz: deepseek_durum_havuzu(
                        Durum::Kapali,
                        None,
                        vec!["deepseek ayardan kapalı".into()],
                    ),
                    retry_after_sn: None,
                    gecici_hata: false,
                };
            }

            let Some(anahtar) = &self.ayar.anahtar else {
                return BaglayiciSonucu {
                    havuz_id: "deepseek".to_string(),
                    havuz: deepseek_durum_havuzu(
                        Durum::Baglanmadi,
                        None,
                        vec!["DeepSeek API anahtarı ortamda tanımlı değil".into()],
                    ),
                    retry_after_sn: None,
                    gecici_hata: false,
                };
            };

            let url = match self.uc_url_dogrula() {
                Ok(u) => u,
                Err(tur) => {
                    return hata_sonucu(
                        tur,
                        None,
                        "DeepSeek URL doğrulaması başarısız",
                        None,
                        false,
                    );
                }
            };

            let Some(istemci) = &self.istemci else {
                return hata_sonucu(
                    "istemci_hatasi",
                    None,
                    "HTTP istemcisi başlatılamadı",
                    None,
                    true,
                );
            };

            let bearer = format!("Bearer {}", anahtar.acik_metin());
            let yanit = match istemci.get(url).header(AUTHORIZATION, bearer).send().await {
                Ok(r) => r,
                Err(e) => {
                    let tur = if e.is_timeout() {
                        "zaman_asimi"
                    } else {
                        "ag_hatasi"
                    };
                    return hata_sonucu(tur, None, "DeepSeek ucuna erişilemedi", None, true);
                }
            };

            let durum_kodu = yanit.status();
            let basliklar = yanit.headers().clone();

            if durum_kodu.is_redirection() {
                return hata_sonucu(
                    "yonlendirme_engellendi",
                    Some(durum_kodu.as_u16()),
                    "yönlendirme (3xx) izlenmedi",
                    None,
                    true,
                );
            }
            if durum_kodu == StatusCode::UNAUTHORIZED || durum_kodu == StatusCode::FORBIDDEN {
                return hata_sonucu(
                    "kimlik_hatasi",
                    Some(durum_kodu.as_u16()),
                    "DeepSeek yetkilendirmesi reddedildi",
                    None,
                    true,
                );
            }
            if durum_kodu == StatusCode::TOO_MANY_REQUESTS {
                let retry = retry_after_coz(&basliklar);
                return hata_sonucu("hiz_siniri", Some(429), "hız sınırı (429)", retry, true);
            }
            if !durum_kodu.is_success() {
                return hata_sonucu(
                    "http_hatasi",
                    Some(durum_kodu.as_u16()),
                    format!("HTTP durum {}", durum_kodu.as_u16()),
                    None,
                    true,
                );
            }

            let bayt = match crate::sinirli_govde_oku(yanit, MAKS_GOVDE_BAYT).await {
                Ok(b) => b,
                Err(crate::GovdeHatasi::CokBuyuk) => {
                    return hata_sonucu(
                        "govde_cok_buyuk",
                        None,
                        "yanıt gövde sınırını aştı",
                        None,
                        true,
                    );
                }
                Err(crate::GovdeHatasi::Ag) => {
                    return hata_sonucu("ag_hatasi", None, "yanıt gövdesi okunamadı", None, true);
                }
            };

            let havuz = deepseek_yanit_ayristir(&bayt, simdi);
            let gecici_hata = havuz.durum == Durum::Hata;
            BaglayiciSonucu {
                havuz_id: "deepseek".to_string(),
                havuz,
                retry_after_sn: None,
                gecici_hata,
            }
        })
    }
}

#[derive(Debug, Deserialize)]
struct HamDeepSeekYanit {
    is_available: bool,
    #[serde(default)]
    balance_infos: Vec<HamBakiye>,
}

#[derive(Debug, Deserialize)]
struct HamBakiye {
    currency: String,
    total_balance: Value,
    granted_balance: Option<Value>,
    topped_up_balance: Option<Value>,
}

/// DeepSeek `/user/balance` JSON gövdesini `HavuzAnlik` yapısına çevirir (`plan.md §6.5`).
pub fn deepseek_yanit_ayristir(govde: &[u8], simdi: UtcZaman) -> HavuzAnlik {
    let Ok(ham) = serde_json::from_slice::<HamDeepSeekYanit>(govde) else {
        return deepseek_durum_havuzu(
            Durum::Hata,
            Some(HataBilgisi {
                tur: "sema_degisti".to_string(),
                http_kodu: None,
            }),
            vec!["DeepSeek bakiye yanıtı beklenmeyen yapıda".into()],
        );
    };

    let secilen = ham
        .balance_infos
        .iter()
        .find(|b| b.currency.eq_ignore_ascii_case("USD"))
        .or_else(|| ham.balance_infos.first());

    let Some(info) = secilen else {
        let mut havuz = HavuzAnlik {
            platform: "deepseek".to_string(),
            etiket: "DeepSeek".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: Some(false),
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::BakiyeApi,
                guven: GuvenEtiketi::ResmiApi,
                okuma: simdi,
                yas_sn: 0,
                hassasiyet: None,
            }),
            pencereler: BTreeMap::new(),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: Some(BakiyeBilgisi {
                para_birimi: "USD".to_string(),
                toplam: 0.0,
                hibe: None,
                yuklenen: None,
                kullanilabilir: Some(ham.is_available),
            }),
            ek: None,
            hata: None,
            notlar: vec!["bakiye bilgisi boş".into()],
        };
        havuz.hesapla_etkin_durum();
        return havuz;
    };

    let Some(toplam) = sayi_degeri_coz(&info.total_balance) else {
        return deepseek_durum_havuzu(
            Durum::Hata,
            Some(HataBilgisi {
                tur: "sema_degisti".to_string(),
                http_kodu: None,
            }),
            vec!["DeepSeek bakiye tutarı ayrıştırılamadı".into()],
        );
    };

    let hibe = info.granted_balance.as_ref().and_then(sayi_degeri_coz);
    let yuklenen = info.topped_up_balance.as_ref().and_then(sayi_degeri_coz);

    let mut havuz = HavuzAnlik {
        platform: "deepseek".to_string(),
        etiket: "DeepSeek".to_string(),
        durum: Durum::Ok,
        etkin_kalan: None,
        baglayici_pencere: None,
        kullanilabilir: None,
        kaynak: Some(KaynakBilgisi {
            tur: KaynakTuru::BakiyeApi,
            guven: GuvenEtiketi::ResmiApi,
            okuma: simdi,
            yas_sn: 0,
            hassasiyet: None,
        }),
        pencereler: BTreeMap::new(),
        alt_sayaclar: BTreeMap::new(),
        tahmin: BTreeMap::new(),
        baski: None,
        bakiye: Some(BakiyeBilgisi {
            para_birimi: info.currency.clone(),
            toplam,
            hibe,
            yuklenen,
            kullanilabilir: Some(ham.is_available),
        }),
        ek: None,
        hata: None,
        notlar: vec![format!("bakiye: {:.2} {}", toplam, info.currency)],
    };
    havuz.hesapla_etkin_durum();
    havuz
}

fn sayi_degeri_coz(val: &Value) -> Option<f64> {
    match val {
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|v| v.is_finite()),
        Value::Number(n) => n.as_f64().filter(|v| v.is_finite()),
        _ => None,
    }
}

fn retry_after_coz(basliklar: &HeaderMap) -> Option<u64> {
    basliklar
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
}

fn hata_sonucu(
    tur: &str,
    http_kodu: Option<u16>,
    not: impl Into<String>,
    retry_after_sn: Option<u64>,
    gecici_hata: bool,
) -> BaglayiciSonucu {
    BaglayiciSonucu {
        havuz_id: "deepseek".to_string(),
        havuz: deepseek_durum_havuzu(
            Durum::Hata,
            Some(HataBilgisi {
                tur: tur.to_string(),
                http_kodu,
            }),
            vec![not.into()],
        ),
        retry_after_sn,
        gecici_hata,
    }
}

fn deepseek_durum_havuzu(
    durum: Durum,
    hata: Option<HataBilgisi>,
    notlar: Vec<String>,
) -> HavuzAnlik {
    HavuzAnlik {
        platform: "deepseek".to_string(),
        etiket: "DeepSeek".to_string(),
        durum,
        etkin_kalan: None,
        baglayici_pencere: None,
        kullanilabilir: Some(false),
        kaynak: None,
        pencereler: BTreeMap::new(),
        alt_sayaclar: BTreeMap::new(),
        tahmin: BTreeMap::new(),
        baski: None,
        bakiye: None,
        ek: None,
        hata,
        notlar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cetvel_depo::GizliDeger;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
    }

    #[tokio::test]
    async fn anahtar_yoksa_istek_atmadan_baglanmadi_doner() {
        let ayar = CozulmusDeepSeek {
            etkin: true,
            taban_url: "https://api.deepseek.com".to_string(),
            anahtar: None,
            aralik_sn: 900,
            zaman_asimi_sn: 2,
        };
        let baglayici = DeepSeekBaglayicisi::yeni(ayar);
        let sonuc = baglayici.oku(z("2026-11-15T18:00:00Z")).await;
        assert_eq!(sonuc.havuz_id, "deepseek");
        assert_eq!(sonuc.havuz.durum, Durum::Baglanmadi);
        assert_eq!(sonuc.havuz.kullanilabilir, Some(false));
        assert!(sonuc.havuz.bakiye.is_none());
        assert!(!sonuc.gecici_hata);
    }

    #[tokio::test]
    async fn sahte_deepseek_bakiye_sunucusunu_ayristirir() {
        let govde = r#"{
            "is_available": true,
            "balance_infos": [
                {
                    "currency": "USD",
                    "total_balance": "14.25",
                    "granted_balance": "2.00",
                    "topped_up_balance": "12.25"
                }
            ]
        }"#;
        let yanit = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            govde.len(),
            govde
        );
        let dinleyici = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = dinleyici.local_addr().expect("addr").port();
        tokio::spawn(async move {
            if let Ok((mut soket, _)) = dinleyici.accept().await {
                let mut buf = [0u8; 1024];
                let _ = soket.read(&mut buf).await;
                let _ = soket.write_all(yanit.as_bytes()).await;
            }
        });

        let ayar = CozulmusDeepSeek {
            etkin: true,
            taban_url: format!("http://127.0.0.1:{port}"),
            anahtar: GizliDeger::yeni("test-key"),
            aralik_sn: 900,
            zaman_asimi_sn: 2,
        };
        let baglayici = DeepSeekBaglayicisi::test_icin_loopback(ayar);
        let sonuc = baglayici.oku(z("2026-11-15T18:00:00Z")).await;

        assert_eq!(sonuc.havuz.durum, Durum::Ok);
        assert_eq!(sonuc.havuz.kullanilabilir, Some(true));
        let bakiye = sonuc.havuz.bakiye.expect("bakiye");
        assert_eq!(bakiye.para_birimi, "USD");
        assert!((bakiye.toplam - 14.25).abs() < 1e-9);
        assert!((bakiye.hibe.unwrap() - 2.0).abs() < 1e-9);
        assert!((bakiye.yuklenen.unwrap() - 12.25).abs() < 1e-9);
    }

    async fn buyuk_govdeli_sunucu(baslik_uzunluk: Option<usize>, gonderilen: usize) -> u16 {
        let dinleyici = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = dinleyici.local_addr().expect("addr").port();
        tokio::spawn(async move {
            if let Ok((mut soket, _)) = dinleyici.accept().await {
                let mut buf = [0u8; 1024];
                let _ = soket.read(&mut buf).await;
                let uzunluk = baslik_uzunluk
                    .map(|u| format!("Content-Length: {u}\r\n"))
                    .unwrap_or_default();
                let baslik = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{uzunluk}Connection: close\r\n\r\n"
                );
                let _ = soket.write_all(baslik.as_bytes()).await;
                let parca = vec![b' '; 16 * 1024];
                let mut kalan = gonderilen;
                while kalan > 0 {
                    let n = kalan.min(parca.len());
                    if soket.write_all(&parca[..n]).await.is_err() {
                        break;
                    }
                    kalan -= n;
                }
            }
        });
        port
    }

    #[tokio::test]
    async fn buyuk_govde_sinirda_kesilir() {
        for (baslik, gonderilen) in [(Some(50 * 1024 * 1024), 256 * 1024), (None, 512 * 1024)] {
            let port = buyuk_govdeli_sunucu(baslik, gonderilen).await;
            let ayar = CozulmusDeepSeek {
                etkin: true,
                taban_url: format!("http://127.0.0.1:{port}"),
                anahtar: GizliDeger::yeni("test-key"),
                aralik_sn: 900,
                zaman_asimi_sn: 5,
            };
            let sonuc = DeepSeekBaglayicisi::test_icin_loopback(ayar)
                .oku(z("2026-11-15T18:00:00Z"))
                .await;
            assert_eq!(sonuc.havuz.durum, Durum::Hata);
            assert_eq!(
                sonuc.havuz.hata.as_ref().map(|h| h.tur.as_str()),
                Some("govde_cok_buyuk"),
                "başlık: {baslik:?}"
            );
        }
    }
}
