//! Bağla akışı: salt-okunur platform taraması, "Test et" sonucu özeti ve
//! Claude `statusLine` zincirleme planı.
//!
//! Tarama hiçbir şey kurmaz, hiçbir ayarı değiştirmez ve gizli değer okumaz: yalnız dosya/ikili
//! varlığı ve anahtarın tanımlı olup olmadığı kontrol edilir (`auth.json` içeriği okunmaz).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cetvel_baglayici::{YerelArac, agy_ikili_coz, codex_ikili_coz, yerel_arac_var_mi};
use cetvel_depo::{AgyOkumaIzni, Ayarlar, atomik_yaz, ortam_hkcu_veya_kimlik_var_mi};
use cetvel_model::{Durum, HavuzAnlik, UtcZaman};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// `bagla.tara` sonucu (yalnız varlık bilgisi; değer yok).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaramaSonucu {
    pub zaman: UtcZaman,
    pub claude_kurulu: bool,
    pub claude_statusline_tanimli: bool,
    pub claude_statusline_cetvel: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kota_bandi_ipucu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_kopru_yas_sn: Option<u64>,
    pub codex_ikili_var: bool,
    pub codex_auth_var: bool,
    /// `bulundu` · `giris_gerek` · `yok`
    pub codex_durum: String,
    pub agy_ikili_var: bool,
    pub agy_okuma: String,
    pub deepseek_anahtar_var: bool,
    /// OpenCode yerel kayıtları (`opencode.db` ya da eski `storage/`) ya da ikilisi bulundu.
    pub opencode_var: bool,
    /// OpenCode sayacı ayarda açık mı (Bağla'dan).
    #[serde(default)]
    pub opencode_etkin: bool,
    /// DeepSeek Harness oturum klasörü (`~/.dsh/sessions`) ya da ikilisi bulundu.
    #[serde(default)]
    pub dsh_var: bool,
    #[serde(default)]
    pub dsh_etkin: bool,
}

/// "Test et" sonucu (gizli değer içermez).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestSonucu {
    pub platform: String,
    pub basarili: bool,
    pub sure_ms: u64,
    pub ozet_tr: String,
    pub ozet_en: String,
    /// Okuma hız sınırına (havuz başına 1/dk) takıldıysa son bilinen sonuç gösterilir.
    #[serde(default)]
    pub onbellekten: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yas_sn: Option<u64>,
}

impl TestSonucu {
    pub fn basarisiz(platform: &str, sure_ms: u64, tr: &str, en: &str) -> Self {
        Self {
            platform: platform.to_string(),
            basarili: false,
            sure_ms,
            ozet_tr: tr.to_string(),
            ozet_en: en.to_string(),
            onbellekten: false,
            yas_sn: None,
        }
    }
}

pub fn agy_izni_metne(izin: AgyOkumaIzni) -> &'static str {
    match izin {
        AgyOkumaIzni::Kapali => "kapali",
        AgyOkumaIzni::IstekUzerine => "istek_uzerine",
        AgyOkumaIzni::Otomatik => "otomatik",
    }
}

pub fn agy_izni_coz(girdi: &str) -> Result<AgyOkumaIzni, String> {
    match girdi.trim().to_ascii_lowercase().as_str() {
        "kapali" => Ok(AgyOkumaIzni::Kapali),
        "istek_uzerine" | "istek-uzerine" => Ok(AgyOkumaIzni::IstekUzerine),
        "otomatik" => Ok(AgyOkumaIzni::Otomatik),
        _ => Err("geçersiz Antigravity okuma kipi".to_string()),
    }
}

/// Bağla "Test et" platform adını normalize eder (`codex`, `agy`, `deepseek`, `claude`,
/// `opencode`, `dsh`).
pub fn platform_normalize(platform: &str) -> Option<&'static str> {
    match platform.trim().to_ascii_lowercase().as_str() {
        "claude" | "claude_max" => Some("claude"),
        "codex" | "codex_plus" => Some("codex"),
        "agy" | "antigravity" => Some("agy"),
        "deepseek" => Some("deepseek"),
        "opencode" => Some("opencode"),
        "dsh" | "deepseek_harness" | "harness" => Some("dsh"),
        // Dinamik havuz kimlikleri platformlarına iner (`agy_gemini_5_xenon`, `claude_pro`).
        diger if diger.starts_with("agy_") => Some("agy"),
        diger if diger.starts_with("claude_") => Some("claude"),
        _ => None,
    }
}

pub fn kullanici_ev_dizini() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

fn path_icinde_ikili_var_mi(adlar: &[&str]) -> bool {
    let Some(path_val) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_val).any(|d| adlar.iter().any(|ad| d.join(ad).is_file()))
}

/// Claude `settings.json` yolu (`~/.claude/settings.json`).
pub fn claude_settings_yolu() -> Option<PathBuf> {
    kullanici_ev_dizini().map(|ev| ev.join(".claude").join("settings.json"))
}

