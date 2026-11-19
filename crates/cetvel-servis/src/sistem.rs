use std::ffi::c_void;
use std::time::Duration;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    CreateMutexW, OpenMutexW, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, QueryFullProcessImageNameW,
    SYNCHRONIZATION_ACCESS_RIGHTS, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::core::{PCWSTR, PWSTR};

/// Eski sürümlerin (SID'siz) mutex adı: çalışan eski servis de "çalışıyor" sayılır.
pub const ESKI_SERVIS_MUTEX_ADI: &str = r"Local\Cetvel-Servis";
const SYNCHRONIZE_FLAG: SYNCHRONIZATION_ACCESS_RIGHTS = SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000);

/// Kullanıcıya özgü servis mutex adı (`Local\Cetvel-Servis-<SID>`): aynı oturumda başka bir
/// Windows kullanıcısıyla (`runas`) çalışan servis bu kullanıcının servisini engellemez.
pub fn servis_mutex_adi_uret(sid: &str) -> String {
    let temiz: String = sid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(96)
        .collect();
    if temiz.is_empty() {
        ESKI_SERVIS_MUTEX_ADI.to_string()
    } else {
        format!("{ESKI_SERVIS_MUTEX_ADI}-{temiz}")
    }
}

fn servis_mutex_adi() -> String {
    cetvel_depo::gecerli_kullanici_sid()
        .map(|sid| servis_mutex_adi_uret(&sid))
        .unwrap_or_else(|_| ESKI_SERVIS_MUTEX_ADI.to_string())
}

