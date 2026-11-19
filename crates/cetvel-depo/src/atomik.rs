use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::DepoHatasi;

static GECICI_SAYAC: AtomicU64 = AtomicU64::new(1);

/// Hedef dosyayı aynı dizinde geçici dosya, `FlushFileBuffers` (`sync_all`) ve
/// atomik yeniden adlandırma (`MoveFileExW` `REPLACE_EXISTING | WRITE_THROUGH`) ile yazar.
pub fn atomik_yaz(hedef: &Path, icerik: &[u8]) -> Result<(), DepoHatasi> {
    let ebeveyn = hedef.parent().ok_or_else(|| {
        DepoHatasi::GecersizYol(format!(
            "hedef dosyanın ebeveyn dizini yok: {}",
            hedef.display()
        ))
    })?;
    std::fs::create_dir_all(ebeveyn)?;

    let gecici = gecici_yol_uret(hedef, ebeveyn);
    let yazim_sonucu = (|| -> Result<(), DepoHatasi> {
        {
            let mut dosya = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&gecici)?;
            dosya.write_all(icerik)?;
            dosya.sync_all()?;
        }
        atomik_yeniden_adlandir(&gecici, hedef)?;
        Ok(())
    })();

    if yazim_sonucu.is_err() {
        let _ = std::fs::remove_file(&gecici);
    }
    yazim_sonucu
}

/// JSON değerini biçimlendirilmiş şekilde atomik olarak yazar.
pub fn atomik_yaz_json<T: Serialize>(hedef: &Path, deger: &T) -> Result<(), DepoHatasi> {
    let mut bayt = serde_json::to_vec_pretty(deger)?;
    bayt.push(b'\n');
    atomik_yaz(hedef, &bayt)
}

fn gecici_yol_uret(hedef: &Path, ebeveyn: &Path) -> PathBuf {
    let dosya_adi = hedef
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("dosya");
    let pid = std::process::id();
    let seq = GECICI_SAYAC.fetch_add(1, Ordering::Relaxed);
    ebeveyn.join(format!(".{dosya_adi}.{pid}.{seq}.tmp"))
}

#[cfg(windows)]
fn atomik_yeniden_adlandir(gecici: &Path, hedef: &Path) -> Result<(), DepoHatasi> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let mut gecici_w: Vec<u16> = gecici.as_os_str().encode_wide().collect();
    gecici_w.push(0);
    let mut hedef_w: Vec<u16> = hedef.as_os_str().encode_wide().collect();
    hedef_w.push(0);

    // SAFETY: Her iki işaretçi de null sonlandırmalı geçerli UTF-16 tamponlarını gösterir.
    unsafe {
        MoveFileExW(
            PCWSTR(gecici_w.as_ptr()),
            PCWSTR(hedef_w.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomik_yeniden_adlandir(gecici: &Path, hedef: &Path) -> Result<(), DepoHatasi> {
    std::fs::rename(gecici, hedef)?;
    Ok(())
}

/// Geçerli süreç belirtecinden oturum açmış kullanıcının SID metnini döndürür.
#[cfg(windows)]
pub fn gecerli_kullanici_sid() -> Result<String, DepoHatasi> {
    use windows::Win32::System::Threading::GetCurrentProcess;
    // SAFETY: Geçerli süreç sözde tutamacı kapatılmaz ve her zaman geçerlidir.
    unsafe { surec_tutamaci_sid(GetCurrentProcess()) }
}

/// Verilen süreç kimliğinin sahibi olan kullanıcının SID metni (ör. pipe sunucusunun ve
/// sonlandırılacak servisin bizim oturumumuza ait olduğunu doğrulamak için).
#[cfg(windows)]
pub fn surec_kullanici_sid(pid: u32) -> Result<String, DepoHatasi> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: Yalnız sorgu hakkıyla açılır ve kapsam sonunda kapatılır.
    unsafe {
        let surec = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
        let sonuc = surec_tutamaci_sid(surec);
        let _ = CloseHandle(surec);
        sonuc
    }
}

#[cfg(not(windows))]
pub fn surec_kullanici_sid(_pid: u32) -> Result<String, DepoHatasi> {
    Err(DepoHatasi::Guvenlik(
        "SID yalnız Windows'ta çözülür".to_string(),
    ))
}

/// # Safety
/// `surec` en az `PROCESS_QUERY_LIMITED_INFORMATION` hakkı olan geçerli bir tutamaç olmalıdır.
#[cfg(windows)]
unsafe fn surec_tutamaci_sid(
    surec: windows::Win32::Foundation::HANDLE,
) -> Result<String, DepoHatasi> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
    use windows::Win32::System::Threading::OpenProcessToken;

    let mut token = HANDLE::default();
    // SAFETY: Süreç belirteci salt okuma amaçlı açılır ve kapsam sonunda kapatılır.
    unsafe {
        OpenProcessToken(surec, TOKEN_QUERY, &mut token)
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    }
    let mut ihtiyac: u32 = 0;
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut ihtiyac) };
    let mut buf = vec![0u8; ihtiyac as usize];
    let sonuc = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr().cast()),
            ihtiyac,
            &mut ihtiyac,
        )
    };
    unsafe {
        let _ = CloseHandle(token);
    }
    sonuc.map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;

    let token_user = unsafe { &*(buf.as_ptr().cast::<TOKEN_USER>()) };
    // SAFETY: `buf` hâlâ canlı; TOKEN_USER içindeki SID onun içini gösterir.
    unsafe { sid_metne(token_user.User.Sid) }
}

