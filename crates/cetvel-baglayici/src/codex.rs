use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use cetvel_depo::CodexAyari;
use cetvel_model::{
    AltSayac, Durum, EkBilgi, GuvenEtiketi, HataBilgisi, HavuzAnlik, KaynakBilgisi, KaynakTuru,
    PencereOlcum, SifirlamaKredisi, UtcZaman, pencere_adi_dakikadan,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::job_object::JobSuresKorumasi;
use crate::{Baglayici, BaglayiciSonucu};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
struct OnbellekIkili {
    yol: PathBuf,
    mtime: Option<SystemTime>,
    sema_dogrulandi: bool,
}

/// `codex.exe app-server` üzerinden Codex kota okuyucusu (`plan.md §6.2`).
pub struct CodexBaglayicisi {
    ayar: CodexAyari,
    onbellek: Mutex<Option<OnbellekIkili>>,
}

impl CodexBaglayicisi {
    pub fn yeni(ayar: CodexAyari) -> Self {
        Self {
            ayar,
            onbellek: Mutex::new(None),
        }
    }

    /// Önbelleği kontrol ederek `codex.exe` yolunu çözer; sürüm/mtime değişmişse yeniler.
    pub fn ikili_yolu_coz(&self) -> Option<(PathBuf, bool)> {
        let mut guard = self.onbellek.lock().ok()?;
        if let Some(mevcut) = guard.as_ref()
            && mevcut.yol.exists()
        {
            let guncel_mtime = std::fs::metadata(&mevcut.yol)
                .and_then(|m| m.modified())
                .ok();
            if guncel_mtime == mevcut.mtime {
                return Some((mevcut.yol.clone(), !mevcut.sema_dogrulandi));
            }
        }
        let yeni_yol = codex_ikili_coz()?;
        let mtime = std::fs::metadata(&yeni_yol).and_then(|m| m.modified()).ok();
        *guard = Some(OnbellekIkili {
            yol: yeni_yol.clone(),
            mtime,
            sema_dogrulandi: false,
        });
        Some((yeni_yol, true))
    }

    fn sema_dogrulandi_isaretle(&self) {
        if let Ok(mut guard) = self.onbellek.lock()
            && let Some(mevcut) = guard.as_mut()
        {
            mevcut.sema_dogrulandi = true;
        }
    }
}

impl Baglayici for CodexBaglayicisi {
    fn havuz_id(&self) -> &str {
        "codex_plus"
    }

    fn oku<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = BaglayiciSonucu> + Send + 'a>> {
        Box::pin(async move {
            if !self.ayar.etkin {
                return BaglayiciSonucu {
                    havuz_id: self.havuz_id().to_string(),
                    havuz: codex_hata_havuzu(
                        Durum::Kapali,
                        "devre_disi",
                        None,
                        vec!["codex devre dışı".into()],
                    ),
                    retry_after_sn: None,
                    gecici_hata: false,
                };
            }

            if let Some((ikili_yol, surum_degisti)) = self.ikili_yolu_coz() {
                if self.ayar.sema_bekcisi && surum_degisti {
                    match codex_sema_kontrol(&ikili_yol, Duration::from_secs(6)).await {
                        SemaKontrolSonucu::Uygun => self.sema_dogrulandi_isaretle(),
                        SemaKontrolSonucu::SemaDegisti => {
                            return BaglayiciSonucu {
                                havuz_id: self.havuz_id().to_string(),
                                havuz: codex_hata_havuzu(
                                    Durum::Hata,
                                    "sema_degisti",
                                    None,
                                    vec![
                                        "codex şeması beklenen rateLimits alanlarını içermiyor"
                                            .into(),
                                    ],
                                ),
                                retry_after_sn: None,
                                gecici_hata: true,
                            };
                        }
                        SemaKontrolSonucu::Atlandi => {
                            self.sema_dogrulandi_isaretle();
                        }
                    }
                }

                let asim = Duration::from_secs(self.ayar.zaman_asimi_sn.max(1));
                match codex_app_server_oku(&ikili_yol, asim).await {
                    Ok(sonuc_degeri) => {
                        let havuz = codex_yanit_ayristir(&sonuc_degeri, simdi);
                        let gecici_hata = havuz.durum == Durum::Hata;
                        return BaglayiciSonucu {
                            havuz_id: self.havuz_id().to_string(),
                            havuz,
                            retry_after_sn: None,
                            gecici_hata,
                        };
                    }
                    Err("giris_gerek") => {
                        // Oturum kapalı: eski rollout yedeği "güncel" gibi gösterilmez.
                        return BaglayiciSonucu {
                            havuz_id: self.havuz_id().to_string(),
                            havuz: codex_giris_gerek_havuzu(),
                            retry_after_sn: None,
                            gecici_hata: false,
                        };
                    }
                    Err(err_tur) => {
                        if self.ayar.rollout_yedek
                            && let Some(yedek) = rollout_yedek_oku_arka_planda(simdi).await
                        {
                            return BaglayiciSonucu {
                                havuz_id: self.havuz_id().to_string(),
                                havuz: yedek,
                                retry_after_sn: None,
                                gecici_hata: true,
                            };
                        }
                        return BaglayiciSonucu {
                            havuz_id: self.havuz_id().to_string(),
                            havuz: codex_hata_havuzu(
                                Durum::Hata,
                                err_tur,
                                None,
                                vec!["codex app-server okunamadı".into()],
                            ),
                            retry_after_sn: None,
                            gecici_hata: true,
                        };
                    }
                }
            }

            if self.ayar.rollout_yedek
                && let Some(yedek) = rollout_yedek_oku_arka_planda(simdi).await
            {
                return BaglayiciSonucu {
                    havuz_id: self.havuz_id().to_string(),
                    havuz: yedek,
                    retry_after_sn: None,
                    gecici_hata: true,
                };
            }

            BaglayiciSonucu {
                havuz_id: self.havuz_id().to_string(),
                havuz: codex_hata_havuzu(
                    Durum::Baglanmadi,
                    "ikili_bulunamadi",
                    None,
                    vec!["codex.exe kurulumu bulunamadı".into()],
                ),
                retry_after_sn: None,
                gecici_hata: false,
            }
        })
    }
}