fn claude_durumu_tara() -> (bool, bool, bool, Option<String>) {
    let Some(ev) = kullanici_ev_dizini() else {
        return (false, false, false, None);
    };
    let claude_dir = ev.join(".claude");
    let claude_kurulu = claude_dir.is_dir();

    let statusline = std::fs::read_to_string(claude_dir.join("settings.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("statusLine").cloned());
    let statusline_tanimli = statusline.is_some();
    let statusline_cetvel = statusline
        .as_ref()
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str)
        .is_some_and(|k| k.contains(STATUSLINE_ISARETI));

    let plugins_dir = claude_dir.join("plugins");
    let mut surum_ipucu = None;
    if let Ok(icerik) = std::fs::read_to_string(plugins_dir.join("installed_plugins.json"))
        && icerik.contains("kota-bandi")
    {
        surum_ipucu = Some("kota-bandi".to_string());
    }
    if surum_ipucu.is_none()
        && let Ok(girdiler) = std::fs::read_dir(plugins_dir.join("cache").join("kota-bandi"))
    {
        surum_ipucu = girdiler
            .flatten()
            .take(8)
            .filter_map(|g| g.file_name().to_str().map(str::to_string))
            .find(|ad| !ad.is_empty());
    }

    (
        claude_kurulu,
        statusline_tanimli,
        statusline_cetvel,
        surum_ipucu,
    )
}

/// DeepSeek anahtarının ortam değişkeni adını çözer (değer okunmaz).
pub fn deepseek_env_adi(ayarlar: &Ayarlar) -> String {
    ayarlar
        .deepseek
        .anahtar_env
        .clone()
        .unwrap_or_else(|| "DEEPSEEK_API_KEY".to_string())
}

/// Codex varlık durumunu iki dosya varlığından (`auth.json` içeriği okunmaz) ve servisteki son
/// okumanın hata türünden türetir: `auth.json` dursa da app-server "oturum kapalı" dediyse
/// (`giris_gerek`, ör. süresi dolmuş giriş) Bağla "Giriş gerek" gösterir.
pub fn codex_durumu(ikili_var: bool, auth_var: bool, son_hata: Option<&str>) -> &'static str {
    match (ikili_var, auth_var) {
        (false, _) => "yok",
        (true, false) => "giris_gerek",
        (true, true) if son_hata == Some("giris_gerek") => "giris_gerek",
        (true, true) => "bulundu",
    }
}

/// Salt-okunur platform taraması (`bagla.tara`).
pub fn tara(
    ayarlar: &Ayarlar,
    claude_havuz: Option<&HavuzAnlik>,
    codex_havuz: Option<&HavuzAnlik>,
    simdi: UtcZaman,
) -> TaramaSonucu {
    let (claude_kurulu, claude_statusline_tanimli, claude_statusline_cetvel, kota_bandi_ipucu) =
        claude_durumu_tara();
    let codex_ikili_var = codex_ikili_coz().is_some();
    let codex_auth_var =
        kullanici_ev_dizini().is_some_and(|ev| ev.join(".codex").join("auth.json").is_file());
    let agy_ikili_var = agy_ikili_coz().is_some();
    let ds_env = deepseek_env_adi(ayarlar);

    TaramaSonucu {
        zaman: simdi,
        claude_kurulu,
        claude_statusline_tanimli,
        claude_statusline_cetvel,
        kota_bandi_ipucu,
        claude_kopru_yas_sn: claude_havuz
            .and_then(|h| h.kaynak.as_ref())
            .map(|k| simdi.fark_sn(k.okuma).max(0) as u64),
        codex_ikili_var,
        codex_auth_var,
        codex_durum: codex_durumu(
            codex_ikili_var,
            codex_auth_var,
            codex_havuz
                .and_then(|h| h.hata.as_ref())
                .map(|h| h.tur.as_str()),
        )
        .to_string(),
        agy_ikili_var,
        agy_okuma: agy_izni_metne(ayarlar.agy.okuma).to_string(),
        deepseek_anahtar_var: ortam_hkcu_veya_kimlik_var_mi(&ds_env),
        opencode_var: yerel_arac_var_mi(YerelArac::OpenCode, ayarlar.opencode.dizin.as_deref())
            || path_icinde_ikili_var_mi(&["opencode.exe", "opencode.cmd"]),
        opencode_etkin: ayarlar.opencode.etkin,
        dsh_var: yerel_arac_var_mi(YerelArac::Dsh, ayarlar.dsh.dizin.as_deref())
            || path_icinde_ikili_var_mi(&["dsh.exe", "dsh.cmd"]),
        dsh_etkin: ayarlar.dsh.etkin,
    }
}

