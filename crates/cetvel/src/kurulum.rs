use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use cetvel_depo::{
    Ayarlar, CETVEL_AUMID, CetvelYollar, KisayolBilgisi, atomik_yaz, kullanici_dacl_uygula,
};

/// Servis görevinin adı (Görev Zamanlayıcı kökünde). Yönetici olmayan kullanıcının görevi kabul
/// edilsin diye XML'de tetikleyici ve sorumlu o anki kullanıcının SID'ine bağlanır
/// (`gorev_xml_uret`): `<UserId>`'siz oturum açılışı tetikleyicisi "herhangi bir kullanıcı"
/// demektir ve standart kullanıcıya "Erişim engellendi" ile reddedilir (kökte de alt klasörde de).
pub const GOREV_ADI: &str = "CetvelServis";
/// Önceki sürümlerin alt klasörlü görev adı: tanınır, `kur` ve `kaldir` onu da siler.
pub const ESKI_GOREV_ADLARI: &[&str] = &[r"Cetvel\CetvelServis"];
/// Önceki sürümlerin görev klasörü: eski görev silinince boş kalırsa o da silinir.
pub const ESKI_GOREV_KLASORU: &str = "Cetvel";
/// Görev kaydı başarısızsa yedek yol: HKCU `Run` altında oturum açılışı kaydının değer adı.
pub const SERVIS_RUN_DEGERI: &str = "CetvelServis";
/// Masaüstü otomatik başlatma (`tauri-plugin-autostart`) kayıt değeri adı.
pub const MASAUSTU_RUN_DEGERI: &str = "Cetvel";
const RUN_ANAHTARI: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_ONAY_ANAHTARI: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// Servisin `kapat` isteğinden sonra kendiliğinden kapanması için beklenen süre.
const DUZGUN_KAPANMA_MS: u64 = 5_000;
/// Veri klasörü silmede kilitli dosya için deneme sayısı ve aralığı (toplam ~3 sn).
const SILME_DENEME: u32 = 10;
const SILME_ARALIK: Duration = Duration::from_millis(300);

/// Credential Manager'daki `Cetvel/<ad>` girdilerinin adları (ayarlardaki anahtar ortam adları).
pub fn kimlik_adlari(ayarlar: &Ayarlar) -> Vec<String> {
    let mut adlar = vec!["DEEPSEEK_API_KEY".to_string()];
    if let Some(ad) = ayarlar.deepseek.anahtar_env.clone() {
        adlar.push(ad);
    }
    adlar.retain(|a| !a.trim().is_empty());
    adlar.sort();
    adlar.dedup();
    adlar
}

/// Silinecek `Cetvel/*` kimlikleri: Credential Manager'daki tüm `Cetvel/*` girdileri (ayarda adı
/// geçmeyen eski anahtarlar dahil) ile ayarlardaki adlardan gerçekten var olanların birleşimi.
pub fn mevcut_kimlikler(
    listelenen: &[String],
    ayardan: &[String],
    var_mi: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut kume: BTreeSet<String> = listelenen.iter().cloned().collect();
    for ad in ayardan {
        if var_mi(ad) {
            kume.insert(ad.clone());
        }
    }
    kume.into_iter().collect()
}

/// Masaüstü uygulamasının HKCU `Run` otomatik başlatma kaydı var mı.
fn masaustu_run_kaydi_var() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    [RUN_ANAHTARI, RUN_ONAY_ANAHTARI].iter().any(|anahtar| {
        hkcu.open_subkey(anahtar)
            .ok()
            .and_then(|k| k.get_raw_value(MASAUSTU_RUN_DEGERI).ok())
            .is_some()
    })
}

/// Masaüstü uygulamasının HKCU `Run` otomatik başlatma kaydını siler (yoksa sessiz).
fn masaustu_run_kaydini_sil() {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for anahtar in [RUN_ANAHTARI, RUN_ONAY_ANAHTARI] {
        if let Ok(k) = hkcu.open_subkey_with_flags(anahtar, KEY_SET_VALUE) {
            let _ = k.delete_value(MASAUSTU_RUN_DEGERI);
        }
    }
}

/// Servisi konsolsuz başlatan aracı: `conhost.exe --headless`. `cetvel.exe` konsol alt sistemli
/// olduğundan doğrudan başlatılınca varsayılan terminal Windows Terminal ise görünür bir pencere
/// açılıp kapanır; `SW_HIDE` bunu engelleyemez. Başsız conhost hiç pencere oluşturmaz.
pub(crate) fn konsolsuz_baslatici() -> String {
    let kok = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    format!(r"{kok}\System32\conhost.exe")
}

/// `conhost --headless`'e verilen argümanlar: servis komutu (görev XML'i ile `Run` aynı).
pub(crate) fn servis_arguman(exe_yolu: &Path) -> String {
    format!("--headless \"{}\" servis --gizli", exe_yolu.display())
}

/// Servisin oturum açılışı kaydının (yedek yol) `Run` değeri: pencere açmadan çalışan servis.
pub fn servis_run_komutu(exe_yolu: &Path) -> String {
    format!("\"{}\" {}", konsolsuz_baslatici(), servis_arguman(exe_yolu))
}

/// Servisin oturum açılışında nasıl başlatıldığı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServisKayitYolu {
    /// Görev Zamanlayıcı görevi (`GOREV_ADI`): çökme sonrası yeniden başlatma dahil.
    Gorev,
    /// HKCU `Run` değeri (`SERVIS_RUN_DEGERI`): yalnız oturum açılışında başlar.
    OturumRun,
}

/// Yedeğe düşme kararı (saf): görev oluşturma erişim reddi ya da başka bir hatayla başarısızsa
/// servis HKCU `Run` ile kaydedilir; yönetici yetkisi hiçbir yolda istenmez.
pub(crate) fn kayit_yolu_sec(gorev_sonucu: &Result<(), String>) -> ServisKayitYolu {
    match gorev_sonucu {
        Ok(()) => ServisKayitYolu::Gorev,
        Err(_) => ServisKayitYolu::OturumRun,
    }
}

/// `schtasks` hata metni erişim reddi mi (yalnız açıklama için; karar her hatada yedektir).
pub(crate) fn erisim_reddi_mi(metin: &str) -> bool {
    let k = metin.to_lowercase();
    ["access is denied", "erişim engellendi", "0x80070005"]
        .iter()
        .any(|i| k.contains(i))
}

/// Sistemde bulunan servis kayıtları (salt okuma; `durum`, `durdur`, `kaldir` kullanır).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServisKaydi {
    /// `GOREV_ADI` görevi var.
    pub gorev: bool,
    /// Bulunan eski adlı görevler.
    pub eski_gorevler: Vec<String>,
    /// HKCU `Run\CetvelServis` değeri (varsa içeriği).
    pub run: Option<String>,
    /// Kullanıcı Görev Yöneticisi > Başlangıç'ta bu kaydı devre dışı bırakmış.
    pub run_devre_disi: bool,
}

/// Kayıt özeti (saf; test edilir): `durum`/`durdur` servis kapalıyken bunu yazar.
pub fn servis_kaydi_ozeti(k: &ServisKaydi) -> String {
    let mut parcalar = Vec::new();
    if k.gorev {
        parcalar.push(format!(
            "Görev Zamanlayıcı '{GOREV_ADI}' (oturum açılışında başlar, çökerse yeniden başlatılır)"
        ));
    }
    for ad in &k.eski_gorevler {
        parcalar.push(format!(
            "eski görev '{ad}' (`cetvel kur` yeniler, `cetvel kaldir` siler)"
        ));
    }
    if k.run.is_some() {
        if k.run_devre_disi {
            parcalar.push(format!(
                "oturum açılışı kaydı HKCU\\{RUN_ANAHTARI}\\{SERVIS_RUN_DEGERI} (Görev Yöneticisi > Başlangıç'ta devre dışı)"
            ));
        } else {
            parcalar.push(format!(
                "oturum açılışı kaydı HKCU\\{RUN_ANAHTARI}\\{SERVIS_RUN_DEGERI} (oturum açılışında başlar; çökerse kendiliğinden yeniden başlamaz)"
            ));
        }
    }
    if parcalar.is_empty() {
        "Servis kaydı yok: oturum açılışında başlamaz (`cetvel kur --baslat`).".to_string()
    } else {
        format!("Servis kaydı: {}.", parcalar.join("; "))
    }
}

/// Servis kaydı işlemlerinin soyutlaması (gerçek: `schtasks` + HKCU; testte sahte).
pub(crate) trait KayitIslemleri {
    fn gorev_olustur(&mut self, ad: &str, xml: &str) -> Result<(), String>;
    fn gorev_var_mi(&mut self, ad: &str) -> bool;
    fn gorev_sil(&mut self, ad: &str) -> Result<(), String>;
    fn gorev_calistir(&mut self, ad: &str) -> Result<(), String>;
    fn run_degeri(&mut self) -> Option<String>;
    fn run_devre_disi_mi(&mut self) -> bool;
    fn run_yaz(&mut self, deger: &str) -> Result<(), String>;
    /// `Run` ve `StartupApproved\Run` altındaki değeri siler; yoksa `Ok`.
    fn run_sil(&mut self) -> Result<(), String>;
    fn servis_calisiyor_mu(&mut self) -> bool;
    /// `cetvel.exe servis --gizli` sürecini pencere açmadan, bu süreçten ayrık başlatır.
    fn ayrik_baslat(&mut self) -> Result<(), String>;
    /// `ESKI_GOREV_KLASORU` boşsa siler: `Ok(true)` silindi, `Ok(false)` yok ya da boş değil.
    fn eski_klasoru_sil(&mut self) -> Result<bool, String>;
}

/// Kayıtları sistemden okur.
pub(crate) fn servis_kaydi_topla(k: &mut impl KayitIslemleri) -> ServisKaydi {
    let run = k.run_degeri();
    ServisKaydi {
        gorev: k.gorev_var_mi(GOREV_ADI),
        eski_gorevler: ESKI_GOREV_ADLARI
            .iter()
            .filter(|ad| k.gorev_var_mi(ad))
            .map(|ad| (*ad).to_string())
            .collect(),
        run_devre_disi: run.is_some() && k.run_devre_disi_mi(),
        run,
    }
}

/// Servisin `--baslat` ile nasıl başlatıldığı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaslatmaSonucu {
    ZatenCalisiyor,
    /// `schtasks /Run`.
    Gorevle,
    /// Doğrudan ayrık süreç (yedek yol ya da `/Run` başarısızsa); `gorev_hatasi` `/Run` hatasıdır.
    Dogrudan {
        gorev_hatasi: Option<String>,
    },
}

