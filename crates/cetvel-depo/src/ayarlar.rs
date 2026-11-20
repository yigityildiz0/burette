use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::DepoHatasi;
use crate::atomik::atomik_yaz;
use crate::gizli::GizliDeger;
use crate::kimlik::{kimlik_oku_gizli, kimlik_var_mi};

/// `%APPDATA%\Cetvel\ayarlar.toml` yapılandırması.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ayarlar {
    pub servis: ServisAyari,
    pub http: HttpAyari,
    pub bildirim: BildirimAyari,
    pub codex: CodexAyari,
    pub agy: AgyAyari,
    pub deepseek: DeepSeekAyari,
    pub masaustu: MasaustuAyari,
    /// Claude dışı platformların ajan olay okuyucusu (`GET /v1/ajanlar`, `ajanlar.json`).
    pub ajan: AjanAyari,
    /// OpenCode yerel kullanım sayacı (varsayılan kapalı; yalnız Bağla'dan açılır).
    pub opencode: YerelSayacAyari,
    /// DeepSeek Harness (`dsh`) yerel kullanım sayacı (varsayılan kapalı; yalnız Bağla'dan açılır).
    pub dsh: YerelSayacAyari,
}

/// Kotası olmayan yerel araç sayacı ayarı (OpenCode, DeepSeek Harness).
///
/// `dizin` boşsa varsayılan konum çalışma anında çözülür: OpenCode için
/// `XDG_DATA_HOME\opencode` ya da `~\.local\share\opencode`, dsh için `DSH_HOME` ya da `~\.dsh`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YerelSayacAyari {
    pub etkin: bool,
    pub dizin: Option<String>,
    /// Yoklama aralığı (saniye, 60..=86400).
    pub aralik_sn: u64,
}

impl Default for YerelSayacAyari {
    fn default() -> Self {
        Self {
            etkin: false,
            dizin: None,
            aralik_sn: 300,
        }
    }
}

/// Ajan olay okuyucusu ayarları. Kaynaklar salt okunur yoklanır; yol alanları boşsa varsayılan
/// konumlar (`CODEX_HOME`/`~\.codex`, `~\.gemini\antigravity*\conversation_summaries.db`,
/// `ORKESTRA_DIR`/`~\.orkestra`) çalışma anında çözülür. `CETVEL_AGY_OZET_DB` ortam değişkeni
/// (`;` ayrılmış yollar) `agy_ozet_db`'nin önüne geçer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AjanAyari {
    pub etkin: bool,
    pub codex: bool,
    pub antigravity: bool,
    /// Codex ana klasörü (`sessions` bunun altındadır).
    pub codex_dizini: Option<String>,
    /// Antigravity özet veritabanı yolları.
    pub agy_ozet_db: Vec<String>,
    pub orkestra_dizini: Option<String>,
    /// Yoklama aralığı (ms, en az 500).
    pub yoklama_ms: u64,
    /// Bitti/hata/iptal ajanın listede kalma süresi.
    pub bitis_bekleme_sn: u64,
    /// Çalışıyor görünüp bu süre etkinliksiz kalan ajan `askida` sayılır.
    pub askida_sn: u64,
    /// Yalnız bu süre içinde değişen kaynaklar izlenir.
    pub pencere_sn: u64,
}

impl Default for AjanAyari {
    fn default() -> Self {
        Self {
            etkin: true,
            codex: true,
            antigravity: true,
            codex_dizini: None,
            agy_ozet_db: Vec::new(),
            orkestra_dizini: None,
            yoklama_ms: 2000,
            bitis_bekleme_sn: 60,
            askida_sn: 600,
            pencere_sn: 7200,
        }
    }
}

/// Arayüzün desteklediği dil kodları (`apps/arayuz` ile aynı küme).
pub const DESTEKLENEN_DILLER: [&str; 11] = [
    "tr", "en", "de", "fr", "es", "pt-BR", "ru", "ja", "ko", "zh-Hans", "zh-Hant",
];
/// Arayüzün renk temaları (açık/koyu kipten bağımsız).
pub const RENK_TEMALARI: [&str; 4] = ["terakota", "okyanus", "adacayi", "karbon"];
/// Görünüm kipleri: `sistem` işletim sisteminin açık/koyu tercihini izler.
pub const KIPLER: [&str; 3] = ["acik", "koyu", "sistem"];
/// Gösterge ve tepsi yüzde yönleri (`kalan` varsayılan).
pub const DEGER_YONLERI: [&str; 2] = ["kalan", "kullanilan"];

const VARSAYILAN_DIL: &str = "tr";
const VARSAYILAN_RENK_TEMASI: &str = "terakota";
const VARSAYILAN_KIP: &str = "koyu";
const VARSAYILAN_DEGER_YONU: &str = "kalan";

/// `dil` desteklenen 11 koddan biri mi (büyük/küçük harf duyarlı; `pt-BR`, `zh-Hans`, `zh-Hant`).
pub fn dil_gecerli_mi(dil: &str) -> bool {
    DESTEKLENEN_DILLER.contains(&dil)
}

/// `renk_temasi` bilinen renk temalarından biri mi.
pub fn renk_temasi_gecerli_mi(renk_temasi: &str) -> bool {
    RENK_TEMALARI.contains(&renk_temasi)
}

/// `kip` bilinen görünüm kiplerinden biri mi.
pub fn kip_gecerli_mi(kip: &str) -> bool {
    KIPLER.contains(&kip)
}

/// `deger_yonu` desteklenen değer yönlerinden (`kalan` / `kullanilan`) biri mi.
pub fn deger_yonu_gecerli_mi(yon: &str) -> bool {
    DEGER_YONLERI.contains(&yon)
}

/// Eski tek alanlı `tema` değerini `(renk_temasi, kip)` çiftine eşler; bilinmeyen değerde `None`.
///
/// - `murekkep` (koyu mürekkep) -> `karbon` + `koyu`: nötr koyu tema.
/// - `kagit` (açık kâğıt) -> `terakota` + `acik`: yeni varsayılan renk, açık kip.
/// - `terminal` (koyu terminal) -> `karbon` + `koyu`: karbon en yakın nötr koyu paletidir; eski
///   adın ayırt edici yeşil/tek renk görünümü yeni temalarda yok, bilinçli kayıp.
///
/// `sistem` kipine eşlenmez: eski değer kullanıcının açık seçimiydi, sessizce otomatiğe geçmemeli.
pub fn eski_temayi_esle(tema: &str) -> Option<(&'static str, &'static str)> {
    match tema {
        "murekkep" | "terminal" => Some(("karbon", "koyu")),
        "kagit" => Some(("terakota", "acik")),
        _ => None,
    }
}

