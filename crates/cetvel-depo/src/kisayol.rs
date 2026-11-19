//! Başlat menüsü kısayolu ve AppUserModelID (AUMID) ataması (`plan.md §9.5`).
//!
//! Paketlenmemiş bir masaüstü uygulamasının Windows toast'ı gösterebilmesi için Başlat
//! menüsünde, `System.AppUserModel.ID` özelliği toast'ın kullandığı AUMID ile aynı olan bir
//! kısayol bulunmalıdır. WScript.Shell bu özelliği atayamadığı için kısayol doğrudan
//! `IShellLinkW` + `IPropertyStore` ile yazılır (gizli PowerShell çağrısı yok; AV dostu).

use std::path::Path;

use crate::DepoHatasi;

/// Cetvel'in Windows bildirim kimliği. Başlat kısayolu ve toast göndericisi aynı değeri kullanır.
pub const CETVEL_AUMID: &str = "Cetvel.Kota";

/// Oluşturulacak kısayolun alanları.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KisayolBilgisi<'a> {
    pub hedef: &'a Path,
    pub argumanlar: &'a str,
    pub aciklama: &'a str,
    pub aumid: &'a str,
}

/// AUMID biçim kuralı (MSDN: en fazla 128 karakter, boşluk yok, nokta ile ayrılmış parçalar).
pub fn aumid_gecerli_mi(aumid: &str) -> bool {
    !aumid.is_empty()
        && aumid.len() <= 128
        && !aumid.starts_with('.')
        && !aumid.ends_with('.')
        && !aumid.contains("..")
        && aumid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

#[cfg(windows)]
fn genis(metin: &str) -> Vec<u16> {
    metin.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn com_hatasi(adim: &str, e: windows::core::Error) -> DepoHatasi {
    DepoHatasi::Guvenlik(format!("kısayol {adim} hatası: {e}"))
}

/// Bu iş parçacığında COM'u başlatır; yalnız kendi başlattığımızı `Drop`'ta kapatır.
#[cfg(windows)]
struct ComKapsami(bool);

#[cfg(windows)]
impl ComKapsami {
    fn ac() -> Self {
        use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
        // SAFETY: Ayrılmış parametre boş; S_OK/S_FALSE başarı, RPC_E_CHANGED_MODE başka kipte
        // zaten başlatılmış demektir (kullanılabilir, kapatılmaz).
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self(hr.is_ok())
    }
}

#[cfg(windows)]
impl Drop for ComKapsami {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: Yalnız bu kapsamın başarıyla başlattığı COM kapatılır.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// Kısayolu (`.lnk`) oluşturur ya da üzerine yazar ve AUMID özelliğini atar.
pub fn kisayol_olustur(kisayol: &Path, bilgi: &KisayolBilgisi<'_>) -> Result<(), DepoHatasi> {
    if !aumid_gecerli_mi(bilgi.aumid) {
        return Err(DepoHatasi::Guvenlik("geçersiz AppUserModelID".to_string()));
    }
    if let Some(ebeveyn) = kisayol.parent() {
        std::fs::create_dir_all(ebeveyn)?;
    }

    #[cfg(windows)]
    {
        use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
        use windows::Win32::System::Com::StructuredStorage::{
            PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0, PropVariantClear,
        };
        use windows::Win32::System::Com::{
            CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemAlloc, IPersistFile,
        };
        use windows::Win32::System::Variant::VT_LPWSTR;
        use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        use windows::core::{Interface, PCWSTR, PWSTR};

        let _com = ComKapsami::ac();
        let hedef = genis(&bilgi.hedef.display().to_string());
        let calisma_dizini = bilgi
            .hedef
            .parent()
            .map(|p| genis(&p.display().to_string()));
        let argumanlar = genis(bilgi.argumanlar);
        let aciklama = genis(bilgi.aciklama);
        let aumid = genis(bilgi.aumid);
        let yol = genis(&kisayol.display().to_string());

        // SAFETY: Tüm dizgeler çağrılar boyunca yaşayan NUL sonlu UTF-16 tamponlarıdır;
        // COM nesneleri `windows` sarmalayıcılarıyla sayılır ve kapsam sonunda bırakılır.
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| com_hatasi("oluşturma", e))?;
            link.SetPath(PCWSTR(hedef.as_ptr()))
                .map_err(|e| com_hatasi("hedef", e))?;
            link.SetArguments(PCWSTR(argumanlar.as_ptr()))
                .map_err(|e| com_hatasi("argüman", e))?;
            link.SetDescription(PCWSTR(aciklama.as_ptr()))
                .map_err(|e| com_hatasi("açıklama", e))?;
            if let Some(dizin) = &calisma_dizini {
                link.SetWorkingDirectory(PCWSTR(dizin.as_ptr()))
                    .map_err(|e| com_hatasi("çalışma dizini", e))?;
            }

            let depo: IPropertyStore = link.cast().map_err(|e| com_hatasi("özellik deposu", e))?;
            // Belgelenen kalıp (InitPropVariantFromString + PropVariantClear): dizge CoTaskMem'e
            // kopyalanır, SetValue kendi kopyasını alır, bizimki PropVariantClear ile bırakılır.
            let bayt = aumid.len() * std::mem::size_of::<u16>();
            let tampon = CoTaskMemAlloc(bayt).cast::<u16>();
            if tampon.is_null() {
                return Err(DepoHatasi::Guvenlik("AUMID belleği ayrılamadı".to_string()));
            }
            std::ptr::copy_nonoverlapping(aumid.as_ptr(), tampon, aumid.len());
            let mut deger = PROPVARIANT {
                Anonymous: PROPVARIANT_0 {
                    Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                        vt: VT_LPWSTR,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: PROPVARIANT_0_0_0 {
                            pwszVal: PWSTR(tampon),
                        },
                    }),
                },
            };
            let ayar = depo.SetValue(&PKEY_AppUserModel_ID, &deger);
            let _ = PropVariantClear(&mut deger);
            ayar.map_err(|e| com_hatasi("AUMID", e))?;
            depo.Commit().map_err(|e| com_hatasi("AUMID kaydı", e))?;

            let dosya: IPersistFile = link.cast().map_err(|e| com_hatasi("dosya", e))?;
            dosya
                .Save(PCWSTR(yol.as_ptr()), true)
                .map_err(|e| com_hatasi("kaydetme", e))?;
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = (kisayol, bilgi);
        Err(DepoHatasi::Guvenlik(
            "kısayol yalnız Windows üzerinde oluşturulur".to_string(),
        ))
    }
}