fn genis(metin: &str) -> Vec<u16> {
    metin.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Erişim reddi bir varlık kanıtıdır: ad var ama açamıyoruz. Kullanıcıya özgü (SID'li) adda bu,
/// aynı kullanıcının yükseltilmiş (yönetici) başlattığı servistir: varsayılan DACL'i yalnız
/// Administrators/SYSTEM'e izin verir. Eski ortak adda ise başka kullanıcı olabilir, "yok" sayılır.
fn mutex_var_mi(ad: &str, erisim_reddi_var_sayilir: bool) -> bool {
    let ad = genis(ad);
    // SAFETY: Yalnız mevcut adlandırılmış mutex varlığını salt-okuma sorgular.
    unsafe {
        match OpenMutexW(SYNCHRONIZE_FLAG, false, PCWSTR(ad.as_ptr())) {
            Ok(h) => {
                let _ = CloseHandle(h);
                true
            }
            Err(e) => erisim_reddi_var_sayilir && erisim_reddi_mi(e.code()),
        }
    }
}

fn erisim_reddi_mi(kod: windows::core::HRESULT) -> bool {
    kod == ERROR_ACCESS_DENIED.to_hresult()
}

/// Tek servis örneği garantisi için kullanıcıya özgü adlandırılmış Windows mutex kilidi.
pub struct TekOrnekKilidi {
    handle_addr: usize,
}

impl TekOrnekKilidi {
    /// Kilidi almayı dener. Bu kullanıcının başka bir servisi (yeni ya da eski adla) çalışıyorsa
    /// `Ok(None)` döner.
    pub fn al() -> std::io::Result<Option<Self>> {
        Self::adla_al(&servis_mutex_adi(), Some(ESKI_SERVIS_MUTEX_ADI))
    }

    fn adla_al(ad: &str, eski_ad: Option<&str>) -> std::io::Result<Option<Self>> {
        if let Some(eski) = eski_ad
            && eski != ad
            && mutex_var_mi(eski, false)
        {
            return Ok(None);
        }
        let ad_genis = genis(ad);
        // SAFETY: NUL sonlu adla yerel mutex oluşturulur/açılır.
        let handle = match unsafe { CreateMutexW(None, false, PCWSTR(ad_genis.as_ptr())) } {
            Ok(h) => h,
            // Aynı ad yükseltilmiş bir örnekte açık (bkz. `mutex_var_mi`): servis zaten çalışıyor.
            Err(e) if erisim_reddi_mi(e.code()) => return Ok(None),
            Err(e) => return Err(std::io::Error::from_raw_os_error(e.code().0)),
        };

        let err = unsafe { GetLastError() };
        if err == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Ok(None);
        }

        Ok(Some(Self {
            handle_addr: handle.0 as usize,
        }))
    }
}

impl Drop for TekOrnekKilidi {
    fn drop(&mut self) {
        let handle = HANDLE(self.handle_addr as *mut c_void);
        // SAFETY: Sahip olunan mutex tutamacı burada bir kez kapatılır.
        unsafe {
            let _ = CloseHandle(handle);
        }
    }
}

/// Bu kullanıcının servisinin (yeni ya da eski sürüm) çalışıp çalışmadığını kilit oluşturmadan
/// kontrol eder.
pub fn servis_calisiyor_mu() -> bool {
    mutex_var_mi(&servis_mutex_adi(), true) || mutex_var_mi(ESKI_SERVIS_MUTEX_ADI, false)
}

/// Sonlandırma güvenlik kuralı (saf): kendi sürecimiz değil, görüntü adı `cetvel.exe` ve sahibi
/// bu oturumun kullanıcısı olmalı (PID yeniden kullanılmış olabilir).
pub fn sonlandirma_uygun_mu(
    pid: u32,
    kendi_pid: u32,
    goruntu_yolu: &str,
    sahip_sid: Option<&str>,
    kendi_sid: Option<&str>,
) -> Result<(), &'static str> {
    if pid == 0 || pid == kendi_pid {
        return Err("kendi süreci ya da geçersiz PID");
    }
    let ad = goruntu_yolu
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ad != "cetvel.exe" {
        return Err("süreç cetvel.exe değil (PID yeniden kullanılmış olabilir)");
    }
    match (sahip_sid, kendi_sid) {
        (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => Ok(()),
        // Yükseltilmiş süreçte belirteç yükseltilmemiş istemciden okunamaz; sonlandırma da
        // reddedileceğinden kullanıcıya yol gösterilir.
        (None, _) => Err(
            "süreç sahibi doğrulanamadı (servis yönetici olarak başlatılmış olabilir; komutu yönetici terminalinden çalıştırın)",
        ),
        _ => Err("süreç bu kullanıcıya ait değil"),
    }
}

/// Sürecin tam görüntü yolu (`QueryFullProcessImageNameW`).
pub fn surec_goruntu_yolu(pid: u32) -> Option<String> {
    // SAFETY: Sorgu hakkıyla açılan tutamaç kapsam sonunda kapatılır; tampon uzunluğu verilir.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut tampon = vec![0u16; 1024];
        let mut uzunluk = tampon.len() as u32;
        let sonuc = QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_WIN32,
            PWSTR(tampon.as_mut_ptr()),
            &mut uzunluk,
        );
        let _ = CloseHandle(h);
        sonuc.ok()?;
        Some(String::from_utf16_lossy(&tampon[..uzunluk as usize]))
    }
}

/// Süreç hâlâ çalışıyor mu (açılamıyorsa çalışmıyor sayılır).
pub fn surec_yasiyor_mu(pid: u32) -> bool {
    // SAFETY: Yalnız bekleme hakkıyla açılır, sıfır zaman aşımıyla sorgulanır, kapatılır.
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else {
            return false;
        };
        let durum = WaitForSingleObject(h, 0);
        let _ = CloseHandle(h);
        durum == WAIT_TIMEOUT
    }
}