/// Eski (`tema`) alanını okuyan eski arayüz sürümleri için kipten türetilen görünüm:
/// `koyu` -> `murekkep`, diğerleri -> `kagit`. Yeni arayüz `kip` ve `renk_temasi` kullanmalı.
pub fn eski_tema_gorunumu(kip: &str) -> &'static str {
    if kip == "koyu" { "murekkep" } else { "kagit" }
}

/// Masaüstü kabuğu (Tauri) tercihleri: Windows ile başlat, kapatınca tepsiye küçült, dil, renk
/// teması ve görünüm kipi.
///
/// Eski dosyalardaki tek alanlı `tema` (`murekkep`/`kagit`/`terminal`) okunurken
/// [`eski_temayi_esle`] ile `renk_temasi` + `kip` çiftine çevrilir; yeni alanlar varsa onlar
/// kazanır. Kayıtta `tema` yazılmaz, böylece dosya ilk kayıtta yeni biçime geçer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "MasaustuAyariHam")]
pub struct MasaustuAyari {
    pub kart_sirasi: Vec<String>,
    pub gizli_kartlar: Vec<String>,
    pub sabit_kartlar: Vec<String>,
    /// Gizli WebView'in bellekte tutulma süresi (saniye, 1..=3600).
    pub pencere_bosta_sn: u64,
    pub windows_ile_baslat: bool,
    pub tepsiye_kucult: bool,
    pub dil: String,
    pub renk_temasi: String,
    pub kip: String,
    pub deger_yonu: String,
}

impl Default for MasaustuAyari {
    fn default() -> Self {
        Self {
            kart_sirasi: Vec::new(),
            gizli_kartlar: Vec::new(),
            sabit_kartlar: Vec::new(),
            pencere_bosta_sn: 60,
            windows_ile_baslat: false,
            tepsiye_kucult: true,
            dil: VARSAYILAN_DIL.to_string(),
            renk_temasi: VARSAYILAN_RENK_TEMASI.to_string(),
            kip: VARSAYILAN_KIP.to_string(),
            deger_yonu: VARSAYILAN_DEGER_YONU.to_string(),
        }
    }
}

impl MasaustuAyari {
    /// Geçersiz dil/renk teması/kip/değer yönünü varsayılana döndürür (elle bozulmuş dosyaya karşı).
    pub fn sinirla(&mut self) {
        self.pencere_bosta_sn = self.pencere_bosta_sn.clamp(1, 3600);
        for liste in [
            &mut self.kart_sirasi,
            &mut self.gizli_kartlar,
            &mut self.sabit_kartlar,
        ] {
            liste.retain(|ad| kart_adi_gecerli_mi(ad));
            liste.truncate(64);
        }
        if !dil_gecerli_mi(&self.dil) {
            self.dil = VARSAYILAN_DIL.to_string();
        }
        if !renk_temasi_gecerli_mi(&self.renk_temasi) {
            self.renk_temasi = VARSAYILAN_RENK_TEMASI.to_string();
        }
        if !kip_gecerli_mi(&self.kip) {
            self.kip = VARSAYILAN_KIP.to_string();
        }
        if !deger_yonu_gecerli_mi(&self.deger_yonu) {
            self.deger_yonu = VARSAYILAN_DEGER_YONU.to_string();
        }
    }
}

/// `[masaustu]` tablosunun diskteki ham biçimi (eski `tema` alanını da taşır).
#[derive(Default, Deserialize)]
#[serde(default)]
struct MasaustuAyariHam {
    kart_sirasi: Vec<String>,
    gizli_kartlar: Vec<String>,
    sabit_kartlar: Vec<String>,
    pencere_bosta_sn: Option<u64>,
    windows_ile_baslat: Option<bool>,
    tepsiye_kucult: Option<bool>,
    dil: Option<String>,
    renk_temasi: Option<String>,
    kip: Option<String>,
    deger_yonu: Option<String>,
    /// Eski sürümlerin tek alanı; yalnız okunur, yazılmaz.
    tema: Option<String>,
}

impl From<MasaustuAyariHam> for MasaustuAyari {
    fn from(ham: MasaustuAyariHam) -> Self {
        let varsayilan = Self::default();
        let eski = ham.tema.as_deref().and_then(eski_temayi_esle);
        let mut sonuc = Self {
            kart_sirasi: ham.kart_sirasi,
            gizli_kartlar: ham.gizli_kartlar,
            sabit_kartlar: ham.sabit_kartlar,
            pencere_bosta_sn: ham.pencere_bosta_sn.unwrap_or(varsayilan.pencere_bosta_sn),
            windows_ile_baslat: ham
                .windows_ile_baslat
                .unwrap_or(varsayilan.windows_ile_baslat),
            tepsiye_kucult: ham.tepsiye_kucult.unwrap_or(varsayilan.tepsiye_kucult),
            dil: ham.dil.unwrap_or(varsayilan.dil),
            renk_temasi: ham
                .renk_temasi
                .or_else(|| eski.map(|(renk, _)| renk.to_string()))
                .unwrap_or(varsayilan.renk_temasi),
            kip: ham
                .kip
                .or_else(|| eski.map(|(_, kip)| kip.to_string()))
                .unwrap_or(varsayilan.kip),
            deger_yonu: ham.deger_yonu.unwrap_or(varsayilan.deger_yonu),
        };
        sonuc.sinirla();
        sonuc
    }
}

/// Kart kimliği: `^[a-z0-9_]{1,40}$` (ASCII).
pub fn kart_adi_gecerli_mi(ad: &str) -> bool {
    (1..=40).contains(&ad.len())
        && ad
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub fn kart_listesi_gecerli_mi(liste: &[String]) -> bool {
    liste.len() <= 64 && liste.iter().all(|ad| kart_adi_gecerli_mi(ad))
}

/// Bozuk/eksik `ayarlar.toml` kurtarma sonucu (bozuksa yedekten açılır).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AyarKurtarma {
    /// Dosya sağlamdı ya da yoktu (varsayılan kullanıldı).
    Yok,
    /// Ana dosya bozuktu; son sağlam yedekten açıldı.
    YedektenAcildi { bozuk_kopya: Option<String> },
    /// Ana dosya ve yedek kullanılamadı; varsayılan ayarlar kullanıldı.
    Varsayilan { bozuk_kopya: Option<String> },
    /// Dosya şu an okunamadı (kilit, paylaşım ihlali, izin); dosyaya dokunulmadı. Dönen varsayılan
    /// ayarlar **yazılmamalı** (kullanıcının gerçek dosyasını ezer); servis öncekileri korur.
    OkunamadiGecici { hata: String },
}

