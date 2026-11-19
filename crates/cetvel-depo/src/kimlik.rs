//! Windows Credential Manager üzerinden yazma-öncelikli sır saklama yardımcıları.
//!
//! Masaüstü arayüzü yalnız `kimlik_kaydet` ve `kimlik_var_mi` çağırır; gizli değer
//! hiçbir zaman UI katmanına geri döndürülmez ve günlüklere yazılmaz.

use std::sync::atomic::{Ordering, compiler_fence};

use crate::DepoHatasi;
use crate::gizli::GizliDeger;

const HEDEF_ONEK: &str = "Cetvel/";

/// Verilen anahtar adı için Windows Credential Manager hedef adını doğrular ve üretir.
pub fn kimlik_hedef_adi(ad: &str) -> Result<String, DepoHatasi> {
    let temiz = ad.trim();
    if temiz.is_empty()
        || temiz.len() > 128
        || temiz
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '\\' || c == '/')
    {
        return Err(DepoHatasi::Guvenlik(
            "geçersiz kimlik anahtar adı".to_string(),
        ));
    }
    Ok(format!("{HEDEF_ONEK}{temiz}"))
}

fn bellek_sifirla(baytlar: &mut [u8]) {
    for b in baytlar.iter_mut() {
        // Derleyicinin ölü-yazma optimizasyonunu engellemek için volatile sıfırlama.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

#[cfg(windows)]
fn genis_nul_ekle(metin: &str) -> Vec<u16> {
    metin.encode_utf16().chain(std::iter::once(0)).collect()
}

/// API anahtarının en fazla bu kadar bayt olmasına izin verilir (Credential Manager blob sınırı 2560).
pub const AZAMI_ANAHTAR_BAYT: usize = 512;

/// Kaydedilecek API anahtarını doğrular: baş/son boşluk kırpılır; boş, aşırı uzun,
/// iç boşluk veya kontrol karakteri içeren değer reddedilir. Hata metni değeri içermez.
pub fn api_anahtari_dogrula(gizli: &str) -> Result<&str, DepoHatasi> {
    let temiz = gizli.trim();
    if temiz.is_empty() {
        return Err(DepoHatasi::Guvenlik(
            "kaydedilecek anahtar boş olamaz".to_string(),
        ));
    }
    if temiz.len() > AZAMI_ANAHTAR_BAYT {
        return Err(DepoHatasi::Guvenlik(format!(
            "anahtar çok uzun (en fazla {AZAMI_ANAHTAR_BAYT} bayt)"
        )));
    }
    if temiz.len() < 8 {
        return Err(DepoHatasi::Guvenlik(
            "anahtar çok kısa (en az 8 karakter)".to_string(),
        ));
    }
    if temiz.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(DepoHatasi::Guvenlik(
            "anahtar boşluk veya kontrol karakteri içeremez".to_string(),
        ));
    }
    Ok(temiz)
}

/// Gizli anahtarı Windows Credential Manager'a kaydeder (değer günlüğe yazılmaz, iade edilmez).
pub fn kimlik_kaydet(ad: &str, gizli: &str) -> Result<(), DepoHatasi> {
    let temiz_gizli = api_anahtari_dogrula(gizli)?;

    #[cfg(windows)]
    {
        use windows::Win32::Security::Credentials::{
            CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredWriteW,
        };
        use windows::core::PWSTR;

        let hedef = kimlik_hedef_adi(ad)?;
        let mut hedef_utf16 = genis_nul_ekle(&hedef);
        let mut kullanici_utf16 = genis_nul_ekle("cetvel");
        let mut yorum_utf16 = genis_nul_ekle("Cetvel API anahtarı");
        let mut blob = temiz_gizli.as_bytes().to_vec();
        let blob_len = u32::try_from(blob.len())
            .map_err(|_| DepoHatasi::Guvenlik("anahtar boyutu sınırı aşıyor".to_string()))?;

        let cred = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(hedef_utf16.as_mut_ptr()),
            Comment: PWSTR(yorum_utf16.as_mut_ptr()),
            CredentialBlobSize: blob_len,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(kullanici_utf16.as_mut_ptr()),
            ..Default::default()
        };

        let sonuc = unsafe { CredWriteW(&cred, 0) };
        bellek_sifirla(&mut blob);
        sonuc.map_err(|e| DepoHatasi::Guvenlik(format!("Credential Manager yazma hatası: {e}")))
    }

    #[cfg(not(windows))]
    {
        let _ = kimlik_hedef_adi(ad)?;
        Err(DepoHatasi::Guvenlik(
            "Windows Credential Manager yalnız Windows üzerinde desteklenir".to_string(),
        ))
    }
}