/// HKCU AppModel kaydından, ardından `%LOCALAPPDATA%\OpenAI\Codex\bin` ve `PATH` içinden `codex.exe` yolunu çözer.
pub fn codex_ikili_coz() -> Option<PathBuf> {
    if let Some(appmodel_yol) = appmodel_codex_coz() {
        return Some(appmodel_yol);
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let bin_kok = PathBuf::from(local_app_data)
            .join("OpenAI")
            .join("Codex")
            .join("bin");
        if let Ok(girdiler) = std::fs::read_dir(&bin_kok) {
            let mut adaylar: Vec<(String, PathBuf)> = Vec::new();
            for girdi in girdiler.flatten() {
                let aday = girdi.path().join("codex.exe");
                if aday.is_file() {
                    adaylar.push((girdi.file_name().to_string_lossy().to_string(), aday));
                }
            }
            if let Some(secilen) = adaylari_surume_gore_sec(adaylar) {
                return Some(secilen);
            }
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dizin in std::env::split_paths(&path_var) {
            let aday = dizin.join("codex.exe");
            if aday.is_file() {
                return Some(aday);
            }
        }
    }

    None
}

fn appmodel_codex_coz() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let paketler = hkcu
        .open_subkey(r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages")
        .ok()?;

    let mut adaylar = Vec::new();
    for alt_ad in paketler.enum_keys().flatten() {
        if !alt_ad.starts_with("OpenAI.Codex_") {
            continue;
        }
        if let Ok(alt_anahtar) = paketler.open_subkey(&alt_ad)
            && let Ok(kok) = alt_anahtar.get_value::<String, _>("PackageRootFolder")
        {
            let aday = PathBuf::from(kok)
                .join("app")
                .join("resources")
                .join("codex.exe");
            if aday.is_file() {
                adaylar.push((alt_ad, aday));
            }
        }
    }

    adaylari_surume_gore_sec(adaylar)
}

/// Paket veya klasör adlarından en yüksek sürümü seçer.
pub fn adaylari_surume_gore_sec(mut adaylar: Vec<(String, PathBuf)>) -> Option<PathBuf> {
    if adaylar.is_empty() {
        return None;
    }
    adaylar.sort_by(|(a_ad, a_yol), (b_ad, b_yol)| {
        let a_surum = surum_parcalari_cikar(a_ad);
        let b_surum = surum_parcalari_cikar(b_ad);
        a_surum.cmp(&b_surum).then_with(|| a_yol.cmp(b_yol))
    });
    adaylar.pop().map(|(_, yol)| yol)
}

fn surum_parcalari_cikar(ad: &str) -> Vec<u64> {
    let mut parcalar = Vec::new();
    for token in ad.split(|c: char| !c.is_ascii_digit()) {
        if !token.is_empty()
            && let Ok(n) = token.parse::<u64>()
        {
            parcalar.push(n);
        }
    }
    parcalar
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemaKontrolSonucu {
    Uygun,
    SemaDegisti,
    Atlandi,
}

async fn codex_sema_kontrol(ikili_yol: &Path, zaman_asimi: Duration) -> SemaKontrolSonucu {
    let benzersiz = format!(
        "cetvel-codex-sema-{}-{}",
        std::process::id(),
        UtcZaman::simdi().as_offset().unix_timestamp()
    );
    let gecici_dizin = std::env::temp_dir().join(benzersiz);
    if std::fs::create_dir_all(&gecici_dizin).is_err() {
        return SemaKontrolSonucu::Atlandi;
    }

    let sonuc = tokio::time::timeout(zaman_asimi, async {
        let mut cmd = Command::new(ikili_yol);
        cmd.arg("app-server")
            .arg("generate-json-schema")
            .arg("--out")
            .arg(&gecici_dizin)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);

        let Ok(mut child) = cmd.spawn() else {
            return SemaKontrolSonucu::Atlandi;
        };
        let job = JobSuresKorumasi::yeni().ok();
        if let (Some(j), Some(h)) = (&job, child.raw_handle()) {
            let _ = j.sureci_ekle(h);
        }
        let status = child.wait().await;
        drop(job);

        match status {
            Ok(s) if s.success() => {
                if sema_dizini_dogrula(&gecici_dizin) {
                    SemaKontrolSonucu::Uygun
                } else {
                    SemaKontrolSonucu::SemaDegisti
                }
            }
            _ => SemaKontrolSonucu::Atlandi,
        }
    })
    .await
    .unwrap_or(SemaKontrolSonucu::Atlandi);

    let _ = std::fs::remove_dir_all(&gecici_dizin);
    sonuc
}

/// Üretilen JSON şema dizininde `rateLimits` alanlarının varlığını denetler.
pub fn sema_dizini_dogrula(dizin: &Path) -> bool {
    let mut dosya_sayisi = 0usize;
    let mut yigin = vec![dizin.to_path_buf()];
    while let Some(mevcut) = yigin.pop() {
        let Ok(girdiler) = std::fs::read_dir(mevcut) else {
            continue;
        };
        for girdi in girdiler.flatten() {
            let yol = girdi.path();
            if yol.is_dir() {
                yigin.push(yol);
            } else if yol.extension().and_then(|e| e.to_str()) == Some("json") {
                dosya_sayisi += 1;
                if let Ok(icerik) = std::fs::read_to_string(&yol)
                    && icerik.contains("rateLimits")
                    && icerik.contains("usedPercent")
                    && icerik.contains("windowDurationMins")
                {
                    return true;
                }
            }
        }
    }
    dosya_sayisi == 0
}

/// `codex.exe app-server` sürecini stdio JSON-RPC üzerinden sorgular ve Job Object ile sonlandırır.
pub async fn codex_app_server_oku(
    ikili_yol: &Path,
    zaman_asimi: Duration,
) -> Result<Value, &'static str> {
    let mut cmd = Command::new(ikili_yol);
    cmd.arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn().map_err(|_| "surec_baslatilamadi")?;
    let job = JobSuresKorumasi::yeni().ok();
    if let (Some(j), Some(h)) = (&job, child.raw_handle()) {
        let _ = j.sureci_ekle(h);
    }

    let okuma_sonucu = tokio::time::timeout(zaman_asimi, async {
        let mut stdin = child.stdin.take().ok_or("stdin_yok")?;
        let stdout = child.stdout.take().ok_or("stdout_yok")?;
        let mut satirlar = BufReader::new(stdout).lines();

        let init_istek = json!({
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "cetvel",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });
        satir_yaz(&mut stdin, &init_istek).await?;
        yanit_bekle(&mut satirlar, 1).await?;

        let initialized = json!({ "method": "initialized" });
        satir_yaz(&mut stdin, &initialized).await?;

        let oku_istek = json!({
            "id": 2,
            "method": "account/rateLimits/read",
            "params": {
                "excludeResetCreditDetails": false
            }
        });
        satir_yaz(&mut stdin, &oku_istek).await?;
        yanit_bekle(&mut satirlar, 2).await
    })
    .await;

    if let Some(j) = job {
        j.sonlandir();
        drop(j);
    }
    let _ = child.kill().await;
    let _ = tokio::time::timeout(Duration::from_millis(500), child.wait()).await;

    match okuma_sonucu {
        Ok(ic) => ic,
        Err(_) => Err("zaman_asimi"),
    }
}

