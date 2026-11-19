use std::path::{Path, PathBuf};

use crate::DepoHatasi;

/// Cetvel uygulamasının yerel ve gezici Windows dizin yolları.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CetvelYollar {
    /// `%LOCALAPPDATA%\Cetvel`
    pub local_kok: PathBuf,
    /// `%APPDATA%\Cetvel`
    pub roaming_kok: PathBuf,
    /// `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Cetvel.lnk`
    pub baslat_kisayolu: PathBuf,
}

impl CetvelYollar {
    /// Ortam değişkenlerinden kullanıcı yollarını çözer (kodda sabit kullanıcı yolu yoktur).
    pub fn ortamdan() -> Result<Self, DepoHatasi> {
        let local_base = env_path_or_userprofile_sub("LOCALAPPDATA", &["AppData", "Local"])
            .ok_or_else(|| DepoHatasi::OrtamEksik("LOCALAPPDATA".to_string()))?;
        let roaming_base = env_path_or_userprofile_sub("APPDATA", &["AppData", "Roaming"])
            .ok_or_else(|| DepoHatasi::OrtamEksik("APPDATA".to_string()))?;

        let baslat_kisayolu = roaming_base
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Cetvel.lnk");

        Ok(Self {
            local_kok: local_base.join("Cetvel"),
            roaming_kok: roaming_base.join("Cetvel"),
            baslat_kisayolu,
        })
    }

    /// Testler için özel kök dizinlerle yol kümesi oluşturur.
    pub fn ozel_koklerle(local_kok: impl AsRef<Path>, roaming_kok: impl AsRef<Path>) -> Self {
        let roaming = roaming_kok.as_ref().to_path_buf();
        Self {
            local_kok: local_kok.as_ref().to_path_buf(),
            baslat_kisayolu: roaming.join("Cetvel.lnk"),
            roaming_kok: roaming,
        }
    }

    pub fn anlik_dosyasi(&self) -> PathBuf {
        self.local_kok.join("anlik.json")
    }

    /// Claude dışı ajan ağacı (`cetvel-ajan`); Pixel Office gibi modlar `$.fs` ile okur.
    pub fn ajanlar_dosyasi(&self) -> PathBuf {
        self.local_kok.join("ajanlar.json")
    }

    pub fn orkestra_uyum_dosyasi(&self) -> PathBuf {
        self.local_kok.join("orkestra-uyum.json")
    }

    pub fn baglanti_dosyasi(&self) -> PathBuf {
        self.local_kok.join("baglanti.json")
    }

    pub fn jeton_dosyasi(&self) -> PathBuf {
        self.local_kok.join("jeton")
    }

    pub fn gecmis_veritabani(&self) -> PathBuf {
        self.local_kok.join("gecmis.sqlite")
    }

    pub fn gunluk_dizini(&self) -> PathBuf {
        self.local_kok.join("gunluk")
    }

    pub fn ayarlar_dosyasi(&self) -> PathBuf {
        self.roaming_kok.join("ayarlar.toml")
    }
}

fn env_path_or_userprofile_sub(env_adi: &str, alt: &[&str]) -> Option<PathBuf> {
    if let Some(val) = std::env::var_os(env_adi)
        && !val.is_empty()
    {
        return Some(PathBuf::from(val));
    }
    let userprofile = std::env::var_os("USERPROFILE")?;
    if userprofile.is_empty() {
        return None;
    }
    let mut path = PathBuf::from(userprofile);
    for parca in alt {
        path.push(parca);
    }
    Some(path)
}
