use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;

use crate::ServisHatasi;
use crate::durum::PaylasilanServisDurumu;

/// SID çözülemezse kullanılan yedek boru yolu.
pub const VARSAYILAN_PIPE_ADI: &str = r"\\.\pipe\cetvel";

/// Oturum kullanıcısına özgü boru adı (`plan.md §5.4`: `\\.\pipe\cetvel-<SID>`); iki Windows
/// kullanıcısının servisleri çakışmaz.
pub fn kullanici_pipe_adi() -> String {
    cetvel_depo::gecerli_kullanici_sid()
        .ok()
        .map(|sid| pipe_adi_uret(&sid))
        .unwrap_or_else(|| VARSAYILAN_PIPE_ADI.to_string())
}

/// SID'den boru adı üretir (yalnız güvenli karakterler).
pub fn pipe_adi_uret(sid: &str) -> String {
    let temiz: String = sid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(96)
        .collect();
    if temiz.is_empty() {
        VARSAYILAN_PIPE_ADI.to_string()
    } else {
        format!(r"\\.\pipe\cetvel-{temiz}")
    }
}
const AZAMI_PIPE_SATIR_BAYT: usize = 8192;
/// Aynı anda işlenen en fazla pipe bağlantısı (aynı kullanıcının hatalı istemcisi belleği tüketmesin).
const AZAMI_ESZAMANLI_PIPE: usize = 32;
/// Boşta bekleyen bağlantı (satır göndermeyen istemci) bu süre sonunda kapatılır.
const PIPE_SATIR_BEKLEME: Duration = Duration::from_secs(120);

/// İstemci tarafı istek zaman aşımı: `bagla.test` bağlayıcıyı çalıştırabildiği için 95 sn,
/// diğerleri 5 sn. Canlı ama yanıtsız servis istemciyi (masaüstü izleyici, `cetvel durum`, MCP)
/// sonsuza dek bekletmez.
pub fn pipe_zaman_asimi(yontem: &str) -> Duration {
    match yontem {
        "bagla.test" => Duration::from_secs(95),
        _ => Duration::from_secs(5),
    }
}

#[derive(Debug, Deserialize)]
struct PipeIstek {
    #[serde(default)]
    id: Option<Value>,
    #[serde(alias = "method")]
    yontem: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipeHata {
    pub kod: String,
    pub mesaj: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipeYanit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sonuc: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hata: Option<PipeHata>,
}

/// Adlandırılmış boru sunucusunu yalnızca yerel kullanıcı + SYSTEM erişimiyle başlatır.
#[cfg(windows)]
pub async fn pipe_sunucusu_baslat(
    durum: Arc<PaylasilanServisDurumu>,
    pipe_adi: impl Into<String>,
    mut kapanis: watch::Receiver<bool>,
) -> Result<tokio::task::JoinHandle<()>, ServisHatasi> {
    let pipe_adi = pipe_adi.into();
    let sid = cetvel_depo::gecerli_kullanici_sid()?;
    // Sahip açıkça kullanıcı SID'i: yükseltilmiş başlatmada varsayılan sahip Administrators olur ve
    // istemcinin sahip doğrulaması (süreç belirteci açılamadığında) bu alana dayanır.
    let sddl = pipe_sddl(&sid);

    // İlk örneği döngüden önce aç ki sunucu dönmeden önce bağlanmaya hazır olsun.
    let mut sonraki_sunucu = guvenli_pipe_olustur(&pipe_adi, &sddl, true)
        .map_err(|e| ServisHatasi::Genel(format!("Named pipe açılamadı ({pipe_adi}): {e}")))?;

    let sinir = Arc::new(tokio::sync::Semaphore::new(AZAMI_ESZAMANLI_PIPE));
    let gorev = tokio::spawn(async move {
        loop {
            let bagli_sunucu = tokio::select! {
                _ = kapanis.changed() => {
                    if *kapanis.borrow() {
                        break;
                    }
                    continue;
                }
                res = sonraki_sunucu.connect() => {
                    match res {
                        Ok(()) => sonraki_sunucu,
                        Err(e) => {
                            tracing::warn!("Named pipe bağlantı hatası: {e}");
                            break;
                        }
                    }
                }
            };

            match guvenli_pipe_olustur(&pipe_adi, &sddl, false) {
                Ok(yeni_dinleyici) => {
                    sonraki_sunucu = yeni_dinleyici;
                }
                Err(e) => {
                    tracing::warn!("Sonraki named pipe örneği açılamadı: {e}");
                    break;
                }
            }

            let Ok(izin) = Arc::clone(&sinir).try_acquire_owned() else {
                tracing::warn!("eşzamanlı pipe bağlantı sınırı aşıldı; bağlantı kapatıldı");
                drop(bagli_sunucu);
                continue;
            };
            let durum_istemci = Arc::clone(&durum);
            tokio::spawn(async move {
                pipe_istemci_isle(bagli_sunucu, durum_istemci).await;
                drop(izin);
            });
        }
    });

    Ok(gorev)
}

#[cfg(not(windows))]
pub async fn pipe_sunucusu_baslat(
    _durum: Arc<PaylasilanServisDurumu>,
    _pipe_adi: impl Into<String>,
    _kapanis: watch::Receiver<bool>,
) -> Result<tokio::task::JoinHandle<()>, ServisHatasi> {
    Ok(tokio::spawn(async {}))
}

#[cfg(windows)]
fn guvenli_pipe_olustur(
    pipe_adi: &str,
    sddl: &str,
    ilk_ornek: bool,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use std::ffi::c_void;
    use tokio::net::windows::named_pipe::ServerOptions;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use windows::core::{BOOL, PCWSTR};

    let mut sddl_w: Vec<u16> = sddl.encode_utf16().collect();
    sddl_w.push(0);

    let mut sd = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl_w.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
        .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    }