impl AyarKurtarma {
    /// Kullanıcıya gösterilecek kısa Türkçe açıklama (kurtarma yoksa `None`).
    pub fn aciklama(&self) -> Option<String> {
        match self {
            Self::Yok => None,
            Self::YedektenAcildi { bozuk_kopya } => Some(format!(
                "ayarlar.toml bozuktu; son sağlam yedekten açıldı{}",
                bozuk_kopya
                    .as_deref()
                    .map(|a| format!(" (bozuk dosya: {a})"))
                    .unwrap_or_default()
            )),
            Self::Varsayilan { bozuk_kopya } => Some(format!(
                "ayarlar.toml okunamadı; varsayılan ayarlar kullanılıyor{}",
                bozuk_kopya
                    .as_deref()
                    .map(|a| format!(" (bozuk dosya: {a})"))
                    .unwrap_or_default()
            )),
            Self::OkunamadiGecici { hata } => Some(format!(
                "ayarlar.toml şu an okunamadı ({hata}); dosyaya dokunulmadı, yeniden denenecek"
            )),
        }
    }

    /// Bu yüklemeden türetilen ayarlar dosyaya geri yazılabilir mi? Geçici okuma hatasında
    /// `false`: varsayılanlar kullanıcının gerçek dosyasının üzerine yazılmamalı.
    pub fn yazmaya_uygun(&self) -> bool {
        !matches!(self, Self::OkunamadiGecici { .. })
    }
}

/// Ayar yükleme sonucu ve varsa kurtarma bilgisi.
#[derive(Debug, Clone)]
pub struct AyarYukleme {
    pub ayarlar: Ayarlar,
    pub kurtarma: AyarKurtarma,
}

/// Yerel HTTP API ayarları.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpAyari {
    pub port: u16,
    /// Port doluysa işletim sisteminden boş port alınıp `baglanti.json`'a yazılır.
    pub otomatik_port: bool,
}

impl Default for HttpAyari {
    fn default() -> Self {
        Self {
            port: 43117,
            otomatik_port: true,
        }
    }
}

/// Windows toast bildirim ayarları.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BildirimAyari {
    pub etkin: bool,
    pub sessiz_baslangic_saat: u8,
    pub sessiz_bitis_saat: u8,
    pub tahmin_bildirimi: bool,
    pub esik_bildirimi: bool,
    pub kredi_bildirimi: bool,
    pub bayat_bildirimi: bool,
    pub sifirlandi_bildirimi: bool,
    /// Claude 5 saatlik pencere eşik yüzdesi (kalan ≤ eşik → bildirim).
    pub claude_5s_esik_yuzde: u8,
    pub claude_hafta_esik_yuzde: u8,
    /// Claude dışındaki havuzlar için eşik yüzdesi.
    pub diger_esik_yuzde: u8,
    /// Platform kimliğine göre özel bildirim eşiği (`"claude": 20` vb., `%1..=%99`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub platform_esikleri: BTreeMap<String, u8>,
    /// Bildirimleri sessize alınan platform kimlikleri (`["codex", ...]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessiz_platformlar: Vec<String>,
}

impl Default for BildirimAyari {
    fn default() -> Self {
        Self {
            etkin: true,
            sessiz_baslangic_saat: 0,
            sessiz_bitis_saat: 8,
            tahmin_bildirimi: true,
            esik_bildirimi: true,
            kredi_bildirimi: true,
            bayat_bildirimi: true,
            sifirlandi_bildirimi: true,
            claude_5s_esik_yuzde: 15,
            claude_hafta_esik_yuzde: 30,
            diger_esik_yuzde: 20,
            platform_esikleri: BTreeMap::new(),
            sessiz_platformlar: Vec::new(),
        }
    }
}

impl BildirimAyari {
    /// Havuz/pencere için kalan oran eşiği (`0.0..=1.0`).
    pub fn esik_orani(&self, havuz_id: &str, pencere: &str) -> f64 {
        let yuzde = if havuz_id == "claude_max" {
            if pencere == "hafta" {
                self.claude_hafta_esik_yuzde
            } else {
                self.claude_5s_esik_yuzde
            }
        } else {
            self.diger_esik_yuzde
        };
        f64::from(yuzde.min(100)) / 100.0
    }

    /// Platform kimliği öncelikli kalan oran eşiği (`0.0..=1.0`).
    ///
    /// Platforma özel eşik tanımlıysa doğrudan onu döndürür; yoksa `esik_orani` yedeğine düşer.
    pub fn platform_esik_orani(&self, platform: &str, havuz_id: &str, pencere: &str) -> f64 {
        let norm = platform.trim().to_ascii_lowercase();
        if let Some(&yuzde) = self.platform_esikleri.get(&norm) {
            return f64::from(yuzde.clamp(1, 99)) / 100.0;
        }
        self.esik_orani(havuz_id, pencere)
    }

    /// Verilen platformun bildirimleri kullanıcı tarafından sessize alınmış mı?
    pub fn platform_sessiz_mi(&self, platform: &str) -> bool {
        let norm = platform.trim().to_ascii_lowercase();
        self.sessiz_platformlar
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&norm))
    }

    /// Bildirim türünün (`esik`, `sifirlandi`, `sifirlama_kredisi`, `bayat`) açık olup olmadığı.
    pub fn tur_acik_mi(&self, tur: &str) -> bool {
        match tur {
            "esik" => self.esik_bildirimi,
            "sifirlandi" => self.sifirlandi_bildirimi,
            "sifirlama_kredisi" => self.kredi_bildirimi,
            "bayat" => self.bayat_bildirimi,
            "tukenme" | "tahmin" => self.tahmin_bildirimi,
            _ => true,
        }
    }
}

/// Antigravity (`agy`) izin kapısı.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyOkumaIzni {
    #[default]
    Kapali,
    #[serde(alias = "istek üzerine", alias = "istek-uzerine")]
    IstekUzerine,
    Otomatik,
}

/// Antigravity (`agy`) bağlayıcısı ayarları. Varsayılan olarak kapalıdır.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgyAyari {
    pub okuma: AgyOkumaIzni,
    pub aralik_sn: u64,
    pub saatlik_tavan: u32,
    pub zaman_asimi_sn: u64,
    pub ikili_yol: Option<String>,
}

impl Default for AgyAyari {
    fn default() -> Self {
        Self {
            okuma: AgyOkumaIzni::Kapali,
            aralik_sn: 1800,
            saatlik_tavan: 6,
            zaman_asimi_sn: 45,
            ikili_yol: None,
        }
    }
}

