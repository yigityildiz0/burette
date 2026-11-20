use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::anlik::{AnlikV2, Durum, HavuzAnlik, KaynakTuru};
use crate::zaman::UtcZaman;

/// Orkestra v1 uyum belgesi (`orkestra-uyum.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OrkestraUyumV1 {
    /// Şema sürümü (sabit `1`).
    pub sema: u32,
    /// Orkestra sözleşmesindeki sabit yazan adı (`"rust-servis"`).
    pub yazan: String,
    /// Sonu `Z` olan UTC üretim zamanı.
    pub zaman: UtcZaman,
    /// Havuz haritası.
    pub havuzlar: BTreeMap<String, UyumHavuz>,
}

/// Orkestra v1 tekil havuz kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UyumHavuz {
    /// `ok | bilinmiyor | kapali | hata`
    pub durum: UyumDurum,
    /// Pencerelerin en küçüğü olan etkin kalan oranı (`0.0..=1.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etkin_kalan: Option<f64>,
    /// Etkin kalanı belirleyen en kısıtlayıcı pencere adı.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baglayici_pencere: Option<String>,
    /// Havuzun kullanılabilir olup olmadığı (`false` ise orkestra kullanmamalıdır).
    pub kullanilabilir: bool,
    /// Aktif ve taze pencereler.
    pub pencereler: BTreeMap<String, UyumPencere>,
    /// Alt sayaçlar.
    #[serde(default)]
    pub alt_sayaclar: BTreeMap<String, UyumAltSayac>,
    /// Bağlayıcı pencere sıfırlandığında havuzun yeniden açılacağı zaman.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soguma_bitis: Option<UtcZaman>,
    /// İnsan ve karar mekanizması için açıklayıcı notlar.
    #[serde(default)]
    pub notlar: Vec<String>,
}

/// Orkestra v1 durum kümesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UyumDurum {
    Ok,
    Bilinmiyor,
    Kapali,
    Hata,
}

/// Orkestra v1 pencere ölçümü.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UyumPencere {
    pub kalan: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sifirlama: Option<UtcZaman>,
    pub kaynak: String,
}

/// Orkestra v1 alt sayaç özeti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UyumAltSayac {
    pub kalan: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sifirlama: Option<UtcZaman>,
}

/// Antigravity (`agy`) varsayılan okuma aralığı (`ayarlar.rs` `AgyAyari::aralik_sn`).
pub const AGY_VARSAYILAN_ARALIK_SN: u64 = 1800;
/// Antigravity (`agy`) okuma gecikmeleri için tampon süre (sn).
pub const AGY_YAS_TAMPON_SN: u64 = 300;

/// Anlık v2 verisini Orkestra v1 uyum biçimine dönüştürür.
pub fn uyum_uret(anlik: &AnlikV2, simdi: UtcZaman) -> OrkestraUyumV1 {
    uyum_uret_agy_araligi(anlik, simdi, None)
}

/// Anlık v2 verisini bilinen `agy` okuma aralığını gözeterek Orkestra v1 uyum biçimine dönüştürür.
pub fn uyum_uret_agy_araligi(
    anlik: &AnlikV2,
    simdi: UtcZaman,
    agy_aralik_sn: Option<u64>,
) -> OrkestraUyumV1 {
    let mut havuzlar = BTreeMap::new();
    for (havuz_id, havuz) in &anlik.havuzlar {
        // Kullanım sayacı (OpenCode, DeepSeek Harness) kota değildir; orkestra yönlendirmesine girmez.
        if havuz.kullanim_sayaci_mi() {
            continue;
        }
        havuzlar.insert(
            havuz_id.clone(),
            havuzu_uyuma_cevir(havuz_id, havuz, simdi, agy_aralik_sn),
        );
    }
    OrkestraUyumV1 {
        sema: 1,
        yazan: "rust-servis".to_string(),
        zaman: anlik.zaman,
        havuzlar,
    }
}