/// `servis_kaydet` sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KayitSonucu {
    pub yol: ServisKayitYolu,
    /// Görev oluşturma hatası (yedeğe düşüldüyse).
    pub gorev_hatasi: Option<String>,
    pub baslatma: Option<BaslatmaSonucu>,
    /// Kaydı engellemeyen sorunlar (ör. eski görev silinemedi).
    pub uyarilar: Vec<String>,
}

/// Servisi kaydeder: önce kökte görev, olmazsa HKCU `Run` yedeği. Diğer yolun ve eski adlı
/// görevin artığı temizlenir ki servis iki kez kayıtlı kalmasın. `baslat` ise servisi hemen
/// başlatır (görevle; görev yoksa ya da `/Run` başarısızsa ayrık süreçle).
pub(crate) fn servis_kaydet(
    k: &mut impl KayitIslemleri,
    xml: &str,
    run_komutu: &str,
    baslat: bool,
) -> Result<KayitSonucu, String> {
    let gorev = k.gorev_olustur(GOREV_ADI, xml);
    let yol = kayit_yolu_sec(&gorev);
    let gorev_hatasi = gorev.err();
    let mut uyarilar = Vec::new();
    match yol {
        ServisKayitYolu::Gorev => {
            if k.run_degeri().is_some()
                && let Err(e) = k.run_sil()
            {
                uyarilar.push(format!("Eski oturum açılışı kaydı silinemedi: {e}"));
            }
        }
        ServisKayitYolu::OturumRun => {
            k.run_yaz(run_komutu).map_err(|e| {
                format!(
                    "Görev Zamanlayıcı kaydı başarısız ({}) ve oturum açılışı kaydı da yazılamadı: {e}",
                    gorev_hatasi.as_deref().unwrap_or("bilinmeyen hata")
                )
            })?;
            if k.run_devre_disi_mi() {
                uyarilar.push(
                    "Oturum açılışı kaydı Görev Yöneticisi > Başlangıç'ta devre dışı; oturum açılınca başlamaz (oradan etkinleştirin)."
                        .to_string(),
                );
            }
            // `/Create /F` başarısız olduysa önceden kalan aynı adlı görev (ör. yönetici yetkisiyle
            // oluşturulmuş) yerinde durabilir: iki kayıt birlikte kalmasın diye silinmeye çalışılır.
            if k.gorev_var_mi(GOREV_ADI)
                && let Err(e) = k.gorev_sil(GOREV_ADI)
            {
                uyarilar.push(format!(
                    "Önceki '{GOREV_ADI}' görevi silinemedi (oturum açılışı kaydıyla birlikte duruyor; tek örnek kilidi ikinci süreci kapatır): {e}"
                ));
            }
        }
    }
    eski_gorevleri_sil(k, &mut uyarilar, false);

    let baslatma = if !baslat {
        None
    } else if k.servis_calisiyor_mu() {
        Some(BaslatmaSonucu::ZatenCalisiyor)
    } else {
        let gorev_calisma = match yol {
            ServisKayitYolu::Gorev => Some(k.gorev_calistir(GOREV_ADI)),
            ServisKayitYolu::OturumRun => None,
        };
        match gorev_calisma {
            Some(Ok(())) => Some(BaslatmaSonucu::Gorevle),
            diger => {
                k.ayrik_baslat()
                    .map_err(|e| format!("Servis kaydedildi ama başlatılamadı: {e}"))?;
                Some(BaslatmaSonucu::Dogrudan {
                    gorev_hatasi: diger.and_then(Result::err),
                })
            }
        }
    };
    Ok(KayitSonucu {
        yol,
        gorev_hatasi,
        baslatma,
        uyarilar,
    })
}

/// Eski adlı görevleri ve boş kalan eski görev klasörünü siler; sorunlar uyarıya düşer.
fn eski_gorevleri_sil(k: &mut impl KayitIslemleri, uyarilar: &mut Vec<String>, yazdir: bool) {
    for ad in ESKI_GOREV_ADLARI {
        if !k.gorev_var_mi(ad) {
            continue;
        }
        match k.gorev_sil(ad) {
            Ok(()) if yazdir => println!("Görev Zamanlayıcı kaydı '{ad}' silindi."),
            Ok(()) => {}
            Err(e) => uyarilar.push(format!("Eski görev '{ad}' silinemedi: {e}")),
        }
    }
    match k.eski_klasoru_sil() {
        Ok(true) if yazdir => {
            println!("Görev Zamanlayıcı'daki boş '{ESKI_GOREV_KLASORU}' klasörü silindi.");
        }
        Ok(_) => {}
        Err(e) => uyarilar.push(format!(
            "Görev Zamanlayıcı'daki boş '{ESKI_GOREV_KLASORU}' klasörü silinemedi (zararsız kalıntı): {e}"
        )),
    }
}

/// Kaldırmada iki yolun (görev + eski adlı görev + `Run` değeri) kayıtlarını siler; sorunları
/// uyarı olarak döndürür (bir adımın hatası diğerlerini durdurmaz).
pub(crate) fn servis_kayitlarini_sil(k: &mut impl KayitIslemleri) -> Vec<String> {
    let mut uyarilar = Vec::new();
    if k.gorev_var_mi(GOREV_ADI) {
        match k.gorev_sil(GOREV_ADI) {
            Ok(()) => println!("Görev Zamanlayıcı kaydı '{GOREV_ADI}' silindi."),
            Err(e) => uyarilar.push(format!("Görev '{GOREV_ADI}' silinemedi: {e}")),
        }
    }
    eski_gorevleri_sil(k, &mut uyarilar, true);
    let vardi = k.run_degeri().is_some();
    match k.run_sil() {
        Ok(()) if k.run_degeri().is_some() => {
            uyarilar.push("Servisin oturum açılışı kaydı silinemedi".to_string());
        }
        Ok(()) => {
            if vardi {
                println!("Servisin oturum açılışı kaydı silindi.");
            }
        }
        Err(e) => uyarilar.push(format!("Servisin oturum açılışı kaydı silinemedi: {e}")),
    }
    uyarilar
}

/// `schtasks` çıktısını okunur metne çevirir (stderr boşsa stdout).
fn schtasks_hatasi(c: &std::process::Output) -> String {
    let stderr = konsol_metni(&c.stderr).trim().to_string();
    if stderr.is_empty() {
        konsol_metni(&c.stdout).trim().to_string()
    } else {
        stderr
    }
}

/// Konsol araçlarının çıktısını çözer: geçerli UTF-8 aynen alınır; değilse konsol (OEM) kod
/// sayfasıdır (ör. Türkçe Windows'ta 857: "Erişim engellendi"deki `ş` UTF-8 değildir).
pub(crate) fn konsol_metni(baytlar: &[u8]) -> String {
    if let Ok(metin) = std::str::from_utf8(baytlar) {
        return metin.to_string();
    }
    #[cfg(windows)]
    if let Some(metin) = oem_coz(baytlar) {
        return metin;
    }
    String::from_utf8_lossy(baytlar).into_owned()
}

#[cfg(windows)]
fn oem_coz(baytlar: &[u8]) -> Option<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetOEMCP() -> u32;
        fn MultiByteToWideChar(
            kod_sayfasi: u32,
            bayraklar: u32,
            kaynak: *const u8,
            kaynak_uzunluk: i32,
            hedef: *mut u16,
            hedef_uzunluk: i32,
        ) -> i32;
    }
    let uzunluk = i32::try_from(baytlar.len()).ok()?;
    // `schtasks` `CREATE_NO_WINDOW` ile kendi yeni konsolunda çalışır; onun kod sayfası bu sürecin
    // konsolununki (PowerShell'de 65001 olabilir) değil, sistem OEM kod sayfasıdır.
    // SAFETY: argümansız Win32 çağrısı.
    let kod_sayfasi = unsafe { GetOEMCP() };
    // SAFETY: kaynak tampon ve uzunluğu doğrudur; boş hedefle yalnız gereken uzunluk sorulur.
    let gerekli = unsafe {
        MultiByteToWideChar(
            kod_sayfasi,
            0,
            baytlar.as_ptr(),
            uzunluk,
            std::ptr::null_mut(),
            0,
        )
    };
    let mut genis = vec![0u16; usize::try_from(gerekli).ok().filter(|n| *n > 0)?];
    // SAFETY: hedef tampon `gerekli` uzunluğunda ayrıldı.
    let yazilan = unsafe {
        MultiByteToWideChar(
            kod_sayfasi,
            0,
            baytlar.as_ptr(),
            uzunluk,
            genis.as_mut_ptr(),
            gerekli,
        )
    };
    let yazilan = usize::try_from(yazilan).ok().filter(|n| *n > 0)?;
    Some(String::from_utf16_lossy(&genis[..yazilan]))
}

