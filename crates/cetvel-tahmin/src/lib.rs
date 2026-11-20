//! Kota tüketim hızı (EWMA), tükenme saati tahmini ve baskı etiketi hesaplamaları.
//! Tüm tahmin çıktıları doğrulama kapısı geçene kadar `deneysel` etiketiyle üretilir.

use std::collections::BTreeSet;

use cetvel_model::{
    Baski, Durum, GuvenEtiketi, HavuzAnlik, PencereOlcum, TahminBilgisi, TahminDurumu, UtcZaman,
};

const MIN_AYRIK_ORNEK: usize = 3;
const MIN_GOZLEM_SURESI_SN: i64 = 600; // 10 dk
const KISA_PENCERE_SN: i64 = 1800; // 30 dk
const EWMA_YARI_OMUR_SN: f64 = 600.0; // 10 dk
const AYNI_SIFIRLAMA_TOLERANSI_SN: i64 = 600;
const TAHMINI_GURULTU_ESIGI: f64 = 0.01;
const MAKS_TAHMIN_UFKU_SN: f64 = 365.0 * 86400.0;

/// Tek bir zaman-serisi kullanım örneği (`gecmis.sqlite` veya bellek içi).
#[derive(Debug, Clone, PartialEq)]
pub struct OrnekNoktasi {
    pub zaman: UtcZaman,
    pub kullanilan: f64,
    pub sifirlama: Option<UtcZaman>,
    pub tahmini_kaynak: bool,
    pub hassasiyet: Option<f64>,
}

/// Verilen pencere ve örnek serisi için EWMA + pencere ortalamalı bitiş saati tahmini üretir.
pub fn pencere_tahmini(
    ornekler: &[OrnekNoktasi],
    mevcut: &PencereOlcum,
    simdi: UtcZaman,
    tahmini_kaynak: bool,
) -> TahminBilgisi {
    let simdiki_u = mevcut.kullanilan;
    if !simdiki_u.is_finite() || simdiki_u <= 0.0 {
        return tahmin_yok("kullanım henüz başlamadı");
    }
    if simdiki_u >= 1.0 {
        return tahmin_yok("pencere kotası zaten doldu");
    }
    let Some(sifirlama) = mevcut.sifirlama else {
        return tahmin_yok("aktif sıfırlama zamanı yok");
    };
    if sifirlama <= simdi {
        return tahmin_yok("pencere sıfırlanma zamanı geçti");
    }

    let suzulen = sifirlama_ici_ornekleri_ayikla(ornekler, sifirlama, simdi);
    if suzulen.is_empty() {
        return tahmin_yok("3'ten az ayrık örnek");
    }

    let ayrik_sayisi = ayrik_deger_sayisi(&suzulen);
    if ayrik_sayisi < MIN_AYRIK_ORNEK {
        return tahmin_yok("3'ten az ayrık örnek");
    }

    let tumu_tamsayi = suzulen
        .iter()
        .all(|o| o.hassasiyet.is_some_and(|h| h >= 1.0));
    if tumu_tamsayi {
        let tamsayi_basamaklar: BTreeSet<i64> = suzulen
            .iter()
            .map(|o| (o.kullanilan * 100.0).round() as i64)
            .collect();
        if tamsayi_basamaklar.len() < 3 {
            return tahmin_yok("tam sayı kaynakta yeterli basamak değişimi yok");
        }
    }

    let ilk = &suzulen[0];
    let son = &suzulen[suzulen.len() - 1];
    let toplam_sure_sn = son.zaman.fark_sn(ilk.zaman);
    if toplam_sure_sn < MIN_GOZLEM_SURESI_SN {
        return tahmin_yok("10 dakikadan kısa gözlem aralığı");
    }

    let min_u = suzulen
        .iter()
        .map(|o| o.kullanilan)
        .fold(f64::INFINITY, f64::min);
    let max_u = suzulen
        .iter()
        .map(|o| o.kullanilan)
        .fold(f64::NEG_INFINITY, f64::max);
    let tumu_tahmini = tahmini_kaynak || suzulen.iter().all(|o| o.tahmini_kaynak);
    if tumu_tahmini && (max_u - min_u) <= TAHMINI_GURULTU_ESIGI {
        return tahmin_yok("tahmini kaynakta değişim gürültü aralığında");
    }

    let r_uzun = (son.kullanilan - ilk.kullanilan) / (toplam_sure_sn as f64);
    if !r_uzun.is_finite() || r_uzun <= 1e-12 {
        return tahmin_yok("tüketim hızı sıfır veya azalıyor");
    }

    // r_uzun pencerenin gerçek başından (ilk kullanım örneği) hesaplanır: sıfırlaması bilinmeyen
    // boşta örnekler (`u = 0`, Codex) yukarıda ayıklandı.
    let r_kisa = kisa_ewma_hizi(&suzulen, son.zaman).unwrap_or(r_uzun);
    let r_kisa = if r_kisa.is_finite() && r_kisa > 1e-12 {
        r_kisa
    } else {
        r_uzun
    };

    let r_hizli = r_kisa.max(r_uzun);
    let r_yavas = r_kisa.min(r_uzun);
    let r_orta = 0.5 * (r_kisa + r_uzun);

    let kalan = (1.0 - simdiki_u).clamp(0.0, 1.0);
    let bitis_tahmini = zaman_ekle_orandan(simdi, kalan, r_orta);
    let bitis_erken = zaman_ekle_orandan(simdi, kalan, r_hizli);
    let bitis_gec = zaman_ekle_orandan(simdi, kalan, r_yavas);
    let sifirlanmadan_biter = bitis_tahmini < sifirlama;

    let gunluk_butce = if mevcut.uzunluk_dk >= 1440 {
        let kalan_gun = (sifirlama.fark_sn(simdi) as f64 / 86400.0).max(1.0 / 24.0);
        Some((mevcut.kalan / kalan_gun).clamp(0.0, 1.0))
    } else {
        None
    };

    TahminBilgisi {
        durum: TahminDurumu::Deneysel,
        etiket: Some("deneysel".to_string()),
        neden: None,
        bitis_tahmini: Some(bitis_tahmini),
        bitis_erken: Some(bitis_erken),
        bitis_gec: Some(bitis_gec),
        saatlik_hiz: Some((r_orta * 3600.0 * 10000.0).round() / 10000.0),
        sifirlanmadan_biter: Some(sifirlanmadan_biter),
        gunluk_butce,
    }
}

