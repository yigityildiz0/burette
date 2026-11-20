use std::collections::{BTreeMap, BTreeSet};

use cetvel_baglayici::BaglayiciSonucu;
use cetvel_model::{AnlikV2, Durum, HavuzAnlik, UtcZaman};

use crate::claude_kopru::suresi_dolan_pencereleri_hesapla;

const GERI_CEKILME_ADIMLARI_SN: [u64; 4] = [120, 300, 900, 900];
const DEVRE_KESICI_SN: u64 = 1800; // 30 dk
const DEVRE_ESIGI: u32 = 5;
/// `baglanmadi` (kurulu değil / anahtar yok) havuzlar bu aralıkla yeniden denenir; böylece
/// sonradan kurulan Codex veya eklenen anahtar servis yeniden başlatılmadan algılanır.
pub const BAGLANMADI_YENIDEN_DENEME_SN: i64 = 600;
/// `Retry-After` ve bağlayıcı bekleme isteklerinin üst sınırı (24 sa): dev değer `i64`'e
/// çevrilirken taşıp geçmişe sarmasın ya da havuzu süresiz kapatmasın.
pub const RETRY_AFTER_TAVAN_SN: u64 = 24 * 3600;
/// Ek köprü havuzlarının güncellenmeden anlıkta kalabileceği süre (sn).
pub const KOPRU_HAVUZ_OMRU_SN: i64 = 30 * 86_400;
/// Yenileme sınırlayıcısının tuttuğu en fazla anahtar sayısı.
const YENILE_ANAHTAR_TAVANI: usize = 128;
/// Elle/zorla yenilemede havuz başına en kısa okuma aralığı (havuz başına 1/dk).
pub const ZORLA_OKUMA_ASGARI_SN: i64 = 60;

/// Tek bir havuz için zamanlama ve bayatlık kuralları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HavuzZamanlamaKurali {
    pub etkin_aralik_sn: u64,
    pub bosta_aralik_sn: u64,
    pub bayat_esigi_sn: u64,
}

impl Default for HavuzZamanlamaKurali {
    fn default() -> Self {
        Self {
            etkin_aralik_sn: 300,
            bosta_aralik_sn: 1800,
            bayat_esigi_sn: bayat_esigi_hesapla(600, 1800),
        }
    }
}

/// Bayat eşiği her zaman en uzun planlı yoklama aralığından büyük olmalı; yoksa havuz kendi
/// planlı aralığı içinde `bayat` olur (ör. Codex boşta 30 dk aralık, eşik 10 dk).
pub fn bayat_esigi_hesapla(taban_sn: u64, en_uzun_aralik_sn: u64) -> u64 {
    taban_sn.max(en_uzun_aralik_sn.saturating_mul(2))
}

/// Havuz durum makinesi evreleri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TakipEvresi {
    Baslangic,
    Aktif {
        son_okuma: UtcZaman,
    },
    GeriCekilme {
        ardisik_hata: u32,
        sonraki_deneme: UtcZaman,
    },
    DevreAcik {
        acilis: UtcZaman,
        sonraki_deneme: UtcZaman,
        ardisik_hata: u32,
    },
    YariAcik {
        ardisik_hata: u32,
    },
    KaliciKapali,
    Baglanmadi {
        son_deneme: UtcZaman,
    },
}

/// Havuz bazında uyarlamalı okuma, geri çekilme ve devre kesici durum makinesi.
#[derive(Debug, Clone)]
pub struct HavuzDurumMakinesi {
    pub kural: HavuzZamanlamaKurali,
    pub evre: TakipEvresi,
}

impl HavuzDurumMakinesi {
    pub fn yeni(kural: HavuzZamanlamaKurali) -> Self {
        Self {
            kural,
            evre: TakipEvresi::Baslangic,
        }
    }

    /// Verilen anda bu havuzun okunması gerekip gerekmediğini belirler.
    pub fn okunmali_mi(
        &mut self,
        simdi: UtcZaman,
        kullanici_etkin: bool,
        uyanma_oldu: bool,
    ) -> bool {
        match self.evre {
            TakipEvresi::Baslangic => true,
            TakipEvresi::KaliciKapali => false,
            TakipEvresi::Baglanmadi { son_deneme } => {
                simdi.fark_sn(son_deneme) >= BAGLANMADI_YENIDEN_DENEME_SN
            }
            TakipEvresi::Aktif { son_okuma } => {
                if uyanma_oldu {
                    return true;
                }
                let aralik = if kullanici_etkin {
                    self.kural.etkin_aralik_sn
                } else {
                    self.kural.bosta_aralik_sn
                };
                simdi.fark_sn(son_okuma) >= aralik as i64
            }
            TakipEvresi::GeriCekilme { sonraki_deneme, .. } => simdi >= sonraki_deneme,
            TakipEvresi::DevreAcik {
                sonraki_deneme,
                ardisik_hata,
                ..
            } => {
                if simdi >= sonraki_deneme {
                    self.evre = TakipEvresi::YariAcik { ardisik_hata };
                    true
                } else {
                    false
                }
            }
            TakipEvresi::YariAcik { .. } => true,
        }
    }

