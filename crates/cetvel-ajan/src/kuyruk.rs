//! Kilitlemeden artımlı JSONL okuma (tail). Windows'ta `std::fs::File::open` okuma/yazma/silme
//! paylaşımına açık açar; yazan süreç engellenmez. Yarım (satır sonu gelmemiş) satır tamponda bekler.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Tek turda en çok okunacak bayt (bellek sınırı; kalan sonraki turda okunur).
const TUR_AZAMI_BAYT: u64 = 32 * 1024 * 1024;
/// İlk okumada bu boyuttan büyük dosyanın yalnız ilk satırı ve sonu okunur (daha küçük dosya
/// turda en çok [`TUR_AZAMI_BAYT`] olmak üzere birkaç turda baştan sona okunur).
const ILK_TAM_OKUMA_SINIRI: u64 = 256 * 1024 * 1024;
/// Büyük dosyada sondan okunacak bölüm.
const ILK_SON_BOLUM: u64 = 32 * 1024 * 1024;
/// Büyük dosyada ilk satır için okunacak en çok bayt.
const ILK_SATIR_AZAMI: u64 = 4 * 1024 * 1024;

/// Bir okuma turunun sonucu.
#[derive(Debug, Default)]
pub struct KuyrukOkuma {
    pub satirlar: Vec<String>,
    /// Dosya küçüldü ya da değiştirildi; çağıran durumu sıfırlamalı.
    pub sifirlandi: bool,
}

/// Tek dosya için artımlı okuyucu.
#[derive(Debug)]
pub struct JsonlKuyruk {
    yol: PathBuf,
    ofset: u64,
    artik: Vec<u8>,
    /// Büyük dosyada ortadan başlarken ilk yarım satır atılır.
    ilk_parcayi_at: bool,
    /// Son okumada görülen dosya boyu.
    son_boy: u64,
}

impl JsonlKuyruk {
    pub fn yeni(yol: impl Into<PathBuf>) -> Self {
        Self {
            yol: yol.into(),
            ofset: 0,
            artik: Vec::new(),
            ilk_parcayi_at: false,
            son_boy: 0,
        }
    }

    /// Son okumada tur sınırı yüzünden okunmamış bayt kaldı mı.
    pub fn bekleyen_var(&self) -> bool {
        self.ofset < self.son_boy
    }

    pub fn yol(&self) -> &Path {
        &self.yol
    }

    /// Yeni tamamlanmış satırları döndürür.
    pub fn oku(&mut self) -> io::Result<KuyrukOkuma> {
        let mut sonuc = KuyrukOkuma::default();
        self.oku_isle(|satir| match satir {
            Some(satir) => sonuc.satirlar.push(satir.to_string()),
            None => sonuc.sifirlandi = true,
        })?;
        Ok(sonuc)
    }

    /// Her satırı kopyalamadan hemen işler; `None` dosyanın sıfırlandığını bildirir.
    /// Bellekte tüm tur yerine yalnız tek satır ve 64 KiB G/Ç tamponu tutulur.
    pub fn oku_isle(&mut self, mut isle: impl FnMut(Option<&str>)) -> io::Result<()> {
        let mut dosya = File::open(&self.yol)?;
        let boy = dosya.metadata()?.len();
        self.son_boy = boy;

        if boy < self.ofset {
            self.ofset = 0;
            self.artik = Vec::new();
            self.ilk_parcayi_at = false;
            isle(None);
        }
        if boy == self.ofset {
            return Ok(());
        }

        if self.ofset == 0 && boy > ILK_TAM_OKUMA_SINIRI {
            // İlk satır (oturum başı) + son bölüm.
            let mut bas = Vec::new();
            BufReader::with_capacity(64 * 1024, (&mut dosya).take(ILK_SATIR_AZAMI))
                .read_until(b'\n', &mut bas)?;
            if let Some(son) = bas.iter().position(|b| *b == b'\n') {
                isle(Some(&satir_metni(&bas[..son])));
            }
            self.ofset = boy - ILK_SON_BOLUM;
            self.ilk_parcayi_at = true;
        }

        dosya.seek(SeekFrom::Start(self.ofset))?;
        let okunacak = (boy - self.ofset).min(TUR_AZAMI_BAYT);
        let mut okuyucu = BufReader::with_capacity(64 * 1024, dosya.take(okunacak));
        loop {
            let adet = okuyucu.read_until(b'\n', &mut self.artik)?;
            self.ofset += adet as u64;
            if adet == 0 || !self.artik.ends_with(b"\n") {
                break;
            }
            let son = self.artik.len() - 1;
            if self.ilk_parcayi_at {
                self.ilk_parcayi_at = false;
            } else {
                let s = satir_metni(&self.artik[..son]);
                if !s.trim().is_empty() {
                    isle(Some(&s));
                }
            }
            self.artik.clear();
            // Tek büyük satır yüzünden her izlenen dosya MiB'larca kapasite tutmasın.
            if self.artik.capacity() > 64 * 1024 {
                self.artik = Vec::new();
            }
        }
        Ok(())
    }
}