/// # Safety
/// `sid` geçerli bir SID'i göstermelidir.
#[cfg(windows)]
unsafe fn sid_metne(sid: windows::Win32::Security::PSID) -> Result<String, DepoHatasi> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::core::PWSTR;

    let mut sid_pwstr = PWSTR::null();
    unsafe {
        ConvertSidToStringSidW(sid, &mut sid_pwstr)
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    }
    let sid_metin = unsafe { sid_pwstr.to_string() }
        .map_err(|e| DepoHatasi::Guvenlik(format!("SID çözülemedi: {e}")))?;
    unsafe {
        let _ = LocalFree(Some(HLOCAL(sid_pwstr.0.cast())));
    }
    Ok(sid_metin)
}

/// Çekirdek nesnesinin (ör. named pipe istemci tutamacı) güvenlik tanımlayıcısındaki sahip SID'i.
/// Süreç belirteci açılamayan (yükseltilmiş) bir sunucunun sahibini doğrulamak için kullanılır;
/// tutamaç `READ_CONTROL` hakkı taşımalıdır (`GENERIC_READ` bunu içerir).
#[cfg(windows)]
pub fn nesne_sahibi_sid(tutamac: windows::Win32::Foundation::HANDLE) -> Result<String, DepoHatasi> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT};
    use windows::Win32::Security::{OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID};

    let mut sahip = PSID::default();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: Çıktı işaretçileri yerel değişkenleri gösterir; `sahip`, `sd` tamponunun içini
    // gösterdiğinden SID metne çevrildikten sonra `sd` bir kez serbest bırakılır.
    unsafe {
        let hata = GetSecurityInfo(
            tutamac,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut sahip),
            None,
            None,
            None,
            Some(&mut sd),
        );
        if hata.is_err() {
            return Err(std::io::Error::from_raw_os_error(hata.0 as i32).into());
        }
        let sonuc = if sahip.is_invalid() {
            Err(DepoHatasi::Guvenlik("nesnenin sahibi yok".to_string()))
        } else {
            sid_metne(sahip)
        };
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        sonuc
    }
}