    /// Elle yenileme / "Test et" isteğinde hemen okunabilir mi? Geri çekilme ve devre kesici
    /// korunur; başarılı son okumadan 60 sn geçmeden yeniden okunmaz.
    pub fn zorla_okunabilir_mi(&self, simdi: UtcZaman) -> bool {
        match self.evre {
            TakipEvresi::Baslangic
            | TakipEvresi::Baglanmadi { .. }
            | TakipEvresi::YariAcik { .. } => true,
            TakipEvresi::Aktif { son_okuma } => simdi.fark_sn(son_okuma) >= ZORLA_OKUMA_ASGARI_SN,
            TakipEvresi::GeriCekilme { .. }
            | TakipEvresi::DevreAcik { .. }
            | TakipEvresi::KaliciKapali => false,
        }
    }

    /// Son başarılı okuma anı (yalnız `Aktif` evrede).
    pub fn son_okuma(&self) -> Option<UtcZaman> {
        match self.evre {
            TakipEvresi::Aktif { son_okuma } => Some(son_okuma),
            _ => None,
        }
    }

    /// Yoklanan havuzda bir pencerenin sıfırlama anı son okumadan sonra geçtiyse `true`
    /// (ör. Codex hafta %100 → sıfırlandı; boşta 30 dk beklemeden hemen okunur).
    pub fn sifirlama_gecti_mi(&self, havuz: Option<&HavuzAnlik>, simdi: UtcZaman) -> bool {
        let Some(son_okuma) = self.son_okuma() else {
            return false;
        };
        if simdi.fark_sn(son_okuma) < ZORLA_OKUMA_ASGARI_SN {
            return false;
        }
        havuz.is_some_and(|h| {
            h.pencereler.values().any(|p| {
                p.sifirlama
                    .is_some_and(|s| s.fark_sn(son_okuma) > 0 && simdi.fark_sn(s) >= 0)
            })
        })
    }

    /// Okuma sonucuna göre durum makinesini ilerletir.
    pub fn sonuc_isle(&mut self, simdi: UtcZaman, sonuc: &BaglayiciSonucu) {
        if sonuc.havuz.durum == Durum::Kapali {
            self.evre = TakipEvresi::KaliciKapali;
            return;
        }
        if sonuc.havuz.durum == Durum::Baglanmadi {
            self.evre = TakipEvresi::Baglanmadi { son_deneme: simdi };
            return;
        }

        if !sonuc.gecici_hata && sonuc.havuz.durum == Durum::Ok {
            self.evre = TakipEvresi::Aktif { son_okuma: simdi };
            return;
        }

        let onceki_hatalar = match self.evre {
            TakipEvresi::GeriCekilme { ardisik_hata, .. }
            | TakipEvresi::DevreAcik { ardisik_hata, .. }
            | TakipEvresi::YariAcik { ardisik_hata } => ardisik_hata,
            _ => 0,
        };
        let yeni_hata_sayisi = onceki_hatalar.saturating_add(1);

        if matches!(self.evre, TakipEvresi::YariAcik { .. }) || yeni_hata_sayisi >= DEVRE_ESIGI {
            let bekleme = sonuc
                .retry_after_sn
                .unwrap_or(DEVRE_KESICI_SN)
                .clamp(DEVRE_KESICI_SN, RETRY_AFTER_TAVAN_SN);
            self.evre = TakipEvresi::DevreAcik {
                acilis: simdi,
                sonraki_deneme: simdi.ekle_sn(bekleme as i64),
                ardisik_hata: yeni_hata_sayisi,
            };
        } else {
            let idx = (yeni_hata_sayisi as usize)
                .saturating_sub(1)
                .min(GERI_CEKILME_ADIMLARI_SN.len() - 1);
            let geri_sn = GERI_CEKILME_ADIMLARI_SN[idx];
            let bekleme = sonuc
                .retry_after_sn
                .unwrap_or(0)
                .min(RETRY_AFTER_TAVAN_SN)
                .max(geri_sn);
            self.evre = TakipEvresi::GeriCekilme {
                ardisik_hata: yeni_hata_sayisi,
                sonraki_deneme: simdi.ekle_sn(bekleme as i64),
            };
        }
    }
}

/// Duvar saati sıçramasıyla uyku/uyanma algılar (> 2 aralık).
pub fn uyanma_algila(onceki_tik: UtcZaman, simdi: UtcZaman, tik_araligi_sn: u64) -> bool {
    let esik = (tik_araligi_sn.saturating_mul(2)).max(120) as i64;
    simdi.fark_sn(onceki_tik) > esik
}