/// Havuzdaki tüm pencereler için tahminleri doldurur ve bağlayıcı pencereye göre baskıyı günceller.
///
/// `taban_esik(pencere)`: o pencerenin orkestra tabanı (Claude 5s %15, hafta %30, diğer %20).
/// Herhangi bir pencere tabanının altındaysa baskı `kaçın` olur.
pub fn havuz_tahminlerini_doldur<F, T>(
    havuz: &mut HavuzAnlik,
    simdi: UtcZaman,
    ornek_getir: F,
    taban_esik: T,
) where
    F: Fn(&str) -> Vec<OrnekNoktasi>,
    T: Fn(&str) -> f64,
{
    if havuz.durum != Durum::Ok || havuz.pencereler.is_empty() {
        return;
    }
    let tahmini_kaynak = havuz
        .kaynak
        .as_ref()
        .is_some_and(|k| k.guven == GuvenEtiketi::Tahmini);

    for (pencere_ad, olcum) in &havuz.pencereler {
        let ornekler = ornek_getir(pencere_ad);
        let tahmin = pencere_tahmini(&ornekler, olcum, simdi, tahmini_kaynak);
        havuz.tahmin.insert(pencere_ad.clone(), tahmin);
    }

    if let Some(baglayici) = havuz.baglayici_pencere.as_deref()
        && let Some(olcum) = havuz.pencereler.get(baglayici)
        && let Some(tahmin) = havuz.tahmin.get(baglayici)
    {
        let mut guncel = baski_hesapla(havuz.baski, olcum, tahmin, simdi, taban_esik(baglayici));
        if havuz
            .pencereler
            .iter()
            .any(|(ad, p)| p.kalan.is_finite() && p.kalan < taban_esik(ad))
        {
            guncel = Baski::Kacin;
        }
        havuz.baski = Some(guncel);
    }
}

