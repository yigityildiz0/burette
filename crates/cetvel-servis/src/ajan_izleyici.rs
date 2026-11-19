//! Ajan olay okuyucusunu (`cetvel-ajan`) servis içinde yoklayan görev.
//!
//! Her turda okuyucu `spawn_blocking` içinde çalışır (dosya/SQLite G/Ç'si async döngüyü tutmaz).
//! İçerik değişince `sira` artar ve `%LOCALAPPDATA%\Cetvel\ajanlar.json` atomik yazılır; içerik
//! aynıysa dosya yalnız kalp atışı aralığında tazelenir. Uykudan dönüşte bir sonraki tur her şeyi
//! zamana göre yeniden değerlendirir (bitiş/askı kararları olay zamanından hesaplanır).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cetvel_ajan::{AjanAnlik, AjanOkuyucu, Kurallar, Yapilandirma};
use cetvel_depo::{AjanAyari, CetvelYollar, atomik_yaz_json};
use cetvel_model::UtcZaman;
use tokio::sync::watch;

use crate::durum::PaylasilanServisDurumu;

/// İçerik değişmese de `ajanlar.json` en geç bu aralıkla yeniden yazılır (tüketici tazeliği görür).
const KALP_ATISI_SN: i64 = 15;

fn yol_cevir(metin: &str) -> PathBuf {
    PathBuf::from(metin.trim())
}

/// Ayarlardan okuyucu yapılandırması üretir; boş yol alanları varsayılan konumlara çözülür.
pub fn yapilandirma_olustur(ayar: &AjanAyari) -> Yapilandirma {
    let kurallar = Kurallar {
        bitis_bekleme_sn: ayar.bitis_bekleme_sn.clamp(5, 3600) as i64,
        askida_sn: ayar.askida_sn.clamp(30, 86_400) as i64,
        pencere_sn: ayar.pencere_sn.clamp(300, 7 * 86_400) as i64,
    };
    if !ayar.etkin {
        return Yapilandirma {
            kurallar,
            ..Default::default()
        };
    }
    let ev = cetvel_ajan::ev_dizini();

    let codex_oturum_koku = ayar.codex.then(|| {
        ayar.codex_dizini
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(yol_cevir)
            .or_else(|| ev.as_deref().map(cetvel_ajan::varsayilan_codex_dizini))
            .map(|d| d.join("sessions"))
    });

    let agy_ozet_db = if ayar.antigravity {
        let ortam: Vec<PathBuf> = std::env::var("CETVEL_AGY_OZET_DB")
            .ok()
            .map(|v| {
                v.split(';')
                    .filter(|p| !p.trim().is_empty())
                    .map(yol_cevir)
                    .collect()
            })
            .unwrap_or_default();
        if !ortam.is_empty() {
            ortam
        } else if !ayar.agy_ozet_db.is_empty() {
            ayar.agy_ozet_db.iter().map(|s| yol_cevir(s)).collect()
        } else {
            ev.as_deref()
                .map(cetvel_ajan::varsayilan_agy_ozet_db)
                .unwrap_or_default()
        }
    } else {
        Vec::new()
    };

    Yapilandirma {
        codex_oturum_koku: codex_oturum_koku.flatten(),
        agy_ozet_db,
        kurallar,
    }
}

/// Tek tur: okuyucuyu çalıştırır, durumu günceller, gerekiyorsa dosyayı yazar.
/// `son_yazim` dosyanın en son yazıldığı an; yazılırsa güncellenir.
pub async fn tur_calistir(
    okuyucu: AjanOkuyucu,
    durum: &PaylasilanServisDurumu,
    yollar: Option<&CetvelYollar>,
    son_yazim: &mut Option<UtcZaman>,
) -> AjanOkuyucu {
    let simdi = UtcZaman::simdi();
    let sonuc = tokio::task::spawn_blocking(move || {
        let mut o = okuyucu;
        let anlik = o.tara(simdi);
        (o, anlik)
    })
    .await;
    let (okuyucu, anlik) = match sonuc {
        Ok(x) => x,
        Err(_) => {
            tracing::warn!("ajan okuyucusu bu turda tamamlanamadı; yeniden kuruluyor");
            let ayar = durum.ayarlar_kopya().ajan.clone();
            return AjanOkuyucu::yeni(yapilandirma_olustur(&ayar));
        }
    };
    yayinla(durum, yollar, anlik, son_yazim);
    okuyucu
}

fn yayinla(
    durum: &PaylasilanServisDurumu,
    yollar: Option<&CetvelYollar>,
    anlik: AjanAnlik,
    son_yazim: &mut Option<UtcZaman>,
) {
    let zaman = anlik.zaman;
    let (degisti, guncel) = durum.ajanlar_guncelle(anlik);
    let kalp = son_yazim.is_none_or(|s| zaman.fark_sn(s) >= KALP_ATISI_SN);
    if let Some(y) = yollar
        && (degisti || kalp)
    {
        match atomik_yaz_json(&y.ajanlar_dosyasi(), &*guncel) {
            Ok(()) => *son_yazim = Some(zaman),
            Err(_) => tracing::warn!("ajanlar.json yazılamadı; sonraki turda yeniden denenecek"),
        }
    }
}