/// Dizin veya dosya üzerinde DACL'i yalnız geçerli oturum sahibi + SYSTEM olarak daraltır (`plan.md §4.4`).
#[cfg(windows)]
pub fn kullanici_dacl_uygula(yol: &Path) -> Result<(), DepoHatasi> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
        SetNamedSecurityInfoW,
    };
    use windows::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    };
    use windows::core::{BOOL, PCWSTR};

    let kullanici_sid_str = gecerli_kullanici_sid()?;

    let sddl = format!("D:P(A;OICI;FA;;;{kullanici_sid_str})(A;OICI;FA;;;SY)");
    let mut sddl_w: Vec<u16> = sddl.encode_utf16().collect();
    sddl_w.push(0);

    let mut sd = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl_w.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
        .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    }

    let mut dacl_present = BOOL(0);
    let mut dacl_defaulted = BOOL(0);
    let mut dacl_ptr: *mut ACL = std::ptr::null_mut();
    let dacl_res = unsafe {
        GetSecurityDescriptorDacl(sd, &mut dacl_present, &mut dacl_ptr, &mut dacl_defaulted)
    };
    if let Err(e) = dacl_res {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(sd.0)));
        }
        return Err(std::io::Error::from_raw_os_error(e.code().0).into());
    }

    let mut yol_w: Vec<u16> = yol.as_os_str().encode_wide().collect();
    yol_w.push(0);

    let set_res = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(yol_w.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(dacl_ptr.cast_const()),
            None,
        )
    };

    unsafe {
        let _ = LocalFree(Some(HLOCAL(sd.0)));
    }

    if set_res.0 != 0 {
        return Err(std::io::Error::from_raw_os_error(set_res.0 as i32).into());
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn kullanici_dacl_uygula(_yol: &Path) -> Result<(), DepoHatasi> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_klasoru(ad: &str) -> PathBuf {
        let pid = std::process::id();
        let seq = GECICI_SAYAC.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("cetvel-depo-test-{ad}-{pid}-{seq}"))
    }

    #[test]
    fn atomik_yazim_yeni_dosya_ve_uzerine_yazmada_temiz_calisir() {
        let kok = test_klasoru("atomik");
        let hedef = kok.join("anlik.json");

        atomik_yaz(&hedef, br#"{"sira":1}"#).expect("ilk yazim");
        assert_eq!(
            std::fs::read_to_string(&hedef).expect("oku"),
            r#"{"sira":1}"#
        );

        atomik_yaz(&hedef, br#"{"sira":2}"#).expect("ikinci yazim");
        assert_eq!(
            std::fs::read_to_string(&hedef).expect("oku2"),
            r#"{"sira":2}"#
        );

        // Dizinde artık geçici `.tmp` dosya kalmamalıdır.
        let tmp_sayisi = std::fs::read_dir(&kok)
            .expect("dir")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(tmp_sayisi, 0);

        let _ = std::fs::remove_dir_all(&kok);
    }

    #[test]
    fn basarisiz_yeniden_adlandirmada_mevcut_dosya_bozulmaz() {
        let kok = test_klasoru("hata-koruma");
        std::fs::create_dir_all(&kok).expect("mkdir");
        // Hedef bir dizin ise MoveFileExW başarısız olur; bu durumda tmp artığı silinmelidir.
        let sahte_hedef_dizin = kok.join("dizin_hedef");
        std::fs::create_dir_all(&sahte_hedef_dizin).expect("mkdir hedef");

        let sonuc = atomik_yaz(&sahte_hedef_dizin, b"test");
        assert!(sonuc.is_err());

        let tmp_sayisi = std::fs::read_dir(&kok)
            .expect("dir")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(tmp_sayisi, 0);

        let _ = std::fs::remove_dir_all(&kok);
    }

    #[test]
    fn kullanici_dacl_dizinde_basariyla_uygulanir() {
        let kok = test_klasoru("dacl");
        std::fs::create_dir_all(&kok).expect("mkdir");
        kullanici_dacl_uygula(&kok).expect("dacl");
        let dosya = kok.join("deneme.txt");
        atomik_yaz(&dosya, b"merhaba").expect("yaz");
        assert_eq!(std::fs::read_to_string(&dosya).expect("oku"), "merhaba");
        let _ = std::fs::remove_dir_all(&kok);
    }
}
