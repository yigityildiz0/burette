//! Kalıcı ve atomik depolama, kullanıcı yolları, ayarlar, SQLite geçmişi ve sır koruması.

pub mod atomik;
pub mod ayarlar;
pub mod baglanti;
pub mod gecmis;
pub mod gizli;
pub mod kimlik;
pub mod kisayol;
pub mod silme;
pub mod yollar;

use thiserror::Error;

#[cfg(windows)]
pub use atomik::nesne_sahibi_sid;
pub use atomik::{
    atomik_yaz, atomik_yaz_json, gecerli_kullanici_sid, kullanici_dacl_uygula, surec_kullanici_sid,
};
pub use ayarlar::{
    AgyAyari, AgyOkumaIzni, AjanAyari, AyarKurtarma, AyarYukleme, Ayarlar, BildirimAyari,
    CodexAyari, CozulmusDeepSeek, DeepSeekAyari, HttpAyari, MasaustuAyari, ServisAyari,
    YerelSayacAyari, ayar_yedek_yolu, bozuk_ayar_adi, ortam_hkcu_veya_kimlik_var_mi,
    ortam_veya_hkcu_metin,
};
pub use ayarlar::{
    DEGER_YONLERI, DESTEKLENEN_DILLER, KIPLER, RENK_TEMALARI, deger_yonu_gecerli_mi,
    dil_gecerli_mi, eski_tema_gorunumu, eski_temayi_esle, kip_gecerli_mi, renk_temasi_gecerli_mi,
};
pub use baglanti::{
    BaglantiBilgisi, baglanti_bilgisi_oku, baglanti_bilgisi_yaz, csprng_jeton_uret,
    jeton_dosyasi_yaz, jeton_yukle_veya_uret,
};
pub use gecmis::{GecmisDeposu, GecmisOrnegi};
pub use gizli::GizliDeger;
pub use kimlik::{
    AZAMI_ANAHTAR_BAYT, api_anahtari_dogrula, kimlik_adi_hedeften, kimlik_hedef_adi, kimlik_kaydet,
    kimlik_listele, kimlik_oku_gizli, kimlik_sil, kimlik_var_mi,
};
pub use kisayol::{CETVEL_AUMID, KisayolBilgisi, kisayol_aumid_oku, kisayol_olustur};
pub use silme::{SilmeSonucu, dizini_dayanikli_sil};
pub use yollar::CetvelYollar;

/// Depolama katmanı hataları.
#[derive(Debug, Error)]
pub enum DepoHatasi {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serileştirme hatası: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML ayar ayrıştırma hatası: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML ayar serileştirme hatası: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("SQLite geçmiş hatası: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("gerekli ortam değişkeni bulunamadı: {0}")]
    OrtamEksik(String),
    #[error("geçersiz dosya yolu: {0}")]
    GecersizYol(String),
    #[error("güvenlik/DACL hatası: {0}")]
    Guvenlik(String),
    #[error("rastgele sayı üreteci hatası: {0}")]
    Rastgele(String),
}