/// Orkestra tabanı varsayılanları: Claude 5s %15, Claude hafta %30, diğer %20.
pub fn varsayilan_taban_esigi(platform: &str, pencere: &str) -> f64 {
    match (platform, pencere) {
        ("claude", "5s") => 0.15,
        ("claude", "hafta") => 0.30,
        _ => 0.20,
    }
}

/// Tahmin ve kalan orana göre baskı etiketini belirler.
///
/// - `kalan <= 0` ya da `kalan < taban_esik` (orkestra tabanı) → `kaçın`.
/// - Tabanın üstünde model bandı (`hesapla_etkin_durum`: ≤ %20 `dikkat`) korunur.
/// - Tahmin yalnız kısa pencerelerde (< 1 gün) ya da doğrulanmış (`hazir`) tahminde baskıyı
///   yükseltir: deneysel günlük/haftalık tahmin, kısa bir yoğun oturumu haftaya yaydığı için
///   doğrulama kapısı geçene dek tepsi rengini değiştirmez.
pub fn baski_hesapla(
    mevcut: Option<Baski>,
    pencere: &PencereOlcum,
    tahmin: &TahminBilgisi,
    _simdi: UtcZaman,
    taban_esik: f64,
) -> Baski {
    if pencere.kalan <= 0.0 || pencere.kalan < taban_esik {
        return Baski::Kacin;
    }

    let tahmin_baskiya_girer = match tahmin.durum {
        TahminDurumu::TahminYok => false,
        TahminDurumu::Hazir => true,
        TahminDurumu::Deneysel => pencere.uzunluk_dk < 1440,
    };
    if tahmin_baskiya_girer
        && tahmin.sifirlanmadan_biter == Some(true)
        && let (Some(bitis), Some(sifirlama)) = (tahmin.bitis_tahmini, pencere.sifirlama)
    {
        let fark_sn = sifirlama.fark_sn(bitis);
        if fark_sn > 7200 {
            return Baski::Kacin;
        }
        if fark_sn > 1800 {
            return match mevcut {
                Some(Baski::Kacin) => Baski::Kacin,
                _ => Baski::Dikkat,
            };
        }
    }

    mevcut.unwrap_or(Baski::Rahat)
}

fn tahmin_yok(neden: &str) -> TahminBilgisi {
    TahminBilgisi {
        durum: TahminDurumu::TahminYok,
        etiket: None,
        neden: Some(neden.to_string()),
        bitis_tahmini: None,
        bitis_erken: None,
        bitis_gec: None,
        saatlik_hiz: None,
        sifirlanmadan_biter: None,
        gunluk_butce: None,
    }
}

fn sifirlama_ici_ornekleri_ayikla(
    ornekler: &[OrnekNoktasi],
    sifirlama: UtcZaman,
    simdi: UtcZaman,
) -> Vec<OrnekNoktasi> {
    // Sıfırlaması olmayan örnekler mevcut pencereye ait sayılmaz: Codex `usedPercent == 0`
    // iken sıfırlama bilerek yazılmaz (pencere henüz başlamadı); hesaplanmış (`hesap`) satırlar
    // da sıfırlamasızdır. Bunlar r_uzun'u sahte bir sıfır noktasından başlatırdı.
    let mut adaylar: Vec<OrnekNoktasi> = ornekler
        .iter()
        .filter(|o| {
            o.zaman <= simdi
                && o.kullanilan.is_finite()
                && (0.0..=1.0).contains(&o.kullanilan)
                && o.sifirlama
                    .is_some_and(|s| (s.fark_sn(sifirlama)).abs() <= AYNI_SIFIRLAMA_TOLERANSI_SN)
        })
        .cloned()
        .collect();

    adaylar.sort_by_key(|o| o.zaman);

    // Aynı zaman damgalı tekrarlarda sonuncuyu koru.
    let mut tekil: Vec<OrnekNoktasi> = Vec::with_capacity(adaylar.len());
    for o in adaylar {
        if let Some(son) = tekil.last_mut()
            && son.zaman == o.zaman
        {
            *son = o;
        } else {
            tekil.push(o);
        }
    }

    // Sıfırlama / büyük düşüş sonrasını al; küçük geri gitmeleri de monoton kesimde süz.
    let mut baslangic = 0usize;
    for i in 1..tekil.len() {
        if tekil[i].kullanilan + 0.05 < tekil[i - 1].kullanilan {
            baslangic = i;
        }
    }

    let mut temiz = Vec::new();
    let mut en_yuksek = -1.0_f64;
    for o in tekil.into_iter().skip(baslangic) {
        if o.kullanilan + 1e-9 >= en_yuksek {
            en_yuksek = o.kullanilan;
            temiz.push(o);
        }
    }
    temiz
}