/// Anahtarın Windows Credential Manager'da mevcut olup olmadığını kontrol eder (değeri döndürmez).
pub fn kimlik_var_mi(ad: &str) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Security::Credentials::{
            CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW,
        };
        use windows::core::PCWSTR;

        let Ok(hedef) = kimlik_hedef_adi(ad) else {
            return false;
        };
        let hedef_utf16 = genis_nul_ekle(&hedef);
        let mut ptr: *mut CREDENTIALW = std::ptr::null_mut();
        let ok = unsafe {
            CredReadW(
                PCWSTR(hedef_utf16.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut ptr,
            )
        };
        if ok.is_err() || ptr.is_null() {
            return false;
        }
        unsafe {
            let cred = &*ptr;
            let bos_mu = cred.CredentialBlob.is_null() || cred.CredentialBlobSize == 0;
            if !cred.CredentialBlob.is_null() && cred.CredentialBlobSize > 0 {
                let dilim = std::slice::from_raw_parts_mut(
                    cred.CredentialBlob,
                    cred.CredentialBlobSize as usize,
                );
                bellek_sifirla(dilim);
            }
            CredFree(ptr.cast());
            !bos_mu
        }
    }

    #[cfg(not(windows))]
    {
        let _ = ad;
        false
    }
}

/// Bağlayıcı/servis çözümlemesi için Windows Credential Manager'dan sıfırlanabilir [`GizliDeger`] okur.
pub fn kimlik_oku_gizli(ad: &str) -> Option<GizliDeger> {
    #[cfg(windows)]
    {
        use windows::Win32::Security::Credentials::{
            CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW,
        };
        use windows::core::PCWSTR;

        let hedef = kimlik_hedef_adi(ad).ok()?;
        let hedef_utf16 = genis_nul_ekle(&hedef);
        let mut ptr: *mut CREDENTIALW = std::ptr::null_mut();
        if unsafe {
            CredReadW(
                PCWSTR(hedef_utf16.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut ptr,
            )
        }
        .is_err()
            || ptr.is_null()
        {
            return None;
        }

        let mut kopya = Vec::new();
        unsafe {
            let cred = &*ptr;
            if !cred.CredentialBlob.is_null() && cred.CredentialBlobSize > 0 {
                let dilim = std::slice::from_raw_parts_mut(
                    cred.CredentialBlob,
                    cred.CredentialBlobSize as usize,
                );
                kopya.extend_from_slice(dilim);
                bellek_sifirla(dilim);
            }
            CredFree(ptr.cast());
        }

        match String::from_utf8(kopya) {
            Ok(metin) => GizliDeger::yeni(metin),
            Err(err) => {
                let mut baytlar = err.into_bytes();
                bellek_sifirla(&mut baytlar);
                None
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = ad;
        None
    }
}

/// Windows Credential Manager kaydını siler (yoksa sessizce `Ok(())` döner).
pub fn kimlik_sil(ad: &str) -> Result<(), DepoHatasi> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::ERROR_NOT_FOUND;
        use windows::Win32::Security::Credentials::{CRED_TYPE_GENERIC, CredDeleteW};
        use windows::core::PCWSTR;

        let hedef = kimlik_hedef_adi(ad)?;
        let hedef_utf16 = genis_nul_ekle(&hedef);
        match unsafe { CredDeleteW(PCWSTR(hedef_utf16.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
            Err(e) => Err(DepoHatasi::Guvenlik(format!(
                "Credential Manager silme hatası: {e}"
            ))),
        }
    }

    #[cfg(not(windows))]
    {
        let _ = kimlik_hedef_adi(ad)?;
        Ok(())
    }
}

/// `Cetvel/<ad>` hedef adından anahtar adını çıkarır (önek yoksa `None`).
pub fn kimlik_adi_hedeften(hedef: &str) -> Option<String> {
    let ad = hedef.strip_prefix(HEDEF_ONEK)?;
    kimlik_hedef_adi(ad).ok().map(|_| ad.to_string())
}

/// Credential Manager'daki tüm `Cetvel/*` girdilerinin anahtar adlarını listeler (değerler okunmaz).
pub fn kimlik_listele() -> Result<Vec<String>, DepoHatasi> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::ERROR_NOT_FOUND;
        use windows::Win32::Security::Credentials::{CREDENTIALW, CredEnumerateW, CredFree};
        use windows::core::PCWSTR;

        let filtre = genis_nul_ekle(&format!("{HEDEF_ONEK}*"));
        let mut adet: u32 = 0;
        let mut dizi: *mut *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: Filtre NUL sonlu; dönen dizi yalnız okunur ve `CredFree` ile bir kez bırakılır.
        let sonuc = unsafe { CredEnumerateW(PCWSTR(filtre.as_ptr()), None, &mut adet, &mut dizi) };
        if let Err(e) = sonuc {
            if e.code() == ERROR_NOT_FOUND.to_hresult() {
                return Ok(Vec::new());
            }
            return Err(DepoHatasi::Guvenlik(format!(
                "Credential Manager listeleme hatası: {e}"
            )));
        }
        let mut adlar = Vec::new();
        if !dizi.is_null() {
            unsafe {
                for i in 0..adet as usize {
                    let cred = *dizi.add(i);
                    if cred.is_null() || (*cred).TargetName.is_null() {
                        continue;
                    }
                    if let Ok(hedef) = (*cred).TargetName.to_string()
                        && let Some(ad) = kimlik_adi_hedeften(&hedef)
                    {
                        adlar.push(ad);
                    }
                }
                CredFree(dizi.cast());
            }
        }
        adlar.sort();
        adlar.dedup();
        Ok(adlar)
    }

    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestKimlikTemizleyici(String);
    impl Drop for TestKimlikTemizleyici {
        fn drop(&mut self) {
            let _ = kimlik_sil(&self.0);
        }
    }

    #[test]
    fn api_anahtari_dogrulama_bos_uzun_ve_bosluklu_degeri_reddeder() {
        assert_eq!(
            api_anahtari_dogrula("  sk-ornek-12345  ").unwrap(),
            "sk-ornek-12345"
        );
        assert!(api_anahtari_dogrula("   ").is_err());
        assert!(api_anahtari_dogrula("kisa").is_err());
        assert!(api_anahtari_dogrula("sk-ornek 12345").is_err());
        assert!(api_anahtari_dogrula("sk-ornek\t12345").is_err());
        assert!(api_anahtari_dogrula(&"a".repeat(AZAMI_ANAHTAR_BAYT + 1)).is_err());
        assert!(api_anahtari_dogrula(&"a".repeat(AZAMI_ANAHTAR_BAYT)).is_ok());
        let hata = api_anahtari_dogrula("sk-gizli deger")
            .unwrap_err()
            .to_string();
        assert!(!hata.contains("gizli"));
    }

    #[test]
    fn kimlik_hedef_adi_dogrulanir() {
        assert_eq!(
            kimlik_hedef_adi("DEEPSEEK_API_KEY").unwrap(),
            "Cetvel/DEEPSEEK_API_KEY"
        );
        assert!(kimlik_hedef_adi("").is_err());
        assert!(kimlik_hedef_adi("ornek anahtar").is_err());
        assert!(kimlik_hedef_adi("ornek/key").is_err());
        assert_eq!(
            kimlik_adi_hedeften("Cetvel/DEEPSEEK_API_KEY").as_deref(),
            Some("DEEPSEEK_API_KEY")
        );
        assert_eq!(kimlik_adi_hedeften("Baska/DEEPSEEK_API_KEY"), None);
        assert_eq!(kimlik_adi_hedeften("Cetvel/"), None);
        assert_eq!(kimlik_adi_hedeften("Cetvel/a/b"), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_credential_manager_yaz_varlik_kontrol_et_ve_sil() {
        let anahtar_ad = format!("CETVEL_TEST_CRED_{}", std::process::id());
        let _temiz = TestKimlikTemizleyici(anahtar_ad.clone());
        let _ = kimlik_sil(&anahtar_ad);

        assert!(!kimlik_var_mi(&anahtar_ad));
        assert!(kimlik_kaydet(&anahtar_ad, "sk-test-gizli-deger").is_ok());
        assert!(kimlik_var_mi(&anahtar_ad));
        assert!(
            kimlik_listele()
                .expect("listele")
                .iter()
                .any(|a| a == &anahtar_ad),
            "Cetvel/* listesinde test girdisi görünmeli"
        );

        let okunan = kimlik_oku_gizli(&anahtar_ad).expect("gizli deger okunmali");
        assert_eq!(okunan.acik_metin(), "sk-test-gizli-deger");
        assert!(!format!("{okunan:?}").contains("sk-test-gizli-deger"));

        assert!(kimlik_sil(&anahtar_ad).is_ok());
        assert!(!kimlik_var_mi(&anahtar_ad));
        assert!(!kimlik_listele().unwrap().iter().any(|a| a == &anahtar_ad));
    }
}
