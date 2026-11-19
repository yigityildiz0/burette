//! Normalize ajan modeli: `GET /v1/ajanlar` ve `ajanlar.json` bu türlerle yazılır.
//!
//! Gizlilik sınırı bu türlerdir: istem/yanıt metni, komut satırı ve dosya yolu taşıyan alan yoktur.
//! Ad, model ve efor yalnız kısa, doğrulanmış kimlik metinleridir (bkz. [`kisa_ad`]).

use std::collections::BTreeMap;

use cetvel_model::UtcZaman;
use serde::{Deserialize, Serialize};

/// `ajanlar.json` biçim sürümü.
pub const AJAN_SEMA_SURUMU: u32 = 1;

/// Ajanın çalıştığı platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Codex,
    Antigravity,
    /// Bu okuyucunun görmediği başka Claude oturumları için ayrılmış değer (bugün kaynak yok).
    ClaudeDis,
}

impl Platform {
    /// Kimlik karmasında ve kaynak özetinde kullanılan kısa ad.
    pub fn kod(self) -> &'static str {
        match self {
            Platform::Codex => "codex",
            Platform::Antigravity => "antigravity",
            Platform::ClaudeDis => "claude_dis",
        }
    }
}

/// Ajanın anlık durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AjanDurumu {
    Calisiyor,
    /// Kullanıcı onayı / girdisi bekliyor ya da tur henüz başlamadı.
    Bekliyor,
    Bitti,
    Hata,
    Iptal,
    /// Çalışıyor görünürken uzun süredir etkinlik yok.
    Askida,
}

impl AjanDurumu {
    /// Bitmiş (bitti, hata, iptal) durumlar kısa süre gösterilip listeden düşer.
    pub fn bitmis_mi(self) -> bool {
        matches!(
            self,
            AjanDurumu::Bitti | AjanDurumu::Hata | AjanDurumu::Iptal
        )
    }
}

/// Son etkinliğin türü (ham metin değil).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Etkinlik {
    Okuma,
    Yazma,
    Arama,
    Komut,
    Web,
    Dusunme,
    Test,
    Derleme,
    /// Sürüm denetimi (git).
    Surum,
    /// Paket kurulumu.
    Kurulum,
    /// Genel araç / MCP çağrısı.
    Arac,
    /// Alt ajan başlatma veya alt ajanla haberleşme.
    AltAjan,
    /// Yanıt yazma.
    Yanit,
    Bekleme,
    Gorsel,
    #[serde(other)]
    Bilinmiyor,
}

impl Etkinlik {
    /// Dış olay dosyalarındaki serbest türü doğrulanmış etkinliğe çevirir (bilinmeyen → `bilinmiyor`).
    pub fn addan(ad: &str) -> Self {
        serde_json::from_value(serde_json::Value::String(ad.trim().to_ascii_lowercase()))
            .unwrap_or(Etkinlik::Bilinmiyor)
    }
}

/// Hata sınıfı. `429` hız/kota sınırıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HataTuru {
    #[serde(rename = "429")]
    HizSiniri,
    ZamanAsimi,
    IzinReddi,
    AracHatasi,
    BaglamAsimi,
    Ag,
    #[serde(other)]
    Bilinmiyor,
}

/// Ajan başına sayaçlar (yalnız sayılar).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AjanSayilari {
    /// Araç / komut çağrısı sayısı.
    pub arac: u64,
    /// Başarısız araç çağrısı sayısı (ajan çalışmaya devam edebilir).
    pub arac_hatasi: u64,
    /// Başlattığı alt ajan sayısı.
    pub alt_ajan: u64,
    /// Platformun bildirdiği adım sayısı (Antigravity); bilinmiyorsa yok.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adim: Option<u64>,
}

/// Tüketicilere (Pixel Office, masaüstü) yayınlanan tek ajan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ajan {
    /// Platform + yerel kimliğin karması (`cx-…`, `ag-…`, `ev-…`); ham kimlik yayınlanmaz.
    pub kimlik: String,
    pub platform: Platform,
    /// Ebeveyn ajanın karması. Ebeveyn listede olmayabilir (bitmiş ve düşmüş olabilir).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ebeveyn: Option<String>,
    /// Ağaçtaki derinlik (kök 0).
    pub derinlik: u32,
    /// Kısa görünen ad (rol/takma ad); istem metni değildir.
    pub ad: String,
    /// Yalnız platform kesin model kimliği bildirdiyse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efor: Option<String>,
    pub durum: AjanDurumu,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etkinlik: Option<Etkinlik>,
    /// Yalnız `durum == hata` iken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hata_turu: Option<HataTuru>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baslama: Option<UtcZaman>,
    pub son_etkinlik: UtcZaman,
    /// Bitmiş durumlarda bitiş anı.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitis: Option<UtcZaman>,
    pub sayilar: AjanSayilari,
}

