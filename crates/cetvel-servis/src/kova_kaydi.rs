//! Dinamik kova kaydı (`%LOCALAPPDATA%\Cetvel\kovalar.json`): havuz ve alt sayaç başına ilk/son
//! görülme. Sağlayıcı yeni havuz ya da alt sayaç çıkarınca uygulama güncellenmeden tanır; yanıttan
//! kaybolan havuz 7 gün `gorulmuyor` (son değerleriyle, `bayat`) gösterilir, sonra düşer.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cetvel_model::{Durum, HavuzAnlik, KesifBilgisi, KovaYasam, UtcZaman};
use serde::{Deserialize, Serialize};

/// Görülmeyen havuzun anlıkta tutulduğu süre (sn).
pub const GORULMUYOR_SAKLAMA_SN: i64 = 7 * 86_400;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KovaKaydi {
    #[serde(default = "sema_bir")]
    pub sema: u32,
    #[serde(default)]
    pub havuzlar: BTreeMap<String, KayitliHavuz>,
}

impl Default for KovaKaydi {
    fn default() -> Self {
        Self {
            sema: 1,
            havuzlar: BTreeMap::new(),
        }
    }
}

fn sema_bir() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KayitliHavuz {
    pub platform: String,
    pub ilk_gorulme: UtcZaman,
    pub son_gorulme: UtcZaman,
    #[serde(default)]
    pub dinamik: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grup: Option<String>,
    #[serde(default)]
    pub alt_sayaclar: BTreeMap<String, KayitliAlt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KayitliAlt {
    pub ilk_gorulme: UtcZaman,
    pub son_gorulme: UtcZaman,
}

impl KovaKaydi {
    /// Dosyadan yükler; yoksa ya da bozuksa boş kayıt (kayıt yalnız ilk görülme tarihlerini taşır,
    /// kaybı veri kaybı değildir).
    pub fn yukle(yol: &Path) -> Self {
        std::fs::read_to_string(yol)
            .ok()
            .and_then(|m| serde_json::from_str::<KovaKaydi>(&m).ok())
            .map(|mut k| {
                k.sema = 1;
                k
            })
            .unwrap_or_else(|| KovaKaydi {
                sema: 1,
                havuzlar: BTreeMap::new(),
            })
    }

    /// Platformun kayıtlı havuz kimlikleri.
    pub fn kimlikler(&self, platform: &str) -> Vec<String> {
        self.havuzlar
            .iter()
            .filter(|(_, h)| h.platform == platform)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Başarıyla okunmuş (`Ok`) havuzu gözlemler: ilk/son görülmeyi günceller. Değiştiyse `true`.
    pub fn gozlemle(&mut self, id: &str, havuz: &HavuzAnlik) -> bool {
        if havuz.durum != Durum::Ok || havuz.kullanim_sayaci_mi() {
            return false;
        }
        let Some(okuma) = havuz.kaynak.as_ref().map(|k| k.okuma) else {
            return false;
        };
        let mut degisti = false;
        let kayit = self.havuzlar.entry(id.to_string()).or_insert_with(|| {
            degisti = true;
            KayitliHavuz {
                platform: havuz.platform.clone(),
                ilk_gorulme: okuma,
                son_gorulme: okuma,
                dinamik: false,
                grup: None,
                alt_sayaclar: BTreeMap::new(),
            }
        });
        if okuma > kayit.son_gorulme {
            kayit.son_gorulme = okuma;
            degisti = true;
        }
        if let Some(k) = &havuz.kesif {
            if k.dinamik && !kayit.dinamik {
                kayit.dinamik = true;
                degisti = true;
            }
            if k.grup.is_some() && kayit.grup != k.grup {
                kayit.grup = k.grup.clone();
                degisti = true;
            }
        }
        for ad in havuz.alt_sayaclar.keys() {
            match kayit.alt_sayaclar.get_mut(ad) {
                Some(a) => {
                    if okuma > a.son_gorulme {
                        a.son_gorulme = okuma;
                        degisti = true;
                    }
                }
                None => {
                    kayit.alt_sayaclar.insert(
                        ad.clone(),
                        KayitliAlt {
                            ilk_gorulme: okuma,
                            son_gorulme: okuma,
                        },
                    );
                    degisti = true;
                }
            }
        }
        degisti
    }

    /// Kayıttaki ilk/son görülmeyi havuza ve alt sayaçlarına yazar (yaşam evresine dokunmaz).
    pub fn havuza_isle(&self, id: &str, havuz: &mut HavuzAnlik) {
        let Some(kayit) = self.havuzlar.get(id) else {
            return;
        };
        match havuz.kesif.as_mut() {
            Some(k) => {
                k.ilk_gorulme = kayit.ilk_gorulme;
                k.son_gorulme = k.son_gorulme.max(kayit.son_gorulme);
                k.dinamik |= kayit.dinamik;
                if k.grup.is_none() {
                    k.grup = kayit.grup.clone();
                }
            }
            None => {
                havuz.kesif = Some(KesifBilgisi {
                    dinamik: kayit.dinamik,
                    grup: kayit.grup.clone(),
                    ilk_gorulme: kayit.ilk_gorulme,
                    son_gorulme: kayit.son_gorulme,
                    yasam: KovaYasam::Aktif,
                    ebeveyn_havuz: None,
                    ebeveyn_guven: None,
                });
            }
        }
        for (ad, alt) in &mut havuz.alt_sayaclar {
            if let Some(k) = kayit.alt_sayaclar.get(ad) {
                alt.ilk_gorulme = Some(k.ilk_gorulme);
            }
        }
    }

    /// Platform başarıyla okunduğu hâlde yanıtta olmayan kayıtlı havuzları `gorulmuyor` (ve
    /// `bayat`) işaretler; 7 günden uzun görülmeyenleri kayıttan ve `havuzlar`dan düşürür.
    /// Düşen kimlikleri döndürür. Platform hiç okunamadıysa çağrılmamalıdır (mevcut bayat mantığı).
    pub fn yasam_uygula(
        &mut self,
        platform: &str,
        gorulen: &BTreeSet<String>,
        havuzlar: &mut BTreeMap<String, HavuzAnlik>,
        simdi: UtcZaman,
    ) -> Vec<String> {
        let mut dusen = Vec::new();
        for (id, kayit) in &self.havuzlar {
            if kayit.platform != platform || gorulen.contains(id) {
                continue;
            }
            if simdi.fark_sn(kayit.son_gorulme) > GORULMUYOR_SAKLAMA_SN {
                dusen.push(id.clone());
                continue;
            }
            if let Some(h) = havuzlar.get_mut(id) {
                gorulmuyor_isaretle(h, kayit);
            }
        }
        for id in &dusen {
            self.havuzlar.remove(id);
            havuzlar.remove(id);
        }
        dusen
    }
}

fn gorulmuyor_isaretle(havuz: &mut HavuzAnlik, kayit: &KayitliHavuz) {
    if !matches!(havuz.durum, Durum::Ok | Durum::Bayat | Durum::Hata) {
        return;
    }
    let kesif = havuz.kesif.get_or_insert_with(|| KesifBilgisi {
        dinamik: kayit.dinamik,
        grup: kayit.grup.clone(),
        ilk_gorulme: kayit.ilk_gorulme,
        son_gorulme: kayit.son_gorulme,
        yasam: KovaYasam::Gorulmuyor,
        ebeveyn_havuz: None,
        ebeveyn_guven: None,
    });
    kesif.yasam = KovaYasam::Gorulmuyor;
    if havuz.durum == Durum::Ok {
        havuz.durum = Durum::Bayat;
    }
    let not = "sağlayıcı yanıtında artık yok; son değerler gösteriliyor".to_string();
    if !havuz.notlar.contains(&not) {
        havuz.notlar.push(not);
    }
    havuz.hesapla_etkin_durum();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cetvel_model::{GuvenEtiketi, KaynakBilgisi, KaynakTuru, PencereOlcum};

    fn havuz(okuma: UtcZaman) -> HavuzAnlik {
        let mut h = HavuzAnlik {
            platform: "agy".into(),
            etiket: "Antigravity Gemini 5 Xenon".into(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::AgyUsage,
                guven: GuvenEtiketi::Tahmini,
                okuma,
                yas_sn: 0,
                hassasiyet: None,
            }),
            pencereler: BTreeMap::from([(
                "hafta".to_string(),
                PencereOlcum {
                    uzunluk_dk: 10080,
                    kullanilan: 0.2,
                    kalan: 0.8,
                    sifirlama: None,
                    etiket: None,
                },
            )]),
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
            kesif: None,
            etkin_kalan_zincir: None,
        };
        h.hesapla_etkin_durum();
        h
    }

    #[test]
    fn ilk_gorulme_korunur_gorulmeyen_yedi_gun_sonra_duser() {
        let t0 = UtcZaman::parse_tarih_veya_zaman("2026-11-20T10:00:00Z").unwrap();
        let mut kayit = KovaKaydi::default();
        assert!(kayit.gozlemle("agy_gemini_5_xenon", &havuz(t0)));
        // Sonraki okuma: ilk görülme değişmez, son görülme ilerler.
        let t1 = t0.ekle_sn(3600);
        let mut h1 = havuz(t1);
        assert!(kayit.gozlemle("agy_gemini_5_xenon", &h1));
        kayit.havuza_isle("agy_gemini_5_xenon", &mut h1);
        let k = h1.kesif.as_ref().unwrap();
        assert_eq!(k.ilk_gorulme, t0);
        assert_eq!(k.son_gorulme, t1);

        // Platform başarıyla okundu, kova yanıtta yok: gorulmuyor + bayat, değerler korunur.
        let mut havuzlar = BTreeMap::from([("agy_gemini_5_xenon".to_string(), h1)]);
        let bos = BTreeSet::new();
        let t2 = t1.ekle_sn(86_400);
        assert!(
            kayit
                .yasam_uygula("agy", &bos, &mut havuzlar, t2)
                .is_empty()
        );
        let h = &havuzlar["agy_gemini_5_xenon"];
        assert_eq!(h.durum, Durum::Bayat);
        assert_eq!(h.kesif.as_ref().unwrap().yasam, KovaYasam::Gorulmuyor);
        assert!((h.pencereler["hafta"].kalan - 0.8).abs() < 1e-9);

        // 7 günden sonra düşer.
        let t3 = t1.ekle_sn(GORULMUYOR_SAKLAMA_SN + 60);
        assert_eq!(
            kayit.yasam_uygula("agy", &bos, &mut havuzlar, t3),
            vec!["agy_gemini_5_xenon".to_string()]
        );
        assert!(havuzlar.is_empty());
        assert!(kayit.havuzlar.is_empty());
    }

    #[test]
    fn alt_sayac_ilk_gorulmesi_yazilir_ve_dosya_gidis_donus() {
        let t0 = UtcZaman::parse_tarih_veya_zaman("2026-11-20T10:00:00Z").unwrap();
        let mut h = havuz(t0);
        h.alt_sayaclar.insert(
            "fable".into(),
            cetvel_model::AltSayac {
                kullanilan: 0.5,
                kalan: Some(0.5),
                uzunluk_dk: Some(10080),
                sifirlama: None,
                etiket: Some("Weekly · Fable".into()),
                ebeveyn: Some("hafta".into()),
                ilk_gorulme: None,
            },
        );
        let mut kayit = KovaKaydi::default();
        kayit.gozlemle("claude_max", &h);
        let mut sonra = h.clone();
        sonra.kaynak.as_mut().unwrap().okuma = t0.ekle_sn(600);
        kayit.gozlemle("claude_max", &sonra);
        kayit.havuza_isle("claude_max", &mut sonra);
        assert_eq!(sonra.alt_sayaclar["fable"].ilk_gorulme, Some(t0));

        let dizin = std::env::temp_dir().join(format!("cetvel-kova-{}", std::process::id()));
        std::fs::create_dir_all(&dizin).unwrap();
        let yol = dizin.join("kovalar.json");
        cetvel_depo::atomik_yaz_json(&yol, &kayit).unwrap();
        assert_eq!(KovaKaydi::yukle(&yol), kayit);
        std::fs::write(&yol, "bozuk").unwrap();
        assert!(KovaKaydi::yukle(&yol).havuzlar.is_empty());
        let _ = std::fs::remove_dir_all(&dizin);
    }
}
