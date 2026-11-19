use std::fmt;

/// Bellekteki API anahtarını loglara ve hata çıktılarına sızdırmadan tutan sarmalayıcı.
#[derive(Clone, PartialEq, Eq)]
pub struct GizliDeger(String);

impl GizliDeger {
    /// Boşluklardan arındırılmış gizli değer oluşturur; boşsa `None` döner.
    pub fn yeni(deger: impl Into<String>) -> Option<Self> {
        let temiz = deger.into().trim().to_string();
        if temiz.is_empty() {
            None
        } else {
            Some(Self(temiz))
        }
    }

    /// Yalnız yetkili bellek-içi kullanım (ör. HTTP `Authorization` başlığı) için düz metin erişimi sağlar.
    pub fn acik_metin(&self) -> &str {
        &self.0
    }

    /// Log ve teşhis çıktılarında kullanılacak sabit maskeli gösterim.
    pub fn maskeli_ozet(&self) -> &'static str {
        "[MASKELI]"
    }
}

impl fmt::Debug for GizliDeger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GizliDeger([MASKELI])")
    }
}

impl fmt::Display for GizliDeger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[MASKELI]")
    }
}

impl Drop for GizliDeger {
    fn drop(&mut self) {
        // SAFETY: `self.0` geçerli UTF-8 bayt tamponuna sahiptir; sıfırlama Drop sonunda bellekten silinir.
        unsafe {
            let bytes = self.0.as_bytes_mut();
            for b in bytes {
                std::ptr::write_volatile(b, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gizli_deger_debug_ve_display_maskelidir() {
        let gizli = GizliDeger::yeni("gizli-ornek-anahtar-12345").expect("gizli");
        let dbg = format!("{gizli:?}");
        let disp = format!("{gizli}");
        assert!(!dbg.contains("ornek"));
        assert!(!disp.contains("ornek"));
        assert!(dbg.contains("MASKELI"));
        assert_eq!(disp, "[MASKELI]");
    }

    #[test]
    fn bos_gizli_deger_reddedilir() {
        assert!(GizliDeger::yeni("   \t\n ").is_none());
    }
}