    let mut sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: BOOL(0),
    };

    let mut secenekler = ServerOptions::new();
    secenekler
        .first_pipe_instance(ilk_ornek)
        .reject_remote_clients(true);

    // SAFETY: `sa` geçerli `SECURITY_ATTRIBUTES` ve SDDL ile oluşturulan `PSECURITY_DESCRIPTOR` taşır;
    // çağrı tamamlandıktan hemen sonra `LocalFree` ile serbest bırakılır.
    let sonuc = unsafe {
        secenekler.create_with_security_attributes_raw(
            pipe_adi,
            (&mut sa as *mut SECURITY_ATTRIBUTES).cast::<c_void>(),
        )
    };

    unsafe {
        let _ = LocalFree(Some(HLOCAL(sd.0)));
    }

    sonuc
}

#[cfg(windows)]
async fn pipe_istemci_isle(
    stream: tokio::net::windows::named_pipe::NamedPipeServer,
    durum: Arc<PaylasilanServisDurumu>,
) {
    let (okuyucu, mut yazici) = tokio::io::split(stream);
    let mut satirlar = BufReader::new(okuyucu);
    let mut tampon = String::new();

    loop {
        tampon.clear();
        // Sınırlı okuma: satır sonu gelmese de en fazla AZAMI + 1 bayt belleğe alınır.
        let mut sinirli = (&mut satirlar).take(AZAMI_PIPE_SATIR_BAYT as u64 + 1);
        let Ok(Ok(okunan)) =
            tokio::time::timeout(PIPE_SATIR_BEKLEME, sinirli.read_line(&mut tampon)).await
        else {
            break;
        };
        if okunan == 0 {
            break;
        }
        if tampon.len() > AZAMI_PIPE_SATIR_BAYT {
            let hata = PipeYanit {
                id: None,
                sonuc: None,
                hata: Some(PipeHata {
                    kod: "cok_buyuk".to_string(),
                    mesaj: "İstek satırı 8 KB sınırını aştı".to_string(),
                }),
            };
            let _ = yanit_satiri_yaz(&mut yazici, &hata).await;
            break;
        }
        let temiz = tampon.trim();
        if temiz.is_empty() {
            continue;
        }

        let yanit = match serde_json::from_str::<PipeIstek>(temiz) {
            Ok(istek) => pipe_komutu_calistir(&durum, istek).await,
            Err(e) => PipeYanit {
                id: None,
                sonuc: None,
                hata: Some(PipeHata {
                    kod: "gecersiz_json".to_string(),
                    mesaj: e.to_string(),
                }),
            },
        };

        if yanit_satiri_yaz(&mut yazici, &yanit).await.is_err() {
            break;
        }
    }
}

async fn yanit_satiri_yaz<W: AsyncWriteExt + Unpin>(
    yazici: &mut W,
    yanit: &PipeYanit,
) -> std::io::Result<()> {
    let mut satir = serde_json::to_string(yanit).map_err(std::io::Error::other)?;
    satir.push('\n');
    yazici.write_all(satir.as_bytes()).await?;
    yazici.flush().await?;
    Ok(())
}