/// Bu sürecin std tutamaçlarını devralınamaz yapar. Aksi halde ayrık başlatılan servis, çağıranın
/// (kurulum programı, terminal) çıktı borusunu miras alır ve çağıran `kur` bitse de borunun
/// kapanmasını süresiz bekler. `kur` hemen ardından çıktığı için kalıcı etkisi yoktur.
#[cfg(windows)]
fn std_tutamaclarini_devralinamaz_yap() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(tur: u32) -> *mut std::ffi::c_void;
        fn SetHandleInformation(tutamac: *mut std::ffi::c_void, maske: u32, bayrak: u32) -> i32;
    }
    const HANDLE_FLAG_INHERIT: u32 = 0x1;
    // STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE ((DWORD)-10, -11, -12).
    for tur in [-10i32, -11, -12] {
        // SAFETY: GetStdHandle geçersiz/boş tutamaç döndürebilir; o durumda atlanır.
        let tutamac = unsafe { GetStdHandle(tur as u32) };
        if !tutamac.is_null() && tutamac as isize != -1 {
            // SAFETY: yalnız miras bayrağı temizlenir; tutamaç bu süreçte geçerli kalır.
            unsafe {
                SetHandleInformation(tutamac, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

/// Gerçek kayıt işlemleri: `schtasks.exe` ve HKCU kayıt defteri.
pub(crate) struct GercekKayit {
    pub exe_yolu: PathBuf,
}

impl GercekKayit {
    fn schtasks(args: &[&str]) -> Result<(), String> {
        let mut komut = Command::new("schtasks.exe");
        komut.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            komut.creation_flags(CREATE_NO_WINDOW);
        }
        let c = komut
            .output()
            .map_err(|e| format!("schtasks başlatılamadı: {e}"))?;
        if c.status.success() {
            Ok(())
        } else {
            Err(schtasks_hatasi(&c))
        }
    }
}

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

impl KayitIslemleri for GercekKayit {
    fn gorev_olustur(&mut self, ad: &str, xml: &str) -> Result<(), String> {
        let gecici_xml =
            std::env::temp_dir().join(format!("cetvel-gorev-{}.xml", std::process::id()));
        let mut utf16_le = vec![0xFFu8, 0xFEu8];
        for u in xml.encode_utf16() {
            utf16_le.extend_from_slice(&u.to_le_bytes());
        }
        atomik_yaz(&gecici_xml, &utf16_le).map_err(|e| e.to_string())?;
        let xml_yolu = gecici_xml.display().to_string();
        let sonuc = Self::schtasks(&["/Create", "/TN", ad, "/XML", &xml_yolu, "/F"]);
        let _ = std::fs::remove_file(&gecici_xml);
        sonuc
    }

    fn gorev_var_mi(&mut self, ad: &str) -> bool {
        Self::schtasks(&["/Query", "/TN", ad]).is_ok()
    }

    fn gorev_sil(&mut self, ad: &str) -> Result<(), String> {
        Self::schtasks(&["/Delete", "/TN", ad, "/F"])
    }

    fn gorev_calistir(&mut self, ad: &str) -> Result<(), String> {
        Self::schtasks(&["/Run", "/TN", ad])
    }

    fn run_degeri(&mut self) -> Option<String> {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(RUN_ANAHTARI)
            .ok()
            .and_then(|k| k.get_value::<String, _>(SERVIS_RUN_DEGERI).ok())
    }

    fn run_devre_disi_mi(&mut self) -> bool {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        // StartupApproved ilk baytı: 02/06 etkin, 03/07 devre dışı (tek sayı = kapalı).
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(RUN_ONAY_ANAHTARI)
            .ok()
            .and_then(|k| k.get_raw_value(SERVIS_RUN_DEGERI).ok())
            .and_then(|d| d.bytes.first().copied())
            .is_some_and(|b| b & 1 == 1)
    }

    fn run_yaz(&mut self, deger: &str) -> Result<(), String> {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let (anahtar, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(RUN_ANAHTARI)
            .map_err(|e| e.to_string())?;
        anahtar
            .set_value(SERVIS_RUN_DEGERI, &deger)
            .map_err(|e| e.to_string())
    }

    fn run_sil(&mut self) -> Result<(), String> {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let mut hata = None;
        for anahtar in [RUN_ANAHTARI, RUN_ONAY_ANAHTARI] {
            if let Ok(k) = hkcu.open_subkey_with_flags(anahtar, KEY_SET_VALUE)
                && let Err(e) = k.delete_value(SERVIS_RUN_DEGERI)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                hata = Some(e.to_string());
            }
        }
        hata.map_or(Ok(()), Err)
    }

    fn servis_calisiyor_mu(&mut self) -> bool {
        cetvel_servis::servis_calisiyor_mu()
    }

    fn ayrik_baslat(&mut self) -> Result<(), String> {
        ayrik_servis_baslat(&self.exe_yolu)
    }

    fn eski_klasoru_sil(&mut self) -> Result<bool, String> {
        gorev_klasorunu_sil(ESKI_GOREV_KLASORU)
    }
}

/// Görev Zamanlayıcı kökündeki boş bir klasörü siler (`schtasks` klasör silemez; COM
/// `ITaskFolder::DeleteFolder`). Yoksa ya da boş değilse `Ok(false)`. COM, çağıranın iş
/// parçacığı modelinden bağımsız olsun diye ayrı iş parçacığında başlatılır.
#[cfg(windows)]
fn gorev_klasorunu_sil(ad: &str) -> Result<bool, String> {
    let ad = ad.to_string();
    std::thread::spawn(move || {
        use windows::Win32::Foundation::{ERROR_DIR_NOT_EMPTY, ERROR_FILE_NOT_FOUND};
        use windows::Win32::System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        };
        use windows::Win32::System::TaskScheduler::{ITaskService, TaskScheduler};
        use windows::Win32::System::Variant::VARIANT;
        use windows::core::{BSTR, HRESULT};

        // SAFETY: bu iş parçacığında COM bir kez başlatılır ve sonda kapatılır.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|e| format!("COM başlatılamadı: {e}"))?;
        // SAFETY: arayüz işaretçileri `windows` sarmalayıcılarıyla yönetilir; boş VARIANT'lar
        // yerel bilgisayara o anki kullanıcıyla bağlanmak demektir.
        let sonuc = unsafe {
            (|| {
                let servis: ITaskService =
                    CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
                let bos = VARIANT::default();
                servis.Connect(&bos, &bos, &bos, &bos)?;
                servis
                    .GetFolder(&BSTR::from("\\"))?
                    .DeleteFolder(&BSTR::from(ad.as_str()), 0)
            })()
        };
        // SAFETY: yukarıdaki başarılı `CoInitializeEx` ile eşleşir.
        unsafe { CoUninitialize() };
        match sonuc {
            Ok(()) => Ok(true),
            Err(e)
                if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
                    || e.code() == HRESULT::from_win32(ERROR_DIR_NOT_EMPTY.0) =>
            {
                Ok(false)
            }
            Err(e) => Err(e.message()),
        }
    })
    .join()
    .map_err(|_| "görev klasörü silme iş parçacığı çöktü".to_string())?
}

#[cfg(not(windows))]
fn gorev_klasorunu_sil(_ad: &str) -> Result<bool, String> {
    Ok(false)
}

/// `cetvel.exe servis --gizli`'yi konsol penceresi açmadan (`CREATE_NO_WINDOW`), bu sürecin
/// konsolundan ve iş nesnesinden ayrık başlatır: kurulum programı ya da terminal kapanınca
/// servis düşmez. İş nesnesi ayrılmaya izin vermiyorsa ayrılma bayrağı olmadan yeniden denenir.
fn ayrik_servis_baslat(exe_yolu: &Path) -> Result<(), String> {
    use std::process::Stdio;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    #[cfg(windows)]
    std_tutamaclarini_devralinamaz_yap();
    let calistir = |bayraklar: u32| {
        let mut komut = Command::new(exe_yolu);
        komut
            .args(["servis", "--gizli"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dizin) = exe_yolu.parent() {
            komut.current_dir(dizin);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            komut.creation_flags(bayraklar);
        }
        #[cfg(not(windows))]
        let _ = bayraklar;
        komut.spawn().map(drop)
    };
    let temel = CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP;
    calistir(temel | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| calistir(temel))
        .map_err(|e| format!("cetvel servis başlatılamadı: {e}"))
}

/// Gerçek sistemdeki servis kayıtları (salt okuma).
pub fn servis_kaydi_oku() -> ServisKaydi {
    let exe_yolu = std::env::current_exe().unwrap_or_default();
    servis_kaydi_topla(&mut GercekKayit { exe_yolu })
}

/// Servis durdurma adımlarının soyutlaması (gerçek: pipe + mutex + süreç; testte sahte).
pub(crate) trait ServisDenetimi {
    fn calisiyor_mu(&mut self) -> bool;
    async fn pid_bul(&mut self) -> Option<u32>;
    async fn kapat_iste(&mut self) -> Result<(), String>;
    async fn bekle(&mut self, ms: u64);
    fn sonlandir(&mut self, pid: u32) -> Result<(), String>;
}

/// Servis durdurma sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurdurmaSonucu {
    /// Servis zaten çalışmıyordu.
    Calismiyordu,
    /// `kapat` isteğiyle süresi içinde kendisi kapandı.
    DuzgunDurdu,
    /// Yanıt vermedi / kapanmadı; doğrulanan süreç sonlandırıldı.
    Sonlandirildi { pid: u32 },
    /// Durdurulamadı (PID bulunamadı ya da sonlandırma reddedildi).
    Durmadi { neden: String },
}

/// `--veri-sil` yalnız servis artık çalışmıyorsa uygulanır (kural saf; bkz. `kaldir_calistir`).
pub(crate) fn veri_silme_guvenli_mi(durdurma: &DurdurmaSonucu) -> bool {
    !matches!(durdurma, DurdurmaSonucu::Durmadi { .. })
}

/// Çalışan servisi önce pipe `kapat` ile durdurur, `zaman_asimi_ms` içinde kapanmazsa doğrulanmış
/// süreci sonlandırır. PID kapatma isteğinden önce bulunur (kapanınca pipe ve dosya gider).
pub(crate) async fn servisi_durdur(
    d: &mut impl ServisDenetimi,
    zaman_asimi_ms: u64,
    adim_ms: u64,
) -> DurdurmaSonucu {
    if !d.calisiyor_mu() {
        return DurdurmaSonucu::Calismiyordu;
    }
    let pid = d.pid_bul().await;
    let kapat_hatasi = d.kapat_iste().await.err();
    let adim = adim_ms.max(1);
    let mut gecen = 0;
    while gecen < zaman_asimi_ms {
        if !d.calisiyor_mu() {
            return DurdurmaSonucu::DuzgunDurdu;
        }
        d.bekle(adim).await;
        gecen += adim;
    }
    if !d.calisiyor_mu() {
        return DurdurmaSonucu::DuzgunDurdu;
    }
    let Some(pid) = pid else {
        return DurdurmaSonucu::Durmadi {
            neden: format!(
                "servis süreci bulunamadı{}",
                kapat_hatasi
                    .map(|h| format!(" (kapat isteği: {h})"))
                    .unwrap_or_default()
            ),
        };
    };
    match d.sonlandir(pid) {
        Ok(()) if !d.calisiyor_mu() => DurdurmaSonucu::Sonlandirildi { pid },
        Ok(()) => DurdurmaSonucu::Durmadi {
            neden: format!("süreç {pid} sonlandırıldı ama servis kilidi hâlâ duruyor"),
        },
        Err(e) => DurdurmaSonucu::Durmadi {
            neden: format!("süreç {pid} sonlandırılamadı: {e}"),
        },
    }
}

/// Gerçek servis denetimi: mutex, `baglanti.json` PID'i / pipe sunucu PID'i, pipe `kapat`.
pub(crate) struct GercekServisDenetimi<'a> {
    pub yollar: &'a CetvelYollar,
}

impl ServisDenetimi for GercekServisDenetimi<'_> {
    fn calisiyor_mu(&mut self) -> bool {
        cetvel_servis::servis_calisiyor_mu()
    }

    async fn pid_bul(&mut self) -> Option<u32> {
        servis_pid_bul(self.yollar).await
    }

    async fn kapat_iste(&mut self) -> Result<(), String> {
        cetvel_servis::servis_pipe_istegi(self.yollar, "kapat", None)
            .await
            .map(|_| ())
    }

    async fn bekle(&mut self, ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    fn sonlandir(&mut self, pid: u32) -> Result<(), String> {
        cetvel_servis::cetvel_surecini_sonlandir(pid, Duration::from_secs(3))
    }
}

/// Servis PID'i: `baglanti.json` (yeni sürüm yazar, süreç canlıysa) → pipe sunucusunun PID'i
/// (eski sürüm ya da dosya silinmişse).
pub(crate) async fn servis_pid_bul(yollar: &CetvelYollar) -> Option<u32> {
    if let Some(pid) = cetvel_depo::baglanti_bilgisi_oku(yollar)
        .ok()
        .and_then(|b| b.pid)
        .filter(|p| cetvel_servis::surec_yasiyor_mu(*p))
    {
        return Some(pid);
    }
    cetvel_servis::pipe_sunucu_pid(&cetvel_servis::servis_pipe_adaylari(yollar))
        .await
        .map(|(_, pid)| pid)
}

/// Görev Zamanlayıcı XML içeriğini üretir (oturum açılışında kullanıcı bağlamında, yönetici
/// gerekmez). `kullanici_sid` (çalışma anında belirteçten okunur, koda yazılmaz) hem oturum açılışı
/// tetikleyicisine hem sorumluya `<UserId>` olarak yazılır: tetikleyici yalnız bu kullanıcının
/// oturum açılışına bağlanır ve standart kullanıcı görevi kaydedebilir. `None` ise `<UserId>`
/// yazılmaz ("herhangi bir kullanıcı"; yalnız yönetici kaydedebilir, kayıt `Run` yedeğine düşer).
/// Eylem `servis --gizli`'dir: `Run` yedeğiyle aynı, kendi konsolu varsa pencere gizlenir.
pub fn gorev_xml_uret(exe_yolu: &Path, kullanici_sid: Option<&str>) -> String {
    let baslatici = xml_kacis(r"%SystemRoot%\System32\conhost.exe");
    let arguman = xml_kacis(&servis_arguman(exe_yolu));
    let kullanici = kullanici_sid
        .map(|sid| format!("\n      <UserId>{}</UserId>", xml_kacis(sid)))
        .unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Cetvel arka plan kota okuma servisi</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>{kullanici}
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">{kullanici}
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{baslatici}</Command>
      <Arguments>{arguman}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn xml_kacis(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Masaüstü uygulamasının (Tauri) yürütülebilir dosya adı; kurulumda `cetvel.exe` ile aynı dizinde.
pub const MASAUSTU_EXE_ADI: &str = "cetvel-masaustu.exe";

/// Başlat menüsü kısayolunun hedefi (masaüstü uygulaması varsa o, yoksa `cetvel durum`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KisayolHedefi {
    pub hedef: PathBuf,
    pub argumanlar: &'static str,
    pub aciklama: &'static str,
}

/// Kısayol hedefini seçer (saf; test edilir): açık `--uygulama` yolu > `cetvel.exe` ile aynı
/// dizindeki masaüstü uygulaması > `cetvel.exe durum`. Kurulumda kısayol Tauri'nin yazdığı
/// `Cetvel.lnk` ile aynı dosyadır; masaüstüne işaret etmesi Windows aramasında "Cetvel"in
/// uygulamayı açmasını, AUMID'i taşıması da servis toast'larının görünmesini sağlar.
pub fn kisayol_hedefi_sec(
    exe_yolu: &Path,
    uygulama: Option<&Path>,
    var_mi: impl Fn(&Path) -> bool,
) -> KisayolHedefi {
    let masaustu = |hedef: PathBuf| KisayolHedefi {
        hedef,
        argumanlar: "",
        aciklama: "Cetvel",
    };
    if let Some(yol) = uygulama {
        return masaustu(yol.to_path_buf());
    }
    let kardes = exe_yolu.with_file_name(MASAUSTU_EXE_ADI);
    if var_mi(&kardes) {
        return masaustu(kardes);
    }
    KisayolHedefi {
        hedef: exe_yolu.to_path_buf(),
        argumanlar: "durum",
        aciklama: "Cetvel Kota Durumu",
    }
}

/// `cetvel kur` seçenekleri.
#[derive(Debug, Clone, Copy)]
pub struct KurSecenekleri<'a> {
    /// Servis görevinin çalıştıracağı `cetvel.exe`.
    pub exe_yolu: &'a Path,
    /// Kısayolun açacağı masaüstü uygulaması (kurulum programı verir).
    pub uygulama: Option<&'a Path>,
    /// Kayıttan sonra servisi oturumu kapatıp açmayı beklemeden başlat (`schtasks /Run`).
    pub baslat: bool,
    pub kuru_calistir: bool,
}

/// `cetvel kur` işlemini yürütür veya `--kuru-calistir` ile yapacaklarını yazdırır.
pub fn kur_calistir(yollar: &CetvelYollar, s: KurSecenekleri<'_>) -> Result<(), String> {
    let sid = cetvel_depo::gecerli_kullanici_sid().ok();
    let xml = gorev_xml_uret(s.exe_yolu, sid.as_deref());
    let kisayol = kisayol_hedefi_sec(s.exe_yolu, s.uygulama, Path::is_file);
    if s.kuru_calistir {
        println!("[KURU CALISTIRMA] Hiçbir sistem değişikliği yapılmayacak.");
        println!("1. Dizin oluşturulacak ve kullanıcı DACL'i uygulanacak:");
        println!("   - {}", yollar.local_kok.display());
        println!("   - {}", yollar.gunluk_dizini().display());
        println!("   - {}", yollar.roaming_kok.display());
        println!(
            "2. Ayar dosyası yoksa varsayılan oluşturulacak: {}",
            yollar.ayarlar_dosyasi().display()
        );
        println!(
            "3. Başlat menüsü kısayolu oluşturulacak (AppUserModelID: {CETVEL_AUMID}; bildirimler bu kimlikle gösterilir): {}",
            yollar.baslat_kisayolu.display()
        );
        println!(
            "   Hedef: {} {}",
            kisayol.hedef.display(),
            kisayol.argumanlar
        );
        println!(
            "4. Görev Zamanlayıcı kökünde görev kaydedilecek (yönetici gerekmez; tetikleyici ve sorumlu bu kullanıcının SID'ine bağlı):"
        );
        println!("   schtasks.exe /Create /TN \"{GOREV_ADI}\" /XML <gecici-gorev.xml> /F");
        println!(
            "   Başarısız olursa (ör. erişim reddi) yedek: HKCU\\{RUN_ANAHTARI}\\{SERVIS_RUN_DEGERI} = {}",
            servis_run_komutu(s.exe_yolu)
        );
        println!(
            "   Kullanılmayan yolun artığı silinecek; eski görev adı da silinecek: {} (boş kalan '{ESKI_GOREV_KLASORU}' görev klasörüyle)",
            ESKI_GOREV_ADLARI.join(", ")
        );
        if s.baslat {
            println!(
                "5. Servis hemen başlatılacak: görev yolunda schtasks.exe /Run /TN \"{GOREV_ADI}\"; yedek yolda (ya da /Run başarısızsa) `cetvel.exe servis --gizli` pencere açmadan ayrık süreç olarak"
            );
        }
        println!("--- Görev XML Önizlemesi ---");
        print!("{xml}");
        return Ok(());
    }

    std::fs::create_dir_all(&yollar.local_kok).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(yollar.gunluk_dizini()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&yollar.roaming_kok).map_err(|e| e.to_string())?;
    kullanici_dacl_uygula(&yollar.local_kok).map_err(|e| e.to_string())?;
    kullanici_dacl_uygula(&yollar.roaming_kok).map_err(|e| e.to_string())?;
    Ayarlar::yoksa_varsayilani_yaz(&yollar.ayarlar_dosyasi()).map_err(|e| e.to_string())?;

    baslat_kisayolu_olustur(&yollar.baslat_kisayolu, kisayol)?;

    let mut kayit = GercekKayit {
        exe_yolu: s.exe_yolu.to_path_buf(),
    };
    let sonuc = servis_kaydet(&mut kayit, &xml, &servis_run_komutu(s.exe_yolu), s.baslat)?;
    match sonuc.yol {
        ServisKayitYolu::Gorev => {
            println!("Cetvel kuruldu: Görev Zamanlayıcı görevi '{GOREV_ADI}' kaydedildi.");
        }
        ServisKayitYolu::OturumRun => {
            let hata = sonuc.gorev_hatasi.as_deref().unwrap_or_default();
            let neden = if erisim_reddi_mi(hata) {
                "erişim reddedildi"
            } else {
                "başarısız"
            };
            println!("Görev Zamanlayıcı kaydı {neden} ({hata}).");
            println!(
                "Cetvel kuruldu: servis oturum açılışında HKCU\\{RUN_ANAHTARI}\\{SERVIS_RUN_DEGERI} ile başlayacak (pencere açmadan; çökerse kendiliğinden yeniden başlamaz)."
            );
        }
    }
    for uyari in &sonuc.uyarilar {
        eprintln!("Uyarı: {uyari}");
    }
    match &sonuc.baslatma {
        None => {}
        Some(BaslatmaSonucu::ZatenCalisiyor) => println!("Cetvel servisi zaten çalışıyor."),
        Some(b) => {
            if let BaslatmaSonucu::Dogrudan {
                gorev_hatasi: Some(h),
            } = b
            {
                println!("schtasks /Run başarısız ({h}); servis doğrudan başlatılıyor.");
            }
            let mut ayakta = servis_ayaga_kalkti_mi(BASLATMA_BEKLEME);
            if !ayakta && *b == BaslatmaSonucu::Gorevle {
                // `/Run` kabul edildi ama görev servisi ayağa kaldırmadı: ayrık süreçle yeniden dene.
                println!("Görev servisi başlatmadı; servis doğrudan başlatılıyor.");
                ayrik_servis_baslat(s.exe_yolu)?;
                ayakta = servis_ayaga_kalkti_mi(BASLATMA_BEKLEME);
            }
            if !ayakta {
                return Err(format!(
                    "Servis kaydedildi ama {} sn içinde başlamadı; `cetvel servis` ile ön planda çalıştırıp hatayı görün (günlük: {}).",
                    BASLATMA_BEKLEME.as_secs(),
                    yollar.gunluk_dizini().display()
                ));
            }
            println!("Cetvel servisi başlatıldı.");
        }
    }
    Ok(())
}

/// `--baslat` sonrası servisin kilidini alması için beklenen en uzun süre.
const BASLATMA_BEKLEME: Duration = Duration::from_secs(8);

/// Servis kilidi (mutex) `sure` içinde görünürse `true`.
fn servis_ayaga_kalkti_mi(sure: Duration) -> bool {
    let adim = Duration::from_millis(200);
    let mut gecen = Duration::ZERO;
    loop {
        if cetvel_servis::servis_calisiyor_mu() {
            return true;
        }
        if gecen >= sure {
            return false;
        }
        std::thread::sleep(adim);
        gecen += adim;
    }
}

/// `cetvel durdur`: çalışan servisi `kaldir` ile aynı yoldan durdurur (pipe `kapat` → 5 sn →
/// doğrulanmış sonlandırma). Kayıtlara ve veriye dokunmaz; kurulum programı yükseltmeden önce
/// `cetvel.exe`'nin kilidini kaldırmak için kullanır.
pub async fn durdur_calistir(yollar: &CetvelYollar) -> DurdurmaSonucu {
    servisi_durdur(&mut GercekServisDenetimi { yollar }, DUZGUN_KAPANMA_MS, 100).await
}

/// Kaldırma planının girdileri: kuru çalıştırma listesi ve gerçek kaldırma aynı veriden üretilir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KaldirPlani {
    /// Bulunan servis kayıtları (görev, eski adlı görev, oturum açılışı `Run` değeri).
    pub servis_kaydi: ServisKaydi,
    pub servis_calisiyor: bool,
    pub servis_pid: Option<u32>,
    pub pipe_adaylari: Vec<String>,
    pub kisayol: PathBuf,
    pub kisayol_var: bool,
    pub run_kaydi_var: bool,
    pub kimlikler: Vec<String>,
    pub veri_sil: bool,
    pub veri_dizinleri: Vec<PathBuf>,
}

