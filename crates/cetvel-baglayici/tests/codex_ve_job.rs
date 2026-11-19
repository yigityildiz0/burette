use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use cetvel_baglayici::{JobSuresKorumasi, codex_app_server_oku, codex_yanit_ayristir};
use cetvel_model::{Durum, UtcZaman};
use serde_json::{Value, json};

fn main() {
    let exe = std::env::current_exe().expect("current_exe");
    let stem = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();

    match stem.as_str() {
        "sahte-codex-ok" => rol_sahte_codex_ok(),
        "sahte-codex-asili" => rol_sahte_codex_asili(&exe),
        "sahte-torun" => std::thread::sleep(Duration::from_secs(60)),
        _ => testleri_calistir(&exe),
    }
}

fn rol_sahte_codex_ok() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for satir_res in stdin.lock().lines() {
        let Ok(satir) = satir_res else { break };
        let Ok(deger) = serde_json::from_str::<Value>(&satir) else {
            continue;
        };
        let metod = deger.get("method").and_then(Value::as_str).unwrap_or("");
        let id = deger.get("id").and_then(Value::as_i64);
        match (metod, id) {
            ("initialize", Some(1)) => {
                writeln!(stdout, "2026-11-15T18:00:00Z INFO starting app-server").ok();
                writeln!(stdout, "{}", json!({"method":"account/rateLimits/updated"})).ok();
                writeln!(
                    stdout,
                    "{}",
                    json!({"id":1,"result":{"userAgent":"sahte-codex"}})
                )
                .ok();
                stdout.flush().ok();
            }
            ("initialized", None) => {}
            ("account/rateLimits/read", Some(2)) => {
                let yanit = json!({
                    "id": 2,
                    "result": {
                        "rateLimits": {
                            "limitId": "codex",
                            "planType": "plus",
                            "primary": {
                                "usedPercent": 15.0,
                                "windowDurationMins": 300,
                                "resetsAt": 1794800000
                            },
                            "secondary": {
                                "usedPercent": 100.0,
                                "windowDurationMins": 10080,
                                "resetsAt": 1795200000
                            }
                        }
                    }
                });
                writeln!(stdout, "{yanit}").ok();
                stdout.flush().ok();
                break;
            }
            _ => {}
        }
    }
}

fn rol_sahte_codex_asili(exe: &Path) {
    let torun_exe = exe.with_file_name("sahte-torun.exe");
    let pid_dosya = exe.with_file_name("torun.pid");
    if let Ok(child) = Command::new(&torun_exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        let _ = std::fs::write(&pid_dosya, child.id().to_string());
    }
    std::thread::sleep(Duration::from_secs(60));
}

fn testleri_calistir(exe: &Path) {
    let gecici = std::env::temp_dir().join(format!("cetvel-codex-test-{}", std::process::id()));
    std::fs::create_dir_all(&gecici).expect("temp dir");

    let ok_exe = kopyala_veya_bagla(exe, &gecici.join("sahte-codex-ok.exe"));
    let asili_exe = kopyala_veya_bagla(exe, &gecici.join("sahte-codex-asili.exe"));
    let torun_exe = kopyala_veya_bagla(exe, &gecici.join("sahte-torun.exe"));

    let rt = tokio::runtime::Runtime::new().expect("tokio rt");

    // 1. Sahte app-server el sıkışması + çöp satır + bildirim atlama + ayrıştırma
    rt.block_on(async {
        let sonuc = codex_app_server_oku(&ok_exe, Duration::from_secs(5))
            .await
            .expect("sahte codex okunmalı");
        let simdi = UtcZaman::parse_tarih_veya_zaman("2026-11-15T18:00:00Z").expect("zaman");
        let havuz = codex_yanit_ayristir(&sonuc, simdi);
        assert_eq!(havuz.durum, Durum::Ok);
        assert!((havuz.pencereler["5s"].kalan - 0.85).abs() < 1e-9);
        assert_eq!(havuz.pencereler["hafta"].kalan, 0.0);
        assert_eq!(havuz.etkin_kalan, Some(0.0));
        assert_eq!(havuz.baglayici_pencere.as_deref(), Some("hafta"));
        assert_eq!(havuz.kullanilabilir, Some(false));
    });

    // 2. Zaman aşımı + Job Object ile çocuk ve torun sürecin öldürülmesi
    rt.block_on(async {
        let res = codex_app_server_oku(&asili_exe, Duration::from_millis(450)).await;
        assert_eq!(res, Err("zaman_asimi"));

        let pid_dosya = gecici.join("torun.pid");
        if let Ok(pid_str) = std::fs::read_to_string(&pid_dosya)
            && let Ok(torun_pid) = pid_str.trim().parse::<u32>()
        {
            std::thread::sleep(Duration::from_millis(100));
            assert!(
                !surec_hayatta_mi(torun_pid),
                "Job Object torun süreci de sonlandırmalıydı (pid={torun_pid})"
            );
        }
    });

    // 3. JobSuresKorumasi Drop doğrudan süreci sonlandırır
    {
        use std::os::windows::io::AsRawHandle;
        let mut child = Command::new(&torun_exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("torun baslat");
        let pid = child.id();
        let job = JobSuresKorumasi::yeni().expect("job");
        job.sureci_ekle(child.as_raw_handle()).expect("ekle");
        drop(job);
        let _ = child.wait();
        assert!(!surec_hayatta_mi(pid));
    }

    let _ = std::fs::remove_dir_all(&gecici);
    println!("codex_ve_job testleri gecti: 3/3");
}

fn kopyala_veya_bagla(kaynak: &Path, hedef: &Path) -> PathBuf {
    if std::fs::hard_link(kaynak, hedef).is_err() {
        std::fs::copy(kaynak, hedef).expect("exe kopyala");
    }
    hedef.to_path_buf()
}

fn surec_hayatta_mi(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const STILL_ACTIVE: u32 = 259;

    // SAFETY: Sadece verilen PID'nin çıkış kodunu sorgular ve tutamacı hemen kapatır.
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code: u32 = 0;
        let ok = GetExitCodeProcess(handle, &mut code).is_ok();
        let _ = CloseHandle(handle);
        ok && code == STILL_ACTIVE
    }
}