async fn pipe_komutu_calistir(durum: &PaylasilanServisDurumu, istek: PipeIstek) -> PipeYanit {
    let id = istek.id.clone();
    match istek.yontem.trim() {
        "anlik.al" => {
            let anlik = durum.anlik_kopya().await;
            PipeYanit {
                id,
                sonuc: serde_json::to_value(anlik).ok(),
                hata: None,
            }
        }
        "ajanlar.al" => PipeYanit {
            id,
            sonuc: serde_json::to_value(&*durum.ajanlar_kopya()).ok(),
            hata: None,
        },
        "saglik" => {
            let saglik = durum.saglik_ozeti().await;
            PipeYanit {
                id,
                sonuc: serde_json::to_value(saglik).ok(),
                hata: None,
            }
        }
        "havuz.al" => {
            let ad = istek
                .params
                .as_ref()
                .and_then(|p| p.get("havuz").or_else(|| p.get("ad")))
                .and_then(Value::as_str)
                .unwrap_or("");
            if ad.is_empty() {
                return PipeYanit {
                    id,
                    sonuc: None,
                    hata: Some(PipeHata {
                        kod: "eksik_parametre".to_string(),
                        mesaj: "havuz adı gerekli".to_string(),
                    }),
                };
            }
            match durum.havuz_kopya(ad).await {
                Some(h) => PipeYanit {
                    id,
                    sonuc: serde_json::to_value(h).ok(),
                    hata: None,
                },
                None => PipeYanit {
                    id,
                    sonuc: None,
                    hata: Some(PipeHata {
                        kod: "bulunamadi".to_string(),
                        mesaj: format!("Havuz bulunamadı: {ad}"),
                    }),
                },
            }
        }
        "yenile" => {
            let havuz = istek
                .params
                .as_ref()
                .and_then(|p| p.get("havuz"))
                .and_then(Value::as_str);
            let yanit = durum
                .yenile_iste(havuz, cetvel_model::UtcZaman::simdi())
                .await;
            basarili(id, &yanit)
        }
        "bagla.tara" => {
            let sonuc = durum.bagla_tara().await;
            basarili(id, &sonuc)
        }
        "bagla.test" => {
            let platform = istek
                .params
                .as_ref()
                .and_then(|p| p.get("platform"))
                .and_then(Value::as_str)
                .unwrap_or("");
            match durum.bagla_test(platform).await {
                Ok(sonuc) => basarili(id, &sonuc),
                Err(e) => hatali(id, "test_hatasi", &e),
            }
        }
        "ayar.yenile" => {
            durum.ayar_yenile_iste.notify_one();
            PipeYanit {
                id,
                sonuc: Some(json!({ "istendi": true })),
                hata: None,
            }
        }
        "ayar.uyari" => PipeYanit {
            id,
            sonuc: Some(json!({ "uyari": durum.ayar_uyarisi() })),
            hata: None,
        },
        "kopru.claude" => {
            let yuk = istek.params.unwrap_or(Value::Null);
            match durum
                .claude_guncelle(yuk, cetvel_model::UtcZaman::simdi())
                .await
            {
                Ok(y) => basarili(id, &y),
                Err(e) => hatali(id, "gecersiz_claude_verisi", &e.to_string()),
            }
        }
        "kapat" => {
            durum.kapat_iste.notify_one();
            PipeYanit {
                id,
                sonuc: Some(json!({ "kapaniyor": true })),
                hata: None,
            }
        }
        diger => PipeYanit {
            id,
            sonuc: None,
            hata: Some(PipeHata {
                kod: "bilinmeyen_yontem".to_string(),
                mesaj: format!("Bilinmeyen pipe komutu: {diger}"),
            }),
        },
    }
}

fn basarili<T: Serialize>(id: Option<Value>, deger: &T) -> PipeYanit {
    PipeYanit {
        id,
        sonuc: serde_json::to_value(deger).ok(),
        hata: None,
    }
}

fn hatali(id: Option<Value>, kod: &str, mesaj: &str) -> PipeYanit {
    PipeYanit {
        id,
        sonuc: None,
        hata: Some(PipeHata {
            kod: kod.to_string(),
            mesaj: mesaj.to_string(),
        }),
    }
}

/// İstemci keşif sırası (`plan.md §5.4`): önce `baglanti.json`'daki ad (servisin yazdığı, yalnız
/// kullanıcı DACL'li dosya), sonra oturum SID'li ad, en son eski sürümlerin sabit adı
/// (`\\.\pipe\cetvel`, geriye uyum). Yalnız `\\.\pipe\cetvel` önekli adlar kabul edilir.
pub fn pipe_adaylari(baglanti_pipe: Option<&str>) -> Vec<String> {
    pipe_adaylari_ile(baglanti_pipe, &kullanici_pipe_adi())
}

fn pipe_adaylari_ile(baglanti_pipe: Option<&str>, sidli: &str) -> Vec<String> {
    let mut adaylar: Vec<String> = Vec::new();
    for aday in [
        baglanti_pipe.map(str::trim),
        Some(sidli),
        Some(VARSAYILAN_PIPE_ADI),
    ]
    .into_iter()
    .flatten()
    {
        let gecerli = aday == VARSAYILAN_PIPE_ADI
            || aday
                .strip_prefix(VARSAYILAN_PIPE_ADI)
                .is_some_and(|ek| ek.starts_with('-') && ek.len() > 1);
        if gecerli && !adaylar.iter().any(|a| a.eq_ignore_ascii_case(aday)) {
            adaylar.push(aday.to_string());
        }
    }
    adaylar
}

