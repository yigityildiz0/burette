use schemars::schema_for;
use thiserror::Error;

use crate::anlik::{AnlikV2, Durum};
use crate::uyum::{OrkestraUyumV1, UyumDurum};

/// Şema ve yapısal doğrulama hataları.
#[derive(Debug, Error)]
pub enum DogrulamaHatasi {
    #[error("beklenmeyen v2 şema sürümü: {0}")]
    GecersizSemaV2(u32),
    #[error("beklenmeyen v1 şema sürümü: {0}")]
    GecersizSemaV1(u32),
    #[error("geçersiz oran ({havuz}/{alan}): {deger}")]
    GecersizOran {
        havuz: String,
        alan: String,
        deger: f64,
    },
    #[error("havuz '{havuz}' için etkin_kalan tutarsız")]
    EtkinKalanTutarsiz { havuz: String },
    #[error("havuz '{havuz}' sıfır etkin_kalan ile kullanılabilir işaretlenmiş")]
    SifirKalanKullanilabilir { havuz: String },
}

/// Anlık v2 için JSON Schema üretir.
pub fn anlik_v2_json_schema() -> schemars::Schema {
    schema_for!(AnlikV2)
}

/// Orkestra v1 uyum belgesi için JSON Schema üretir.
pub fn orkestra_uyum_v1_json_schema() -> schemars::Schema {
    schema_for!(OrkestraUyumV1)
}

/// Anlık v2 belgesinin değişmez kurallarını doğrular.
pub fn anlik_dogrula(anlik: &AnlikV2) -> Result<(), DogrulamaHatasi> {
    if anlik.sema != 2 {
        return Err(DogrulamaHatasi::GecersizSemaV2(anlik.sema));
    }

    for (havuz_id, havuz) in &anlik.havuzlar {
        for (pencere_ad, olcum) in &havuz.pencereler {
            if !(0.0..=1.0).contains(&olcum.kalan) || !olcum.kalan.is_finite() {
                return Err(DogrulamaHatasi::GecersizOran {
                    havuz: havuz_id.clone(),
                    alan: format!("{pencere_ad}.kalan"),
                    deger: olcum.kalan,
                });
            }
            if !(0.0..=1.0).contains(&olcum.kullanilan) || !olcum.kullanilan.is_finite() {
                return Err(DogrulamaHatasi::GecersizOran {
                    havuz: havuz_id.clone(),
                    alan: format!("{pencere_ad}.kullanilan"),
                    deger: olcum.kullanilan,
                });
            }
        }

        if let Some(etkin) = havuz.etkin_kalan {
            if !(0.0..=1.0).contains(&etkin) || !etkin.is_finite() {
                return Err(DogrulamaHatasi::GecersizOran {
                    havuz: havuz_id.clone(),
                    alan: "etkin_kalan".to_string(),
                    deger: etkin,
                });
            }
            if let Some(min_pencere) = havuz.pencereler.values().map(|p| p.kalan).reduce(f64::min)
                && (etkin - min_pencere).abs() > 1e-9
            {
                return Err(DogrulamaHatasi::EtkinKalanTutarsiz {
                    havuz: havuz_id.clone(),
                });
            }
            if etkin <= 0.0 && havuz.kullanilabilir == Some(true) {
                return Err(DogrulamaHatasi::SifirKalanKullanilabilir {
                    havuz: havuz_id.clone(),
                });
            }
        } else if !havuz.pencereler.is_empty() && havuz.durum == Durum::Ok {
            return Err(DogrulamaHatasi::EtkinKalanTutarsiz {
                havuz: havuz_id.clone(),
            });
        }
    }

    Ok(())
}

/// Orkestra v1 uyum belgesinin temel değişmez kurallarını doğrular.
pub fn uyum_dogrula(uyum: &OrkestraUyumV1) -> Result<(), DogrulamaHatasi> {
    if uyum.sema != 1 {
        return Err(DogrulamaHatasi::GecersizSemaV1(uyum.sema));
    }

    for (havuz_id, havuz) in &uyum.havuzlar {
        for (pencere_ad, olcum) in &havuz.pencereler {
            if !(0.0..=1.0).contains(&olcum.kalan) || !olcum.kalan.is_finite() {
                return Err(DogrulamaHatasi::GecersizOran {
                    havuz: havuz_id.clone(),
                    alan: format!("{pencere_ad}.kalan"),
                    deger: olcum.kalan,
                });
            }
        }
        if let Some(etkin) = havuz.etkin_kalan
            && etkin <= 0.0
            && havuz.kullanilabilir
        {
            return Err(DogrulamaHatasi::SifirKalanKullanilabilir {
                havuz: havuz_id.clone(),
            });
        }
        if havuz.durum != UyumDurum::Ok && havuz.kullanilabilir {
            return Err(DogrulamaHatasi::SifirKalanKullanilabilir {
                havuz: havuz_id.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semalar_uretilebiliyor() {
        let v2 = serde_json::to_value(anlik_v2_json_schema()).expect("v2 sema");
        assert!(v2.get("$schema").is_some());
        assert!(v2["properties"].get("havuzlar").is_some());

        let v1 = serde_json::to_value(orkestra_uyum_v1_json_schema()).expect("v1 sema");
        assert!(v1["properties"].get("havuzlar").is_some());
    }
}