fn satir_metni(bayt: &[u8]) -> std::borrow::Cow<'_, str> {
    let b = bayt.strip_suffix(b"\r").unwrap_or(bayt);
    String::from_utf8_lossy(b)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn buyuk_jsonl_turlar_arasinda_satir_kaybetmez_kapasite_tutmaz() {
        let kok = std::env::temp_dir().join(format!("cetvel-ajan-akis-{}", std::process::id()));
        std::fs::create_dir_all(&kok).unwrap();
        let yol = kok.join("akis.jsonl");
        let satir = "x".repeat(8190) + "\n";
        let adet = 4200;
        let mut dosya = std::io::BufWriter::new(File::create(&yol).unwrap());
        for _ in 0..adet {
            dosya.write_all(satir.as_bytes()).unwrap();
        }
        dosya.flush().unwrap();
        drop(dosya);
        let mut kuyruk = JsonlKuyruk::yeni(&yol);
        let mut sayi = 0;
        loop {
            kuyruk
                .oku_isle(|s| {
                    let s = s.expect("dosya sıfırlanmadı");
                    assert_eq!(s.len(), 8190);
                    sayi += 1;
                })
                .unwrap();
            if !kuyruk.bekleyen_var() {
                break;
            }
        }
        assert_eq!(sayi, adet);
        assert!(kuyruk.artik.is_empty());
        assert!(kuyruk.artik.capacity() <= 64 * 1024);
        std::fs::remove_dir_all(kok).unwrap();
    }

    #[test]
    fn yarim_satir_bekler_kesilen_dosya_sifirlanir() {
        let kok = std::env::temp_dir().join(format!("cetvel-ajan-kuyruk-{}", std::process::id()));
        std::fs::create_dir_all(&kok).unwrap();
        let yol = kok.join("a.jsonl");
        std::fs::write(&yol, b"{\"a\":1}\n{\"b\":").unwrap();
        let mut k = JsonlKuyruk::yeni(&yol);
        let o = k.oku().unwrap();
        assert_eq!(o.satirlar, vec!["{\"a\":1}".to_string()]);

        // Yazan süreç dosyayı açık tutarken de okunabilir (paylaşım).
        let mut yazan = std::fs::OpenOptions::new().append(true).open(&yol).unwrap();
        yazan.write_all(b"2}\r\n").unwrap();
        let o = k.oku().unwrap();
        assert_eq!(o.satirlar, vec!["{\"b\":2}".to_string()]);
        assert!(k.oku().unwrap().satirlar.is_empty());
        drop(yazan);

        std::fs::write(&yol, b"{\"c\":3}\n").unwrap();
        let o = k.oku().unwrap();
        assert!(o.sifirlandi);
        assert_eq!(o.satirlar, vec!["{\"c\":3}".to_string()]);
        let _ = std::fs::remove_dir_all(&kok);
    }
}