/// Pipe istemci hatası: yalnız "pipe yok" durumunda bir sonraki aday denenir (canlı ama yavaş
/// servis başka bir ada düşülerek atlanmaz).
#[derive(Debug, Clone, PartialEq, Eq)]
enum PipeIstemciHatasi {
    Yok(String),
    Diger(String),
}

impl PipeIstemciHatasi {
    fn metin(self) -> String {
        match self {
            Self::Yok(m) | Self::Diger(m) => m,
        }
    }
}

/// İstemci olarak yerel Cetvel named pipe sunucusuna tek JSON satırı gönderip yanıt alır.
/// İsteğin tamamı [`pipe_zaman_asimi`] ile sınırlıdır; aşılırsa `Err` döner. Sunucu sürecinin
/// sahibi bu kullanıcı değilse (pipe adı genel ad alanında; başka kullanıcı adı önceden almış
/// olabilir) istek gönderilmez.
pub async fn pipe_istek_gonder(
    pipe_adi: &str,
    yontem: &str,
    params: Option<Value>,
) -> Result<Value, String> {
    pipe_istek_gonder_sureli(pipe_adi, yontem, params, pipe_zaman_asimi(yontem)).await
}

/// [`pipe_istek_gonder`]'in açık zaman aşımlı sürümü.
pub async fn pipe_istek_gonder_sureli(
    pipe_adi: &str,
    yontem: &str,
    params: Option<Value>,
    sure: Duration,
) -> Result<Value, String> {
    pipe_istek_turlu(pipe_adi, yontem, params, sure)
        .await
        .map_err(PipeIstemciHatasi::metin)
}

async fn pipe_istek_turlu(
    pipe_adi: &str,
    yontem: &str,
    params: Option<Value>,
    sure: Duration,
) -> Result<Value, PipeIstemciHatasi> {
    match tokio::time::timeout(sure, pipe_istek_gonder_ic(pipe_adi, yontem, params)).await {
        Ok(sonuc) => sonuc,
        Err(_) => Err(PipeIstemciHatasi::Diger(format!(
            "Pipe zaman aşımı ({} ms, {yontem}): servis yanıt vermiyor",
            sure.as_millis()
        ))),
    }
}

/// Adayları sırayla dener ([`pipe_adaylari`]); yalnız var olmayan pipe'ta sonrakine geçer.
/// Başarıda yanıtı veren pipe adını da döner.
pub async fn pipe_istek_kesifle(
    adaylar: &[String],
    yontem: &str,
    params: Option<Value>,
) -> Result<(String, Value), String> {
    let mut yoklar = Vec::new();
    for aday in adaylar {
        match pipe_istek_turlu(aday, yontem, params.clone(), pipe_zaman_asimi(yontem)).await {
            Ok(deger) => return Ok((aday.clone(), deger)),
            Err(PipeIstemciHatasi::Yok(_)) => yoklar.push(aday.as_str()),
            Err(PipeIstemciHatasi::Diger(m)) => return Err(m),
        }
    }
    Err(format!(
        "Cetvel pipe'ı bulunamadı (denenen: {})",
        yoklar.join(", ")
    ))
}

/// Var olan ilk aday pipe'ın sunucu süreç kimliği (kaldırmada yanıtsız servisi bulmak için;
/// istek gönderilmez, yalnız bağlanıp `GetNamedPipeServerProcessId` sorgulanır).
pub async fn pipe_sunucu_pid(adaylar: &[String]) -> Option<(String, u32)> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use tokio::net::windows::named_pipe::ClientOptions;
        const ERROR_PIPE_BUSY: i32 = 231;
        for aday in adaylar {
            // Tüm örnekler anlık meşgulse kısa süre beklenir: `baglanti.json`'da `pid` yoksa
            // (eski sürüm) yanıtsız servisi bulmanın tek yolu budur.
            let mut deneme = 0;
            let istemci = loop {
                match ClientOptions::new().open(aday) {
                    Ok(c) => break Some(c),
                    Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && deneme < 20 => {
                        deneme += 1;
                        tokio::time::sleep(Duration::from_millis(30)).await;
                    }
                    Err(_) => break None,
                }
            };
            let Some(istemci) = istemci else {
                continue;
            };
            let mut pid = 0u32;
            // SAFETY: Açık istemci tutamacı ödünç alınır; çağrı yalnız PID yazar.
            let ok = unsafe {
                windows::Win32::System::Pipes::GetNamedPipeServerProcessId(
                    windows::Win32::Foundation::HANDLE(istemci.as_raw_handle()),
                    &mut pid,
                )
            };
            drop(istemci);
            if ok.is_ok() && pid != 0 {
                return Some((aday.clone(), pid));
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        let _ = adaylar;
        None
    }
}