/// Yoklanan bağlayıcıların son sonuçlarını biriktirir ve yaşlarını günceller.
///
/// `sira` sayacı burada tutulmaz: tek yetkili sayaç paylaşılan durumdaki yayımlanmış anlıktır
/// (köprü güncellemeleri de aynı sayacı artırır; bkz. [`yayin_birlestir`]).
#[derive(Debug, Clone, Default)]
pub struct YayinKoordinatoru {
    pub havuzlar: BTreeMap<String, HavuzAnlik>,
    pub bayat_esikleri: BTreeMap<String, u64>,
}

impl YayinKoordinatoru {
    pub fn yeni(bayat_esikleri: BTreeMap<String, u64>) -> Self {
        Self {
            havuzlar: BTreeMap::new(),
            bayat_esikleri,
        }
    }

    /// Yeni okunan havuz sonucunu mevcut durumla birleştirir; geçici hatada önceki taze veriyi korur.
    pub fn havuz_sonucu_birlestir(&mut self, sonuc: BaglayiciSonucu) {
        let havuz_id = sonuc.havuz_id;
        let yeni = sonuc.havuz;
        // Geçici hatada gelen yedek (ör. Codex rollout dosyası, `bayat`) mevcut ölçümden eskiyse
        // uygulanmaz: bir dakika önceki geçerli app-server verisi saatlerce eski yedekle ezilmez.
        if sonuc.gecici_hata
            && yeni.durum != Durum::Hata
            && let Some(mevcut) = self.havuzlar.get_mut(&havuz_id)
            && !mevcut.pencereler.is_empty()
        {
            let mevcut_okuma = mevcut.kaynak.as_ref().map(|k| k.okuma);
            let yeni_okuma = yeni.kaynak.as_ref().map(|k| k.okuma);
            let yedek_eski = match (mevcut_okuma, yeni_okuma) {
                (Some(m), Some(y)) => y <= m,
                (Some(_), None) => true,
                _ => false,
            };
            if yedek_eski {
                if yeni.hata.is_some() {
                    mevcut.hata = yeni.hata;
                }
                let not = "geçici okuma hatası; daha eski yedek uygulanmadı".to_string();
                if !mevcut.notlar.contains(&not) {
                    mevcut.notlar.push(not);
                }
                return;
            }
        }
        match yeni.durum {
            Durum::Hata => {
                if let Some(mevcut) = self.havuzlar.get_mut(&havuz_id)
                    && !mevcut.pencereler.is_empty()
                {
                    // Kalıcı askı (agy model turu): son bilinen değerler bayat ve hata notuyla.
                    let askida = yeni
                        .hata
                        .as_ref()
                        .is_some_and(|h| h.tur == cetvel_baglayici::agy::AGY_MODEL_TURU_HATA);
                    mevcut.hata = yeni.hata;
                    for not in yeni.notlar {
                        if !mevcut.notlar.contains(&not) {
                            mevcut.notlar.push(not);
                        }
                    }
                    if askida && mevcut.durum == Durum::Ok {
                        mevcut.durum = Durum::Bayat;
                        mevcut.hesapla_etkin_durum();
                    }
                    return;
                }
                self.havuzlar.insert(havuz_id, yeni);
            }
            _ => {
                self.havuzlar.insert(havuz_id, yeni);
            }
        }
    }

    /// Servis yeniden başlarken son bilinen yoklanan havuzları (yoksa) tohumlar; ilk okuma
    /// hata verirse önceki pencereler korunur ve yaşla bayatlaşır.
    pub fn tohumla(&mut self, havuzlar: BTreeMap<String, HavuzAnlik>) {
        for (id, h) in havuzlar {
            self.havuzlar.entry(id).or_insert(h);
        }
    }

    /// Ayar değişince artık yoklanmayan havuzları bırakır.
    pub fn kimlikleri_koru(&mut self, kimlikler: &BTreeSet<String>) {
        self.havuzlar.retain(|id, _| kimlikler.contains(id));
    }

    /// Yaşları ve bayatlık durumlarını günceller, yayına aday havuz kopyalarını döndürür.
    pub fn yaslandirilmis(&mut self, simdi: UtcZaman) -> BTreeMap<String, HavuzAnlik> {
        for (id, havuz) in &mut self.havuzlar {
            let esik = self.bayat_esikleri.get(id).copied().unwrap_or(600);
            havuz.yas_guncelle(simdi, esik);
        }
        self.havuzlar.clone()
    }
}

/// Köprü (kota-bandi `POST /v1/claude`) üzerinden beslenen, yoklanmayan havuz mu?
pub fn kopru_havuzu_mu(havuz: &HavuzAnlik) -> bool {
    havuz.platform == "claude"
}