async fn satir_yaz(
    stdin: &mut tokio::process::ChildStdin,
    deger: &Value,
) -> Result<(), &'static str> {
    let mut metin = serde_json::to_string(deger).map_err(|_| "serilestirme_hatasi")?;
    metin.push('\n');
    stdin
        .write_all(metin.as_bytes())
        .await
        .map_err(|_| "yazma_hatasi")?;
    stdin.flush().await.map_err(|_| "yazma_hatasi")
}

async fn yanit_bekle(
    satirlar: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    beklenen_id: i64,
) -> Result<Value, &'static str> {
    while let Some(satir) = satirlar.next_line().await.map_err(|_| "okuma_hatasi")? {
        let temiz = satir.trim();
        if temiz.is_empty() {
            continue;
        }
        let Ok(deger) = serde_json::from_str::<Value>(temiz) else {
            // JSON olmayan günlük satırları atlanır (`plan.md §6.2`).
            continue;
        };
        let Some(id_val) = deger.get("id").and_then(Value::as_i64) else {
            continue;
        };
        if id_val != beklenen_id {
            continue;
        }
        if let Some(hata) = deger.get("error").filter(|e| !e.is_null()) {
            return Err(codex_rpc_hata_turu(hata));
        }
        return deger.get("result").cloned().ok_or("sema_degisti");
    }
    Err("beklenmeyen_cikis")
}