/// Pipe güvenlik tanımlayıcısı: sahip ve erişim yalnız bu kullanıcı (+ SYSTEM erişimi).
fn pipe_sddl(sid: &str) -> String {
    format!("O:{sid}D:P(A;;GA;;;{sid})(A;;GA;;;SY)")
}

/// Pipe sunucusunun sahibi bu kullanıcı mı (saf karşılaştırma; SID'ler büyük/küçük harf duyarsız).
/// Adaylar: sunucu sürecinin belirteç kullanıcısı ve pipe nesnesinin sahibi. Yükseltilmiş sunucunun
/// belirteci yükseltilmemiş istemciden açılamaz; o zaman pipe sahibi belirleyicidir. Başka bir
/// kullanıcı sahibi bizim SID'imize ayarlayamaz (SeRestorePrivilege gerekir).
fn sunucu_sahibi_uygun_mu(sunucu_sidleri: &[Option<&str>], kendi_sid: Option<&str>) -> bool {
    let Some(kendi) = kendi_sid else {
        return false;
    };
    sunucu_sidleri
        .iter()
        .flatten()
        .any(|s| s.eq_ignore_ascii_case(kendi))
}

#[cfg(windows)]
fn sunucu_sahibini_dogrula(
    istemci: &tokio::net::windows::named_pipe::NamedPipeClient,
    pipe_adi: &str,
) -> Result<(), PipeIstemciHatasi> {
    use std::os::windows::io::AsRawHandle;
    let mut pid = 0u32;
    // SAFETY: Açık istemci tutamacı ödünç alınır; çağrı yalnız PID yazar.
    let ok = unsafe {
        windows::Win32::System::Pipes::GetNamedPipeServerProcessId(
            windows::Win32::Foundation::HANDLE(istemci.as_raw_handle()),
            &mut pid,
        )
    };
    let surec_sahibi = ok
        .ok()
        .and_then(|_| cetvel_depo::surec_kullanici_sid(pid).ok());
    let pipe_sahibi =
        cetvel_depo::nesne_sahibi_sid(windows::Win32::Foundation::HANDLE(istemci.as_raw_handle()))
            .ok();
    let kendi = cetvel_depo::gecerli_kullanici_sid().ok();
    if sunucu_sahibi_uygun_mu(
        &[surec_sahibi.as_deref(), pipe_sahibi.as_deref()],
        kendi.as_deref(),
    ) {
        Ok(())
    } else {
        Err(PipeIstemciHatasi::Diger(format!(
            "Pipe sunucusu bu kullanıcıya ait değil ({pipe_adi}); istek gönderilmedi"
        )))
    }
}

