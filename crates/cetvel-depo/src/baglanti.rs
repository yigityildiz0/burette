use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::DepoHatasi;
use crate::atomik::{atomik_yaz, atomik_yaz_json, kullanici_dacl_uygula};
use crate::yollar::CetvelYollar;

/// `%LOCALAPPDATA%\Cetvel\baglanti.json` içeriği.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaglantiBilgisi {
    pub port: u16,
    #[serde(default)]
    pub adresler: Vec<String>,
    pub jeton: String,
    pub pipe: String,
    pub surum: String,
    /// Servis süreç kimliği (kaldırmada zaman aşımında süreci sonlandırmak için; eski sürümde yok).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

impl fmt::Debug for BaglantiBilgisi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BaglantiBilgisi")
            .field("port", &self.port)
            .field("adresler", &self.adresler)
            .field("jeton", &"[MASKELI]")
            .field("pipe", &self.pipe)
            .field("surum", &self.surum)
            .field("pid", &self.pid)
            .finish()
    }
}

/// İşletim sistemi CSPRNG'sinden 256 bit (64 hex karakter) yerel erişim jetonu üretir.
pub fn csprng_jeton_uret() -> Result<String, DepoHatasi> {
    let mut baytlar = [0u8; 32];
    getrandom::fill(&mut baytlar)
        .map_err(|e| DepoHatasi::Rastgele(format!("CSPRNG hatası: {e}")))?;
    let mut hex = String::with_capacity(64);
    for b in baytlar {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{b:02x}");
    }
    Ok(hex)
}

/// Jeton dosyasından geçerli bir jeton okur; yoksa veya bozuksa yeni 256 bit jeton üretir.
pub fn jeton_yukle_veya_uret(yollar: &CetvelYollar) -> Result<String, DepoHatasi> {
    let jeton_yol = yollar.jeton_dosyasi();
    if jeton_yol.exists()
        && let Ok(mevcut) = std::fs::read_to_string(&jeton_yol)
    {
        let temiz = mevcut.trim().to_string();
        if temiz.len() >= 32
            && temiz
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Ok(temiz);
        }
    }

    let yeni = csprng_jeton_uret()?;
    jeton_dosyasi_yaz(&jeton_yol, &yeni)?;
    Ok(yeni)
}

/// Jetonu atomik olarak yazar ve kullanıcı DACL'ini uygular.
pub fn jeton_dosyasi_yaz(yol: &Path, jeton: &str) -> Result<(), DepoHatasi> {
    if let Some(ebeveyn) = yol.parent() {
        std::fs::create_dir_all(ebeveyn)?;
        let _ = kullanici_dacl_uygula(ebeveyn);
    }
    atomik_yaz(yol, format!("{jeton}\n").as_bytes())?;
    let _ = kullanici_dacl_uygula(yol);
    Ok(())
}

/// `baglanti.json` ve `jeton` dosyalarını atomik olarak ve kullanıcı-yalnız DACL ile yazar.
pub fn baglanti_bilgisi_yaz(
    yollar: &CetvelYollar,
    bilgi: &BaglantiBilgisi,
) -> Result<(), DepoHatasi> {
    std::fs::create_dir_all(&yollar.local_kok)?;
    let _ = kullanici_dacl_uygula(&yollar.local_kok);

    jeton_dosyasi_yaz(&yollar.jeton_dosyasi(), &bilgi.jeton)?;
    let baglanti_yol = yollar.baglanti_dosyasi();
    atomik_yaz_json(&baglanti_yol, bilgi)?;
    let _ = kullanici_dacl_uygula(&baglanti_yol);
    Ok(())
}

/// Kayıtlı `baglanti.json` dosyasını okur.
pub fn baglanti_bilgisi_oku(yollar: &CetvelYollar) -> Result<BaglantiBilgisi, DepoHatasi> {
    let icerik = std::fs::read_to_string(yollar.baglanti_dosyasi())?;
    let bilgi: BaglantiBilgisi = serde_json::from_str(&icerik)?;
    Ok(bilgi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jeton_256_bit_ve_debug_maskeli() {
        let j = csprng_jeton_uret().expect("jeton");
        assert_eq!(j.len(), 64);
        assert!(j.bytes().all(|b| b.is_ascii_hexdigit()));

        let bilgi = BaglantiBilgisi {
            port: 43117,
            adresler: vec!["127.0.0.1:43117".into()],
            jeton: j.clone(),
            pipe: r"\\.\pipe\cetvel".into(),
            surum: "0.1.0".into(),
            pid: None,
        };
        let dbg = format!("{bilgi:?}");
        assert!(!dbg.contains(&j));
        assert!(dbg.contains("MASKELI"));
    }

    #[test]
    fn baglanti_bilgisi_yazilir_ve_geri_okunur() {
        let kok = std::env::temp_dir().join(format!("cetvel-baglanti-test-{}", std::process::id()));
        let yollar = CetvelYollar::ozel_koklerle(kok.join("local"), kok.join("roaming"));
        let jeton = jeton_yukle_veya_uret(&yollar).expect("jeton");
        let bilgi = BaglantiBilgisi {
            port: 43117,
            adresler: vec!["127.0.0.1:43117".into(), "[::1]:43117".into()],
            jeton: jeton.clone(),
            pipe: r"\\.\pipe\cetvel-S-1-5-21-1".into(),
            surum: "0.1.0".into(),
            pid: Some(4242),
        };
        baglanti_bilgisi_yaz(&yollar, &bilgi).expect("yaz");
        let okunan = baglanti_bilgisi_oku(&yollar).expect("oku");
        assert_eq!(okunan, bilgi);
        assert_eq!(
            std::fs::read_to_string(yollar.jeton_dosyasi())
                .expect("jeton oku")
                .trim(),
            jeton
        );
        let _ = std::fs::remove_dir_all(&kok);
    }

    #[test]
    fn eski_surum_baglanti_json_pid_olmadan_okunur() {
        let eski = r#"{"port":43117,"jeton":"j","pipe":"\\\\.\\pipe\\cetvel","surum":"0.0.9"}"#;
        let b: BaglantiBilgisi = serde_json::from_str(eski).expect("eski biçim");
        assert_eq!(b.pid, None);
        assert_eq!(b.pipe, r"\\.\pipe\cetvel");
        assert!(b.adresler.is_empty());
        // pid yoksa serileştirmede alan yazılmaz (eski okuyucular etkilenmez).
        assert!(!serde_json::to_string(&b).unwrap().contains("pid"));
    }
}