/// app-server JSON-RPC hata nesnesini sınıflandırır: oturum kapalı / kimlik doğrulama gerekli
/// durumları `giris_gerek`, diğerleri `rpc_hatasi`. Ham hata metni yalnız sınıflandırmada
/// kullanılır, anlığa yazılmaz.
pub fn codex_rpc_hata_turu(hata: &Value) -> &'static str {
    const ISARETLER: [&str; 12] = [
        "not logged in",
        "not authenticated",
        "unauthenticated",
        "unauthorized",
        "authentication required",
        "auth required",
        "login required",
        "please log in",
        "please login",
        "sign in",
        "codex login",
        "no auth",
    ];
    let mut metin = String::new();
    for alan in ["message", "data", "code"] {
        if let Some(v) = hata.get(alan) {
            metin.push_str(&v.to_string().to_ascii_lowercase());
            metin.push(' ');
        }
    }
    if hata.is_string() {
        metin.push_str(&hata.to_string().to_ascii_lowercase());
    }
    let kod_401 = hata.get("code").and_then(Value::as_i64) == Some(401)
        || metin.contains("\"401\"")
        || metin.contains(" 401 ");
    if kod_401 || ISARETLER.iter().any(|i| kelime_olarak_iceriyor(&metin, i)) {
        "giris_gerek"
    } else {
        "rpc_hatasi"
    }
}

/// `isaret` metinde kelime sınırlarıyla geçiyor mu ("design in" içindeki "sign in" ya da
/// "no authority" içindeki "no auth" eşleşmez). Sınır: ASCII harf/rakam olmayan karakter.
fn kelime_olarak_iceriyor(metin: &str, isaret: &str) -> bool {
    let sinir = |c: Option<char>| c.is_none_or(|c| !c.is_ascii_alphanumeric());
    metin.match_indices(isaret).any(|(bas, _)| {
        sinir(metin[..bas].chars().next_back()) && sinir(metin[bas + isaret.len()..].chars().next())
    })
}

/// Codex oturumu kapalıyken üretilen havuz: `baglanmadi` (servis 10 dk'da bir yeniden dener;
/// kullanıcı `codex login` yapınca kendiliğinden düzelir), hata türü `giris_gerek`.
fn codex_giris_gerek_havuzu() -> HavuzAnlik {
    let mut h = codex_hata_havuzu(
        Durum::Baglanmadi,
        "giris_gerek",
        None,
        vec!["Codex oturumu kapalı: terminalde `codex login` ile giriş yapın".into()],
    );
    h.hata = Some(HataBilgisi {
        tur: "giris_gerek".to_string(),
        http_kodu: None,
    });
    h
}

#[derive(Debug, Deserialize)]
struct HamCodexSonuc {
    #[serde(rename = "rateLimits", alias = "rate_limits")]
    rate_limits: Option<HamRateLimitGrubu>,
    #[serde(
        rename = "rateLimitsByLimitId",
        alias = "rate_limits_by_limit_id",
        default
    )]
    rate_limits_by_limit_id: BTreeMap<String, HamRateLimitGrubu>,
}