fn kaynak_etiketi(havuz: &HavuzAnlik) -> &'static str {
    let Some(kaynak) = &havuz.kaynak else {
        return "bilinmiyor";
    };
    match kaynak.tur {
        KaynakTuru::CodexAppServer => "app-server",
        KaynakTuru::RolloutYedek => "rollout_yedek",
        KaynakTuru::AgyUsage => "agy_usage",
        KaynakTuru::ResmiApi => "api_quota",
        KaynakTuru::ModKoprusu => "mod_koprusu",
        KaynakTuru::Statusline => "statusline",
        KaynakTuru::BakiyeApi => "bakiye_api",
        KaynakTuru::Hesap => "hesap",
        KaynakTuru::YerelGunluk => "yerel_gunluk",
        KaynakTuru::Bilinmiyor => "bilinmiyor",
    }
}

fn pencere_yas_esigi_sn(havuz_id: &str, pencere_adi: &str, agy_aralik_sn: Option<u64>) -> u64 {
    if havuz_id == "claude_max" {
        if pencere_adi == "5s" {
            900 // 15 dk
        } else {
            2700 // 45 dk
        }
    } else if havuz_id.starts_with("agy_") {
        let asgari = AGY_VARSAYILAN_ARALIK_SN + AGY_YAS_TAMPON_SN;
        agy_aralik_sn
            .unwrap_or(AGY_VARSAYILAN_ARALIK_SN)
            .saturating_add(AGY_YAS_TAMPON_SN)
            .max(asgari)
    } else {
        600 // 10 dk
    }
}