/// `yas_sn`, tahmin ve baskı (türetilmiş alanlar) dışındaki içerikten değişiklik imzası çıkarır.
pub fn icerik_imzasi(havuzlar: &BTreeMap<String, HavuzAnlik>) -> String {
    let mut kopya = havuzlar.clone();
    for h in kopya.values_mut() {
        if let Some(k) = h.kaynak.as_mut() {
            k.yas_sn = 0;
        }
        h.tahmin.clear();
        h.baski = None;
    }
    serde_json::to_string(&kopya).unwrap_or_default()
}

/// Servis döngüsünün yayın kararı (saf fonksiyon; paylaşılan anlığın yazma kilidi altında çağrılır).
///
/// - `mevcut`: şu an yayımlanmış anlık (köprü güncellemeleri dahil, tek `sira` kaynağı).
/// - `yoklanan`: koordinatörün yaşlandırılmış yoklanan havuzları.
/// - Köprü havuzları `mevcut`tan alınır, yaşı güncellenir; süresi dolan pencereler hesaplanır.
/// - İçerik değiştiyse `sira = mevcut.sira + 1`; değişmediyse yalnız kalp atışında aynı `sira`
///   ile yeniden yazılır; ikisi de yoksa `None`.
pub fn yayin_birlestir(
    mevcut: &AnlikV2,
    yoklanan: BTreeMap<String, HavuzAnlik>,
    yoklanan_kimlikler: &BTreeSet<String>,
    simdi: UtcZaman,
    kopru_bayat_esigi_sn: u64,
    kalp_atisi_geldi: bool,
) -> Option<AnlikV2> {
    let mut havuzlar = yoklanan;
    for (id, h) in &mevcut.havuzlar {
        if yoklanan_kimlikler.contains(id) || havuzlar.contains_key(id) || !kopru_havuzu_mu(h) {
            continue;
        }
        // 30 gündür güncellenmeyen ek köprü havuzu (`claude_max` dışı) temizlenir.
        if id != "claude_max"
            && h.kaynak
                .as_ref()
                .is_some_and(|k| simdi.fark_sn(k.okuma) > KOPRU_HAVUZ_OMRU_SN)
        {
            continue;
        }
        let mut kopya = h.clone();
        kopya.yas_guncelle(simdi, kopru_bayat_esigi_sn);
        havuzlar.insert(id.clone(), kopya);
    }
    for h in havuzlar.values_mut() {
        let _ = suresi_dolan_pencereleri_hesapla(h, simdi);
    }

    let degisti = mevcut.sira == 0 || icerik_imzasi(&havuzlar) != icerik_imzasi(&mevcut.havuzlar);
    if degisti {
        Some(AnlikV2::yeni(
            simdi,
            mevcut.sira.saturating_add(1).max(1),
            havuzlar,
        ))
    } else if kalp_atisi_geldi {
        Some(AnlikV2::yeni(simdi, mevcut.sira, havuzlar))
    } else {
        None
    }
}

/// Havuz başına yenileme isteği hız sınırı (1/dk). `*` tüm havuzlar demektir.
#[derive(Debug, Clone)]
pub struct YenileSinirlayici {
    aralik_sn: i64,
    son: BTreeMap<String, UtcZaman>,
}

impl Default for YenileSinirlayici {
    fn default() -> Self {
        Self::yeni(ZORLA_OKUMA_ASGARI_SN)
    }
}

impl YenileSinirlayici {
    pub fn yeni(aralik_sn: i64) -> Self {
        Self {
            aralik_sn: aralik_sn.max(1),
            son: BTreeMap::new(),
        }
    }

    /// İzin varsa kaydeder ve `Ok(())`; yoksa kalan bekleme süresini (sn) döndürür.
    pub fn izin_ver(&mut self, anahtar: &str, simdi: UtcZaman) -> Result<(), u64> {
        let temiz = anahtar.trim();
        let temiz = if temiz.is_empty() { "*" } else { temiz };
        if let Some(onceki) = self.son.get(temiz) {
            let gecen = simdi.fark_sn(*onceki);
            if (0..self.aralik_sn).contains(&gecen) {
                return Err((self.aralik_sn - gecen) as u64);
            }
        }
        // Serbest metin anahtarlar haritayı sınırsız büyütmesin: süresi dolanlar atılır, yine
        // de doluysa en eski giriş çıkarılır.
        if self.son.len() >= YENILE_ANAHTAR_TAVANI {
            let aralik = self.aralik_sn;
            self.son.retain(|_, t| simdi.fark_sn(*t) < aralik);
            if self.son.len() >= YENILE_ANAHTAR_TAVANI
                && let Some(eski) = self
                    .son
                    .iter()
                    .min_by_key(|(_, t)| **t)
                    .map(|(k, _)| k.clone())
            {
                self.son.remove(&eski);
            }
        }
        self.son.insert(temiz.to_string(), simdi);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cetvel_model::{GuvenEtiketi, HataBilgisi, KaynakBilgisi, KaynakTuru, PencereOlcum};

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
    }