/// Havuzdan maskeli test özeti üretir: `(başarılı, tr, en)`. Gizli değer veya üçüncü taraf metni içermez.
pub fn havuz_test_ozeti(havuz: &HavuzAnlik) -> (bool, String, String) {
    if havuz.durum == Durum::Ok {
        if let Some(bakiye) = &havuz.bakiye {
            let bakiye_metin = format!("{:.2} {}", bakiye.toplam, bakiye.para_birimi);
            return (
                true,
                format!("bakiye {bakiye_metin}"),
                format!("balance {bakiye_metin}"),
            );
        }
        let mut parca_tr = Vec::new();
        let mut parca_en = Vec::new();
        for (ad, olcum) in &havuz.pencereler {
            let yuzde = (olcum.kullanilan.clamp(0.0, 1.0) * 100.0).round() as u32;
            parca_tr.push(format!("{ad} %{yuzde} kullanıldı"));
            parca_en.push(format!("{ad} {yuzde}% used"));
        }
        if !parca_tr.is_empty() {
            return (true, parca_tr.join(" · "), parca_en.join(" · "));
        }
        return (
            true,
            "bağlantı başarılı".to_string(),
            "connected".to_string(),
        );
    }

    let kod = havuz
        .hata
        .as_ref()
        .map(|h| h.tur.clone())
        .unwrap_or_else(|| match havuz.durum {
            Durum::Baglanmadi => "baglanmadi".to_string(),
            Durum::Kapali => "kapali".to_string(),
            _ => "okunamadi".to_string(),
        });
    (false, format!("durum: {kod}"), format!("status: {kod}"))
}

// ---------------------------------------------------------------------------
// Claude statusLine zincirleme (önizle → tarihli yedek → açık onay → yaz)
// ---------------------------------------------------------------------------

/// Cetvel'in statusLine komutunu tanıyan işaret.
pub const STATUSLINE_ISARETI: &str = "kopru statusline";

/// statusLine değişiklik planı.
#[derive(Debug, Clone, PartialEq)]
pub struct StatuslinePlani {
    pub yeni_settings: Value,
    pub onceki_komut: Option<String>,
    pub zaten_cetvel: bool,
    pub fark_metni: String,
}

/// Bayt dizisini küçük harfli onaltılık metne çevirir (komut satırında tırnak sorunu olmaz).
pub fn hex_kodla(metin: &str) -> String {
    metin.bytes().map(|b| format!("{b:02x}")).collect()
}

/// [`hex_kodla`] çıktısını geri çözer.
pub fn hex_coz(hex: &str) -> Option<String> {
    let temiz = hex.trim();
    if !temiz.len().is_multiple_of(2) || temiz.len() > 16_384 {
        return None;
    }
    let baytlar: Option<Vec<u8>> = (0..temiz.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(temiz.get(i..i + 2)?, 16).ok())
        .collect();
    String::from_utf8(baytlar?).ok()
}

/// Claude Code'un Windows'ta statusLine/hook komutlarını çalıştırdığı kabuk: Git Bash bulunursa
/// onu (`bash -c`), bulunmazsa PowerShell'i kullanır. Cetvel önceki komutu aynı kabukla çalıştırır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatuslineKabugu {
    Bash(PathBuf),
    PowerShell(PathBuf),
}

/// Git for Windows `bash.exe` yolunu Claude Code ile aynı sırayla arar:
/// `CLAUDE_CODE_GIT_BASH_PATH` → `PATH`'teki `git.exe`'nin kurulum kökü → bilinen kurulum dizinleri.
/// WSL'in `System32\bash.exe`'si bilerek seçilmez.
pub fn git_bash_yolu() -> Option<PathBuf> {
    git_bash_yolu_ortamla(|ad| std::env::var_os(ad))
}

/// Test edilebilir sürüm: ortam değişkenleri verilen okuyucudan alınır.
pub fn git_bash_yolu_ortamla<F>(ortam: F) -> Option<PathBuf>
where
    F: Fn(&str) -> Option<OsString>,
{
    if let Some(yol) = ortam("CLAUDE_CODE_GIT_BASH_PATH")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(yol);
    }
    if let Some(path_val) = ortam("PATH") {
        for dizin in std::env::split_paths(&path_val) {
            if !dizin.join("git.exe").is_file() {
                continue;
            }
            // <kök>\cmd\git.exe, <kök>\bin\git.exe, <kök>\mingw64\bin\git.exe
            let mut adaylar = Vec::new();
            if let Some(kok) = dizin.parent() {
                adaylar.push(kok.join("bin").join("bash.exe"));
                if let Some(ust) = kok.parent() {
                    adaylar.push(ust.join("bin").join("bash.exe"));
                }
            }
            if let Some(b) = adaylar.into_iter().find(|b| b.is_file()) {
                return Some(b);
            }
        }
    }
    for (degisken, alt) in [
        ("ProgramFiles", r"Git\bin\bash.exe"),
        ("ProgramW6432", r"Git\bin\bash.exe"),
        ("LOCALAPPDATA", r"Programs\Git\bin\bash.exe"),
    ] {
        if let Some(aday) = ortam(degisken)
            .map(|k| PathBuf::from(k).join(alt))
            .filter(|p| p.is_file())
        {
            return Some(aday);
        }
    }
    None
}