/// Yanıt vermeyen Cetvel servis sürecini doğrulayarak sonlandırır ve çıkmasını bekler.
/// [`sonlandirma_uygun_mu`] kuralını geçmeyen süreç asla sonlandırılmaz.
pub fn cetvel_surecini_sonlandir(pid: u32, bekleme: Duration) -> Result<(), String> {
    let yol = surec_goruntu_yolu(pid).ok_or_else(|| "süreç bulunamadı".to_string())?;
    let sahip = cetvel_depo::surec_kullanici_sid(pid).ok();
    let kendi = cetvel_depo::gecerli_kullanici_sid().ok();
    sonlandirma_uygun_mu(
        pid,
        std::process::id(),
        &yol,
        sahip.as_deref(),
        kendi.as_deref(),
    )
    .map_err(str::to_string)?;
    // SAFETY: Doğrulanmış PID için sonlandırma + bekleme hakkıyla tutamaç açılır ve kapatılır.
    unsafe {
        let h = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid)
            .map_err(|e| format!("süreç açılamadı: {e}"))?;
        let sonuc = TerminateProcess(h, 1);
        let bekle = WaitForSingleObject(h, bekleme.as_millis().min(u32::MAX as u128) as u32);
        let _ = CloseHandle(h);
        sonuc.map_err(|e| format!("süreç sonlandırılamadı: {e}"))?;
        if bekle != WAIT_OBJECT_0 {
            return Err("süreç sonlandırma sonrası kapanmadı".to_string());
        }
    }
    Ok(())
}

/// Son klavye/fare girdisi eşik süreden kısa önce gerçekleştiyse `true` döner (`plan.md §4.1`).
pub fn kullanici_etkin_mi(esik_sn: u64) -> bool {
    let mut bilgi = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `cbSize` doğru ayarlanmış `LASTINPUTINFO` yapısına yazılır.
    let ok = unsafe { GetLastInputInfo(&mut bilgi) }.as_bool();
    if !ok {
        return true;
    }
    let simdi_ms = unsafe { GetTickCount() };
    let gecen_ms = simdi_ms.wrapping_sub(bilgi.dwTime) as u64;
    gecen_ms <= esik_sn.saturating_mul(1000)
}

/// Dosya adlarında kullanılacak yerel tarih-saat etiketi (`2026-11-16 14.30.05`).
pub fn zaman_etiketi_bicimle(
    yil: u16,
    ay: u16,
    gun: u16,
    saat: u16,
    dakika: u16,
    saniye: u16,
) -> String {
    format!("{yil:04}-{ay:02}-{gun:02} {saat:02}.{dakika:02}.{saniye:02}")
}