/// Servis kapanana dek ajan okuyucusunu yoklar.
pub async fn ajan_izleyicisi_calistir(
    durum: Arc<PaylasilanServisDurumu>,
    yollar: Option<CetvelYollar>,
    mut kapanis: watch::Receiver<bool>,
) {
    let mut ayar = durum.ayarlar_kopya().ajan.clone();
    let mut okuyucu = AjanOkuyucu::yeni(yapilandirma_olustur(&ayar));
    let mut son_yazim: Option<UtcZaman> = None;
    loop {
        if *kapanis.borrow() {
            break;
        }
        let guncel = durum.ayarlar_kopya().ajan.clone();
        if guncel != ayar {
            ayar = guncel;
            okuyucu = AjanOkuyucu::yeni(yapilandirma_olustur(&ayar));
        }
        okuyucu = tur_calistir(okuyucu, &durum, yollar.as_ref(), &mut son_yazim).await;

        let bekle = Duration::from_millis(ayar.yoklama_ms.clamp(500, 60_000));
        tokio::select! {
            _ = tokio::time::sleep(bekle) => {}
            _ = kapanis.changed() => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kapali_ayar_hic_kaynak_acmaz_ve_sinirlar_kirpilir() {
        let ayar = AjanAyari {
            etkin: false,
            ..Default::default()
        };
        let y = yapilandirma_olustur(&ayar);
        assert!(y.codex_oturum_koku.is_none() && y.agy_ozet_db.is_empty());

        let ayar = AjanAyari {
            codex: false,
            antigravity: true,
            agy_ozet_db: vec!["X:\\ayar\\ozet.db".to_string()],
            bitis_bekleme_sn: 0,
            pencere_sn: 1,
            ..Default::default()
        };
        let y = yapilandirma_olustur(&ayar);
        assert!(y.codex_oturum_koku.is_none());
        if std::env::var_os("CETVEL_AGY_OZET_DB").is_none() {
            assert_eq!(y.agy_ozet_db, vec![PathBuf::from("X:\\ayar\\ozet.db")]);
        }
        assert_eq!(y.kurallar.bitis_bekleme_sn, 5);
        assert_eq!(y.kurallar.pencere_sn, 300);
    }

    #[tokio::test]
    async fn tur_dosyayi_atomik_yazar_icerik_degismezse_sira_artmaz() {
        let kok = std::env::temp_dir().join(format!("cetvel-ajan-izleyici-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let yollar = CetvelYollar::ozel_koklerle(kok.join("local"), kok.join("roaming"));
        let durum = PaylasilanServisDurumu::yeni(
            cetvel_depo::Ayarlar::default(),
            Some(yollar.clone()),
            "jeton",
            r"\\.\pipe\cetvel-test-ajan",
            None,
        );
        let okuyucu = AjanOkuyucu::yeni(Yapilandirma {
            codex_oturum_koku: Some(kok.join("yok-sessions")),
            ..Default::default()
        });
        let mut son_yazim = None;
        let okuyucu = tur_calistir(okuyucu, &durum, Some(&yollar), &mut son_yazim).await;
        let ilk = durum.ajanlar_kopya();
        assert_eq!(ilk.sira, 1);
        assert!(ilk.ajanlar.is_empty());
        assert_eq!(
            ilk.kaynaklar["codex"].durum,
            cetvel_ajan::KaynakDurumu::Bulunamadi
        );
        let dosya: AjanAnlik =
            serde_json::from_str(&std::fs::read_to_string(yollar.ajanlar_dosyasi()).unwrap())
                .unwrap();
        assert_eq!(dosya.sira, 1);
        assert_eq!(dosya.kaynaklar.len(), ilk.kaynaklar.len());
        assert!(son_yazim.is_some());

        let _ = tur_calistir(okuyucu, &durum, Some(&yollar), &mut son_yazim).await;
        assert_eq!(durum.ajanlar_kopya().sira, 1, "içerik aynı: sira artmaz");
        let _ = std::fs::remove_dir_all(&kok);
    }

    #[tokio::test]
    async fn yeniden_baslatmada_ajan_sirasi_geri_gitmez() {
        let durum = PaylasilanServisDurumu::yeni(
            cetvel_depo::Ayarlar::default(),
            None,
            "jeton",
            r"\\.\pipe\cetvel-test-ajan-sira",
            None,
        );
        durum.ajan_sirasini_tohumla(14);
        // Boş başlangıç anlığı önceki "ajan-14" ile karışmaz.
        assert_eq!(durum.ajanlar_kopya().sira, 15);
        let okuyucu = AjanOkuyucu::yeni(Yapilandirma::default());
        let mut son_yazim = None;
        let _ = tur_calistir(okuyucu, &durum, None, &mut son_yazim).await;
        assert!(durum.ajanlar_kopya().sira >= 15, "sira geri gitmemeli");
        // Daha küçük ya da sıfır tohum mevcut sirayı düşürmez.
        durum.ajan_sirasini_tohumla(0);
        durum.ajan_sirasini_tohumla(3);
        assert!(durum.ajanlar_kopya().sira >= 15);
    }
}