/// Kısayolun `System.AppUserModel.ID` değerini okur (yoksa `None`).
pub fn kisayol_aumid_oku(kisayol: &Path) -> Result<Option<String>, DepoHatasi> {
    #[cfg(windows)]
    {
        use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
        use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
        use windows::Win32::System::Com::{
            CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ,
        };
        use windows::Win32::System::Variant::VT_LPWSTR;
        use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        use windows::core::{Interface, PCWSTR};

        let _com = ComKapsami::ac();
        let yol = genis(&kisayol.display().to_string());
        // SAFETY: Kısayol salt okunur yüklenir; PROPVARIANT `PropVariantClear` ile bırakılır.
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| com_hatasi("oluşturma", e))?;
            let dosya: IPersistFile = link.cast().map_err(|e| com_hatasi("dosya", e))?;
            dosya
                .Load(PCWSTR(yol.as_ptr()), STGM_READ)
                .map_err(|e| com_hatasi("yükleme", e))?;
            let depo: IPropertyStore = link.cast().map_err(|e| com_hatasi("özellik deposu", e))?;
            let mut deger = depo
                .GetValue(&PKEY_AppUserModel_ID)
                .map_err(|e| com_hatasi("AUMID okuma", e))?;
            let ic = &deger.Anonymous.Anonymous;
            let sonuc = if ic.vt == VT_LPWSTR && !ic.Anonymous.pwszVal.is_null() {
                ic.Anonymous.pwszVal.to_string().ok()
            } else {
                None
            };
            let _ = PropVariantClear(&mut deger);
            Ok(sonuc)
        }
    }

    #[cfg(not(windows))]
    {
        let _ = kisayol;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aumid_bicimi_dogrulanir() {
        assert!(aumid_gecerli_mi(CETVEL_AUMID));
        assert!(!aumid_gecerli_mi(""));
        assert!(!aumid_gecerli_mi("Cetvel Kota"));
        assert!(!aumid_gecerli_mi(".Cetvel"));
        assert!(!aumid_gecerli_mi("Cetvel..Kota"));
        assert!(!aumid_gecerli_mi(&"a".repeat(129)));
    }

    /// Gerçek kısayol yalnız geçici dizine yazılır (Başlat menüsüne dokunulmaz, toast açılmaz).
    #[cfg(windows)]
    #[test]
    fn kisayol_aumid_ile_yazilir_ve_geri_okunur() {
        let d = std::env::temp_dir().join(format!("cetvel-kisayol-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let lnk = d.join("Alt Klasör").join("Cetvel.lnk");
        let hedef = std::env::current_exe().expect("exe");
        // COM tek iş parçacıklı daire ister; test iş parçacığının kipinden bağımsız olsun.
        let lnk_k = lnk.clone();
        std::thread::spawn(move || {
            kisayol_olustur(
                &lnk_k,
                &KisayolBilgisi {
                    hedef: &hedef,
                    argumanlar: "durum",
                    aciklama: "Cetvel Kota Durumu",
                    aumid: CETVEL_AUMID,
                },
            )
            .expect("kısayol");
            assert_eq!(
                kisayol_aumid_oku(&lnk_k).expect("oku").as_deref(),
                Some(CETVEL_AUMID)
            );
            // Üzerine yazma (yeniden kurma) da çalışır.
            kisayol_olustur(
                &lnk_k,
                &KisayolBilgisi {
                    hedef: &hedef,
                    argumanlar: "durum",
                    aciklama: "Cetvel",
                    aumid: CETVEL_AUMID,
                },
            )
            .expect("üzerine yazma");
        })
        .join()
        .expect("iş parçacığı");
        assert!(lnk.is_file());
        assert!(
            kisayol_olustur(
                &lnk,
                &KisayolBilgisi {
                    hedef: Path::new("C:/x.exe"),
                    argumanlar: "",
                    aciklama: "",
                    aumid: "geçersiz kimlik",
                },
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