fn havuzu_uyuma_cevir(
    havuz_id: &str,
    havuz: &HavuzAnlik,
    simdi: UtcZaman,
    agy_aralik_sn: Option<u64>,
) -> UyumHavuz {
    let mut notlar = havuz.notlar.clone();
    let mut alt_sayaclar = BTreeMap::new();
    for (ad, alt) in &havuz.alt_sayaclar {
        let kalan = alt
            .kalan
            .unwrap_or_else(|| (1.0 - alt.kullanilan).clamp(0.0, 1.0));
        alt_sayaclar.insert(
            ad.clone(),
            UyumAltSayac {
                kalan,
                sifirlama: alt.sifirlama,
            },
        );
    }

    match havuz.durum {
        Durum::Kapali => UyumHavuz {
            durum: UyumDurum::Kapali,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: false,
            pencereler: BTreeMap::new(),
            alt_sayaclar: BTreeMap::new(),
            soguma_bitis: None,
            notlar,
        },
        Durum::Baglanmadi => {
            if notlar.is_empty() {
                notlar.push("bağlanmadı".to_string());
            }
            UyumHavuz {
                durum: UyumDurum::Bilinmiyor,
                etkin_kalan: None,
                baglayici_pencere: None,
                kullanilabilir: false,
                pencereler: BTreeMap::new(),
                alt_sayaclar: BTreeMap::new(),
                soguma_bitis: None,
                notlar,
            }
        }
        Durum::Hata => {
            if let Some(hata) = &havuz.hata {
                let not = match hata.http_kodu {
                    Some(kod) => format!("hata: {} (HTTP {})", hata.tur, kod),
                    None => format!("hata: {}", hata.tur),
                };
                if !notlar.contains(&not) {
                    notlar.push(not);
                }
            }
            UyumHavuz {
                durum: UyumDurum::Hata,
                etkin_kalan: None,
                baglayici_pencere: None,
                kullanilabilir: false,
                pencereler: BTreeMap::new(),
                alt_sayaclar: BTreeMap::new(),
                soguma_bitis: None,
                notlar,
            }
        }
        Durum::Bilinmiyor => UyumHavuz {
            durum: UyumDurum::Bilinmiyor,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: false,
            pencereler: BTreeMap::new(),
            alt_sayaclar: BTreeMap::new(),
            soguma_bitis: None,
            notlar,
        },
        Durum::Ok | Durum::Bayat => {
            let yas_sn = havuz
                .kaynak
                .as_ref()
                .map(|k| simdi.fark_sn(k.okuma).max(0) as u64)
                .unwrap_or(0);
            let kaynak = kaynak_etiketi(havuz).to_string();

            let mut pencereler = BTreeMap::new();
            let mut atilan_var = false;
            for (ad, olcum) in &havuz.pencereler {
                let esik = pencere_yas_esigi_sn(havuz_id, ad, agy_aralik_sn);
                if havuz.durum == Durum::Bayat || yas_sn > esik {
                    atilan_var = true;
                    continue;
                }
                pencereler.insert(
                    ad.clone(),
                    UyumPencere {
                        kalan: olcum.kalan.clamp(0.0, 1.0),
                        sifirlama: olcum.sifirlama,
                        kaynak: kaynak.clone(),
                    },
                );
            }

            if atilan_var || (havuz.durum == Durum::Bayat && pencereler.is_empty()) {
                let dk = (yas_sn / 60).max(1);
                let bayat_notu = format!("son okuma {dk} dk önce");
                if !notlar.contains(&bayat_notu) {
                    notlar.push(bayat_notu);
                }
            }

            // Sağ kalan taze pencereler üzerinden etkin kalan & bağlayıcı pencereyi yeniden hesapla.
            let mut en_kucuk: Option<(String, &UyumPencere, u64)> = None;
            for (ad, p) in &pencereler {
                let uzunluk_dk = havuz.pencereler.get(ad).map(|v| v.uzunluk_dk).unwrap_or(0);
                match &en_kucuk {
                    None => en_kucuk = Some((ad.clone(), p, uzunluk_dk)),
                    Some((mevcut_ad, mevcut, mevcut_uzunluk)) => {
                        let fark = p.kalan - mevcut.kalan;
                        if fark < -1e-9
                            || (fark.abs() <= 1e-9
                                && (uzunluk_dk > *mevcut_uzunluk
                                    || (uzunluk_dk == *mevcut_uzunluk && ad < mevcut_ad)))
                        {
                            en_kucuk = Some((ad.clone(), p, uzunluk_dk));
                        }
                    }
                }
            }

            if let Some((baglayici, olcum, _)) = en_kucuk {
                let etkin = olcum.kalan.clamp(0.0, 1.0);
                let kullanilabilir = etkin > 0.0;
                let soguma_bitis = if !kullanilabilir {
                    olcum.sifirlama
                } else {
                    None
                };
                if !kullanilabilir {
                    let uyari = format!("havuz kullanılamaz: {baglayici} penceresi %0 kalan");
                    if !notlar.contains(&uyari) {
                        notlar.push(uyari);
                    }
                }
                UyumHavuz {
                    durum: UyumDurum::Ok,
                    etkin_kalan: Some(etkin),
                    baglayici_pencere: Some(baglayici),
                    kullanilabilir,
                    pencereler,
                    alt_sayaclar,
                    soguma_bitis,
                    notlar,
                }
            } else {
                // Bakiye tabanlı ya da muaf havuzlarda pencere olmayabilir; yalnız gerçekten okunan boş pencere mi yoksa bayat mı ayır.
                let bayat_veya_bos = havuz.durum == Durum::Bayat || !havuz.pencereler.is_empty();
                if bayat_veya_bos {
                    UyumHavuz {
                        durum: UyumDurum::Bilinmiyor,
                        etkin_kalan: None,
                        baglayici_pencere: None,
                        kullanilabilir: false,
                        pencereler,
                        alt_sayaclar: BTreeMap::new(),
                        soguma_bitis: None,
                        notlar,
                    }
                } else {
                    let muaf = havuz.ek.as_ref().and_then(|e| e.muaf) == Some(true);
                    if let Some(bakiye) = &havuz.bakiye {
                        let bakiye_notu =
                            format!("bakiye: {:.2} {}", bakiye.toplam, bakiye.para_birimi);
                        if !notlar.contains(&bakiye_notu) {
                            notlar.push(bakiye_notu);
                        }
                    }
                    let kullanilabilir =
                        muaf || havuz.bakiye.as_ref().is_some_and(|b| b.toplam > 0.0);
                    UyumHavuz {
                        durum: UyumDurum::Ok,
                        etkin_kalan: None,
                        baglayici_pencere: None,
                        kullanilabilir,
                        pencereler,
                        alt_sayaclar,
                        soguma_bitis: None,
                        notlar,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anlik::{GuvenEtiketi, KaynakBilgisi, PencereOlcum};

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
    }

    #[test]
    fn haftalik_sifir_bes_saatlik_yuz_orkestra_uyumunda_kullanilamaz_yansir() {
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 0.0,
                kalan: 1.0,
                sifirlama: None,
            },
        );
        let hafta_sifirlama = z("2026-11-20T07:00:00Z");
        pencereler.insert(
            "hafta".to_string(),
            PencereOlcum {
                uzunluk_dk: 10080,
                kullanilan: 1.0,
                kalan: 0.0,
                sifirlama: Some(hafta_sifirlama),
            },
        );

        let okuma = z("2026-11-15T18:25:00Z");
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
                okuma,
                yas_sn: 0,
                hassasiyet: None,
            }),
            pencereler,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        havuz.hesapla_etkin_durum();

        let mut havuzlar = BTreeMap::new();
        havuzlar.insert("codex_plus".to_string(), havuz);
        let anlik = AnlikV2::yeni(okuma, 1, havuzlar);

        let simdi = z("2026-11-15T18:26:00Z");
        let uyum = uyum_uret(&anlik, simdi);
        assert_eq!(uyum.sema, 1);
        assert_eq!(uyum.yazan, "rust-servis");

        let codex = uyum.havuzlar.get("codex_plus").expect("codex_plus");
        assert_eq!(codex.durum, UyumDurum::Ok);
        assert_eq!(codex.etkin_kalan, Some(0.0));
        assert_eq!(codex.baglayici_pencere.as_deref(), Some("hafta"));
        assert!(!codex.kullanilabilir);
        assert_eq!(codex.soguma_bitis, Some(hafta_sifirlama));
        assert_eq!(codex.pencereler["5s"].kaynak, "app-server");
        assert_eq!(codex.pencereler["hafta"].kalan, 0.0);
    }

    #[test]
    fn bayat_codex_penceresi_uyumda_bilinmiyor_olur() {
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 0.1,
                kalan: 0.9,
                sifirlama: Some(z("2026-11-15T22:00:00Z")),
            },
        );
        let okuma = z("2026-11-15T18:00:00Z");
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
                okuma,
                yas_sn: 900,
                hassasiyet: None,
            }),
            pencereler,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
        };
        havuz.hesapla_etkin_durum();

        let mut havuzlar = BTreeMap::new();
        havuzlar.insert("codex_plus".to_string(), havuz);
        let anlik = AnlikV2::yeni(z("2026-11-15T18:15:00Z"), 1, havuzlar);

        let uyum = uyum_uret(&anlik, z("2026-11-15T18:15:00Z"));
        let codex = &uyum.havuzlar["codex_plus"];
        assert_eq!(codex.durum, UyumDurum::Bilinmiyor);
        assert!(!codex.kullanilabilir);
        assert!(codex.pencereler.is_empty());
        assert!(
            codex
                .notlar
                .iter()
                .any(|n| n.contains("son okuma 15 dk önce"))
        );
    }

    #[test]
    fn kullanim_sayaci_kullanilabilir_ama_orkestra_uyumuna_girmez() {
        // Arayüzün örnek anlığı gerçek şemayla okunur; OpenCode havuzu kota değil, sayaçtır.
        let ham = include_str!("../../../apps/arayuz/ornek-anlik.json");
        let anlik: AnlikV2 = serde_json::from_str(ham).expect("ornek-anlik.json şemaya uymalı");
        let mut oc = anlik.havuzlar["opencode"].clone();
        let sayac = oc
            .ek
            .as_ref()
            .and_then(|e| e.kullanim.clone())
            .expect("sayaç");
        assert!(sayac.kota_yok);
        assert_eq!(sayac.oturum_24s, 4);
        assert_eq!(oc.kaynak.as_ref().unwrap().tur, KaynakTuru::YerelGunluk);
        oc.kullanilabilir = None;
        oc.hesapla_etkin_durum();
        assert_eq!(oc.kullanilabilir, Some(true));
        assert!(oc.etkin_kalan.is_none());

        let uyum = uyum_uret(&anlik, anlik.zaman);
        assert!(
            !uyum.havuzlar.contains_key("opencode"),
            "sayaç orkestraya kota diye gitmez"
        );
        assert!(uyum.havuzlar.contains_key("claude_max"));
    }

    fn ornek_agy_anlik(okuma: UtcZaman, yas_sn: u64) -> AnlikV2 {
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 0.1,
                kalan: 0.9,
                sifirlama: Some(okuma.ekle_sn(18_000)),
            },
        );
        pencereler.insert(
            "hafta".to_string(),
            PencereOlcum {
                uzunluk_dk: 10080,
                kullanilan: 0.2,
                kalan: 0.8,
                sifirlama: Some(okuma.ekle_sn(86_400)),
            },
        );
        let mut havuz = HavuzAnlik {
            platform: "agy".to_string(),
            etiket: "Antigravity Gemini".to_string(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::AgyUsage,
                guven: GuvenEtiketi::Tahmini,
                okuma,
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
            notlar: Vec::new(),
        };
        havuz.hesapla_etkin_durum();
        let mut havuzlar = BTreeMap::new();
        havuzlar.insert("agy_gemini".to_string(), havuz);
        AnlikV2::yeni(okuma.ekle_sn(yas_sn as i64), 1, havuzlar)
    }

    #[test]
    fn agy_havuzu_varsayilan_aralikta_on_bes_ve_otuz_bes_dakika_icinde_taze_kalir() {
        let okuma = z("2026-11-16T10:00:00Z");
        // 15 dk (900 sn): eski 600 sn eşiğinde düşüyordu, şimdi 1800 + 300 sn eşiğinde taze.
        let anlik_15 = ornek_agy_anlik(okuma, 900);
        let uyum_15 = uyum_uret(&anlik_15, okuma.ekle_sn(900));
        let agy_15 = &uyum_15.havuzlar["agy_gemini"];
        assert_eq!(agy_15.durum, UyumDurum::Ok);
        assert!(agy_15.kullanilabilir);
        assert_eq!(agy_15.pencereler.len(), 2);

        // Tam sınır: 1800 + 300 = 2100 sn taze, 2101 sn düşer.
        let anlik_sinir = ornek_agy_anlik(okuma, 2100);
        let uyum_sinir = uyum_uret(&anlik_sinir, okuma.ekle_sn(2100));
        assert_eq!(uyum_sinir.havuzlar["agy_gemini"].durum, UyumDurum::Ok);

        let anlik_eski = ornek_agy_anlik(okuma, 2101);
        let uyum_eski = uyum_uret(&anlik_eski, okuma.ekle_sn(2101));
        let agy_eski = &uyum_eski.havuzlar["agy_gemini"];
        assert_eq!(agy_eski.durum, UyumDurum::Bilinmiyor);
        assert!(!agy_eski.kullanilabilir);
        assert!(agy_eski.pencereler.is_empty());
    }

    #[test]
    fn agy_pencere_yas_esigi_ayardan_gelen_araliktan_turetilir() {
        let okuma = z("2026-11-16T10:00:00Z");
        // Özel aralık 3600 sn -> eşik 3600 + 300 = 3900 sn.
        let anlik_55dk = ornek_agy_anlik(okuma, 3300);
        let uyum_55dk = uyum_uret_agy_araligi(&anlik_55dk, okuma.ekle_sn(3300), Some(3600));
        assert_eq!(uyum_55dk.havuzlar["agy_gemini"].durum, UyumDurum::Ok);
        assert!(uyum_55dk.havuzlar["agy_gemini"].kullanilabilir);

        let anlik_66dk = ornek_agy_anlik(okuma, 3960);
        let uyum_66dk = uyum_uret_agy_araligi(&anlik_66dk, okuma.ekle_sn(3960), Some(3600));
        assert_eq!(
            uyum_66dk.havuzlar["agy_gemini"].durum,
            UyumDurum::Bilinmiyor
        );

        // Kısa ayar verilse bile eşik en az 1800 + 300 = 2100 sn kalır.
        let anlik_20dk = ornek_agy_anlik(okuma, 1200);
        let uyum_20dk = uyum_uret_agy_araligi(&anlik_20dk, okuma.ekle_sn(1200), Some(600));
        assert_eq!(uyum_20dk.havuzlar["agy_gemini"].durum, UyumDurum::Ok);
    }
}
