use std::ffi::c_void;
use std::os::windows::io::RawHandle;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};

/// Alt süreç ve çocuk süreç ağacını Windows Job Object ile güvenli biçimde öldüren RAII koruması.
pub struct JobSuresKorumasi {
    /// Win32 HANDLE değeri (send/sync sarmalaması için ham işaretçi yerine adres olarak tutulur).
    handle_addr: usize,
}

impl JobSuresKorumasi {
    /// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` bayrağıyla yeni bir anonim Job Object oluşturur.
    pub fn yeni() -> std::io::Result<Self> {
        // SAFETY: Geçerli boş güvenlik öznitelikleri ve ad ile yeni çekirdek Job Object oluşturulur.
        let handle = unsafe { CreateJobObjectW(None, None) }
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        // SAFETY: `info` yapısı başlatılmış ve boyutu doğru verilmiştir.
        let set_res = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast::<c_void>(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if let Err(e) = set_res {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(std::io::Error::from_raw_os_error(e.code().0));
        }

        Ok(Self {
            handle_addr: handle.0 as usize,
        })
    }

    /// Başlatılmış alt süreci bu Job Object'e bağlar.
    pub fn sureci_ekle(&self, process_handle: RawHandle) -> std::io::Result<()> {
        let job = HANDLE(self.handle_addr as *mut c_void);
        let proc_h = HANDLE(process_handle);
        // SAFETY: `job` ve `proc_h` açık çekirdek tutamaçlarıdır.
        unsafe { AssignProcessToJobObject(job, proc_h) }
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))
    }

    /// Job içindeki tüm süreç ağacını hemen sonlandırır.
    pub fn sonlandir(&self) {
        let job = HANDLE(self.handle_addr as *mut c_void);
        // SAFETY: Geçerli Job tutamacı tüm bağlı alt süreçlerle birlikte sonlandırılır.
        unsafe {
            let _ = TerminateJobObject(job, 1);
        }
    }
}

impl Drop for JobSuresKorumasi {
    fn drop(&mut self) {
        self.sonlandir();
        let job = HANDLE(self.handle_addr as *mut c_void);
        // SAFETY: Tutamaç yalnız burada bir kez kapatılır.
        unsafe {
            let _ = CloseHandle(job);
        }
    }
}