#[derive(Debug, Deserialize)]
struct HamRateLimitGrubu {
    primary: Option<HamPencere>,
    secondary: Option<HamPencere>,
    #[serde(rename = "limitId", alias = "limit_id")]
    limit_id: Option<String>,
    #[serde(rename = "planType", alias = "plan_type")]
    plan_type: Option<String>,
    credits: Option<HamCredits>,
    #[serde(rename = "rateLimitResetCredits", alias = "rate_limit_reset_credits")]
    rate_limit_reset_credits: Option<HamResetCredits>,
    #[serde(rename = "ordinaryUsageAllowed", alias = "ordinary_usage_allowed")]
    ordinary_usage_allowed: Option<bool>,
    #[serde(rename = "rateLimitReachedType", alias = "rate_limit_reached_type")]
    rate_limit_reached_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HamPencere {
    #[serde(rename = "usedPercent", alias = "used_percent")]
    used_percent: Option<f64>,
    #[serde(
        rename = "windowDurationMins",
        alias = "window_duration_mins",
        alias = "window_minutes"
    )]
    window_duration_mins: Option<u64>,
    #[serde(rename = "resetsAt", alias = "resets_at")]
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct HamCredits {
    #[serde(rename = "hasCredits", alias = "has_credits")]
    _has_credits: Option<bool>,
    unlimited: Option<bool>,
    balance: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct HamResetCredits {
    #[serde(rename = "availableCount", alias = "available_count")]
    available_count: Option<u64>,
    #[serde(rename = "expiresAt", alias = "expires_at")]
    expires_at: Option<Value>,
}

/// `account/rateLimits/read` JSON sonucunu `HavuzAnlik` yapısına ayrıştırır.
pub fn codex_yanit_ayristir(sonuc: &Value, simdi: UtcZaman) -> HavuzAnlik {
    let Ok(ham) = serde_json::from_value::<HamCodexSonuc>(sonuc.clone()) else {
        return codex_hata_havuzu(
            Durum::Hata,
            "sema_degisti",
            None,
            vec!["codex yanıtı beklenmeyen yapıda".into()],
        );
    };

    let ana_grup = ham
        .rate_limits
        .as_ref()
        .or_else(|| ham.rate_limits_by_limit_id.get("codex"))
        .or_else(|| ham.rate_limits_by_limit_id.values().next());

    let Some(grup) = ana_grup else {
        return codex_hata_havuzu(
            Durum::Hata,
            "sema_degisti",
            None,
            vec!["codex rateLimits nesnesi boş".into()],
        );
    };

    let mut pencereler = BTreeMap::new();
    pencere_ekle(&mut pencereler, grup.primary.as_ref());
    pencere_ekle(&mut pencereler, grup.secondary.as_ref());

    if pencereler.is_empty() {
        return codex_hata_havuzu(
            Durum::Hata,
            "sema_degisti",
            None,
            vec!["codex yanıtında geçerli pencere alanı bulunamadı".into()],
        );
    }

    let mut alt_sayaclar = BTreeMap::new();
    let ana_id = grup.limit_id.as_deref().unwrap_or("codex");
    for (limit_id, alt_grup) in &ham.rate_limits_by_limit_id {
        if limit_id == ana_id {
            continue;
        }
        if let Some(p) = &alt_grup.primary
            && let Some(used_pct) = p.used_percent
        {
            let kullanilan = (used_pct / 100.0).clamp(0.0, 1.0);
            let sifirlama = if used_pct <= 0.0 {
                None
            } else {
                p.resets_at.and_then(UtcZaman::from_unix_timestamp)
            };
            alt_sayaclar.insert(
                limit_id.clone(),
                AltSayac {
                    kullanilan,
                    kalan: Some((1.0 - kullanilan).clamp(0.0, 1.0)),
                    uzunluk_dk: p.window_duration_mins,
                    sifirlama,
                },
            );
        }
    }

    let mut ek = EkBilgi {
        olagan_kullanim: grup.ordinary_usage_allowed,
        limit_turu: grup.rate_limit_reached_type.clone(),
        plan_turu: grup.plan_type.clone(),
        ..Default::default()
    };

    if let Some(credits) = &grup.credits {
        ek.sinirsiz_kredi = credits.unlimited;
        ek.kredi_bakiye = credits.balance.as_ref().and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        });
    }

    if let Some(reset_credits) = &grup.rate_limit_reset_credits
        && let Some(adet) = reset_credits.available_count
    {
        let son = reset_credits.expires_at.as_ref().and_then(|v| match v {
            Value::Number(n) => n.as_i64().and_then(UtcZaman::from_unix_timestamp),
            Value::String(s) => UtcZaman::parse_tarih_veya_zaman(s).ok(),
            _ => None,
        });
        ek.sifirlama_kredisi = Some(SifirlamaKredisi { adet, son });
    }

    let mut havuz = HavuzAnlik {
        platform: "codex".to_string(),
        etiket: "Codex".to_string(),
        durum: Durum::Ok,
        etkin_kalan: None,
        baglayici_pencere: None,
        kullanilabilir: None,
        kaynak: Some(KaynakBilgisi {
            tur: KaynakTuru::CodexAppServer,
            guven: GuvenEtiketi::ResmiArayuz,
            okuma: simdi,
            yas_sn: 0,
            hassasiyet: None,
        }),
        pencereler,
        alt_sayaclar,
        tahmin: BTreeMap::new(),
        baski: None,
        bakiye: None,
        ek: Some(ek),
        hata: None,
        notlar: Vec::new(),
    };
    havuz.hesapla_etkin_durum();
    havuz
}