/// Bir kaynağın okunma durumu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KaynakDurumu {
    Izleniyor,
    #[default]
    Kapali,
    Bulunamadi,
    Okunamadi,
}

/// Kaynak başına özet (yol içermez).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KaynakOzeti {
    pub durum: KaynakDurumu,
    /// Bu anlıkta listelenen ajan sayısı.
    pub ajan: usize,
}

/// `GET /v1/ajanlar` ve `%LOCALAPPDATA%\Cetvel\ajanlar.json` gövdesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AjanAnlik {
    pub surum: u32,
    pub zaman: UtcZaman,
    /// İçerik değiştikçe artar (yalnız zaman değişince artmaz).
    pub sira: u64,
    pub kaynaklar: BTreeMap<String, KaynakOzeti>,
    pub ajanlar: Vec<Ajan>,
}

impl AjanAnlik {
    pub fn bos(zaman: UtcZaman) -> Self {
        Self {
            surum: AJAN_SEMA_SURUMU,
            zaman,
            sira: 0,
            kaynaklar: BTreeMap::new(),
            ajanlar: Vec::new(),
        }
    }

    /// `zaman` ve `sira` dışındaki içerik aynı mı (yayın kararı için).
    pub fn icerik_ayni_mi(&self, diger: &AjanAnlik) -> bool {
        self.kaynaklar == diger.kaynaklar && self.ajanlar == diger.ajanlar
    }
}

/// Platform + yerel kimlikten kararlı kısa karma üretir (FNV-1a 64; sürümler arası sabit).
pub fn kimlik_karma(platform: Platform, yerel: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in platform.kod().bytes().chain([0u8]).chain(yerel.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let onek = match platform {
        Platform::Codex => "cx",
        Platform::Antigravity => "ag",
        Platform::ClaudeDis => "cl",
    };
    format!("{onek}-{h:016x}")
}

/// Platformun verdiği rol/takma adı güvenli kısa ada çevirir. Yol, boşluklu uzun metin ya da
/// izin verilmeyen karakter içeriyorsa `None` döner (istem metni ad olarak sızmaz).
pub fn kisa_ad(ham: &str) -> Option<String> {
    let t = ham.trim();
    if t.is_empty() || t.chars().count() > 32 || t.split_whitespace().count() > 3 {
        return None;
    }
    let uygun = t
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.'));
    if !uygun || t.contains("..") {
        return None;
    }
    Some(t.to_string())
}

/// Model/efor kimliği: kısa, boşluksuz, yalnız `[A-Za-z0-9._:/-]`.
pub fn model_kimligi(ham: &str) -> Option<String> {
    let t = ham.trim();
    if t.is_empty() || t.len() > 64 {
        return None;
    }
    let uygun = t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/' | '@'));
    // Yol gibi görünen değer (`C:\…`, `/home/…`) model kimliği sayılmaz.
    if !uygun || t.starts_with('/') || t.contains("//") {
        return None;
    }
    Some(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn karma_kararli_ve_platforma_bagli() {
        let a = kimlik_karma(Platform::Codex, "abc");
        assert_eq!(a, kimlik_karma(Platform::Codex, "abc"));
        assert_ne!(a, kimlik_karma(Platform::Antigravity, "abc"));
        assert!(a.starts_with("cx-") && a.len() == 19);
        assert!(!a.contains("abc"));
    }

    #[test]
    fn kisa_ad_yol_ve_istemi_reddeder() {
        assert_eq!(kisa_ad("research").as_deref(), Some("research"));
        assert_eq!(
            kisa_ad("web-search-agent").as_deref(),
            Some("web-search-agent")
        );
        assert!(kisa_ad(r"C:\Users\x\proje").is_none());
        assert!(kisa_ad("lütfen şu dosyayı okuyup özetle ve bana yaz").is_none());
        assert!(kisa_ad("../gizli").is_none());
        assert!(model_kimligi("gpt-6.1-sol").is_some());
        assert!(model_kimligi("/etc/passwd").is_none());
        assert!(model_kimligi("bir model adı").is_none());
    }

    #[test]
    fn hata_turu_429_olarak_yazilir_ve_bilinmeyen_etkinlik_duser() {
        assert_eq!(
            serde_json::to_string(&HataTuru::HizSiniri).unwrap(),
            "\"429\""
        );
        assert_eq!(Etkinlik::addan("Test"), Etkinlik::Test);
        assert_eq!(Etkinlik::addan("rm -rf /"), Etkinlik::Bilinmiyor);
    }
}