fn ayrik_deger_sayisi(ornekler: &[OrnekNoktasi]) -> usize {
    let mut degerler: BTreeSet<i64> = BTreeSet::new();
    for o in ornekler {
        degerler.insert((o.kullanilan * 10_000.0).round() as i64);
    }
    degerler.len()
}

fn kisa_ewma_hizi(ornekler: &[OrnekNoktasi], son_zaman: UtcZaman) -> Option<f64> {
    let baslangic = son_zaman.ekle_sn(-KISA_PENCERE_SN);
    // Tohum: kısa penceredeki ortalama hız (ilk aralığa düşen tek bir basamak EWMA'yı
    // şişirmesin). Kısa pencerenin başındaki örnek, pencereye giren ilk aralığın başıdır.
    let ilk_idx = ornekler
        .iter()
        .position(|o| o.zaman >= baslangic)
        .map(|i| i.saturating_sub(1))?;
    let (ilk, son) = (&ornekler[ilk_idx], ornekler.last()?);
    let tohum_dt = son.zaman.fark_sn(ilk.zaman) as f64;
    if tohum_dt <= 0.0 {
        return None;
    }
    let mut ewma: Option<f64> = Some((son.kullanilan - ilk.kullanilan).max(0.0) / tohum_dt);

    for pencere in ornekler.windows(2) {
        let a = &pencere[0];
        let b = &pencere[1];
        if b.zaman < baslangic {
            continue;
        }
        let dt = b.zaman.fark_sn(a.zaman) as f64;
        if dt <= 0.0 {
            continue;
        }
        // Artışsız aralık 0 hız sayılır: tam sayı yüzdeli / kesikli kaynakta %1'lik basamak tek
        // yoklama aralığına yüklenip hızı basamak sıklığı kadar katlamasın.
        let du = (b.kullanilan - a.kullanilan).max(0.0);
        let anlik_hiz = du / dt;
        ewma = Some(match ewma {
            None => anlik_hiz,
            Some(onceki) => {
                let alpha = 1.0 - 0.5_f64.powf(dt / EWMA_YARI_OMUR_SN);
                alpha * anlik_hiz + (1.0 - alpha) * onceki
            }
        });
    }

    ewma
}

