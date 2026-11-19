//! Okuyucuların tuttuğu iç ajan kaydı ve yayın kuralları (bitmişin kısa süre kalıp düşmesi, askı).

use cetvel_model::UtcZaman;

use crate::model::{Ajan, AjanDurumu, AjanSayilari, Etkinlik, HataTuru, Platform, kimlik_karma};

/// Görünürlük kuralları (saniye).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kurallar {
    /// Bitti/hata/iptal ajan bu süre sonra listeden düşer.
    pub bitis_bekleme_sn: i64,
    /// Çalışıyor görünüp bu süredir etkinliği olmayan ajan `askida` olur.
    pub askida_sn: i64,
    /// Bu süredir etkinliği olmayan her ajan listeden düşer (ve kaynağı izlenmez).
    pub pencere_sn: i64,
}

impl Default for Kurallar {
    fn default() -> Self {
        Self {
            bitis_bekleme_sn: 60,
            askida_sn: 600,
            pencere_sn: 7200,
        }
    }
}

/// Okuyucu içindeki tek ajan. `yerel_kimlik` ve `ebeveyn_yerel` platformun ham kimlikleridir ve
/// yalnız karma olarak yayınlanır.
#[derive(Debug, Clone, PartialEq)]
pub struct AjanKaydi {
    pub platform: Platform,
    pub yerel_kimlik: String,
    pub ebeveyn_yerel: Option<String>,
    pub derinlik: u32,
    pub ad: String,
    pub model: Option<String>,
    pub efor: Option<String>,
    pub durum: AjanDurumu,
    pub etkinlik: Option<Etkinlik>,
    pub hata_turu: Option<HataTuru>,
    /// Geçerli turda görülen hata (Codex: `error` ardından gelen `task_complete` hatayı ezmez).
    /// Yalnız [`Self::tur_hatasi_isle`] doldurur, [`Self::basladi`] temizler.
    pub tur_hatasi: Option<HataTuru>,
    pub baslama: Option<UtcZaman>,
    pub son_etkinlik: UtcZaman,
    pub bitis: Option<UtcZaman>,
    pub sayilar: AjanSayilari,
}

impl AjanKaydi {
    pub fn yeni(platform: Platform, yerel_kimlik: impl Into<String>, zaman: UtcZaman) -> Self {
        Self {
            platform,
            yerel_kimlik: yerel_kimlik.into(),
            ebeveyn_yerel: None,
            derinlik: 0,
            ad: String::new(),
            model: None,
            efor: None,
            durum: AjanDurumu::Bekliyor,
            etkinlik: None,
            hata_turu: None,
            tur_hatasi: None,
            baslama: None,
            son_etkinlik: zaman,
            bitis: None,
            sayilar: AjanSayilari::default(),
        }
    }

    /// Olay zamanını işler (geri giden zaman son etkinliği geri almaz).
    pub fn zaman_isle(&mut self, zaman: UtcZaman) {
        if zaman > self.son_etkinlik {
            self.son_etkinlik = zaman;
        }
        if self.baslama.is_none() {
            self.baslama = Some(zaman);
        }
    }

    /// Yeni tur / iş başladı: bitiş ve hata temizlenir.
    pub fn basladi(&mut self, zaman: UtcZaman) {
        self.zaman_isle(zaman);
        self.durum = AjanDurumu::Calisiyor;
        self.bitis = None;
        self.hata_turu = None;
        self.tur_hatasi = None;
    }

    /// Tur içinde hata görüldü: ajan `hata` olur ve tur kapanana dek hata hatırlanır.
    pub fn tur_hatasi_isle(&mut self, zaman: UtcZaman, hata: HataTuru) {
        self.tur_hatasi = Some(hata);
        self.bitir(zaman, AjanDurumu::Hata, Some(hata));
    }

    /// Etkinlik olayı. Bitmiş ajanı yeniden canlandırmaz (yeni tur yalnız [`Self::basladi`] ile);
    /// tek istisna tur içi hatadır: hata ölümcül değilse ve ajan çalışmayı sürdürüyorsa yeniden
    /// `calisiyor` olur (tur kapanınca yine `hata` yayınlanır).
    pub fn etkinlik_isle(&mut self, zaman: UtcZaman, etkinlik: Etkinlik) {
        self.zaman_isle(zaman);
        if self.durum == AjanDurumu::Hata && self.tur_hatasi.is_some() {
            self.bitis = None;
        } else if self.durum.bitmis_mi() {
            return;
        }
        self.durum = AjanDurumu::Calisiyor;
        self.etkinlik = Some(etkinlik);
    }

    /// Ajan bitti / hata / iptal. Turda hata görüldüyse `bitti` hatayı ezmez, `hata` kalır.
    pub fn bitir(&mut self, zaman: UtcZaman, durum: AjanDurumu, hata: Option<HataTuru>) {
        self.zaman_isle(zaman);
        let (durum, hata) = match (durum, self.tur_hatasi) {
            (AjanDurumu::Bitti, Some(t)) => (AjanDurumu::Hata, Some(t)),
            (AjanDurumu::Hata, t) => (durum, hata.or(t)),
            _ => (durum, hata),
        };
        self.durum = durum;
        self.bitis = Some(zaman);
        self.hata_turu = if durum == AjanDurumu::Hata {
            Some(hata.unwrap_or(HataTuru::Bilinmiyor))
        } else {
            None
        };
    }