/// Kuru çalıştırmada yazdırılan, yapılacak her adımı sayan satırlar (saf; test edilir).
pub fn kaldir_plan_satirlari(p: &KaldirPlani) -> Vec<String> {
    let mut s = Vec::new();
    let mut no = 0;
    let mut adim = |metin: String, s: &mut Vec<String>| {
        no += 1;
        s.push(format!("{no}. {metin}"));
    };
    let k = &p.servis_kaydi;
    adim(
        format!(
            "Görev Zamanlayıcı kaydı silinecek{}: schtasks.exe /Delete /TN \"{GOREV_ADI}\" /F",
            if k.gorev { "" } else { " (şu an yok)" }
        ),
        &mut s,
    );
    for ad in &k.eski_gorevler {
        adim(
            format!(
                "Eski adlı görev silinecek: schtasks.exe /Delete /TN \"{ad}\" /F (boş kalan '{ESKI_GOREV_KLASORU}' görev klasörü de silinir)"
            ),
            &mut s,
        );
    }
    adim(
        format!(
            "Servisin oturum açılışı kaydı silinecek{}: HKCU\\{RUN_ANAHTARI}\\{SERVIS_RUN_DEGERI} ve StartupApproved\\Run{}",
            if k.run.is_some() { "" } else { " (şu an yok)" },
            k.run
                .as_deref()
                .map(|r| format!(" (değer: {r})"))
                .unwrap_or_default()
        ),
        &mut s,
    );
    if p.servis_calisiyor {
        let pid = p
            .servis_pid
            .map_or_else(|| "PID bilinmiyor".to_string(), |pid| format!("PID {pid}"));
        adim(
            format!(
                "Çalışan servis durdurulacak ({pid}): pipe `kapat` isteği, {} sn içinde kapanmazsa süreç sonlandırılacak (yalnız bu kullanıcının cetvel.exe süreci)",
                DUZGUN_KAPANMA_MS / 1000
            ),
            &mut s,
        );
        s.push(format!(
            "   Denenecek pipe adları: {}",
            p.pipe_adaylari.join(", ")
        ));
    } else {
        adim("Servis çalışmıyor; durdurma gerekmez.".to_string(), &mut s);
    }
    adim(
        format!(
            "Başlat menüsü kısayolu silinecek{}: {}",
            if p.kisayol_var { "" } else { " (şu an yok)" },
            p.kisayol.display()
        ),
        &mut s,
    );
    adim(
        format!(
            "Masaüstü otomatik başlatma kaydı silinecek{}: HKCU\\{RUN_ANAHTARI}\\{MASAUSTU_RUN_DEGERI} ve StartupApproved\\Run",
            if p.run_kaydi_var { "" } else { " (şu an yok)" }
        ),
        &mut s,
    );
    if p.veri_sil {
        adim(
            "Veri ve ayar klasörleri silinecek (kilitli dosya için ~3 sn yeniden denenir):"
                .to_string(),
            &mut s,
        );
        for d in &p.veri_dizinleri {
            s.push(format!("   - {}", d.display()));
        }
        if p.kimlikler.is_empty() {
            adim(
                "Credential Manager'da Cetvel/* girdisi yok.".to_string(),
                &mut s,
            );
        } else {
            adim(
                format!(
                    "Credential Manager girdileri silinecek ({} adet):",
                    p.kimlikler.len()
                ),
                &mut s,
            );
            for ad in &p.kimlikler {
                s.push(format!("   - Cetvel/{ad}"));
            }
        }
    } else {
        adim(
            format!(
                "Veri, ayar klasörleri ve {} kayıtlı Cetvel/* anahtarı korunacak (silmek için --veri-sil).",
                p.kimlikler.len()
            ),
            &mut s,
        );
    }
    s.push(
        "Not: masaüstü uygulaması açıksa tepsiden Çıkış ile kapatın (servisi yeniden başlatabilir)."
            .to_string(),
    );
    s
}