/// PowerShell yürütücüsü (`pwsh.exe` PATH'te varsa o, yoksa Windows PowerShell).
pub fn powershell_yolu() -> PathBuf {
    if let Some(path_val) = std::env::var_os("PATH") {
        for dizin in std::env::split_paths(&path_val) {
            let aday = dizin.join("pwsh.exe");
            if aday.is_file() {
                return aday;
            }
        }
    }
    let kok = std::env::var_os("SystemRoot").unwrap_or_else(|| OsString::from(r"C:\Windows"));
    PathBuf::from(kok).join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}

/// Claude Code'un statusLine komutlarını çalıştıracağı kabuk.
pub fn statusline_kabugu() -> StatuslineKabugu {
    match git_bash_yolu() {
        Some(b) => StatuslineKabugu::Bash(b),
        None => StatuslineKabugu::PowerShell(powershell_yolu()),
    }
}

/// Yol hem Git Bash'te hem PowerShell'de (ve cmd'de) tırnaksız tek sözcük olarak çalışır mı?
fn kabukta_tirnaksiz_guvenli(yol: &str) -> bool {
    !yol.is_empty()
        && !yol.starts_with(['~', '-'])
        && yol
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '/' | ':' | '.' | '_' | '-' | '~' | '+'))
}

/// statusLine'da cetvel.exe'yi çağıran metni üretir. Ters bölüler ileri bölüye çevrilir (Git Bash
/// `\` karakterini kaçış sayar). Yol boşluk/özel karakter içeriyorsa 8.3 kısa yol denenir; o da
/// yoksa kabuğa özgü biçim seçilir: Git Bash için `"yol"`, PowerShell için `& "yol"`.
pub fn statusline_exe_metni(cetvel_exe: &str, kisa_yol: Option<&str>, bash: bool) -> String {
    let duz = |y: &str| y.replace('\\', "/").replace('"', "");
    let uzun = duz(cetvel_exe);
    if kabukta_tirnaksiz_guvenli(&uzun) {
        return uzun;
    }
    if let Some(kisa) = kisa_yol.map(duz).filter(|k| kabukta_tirnaksiz_guvenli(k)) {
        return kisa;
    }
    if bash {
        format!("\"{uzun}\"")
    } else {
        format!("& \"{uzun}\"")
    }
}

/// 8.3 kısa yol (`C:\PROGRA~1\...`); birim kısa adları kapalıysa `None`.
#[cfg(windows)]
pub fn kisa_yol_al(yol: &Path) -> Option<String> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;
    use windows::core::PCWSTR;

    let genis: Vec<u16> = yol.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut tampon = vec![0u16; 1024];
    // SAFETY: `genis` NUL sonlu geçerli UTF-16 dizgedir; tampon uzunluğu çağrıya verilir.
    let uzunluk = unsafe { GetShortPathNameW(PCWSTR(genis.as_ptr()), Some(&mut tampon)) } as usize;
    if uzunluk == 0 || uzunluk >= tampon.len() {
        return None;
    }
    tampon.truncate(uzunluk);
    Some(OsString::from_wide(&tampon).to_string_lossy().into_owned())
}

#[cfg(not(windows))]
pub fn kisa_yol_al(_yol: &Path) -> Option<String> {
    None
}

/// Bu makinede (kısa yol ve kabuk keşfiyle) statusLine çağrı metnini hazırlar.
pub fn statusline_cagri_metni_hazirla(cetvel_exe: &str) -> String {
    let kisa = kisa_yol_al(Path::new(cetvel_exe));
    statusline_exe_metni(cetvel_exe, kisa.as_deref(), git_bash_yolu().is_some())
}

/// Cetvel statusLine komut satırını üretir; varsa önceki komut zincirlenir.
/// `exe_metni` [`statusline_exe_metni`] çıktısıdır (kabuğa uygun çağrı biçimi).
pub fn statusline_komutu(exe_metni: &str, onceki: Option<&str>) -> String {
    let mut komut = format!("{exe_metni} {STATUSLINE_ISARETI}");
    if let Some(o) = onceki.map(str::trim).filter(|o| !o.is_empty()) {
        komut.push_str(" --onceki-hex ");
        komut.push_str(&hex_kodla(o));
    }
    komut
}

/// Cetvel statusLine komutundan zincirlenmiş önceki komutu çözer (yoksa `None`).
pub fn statusline_onceki_komutu_coz(komut: &str) -> Option<String> {
    let mut parcalar = komut.split_whitespace();
    while let Some(p) = parcalar.next() {
        if p == "--onceki-hex" {
            return parcalar.next().and_then(hex_coz);
        }
    }
    None
}

/// `cetvel kaldir` için statusLine geri alma planı.
#[derive(Debug, Clone, PartialEq)]
pub struct StatuslineGeriAlma {
    pub yeni_settings: Value,
    pub onceki_komut: Option<String>,
    pub fark_metni: String,
}

