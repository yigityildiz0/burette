//! Platform kota bağlayıcıları (`Codex`, `Antigravity`, `DeepSeek`) ve kotası olmayan yerel
//! araç sayaçları (OpenCode, DeepSeek Harness).

pub mod agy;
pub mod codex;
pub mod deepseek;
pub mod job_object;
pub mod yerel_sayac;

use std::future::Future;
use std::pin::Pin;

use cetvel_model::{HavuzAnlik, UtcZaman};

pub use agy::{AntigravityBaglayicisi, agy_cikti_ayristir, agy_ikili_coz};
pub use codex::{CodexBaglayicisi, codex_app_server_oku, codex_ikili_coz, codex_yanit_ayristir};
pub use deepseek::{DeepSeekBaglayicisi, deepseek_yanit_ayristir};
pub use job_object::{JobSuresKorumasi, askidaki_sureci_devam_ettir};
pub use yerel_sayac::{
    YerelArac, YerelSayacBaglayicisi, dsh_dizini_coz, opencode_dizini_coz, yerel_arac_var_mi,
    yerel_sayac_oku,
};

/// Sınırlı gövde okuma hatası.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GovdeHatasi {
    /// `Content-Length` ya da akan veri sınırı aştı (gövde belleğe tamamen alınmadı).
    CokBuyuk,
    /// Ağ/okuma hatası.
    Ag,
}

/// HTTP yanıt gövdesini en fazla `sinir` bayt okur: `Content-Length` sınırı aşıyorsa hiç
/// indirmez, parça parça okurken aşılırsa durur (hatalı/sahte sunucu belleği tüketmesin).
pub(crate) async fn sinirli_govde_oku(
    mut yanit: reqwest::Response,
    sinir: usize,
) -> Result<Vec<u8>, GovdeHatasi> {
    if yanit
        .content_length()
        .is_some_and(|uzunluk| uzunluk > sinir as u64)
    {
        return Err(GovdeHatasi::CokBuyuk);
    }
    let mut govde = Vec::new();
    while let Some(parca) = yanit.chunk().await.map_err(|_| GovdeHatasi::Ag)? {
        if govde.len() + parca.len() > sinir {
            return Err(GovdeHatasi::CokBuyuk);
        }
        govde.extend_from_slice(&parca);
    }
    Ok(govde)
}

/// Tek bir bağlayıcı okuma turunun sonucu.
#[derive(Debug, Clone)]
pub struct BaglayiciSonucu {
    pub havuz_id: String,
    pub havuz: HavuzAnlik,
    pub retry_after_sn: Option<u64>,
    pub gecici_hata: bool,
}

/// Platform bağımsız kota okuyucu arayüzü.
pub trait Baglayici: Send + Sync {
    fn havuz_id(&self) -> &str;
    fn oku<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = BaglayiciSonucu> + Send + 'a>>;

    fn havuz_kimlikleri(&self) -> Vec<String> {
        vec![self.havuz_id().to_string()]
    }

    fn oku_coklu<'a>(
        &'a self,
        simdi: UtcZaman,
    ) -> Pin<Box<dyn Future<Output = Vec<BaglayiciSonucu>> + Send + 'a>> {
        Box::pin(async move { vec![self.oku(simdi).await] })
    }
}