#[cfg(windows)]
async fn pipe_istek_gonder_ic(
    pipe_adi: &str,
    yontem: &str,
    params: Option<Value>,
) -> Result<Value, PipeIstemciHatasi> {
    use tokio::net::windows::named_pipe::ClientOptions;

    const ERROR_FILE_NOT_FOUND: i32 = 2;
    const ERROR_PIPE_BUSY: i32 = 231;
    let mut deneme = 0;
    let istemci = loop {
        match ClientOptions::new().open(pipe_adi) {
            Ok(c) => break c,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && deneme < 10 => {
                deneme += 1;
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            Err(e) if e.raw_os_error() == Some(ERROR_FILE_NOT_FOUND) => {
                return Err(PipeIstemciHatasi::Yok(format!(
                    "Pipe açılamadı ({pipe_adi}): {e}"
                )));
            }
            Err(e) => {
                return Err(PipeIstemciHatasi::Diger(format!(
                    "Pipe açılamadı ({pipe_adi}): {e}"
                )));
            }
        }
    };
    sunucu_sahibini_dogrula(&istemci, pipe_adi)?;
    let diger = PipeIstemciHatasi::Diger;

    let (okuyucu, mut yazici) = tokio::io::split(istemci);
    let mut istek = json!({ "id": 1, "yontem": yontem });
    if let Some(p) = params {
        istek["params"] = p;
    }
    let mut satir = serde_json::to_string(&istek).map_err(|e| diger(e.to_string()))?;
    satir.push('\n');
    yazici
        .write_all(satir.as_bytes())
        .await
        .map_err(|e| diger(format!("Pipe yazma hatası: {e}")))?;
    yazici
        .flush()
        .await
        .map_err(|e| diger(format!("Pipe flush hatası: {e}")))?;

    let mut satir_okuyucu = BufReader::new(okuyucu);
    let mut yanit_satir = String::new();
    satir_okuyucu
        .read_line(&mut yanit_satir)
        .await
        .map_err(|e| diger(format!("Pipe okuma hatası: {e}")))?;
    let yanit: PipeYanit = serde_json::from_str(yanit_satir.trim())
        .map_err(|e| diger(format!("Pipe JSON hatası: {e}")))?;
    if let Some(hata) = yanit.hata {
        return Err(diger(format!("{}: {}", hata.kod, hata.mesaj)));
    }
    yanit
        .sonuc
        .ok_or_else(|| diger("Pipe yanıtında sonuç yok".to_string()))
}

#[cfg(not(windows))]
async fn pipe_istek_gonder_ic(
    _pipe_adi: &str,
    _yontem: &str,
    _params: Option<Value>,
) -> Result<Value, PipeIstemciHatasi> {
    Err(PipeIstemciHatasi::Diger(
        "Named pipe yalnız Windows üzerinde desteklenir".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU64, Ordering};

    use cetvel_depo::Ayarlar;
    use cetvel_model::{Durum, HavuzAnlik, PencereOlcum, UtcZaman};

    use super::*;

    static PIPE_TEST_SAYAC: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn pipe_adi_sid_ile_uretilir_ve_temizlenir() {
        assert_eq!(
            pipe_adi_uret("S-1-5-21-1000"),
            r"\\.\pipe\cetvel-S-1-5-21-1000"
        );
        assert_eq!(pipe_adi_uret("S-1 5\\x"), r"\\.\pipe\cetvel-S-15x");
        assert_eq!(pipe_adi_uret(""), VARSAYILAN_PIPE_ADI);
        assert!(kullanici_pipe_adi().starts_with(r"\\.\pipe\cetvel"));
    }

    #[test]
    fn pipe_adaylari_baglanti_sonra_sid_sonra_eski_ad_sirasinda() {
        let sidli = r"\\.\pipe\cetvel-S-1-5-21-9";
        assert_eq!(
            pipe_adaylari_ile(Some(r"\\.\pipe\cetvel-S-1-5-21-8"), sidli),
            vec![
                r"\\.\pipe\cetvel-S-1-5-21-8".to_string(),
                sidli.to_string(),
                VARSAYILAN_PIPE_ADI.to_string()
            ]
        );
        // Eski servisin baglanti.json'u sabit adı yazar: önce o, tekrar eklenmez.
        assert_eq!(
            pipe_adaylari_ile(Some(VARSAYILAN_PIPE_ADI), sidli),
            vec![VARSAYILAN_PIPE_ADI.to_string(), sidli.to_string()]
        );
        // Cetvel dışı / bozuk ad yok sayılır.
        assert_eq!(
            pipe_adaylari_ile(Some(r"\\.\pipe\baskasi"), sidli),
            vec![sidli.to_string(), VARSAYILAN_PIPE_ADI.to_string()]
        );
        assert_eq!(pipe_adaylari_ile(Some("  "), sidli).len(), 2);
        assert_eq!(pipe_adaylari_ile(Some(r"\\.\pipe\cetvel-"), sidli).len(), 2);
        assert!(pipe_adaylari(None).len() >= 2);
        assert!(sunucu_sahibi_uygun_mu(
            &[Some("S-1-5-21-1")],
            Some("s-1-5-21-1")
        ));
        assert!(!sunucu_sahibi_uygun_mu(
            &[Some("S-1-5-21-2")],
            Some("S-1-5-21-1")
        ));
        assert!(!sunucu_sahibi_uygun_mu(&[None], Some("S-1-5-21-1")));
        // Yükseltilmiş sunucu: süreç belirteci okunamaz, pipe sahibi bizim SID'imiz -> kabul.
        assert!(sunucu_sahibi_uygun_mu(
            &[None, Some("S-1-5-21-1")],
            Some("S-1-5-21-1")
        ));
        // Başka kullanıcı (belirteç de sahip de farklı) ya da sahibi Administrators -> ret.
        assert!(!sunucu_sahibi_uygun_mu(
            &[Some("S-1-5-21-2"), Some("S-1-5-32-544")],
            Some("S-1-5-21-1")
        ));
        assert!(!sunucu_sahibi_uygun_mu(&[Some("S-1-5-21-1")], None));
    }

    #[tokio::test]
    async fn sunucu_pid_mesgul_pipe_bosalinca_bulunur() {
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
        let no = PIPE_TEST_SAYAC.fetch_add(1, Ordering::Relaxed);
        let ad = format!(r"\\.\pipe\cetvel-test-mesgul-{}-{no}", std::process::id());
        let sunucu = ServerOptions::new()
            .first_pipe_instance(true)
            .max_instances(1)
            .create(&ad)
            .expect("tek ornekli pipe");
        let ilk = ClientOptions::new().open(&ad).expect("ilk istemci");
        sunucu.connect().await.expect("baglanti");
        // Tek örnek dolu: ikinci açılış ERROR_PIPE_BUSY alır.
        assert_eq!(
            ClientOptions::new().open(&ad).unwrap_err().raw_os_error(),
            Some(231)
        );
        let bosalt = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(ilk);
            sunucu.disconnect().expect("ayir");
            let _ = sunucu.connect().await;
            sunucu
        });
        let bulunan = pipe_sunucu_pid(std::slice::from_ref(&ad)).await;
        assert_eq!(bulunan, Some((ad.clone(), std::process::id())));
        bosalt.abort();
    }

    #[tokio::test]
    async fn pipe_sahibi_sddl_ile_kullanici_sidi_olur() {
        use std::os::windows::io::AsRawHandle;
        use tokio::net::windows::named_pipe::ClientOptions;
        let no = PIPE_TEST_SAYAC.fetch_add(1, Ordering::Relaxed);
        let ad = format!(r"\\.\pipe\cetvel-test-sahip-{}-{no}", std::process::id());
        let sid = cetvel_depo::gecerli_kullanici_sid().expect("sid");
        let sddl = pipe_sddl(&sid);
        assert!(sddl.starts_with(&format!("O:{sid}D:P(")));
        let sunucu = guvenli_pipe_olustur(&ad, &sddl, true).expect("guvenli pipe");
        let istemci = ClientOptions::new().open(&ad).expect("istemci");
        let sahip = cetvel_depo::nesne_sahibi_sid(windows::Win32::Foundation::HANDLE(
            istemci.as_raw_handle(),
        ))
        .expect("sahip sid");
        assert!(sahip.eq_ignore_ascii_case(&sid));
        assert!(sunucu_sahibini_dogrula(&istemci, &ad).is_ok());
        drop(istemci);
        drop(sunucu);
    }

    #[tokio::test]
    async fn kesif_yok_olan_adi_atlar_canli_pipe_a_ulasir_ve_sunucu_pid_bulunur() {
        let no = PIPE_TEST_SAYAC.fetch_add(1, Ordering::Relaxed);
        let canli = format!(r"\\.\pipe\cetvel-test-kesif-{}-{no}", std::process::id());
        let yok = format!(r"\\.\pipe\cetvel-test-yok-{}-{no}", std::process::id());
        let durum = Arc::new(PaylasilanServisDurumu::yeni(
            Ayarlar::default(),
            None,
            "j",
            &canli,
            None,
        ));
        let (kap_tx, kap_rx) = watch::channel(false);
        let gorev = pipe_sunucusu_baslat(Arc::clone(&durum), &canli, kap_rx)
            .await
            .expect("pipe");

        let adaylar = vec![yok.clone(), canli.clone()];
        let (ad, saglik) = pipe_istek_kesifle(&adaylar, "saglik", None)
            .await
            .expect("keşif");
        assert_eq!(ad, canli);
        assert_eq!(saglik["durum"], "ok");
        assert_eq!(
            pipe_sunucu_pid(&adaylar).await,
            Some((canli.clone(), std::process::id()))
        );
        assert!(
            pipe_istek_kesifle(std::slice::from_ref(&yok), "saglik", None)
                .await
                .unwrap_err()
                .contains("bulunamadı")
        );
        assert_eq!(pipe_sunucu_pid(std::slice::from_ref(&yok)).await, None);

        let _ = kap_tx.send(true);
        let _ = tokio::time::timeout(Duration::from_secs(2), gorev).await;
    }

    #[tokio::test]
    async fn pipe_anlik_al_ve_saglik_komutlarini_yanitlar() {
        let no = PIPE_TEST_SAYAC.fetch_add(1, Ordering::Relaxed);
        let pipe_adi = format!(r"\\.\pipe\cetvel-test-{}-{no}", std::process::id());

        let durum = Arc::new(PaylasilanServisDurumu::yeni(
            Ayarlar::default(),
            None,
            "pipe-test-jeton",
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
                    kullanilan: 0.1,
                    kalan: 0.9,
                    sifirlama: Some(simdi.ekle_sn(3600)),
                },
            )]),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        codex.hesapla_etkin_durum();
        durum
            .anlik_yayinla(
                cetvel_model::AnlikV2::yeni(
                    simdi,
                    3,
                    BTreeMap::from([("codex_plus".to_string(), codex)]),
                ),
                simdi,
            )
            .await;

        let (kap_tx, kap_rx) = watch::channel(false);
        let gorev = pipe_sunucusu_baslat(Arc::clone(&durum), &pipe_adi, kap_rx)
            .await
            .expect("test pipe baslamali");

        let saglik = pipe_istek_gonder(&pipe_adi, "saglik", None)
            .await
            .expect("saglik yaniti");
        assert_eq!(saglik["durum"], "ok");
        assert_eq!(saglik["sira"], 3);

        let anlik = pipe_istek_gonder(&pipe_adi, "anlik.al", None)
            .await
            .expect("anlik yaniti");
        assert_eq!(anlik["sira"], 3);
        assert_eq!(anlik["havuzlar"]["codex_plus"]["etkin_kalan"], 0.9);

        let ajanlar = pipe_istek_gonder(&pipe_adi, "ajanlar.al", None)
            .await
            .expect("ajanlar yaniti");
        assert_eq!(ajanlar["surum"], 1);
        assert!(ajanlar["ajanlar"].is_array());

        // Yenile: ilk istek kabul, aynı havuz dakikada bir (hız sınırı)
        let y1 = pipe_istek_gonder(&pipe_adi, "yenile", Some(json!({ "havuz": "codex_plus" })))
            .await
            .expect("yenile");
        assert_eq!(y1["istendi"], true);
        let y2 = pipe_istek_gonder(&pipe_adi, "yenile", Some(json!({ "havuz": "codex_plus" })))
            .await
            .expect("yenile 2");
        assert_eq!(y2["istendi"], false);
        assert!(y2["bekle_sn"].as_u64().is_some_and(|b| b > 0 && b <= 60));
        assert!(durum.yenile_hedeflerini_al().contains("codex_plus"));

        // Desteklenmeyen platform testi hata döner; ayar.yenile kabul edilir
        assert!(
            pipe_istek_gonder(
                &pipe_adi,
                "bagla.test",
                Some(json!({ "platform": "gemini" }))
            )
            .await
            .is_err()
        );
        let ay = pipe_istek_gonder(&pipe_adi, "ayar.yenile", None)
            .await
            .expect("ayar yenile");
        assert_eq!(ay["istendi"], true);

        let _ = kap_tx.send(true);
        let _ = tokio::time::timeout(Duration::from_millis(500), gorev).await;
    }

    #[tokio::test]
    async fn yanitsiz_pipe_istemcisi_zaman_asimiyla_doner() {
        use tokio::net::windows::named_pipe::ServerOptions;
        let no = PIPE_TEST_SAYAC.fetch_add(1, Ordering::Relaxed);
        let pipe_adi = format!(r"\\.\pipe\cetvel-test-askida-{}-{no}", std::process::id());
        let sunucu = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_adi)
            .expect("test borusu");
        // Bağlantıyı kabul eder ama hiç yanıt vermez (askıdaki servis).
        let askida = tokio::spawn(async move {
            let _ = sunucu.connect().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
            drop(sunucu);
        });
        let bas = std::time::Instant::now();
        let sonuc =
            pipe_istek_gonder_sureli(&pipe_adi, "anlik.al", None, Duration::from_millis(300)).await;
        assert!(sonuc.unwrap_err().contains("zaman aşımı"));
        assert!(bas.elapsed() < Duration::from_secs(3));
        assert_eq!(pipe_zaman_asimi("bagla.test"), Duration::from_secs(95));
        assert_eq!(pipe_zaman_asimi("anlik.al"), Duration::from_secs(5));
        askida.abort();
    }

    #[tokio::test]
    async fn satir_sinirini_asan_istek_bellege_tamamen_alinmadan_reddedilir() {
        use tokio::io::AsyncReadExt;
        use tokio::net::windows::named_pipe::ClientOptions;
        let no = PIPE_TEST_SAYAC.fetch_add(1, Ordering::Relaxed);
        let pipe_adi = format!(r"\\.\pipe\cetvel-test-buyuk-{}-{no}", std::process::id());
        let durum = Arc::new(PaylasilanServisDurumu::yeni(
            Ayarlar::default(),
            None,
            "j",
            &pipe_adi,
            None,
        ));
        let (kap_tx, kap_rx) = watch::channel(false);
        let gorev = pipe_sunucusu_baslat(Arc::clone(&durum), &pipe_adi, kap_rx)
            .await
            .expect("pipe");
        let istemci = ClientOptions::new().open(&pipe_adi).expect("aç");
        let (mut okuyucu, mut yazici) = tokio::io::split(istemci);
        // Satır sonu olmadan 64 KB gönder: sunucu 8 KB + 1'de keser ve hata yanıtı döner.
        let veri = vec![b'a'; 64 * 1024];
        let yazma = tokio::spawn(async move {
            let _ = yazici.write_all(&veri).await;
        });
        let mut yanit = Vec::new();
        let mut parca = [0u8; 512];
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            while let Ok(n) = okuyucu.read(&mut parca).await {
                if n == 0 {
                    break;
                }
                yanit.extend_from_slice(&parca[..n]);
                if yanit.contains(&10u8) {
                    break;
                }
            }
        })
        .await;
        yazma.abort();
        let metin = String::from_utf8_lossy(&yanit);
        assert!(metin.contains("cok_buyuk"), "yanıt: {metin}");
        let _ = kap_tx.send(true);
        let _ = tokio::time::timeout(Duration::from_millis(500), gorev).await;
    }
}