    /// Kurallara göre yayınlanacak biçim; düşmesi gerekiyorsa `None`.
    pub fn yayin(&self, simdi: UtcZaman, kurallar: &Kurallar) -> Option<Ajan> {
        let durgun = simdi.fark_sn(self.son_etkinlik);
        if durgun > kurallar.pencere_sn {
            return None;
        }
        let mut durum = self.durum;
        if durum.bitmis_mi() {
            let bitis = self.bitis.unwrap_or(self.son_etkinlik);
            if simdi.fark_sn(bitis) > kurallar.bitis_bekleme_sn {
                return None;
            }
        } else if durum == AjanDurumu::Calisiyor && durgun > kurallar.askida_sn {
            durum = AjanDurumu::Askida;
        }
        Some(Ajan {
            kimlik: kimlik_karma(self.platform, &self.yerel_kimlik),
            platform: self.platform,
            ebeveyn: self
                .ebeveyn_yerel
                .as_deref()
                .map(|e| kimlik_karma(self.platform, e)),
            derinlik: self.derinlik,
            ad: self.ad.clone(),
            model: self.model.clone(),
            efor: self.efor.clone(),
            durum,
            etkinlik: self.etkinlik,
            hata_turu: if durum == AjanDurumu::Hata {
                self.hata_turu
            } else {
                None
            },
            baslama: self.baslama,
            son_etkinlik: self.son_etkinlik,
            bitis: if durum.bitmis_mi() { self.bitis } else { None },
            sayilar: self.sayilar.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).unwrap()
    }

    #[test]
    fn bitmis_ajan_bekleme_suresi_sonra_duser_askida_hesaplanir() {
        let k = Kurallar::default();
        let mut a = AjanKaydi::yeni(Platform::Codex, "t1", z("2026-11-16T10:00:00Z"));
        a.basladi(z("2026-11-16T10:00:00Z"));
        a.etkinlik_isle(z("2026-11-16T10:00:05Z"), Etkinlik::Test);
        let y = a.yayin(z("2026-11-16T10:00:10Z"), &k).unwrap();
        assert_eq!(y.durum, AjanDurumu::Calisiyor);
        assert_eq!(y.etkinlik, Some(Etkinlik::Test));

        let y = a.yayin(z("2026-11-16T10:11:00Z"), &k).unwrap();
        assert_eq!(y.durum, AjanDurumu::Askida);
        assert!(a.yayin(z("2026-11-16T12:10:00Z"), &k).is_none());

        a.bitir(z("2026-11-16T10:01:00Z"), AjanDurumu::Bitti, None);
        assert_eq!(
            a.yayin(z("2026-11-16T10:01:59Z"), &k).unwrap().durum,
            AjanDurumu::Bitti
        );
        assert!(a.yayin(z("2026-11-16T10:02:01Z"), &k).is_none());

        // Bitmiş ajana gelen geç etkinlik onu canlandırmaz; yeni tur canlandırır.
        a.etkinlik_isle(z("2026-11-16T10:01:30Z"), Etkinlik::Komut);
        assert_eq!(a.durum, AjanDurumu::Bitti);
        a.basladi(z("2026-11-16T10:05:00Z"));
        assert_eq!(a.durum, AjanDurumu::Calisiyor);
        assert!(a.bitis.is_none());
    }

    #[test]
    fn hata_turu_yalniz_hata_durumunda_yayinlanir() {
        let k = Kurallar::default();
        let t = z("2026-11-16T10:00:00Z");
        let mut a = AjanKaydi::yeni(Platform::Antigravity, "e", t);
        a.bitir(t, AjanDurumu::Hata, Some(HataTuru::HizSiniri));
        assert_eq!(a.yayin(t, &k).unwrap().hata_turu, Some(HataTuru::HizSiniri));
        a.bitir(t, AjanDurumu::Iptal, Some(HataTuru::Ag));
        assert_eq!(a.yayin(t, &k).unwrap().hata_turu, None);
    }

    #[test]
    fn tur_hatasi_sonraki_bitti_ile_ezilmez_yeni_tur_temizler() {
        let k = Kurallar::default();
        let mut a = AjanKaydi::yeni(Platform::Codex, "t", z("2026-11-16T10:00:00Z"));
        a.basladi(z("2026-11-16T10:00:00Z"));
        a.tur_hatasi_isle(z("2026-11-16T10:00:10Z"), HataTuru::HizSiniri);
        // Ölümcül olmayan hata: çalışma sürerse ajan canlı kalır.
        a.etkinlik_isle(z("2026-11-16T10:00:20Z"), Etkinlik::Komut);
        assert_eq!(a.durum, AjanDurumu::Calisiyor);
        assert!(a.bitis.is_none());
        a.bitir(z("2026-11-16T10:00:30Z"), AjanDurumu::Bitti, None);
        let y = a.yayin(z("2026-11-16T10:00:31Z"), &k).unwrap();
        assert_eq!(y.durum, AjanDurumu::Hata);
        assert_eq!(y.hata_turu, Some(HataTuru::HizSiniri));

        a.basladi(z("2026-11-16T10:05:00Z"));
        a.bitir(z("2026-11-16T10:05:30Z"), AjanDurumu::Bitti, None);
        assert_eq!(a.durum, AjanDurumu::Bitti);
        assert_eq!(a.hata_turu, None);
    }
}