/// DeepSeek bakiye API ayarları.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeepSeekAyari {
    pub etkin: bool,
    pub taban_url: Option<String>,
    pub anahtar_env: Option<String>,
    pub aralik_sn: u64,
    pub zaman_asimi_sn: u64,
}

impl Default for DeepSeekAyari {
    fn default() -> Self {
        Self {
            etkin: false,
            taban_url: None,
            anahtar_env: Some("DEEPSEEK_API_KEY".to_string()),
            aralik_sn: 900,
            zaman_asimi_sn: 10,
        }
    }
}

/// Ortamla çözümlenmiş DeepSeek ayarları (anahtar yalnız bellekte).
#[derive(Debug, Clone)]
pub struct CozulmusDeepSeek {
    pub etkin: bool,
    pub taban_url: String,
    pub anahtar: Option<GizliDeger>,
    pub aralik_sn: u64,
    pub zaman_asimi_sn: u64,
}

/// Servis zamanlayıcı ve kalp atışı ayarları.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServisAyari {
    /// Değişiklik olmasa da anlık dosyası kalp atışı aralığı (saniye).
    pub kalp_atisi_sn: u64,
    /// Kullanıcının etkin sayılması için maksimum boşta kalma süresi (saniye).
    pub bosta_esik_sn: u64,
}

impl Default for ServisAyari {
    fn default() -> Self {
        Self {
            kalp_atisi_sn: 60,
            bosta_esik_sn: 600,
        }
    }
}

/// Codex bağlayıcısı ayarları.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodexAyari {
    pub etkin: bool,
    pub etkin_aralik_sn: u64,
    pub bosta_aralik_sn: u64,
    pub zaman_asimi_sn: u64,
    pub rollout_yedek: bool,
    pub sema_bekcisi: bool,
}

impl Default for CodexAyari {
    fn default() -> Self {
        Self {
            etkin: true,
            etkin_aralik_sn: 300,
            bosta_aralik_sn: 1800,
            zaman_asimi_sn: 15,
            rollout_yedek: true,
            sema_bekcisi: true,
        }
    }
}

/// `ayarlar.toml` yanındaki son sağlam yedeğin yolu.
pub fn ayar_yedek_yolu(yol: &Path) -> std::path::PathBuf {
    yol.with_file_name("ayarlar.yedek.toml")
}

/// Bozuk ayar dosyasının saklanacağı adı üretir; tarih adın sonundadır
/// (ör. `ayarlar.bozuk 2026-11-16 14.30.05.toml`).
pub fn bozuk_ayar_adi(zaman_etiketi: &str) -> String {
    let temiz: String = zaman_etiketi
        .chars()
        .map(|c| match c {
            ':' | '/' | '\\' | '*' | '?' | '"' | '<' | '>' | '|' => '.',
            diger => diger,
        })
        .collect();
    format!("ayarlar.bozuk {}.toml", temiz.trim())
}

impl Ayarlar {
    /// Dosya varsa okur, yoksa varsayılanı döndürür. Bozuk dosyada hata döner (kurtarma için
    /// [`Ayarlar::yukle_kurtarmali`] kullanılır).
    pub fn yukle_veya_varsayilan(yol: &Path) -> Result<Self, DepoHatasi> {
        if !yol.exists() {
            return Ok(Self::default());
        }
        let icerik = std::fs::read_to_string(yol)?;
        let mut ayarlar: Self = toml::from_str(&icerik)?;
        ayarlar.sinirla();
        Ok(ayarlar)
    }