/// statusLine Cetvel köprüsünü kullanıyorsa özgün komutu geri koyan (önceki komut yoksa
/// `statusLine` alanını kaldıran) planı döndürür; Cetvel'e ait değilse `None`.
pub fn statusline_geri_al_plani(mevcut: &Value) -> Option<StatuslineGeriAlma> {
    let mut nesne = mevcut.as_object()?.clone();
    let sl = nesne.get("statusLine")?.as_object()?.clone();
    let komut = sl.get("command")?.as_str()?;
    if !komut.contains(STATUSLINE_ISARETI) {
        return None;
    }
    let onceki = statusline_onceki_komutu_coz(komut);
    let eski_metin = format!("- \"statusLine\": {}\n", Value::Object(sl.clone()));
    let fark_metni = match &onceki {
        Some(o) => {
            let mut yeni_sl = sl;
            yeni_sl.insert("command".to_string(), json!(o));
            let yeni_deger = Value::Object(yeni_sl);
            let fark = format!("{eski_metin}+ \"statusLine\": {yeni_deger}");
            nesne.insert("statusLine".to_string(), yeni_deger);
            fark
        }
        None => {
            nesne.remove("statusLine");
            format!("{eski_metin}(statusLine kaldırılır; Cetvel öncesinde tanımlı değildi)")
        }
    };
    Some(StatuslineGeriAlma {
        yeni_settings: Value::Object(nesne),
        onceki_komut: onceki,
        fark_metni,
    })
}

/// `settings.json`'u yazar; varsa eski içeriği önce tarihli, üzerine yazılmayan bir yedeğe alır.
/// Dönüş: oluşturulan yedeğin dosya adı.
pub fn settings_yedekle_ve_yaz(
    settings_yolu: &Path,
    eski_ham: Option<&str>,
    yeni: &Value,
    zaman_etiketi: &str,
) -> Result<Option<String>, String> {
    let mut yedek_adi = None;
    if let Some(ham) = eski_ham {
        let klasor = settings_yolu
            .parent()
            .ok_or_else(|| "settings.json klasörü çözülemedi".to_string())?;
        let mut sira = 1u32;
        let hedef: PathBuf = loop {
            let aday = klasor.join(statusline_yedek_adi(zaman_etiketi, sira));
            if !aday.exists() {
                break aday;
            }
            sira += 1;
            if sira > 50 {
                return Err("yedek dosya adı üretilemedi".to_string());
            }
        };
        atomik_yaz(&hedef, ham.as_bytes()).map_err(|e| e.to_string())?;
        yedek_adi = hedef.file_name().map(|a| a.to_string_lossy().into_owned());
    } else if let Some(klasor) = settings_yolu.parent() {
        std::fs::create_dir_all(klasor).map_err(|e| e.to_string())?;
    }
    let metin = serde_json::to_string_pretty(yeni).map_err(|e| e.to_string())?;
    atomik_yaz(settings_yolu, format!("{metin}\n").as_bytes()).map_err(|e| e.to_string())?;
    Ok(yedek_adi)
}

/// Mevcut `settings.json` içeriğinden (yoksa `None`) statusLine planını üretir.
/// Mevcut komut korunur ve zincirlenir; diğer statusLine alanları (ör. `padding`) olduğu gibi kalır.
/// `exe_metni` kabuğa uygun çağrı metnidir ([`statusline_cagri_metni_hazirla`]).
pub fn statusline_plani(
    mevcut: Option<&Value>,
    exe_metni: &str,
) -> Result<StatuslinePlani, String> {
    let kok = mevcut.cloned().unwrap_or_else(|| Value::Object(Map::new()));
    let Value::Object(mut nesne) = kok else {
        return Err("Claude settings.json kökü JSON nesnesi değil".to_string());
    };

    let eski = nesne.get("statusLine").cloned();
    let (onceki_komut, mut yeni_sl) = match &eski {
        None | Some(Value::Null) => (None, Map::new()),
        Some(Value::Object(sl)) => {
            let tur = sl.get("type").and_then(Value::as_str).unwrap_or("command");
            if tur != "command" {
                return Err(format!(
                    "desteklenmeyen statusLine türü ({tur}); settings.json elle düzenlenmeli"
                ));
            }
            let komut = sl
                .get("command")
                .and_then(Value::as_str)
                .map(str::to_string);
            if let Some(k) = komut.as_deref().filter(|k| k.contains(STATUSLINE_ISARETI)) {
                // Zaten Cetvel: zincirdeki özgün komut korunur. Çağrı biçimi güncelse değişiklik
                // yok; eski biçimse (ör. PowerShell'de çalışmayan `"C:\...\cetvel.exe" ...`)
                // aynı özgün komutla yeniden yazılır.
                let ozgun = statusline_onceki_komutu_coz(k);
                if statusline_komutu(exe_metni, ozgun.as_deref()) == k {
                    return Ok(StatuslinePlani {
                        yeni_settings: Value::Object(nesne),
                        onceki_komut: None,
                        zaten_cetvel: true,
                        fark_metni: "statusLine zaten Cetvel köprüsünü kullanıyor; değişiklik yok."
                            .to_string(),
                    });
                }
                (ozgun, sl.clone())
            } else {
                (komut, sl.clone())
            }
        }
        Some(_) => {
            return Err(
                "statusLine alanı beklenmeyen biçimde; settings.json elle düzenlenmeli".to_string(),
            );
        }
    };

    yeni_sl.insert("type".to_string(), json!("command"));
    yeni_sl.insert(
        "command".to_string(),
        json!(statusline_komutu(exe_metni, onceki_komut.as_deref())),
    );
    let yeni_deger = Value::Object(yeni_sl);
    let eski_metin = eski
        .as_ref()
        .map(|e| format!("- \"statusLine\": {e}\n"))
        .unwrap_or_default();
    let fark_metni = format!("{eski_metin}+ \"statusLine\": {yeni_deger}");
    nesne.insert("statusLine".to_string(), yeni_deger);

    Ok(StatuslinePlani {
        yeni_settings: Value::Object(nesne),
        onceki_komut,
        zaten_cetvel: false,
        fark_metni,
    })
}