/// Şu anki yerel duvar saatiyle (yaz saati dahil) dosya adı etiketi üretir.
pub fn yerel_zaman_etiketi() -> String {
    // SAFETY: `GetLocalTime` salt okuma ile yerel duvar saatini döndürür.
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    zaman_etiketi_bicimle(
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zaman_etiketi_dosya_adina_uygun_bicimlenir() {
        assert_eq!(
            zaman_etiketi_bicimle(2026, 11, 6, 9, 5, 3),
            "2026-11-06 09.05.03"
        );
        let canli = yerel_zaman_etiketi();
        assert_eq!(canli.len(), 19);
        assert!(!canli.contains(':'));
    }

    #[test]
    fn ayni_surecte_ikinci_mutex_kilidi_alinamaz_ve_drop_ile_serbest_kalir() {
        // Gerçek servis adına dokunmamak için test adları kullanılır.
        let ad = format!(r"Local\Cetvel-Test-{}", std::process::id());
        let ilk = TekOrnekKilidi::adla_al(&ad, None)
            .expect("ilk")
            .expect("kilit alinmali");
        assert!(mutex_var_mi(&ad, true));
        assert!(
            TekOrnekKilidi::adla_al(&ad, None)
                .expect("ikinci")
                .is_none()
        );
        drop(ilk);
        assert!(!mutex_var_mi(&ad, true));
    }

    #[test]
    fn erisim_reddedilen_kullaniciya_ozgu_mutex_calisan_servis_sayilir() {
        use windows::Win32::Foundation::{HLOCAL, LocalFree};
        use windows::Win32::Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
        };
        use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
        use windows::core::BOOL;

        // Yükseltilmiş servisin mutex'ini taklit eder: DACL bu kullanıcıya izin vermez.
        let ad = format!(r"Local\Cetvel-Test-Reddi-{}", std::process::id());
        let sddl = genis("D:P(A;;GA;;;SY)");
        let mut sd = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut sd,
                None,
            )
            .expect("sddl");
        }
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: BOOL(0),
        };
        let ad_genis = genis(&ad);
        let tutamac =
            unsafe { CreateMutexW(Some(&sa), false, PCWSTR(ad_genis.as_ptr())) }.expect("mutex");
        unsafe {
            let _ = LocalFree(Some(HLOCAL(sd.0)));
        }
        assert!(mutex_var_mi(&ad, true));
        assert!(!mutex_var_mi(&ad, false));
        assert!(
            TekOrnekKilidi::adla_al(&ad, None)
                .expect("hata degil")
                .is_none()
        );
        unsafe {
            let _ = CloseHandle(tutamac);
        }
        assert!(!mutex_var_mi(&ad, true));
    }

    #[test]
    fn eski_adli_mutex_varsa_yeni_kilit_alinmaz() {
        let eski = format!(r"Local\Cetvel-Test-Eski-{}", std::process::id());
        let yeni = format!("{eski}-S-1-5-21-1");
        let eski_kilit = TekOrnekKilidi::adla_al(&eski, None).unwrap().unwrap();
        assert!(
            TekOrnekKilidi::adla_al(&yeni, Some(&eski))
                .unwrap()
                .is_none()
        );
        drop(eski_kilit);
        let yeni_kilit = TekOrnekKilidi::adla_al(&yeni, Some(&eski)).unwrap();
        assert!(yeni_kilit.is_some());
    }

    #[test]
    fn servis_mutex_adi_kullanici_sid_icerir() {
        assert_eq!(
            servis_mutex_adi_uret("S-1-5-21-77"),
            r"Local\Cetvel-Servis-S-1-5-21-77"
        );
        assert_eq!(
            servis_mutex_adi_uret(r"S-1 5\x"),
            r"Local\Cetvel-Servis-S-15x"
        );
        assert_eq!(servis_mutex_adi_uret(""), ESKI_SERVIS_MUTEX_ADI);
        assert!(servis_mutex_adi().starts_with(ESKI_SERVIS_MUTEX_ADI));
    }

    #[test]
    fn sonlandirma_yalniz_bu_kullanicinin_cetvel_surecine_izinli() {
        let sid = Some("S-1-5-21-1");
        assert!(sonlandirma_uygun_mu(10, 1, r"C:\A\cetvel.exe", sid, sid).is_ok());
        assert!(sonlandirma_uygun_mu(10, 1, "C:/A/CETVEL.EXE", sid, sid).is_ok());
        assert!(sonlandirma_uygun_mu(1, 1, r"C:\A\cetvel.exe", sid, sid).is_err());
        assert!(sonlandirma_uygun_mu(0, 1, r"C:\A\cetvel.exe", sid, sid).is_err());
        assert!(sonlandirma_uygun_mu(10, 1, r"C:\A\notepad.exe", sid, sid).is_err());
        assert!(sonlandirma_uygun_mu(10, 1, r"C:\A\cetvel-masaustu.exe", sid, sid).is_err());
        assert!(sonlandirma_uygun_mu(10, 1, r"C:\A\cetvel.exe", Some("S-1-5-21-2"), sid).is_err());
        assert!(
            sonlandirma_uygun_mu(10, 1, r"C:\A\cetvel.exe", None, sid)
                .unwrap_err()
                .contains("yönetici")
        );
    }

    #[test]
    fn surec_yardimcilari_kendi_surecimizi_tanir_ama_sonlandirmaz() {
        let pid = std::process::id();
        assert!(surec_yasiyor_mu(pid));
        assert!(surec_goruntu_yolu(pid).is_some_and(|y| y.ends_with(".exe")));
        assert!(cetvel_surecini_sonlandir(pid, Duration::from_millis(10)).is_err());
        assert!(!surec_yasiyor_mu(u32::MAX - 3));
    }
}