fn zaman_ekle_orandan(simdi: UtcZaman, kalan: f64, hiz_sn: f64) -> UtcZaman {
    let saniye = (kalan / hiz_sn).clamp(0.0, MAKS_TAHMIN_UFKU_SN).round() as i64;
    simdi.ekle_sn(saniye)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
    }

    #[test]
    fn ucten_az_ayrik_ornekte_tahmin_yok_doner() {
        let t0 = z("2026-11-15T18:00:00Z");
        let sifirlama = t0.ekle_sn(5 * 3600);
        let mevcut = PencereOlcum {
            uzunluk_dk: 300,
            kullanilan: 0.2,
            kalan: 0.8,
            sifirlama: Some(sifirlama),
        };
        let ornekler = vec![
            OrnekNoktasi {
                zaman: t0,
                kullanilan: 0.1,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: Some(0.1),
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(900),
                kullanilan: 0.2,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: Some(0.1),
            },
        ];

        let sonuc = pencere_tahmini(&ornekler, &mevcut, t0.ekle_sn(900), false);
        assert_eq!(sonuc.durum, TahminDurumu::TahminYok);
        assert!(sonuc.bitis_tahmini.is_none());
    }

    #[test]
    fn on_dakikadan_kisa_aralikta_tahmin_yok_doner() {
        let t0 = z("2026-11-15T18:00:00Z");
        let sifirlama = t0.ekle_sn(5 * 3600);
        let mevcut = PencereOlcum {
            uzunluk_dk: 300,
            kullanilan: 0.3,
            kalan: 0.7,
            sifirlama: Some(sifirlama),
        };
        let ornekler = vec![
            OrnekNoktasi {
                zaman: t0,
                kullanilan: 0.1,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: None,
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(180),
                kullanilan: 0.2,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: None,
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(360),
                kullanilan: 0.3,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: None,
            },
        ];

        let sonuc = pencere_tahmini(&ornekler, &mevcut, t0.ekle_sn(360), false);
        assert_eq!(sonuc.durum, TahminDurumu::TahminYok);
    }

    #[test]
    fn tahmini_kaynak_gurultu_araliginda_tahmin_yok_doner() {
        let t0 = z("2026-11-15T18:00:00Z");
        let sifirlama = t0.ekle_sn(5 * 3600);
        let mevcut = PencereOlcum {
            uzunluk_dk: 300,
            kullanilan: 0.109,
            kalan: 0.891,
            sifirlama: Some(sifirlama),
        };
        let ornekler = vec![
            OrnekNoktasi {
                zaman: t0,
                kullanilan: 0.100,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: true,
                hassasiyet: None,
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(600),
                kullanilan: 0.105,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: true,
                hassasiyet: None,
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(1200),
                kullanilan: 0.109,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: true,
                hassasiyet: None,
            },
        ];

        let sonuc = pencere_tahmini(&ornekler, &mevcut, t0.ekle_sn(1200), true);
        assert_eq!(sonuc.durum, TahminDurumu::TahminYok);
    }

    #[test]
    fn yeterli_ornekte_deneysel_etiketiyle_bitis_saati_ve_koni_hesaplar() {
        let t0 = z("2026-11-15T18:00:00Z");
        let sifirlama = t0.ekle_sn(5 * 3600); // 23:00:00Z
        let simdi = t0.ekle_sn(3600); // 19:00:00Z
        let mevcut = PencereOlcum {
            uzunluk_dk: 300,
            kullanilan: 0.50,
            kalan: 0.50,
            sifirlama: Some(sifirlama),
        };
        // İlk 30 dk yavaş (%10), sonraki 30 dk hızlı (%40) -> toplam 1 saatte %50.
        let ornekler = vec![
            OrnekNoktasi {
                zaman: t0,
                kullanilan: 0.0,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: Some(0.1),
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(1800),
                kullanilan: 0.10,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: Some(0.1),
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(2700),
                kullanilan: 0.30,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: Some(0.1),
            },
            OrnekNoktasi {
                zaman: simdi,
                kullanilan: 0.50,
                sifirlama: Some(sifirlama),
                tahmini_kaynak: false,
                hassasiyet: Some(0.1),
            },
        ];

        let tahmin = pencere_tahmini(&ornekler, &mevcut, simdi, false);
        assert_eq!(tahmin.durum, TahminDurumu::Deneysel);
        assert_eq!(tahmin.etiket.as_deref(), Some("deneysel"));
        assert_eq!(tahmin.sifirlanmadan_biter, Some(true));
        let erken = tahmin.bitis_erken.expect("erken");
        let orta = tahmin.bitis_tahmini.expect("orta");
        let gec = tahmin.bitis_gec.expect("gec");
        assert!(erken <= orta && orta <= gec);
        assert!(orta < sifirlama);
    }

    fn nokta(zaman: UtcZaman, u: f64, s: Option<UtcZaman>, h: Option<f64>) -> OrnekNoktasi {
        OrnekNoktasi {
            zaman,
            kullanilan: u,
            sifirlama: s,
            tahmini_kaynak: false,
            hassasiyet: h,
        }
    }

    #[test]
    fn pencere_oncesi_bosta_ornekler_hizi_dusurmez() {
        // Codex: 3 sa boşta (u=0, sifirlama yok), sonra 2 dk'da bir +%1; 20 dk sonra u=%10.
        let t0 = z("2026-11-15T12:00:00Z");
        let baslama = t0.ekle_sn(3 * 3600);
        let sifirlama = baslama.ekle_sn(5 * 3600);
        let mut ornekler = Vec::new();
        for i in 0..36 {
            ornekler.push(nokta(t0.ekle_sn(i * 300), 0.0, None, None));
        }
        for i in 1..=10 {
            ornekler.push(nokta(
                baslama.ekle_sn(i * 120),
                i as f64 / 100.0,
                Some(sifirlama),
                None,
            ));
        }
        let simdi = baslama.ekle_sn(1200);
        let mevcut = PencereOlcum {
            uzunluk_dk: 300,
            kullanilan: 0.10,
            kalan: 0.90,
            sifirlama: Some(sifirlama),
        };
        let t = pencere_tahmini(&ornekler, &mevcut, simdi, false);
        assert_eq!(t.durum, TahminDurumu::Deneysel);
        let hiz = t.saatlik_hiz.unwrap();
        assert!((hiz - 0.30).abs() < 0.03, "saatlik hız {hiz}");
        let bitis_dk = t.bitis_tahmini.unwrap().fark_sn(simdi) / 60;
        assert!((165..=195).contains(&bitis_dk), "bitiş +{bitis_dk} dk");
    }

    #[test]
    fn tam_sayi_kaynakta_basamak_hizi_katlamaz() {
        // 60 sn yoklama, 5 dk'da bir +%1 (gerçek hız saatte %12), 1 saat sonra u=%22.
        let t0 = z("2026-11-15T12:00:00Z");
        let sifirlama = t0.ekle_sn(6 * 3600);
        let mut ornekler = Vec::new();
        for i in 0..=60 {
            let u = 0.10 + ((i / 5) as f64) / 100.0;
            ornekler.push(nokta(t0.ekle_sn(i * 60), u, Some(sifirlama), Some(1.0)));
        }
        let simdi = t0.ekle_sn(3600);
        let mevcut = PencereOlcum {
            uzunluk_dk: 300,
            kullanilan: 0.22,
            kalan: 0.78,
            sifirlama: Some(sifirlama),
        };
        let t = pencere_tahmini(&ornekler, &mevcut, simdi, false);
        assert_eq!(t.durum, TahminDurumu::Deneysel);
        let bitis_dk = t.bitis_tahmini.unwrap().fark_sn(simdi) / 60;
        // Gerçek bitiş ≈ +390 dk; eski kod +130 dk veriyordu.
        assert!((320..=470).contains(&bitis_dk), "bitiş +{bitis_dk} dk");
        assert_eq!(t.sifirlanmadan_biter, Some(bitis_dk < 300));
    }

    #[test]
    fn baski_orkestra_tabanini_ve_model_bandini_izler() {
        let simdi = z("2026-11-15T12:00:00Z");
        let pencere = |kalan: f64, uzunluk_dk: u64| PencereOlcum {
            uzunluk_dk,
            kullanilan: 1.0 - kalan,
            kalan,
            sifirlama: Some(simdi.ekle_sn(3600)),
        };
        let yok = tahmin_yok("test");
        // Claude 5s: %18 tabanın (%15) üstünde → model bandı (dikkat) korunur; %15 eşitlik kaçın değil.
        assert_eq!(
            baski_hesapla(Some(Baski::Dikkat), &pencere(0.18, 300), &yok, simdi, 0.15),
            Baski::Dikkat
        );
        assert_eq!(
            baski_hesapla(Some(Baski::Dikkat), &pencere(0.15, 300), &yok, simdi, 0.15),
            Baski::Dikkat
        );
        assert_eq!(
            baski_hesapla(Some(Baski::Dikkat), &pencere(0.14, 300), &yok, simdi, 0.15),
            Baski::Kacin
        );
        // Claude hafta %25 < %30 → kaçın.
        assert_eq!(
            baski_hesapla(Some(Baski::Rahat), &pencere(0.25, 10080), &yok, simdi, 0.30),
            Baski::Kacin
        );
        assert_eq!(varsayilan_taban_esigi("claude", "5s"), 0.15);
        assert_eq!(varsayilan_taban_esigi("claude", "hafta"), 0.30);
        assert_eq!(varsayilan_taban_esigi("codex", "hafta"), 0.20);
    }

    #[test]
    fn deneysel_haftalik_tahmin_baskiyi_yukseltmez() {
        let simdi = z("2026-11-15T12:00:00Z");
        let sifirlama = simdi.ekle_sn(92 * 3600);
        let pencere = PencereOlcum {
            uzunluk_dk: 10080,
            kullanilan: 0.35,
            kalan: 0.65,
            sifirlama: Some(sifirlama),
        };
        let mut tahmin = tahmin_yok("x");
        tahmin.durum = TahminDurumu::Deneysel;
        tahmin.sifirlanmadan_biter = Some(true);
        tahmin.bitis_tahmini = Some(simdi.ekle_sn(52 * 3600));
        assert_eq!(
            baski_hesapla(Some(Baski::Rahat), &pencere, &tahmin, simdi, 0.20),
            Baski::Rahat
        );
        // Kısa pencerede deneysel tahmin baskıyı yine yükseltir.
        let kisa = PencereOlcum {
            uzunluk_dk: 300,
            sifirlama: Some(simdi.ekle_sn(4 * 3600)),
            ..pencere.clone()
        };
        tahmin.bitis_tahmini = Some(simdi.ekle_sn(3600));
        assert_eq!(
            baski_hesapla(Some(Baski::Rahat), &kisa, &tahmin, simdi, 0.20),
            Baski::Kacin
        );
    }

    #[test]
    fn havuzda_herhangi_pencere_tabanin_altindaysa_kacin() {
        use std::collections::BTreeMap;
        let simdi = z("2026-11-15T12:00:00Z");
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan: 0.78,
                kalan: 0.22,
                sifirlama: Some(simdi.ekle_sn(3600)),
            },
        );
        pencereler.insert(
            "hafta".to_string(),
            PencereOlcum {
                uzunluk_dk: 10080,
                kullanilan: 0.75,
                kalan: 0.25,
                sifirlama: Some(simdi.ekle_sn(86_400)),
            },
        );
        let mut havuz = HavuzAnlik {
            platform: "claude".into(),
            etiket: "Claude".into(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: None,
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
        assert_eq!(havuz.baglayici_pencere.as_deref(), Some("5s"));
        havuz_tahminlerini_doldur(
            &mut havuz,
            simdi,
            |_| Vec::new(),
            |p| varsayilan_taban_esigi("claude", p),
        );
        assert_eq!(havuz.baski, Some(Baski::Kacin), "hafta %25 < %30");
    }

    #[test]
    fn sifirlama_gecisini_ayiklar_ve_son_donemi_kullanir() {
        let t0 = z("2026-11-15T12:00:00Z");
        let yeni_sifirlama = z("2026-11-15T22:00:00Z");
        let mevcut = PencereOlcum {
            uzunluk_dk: 300,
            kullanilan: 0.30,
            kalan: 0.70,
            sifirlama: Some(yeni_sifirlama),
        };
        let ornekler = vec![
            OrnekNoktasi {
                zaman: t0,
                kullanilan: 0.85,
                sifirlama: Some(yeni_sifirlama),
                tahmini_kaynak: false,
                hassasiyet: None,
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(600),
                kullanilan: 0.10,
                sifirlama: Some(yeni_sifirlama),
                tahmini_kaynak: false,
                hassasiyet: None,
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(1200),
                kullanilan: 0.20,
                sifirlama: Some(yeni_sifirlama),
                tahmini_kaynak: false,
                hassasiyet: None,
            },
            OrnekNoktasi {
                zaman: t0.ekle_sn(1800),
                kullanilan: 0.30,
                sifirlama: Some(yeni_sifirlama),
                tahmini_kaynak: false,
                hassasiyet: None,
            },
        ];

        let simdi = t0.ekle_sn(1800);
        let tahmin = pencere_tahmini(&ornekler, &mevcut, simdi, false);
        assert_eq!(tahmin.durum, TahminDurumu::Deneysel);
        assert_eq!(tahmin.etiket.as_deref(), Some("deneysel"));
        // 0.10 -> 0.30 arası 20 dakikada +0.20 (saatte %60); kalan 0.70 -> ~70 dk sonra
        let orta = tahmin.bitis_tahmini.expect("orta");
        assert!(orta > simdi.ekle_sn(3600) && orta < simdi.ekle_sn(4800));
    }
}