    /// Ayarları okur; ayrıştırılamıyorsa bozuk kopyayı saklayıp son sağlam yedekten, o da yoksa
    /// varsayılandan açar. Böylece servis ve masaüstü bozuk bir dosya yüzünden düşmez.
    pub fn yukle_kurtarmali(yol: &Path, zaman_etiketi: &str) -> AyarYukleme {
        let baytlar = match std::fs::read(yol) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return AyarYukleme {
                    ayarlar: Self::default(),
                    kurtarma: AyarKurtarma::Yok,
                };
            }
            Err(e) => {
                // Kilit/izin/paylaşım ihlali: dosyaya dokunma; çağıran yazmamalı ve yeniden denemeli.
                return AyarYukleme {
                    ayarlar: Self::default(),
                    kurtarma: AyarKurtarma::OkunamadiGecici {
                        hata: e.to_string(),
                    },
                };
            }
        };
        // UTF-8 olmayan dosya (ANSI/Windows-1254, UTF-16) bozuk dosya yolundan geçer: ham baytlar
        // saklanır, yedekten açılır. Baştaki UTF-8 BOM'u kabul edilir.
        let metin = std::str::from_utf8(&baytlar)
            .ok()
            .map(|m| m.strip_prefix('\u{feff}').unwrap_or(m));
        if let Some(m) = metin
            && let Ok(mut ayarlar) = toml::from_str::<Self>(m)
        {
            ayarlar.sinirla();
            return AyarYukleme {
                ayarlar,
                kurtarma: AyarKurtarma::Yok,
            };
        }

        let hedef = yol.with_file_name(bozuk_ayar_adi(zaman_etiketi));
        let bozuk_kopya = atomik_yaz(&hedef, &baytlar)
            .ok()
            .and(hedef.file_name().map(|a| a.to_string_lossy().into_owned()));

        if let Ok(icerik) = std::fs::read_to_string(ayar_yedek_yolu(yol))
            && let Ok(mut ayarlar) = toml::from_str::<Self>(&icerik)
        {
            ayarlar.sinirla();
            let _ = atomik_yaz(yol, icerik.as_bytes());
            return AyarYukleme {
                ayarlar,
                kurtarma: AyarKurtarma::YedektenAcildi { bozuk_kopya },
            };
        }

        let _ = atomik_yaz(yol, Self::ornek_toml_metni().as_bytes());
        AyarYukleme {
            ayarlar: Self::default(),
            kurtarma: AyarKurtarma::Varsayilan { bozuk_kopya },
        }
    }

    /// Kullanıcının elle yazabileceği değerleri güvenli aralıklara kıstırır.
    pub fn sinirla(&mut self) {
        self.servis.kalp_atisi_sn = self.servis.kalp_atisi_sn.clamp(10, 3600);
        self.codex.etkin_aralik_sn = self.codex.etkin_aralik_sn.clamp(60, 86_400);
        self.codex.bosta_aralik_sn = self.codex.bosta_aralik_sn.clamp(60, 86_400);
        self.agy.aralik_sn = self.agy.aralik_sn.clamp(600, 86_400);
        self.opencode.aralik_sn = self.opencode.aralik_sn.clamp(60, 86_400);
        self.dsh.aralik_sn = self.dsh.aralik_sn.clamp(60, 86_400);
        self.bildirim.sessiz_baslangic_saat %= 24;
        self.bildirim.sessiz_bitis_saat %= 24;
        self.bildirim.claude_5s_esik_yuzde = self.bildirim.claude_5s_esik_yuzde.clamp(1, 99);
        self.bildirim.claude_hafta_esik_yuzde = self.bildirim.claude_hafta_esik_yuzde.clamp(1, 99);
        self.bildirim.diger_esik_yuzde = self.bildirim.diger_esik_yuzde.clamp(1, 99);
        self.bildirim.platform_esikleri = self
            .bildirim
            .platform_esikleri
            .iter()
            .filter(|(k, _)| kart_adi_gecerli_mi(k))
            .take(64)
            .map(|(k, v)| (k.clone(), (*v).clamp(1, 99)))
            .collect();
        let mut temiz_sessiz: Vec<String> = Vec::new();
        for ad in &self.bildirim.sessiz_platformlar {
            if kart_adi_gecerli_mi(ad) && !temiz_sessiz.iter().any(|mevcut| mevcut == ad) {
                temiz_sessiz.push(ad.clone());
                if temiz_sessiz.len() >= 64 {
                    break;
                }
            }
        }
        self.bildirim.sessiz_platformlar = temiz_sessiz;
        self.masaustu.sinirla();
    }

    /// Varsayılan ayarları henüz yoksa atomik olarak oluşturur.
    pub fn yoksa_varsayilani_yaz(yol: &Path) -> Result<(), DepoHatasi> {
        if yol.exists() {
            return Ok(());
        }
        let ornek = Self::ornek_toml_metni();
        atomik_yaz(yol, ornek.as_bytes())
    }

    /// Mevcut ayarları atomik biçimde `ayarlar.toml` dosyasına kaydeder.
    pub fn kaydet(&self, yol: &Path) -> Result<(), DepoHatasi> {
        if let Some(ebeveyn) = yol.parent() {
            std::fs::create_dir_all(ebeveyn)?;
        }
        let mut metin = toml::to_string_pretty(self)?;
        if !metin.ends_with('\n') {
            metin.push('\n');
        }
        // Üzerine yazmadan önce mevcut sağlam dosyayı yedek olarak sakla (bozulursa buradan açılır).
        if let Ok(eski) = std::fs::read_to_string(yol)
            && toml::from_str::<Self>(&eski).is_ok()
        {
            let _ = atomik_yaz(&ayar_yedek_yolu(yol), eski.as_bytes());
        }
        atomik_yaz(yol, metin.as_bytes())
    }

    /// Yorum satırlarıyla temiz varsayılan `ayarlar.toml` metni üretir.
    pub fn ornek_toml_metni() -> String {
        let mut metin = toml::to_string_pretty(&Self::default()).unwrap_or_default();
        metin.push_str(
            "\n# Antigravity okuma modu: \"kapali\" | \"istek_uzerine\" | \"otomatik\"\n\
             # DeepSeek bakiye okuması için DEEPSEEK_API_KEY ortam değişkeni kullanılır.\n",
        );
        metin
    }

    /// DeepSeek ayarını ortam değişkenleri ve kimlik kasasıyla (varsa) çözer.
    pub fn deepseek_coz(&self) -> CozulmusDeepSeek {
        let mut cozulmus = self.deepseek_coz_ortamla(ortam_veya_hkcu_metin);
        if cozulmus.anahtar.is_none()
            && let Some(ad) = self.deepseek.anahtar_env.as_deref()
        {
            cozulmus.anahtar = kimlik_oku_gizli(ad);
        }
        cozulmus
    }

    /// Test edilebilirlik için özel ortam okuyucu ile DeepSeek ayarını çözer.
    pub fn deepseek_coz_ortamla<F>(&self, ortam_oku: F) -> CozulmusDeepSeek
    where
        F: Fn(&str) -> Option<String>,
    {
        let anahtar = self
            .deepseek
            .anahtar_env
            .as_deref()
            .and_then(&ortam_oku)
            .and_then(GizliDeger::yeni);
        let taban_url = self
            .deepseek
            .taban_url
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://api.deepseek.com".to_string());

        CozulmusDeepSeek {
            etkin: self.deepseek.etkin,
            taban_url,
            anahtar,
            aralik_sn: self.deepseek.aralik_sn,
            zaman_asimi_sn: self.deepseek.zaman_asimi_sn,
        }
    }
}

/// Önce süreç ortam değişkenine, yoksa Windows `HKCU\Environment` anahtarına bakar.
pub fn ortam_veya_hkcu_metin(ad: &str) -> Option<String> {
    if let Ok(deger) = std::env::var(ad) {
        let temiz = deger.trim().to_string();
        if !temiz.is_empty() {
            return Some(temiz);
        }
    }
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;

        if let Ok(env_key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Environment")
            && let Ok(deger) = env_key.get_value::<String, _>(ad)
        {
            let temiz = deger.trim().to_string();
            if !temiz.is_empty() {
                return Some(temiz);
            }
        }
    }
    None
}