fn pencere_ekle(hedef: &mut BTreeMap<String, PencereOlcum>, pencere: Option<&HamPencere>) {
    let Some(p) = pencere else { return };
    let (Some(used_pct), Some(uzunluk_dk)) = (p.used_percent, p.window_duration_mins) else {
        return;
    };
    if !used_pct.is_finite() || uzunluk_dk == 0 {
        return;
    }
    let kullanilan = (used_pct / 100.0).clamp(0.0, 1.0);
    let kalan = (1.0 - kullanilan).clamp(0.0, 1.0);
    // `plan.md §6.2`: usedPercent == 0 iken resetsAt kayar; sıfırlama alanı yazılmaz.
    let sifirlama = if used_pct <= 0.0 {
        None
    } else {
        p.resets_at.and_then(UtcZaman::from_unix_timestamp)
    };
    let ad = pencere_adi_dakikadan(uzunluk_dk);
    hedef.insert(
        ad,
        PencereOlcum {
            uzunluk_dk,
            kullanilan,
            kalan,
            sifirlama,
        },
    );
}

/// Rollout dosyasının önce son 512 KB'ı, kayıt bulunmazsa son 8 MB'ı okunur (dosya yüzlerce MB
/// olabilir; tamamı belleğe alınmaz).
const ROLLOUT_KUYRUK_ADIMLARI: [u64; 2] = [512 * 1024, 8 * 1024 * 1024];

/// `~/.codex/sessions/**/rollout-*.jsonl` içindeki son `token_count.rate_limits` kaydını yedek olarak okur.
pub fn rollout_yedek_oku(simdi: UtcZaman) -> Option<HavuzAnlik> {
    let userprofile = std::env::var_os("USERPROFILE")?;
    let sessions_kok = PathBuf::from(userprofile).join(".codex").join("sessions");
    let en_yeni = en_yeni_rollout_dosyasi(&sessions_kok)?;
    rollout_dosyasindan_oku(&en_yeni, simdi)
}

/// [`rollout_yedek_oku`]'yu tokio çalışan iş parçacığını bloklamadan (`spawn_blocking`) çalıştırır:
/// dizin taraması ve dosya okuması diğer bağlayıcıları ve "Test et" yanıtlarını geciktirmez.
pub async fn rollout_yedek_oku_arka_planda(simdi: UtcZaman) -> Option<HavuzAnlik> {
    tokio::task::spawn_blocking(move || rollout_yedek_oku(simdi))
        .await
        .ok()
        .flatten()
}

fn rollout_dosyasindan_oku(yol: &Path, simdi: UtcZaman) -> Option<HavuzAnlik> {
    let uzunluk = std::fs::metadata(yol).ok()?.len();
    for azami in ROLLOUT_KUYRUK_ADIMLARI {
        let metin = dosya_kuyrugu_oku(yol, azami)?;
        if let Some(h) = codex_rollout_ayristir(&metin, simdi) {
            return Some(h);
        }
        if azami >= uzunluk {
            break;
        }
    }
    None
}

/// Dosyanın son `azami` baytını okur; ortadan başlandıysa ilk (yarım) satır atılır.
fn dosya_kuyrugu_oku(yol: &Path, azami: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut dosya = std::fs::File::open(yol).ok()?;
    let uzunluk = dosya.metadata().ok()?.len();
    let baslangic = uzunluk.saturating_sub(azami);
    dosya.seek(SeekFrom::Start(baslangic)).ok()?;
    let mut tampon = Vec::with_capacity((uzunluk - baslangic) as usize);
    dosya.take(azami).read_to_end(&mut tampon).ok()?;
    let mut metin = String::from_utf8_lossy(&tampon).into_owned();
    if baslangic > 0 {
        match metin.find('\n') {
            Some(i) => {
                metin.drain(..=i);
            }
            None => metin.clear(),
        }
    }
    Some(metin)
}

fn en_yeni_rollout_dosyasi(kok: &Path) -> Option<PathBuf> {
    if !kok.is_dir() {
        return None;
    }
    let mut yigin = vec![kok.to_path_buf()];
    let mut en_iyi: Option<(SystemTime, PathBuf)> = None;
    while let Some(dizin) = yigin.pop() {
        let Ok(girdiler) = std::fs::read_dir(dizin) else {
            continue;
        };
        for girdi in girdiler.flatten() {
            let yol = girdi.path();
            if yol.is_dir() {
                yigin.push(yol);
            } else if let Some(ad) = yol.file_name().and_then(|n| n.to_str())
                && ad.starts_with("rollout-")
                && ad.ends_with(".jsonl")
                && let Ok(mtime) = girdi.metadata().and_then(|m| m.modified())
            {
                let daha_yeni = en_iyi
                    .as_ref()
                    .is_none_or(|(mevcut_t, _)| mtime > *mevcut_t);
                if daha_yeni {
                    en_iyi = Some((mtime, yol));
                }
            }
        }
    }
    en_iyi.map(|(_, p)| p)
}