    fn ok_sonuc(id: &str, okuma: UtcZaman, kalan: f64) -> BaglayiciSonucu {
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 1.0 - kalan,
                kalan,
                sifirlama: None,
                etiket: None,
            },
        );
        let mut h = HavuzAnlik {
            platform: "codex".into(),
            etiket: "Codex".into(),
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
            kesif: None,
            etkin_kalan_zincir: None,
        };
        h.hesapla_etkin_durum();
        BaglayiciSonucu {
            havuz_id: id.to_string(),
            havuz: h,
            retry_after_sn: None,
            gecici_hata: false,
        }
    }

    fn hata_sonuc(id: &str, retry_after_sn: Option<u64>) -> BaglayiciSonucu {
        BaglayiciSonucu {
            havuz_id: id.to_string(),
            havuz: HavuzAnlik {
                platform: "deepseek".into(),
                etiket: "DeepSeek".into(),
                durum: Durum::Hata,
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
                hata: Some(HataBilgisi {
                    tur: "hiz_siniri".into(),
                    http_kodu: Some(429),
                }),
                notlar: Vec::new(),
                kesif: None,
                etkin_kalan_zincir: None,
            },
            retry_after_sn,
            gecici_hata: true,
        }
    }

    #[test]
    fn durum_makinesi_geri_cekilme_ve_devre_kesici_uygular() {
        let mut dm = HavuzDurumMakinesi::yeni(HavuzZamanlamaKurali::default());
        let t0 = z("2026-11-15T18:00:00Z");

        assert!(dm.okunmali_mi(t0, true, false));
        dm.sonuc_isle(t0, &hata_sonuc("deepseek", None));
        assert!(matches!(
            dm.evre,
            TakipEvresi::GeriCekilme {
                ardisik_hata: 1,
                ..
            }
        ));
        // 2 dk dolmadan tekrar okunmamalı
        assert!(!dm.okunmali_mi(t0.ekle_sn(60), true, false));
        assert!(dm.okunmali_mi(t0.ekle_sn(120), true, false));

        // 2..4. hatalar: 5 dk, 15 dk, 15 dk; 5. hatada 30 dk devre kesici açılır
        let t1 = t0.ekle_sn(120);
        dm.sonuc_isle(t1, &hata_sonuc("deepseek", None)); // 2 -> 300s
        assert!(!dm.okunmali_mi(t1.ekle_sn(200), true, false));
        assert!(dm.okunmali_mi(t1.ekle_sn(300), true, false));

        let t2 = t1.ekle_sn(300);
        dm.sonuc_isle(t2, &hata_sonuc("deepseek", Some(1200))); // 3 -> max(900, Retry-After 1200)
        assert!(!dm.okunmali_mi(t2.ekle_sn(1000), true, false));
        assert!(dm.okunmali_mi(t2.ekle_sn(1200), true, false));

        let t3 = t2.ekle_sn(1200);
        dm.sonuc_isle(t3, &hata_sonuc("deepseek", None)); // 4 -> 900s
        let t4 = t3.ekle_sn(900);
        dm.sonuc_isle(t4, &hata_sonuc("deepseek", None)); // 5 -> DevreAcik (1800s)

        assert!(matches!(dm.evre, TakipEvresi::DevreAcik { .. }));
        assert!(!dm.okunmali_mi(t4.ekle_sn(1799), true, false));
        // 30 dk sonra YariAcik probu başlar
        assert!(dm.okunmali_mi(t4.ekle_sn(1800), true, false));
        assert!(matches!(dm.evre, TakipEvresi::YariAcik { .. }));

        // Prob başarılı olursa normale döner
        let t5 = t4.ekle_sn(1800);
        dm.sonuc_isle(t5, &ok_sonuc("deepseek", t5, 0.9));
        assert!(matches!(dm.evre, TakipEvresi::Aktif { .. }));
    }

    #[test]
    fn uyarlamali_etkin_ve_bosta_aralik_ile_uyanma_calisir() {
        let mut dm = HavuzDurumMakinesi::yeni(HavuzZamanlamaKurali {
            etkin_aralik_sn: 300,
            bosta_aralik_sn: 1800,
            bayat_esigi_sn: 600,
        });
        let t0 = z("2026-11-15T18:00:00Z");
        dm.sonuc_isle(t0, &ok_sonuc("codex_plus", t0, 0.8));

        // Etkin kullanıcı: 300 sn
        assert!(!dm.okunmali_mi(t0.ekle_sn(299), true, false));
        assert!(dm.okunmali_mi(t0.ekle_sn(300), true, false));
        // Boşta kullanıcı: 1800 sn, ama uyanma olursa hemen okunur
        assert!(!dm.okunmali_mi(t0.ekle_sn(600), false, false));
        assert!(dm.okunmali_mi(t0.ekle_sn(600), false, true));
        assert!(uyanma_algila(t0, t0.ekle_sn(300), 30));
        assert!(!uyanma_algila(t0, t0.ekle_sn(45), 30));
    }

    fn claude_havuzu(kullanilan: f64, okuma: UtcZaman) -> HavuzAnlik {
        let mut h = ok_sonuc("claude_max", okuma, 1.0 - kullanilan).havuz;
        h.platform = "claude".into();
        h.etiket = "Claude".into();
        h
    }

    #[test]
    fn yayin_birlestir_sira_kalp_atisi_ve_degisimsizlik() {
        let t0 = z("2026-11-15T18:00:00Z");
        let kimlikler = BTreeSet::from(["codex_plus".to_string()]);
        let bos = AnlikV2::yeni(t0, 0, BTreeMap::new());

        let mut k = YayinKoordinatoru::yeni(BTreeMap::from([("codex_plus".to_string(), 600)]));
        k.havuz_sonucu_birlestir(ok_sonuc("codex_plus", t0, 0.75));

        // İlk yayın: sira 1
        let a1 = yayin_birlestir(&bos, k.yaslandirilmis(t0), &kimlikler, t0, 900, false)
            .expect("ilk yayin");
        assert_eq!(a1.sira, 1);

        // Değişiklik yok, kalp atışı yok: yayın yok
        let t1 = t0.ekle_sn(10);
        assert!(yayin_birlestir(&a1, k.yaslandirilmis(t1), &kimlikler, t1, 900, false).is_none());

        // Kalp atışı: aynı sira, güncel yaş
        let t2 = t0.ekle_sn(60);
        let a2 = yayin_birlestir(&a1, k.yaslandirilmis(t2), &kimlikler, t2, 900, true)
            .expect("kalp atisi");
        assert_eq!(a2.sira, 1);
        assert_eq!(
            a2.havuzlar["codex_plus"].kaynak.as_ref().unwrap().yas_sn,
            60
        );

        // Bayatlığa geçiş anlamlı değişimdir: sira 2
        let t3 = t0.ekle_sn(700);
        let a3 =
            yayin_birlestir(&a2, k.yaslandirilmis(t3), &kimlikler, t3, 900, false).expect("bayat");
        assert_eq!(a3.sira, 2);
        assert_eq!(a3.havuzlar["codex_plus"].durum, Durum::Bayat);
    }

    #[test]
    fn kopru_ve_baglayici_ayni_tikte_degisince_sira_her_seferinde_artar_ve_kopru_korunur() {
        // C6: yayın 10 → köprü 11 → Codex değişir → yeni içerik 12 olmalı (11 değil).
        let t0 = z("2026-11-15T18:00:00Z");
        let kimlikler = BTreeSet::from(["codex_plus".to_string()]);
        let mut k = YayinKoordinatoru::yeni(BTreeMap::new());
        k.havuz_sonucu_birlestir(ok_sonuc("codex_plus", t0, 0.80));
        let mut yayinlanan = AnlikV2::yeni(t0, 10, k.yaslandirilmis(t0));

        // Köprü güncellemesi (paylaşılan durumda sira+1 ile uygulanır)
        yayinlanan.sira += 1;
        yayinlanan
            .havuzlar
            .insert("claude_max".into(), claude_havuzu(0.30, t0));

        // Aynı tikte Codex değişti
        k.havuz_sonucu_birlestir(ok_sonuc("codex_plus", t0, 0.60));
        let yeni = yayin_birlestir(
            &yayinlanan,
            k.yaslandirilmis(t0),
            &kimlikler,
            t0,
            900,
            false,
        )
        .expect("degisim yayinlanmali");
        assert_eq!(yeni.sira, 12);
        assert!(yeni.havuzlar.contains_key("claude_max"), "köprü ezilmemeli");
        assert!((yeni.havuzlar["codex_plus"].pencereler["5s"].kalan - 0.60).abs() < 1e-9);
    }

    #[test]
    fn kapatilan_baglayicinin_eski_havuzu_atilir_kopru_havuzu_kalir() {
        let t0 = z("2026-11-15T18:00:00Z");
        let mut mevcut = AnlikV2::yeni(t0, 5, BTreeMap::new());
        mevcut
            .havuzlar
            .insert("agy_gemini".into(), ok_sonuc("agy_gemini", t0, 0.5).havuz);
        mevcut
            .havuzlar
            .insert("claude_max".into(), claude_havuzu(0.2, t0));
        let yeni = yayin_birlestir(&mevcut, BTreeMap::new(), &BTreeSet::new(), t0, 900, false)
            .expect("degisim");
        assert!(!yeni.havuzlar.contains_key("agy_gemini"));
        assert!(yeni.havuzlar.contains_key("claude_max"));
        assert_eq!(yeni.sira, 6);
    }

    #[test]
    fn yoklanan_havuzda_sifirlama_gecince_hesaplanir_sira_artar_ve_okuma_istenir() {
        // B4: Codex %100, sıfırlama 18:30; 18:31'de hesap + hemen okuma
        let t0 = z("2026-11-15T18:10:00Z");
        let sifir = z("2026-11-15T18:30:00Z");
        let mut sonuc = ok_sonuc("codex_plus", t0, 0.0);
        sonuc.havuz.pencereler.get_mut("5s").expect("5s").sifirlama = Some(sifir);
        sonuc.havuz.hesapla_etkin_durum();
        let kimlikler = BTreeSet::from(["codex_plus".to_string()]);
        let mut k = YayinKoordinatoru::yeni(BTreeMap::from([("codex_plus".to_string(), 3600)]));
        let mut dm = HavuzDurumMakinesi::yeni(HavuzZamanlamaKurali::default());
        dm.sonuc_isle(t0, &sonuc);
        k.havuz_sonucu_birlestir(sonuc);
        let a1 = yayin_birlestir(
            &AnlikV2::yeni(t0, 0, BTreeMap::new()),
            k.yaslandirilmis(t0),
            &kimlikler,
            t0,
            900,
            false,
        )
        .expect("ilk");
        assert_eq!(a1.havuzlar["codex_plus"].kullanilabilir, Some(false));

        let t1 = z("2026-11-15T18:31:00Z");
        assert!(
            !dm.okunmali_mi(t1, false, false),
            "boşta aralık 30 dk dolmadı"
        );
        assert!(dm.sifirlama_gecti_mi(k.havuzlar.get("codex_plus"), t1));
        let a2 = yayin_birlestir(&a1, k.yaslandirilmis(t1), &kimlikler, t1, 900, false)
            .expect("sifirlama sonrasi degisim");
        assert_eq!(a2.sira, a1.sira + 1);
        let p = &a2.havuzlar["codex_plus"].pencereler["5s"];
        assert_eq!(p.kullanilan, 0.0);
        assert_eq!(a2.havuzlar["codex_plus"].kullanilabilir, Some(true));

        // Okuma sonrası (sıfırlama son okumadan önce kaldı) tekrar tetiklenmez
        dm.sonuc_isle(t1, &ok_sonuc("codex_plus", t1, 1.0));
        assert!(!dm.sifirlama_gecti_mi(k.havuzlar.get("codex_plus"), t1.ekle_sn(120)));
    }

    #[test]
    fn eski_yedek_taze_olcumu_ezmez_yeni_yedek_uygulanir() {
        let t = z("2026-11-15T18:00:00Z");
        let mut k = YayinKoordinatoru::yeni(BTreeMap::new());
        k.havuz_sonucu_birlestir(ok_sonuc("codex_plus", t, 0.60));

        // App-server zaman aşımı → rollout yedeği (3 saat önceki %90 kalan, bayat, geçici hata).
        let mut yedek = ok_sonuc("codex_plus", t.ekle_sn(-3 * 3600), 0.90);
        yedek.havuz.durum = Durum::Bayat;
        yedek.havuz.kaynak.as_mut().unwrap().tur = KaynakTuru::RolloutYedek;
        yedek.gecici_hata = true;
        k.havuz_sonucu_birlestir(yedek);
        let h = &k.havuzlar["codex_plus"];
        assert_eq!(h.durum, Durum::Ok);
        assert!((h.pencereler["5s"].kalan - 0.60).abs() < 1e-9);
        assert_eq!(h.kaynak.as_ref().unwrap().tur, KaynakTuru::CodexAppServer);

        // Yedek mevcut ölçümden yeniyse uygulanır.
        let mut yeni_yedek = ok_sonuc("codex_plus", t.ekle_sn(600), 0.50);
        yeni_yedek.havuz.durum = Durum::Bayat;
        yeni_yedek.gecici_hata = true;
        k.havuz_sonucu_birlestir(yeni_yedek);
        assert!((k.havuzlar["codex_plus"].pencereler["5s"].kalan - 0.50).abs() < 1e-9);

        // Hiç veri yokken yedek gösterilir.
        let mut bos = YayinKoordinatoru::yeni(BTreeMap::new());
        let mut ilk_yedek = ok_sonuc("codex_plus", t.ekle_sn(-3600), 0.70);
        ilk_yedek.havuz.durum = Durum::Bayat;
        ilk_yedek.gecici_hata = true;
        bos.havuz_sonucu_birlestir(ilk_yedek);
        assert_eq!(bos.havuzlar["codex_plus"].durum, Durum::Bayat);
    }

    #[test]
    fn bayat_esigi_planli_araliktan_hep_buyuk() {
        assert_eq!(bayat_esigi_hesapla(600, 1800), 3600);
        assert_eq!(bayat_esigi_hesapla(600, 120), 600);
        let k = HavuzZamanlamaKurali::default();
        assert!(k.bayat_esigi_sn > k.bosta_aralik_sn);
    }

    #[test]
    fn baglanmadi_evresi_on_dakikada_bir_yeniden_denenir() {
        let mut dm = HavuzDurumMakinesi::yeni(HavuzZamanlamaKurali::default());
        let t0 = z("2026-11-15T18:00:00Z");
        let mut s = ok_sonuc("codex_plus", t0, 1.0);
        s.havuz.durum = Durum::Baglanmadi;
        dm.sonuc_isle(t0, &s);
        assert!(matches!(dm.evre, TakipEvresi::Baglanmadi { .. }));
        assert!(!dm.okunmali_mi(t0.ekle_sn(599), true, false));
        assert!(dm.okunmali_mi(t0.ekle_sn(600), true, false));
        assert!(dm.zorla_okunabilir_mi(t0.ekle_sn(1)));
    }

    #[test]
    fn zorla_okuma_geri_cekilmeyi_ve_dakikalik_siniri_korur() {
        let mut dm = HavuzDurumMakinesi::yeni(HavuzZamanlamaKurali::default());
        let t0 = z("2026-11-15T18:00:00Z");
        assert!(dm.zorla_okunabilir_mi(t0));
        dm.sonuc_isle(t0, &ok_sonuc("codex_plus", t0, 0.9));
        assert!(!dm.zorla_okunabilir_mi(t0.ekle_sn(59)));
        assert!(dm.zorla_okunabilir_mi(t0.ekle_sn(60)));
        dm.sonuc_isle(t0.ekle_sn(60), &hata_sonuc("codex_plus", None));
        assert!(!dm.zorla_okunabilir_mi(t0.ekle_sn(61)));
    }

    #[test]
    fn yenile_sinirlayici_havuz_basina_dakikada_bir_izin_verir() {
        let mut s = YenileSinirlayici::default();
        let t0 = z("2026-11-15T18:00:00Z");
        assert!(s.izin_ver("codex_plus", t0).is_ok());
        assert_eq!(s.izin_ver("codex_plus", t0.ekle_sn(20)), Err(40));
        assert!(s.izin_ver("deepseek", t0.ekle_sn(20)).is_ok());
        assert!(s.izin_ver("", t0).is_ok());
        assert_eq!(s.izin_ver("*", t0.ekle_sn(1)), Err(59));
        assert!(s.izin_ver("codex_plus", t0.ekle_sn(60)).is_ok());
    }

    #[test]
    fn model_turu_askisi_son_degerleri_bayat_ve_hata_notuyla_tutar() {
        let t0 = z("2026-11-20T10:00:00Z");
        let mut k = YayinKoordinatoru::default();
        k.havuz_sonucu_birlestir(ok_sonuc("agy_gemini_5_xenon", t0, 0.7));
        let mut aski = hata_sonuc("agy_gemini_5_xenon", None);
        aski.gecici_hata = false;
        aski.havuz.hata = Some(HataBilgisi {
            tur: cetvel_baglayici::agy::AGY_MODEL_TURU_HATA.to_string(),
            http_kodu: None,
        });
        aski.havuz.notlar = vec!["agy /usage model isteğine dönüştü".into()];
        k.havuz_sonucu_birlestir(aski);
        let h = &k.havuzlar["agy_gemini_5_xenon"];
        assert_eq!(h.durum, Durum::Bayat);
        assert!((h.pencereler["5s"].kalan - 0.7).abs() < 1e-9);
        assert!(h.notlar.iter().any(|n| n.contains("model isteğine")));
        assert_eq!(h.kullanilabilir, Some(false));
    }

    #[test]
    fn retry_after_dev_deger_kistirilir() {
        let t0 = z("2026-11-20T10:00:00Z");
        let mut dm = HavuzDurumMakinesi::yeni(HavuzZamanlamaKurali::default());
        dm.sonuc_isle(t0, &hata_sonuc("deepseek", Some(u64::MAX)));
        match dm.evre {
            TakipEvresi::GeriCekilme { sonraki_deneme, .. } => {
                assert_eq!(sonraki_deneme.fark_sn(t0), RETRY_AFTER_TAVAN_SN as i64);
            }
            diger => panic!("beklenmeyen evre {diger:?}"),
        }
    }
}