/// Kaldırma raporu: tamamlanan adımlar dışında kalan uyarılar (kısmi başarı).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct KaldirRaporu {
    pub uyarilar: Vec<String>,
}

/// Kaldırma planını sistemden toplar (salt okuma).
pub async fn kaldir_plani_topla(
    yollar: &CetvelYollar,
    ayarlar: &Ayarlar,
    veri_sil: bool,
) -> KaldirPlani {
    let servis_calisiyor = cetvel_servis::servis_calisiyor_mu();
    let servis_pid = if servis_calisiyor {
        servis_pid_bul(yollar).await
    } else {
        None
    };
    let listelenen = cetvel_depo::kimlik_listele().unwrap_or_default();
    KaldirPlani {
        servis_kaydi: servis_kaydi_oku(),
        servis_calisiyor,
        servis_pid,
        pipe_adaylari: cetvel_servis::servis_pipe_adaylari(yollar),
        kisayol: yollar.baslat_kisayolu.clone(),
        kisayol_var: yollar.baslat_kisayolu.exists(),
        run_kaydi_var: masaustu_run_kaydi_var(),
        kimlikler: mevcut_kimlikler(
            &listelenen,
            &kimlik_adlari(ayarlar),
            cetvel_depo::kimlik_var_mi,
        ),
        veri_sil,
        veri_dizinleri: vec![yollar.local_kok.clone(), yollar.roaming_kok.clone()],
    }
}

/// `cetvel kaldir`: servis kayıtları (görev + `Run`) → servis durdurma → statusLine → kısayol → Run kaydı → (veri ve
/// kimlikler). Bir adımın sorunu sonrakileri durdurmaz; sorunlar rapordaki uyarılarda toplanır.
pub async fn kaldir_calistir(
    yollar: &CetvelYollar,
    ayarlar: &Ayarlar,
    veri_sil: bool,
    kuru_calistir: bool,
    statusline_birak: bool,
) -> Result<KaldirRaporu, String> {
    let plan = kaldir_plani_topla(yollar, ayarlar, veri_sil).await;
    if kuru_calistir {
        println!("[KURU CALISTIRMA] Hiçbir sistem değişikliği yapılmayacak.");
        for satir in kaldir_plan_satirlari(&plan) {
            println!("{satir}");
        }
        if !statusline_birak && let Err(e) = statusline_geri_al(true) {
            println!("• Claude statusLine denetlenemedi: {e}");
        }
        return Ok(KaldirRaporu::default());
    }

    let mut rapor = KaldirRaporu::default();

    // Kayıtlar önce silinir: sonlandırılan servis görevin RestartOnFailure'ı ile geri gelmesin.
    // Görev (kök + eski alt klasörlü ad) ve oturum açılışı `Run` yedeği birlikte temizlenir.
    let exe_yolu = std::env::current_exe().unwrap_or_default();
    rapor
        .uyarilar
        .extend(servis_kayitlarini_sil(&mut GercekKayit { exe_yolu }));

    let durdurma =
        servisi_durdur(&mut GercekServisDenetimi { yollar }, DUZGUN_KAPANMA_MS, 100).await;
    let veri_silinsin = veri_silme_guvenli_mi(&durdurma);
    match durdurma {
        DurdurmaSonucu::Calismiyordu => {}
        DurdurmaSonucu::DuzgunDurdu => println!("Cetvel servisi durduruldu."),
        DurdurmaSonucu::Sonlandirildi { pid } => println!(
            "Cetvel servisi {} sn içinde kapanmadı; süreç {pid} sonlandırıldı.",
            DUZGUN_KAPANMA_MS / 1000
        ),
        DurdurmaSonucu::Durmadi { neden } => rapor
            .uyarilar
            .push(format!("Cetvel servisi durdurulamadı: {neden}")),
    }

    if !statusline_birak && let Err(e) = statusline_geri_al(false) {
        rapor
            .uyarilar
            .push(format!("Claude statusLine geri alınamadı: {e}"));
    }

    if yollar.baslat_kisayolu.exists()
        && let Err(e) = std::fs::remove_file(&yollar.baslat_kisayolu)
    {
        rapor
            .uyarilar
            .push(format!("Başlat kısayolu silinemedi: {e}"));
    }

    masaustu_run_kaydini_sil();
    if masaustu_run_kaydi_var() {
        rapor
            .uyarilar
            .push("Masaüstü otomatik başlatma kaydı silinemedi".to_string());
    }

    if veri_sil && !veri_silinsin {
        // Canlı servis `baglanti.json`'u ve geçmişi yeniden yazar: anahtarları geri dönüşsüz silip
        // yarım ama çalışan bir kurulum bırakmamak için veri ve kimlik adımı atlanır.
        rapor.uyarilar.push(format!(
            "Servis hâlâ çalıştığı için {} Cetvel/* anahtarı ve veri klasörleri silinmedi. Servisi durdurup (yönetici olarak başlatıldıysa yönetici terminalinden) `cetvel kaldir --veri-sil` komutunu yeniden çalıştırın.",
            plan.kimlikler.len()
        ));
    } else if veri_sil {
        for ad in &plan.kimlikler {
            if let Err(e) = cetvel_depo::kimlik_sil(ad) {
                rapor
                    .uyarilar
                    .push(format!("Cetvel/{ad} kimlik girdisi silinemedi: {e}"));
            }
        }
        for dizin in &plan.veri_dizinleri {
            let sonuc = cetvel_depo::dizini_dayanikli_sil(dizin, SILME_DENEME, SILME_ARALIK);
            if !sonuc.tamam_mi() {
                let liste: Vec<String> = sonuc
                    .silinemeyenler
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect();
                rapor.uyarilar.push(format!(
                    "Kilitli olduğu için silinemedi (masaüstü uygulamasını kapatıp `cetvel kaldir --veri-sil` komutunu yeniden çalıştırın): {}",
                    liste.join(", ")
                ));
            }
        }
    } else if !plan.kimlikler.is_empty() {
        println!(
            "{} kayıtlı Cetvel/* anahtarı korundu (silmek için --veri-sil).",
            plan.kimlikler.len()
        );
    }

    if rapor.uyarilar.is_empty() {
        println!("Cetvel kaldırıldı.");
    } else {
        println!("Cetvel kısmen kaldırıldı; kalan sorunlar aşağıda.");
    }
    Ok(rapor)
}

