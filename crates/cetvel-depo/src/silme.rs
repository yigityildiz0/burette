//! Kaldırmada veri klasörlerini kilitli dosyaya rağmen düşmeden silme.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Dayanıklı silme sonucu.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SilmeSonucu {
    /// Tüm denemelerden sonra hâlâ silinemeyen dosya/dizinler (ör. açık `gecmis.sqlite`).
    pub silinemeyenler: Vec<PathBuf>,
}

impl SilmeSonucu {
    pub fn tamam_mi(&self) -> bool {
        self.silinemeyenler.is_empty()
    }
}

/// Dizini siler; kilitli dosya varsa (servis kapanırken SQLite WAL, açık masaüstü) `deneme`
/// kez `bekleme` aralığıyla yeniden dener. Her turda silinebilen her şeyi siler; hata yerine
/// silinemeyenlerin listesini döner (kaldırma diğer adımlarla sürer). Yol yoksa boş sonuç.
pub fn dizini_dayanikli_sil(yol: &Path, deneme: u32, bekleme: Duration) -> SilmeSonucu {
    let mut kalanlar = Vec::new();
    for tur in 0..deneme.max(1) {
        if !yol.exists() {
            return SilmeSonucu::default();
        }
        if std::fs::remove_dir_all(yol).is_ok() {
            return SilmeSonucu::default();
        }
        kalanlar.clear();
        agaci_sil(yol, &mut kalanlar);
        if !yol.exists() {
            return SilmeSonucu::default();
        }
        if tur + 1 < deneme.max(1) {
            std::thread::sleep(bekleme);
        }
    }
    if kalanlar.is_empty() && yol.exists() {
        kalanlar.push(yol.to_path_buf());
    }
    SilmeSonucu {
        silinemeyenler: kalanlar,
    }
}

/// Ağacı alttan yukarı siler; silinemeyen dosyaları `kalanlar`a ekler.
fn agaci_sil(yol: &Path, kalanlar: &mut Vec<PathBuf>) {
    let Ok(meta) = std::fs::symlink_metadata(yol) else {
        return;
    };
    if meta.is_dir() && !meta.file_type().is_symlink() {
        if let Ok(girdiler) = std::fs::read_dir(yol) {
            for girdi in girdiler.flatten() {
                agaci_sil(&girdi.path(), kalanlar);
            }
        }
        // İçinde kalan dosya varsa bu başarısız olur; dizin kendisi listelenmez.
        let _ = std::fs::remove_dir(yol);
    } else if std::fs::remove_file(yol).is_err() {
        // Salt okunur bayrak varsa kaldırıp bir kez daha dene.
        let mut izin = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        izin.set_readonly(false);
        let _ = std::fs::set_permissions(yol, izin);
        if std::fs::remove_file(yol).is_err() {
            kalanlar.push(yol.to_path_buf());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gecici(ad: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cetvel-silme-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("alt")).unwrap();
        std::fs::write(d.join("anlik.json"), "{}").unwrap();
        std::fs::write(d.join("alt").join("gunluk.log"), "x").unwrap();
        d
    }

    #[test]
    fn olmayan_dizin_ve_normal_dizin_silinir() {
        let yok = std::env::temp_dir().join("cetvel-silme-hic-yok-xyz");
        assert!(dizini_dayanikli_sil(&yok, 2, Duration::from_millis(1)).tamam_mi());
        let d = gecici("normal");
        assert!(dizini_dayanikli_sil(&d, 2, Duration::from_millis(1)).tamam_mi());
        assert!(!d.exists());
    }

    #[cfg(windows)]
    #[test]
    fn kilitli_dosya_kalanlara_yazilir_digerleri_silinir_kilit_kalkinca_tamamlanir() {
        use std::os::windows::fs::OpenOptionsExt;
        let d = gecici("kilitli");
        let db = d.join("gecmis.sqlite");
        std::fs::write(&db, "veri").unwrap();
        // Paylaşımsız açık tutamaç: SQLite/servis kilidinin benzeri (silme reddedilir).
        let kilit = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&db)
            .unwrap();

        let sonuc = dizini_dayanikli_sil(&d, 3, Duration::from_millis(5));
        assert_eq!(sonuc.silinemeyenler, vec![db.clone()]);
        assert!(
            !d.join("anlik.json").exists(),
            "kilitsiz dosyalar silinmeli"
        );
        assert!(!d.join("alt").exists());

        drop(kilit);
        assert!(dizini_dayanikli_sil(&d, 3, Duration::from_millis(5)).tamam_mi());
        assert!(!d.exists());
    }
}