/// Rollout JSONL metninden en son `token_count.rate_limits` kaydını ayrıştırır.
pub fn codex_rollout_ayristir(icerik: &str, simdi: UtcZaman) -> Option<HavuzAnlik> {
    for satir in icerik.lines().rev() {
        let temiz = satir.trim();
        if temiz.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(temiz) else {
            continue;
        };
        let payload = val.get("payload").unwrap_or(&val);
        let tip = payload
            .get("type")
            .or_else(|| val.get("type"))
            .and_then(Value::as_str);
        if tip != Some("token_count") {
            continue;
        }
        let Some(rate_limits_val) = payload
            .get("rate_limits")
            .or_else(|| payload.get("info").and_then(|i| i.get("rate_limits")))
        else {
            continue;
        };

        let okuma_zamani = val
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|s| UtcZaman::parse_tarih_veya_zaman(s).ok())
            .unwrap_or(simdi);
        let yas_sn = simdi.fark_sn(okuma_zamani).max(0) as u64;

        let Ok(grup) = serde_json::from_value::<HamRateLimitGrubu>(rate_limits_val.clone()) else {
            continue;
        };
        let mut pencereler = BTreeMap::new();
        pencere_ekle(&mut pencereler, grup.primary.as_ref());
        pencere_ekle(&mut pencereler, grup.secondary.as_ref());
        if pencereler.is_empty() {
            continue;
        }

        let mut havuz = HavuzAnlik {
            platform: "codex".to_string(),
            etiket: "Codex".to_string(),
            durum: Durum::Bayat,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::RolloutYedek,
                guven: GuvenEtiketi::Bayat,
                okuma: okuma_zamani,
                yas_sn,
                hassasiyet: None,
            }),
            pencereler,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: vec!["app-server başarısız; son yerel rollout kaydı gösteriliyor".into()],
        };
        havuz.hesapla_etkin_durum();
        return Some(havuz);
    }
    None
}