/// Tarihli, üzerine yazılmayan yedek adı (`settings.cetvel-yedek 2026-11-16 14.30.05.json`).
pub fn statusline_yedek_adi(zaman_etiketi: &str, sira: u32) -> String {
    let temiz: String = zaman_etiketi
        .chars()
        .map(|c| {
            if matches!(c, ':' | '/' | '\\') {
                '.'
            } else {
                c
            }
        })
        .collect();
    if sira <= 1 {
        format!("settings.cetvel-yedek {}.json", temiz.trim())
    } else {
        format!("settings.cetvel-yedek {} ({sira}).json", temiz.trim())
    }
}

/// statusLine stdin JSON'undan köprüye gönderilebilecek kota yükünü çıkarır (varsa).
/// Yalnız `rate_limits` nesnesi iletilir; biçim köprüde doğrulanır, uymazsa reddedilir.
pub fn statusline_kota_yuku(stdin_json: &Value) -> Option<Value> {
    let mut rl = stdin_json.get("rate_limits")?.as_object()?.clone();
    if rl.is_empty() {
        return None;
    }
    // Köprü kaynağı statusLine olarak etiketlesin (hassasiyet 1.0; OAuth 0.1 sanılmasın).
    rl.insert("kaynak".to_string(), json!("statusline"));
    Some(Value::Object(rl))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "C:/Araclar/Cetvel/cetvel.exe";

    #[test]
    fn hex_kodlama_gidis_donus_ve_gecersiz_girdi() {
        let k = "node \"C:\\x y\\sl.js\" --a 'b' & echo ç";
        assert_eq!(hex_coz(&hex_kodla(k)).as_deref(), Some(k));
        assert!(hex_coz("abc").is_none());
        assert!(hex_coz("zz").is_none());
    }

    #[test]
    fn statusline_yoksa_yeni_cetvel_komutu_eklenir_diger_ayarlar_korunur() {
        let mevcut = json!({ "model": "opus", "env": { "A": "1" } });
        let plan = statusline_plani(Some(&mevcut), EXE).expect("plan");
        assert!(!plan.zaten_cetvel);
        assert!(plan.onceki_komut.is_none());
        assert_eq!(plan.yeni_settings["model"], "opus");
        assert_eq!(plan.yeni_settings["env"]["A"], "1");
        let komut = plan.yeni_settings["statusLine"]["command"]
            .as_str()
            .unwrap();
        assert_eq!(komut, format!("{EXE} kopru statusline"));
        assert!(plan.fark_metni.starts_with("+ \"statusLine\""));
    }

    #[test]
    fn mevcut_statusline_ezilmez_zincirlenir_ve_alanlari_korunur() {
        let mevcut = json!({
            "statusLine": { "type": "command", "command": "bash ~/.claude/sl.sh", "padding": 2 }
        });
        let plan = statusline_plani(Some(&mevcut), EXE).expect("plan");
        assert_eq!(plan.onceki_komut.as_deref(), Some("bash ~/.claude/sl.sh"));
        let sl = &plan.yeni_settings["statusLine"];
        assert_eq!(sl["padding"], 2);
        let komut = sl["command"].as_str().unwrap();
        let hex = komut.split("--onceki-hex ").nth(1).expect("zincir");
        assert_eq!(hex_coz(hex).as_deref(), Some("bash ~/.claude/sl.sh"));
        assert!(plan.fark_metni.contains("- \"statusLine\""));
    }

    #[test]
    fn zaten_cetvel_ise_degisiklik_yok_ve_desteklenmeyen_tur_reddedilir() {
        let ayni =
            json!({ "statusLine": { "type": "command", "command": statusline_komutu(EXE, None) } });
        let plan = statusline_plani(Some(&ayni), EXE).expect("plan");
        assert!(plan.zaten_cetvel);
        assert_eq!(plan.yeni_settings, ayni);

        let diger = json!({ "statusLine": { "type": "static", "text": "x" } });
        assert!(statusline_plani(Some(&diger), EXE).is_err());
        assert!(statusline_plani(Some(&json!([1, 2])), EXE).is_err());
        assert!(statusline_plani(None, EXE).is_ok());
    }

    #[test]
    fn yedek_adi_tarihli_ve_cakismada_sirali() {
        assert_eq!(
            statusline_yedek_adi("2026-11-16 14:30:05", 1),
            "settings.cetvel-yedek 2026-11-16 14.30.05.json"
        );
        assert_eq!(
            statusline_yedek_adi("2026-11-16 14:30:05", 2),
            "settings.cetvel-yedek 2026-11-16 14.30.05 (2).json"
        );
    }

    #[test]
    fn kota_yuku_yalniz_rate_limits_nesnesinden_cikar() {
        let girdi = json!({ "model": { "id": "x" }, "rate_limits": { "five_hour": { "used_percentage": 12 } } });
        assert_eq!(
            statusline_kota_yuku(&girdi),
            Some(json!({ "five_hour": { "used_percentage": 12 }, "kaynak": "statusline" }))
        );
        assert!(statusline_kota_yuku(&json!({ "model": "x" })).is_none());
        assert!(statusline_kota_yuku(&json!({ "rate_limits": {} })).is_none());
    }

    #[test]
    fn exe_metni_kabuklarda_calisacak_bicimde_uretilir() {
        // Boşluksuz yol: tırnaksız, ileri bölülü (Git Bash, PowerShell, cmd ortak).
        assert_eq!(
            statusline_exe_metni(r"C:\Araclar\Cetvel\cetvel.exe", None, true),
            "C:/Araclar/Cetvel/cetvel.exe"
        );
        // Boşluklu yol: kısa yol varsa o kullanılır.
        assert_eq!(
            statusline_exe_metni(
                r"C:\Program Files\Cetvel\cetvel.exe",
                Some(r"C:\PROGRA~1\Cetvel\cetvel.exe"),
                false
            ),
            "C:/PROGRA~1/Cetvel/cetvel.exe"
        );
        // Kısa yol yoksa kabuğa özgü: bash `"..."`, PowerShell `& "..."`.
        assert_eq!(
            statusline_exe_metni(r"C:\Program Files\Cetvel\cetvel.exe", None, true),
            "\"C:/Program Files/Cetvel/cetvel.exe\""
        );
        assert_eq!(
            statusline_exe_metni(r"C:\Program Files (x86)\C\cetvel.exe", None, false),
            "& \"C:/Program Files (x86)/C/cetvel.exe\""
        );
        // Türkçe karakter sorun değil, virgül/ampersand tırnak gerektirir.
        assert_eq!(
            statusline_exe_metni(r"C:\Kullanıcı\Çetvel\cetvel.exe", None, false),
            "C:/Kullanıcı/Çetvel/cetvel.exe"
        );
        assert!(statusline_exe_metni(r"C:\a,b\cetvel.exe", None, false).starts_with("& "));
    }

    #[test]
    fn eski_bicimli_cetvel_sarmalayicisi_ozgun_komutla_guncellenir() {
        let eski_komut = format!(
            "\"C:\\Program Files\\Cetvel\\cetvel.exe\" kopru statusline --onceki-hex {}",
            hex_kodla("jq -r '.model.display_name'")
        );
        let mevcut = json!({ "statusLine": { "type": "command", "command": eski_komut } });
        let plan = statusline_plani(Some(&mevcut), EXE).expect("plan");
        assert!(!plan.zaten_cetvel, "eski biçim güncellenmeli");
        assert_eq!(
            plan.onceki_komut.as_deref(),
            Some("jq -r '.model.display_name'")
        );
        let yeni = plan.yeni_settings["statusLine"]["command"]
            .as_str()
            .unwrap();
        assert!(yeni.starts_with("C:/Araclar/Cetvel/cetvel.exe kopru statusline"));
        assert_eq!(
            statusline_onceki_komutu_coz(yeni).as_deref(),
            Some("jq -r '.model.display_name'"),
            "zincir iç içe sarılmaz"
        );
        // Güncel biçim ikinci kez sarılmaz.
        let ikinci = statusline_plani(Some(&plan.yeni_settings), EXE).expect("plan");
        assert!(ikinci.zaten_cetvel);
    }

    #[test]
    fn kisa_yol_bosluksuz_ve_var_olan_dosyayi_gosterir() {
        let d = std::env::temp_dir().join(format!("cetvel kisa yol {}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let exe = d.join("cetvel.exe");
        std::fs::write(&exe, b"").unwrap();
        // Birimde 8.3 adları kapalıysa `None` (o zaman kabuğa özgü tırnaklı biçim kullanılır).
        if let Some(kisa) = kisa_yol_al(&exe) {
            assert!(!kisa.contains(' '), "{kisa}");
            assert!(Path::new(&kisa).is_file());
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn geri_alma_onceki_komutu_koyar_ya_da_alani_kaldirir() {
        let zincir = json!({
            "model": "opus",
            "statusLine": {
                "type": "command",
                "command": statusline_komutu(EXE, Some("bash ~/.claude/sl.sh")),
                "padding": 1
            }
        });
        let plan = statusline_geri_al_plani(&zincir).expect("cetvel");
        assert_eq!(plan.onceki_komut.as_deref(), Some("bash ~/.claude/sl.sh"));
        assert_eq!(
            plan.yeni_settings["statusLine"]["command"],
            "bash ~/.claude/sl.sh"
        );
        assert_eq!(plan.yeni_settings["statusLine"]["padding"], 1);
        assert_eq!(plan.yeni_settings["model"], "opus");

        let yalniz =
            json!({ "statusLine": { "type": "command", "command": statusline_komutu(EXE, None) } });
        let plan = statusline_geri_al_plani(&yalniz).expect("cetvel");
        assert!(plan.onceki_komut.is_none());
        assert!(plan.yeni_settings.get("statusLine").is_none());

        let baskasi = json!({ "statusLine": { "type": "command", "command": "jq -r .x" } });
        assert!(statusline_geri_al_plani(&baskasi).is_none());
        assert!(statusline_geri_al_plani(&json!({})).is_none());
    }

    #[test]
    fn settings_yedekle_ve_yaz_tarihli_yedek_birakir() {
        let d = std::env::temp_dir().join(format!("cetvel-settings-yaz-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let yol = d.join("settings.json");
        std::fs::write(&yol, "{\"a\":1}").unwrap();
        let yedek = settings_yedekle_ve_yaz(
            &yol,
            Some("{\"a\":1}"),
            &json!({"a": 2}),
            "2026-11-16 10:00:00",
        )
        .unwrap()
        .expect("yedek adı");
        assert_eq!(
            std::fs::read_to_string(d.join(&yedek)).unwrap(),
            "{\"a\":1}"
        );
        let yeni: Value = serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
        assert_eq!(yeni["a"], 2);
        let ikinci = settings_yedekle_ve_yaz(
            &yol,
            Some("{\"a\":2}"),
            &json!({"a": 3}),
            "2026-11-16 10:00:00",
        )
        .unwrap()
        .unwrap();
        assert_ne!(ikinci, yedek, "yedek üzerine yazılmaz");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn git_bash_wsl_yerine_git_kokunden_bulunur() {
        let d = std::env::temp_dir().join(format!("cetvel-gitbash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let git_cmd = d.join("Git").join("cmd");
        let git_bin = d.join("Git").join("bin");
        let wsl = d.join("System32");
        for k in [&git_cmd, &git_bin, &wsl] {
            std::fs::create_dir_all(k).unwrap();
        }
        std::fs::write(git_cmd.join("git.exe"), b"").unwrap();
        std::fs::write(git_bin.join("bash.exe"), b"").unwrap();
        std::fs::write(wsl.join("bash.exe"), b"").unwrap();
        let path = std::env::join_paths([wsl.clone(), git_cmd.clone()]).unwrap();
        let bulunan = git_bash_yolu_ortamla(|ad| (ad == "PATH").then(|| path.clone()));
        assert_eq!(bulunan, Some(git_bin.join("bash.exe")));
        // Git yoksa (yalnız WSL bash) bulunmaz → PowerShell'e düşülür.
        let yalniz_wsl = std::env::join_paths([wsl.clone()]).unwrap();
        assert!(git_bash_yolu_ortamla(|ad| (ad == "PATH").then(|| yalniz_wsl.clone())).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn platform_ve_codex_durum_esleme() {
        assert_eq!(platform_normalize("Codex"), Some("codex"));
        assert_eq!(platform_normalize("claude_max"), Some("claude"));
        assert_eq!(platform_normalize("opencode"), Some("opencode"));
        assert_eq!(platform_normalize("DeepSeek_Harness"), Some("dsh"));
        assert_eq!(platform_normalize("gemini"), None);
        assert_eq!(codex_durumu(false, true, None), "yok");
        assert_eq!(codex_durumu(true, false, None), "giris_gerek");
        assert_eq!(codex_durumu(true, true, None), "bulundu");
        // auth.json var ama app-server oturumu reddetti (süresi dolmuş giriş).
        assert_eq!(codex_durumu(true, true, Some("giris_gerek")), "giris_gerek");
        assert_eq!(codex_durumu(true, true, Some("zaman_asimi")), "bulundu");
        assert_eq!(codex_durumu(false, true, Some("giris_gerek")), "yok");
        assert_eq!(agy_izni_coz("OTOMATIK").unwrap(), AgyOkumaIzni::Otomatik);
        assert!(agy_izni_coz("her_sey").is_err());
    }
}
