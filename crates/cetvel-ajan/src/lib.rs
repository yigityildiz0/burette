//! Claude dışı platformların canlı ajan ağacını okuyan, normalize eden olay okuyucusu.
//!
//! Kaynaklar salt okunur ve kilitlenmeden yoklanır (Codex rollout JSONL, Antigravity özet
//! SQLite'ı). Çıktı [`AjanAnlik`]: yalnız tür, sayı, zaman ve karma
//! kimlik; istem/yanıt metni, komut satırı ve dosya yolu yoktur. Servis bu crate'i yoklama
//! döngüsünde çalıştırıp `GET /v1/ajanlar` ve `ajanlar.json` olarak yayınlar.

pub mod antigravity;
pub mod codex;
pub mod kayit;
pub mod kuyruk;
pub mod model;
pub mod siniflandir;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cetvel_model::UtcZaman;

pub use antigravity::AntigravityOkuyucu;
pub use codex::CodexOkuyucu;
pub use kayit::{AjanKaydi, Kurallar};
pub use model::{
    AJAN_SEMA_SURUMU, Ajan, AjanAnlik, AjanDurumu, AjanSayilari, Etkinlik, HataTuru, KaynakDurumu,
    KaynakOzeti, Platform, kimlik_karma,
};

/// Bir kaynağın tek yoklama sonucu.
#[derive(Debug, Default)]
pub struct OkumaSonucu {
    pub durum: KaynakDurumu,
    pub kayitlar: Vec<AjanKaydi>,
}

impl OkumaSonucu {
    pub fn durum(durum: KaynakDurumu) -> Self {
        Self {
            durum,
            kayitlar: Vec::new(),
        }
    }
}

/// Okuyucu yapılandırması. `None` / boş kaynak kapalıdır.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Yapilandirma {
    /// `<CODEX_HOME>\sessions`.
    pub codex_oturum_koku: Option<PathBuf>,
    /// Antigravity `conversation_summaries.db` yolları.
    pub agy_ozet_db: Vec<PathBuf>,
    pub kurallar: Kurallar,
}

/// Kullanıcı ev klasörü (`USERPROFILE`, yoksa `HOME`); kodda sabit kullanıcı yolu yoktur.
pub fn ev_dizini() -> Option<PathBuf> {
    ["USERPROFILE", "HOME"]
        .iter()
        .filter_map(std::env::var_os)
        .find(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// `CODEX_HOME` ya da `<ev>\.codex`.
pub fn varsayilan_codex_dizini(ev: &Path) -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| ev.join(".codex"))
}

/// Antigravity CLI ve masaüstü uygulamasının özet veritabanı varsayılan yolları.
pub fn varsayilan_agy_ozet_db(ev: &Path) -> Vec<PathBuf> {
    ["antigravity-cli", "antigravity"]
        .iter()
        .map(|d| ev.join(".gemini").join(d).join("conversation_summaries.db"))
        .collect()
}

/// Codex ve Antigravity kaynaklarını birleştiren okuyucu. Durumludur: dosya ofsetleri ve önbellek turlar arasında korunur.
#[derive(Debug)]
pub struct AjanOkuyucu {
    codex: Option<CodexOkuyucu>,
    agy: Option<AntigravityOkuyucu>,
    kurallar: Kurallar,
}

impl AjanOkuyucu {
    pub fn yeni(y: Yapilandirma) -> Self {
        Self {
            codex: y.codex_oturum_koku.map(CodexOkuyucu::yeni),
            agy: (!y.agy_ozet_db.is_empty()).then(|| AntigravityOkuyucu::yeni(y.agy_ozet_db)),
            kurallar: y.kurallar,
        }
    }

    /// Tüm kaynakları bir kez yoklar ve normalize anlık üretir (`sira` 0; yayıncı atar).
    ///
    /// Her kaynak ayrı yalıtılır: birinde beklenmeyen bir panik o kaynağı yalnız bu tur için
    /// `okunamadi` yapar; diğer kaynaklar ve okuyucunun durumu (Codex ofsetleri, önbellekler)
    /// korunur. Paniğe yol açan satır ofset ilerlediği için sonraki turda yeniden okunmaz.
    pub fn tara(&mut self, simdi: UtcZaman) -> AjanAnlik {
        let k = self.kurallar;
        let sonuclar = [
            (
                Platform::Codex,
                self.codex
                    .as_mut()
                    .map(|o| yalitilmis(|| o.tara(simdi, &k))),
            ),
            (
                Platform::Antigravity,
                self.agy.as_mut().map(|o| yalitilmis(|| o.tara(simdi, &k))),
            ),
        ];
        anlik_derle(sonuclar, simdi, &k)
    }
}

/// Bir kaynağın yoklamasını panikten yalıtır; panikte kaynak bu tur `okunamadi` döner.
fn yalitilmis(tara: impl FnOnce() -> OkumaSonucu) -> OkumaSonucu {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(tara))
        .unwrap_or_else(|_| OkumaSonucu::durum(KaynakDurumu::Okunamadi))
}

/// Kaynak sonuçlarını tekilleştirir, kurallara göre süzer ve sıralar.
pub fn anlik_derle(
    sonuclar: impl IntoIterator<Item = (Platform, Option<OkumaSonucu>)>,
    simdi: UtcZaman,
    kurallar: &Kurallar,
) -> AjanAnlik {
    let mut anlik = AjanAnlik::bos(simdi);
    for (platform, sonuc) in sonuclar {
        let Some(sonuc) = sonuc else {
            anlik.kaynaklar.insert(
                platform.kod().to_string(),
                KaynakOzeti {
                    durum: KaynakDurumu::Kapali,
                    ajan: 0,
                },
            );
            continue;
        };
        let mut tekil: BTreeMap<String, AjanKaydi> = BTreeMap::new();
        for k in sonuc.kayitlar {
            match tekil.get(&k.yerel_kimlik) {
                Some(var) if var.son_etkinlik >= k.son_etkinlik => {}
                _ => {
                    tekil.insert(k.yerel_kimlik.clone(), k);
                }
            }
        }
        let gorunen: Vec<Ajan> = tekil
            .values()
            .filter_map(|k| k.yayin(simdi, kurallar))
            .collect();
        anlik.kaynaklar.insert(
            platform.kod().to_string(),
            KaynakOzeti {
                durum: sonuc.durum,
                ajan: gorunen.len(),
            },
        );
        anlik.ajanlar.extend(gorunen);
    }
    anlik.ajanlar.sort_by(|a, b| {
        (a.platform, a.baslama.unwrap_or(a.son_etkinlik), &a.kimlik).cmp(&(
            b.platform,
            b.baslama.unwrap_or(b.son_etkinlik),
            &b.kimlik,
        ))
    });
    anlik
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kaynak_panigi_yalitilir_okunamadi_doner() {
        let sonuc = yalitilmis(|| panic!("deneme paniği"));
        assert_eq!(sonuc.durum, KaynakDurumu::Okunamadi);
        assert!(sonuc.kayitlar.is_empty());
        let sonuc = yalitilmis(|| OkumaSonucu::durum(KaynakDurumu::Izleniyor));
        assert_eq!(sonuc.durum, KaynakDurumu::Izleniyor);
    }
}