/// Anahtarın süreç ortamı, `HKCU\Environment` veya Windows Credential Manager'da varlığını kontrol eder (değeri döndürmez).
pub fn ortam_hkcu_veya_kimlik_var_mi(ad: &str) -> bool {
    ortam_veya_hkcu_metin(ad).is_some() || kimlik_var_mi(ad)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gecici_dizin(ad: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("cetvel-ayar-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("gecici dizin");
        d
    }

    fn masaustu_oku(toml_metni: &str) -> MasaustuAyari {
        toml::from_str::<Ayarlar>(toml_metni)
            .expect("gecerli toml")
            .masaustu
    }

    #[test]
    fn desteklenen_degerler_ve_dogrulayicilar_tam_kumeyi_kabul_eder() {
        for dil in [
            "tr", "en", "de", "fr", "es", "pt-BR", "ru", "ja", "ko", "zh-Hans", "zh-Hant",
        ] {
            assert!(dil_gecerli_mi(dil), "{dil}");
        }
        assert_eq!(DESTEKLENEN_DILLER.len(), 11);
        for dil in ["", "xx", "pt", "pt-br", "zh", "EN", "zh-hans"] {
            assert!(!dil_gecerli_mi(dil), "{dil}");
        }
        for renk in ["terakota", "okyanus", "adacayi", "karbon"] {
            assert!(renk_temasi_gecerli_mi(renk), "{renk}");
        }
        for renk in ["murekkep", "kagit", "terminal", "", "Karbon"] {
            assert!(!renk_temasi_gecerli_mi(renk), "{renk}");
        }
        for kip in ["acik", "koyu", "sistem"] {
            assert!(kip_gecerli_mi(kip), "{kip}");
        }
        for kip in ["", "dark", "Koyu", "murekkep"] {
            assert!(!kip_gecerli_mi(kip), "{kip}");
        }
        assert!(deger_yonu_gecerli_mi("kalan"));
        assert!(deger_yonu_gecerli_mi("kullanilan"));
        assert!(!deger_yonu_gecerli_mi("used"));
        assert_eq!(MasaustuAyari::default().deger_yonu, "kalan");
        assert_eq!(
            masaustu_oku("[masaustu]\ndeger_yonu = \"kullanilan\"\n").deger_yonu,
            "kullanilan"
        );
        assert_eq!(
            masaustu_oku("[masaustu]\ndeger_yonu = \"gecersiz\"\n").deger_yonu,
            "kalan"
        );
    }

    #[test]
    fn eski_tema_degerleri_yeni_renk_ve_kipe_eslenir() {
        assert_eq!(eski_temayi_esle("murekkep"), Some(("karbon", "koyu")));
        assert_eq!(eski_temayi_esle("kagit"), Some(("terakota", "acik")));
        assert_eq!(eski_temayi_esle("terminal"), Some(("karbon", "koyu")));
        assert_eq!(eski_temayi_esle("okyanus"), None);
        assert_eq!(eski_temayi_esle(""), None);
        // Eşleme çıktıları geçerli yeni değerler olmalı.
        for eski in ["murekkep", "kagit", "terminal"] {
            let (renk, kip) = eski_temayi_esle(eski).expect("eslenir");
            assert!(renk_temasi_gecerli_mi(renk) && kip_gecerli_mi(kip));
        }
        assert_eq!(eski_tema_gorunumu("koyu"), "murekkep");
        assert_eq!(eski_tema_gorunumu("acik"), "kagit");
        assert_eq!(eski_tema_gorunumu("sistem"), "kagit");
    }

    #[test]
    fn eski_ayar_dosyasi_okunur_ve_yeni_alanlara_cevrilir() {
        let m =
            masaustu_oku("[masaustu]\ndil = \"en\"\ntema = \"murekkep\"\ntepsiye_kucult = false\n");
        assert_eq!(m.dil, "en");
        assert_eq!((m.renk_temasi.as_str(), m.kip.as_str()), ("karbon", "koyu"));
        assert!(!m.tepsiye_kucult);
        assert!(!m.windows_ile_baslat);

        let m = masaustu_oku("[masaustu]\ntema = \"kagit\"\n");
        assert_eq!(
            (m.renk_temasi.as_str(), m.kip.as_str()),
            ("terakota", "acik")
        );
        let m = masaustu_oku("[masaustu]\ntema = \"terminal\"\n");
        assert_eq!((m.renk_temasi.as_str(), m.kip.as_str()), ("karbon", "koyu"));
    }

    #[test]
    fn yeni_alanlar_eski_tema_alanindan_oncelikli_ve_gecersizler_varsayilana_doner() {
        // Her iki biçim bir arada: alan alan yeni değer kazanır, eksik alan eski değerden gelir.
        let m = masaustu_oku("[masaustu]\ntema = \"murekkep\"\nrenk_temasi = \"okyanus\"\n");
        assert_eq!(
            (m.renk_temasi.as_str(), m.kip.as_str()),
            ("okyanus", "koyu")
        );
        let m = masaustu_oku("[masaustu]\ntema = \"kagit\"\nkip = \"sistem\"\n");
        assert_eq!(
            (m.renk_temasi.as_str(), m.kip.as_str()),
            ("terakota", "sistem")
        );

        // Bilinmeyen eski/yeni değerler varsayılana düşer, dosya yine okunur.
        let m = masaustu_oku("[masaustu]\ntema = \"gokkusagi\"\ndil = \"xx\"\n");
        assert_eq!(m, MasaustuAyari::default());
        let m =
            masaustu_oku("[masaustu]\nrenk_temasi = \"murekkep\"\nkip = \"dark\"\ndil = \"pt\"\n");
        assert_eq!(m, MasaustuAyari::default());

        // Tablo yoksa ya da boşsa varsayılanlar.
        assert_eq!(masaustu_oku(""), MasaustuAyari::default());
        assert_eq!(masaustu_oku("[masaustu]\n"), MasaustuAyari::default());
        let v = MasaustuAyari::default();
        assert_eq!(
            (v.dil.as_str(), v.renk_temasi.as_str(), v.kip.as_str()),
            ("tr", "terakota", "koyu")
        );
    }

    #[test]
    fn tum_yeni_degerler_kayit_ve_yuklemede_korunur_eski_tema_yazilmaz() {
        let d = gecici_dizin("yeni-tema");
        let yol = d.join("ayarlar.toml");
        for dil in DESTEKLENEN_DILLER {
            for renk in RENK_TEMALARI {
                for kip in KIPLER {
                    let mut a = Ayarlar::default();
                    a.masaustu.dil = dil.to_string();
                    a.masaustu.renk_temasi = renk.to_string();
                    a.masaustu.kip = kip.to_string();
                    a.kaydet(&yol).expect("kaydet");
                    let metin = std::fs::read_to_string(&yol).expect("oku");
                    assert!(!metin.contains("tema ="), "eski alan yazılmamalı: {metin}");
                    let geri = Ayarlar::yukle_kurtarmali(&yol, "x");
                    assert_eq!(geri.kurtarma, AyarKurtarma::Yok);
                    assert_eq!(geri.ayarlar.masaustu, a.masaustu);
                }
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn eski_dosya_yuklenir_diger_ayarlar_bozulmaz_ve_ilk_kayitta_yeni_bicime_gecer() {
        let d = gecici_dizin("eski-dosya");
        let yol = d.join("ayarlar.toml");
        std::fs::write(
            &yol,
            "[codex]\netkin_aralik_sn = 900\n[masaustu]\ndil = \"en\"\ntema = \"terminal\"\nwindows_ile_baslat = true\n",
        )
        .expect("yaz");
        let y = Ayarlar::yukle_kurtarmali(&yol, "x");
        assert_eq!(y.kurtarma, AyarKurtarma::Yok);
        assert_eq!(y.ayarlar.codex.etkin_aralik_sn, 900);
        assert_eq!(y.ayarlar.masaustu.dil, "en");
        assert_eq!(y.ayarlar.masaustu.renk_temasi, "karbon");
        assert_eq!(y.ayarlar.masaustu.kip, "koyu");
        assert!(y.ayarlar.masaustu.windows_ile_baslat);
        y.ayarlar.kaydet(&yol).expect("kaydet");
        let metin = std::fs::read_to_string(&yol).expect("oku");
        assert!(metin.contains("renk_temasi = \"karbon\""));
        assert!(metin.contains("kip = \"koyu\""));
        assert!(!metin.contains("terminal"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sinirla_gecersiz_masaustu_degerlerini_varsayilana_cevirir_gecerlileri_korur() {
        let mut a = Ayarlar::default();
        a.masaustu.dil = "ja".to_string();
        a.masaustu.renk_temasi = "adacayi".to_string();
        a.masaustu.kip = "acik".to_string();
        a.sinirla();
        assert_eq!(
            (
                a.masaustu.dil.as_str(),
                a.masaustu.renk_temasi.as_str(),
                a.masaustu.kip.as_str()
            ),
            ("ja", "adacayi", "acik")
        );
        a.masaustu.dil = "xx".to_string();
        a.masaustu.renk_temasi = "murekkep".to_string();
        a.masaustu.kip = "gece".to_string();
        a.sinirla();
        assert_eq!(a.masaustu, MasaustuAyari::default());
    }

    #[test]
    fn bozuk_ayar_yedekten_acilir_ve_bozuk_kopya_saklanir() {
        let d = gecici_dizin("yedek");
        let yol = d.join("ayarlar.toml");
        let mut ilk = Ayarlar::default();
        ilk.agy.okuma = AgyOkumaIzni::IstekUzerine;
        ilk.kaydet(&yol).expect("ilk kayit");
        let mut ikinci = ilk.clone();
        ikinci.codex.etkin_aralik_sn = 600;
        ikinci.kaydet(&yol).expect("ikinci kayit (yedek olusur)");
        assert!(ayar_yedek_yolu(&yol).exists());

        std::fs::write(&yol, "bu [ gecerli toml degil").expect("bozuk yaz");
        let sonuc = Ayarlar::yukle_kurtarmali(&yol, "2026-11-16 14:30:05");
        assert!(matches!(
            sonuc.kurtarma,
            AyarKurtarma::YedektenAcildi { .. }
        ));
        assert!(sonuc.kurtarma.aciklama().is_some());
        assert_eq!(sonuc.ayarlar.agy.okuma, AgyOkumaIzni::IstekUzerine);
        assert!(d.join("ayarlar.bozuk 2026-11-16 14.30.05.toml").exists());
        assert!(Ayarlar::yukle_veya_varsayilan(&yol).is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn utf8_olmayan_ayar_bozuk_sayilir_ham_bayt_saklanir_yedekten_acilir() {
        let d = gecici_dizin("ansi");
        let yol = d.join("ayarlar.toml");
        let mut ilk = Ayarlar::default();
        ilk.agy.okuma = AgyOkumaIzni::Otomatik;
        ilk.kaydet(&yol).expect("ilk");
        ilk.kaydet(&yol).expect("ikinci (yedek oluşur)");
        // Windows-1254 "ğ" (0xF0) içeren yorum: geçerli UTF-8 değil.
        let ansi: Vec<u8> = b"# de\xf0i\xfeiklik\n[agy]\nokuma = \"kapali\"\n".to_vec();
        std::fs::write(&yol, &ansi).expect("ansi yaz");
        let sonuc = Ayarlar::yukle_kurtarmali(&yol, "2026-11-16 15.00.00");
        assert!(matches!(
            sonuc.kurtarma,
            AyarKurtarma::YedektenAcildi { .. }
        ));
        assert!(sonuc.kurtarma.yazmaya_uygun());
        assert_eq!(sonuc.ayarlar.agy.okuma, AgyOkumaIzni::Otomatik);
        assert_eq!(
            std::fs::read(d.join("ayarlar.bozuk 2026-11-16 15.00.00.toml")).unwrap(),
            ansi
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn utf8_bom_ile_kaydedilen_ayar_okunur() {
        let d = gecici_dizin("bom");
        let yol = d.join("ayarlar.toml");
        std::fs::write(&yol, "\u{feff}[codex]\netkin_aralik_sn = 600\n").expect("yaz");
        let sonuc = Ayarlar::yukle_kurtarmali(&yol, "x");
        assert_eq!(sonuc.kurtarma, AyarKurtarma::Yok);
        assert_eq!(sonuc.ayarlar.codex.etkin_aralik_sn, 600);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn gecici_okuma_hatasi_yazmaya_uygun_degil() {
        // Dizin yolu `read` ile okunamaz (NotFound değil): geçici hata gibi davranır.
        let d = gecici_dizin("gecici");
        let sonuc = Ayarlar::yukle_kurtarmali(&d, "x");
        assert!(matches!(
            sonuc.kurtarma,
            AyarKurtarma::OkunamadiGecici { .. }
        ));
        assert!(!sonuc.kurtarma.yazmaya_uygun());
        assert!(sonuc.kurtarma.aciklama().is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bozuk_ayar_yedek_yoksa_varsayilanla_acilir() {
        let d = gecici_dizin("varsayilan");
        let yol = d.join("ayarlar.toml");
        std::fs::write(&yol, "[[[").expect("bozuk yaz");
        let sonuc = Ayarlar::yukle_kurtarmali(&yol, "2026-11-16 09.00");
        assert!(matches!(sonuc.kurtarma, AyarKurtarma::Varsayilan { .. }));
        assert_eq!(sonuc.ayarlar, Ayarlar::default());
        assert!(Ayarlar::yukle_veya_varsayilan(&yol).is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn saglam_ayar_kurtarma_gerektirmez_ve_sinirlar_uygulanir() {
        let d = gecici_dizin("saglam");
        let yol = d.join("ayarlar.toml");
        std::fs::write(
            &yol,
            "[codex]\netkin_aralik_sn = 1\n[masaustu]\ndil = \"xx\"\n",
        )
        .expect("yaz");
        let sonuc = Ayarlar::yukle_kurtarmali(&yol, "2026-11-16 09.00");
        assert_eq!(sonuc.kurtarma, AyarKurtarma::Yok);
        assert_eq!(sonuc.ayarlar.codex.etkin_aralik_sn, 60);
        assert_eq!(sonuc.ayarlar.masaustu.dil, "tr");
        assert_eq!(sonuc.ayarlar.masaustu.renk_temasi, "terakota");
        assert_eq!(sonuc.ayarlar.masaustu.kip, "koyu");
        assert!(sonuc.ayarlar.masaustu.tepsiye_kucult);
        assert!(sonuc.ayarlar.http.otomatik_port);
        let yok = Ayarlar::yukle_kurtarmali(&d.join("olmayan.toml"), "x");
        assert_eq!(yok.kurtarma, AyarKurtarma::Yok);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bildirim_esikleri_ve_tur_anahtarlari_ayardan_gelir() {
        let mut b = BildirimAyari::default();
        assert!((b.esik_orani("claude_max", "5s") - 0.15).abs() < 1e-9);
        assert!((b.esik_orani("claude_max", "hafta") - 0.30).abs() < 1e-9);
        assert!((b.esik_orani("codex_plus", "hafta") - 0.20).abs() < 1e-9);
        b.diger_esik_yuzde = 35;
        b.bayat_bildirimi = false;
        assert!((b.esik_orani("deepseek", "gun") - 0.35).abs() < 1e-9);
        assert!(!b.tur_acik_mi("bayat"));
        assert!(b.tur_acik_mi("esik"));
        assert_eq!(
            bozuk_ayar_adi("2026-11-16 14:30"),
            "ayarlar.bozuk 2026-11-16 14.30.toml"
        );
    }

    #[test]
    fn toml_ayristirma_calisir() {
        let toml_src = r#"
[servis]
kalp_atisi_sn = 30

[codex]
etkin = false
"#;
        let ayar: Ayarlar = toml::from_str(toml_src).expect("toml");
        assert_eq!(ayar.servis.kalp_atisi_sn, 30);
        assert!(!ayar.codex.etkin);
    }

    #[test]
    fn agy_ve_deepseek_varsayilanlari_ve_turkce_aliaslar_calisir() {
        let varsayilan = Ayarlar::default();
        assert_eq!(varsayilan.agy.okuma, AgyOkumaIzni::Kapali);
        assert!(!varsayilan.deepseek.etkin);
        assert_eq!(varsayilan.http.port, 43_117);
        assert!(
            varsayilan.bildirim.sessiz_baslangic_saat == 0
                && varsayilan.bildirim.sessiz_bitis_saat == 8
        );

        let toml_src = r#"
[agy]
okuma = "istek üzerine"

[deepseek]
etkin = true
anahtar_env = "ORNEK_DS_KEY"
"#;
        let ayar: Ayarlar = toml::from_str(toml_src).expect("toml");
        assert_eq!(ayar.agy.okuma, AgyOkumaIzni::IstekUzerine);
        let ds = ayar.deepseek_coz_ortamla(|ad| match ad {
            "ORNEK_DS_KEY" => Some("ds-test-key".to_string()),
            _ => None,
        });
        assert!(ds.etkin);
        assert_eq!(ds.taban_url, "https://api.deepseek.com");
        assert_eq!(
            ds.anahtar.as_ref().map(|k| k.acik_metin()),
            Some("ds-test-key")
        );
    }

    #[test]
    fn bildirim_platform_esigi_ve_sessize_alma_calisir_ve_sinirlanir() {
        let toml_src = r#"
[bildirim]
sessiz_platformlar = ["codex", "GECERSIZ-PLATFORM", "codex", "agy"]

[bildirim.platform_esikleri]
codex = 0
claude = 120
"BAD KEY" = 50
"#;
        let mut ayar: Ayarlar = toml::from_str(toml_src).expect("toml");
        ayar.sinirla();
        assert!(ayar.bildirim.platform_sessiz_mi("codex"));
        assert!(ayar.bildirim.platform_sessiz_mi("CODEX"));
        assert!(!ayar.bildirim.platform_sessiz_mi("claude"));
        assert_eq!(ayar.bildirim.sessiz_platformlar, vec!["codex", "agy"]);
        assert!(!ayar.bildirim.platform_esikleri.contains_key("BAD KEY"));
        assert_eq!(
            ayar.bildirim
                .platform_esik_orani("codex", "codex_plus", "5s"),
            0.01
        );
        assert_eq!(
            ayar.bildirim
                .platform_esik_orani("claude", "claude_max", "5s"),
            0.99
        );
        // Tanımsız platform genel yedeğe düşer
        assert_eq!(
            ayar.bildirim.platform_esik_orani("agy", "agy_gemini", "5s"),
            0.20
        );
    }
}

#[cfg(test)]
mod kart_testleri {
    use super::*;

    #[test]
    fn kart_sinirlari_ascii_uzunluk_ve_oge_sayisi() {
        assert!(kart_listesi_gecerli_mi(&vec!["a0_".repeat(13); 64]));
        assert!(kart_adi_gecerli_mi(&"a".repeat(40)));
        assert!(!kart_adi_gecerli_mi(&"a".repeat(41)));
        assert!(!kart_listesi_gecerli_mi(&vec!["a".into(); 65]));
        for ad in ["", "A", "ş", "a-b", "a b", "a\n"] {
            assert!(!kart_adi_gecerli_mi(ad));
        }
    }

    #[test]
    fn eski_ayar_ve_kart_kayit_geri_okumasi_uyumlu() {
        let eski: MasaustuAyari = toml::from_str("dil = 'en'").unwrap();
        assert!(eski.kart_sirasi.is_empty());
        assert_eq!(eski.pencere_bosta_sn, 60);
        let a = MasaustuAyari {
            kart_sirasi: vec!["codex_plus".into(), "claude_max".into()],
            gizli_kartlar: vec!["deepseek".into()],
            sabit_kartlar: vec!["codex_plus".into()],
            pencere_bosta_sn: 120,
            ..Default::default()
        };
        let metin = toml::to_string(&a).unwrap();
        assert_eq!(toml::from_str::<MasaustuAyari>(&metin).unwrap(), a);
    }

    #[test]
    fn sinirla_elle_bozulmus_kart_listelerini_temizler_ve_64_ile_kirpar() {
        let mut fazla_kart: Vec<String> = (0..70).map(|i| format!("kart_{i}")).collect();
        fazla_kart.insert(1, "GECERSIZ-AD".into());
        fazla_kart.insert(2, "".into());
        let toml_src = format!(
            "[masaustu]\npencere_bosta_sn = 0\nkart_sirasi = {:?}\ngizli_kartlar = [\"ok_1\", \"kötü\"]\nsabit_kartlar = [\"A\", \"ok_2\"]\n",
            fazla_kart
        );
        let ayar: Ayarlar = toml::from_str(&toml_src).unwrap();
        assert_eq!(ayar.masaustu.pencere_bosta_sn, 1);
        assert_eq!(ayar.masaustu.kart_sirasi.len(), 64);
        assert_eq!(ayar.masaustu.kart_sirasi[0], "kart_0");
        assert_eq!(ayar.masaustu.kart_sirasi[1], "kart_1");
        assert_eq!(ayar.masaustu.gizli_kartlar, ["ok_1"]);
        assert_eq!(ayar.masaustu.sabit_kartlar, ["ok_2"]);
    }
}