fn codex_hata_havuzu(
    durum: Durum,
    hata_turu: &str,
    http_kodu: Option<u16>,
    notlar: Vec<String>,
) -> HavuzAnlik {
    HavuzAnlik {
        platform: "codex".to_string(),
        etiket: "Codex".to_string(),
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
        hata: (durum == Durum::Hata).then(|| HataBilgisi {
            tur: hata_turu.to_string(),
            http_kodu,
        }),
        notlar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_hatasi_giris_gerek_ile_digerlerinden_ayrilir() {
        let giris = [
            json!({ "code": -32000, "message": "Not logged in. Run `codex login`." }),
            json!({ "code": 401, "message": "request failed" }),
            json!({ "code": -32603, "message": "x", "data": { "reason": "Unauthorized" } }),
            json!({ "message": "Authentication required: please sign in with ChatGPT" }),
            json!({ "message": "Sign in to continue" }),
            json!({ "message": "error: no auth.json" }),
        ];
        for h in giris {
            assert_eq!(codex_rpc_hata_turu(&h), "giris_gerek", "{h}");
        }
        let diger = [
            json!({ "code": -32601, "message": "Method not found" }),
            json!({ "code": -32603, "message": "internal error: rate limit service 503" }),
            json!({ "code": 4010, "message": "beklenmeyen" }),
            // Alt dizge yanlış pozitifleri: kelime sınırı olmadan eşleşmemeli.
            json!({ "code": -32603, "message": "rate limit window not exposed by design in this build" }),
            json!({ "message": "reassign in progress" }),
            json!({ "message": "no authority for model list" }),
        ];
        for h in diger {
            assert_eq!(codex_rpc_hata_turu(&h), "rpc_hatasi", "{h}");
        }
        let havuz = codex_giris_gerek_havuzu();
        assert_eq!(havuz.durum, Durum::Baglanmadi);
        assert_eq!(
            havuz.hata.as_ref().map(|h| h.tur.as_str()),
            Some("giris_gerek")
        );
        assert_eq!(havuz.kullanilabilir, Some(false));
        assert!(havuz.notlar[0].contains("codex login"));
    }

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
    }

    #[test]
    fn codex_haftalik_sifir_bes_saatlik_yuz_ayristirilir() {
        let ornek = json!({
            "rateLimits": {
                "limitId": "codex",
                "planType": "plus",
                "primary": {
                    "usedPercent": 0.0,
                    "windowDurationMins": 300,
                    "resetsAt": 1794800000
                },
                "secondary": {
                    "usedPercent": 100.0,
                    "windowDurationMins": 10080,
                    "resetsAt": 1795200000
                },
                "ordinaryUsageAllowed": true,
                "rateLimitReachedType": "weekly_limit",
                "rateLimitResetCredits": {
                    "availableCount": 1,
                    "expiresAt": 1795500000
                }
            },
            "rateLimitsByLimitId": {
                "codex": {
                    "primary": { "usedPercent": 0.0, "windowDurationMins": 300 }
                },
                "codex_extra": {
                    "primary": { "usedPercent": 25.0, "windowDurationMins": 10080, "resetsAt": 1795200000 }
                }
            }
        });

        let havuz = codex_yanit_ayristir(&ornek, z("2026-11-15T18:00:00Z"));
        assert_eq!(havuz.durum, Durum::Ok);
        assert_eq!(havuz.pencereler["5s"].kalan, 1.0);
        // usedPercent == 0 iken resetsAt yazılmamalıdır.
        assert!(havuz.pencereler["5s"].sifirlama.is_none());
        assert_eq!(havuz.pencereler["hafta"].kalan, 0.0);
        assert!(havuz.pencereler["hafta"].sifirlama.is_some());

        // Etkin kalan & bağlayıcı pencere
        assert_eq!(havuz.etkin_kalan, Some(0.0));
        assert_eq!(havuz.baglayici_pencere.as_deref(), Some("hafta"));
        assert_eq!(havuz.kullanilabilir, Some(false));
        assert!(havuz.alt_sayaclar.contains_key("codex_extra"));
        assert_eq!(
            havuz
                .ek
                .as_ref()
                .and_then(|e| e.sifirlama_kredisi.as_ref())
                .map(|c| c.adet),
            Some(1)
        );
    }

    #[test]
    fn codex_sema_uyumsuz_yanit_sema_degisti_dondurur() {
        let bozuk = json!({ "unexpected": true });
        let havuz = codex_yanit_ayristir(&bozuk, z("2026-11-15T18:00:00Z"));
        assert_eq!(havuz.durum, Durum::Hata);
        assert_eq!(
            havuz.hata.as_ref().map(|h| h.tur.as_str()),
            Some("sema_degisti")
        );
        assert_eq!(havuz.kullanilabilir, Some(false));
    }

    #[test]
    fn rollout_yedek_son_token_count_kaydini_alir() {
        let icerik = r#"{"timestamp":"2026-11-15T10:00:00Z","type":"event_msg","payload":{"type":"agent_message"}}
{"timestamp":"2026-11-15T12:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":40.0,"window_minutes":300,"resets_at":1794760000},"secondary":{"used_percent":10.0,"window_minutes":10080,"resets_at":1795200000}}}}
"#;
        let havuz = codex_rollout_ayristir(icerik, z("2026-11-15T12:30:00Z")).expect("rollout");
        assert_eq!(havuz.durum, Durum::Bayat);
        assert_eq!(havuz.kaynak.as_ref().map(|k| k.yas_sn), Some(1800));
        assert!((havuz.pencereler["5s"].kalan - 0.6).abs() < 1e-9);
        assert_eq!(havuz.etkin_kalan, Some(0.6));
        assert_eq!(havuz.baglayici_pencere.as_deref(), Some("5s"));
    }

    #[test]
    fn buyuk_rollout_dosyasinin_yalniz_kuyrugu_okunur() {
        let d = std::env::temp_dir().join(format!("cetvel-rollout-kuyruk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let kayit = r#"{"timestamp":"2026-11-15T12:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":40.0,"window_minutes":300,"resets_at":1794760000}}}}"#;
        let dolgu = format!(
            "{}\n",
            r#"{"timestamp":"2026-11-15T10:00:00Z","type":"event_msg","payload":{"type":"agent_message","text":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}}"#
        );
        // Son kayıt sonda: ilk (512 KB) geçişte bulunur; dosyanın başı (2 MB dolgu) okunmaz.
        let sonda = d.join("rollout-sonda.jsonl");
        std::fs::write(&sonda, format!("{}{kayit}\n", dolgu.repeat(20_000))).unwrap();
        let simdi = z("2026-11-15T12:30:00Z");
        let havuz = rollout_dosyasindan_oku(&sonda, simdi).expect("sondaki kayıt");
        assert!((havuz.pencereler["5s"].kalan - 0.6).abs() < 1e-9);
        assert!(dosya_kuyrugu_oku(&sonda, 1024).unwrap().len() <= 1024);

        // Kayıt sondan ~1 MB önce: ikinci (8 MB) geçişte bulunur.
        let ortada = d.join("rollout-ortada.jsonl");
        std::fs::write(&ortada, format!("{kayit}\n{}", dolgu.repeat(10_000))).unwrap();
        assert!(rollout_dosyasindan_oku(&ortada, simdi).is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn appmodel_adaylari_en_yuksek_surumu_secer() {
        let adaylar = vec![
            (
                "OpenAI.Codex_0.167.0_x64__abc".to_string(),
                PathBuf::from("eski.exe"),
            ),
            (
                "OpenAI.Codex_0.169.1_x64__abc".to_string(),
                PathBuf::from("yeni.exe"),
            ),
        ];
        assert_eq!(
            adaylari_surume_gore_sec(adaylar),
            Some(PathBuf::from("yeni.exe"))
        );
    }
}