/// `cetvel kaldir`: Claude `settings.json`'daki Cetvel statusLine sarmalayıcısını özgün komutla
/// (yoksa alanı kaldırarak) geri alır. Önce tarihli yedek alınır; `kuru` ise yalnız fark yazdırılır.
/// Dönüş: yazıldıysa yedek dosya adı.
pub fn statusline_geri_al(kuru: bool) -> Result<Option<String>, String> {
    let Some(yol) = cetvel_servis::bagla::claude_settings_yolu() else {
        return Ok(None);
    };
    statusline_geri_al_dosyada(&yol, kuru, &cetvel_servis::yerel_zaman_etiketi())
}

fn statusline_geri_al_dosyada(
    yol: &Path,
    kuru: bool,
    zaman_etiketi: &str,
) -> Result<Option<String>, String> {
    let ham = match std::fs::read_to_string(yol) {
        Ok(h) => h,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let deger: serde_json::Value = serde_json::from_str(&ham)
        .map_err(|e| format!("settings.json ayrıştırılamadı (dokunulmadı): {e}"))?;
    let Some(plan) = cetvel_servis::bagla::statusline_geri_al_plani(&deger) else {
        return Ok(None);
    };
    let ozgun = plan
        .onceki_komut
        .as_deref()
        .unwrap_or("(yok: statusLine alanı kaldırılır)");
    if kuru {
        println!(
            "• Claude statusLine geri alınacak ({}); özgün komut: {ozgun}",
            yol.display()
        );
        println!("{}", plan.fark_metni);
        return Ok(None);
    }
    let yedek = cetvel_servis::bagla::settings_yedekle_ve_yaz(
        yol,
        Some(&ham),
        &plan.yeni_settings,
        zaman_etiketi,
    )?;
    println!("Claude statusLine geri alındı; özgün komut: {ozgun}");
    if let Some(y) = &yedek {
        println!("  Önceki settings.json yedeği: {y}");
    }
    Ok(yedek)
}

/// Başlat menüsü kısayolunu `IShellLinkW` ile yazar ve [`CETVEL_AUMID`]'i atar (toast için
/// gerekli; PowerShell/WScript.Shell kullanılmaz). COM, ayrı tek iş parçacıklı dairede çalışır.
fn baslat_kisayolu_olustur(kisayol: &Path, hedef: KisayolHedefi) -> Result<(), String> {
    let kisayol = kisayol.to_path_buf();
    std::thread::spawn(move || {
        cetvel_depo::kisayol_olustur(
            &kisayol,
            &KisayolBilgisi {
                hedef: &hedef.hedef,
                argumanlar: hedef.argumanlar,
                aciklama: hedef.aciklama,
                aumid: CETVEL_AUMID,
            },
        )
        .map_err(|e| format!("Başlat kısayolu oluşturulamadı: {e}"))
    })
    .join()
    .map_err(|_| "Başlat kısayolu iş parçacığı çöktü".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kaldirmada_silinecek_kimlik_adlari_ayarlardan_tekil_toplanir() {
        let mut ayar = Ayarlar::default();
        ayar.deepseek.anahtar_env = Some("OZEL_KEY".into());
        let adlar = kimlik_adlari(&ayar);
        assert_eq!(adlar, vec!["DEEPSEEK_API_KEY", "OZEL_KEY"]);
        let adlar = kimlik_adlari(&Ayarlar::default());
        assert_eq!(adlar, vec!["DEEPSEEK_API_KEY"]);
    }

    #[test]
    fn kaldirmada_statusline_ozgun_komuta_doner_ve_yedek_alinir() {
        let d = std::env::temp_dir().join(format!("cetvel-kaldir-sl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let yol = d.join("settings.json");
        let komut = cetvel_servis::bagla::statusline_komutu(
            "C:/x/cetvel.exe",
            Some("bash ~/.claude/statusline.sh"),
        );
        let ham = serde_json::to_string(&serde_json::json!({
            "theme": "dark",
            "statusLine": { "type": "command", "command": komut }
        }))
        .unwrap();
        std::fs::write(&yol, &ham).unwrap();

        // Kuru çalıştırma dosyaya dokunmaz.
        assert!(
            statusline_geri_al_dosyada(&yol, true, "e")
                .unwrap()
                .is_none()
        );
        assert_eq!(std::fs::read_to_string(&yol).unwrap(), ham);

        let yedek = statusline_geri_al_dosyada(&yol, false, "2026-11-16 10.00.00")
            .unwrap()
            .expect("yedek");
        assert_eq!(std::fs::read_to_string(d.join(yedek)).unwrap(), ham);
        let yeni: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
        assert_eq!(
            yeni["statusLine"]["command"],
            "bash ~/.claude/statusline.sh"
        );
        assert_eq!(yeni["theme"], "dark");

        // Artık Cetvel'e ait değil: ikinci çağrı bir şey yapmaz.
        assert!(
            statusline_geri_al_dosyada(&yol, false, "x")
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Sahte servis: `kapanma_turu` kadar bekleme turundan sonra kendiliğinden kapanır
    /// (`None`: hiç kapanmaz); sonlandırılınca kapanır.
    struct SahteServis {
        calisiyor: bool,
        kapanma_turu: Option<u32>,
        tur: u32,
        pid: Option<u32>,
        kapat_istendi: bool,
        sonlandirilan: Option<u32>,
        sonlandirma_reddi: bool,
    }

    impl SahteServis {
        fn yeni(calisiyor: bool, kapanma_turu: Option<u32>, pid: Option<u32>) -> Self {
            Self {
                calisiyor,
                kapanma_turu,
                tur: 0,
                pid,
                kapat_istendi: false,
                sonlandirilan: None,
                sonlandirma_reddi: false,
            }
        }
    }

    impl ServisDenetimi for SahteServis {
        fn calisiyor_mu(&mut self) -> bool {
            self.calisiyor
        }
        async fn pid_bul(&mut self) -> Option<u32> {
            self.pid
        }
        async fn kapat_iste(&mut self) -> Result<(), String> {
            self.kapat_istendi = true;
            Ok(())
        }
        async fn bekle(&mut self, _ms: u64) {
            self.tur += 1;
            if self.kapat_istendi && self.kapanma_turu.is_some_and(|k| self.tur >= k) {
                self.calisiyor = false;
            }
        }
        fn sonlandir(&mut self, pid: u32) -> Result<(), String> {
            if self.sonlandirma_reddi {
                return Err("cetvel.exe değil".into());
            }
            self.sonlandirilan = Some(pid);
            self.calisiyor = false;
            Ok(())
        }
    }

    #[tokio::test]
    async fn servis_durdurma_kapat_zaman_asimi_ve_sonlandirma_akisi() {
        // Çalışmıyor: hiçbir şey yapılmaz.
        let mut s = SahteServis::yeni(false, None, Some(7));
        assert_eq!(
            servisi_durdur(&mut s, 500, 100).await,
            DurdurmaSonucu::Calismiyordu
        );
        assert!(!s.kapat_istendi);

        // `kapat` ile 3. turda kapanır: sonlandırma yok.
        let mut s = SahteServis::yeni(true, Some(3), Some(7));
        assert_eq!(
            servisi_durdur(&mut s, 500, 100).await,
            DurdurmaSonucu::DuzgunDurdu
        );
        assert!(s.kapat_istendi);
        assert_eq!(s.sonlandirilan, None);

        // Yanıtsız servis: zaman aşımında PID sonlandırılır.
        let mut s = SahteServis::yeni(true, None, Some(7));
        assert_eq!(
            servisi_durdur(&mut s, 500, 100).await,
            DurdurmaSonucu::Sonlandirildi { pid: 7 }
        );
        assert_eq!(s.tur, 5);

        // PID bilinmiyorsa sonlandırılmaz, neden bildirilir.
        let mut s = SahteServis::yeni(true, None, None);
        assert!(matches!(
            servisi_durdur(&mut s, 300, 100).await,
            DurdurmaSonucu::Durmadi { .. }
        ));

        // Güvenlik kuralı sonlandırmayı reddederse durmadı sayılır.
        let mut s = SahteServis::yeni(true, None, Some(9));
        s.sonlandirma_reddi = true;
        match servisi_durdur(&mut s, 200, 100).await {
            DurdurmaSonucu::Durmadi { neden } => assert!(neden.contains("cetvel.exe değil")),
            diger => panic!("beklenmeyen: {diger:?}"),
        }
    }

    #[test]
    fn servis_durmadiysa_veri_ve_kimlik_silinmez() {
        assert!(veri_silme_guvenli_mi(&DurdurmaSonucu::Calismiyordu));
        assert!(veri_silme_guvenli_mi(&DurdurmaSonucu::DuzgunDurdu));
        assert!(veri_silme_guvenli_mi(&DurdurmaSonucu::Sonlandirildi {
            pid: 7
        }));
        assert!(!veri_silme_guvenli_mi(&DurdurmaSonucu::Durmadi {
            neden: "süreç sahibi doğrulanamadı".to_string()
        }));
    }

    fn ornek_plan(calisiyor: bool, veri_sil: bool) -> KaldirPlani {
        KaldirPlani {
            servis_kaydi: ServisKaydi {
                gorev: false,
                eski_gorevler: vec![ESKI_GOREV_ADLARI[0].to_string()],
                run: Some(ORNEK_RUN.to_string()),
                run_devre_disi: false,
            },
            servis_calisiyor: calisiyor,
            servis_pid: calisiyor.then_some(4242),
            pipe_adaylari: vec![
                r"\\.\pipe\cetvel-S-1-5-21-1".into(),
                r"\\.\pipe\cetvel".into(),
            ],
            kisayol: PathBuf::from(r"C:\Kisayol\Cetvel.lnk"),
            kisayol_var: true,
            run_kaydi_var: true,
            kimlikler: vec!["DEEPSEEK_API_KEY".into(), "ESKI_KEY".into()],
            veri_sil,
            veri_dizinleri: vec![PathBuf::from(r"C:\L\Cetvel"), PathBuf::from(r"C:\R\Cetvel")],
        }
    }

    #[test]
    fn kuru_calistirma_tum_kaldirma_adimlarini_listeler() {
        let metin = kaldir_plan_satirlari(&ornek_plan(true, true)).join("\n");
        for beklenen in [
            r#"schtasks.exe /Delete /TN "CetvelServis" /F"#,
            "(şu an yok)",
            r#"Eski adlı görev silinecek: schtasks.exe /Delete /TN "Cetvel\CetvelServis""#,
            r"CurrentVersion\Run\CetvelServis",
            "servis --gizli",
            "PID 4242",
            "pipe `kapat`",
            "süreç sonlandırılacak",
            r"\\.\pipe\cetvel-S-1-5-21-1",
            r"C:\Kisayol\Cetvel.lnk",
            "CurrentVersion\\Run\\Cetvel",
            "StartupApproved",
            r"C:\L\Cetvel",
            r"C:\R\Cetvel",
            "Cetvel/DEEPSEEK_API_KEY",
            "Cetvel/ESKI_KEY",
            "2 adet",
        ] {
            assert!(metin.contains(beklenen), "eksik: {beklenen}\n{metin}");
        }
        // Veri silinmeyecekse kimlikler sayılır ama silinmez; çalışmayan servis durdurulmaz.
        let korunan = kaldir_plan_satirlari(&ornek_plan(false, false)).join("\n");
        assert!(korunan.contains("2 kayıtlı Cetvel/* anahtarı korunacak"));
        assert!(korunan.contains("Servis çalışmıyor"));
        assert!(!korunan.contains("Cetvel/ESKI_KEY"));
        assert!(!korunan.contains(r"C:\L\Cetvel"));
    }

    #[test]
    fn mevcut_kimlikler_listelenen_ve_var_olan_ayar_adlarinin_birlesimi() {
        let liste = vec!["ESKI_KEY".to_string(), "DEEPSEEK_API_KEY".to_string()];
        let ayar = vec!["DEEPSEEK_API_KEY".to_string(), "OZEL".to_string()];
        let sonuc = mevcut_kimlikler(&liste, &ayar, |ad| ad == "OZEL");
        assert_eq!(sonuc, vec!["DEEPSEEK_API_KEY", "ESKI_KEY", "OZEL"]);
    }

    #[test]
    fn kisayol_hedefi_uygulama_kardes_ve_yedek_sirasini_izler() {
        let cli = Path::new(r"C:\Kur\Cetvel\cetvel.exe");
        let acik = Path::new(r"D:\Baska\cetvel-masaustu.exe");
        let h = kisayol_hedefi_sec(cli, Some(acik), |_| false);
        assert_eq!(h.hedef, acik);
        assert_eq!(h.argumanlar, "");

        let kardes = cli.with_file_name(MASAUSTU_EXE_ADI);
        let h = kisayol_hedefi_sec(cli, None, |p| p == kardes.as_path());
        assert_eq!(h.hedef, kardes);
        assert_eq!(h.argumanlar, "");
        assert_eq!(h.aciklama, "Cetvel");

        let h = kisayol_hedefi_sec(cli, None, |_| false);
        assert_eq!(h.hedef, cli);
        assert_eq!(h.argumanlar, "durum");
    }

    /// Sahte kayıt sistemi: görevler, `Run` değeri ve servis süreci bellekte tutulur.
    #[derive(Default)]
    struct SahteKayit {
        gorevler: BTreeSet<String>,
        run: Option<String>,
        run_devre_disi: bool,
        olusturma_hatasi: Option<String>,
        silme_reddi: BTreeSet<String>,
        run_calistir_hatasi: bool,
        run_yazma_hatasi: bool,
        calisiyor: bool,
        ayrik_baslatma: u32,
        gorev_calistirma: u32,
        /// Görev Zamanlayıcı'da eski görev klasörü var mı (içindeki görevler `gorevler`de).
        eski_klasor: bool,
        klasor_silme_reddi: bool,
    }

    impl KayitIslemleri for SahteKayit {
        fn gorev_olustur(&mut self, ad: &str, xml: &str) -> Result<(), String> {
            assert!(xml.contains("<LogonTrigger>"));
            if let Some(h) = &self.olusturma_hatasi {
                return Err(h.clone());
            }
            self.gorevler.insert(ad.to_string());
            Ok(())
        }
        fn gorev_var_mi(&mut self, ad: &str) -> bool {
            self.gorevler.contains(ad)
        }
        fn gorev_sil(&mut self, ad: &str) -> Result<(), String> {
            if self.silme_reddi.contains(ad) {
                return Err("ERROR: Erişim engellendi.".into());
            }
            self.gorevler.remove(ad);
            Ok(())
        }
        fn gorev_calistir(&mut self, _ad: &str) -> Result<(), String> {
            self.gorev_calistirma += 1;
            if self.run_calistir_hatasi {
                return Err("görev başlatılamadı".into());
            }
            self.calisiyor = true;
            Ok(())
        }
        fn run_degeri(&mut self) -> Option<String> {
            self.run.clone()
        }
        fn run_devre_disi_mi(&mut self) -> bool {
            self.run_devre_disi
        }
        fn run_yaz(&mut self, deger: &str) -> Result<(), String> {
            if self.run_yazma_hatasi {
                return Err("kayıt defteri yazılamadı".into());
            }
            self.run = Some(deger.to_string());
            Ok(())
        }
        fn run_sil(&mut self) -> Result<(), String> {
            self.run = None;
            self.run_devre_disi = false;
            Ok(())
        }
        fn servis_calisiyor_mu(&mut self) -> bool {
            self.calisiyor
        }
        fn ayrik_baslat(&mut self) -> Result<(), String> {
            self.ayrik_baslatma += 1;
            self.calisiyor = true;
            Ok(())
        }
        fn eski_klasoru_sil(&mut self) -> Result<bool, String> {
            let onek = format!("{ESKI_GOREV_KLASORU}\\");
            if !self.eski_klasor || self.gorevler.iter().any(|g| g.starts_with(&onek)) {
                return Ok(false);
            }
            if self.klasor_silme_reddi {
                return Err("Erişim engellendi.".into());
            }
            self.eski_klasor = false;
            Ok(true)
        }
    }

    const ORNEK_RUN: &str = r#""C:\Kur\cetvel.exe" servis --gizli"#;
    const ORNEK_SID: &str = "S-1-5-21-1-2-3-1001";

    #[test]
    fn gorev_adi_kok_klasorde_eski_alt_klasorlu_ad_taninir() {
        // Görev kökte durur; önceki sürümün alt klasörlü adı tanınır (ve silinir).
        assert!(!GOREV_ADI.contains('\\'));
        assert_eq!(GOREV_ADI, "CetvelServis");
        assert!(ESKI_GOREV_ADLARI.contains(&r"Cetvel\CetvelServis"));
        assert!(!ESKI_GOREV_ADLARI.contains(&GOREV_ADI));
        let run = servis_run_komutu(Path::new(r"C:\Program Files\Cetvel\cetvel.exe"));
        assert!(run.starts_with('"') && run.contains(r"\System32\conhost.exe"), "{run}");
        assert!(
            run.ends_with(r#"" --headless "C:\Program Files\Cetvel\cetvel.exe" servis --gizli"#),
            "{run}"
        );
    }

    #[test]
    fn gorev_olusturma_hatasinda_oturum_run_yedegine_dusulur() {
        assert_eq!(kayit_yolu_sec(&Ok(())), ServisKayitYolu::Gorev);
        assert_eq!(
            kayit_yolu_sec(&Err("ERROR: Erişim engellendi.".into())),
            ServisKayitYolu::OturumRun
        );
        assert_eq!(
            kayit_yolu_sec(&Err("ERROR: The task XML is malformed.".into())),
            ServisKayitYolu::OturumRun
        );
        assert!(erisim_reddi_mi("ERROR: Erişim engellendi."));
        assert!(erisim_reddi_mi("ERROR: Access is denied."));
        assert!(!erisim_reddi_mi("ERROR: The task XML is malformed."));

        // schtasks çıktısı UTF-8 değilse (OEM kod sayfası) bozulmadan ya da en kötü kayıplı çözülür.
        assert_eq!(
            konsol_metni("ERROR: Erişim engellendi.".as_bytes()),
            "ERROR: Erişim engellendi."
        );
        let oem = konsol_metni(b"ERROR: Eri\x9fim engellendi.");
        assert!(oem.starts_with("ERROR: Eri") && oem.ends_with("im engellendi."));
    }

    #[test]
    fn gorev_yolu_kaydeder_eski_gorevi_ve_run_artigini_siler_gorevle_baslatir() {
        let mut k = SahteKayit {
            gorevler: [ESKI_GOREV_ADLARI[0].to_string()].into(),
            run: Some(ORNEK_RUN.into()),
            eski_klasor: true,
            ..Default::default()
        };
        let xml = gorev_xml_uret(Path::new("c.exe"), Some(ORNEK_SID));
        let s = servis_kaydet(&mut k, &xml, ORNEK_RUN, true).unwrap();
        assert_eq!(s.yol, ServisKayitYolu::Gorev);
        assert_eq!(s.gorev_hatasi, None);
        assert_eq!(s.baslatma, Some(BaslatmaSonucu::Gorevle));
        assert!(s.uyarilar.is_empty(), "{:?}", s.uyarilar);
        assert_eq!(k.gorevler, [GOREV_ADI.to_string()].into());
        assert!(!k.eski_klasor, "eski görev silinince boş klasör de silinir");
        assert_eq!(k.run, None);
        assert_eq!(k.ayrik_baslatma, 0);
    }

    #[test]
    fn yedek_yolda_onceki_ayni_adli_gorev_silinir_silinemezse_uyarilir() {
        // `/Create /F` reddedildi ama eski bir `CetvelServis` görevi duruyor: iki kayıt kalmasın.
        let mut k = SahteKayit {
            gorevler: [GOREV_ADI.to_string()].into(),
            olusturma_hatasi: Some("ERROR: Erişim engellendi.".into()),
            ..Default::default()
        };
        let s = servis_kaydet(&mut k, "<LogonTrigger>", ORNEK_RUN, false).unwrap();
        assert_eq!(s.yol, ServisKayitYolu::OturumRun);
        assert!(k.gorevler.is_empty(), "{:?}", k.gorevler);
        assert_eq!(k.run.as_deref(), Some(ORNEK_RUN));
        assert!(s.uyarilar.is_empty(), "{:?}", s.uyarilar);

        // Silinemiyorsa (ör. yönetici görevi) kayıt yine tamamlanır, tutarsızlık uyarıyla söylenir.
        let mut k = SahteKayit {
            gorevler: [GOREV_ADI.to_string()].into(),
            olusturma_hatasi: Some("ERROR: Erişim engellendi.".into()),
            silme_reddi: [GOREV_ADI.to_string()].into(),
            ..Default::default()
        };
        let s = servis_kaydet(&mut k, "<LogonTrigger>", ORNEK_RUN, false).unwrap();
        assert_eq!(k.run.as_deref(), Some(ORNEK_RUN));
        let uyarilar = s.uyarilar.join("\n");
        assert!(
            uyarilar.contains("'CetvelServis' görevi silinemedi"),
            "{uyarilar}"
        );
    }

    #[test]
    fn eski_gorev_klasoru_bossa_silinir_doluysa_ya_da_reddedilirse_kalir() {
        // Eski görev silinemezse klasör boş değildir: dokunulmaz, yalnız görevin uyarısı çıkar.
        let mut k = SahteKayit {
            gorevler: [ESKI_GOREV_ADLARI[0].to_string()].into(),
            silme_reddi: [ESKI_GOREV_ADLARI[0].to_string()].into(),
            eski_klasor: true,
            ..Default::default()
        };
        let uyarilar = servis_kayitlarini_sil(&mut k);
        assert_eq!(uyarilar.len(), 1, "{uyarilar:?}");
        assert!(k.eski_klasor);

        // Klasör silme reddi zararsız kalıntı olarak uyarılır; kalan temizlik sürer.
        let mut k = SahteKayit {
            eski_klasor: true,
            klasor_silme_reddi: true,
            run: Some(ORNEK_RUN.into()),
            ..Default::default()
        };
        let uyarilar = servis_kayitlarini_sil(&mut k);
        assert_eq!(uyarilar.len(), 1, "{uyarilar:?}");
        assert!(uyarilar[0].contains("zararsız kalıntı"), "{uyarilar:?}");
        assert_eq!(k.run, None);

        // Eski görev yokken kalan boş klasör de temizlenir.
        let mut k = SahteKayit {
            eski_klasor: true,
            ..Default::default()
        };
        assert!(servis_kayitlarini_sil(&mut k).is_empty());
        assert!(!k.eski_klasor);
    }

    #[test]
    fn erisim_reddinde_run_yazilir_ve_servis_ayrik_baslatilir() {
        let mut k = SahteKayit {
            gorevler: [ESKI_GOREV_ADLARI[0].to_string()].into(),
            olusturma_hatasi: Some("ERROR: Erişim engellendi.".into()),
            // Eski görev de yönetici olmadan silinemiyor: kayıt yine tamamlanır, uyarı verilir.
            silme_reddi: [ESKI_GOREV_ADLARI[0].to_string()].into(),
            run_devre_disi: true,
            ..Default::default()
        };
        let s = servis_kaydet(&mut k, "<LogonTrigger>", ORNEK_RUN, true).unwrap();
        assert_eq!(s.yol, ServisKayitYolu::OturumRun);
        assert_eq!(s.gorev_hatasi.as_deref(), Some("ERROR: Erişim engellendi."));
        assert_eq!(
            s.baslatma,
            Some(BaslatmaSonucu::Dogrudan { gorev_hatasi: None })
        );
        assert_eq!(k.run.as_deref(), Some(ORNEK_RUN));
        assert_eq!(k.gorev_calistirma, 0, "yedek yolda schtasks /Run denenmez");
        assert_eq!(k.ayrik_baslatma, 1);
        let uyarilar = s.uyarilar.join("\n");
        assert!(uyarilar.contains("devre dışı"), "{uyarilar}");
        assert!(uyarilar.contains(r"Cetvel\CetvelServis"), "{uyarilar}");

        // Run da yazılamazsa kayıt hatası iki nedeni de söyler; servis başlatılmaz.
        let mut k = SahteKayit {
            olusturma_hatasi: Some("ERROR: Erişim engellendi.".into()),
            run_yazma_hatasi: true,
            ..Default::default()
        };
        let h = servis_kaydet(&mut k, "<LogonTrigger>", ORNEK_RUN, true).unwrap_err();
        assert!(
            h.contains("Erişim engellendi") && h.contains("yazılamadı"),
            "{h}"
        );
        assert_eq!(k.ayrik_baslatma, 0);
    }

    #[test]
    fn baslatma_calisan_serviste_atlanir_run_hatasinda_ayrik_surece_duser() {
        let mut k = SahteKayit {
            calisiyor: true,
            ..Default::default()
        };
        let s = servis_kaydet(&mut k, "<LogonTrigger>", ORNEK_RUN, true).unwrap();
        assert_eq!(s.baslatma, Some(BaslatmaSonucu::ZatenCalisiyor));
        assert_eq!(k.gorev_calistirma + k.ayrik_baslatma, 0);

        let mut k = SahteKayit {
            run_calistir_hatasi: true,
            ..Default::default()
        };
        let s = servis_kaydet(&mut k, "<LogonTrigger>", ORNEK_RUN, true).unwrap();
        assert_eq!(s.yol, ServisKayitYolu::Gorev);
        assert_eq!(
            s.baslatma,
            Some(BaslatmaSonucu::Dogrudan {
                gorev_hatasi: Some("görev başlatılamadı".into())
            })
        );
        assert_eq!(k.ayrik_baslatma, 1);

        // `--baslat` yoksa hiçbir şey başlatılmaz.
        let mut k = SahteKayit::default();
        let s = servis_kaydet(&mut k, "<LogonTrigger>", ORNEK_RUN, false).unwrap();
        assert_eq!(s.baslatma, None);
        assert!(!k.calisiyor);
    }

    #[test]
    fn kaldirma_iki_yolun_ve_eski_gorevin_kayitlarini_temizler() {
        let mut k = SahteKayit {
            gorevler: [GOREV_ADI.to_string(), ESKI_GOREV_ADLARI[0].to_string()].into(),
            run: Some(ORNEK_RUN.into()),
            run_devre_disi: true,
            ..Default::default()
        };
        let once = servis_kaydi_topla(&mut k);
        assert!(once.gorev && once.run.is_some() && once.run_devre_disi);
        assert_eq!(once.eski_gorevler, vec![ESKI_GOREV_ADLARI[0].to_string()]);

        let uyarilar = servis_kayitlarini_sil(&mut k);
        assert!(uyarilar.is_empty(), "{uyarilar:?}");
        assert!(k.gorevler.is_empty());
        assert_eq!(k.run, None);
        assert_eq!(servis_kaydi_topla(&mut k), ServisKaydi::default());

        // Bir görev silinemese de Run yine temizlenir; sorun uyarıya düşer.
        let mut k = SahteKayit {
            gorevler: [ESKI_GOREV_ADLARI[0].to_string()].into(),
            silme_reddi: [ESKI_GOREV_ADLARI[0].to_string()].into(),
            run: Some(ORNEK_RUN.into()),
            ..Default::default()
        };
        let uyarilar = servis_kayitlarini_sil(&mut k);
        assert_eq!(uyarilar.len(), 1, "{uyarilar:?}");
        assert!(uyarilar[0].contains("Erişim engellendi"));
        assert_eq!(k.run, None);

        // Hiç kayıt yoksa sessiz ve hatasız.
        assert!(servis_kayitlarini_sil(&mut SahteKayit::default()).is_empty());
    }

    #[test]
    fn kayit_ozeti_iki_yolu_ve_yoklugu_dogru_soyler() {
        let yok = servis_kaydi_ozeti(&ServisKaydi::default());
        assert!(yok.contains("Servis kaydı yok"), "{yok}");

        let gorev = servis_kaydi_ozeti(&ServisKaydi {
            gorev: true,
            ..Default::default()
        });
        assert!(gorev.contains("'CetvelServis'") && gorev.contains("yeniden başlatılır"));

        let run = servis_kaydi_ozeti(&ServisKaydi {
            run: Some(ORNEK_RUN.into()),
            ..Default::default()
        });
        assert!(run.contains(r"Run\CetvelServis") && run.contains("yeniden başlamaz"));

        let kapali = servis_kaydi_ozeti(&ServisKaydi {
            run: Some(ORNEK_RUN.into()),
            run_devre_disi: true,
            eski_gorevler: vec![ESKI_GOREV_ADLARI[0].to_string()],
            ..Default::default()
        });
        assert!(kapali.contains("devre dışı") && kapali.contains("eski görev"));
    }

    #[test]
    fn gorev_xml_uret_gecerli_xml_ve_kacis_icermelidir() {
        let xml = gorev_xml_uret(
            Path::new(r"C:\Program Files\Cetvel & Co\cetvel.exe"),
            Some(ORNEK_SID),
        );
        assert!(xml.contains("Cetvel &amp; Co\\cetvel.exe"));
        // Görev de `Run` yedeği gibi başsız conhost üzerinden, hiç pencere açmadan çalışır.
        assert!(xml.contains(r"<Command>%SystemRoot%\System32\conhost.exe</Command>"));
        assert!(xml.contains(
            "<Arguments>--headless &quot;C:\\Program Files\\Cetvel &amp; Co\\cetvel.exe&quot; servis --gizli</Arguments>"
        ));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
    }

    #[test]
    fn gorev_xml_tetikleyici_ve_sorumlu_kullanici_sidine_baglanir() {
        // `<UserId>`'siz oturum açılışı tetikleyicisi "herhangi bir kullanıcı" demektir ve
        // standart kullanıcıya reddedilir: SID hem tetikleyicide hem sorumluda olmalı.
        let xml = gorev_xml_uret(Path::new("c.exe"), Some(ORNEK_SID));
        let kimlik = format!("<UserId>{ORNEK_SID}</UserId>");
        assert_eq!(xml.matches(&kimlik).count(), 2, "{xml}");
        let tetik = &xml[xml.find("<LogonTrigger>").unwrap()..xml.find("</LogonTrigger>").unwrap()];
        assert!(tetik.contains(&kimlik), "{tetik}");
        let sorumlu = &xml[xml.find("<Principal ").unwrap()..xml.find("</Principal>").unwrap()];
        assert!(sorumlu.contains(&kimlik), "{sorumlu}");
        // Şemada `UserId`, `Principal` içinde `LogonType`'tan önce gelir.
        assert!(sorumlu.find("<UserId>").unwrap() < sorumlu.find("<LogonType>").unwrap());

        // SID okunamazsa `<UserId>` yazılmaz (kayıt reddedilirse `Run` yedeğine düşülür).
        assert!(!gorev_xml_uret(Path::new("c.exe"), None).contains("<UserId>"));
    }
}
