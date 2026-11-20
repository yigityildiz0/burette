mod kurulum;

use std::path::PathBuf;
use std::process::ExitCode;

use cetvel_depo::{Ayarlar, CetvelYollar, atomik_yaz_json};
use cetvel_model::{
    AnlikV2, OrkestraUyumV1, UtcZaman, anlik_dogrula, anlik_v2_json_schema,
    orkestra_uyum_v1_json_schema, uyum_dogrula,
};
use cetvel_servis::{
    OkuSecenekleri, anlik_ve_uyum_yayinla_ayarli, ayarlari_kurtarmali_yukle, servis_calisiyor_mu,
    servis_calistir, servis_pipe_istegi, tek_sefer_oku_secenekli,
};
use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "cetvel",
    version,
    about = "Cetvel — Yerel yapay zeka kota ölçüm ve yayın CLI'ı"
)]
struct Cli {
    /// Durumu JSON biçiminde yazdır
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    komut: Option<Komutlar>,
}

#[derive(Subcommand, Debug)]
enum Komutlar {
    /// Kayıtlı anlık kota durumunu gösterir (varsayılan)
    Durum(DurumArgs),
    /// Bağlayıcılardan tek seferlik okuma yapıp Anlık v2 JSON çıktısı verir
    Oku(OkuArgs),
    /// Arka plan zamanlayıcı servisini başlatır
    Servis(ServisArgs),
    /// MCP stdio (JSON-RPC 2.0) sunucusunu başlatır
    Mcp,
    /// Servis kaydını (Görev Zamanlayıcı ya da oturum açılışı yedeği) ve kullanıcı dizinlerini kurar
    Kur(KurArgs),
    /// Servis kayıtlarını (görev ve oturum açılışı yedeği) ve kısayolu kaldırır
    Kaldir(KaldirArgs),
    /// Çalışan servisi durdurur (kayıtlara ve veriye dokunmaz)
    Durdur,
    /// Şema doğrulaması ve isteğe bağlı JSON Schema ihracı yapar
    Dogrula(DogrulaArgs),
    /// Platform köprüleri (ör. Claude Code statusLine zinciri)
    Kopru(KopruArgs),
}

#[derive(Args, Debug)]
struct KopruArgs {
    #[command(subcommand)]
    tur: KopruTuru,
}

#[derive(Subcommand, Debug)]
enum KopruTuru {
    /// Claude Code statusLine komutu: stdin'i önceki statusLine komutuna aynen iletir, çıktısını
    /// yazdırır ve stdin'de `rate_limits` varsa servise (pipe `kopru.claude`) gönderir.
    Statusline(StatuslineArgs),
}

#[derive(Args, Debug)]
struct StatuslineArgs {
    /// Zincirlenen önceki statusLine komutu (onaltılık kodlu; Bağla ekranı üretir)
    #[arg(long)]
    onceki_hex: Option<String>,
}

#[derive(Args, Debug, Default)]
struct DurumArgs {
    /// Servis kapalıyken tanı amacıyla bağlayıcıları doğrudan bir kez okur
    #[arg(long)]
    dogrudan: bool,
}

#[derive(Args, Debug)]
struct OkuArgs {
    /// Yalnız Codex bağlayıcısını oku
    #[arg(long)]
    codex: bool,
    /// Antigravity (`agy -p /usage`) okumasını iste (ayarlar.toml iznine tabidir)
    #[arg(long)]
    agy: bool,
    /// DeepSeek bakiye bağlayıcısını oku
    #[arg(long)]
    deepseek: bool,
    /// Okunan sonucu anlik.json ve orkestra-uyum.json dosyalarına da atomik yaz
    #[arg(long)]
    yaz: bool,
}

#[derive(Args, Debug)]
struct ServisArgs {
    /// Yalnız bir okuma ve yayın turu çalıştırıp çıkar
    #[arg(long)]
    tek_tur: bool,
    /// Pencere göstermeden çalışır: Görev Zamanlayıcı görevi ve oturum açılışı kaydı (HKCU `Run`)
    /// bu bayrakla başlatır; süreç kendi konsoluyla açıldıysa pencere gizlenir ve konsol bırakılır.
    /// (Varsayılan terminal Windows Terminal ise pencere sahte konsol penceresidir; gizleme
    /// etkisiz kalabilir, `FreeConsole` sonrası kapanır.)
    #[arg(long)]
    gizli: bool,
}

#[derive(Args, Debug)]
struct KurArgs {
    /// Gerçek sistem değişikliği yapmadan atılacak adımları yazdırır
    #[arg(long)]
    kuru_calistir: bool,
    /// Başlat menüsü kısayolunun açacağı masaüstü uygulaması (varsayılan: yanındaki
    /// `cetvel-masaustu.exe`, yoksa `cetvel durum`)
    #[arg(long, value_name = "YOL")]
    uygulama: Option<PathBuf>,
    /// Kayıttan sonra servisi hemen başlatır (oturumu kapatıp açmak gerekmez)
    #[arg(long)]
    baslat: bool,
}

#[derive(Args, Debug)]
struct KaldirArgs {
    /// Gerçek sistem değişikliği yapmadan kaldırılacakları yazdırır
    #[arg(long)]
    kuru_calistir: bool,
    /// %LOCALAPPDATA%\Cetvel ve %APPDATA%\Cetvel klasörlerini de siler
    #[arg(long)]
    veri_sil: bool,
    /// Claude `settings.json` statusLine zincirini geri almadan bırakır (varsayılan: geri alınır)
    #[arg(long)]
    statusline_birak: bool,
}

#[derive(Args, Debug)]
struct DogrulaArgs {
    /// Doğrulanacak belirli bir JSON dosyası
    #[arg(long)]
    dosya: Option<PathBuf>,
    /// Üretilen v2/v1 JSON şema dosyalarının yazılacağı dizin
    #[arg(long)]
    sema_yaz: Option<PathBuf>,
}

// Dosya ve SQLite işleri sırasında HTTP/pipe yanıtları da ilerleyebilsin.
#[tokio::main(worker_threads = 2)]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let yollar = match CetvelYollar::ortamdan() {
        Ok(y) => y,
        Err(e) => {
            eprintln!("Ortam yolları çözülemedi: {e}");
            return ExitCode::from(1);
        }
    };
    let yukleme = ayarlari_kurtarmali_yukle(&yollar);
    if let Some(aciklama) = yukleme.kurtarma.aciklama() {
        eprintln!("Uyarı: {aciklama}");
    }
    let ayarlar: Ayarlar = yukleme.ayarlar;

    match cli.komut {
        None => durum_komutu(&yollar, &ayarlar, DurumArgs::default(), cli.json).await,
        Some(Komutlar::Durum(args)) => durum_komutu(&yollar, &ayarlar, args, cli.json).await,
        Some(Komutlar::Oku(args)) => oku_komutu(&yollar, &ayarlar, args).await,
        Some(Komutlar::Servis(args)) => servis_komutu(&yollar, args).await,
        Some(Komutlar::Mcp) => mcp_komutu(yollar).await,
        Some(Komutlar::Kur(args)) => kur_komutu(&yollar, args),
        Some(Komutlar::Kaldir(args)) => kaldir_komutu(&yollar, &ayarlar, args).await,
        Some(Komutlar::Durdur) => durdur_komutu(&yollar).await,
        Some(Komutlar::Dogrula(args)) => dogrula_komutu(&yollar, args),
        Some(Komutlar::Kopru(KopruArgs {
            tur: KopruTuru::Statusline(args),
        })) => statusline_komutu(args).await,
    }
}

async fn durum_komutu(
    yollar: &CetvelYollar,
    ayarlar: &Ayarlar,
    args: DurumArgs,
    json_cikti: bool,
) -> ExitCode {
    if args.dogrudan {
        let anlik = tek_sefer_oku_secenekli(ayarlar, &OkuSecenekleri::default()).await;
        if json_cikti {
            println!(
                "{}",
                serde_json::to_string_pretty(&anlik).unwrap_or_default()
            );
        } else {
            tablo_yazdir(&anlik, true, true);
        }
        return ExitCode::SUCCESS;
    }

    let servis_aktif = servis_calisiyor_mu();
    if servis_aktif
        && let Ok(deger) = servis_pipe_istegi(yollar, "anlik.al", None).await
        && let Ok(anlik) = serde_json::from_value::<AnlikV2>(deger)
    {
        if json_cikti {
            println!(
                "{}",
                serde_json::to_string_pretty(&anlik).unwrap_or_default()
            );
        } else {
            tablo_yazdir(&anlik, true, false);
        }
        return ExitCode::SUCCESS;
    }

    let anlik_yol = yollar.anlik_dosyasi();
    if anlik_yol.exists() {
        match std::fs::read_to_string(&anlik_yol)
            .ok()
            .and_then(|s| serde_json::from_str::<AnlikV2>(&s).ok())
        {
            Some(anlik) => {
                if json_cikti {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&anlik).unwrap_or_default()
                    );
                } else {
                    tablo_yazdir(&anlik, servis_aktif, false);
                }
                if !servis_aktif {
                    kayit_ozeti_yaz();
                }
                if servis_aktif {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(3)
                }
            }
            None => {
                eprintln!(
                    "Anlık dosyası okunamadı veya bozuk: {}",
                    anlik_yol.display()
                );
                ExitCode::from(if servis_aktif { 1 } else { 3 })
            }
        }
    } else {
        if !servis_aktif {
            eprintln!("Servis çalışmıyor ve henüz anlık dosyası yok.");
            kayit_ozeti_yaz();
            eprintln!("Başlatmak için: cetvel servis  |  Tek sefer okuma: cetvel oku");
            return ExitCode::from(3);
        }
        eprintln!("Servis çalışıyor ancak henüz anlık yazılmadı.");
        ExitCode::SUCCESS
    }
}

/// Servis çalışmıyorken nasıl kayıtlı olduğunu (görev / oturum açılışı `Run` / yok) yazar; stdout
/// JSON çıktısını bozmamak için stderr'e.
fn kayit_ozeti_yaz() {
    eprintln!(
        "{}",
        kurulum::servis_kaydi_ozeti(&kurulum::servis_kaydi_oku())
    );
}

async fn oku_komutu(yollar: &CetvelYollar, ayarlar: &Ayarlar, args: OkuArgs) -> ExitCode {
    let anlik = tek_sefer_oku_secenekli(
        ayarlar,
        &OkuSecenekleri {
            codex: args.codex,
            agy: args.agy,
            deepseek: args.deepseek,
        },
    )
    .await;
    if args.yaz
        && let Err(e) = anlik_ve_uyum_yayinla_ayarli(
            yollar,
            &anlik,
            UtcZaman::simdi(),
            Some(ayarlar.agy.aralik_sn),
        )
    {
        eprintln!("Anlık dosyası yazılamadı: {e}");
        return ExitCode::from(1);
    }
    match serde_json::to_string_pretty(&anlik) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("JSON serileştirme hatası: {e}");
            ExitCode::from(1)
        }
    }
}

/// Süreç kendi konsoluyla başlatıldıysa (Görev Zamanlayıcı oturum açılışında konsol alt sistemli
/// exe için yeni pencere açar) konsoldan ayrılır: pencere kapanmaz ve kapatılınca (CTRL_CLOSE)
/// tek veri katmanı olan servis düşmez. Bir terminalden çalıştırıldıysa (konsolu paylaşan başka
/// süreç var) dokunulmaz; masaüstünün `CREATE_NO_WINDOW` ile başlattığı süreçte konsol zaten yoktur.
#[cfg(windows)]
fn yalniz_kendi_konsolumuzsa_birak(gizle: bool) {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleProcessList(liste: *mut u32, adet: u32) -> u32;
        fn FreeConsole() -> i32;
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn ShowWindow(pencere: *mut std::ffi::c_void, komut: i32) -> i32;
    }
    const SW_HIDE: i32 = 0;
    let mut liste = [0u32; 4];
    // SAFETY: tampon ve uzunluğu doğru verilir; yalnız konsola bağlı süreç kimlikleri yazılır.
    let adet = unsafe { GetConsoleProcessList(liste.as_mut_ptr(), liste.len() as u32) };
    if adet == 1 {
        if gizle {
            // SAFETY: argümansız Win32 çağrısı; konsol yoksa boş tutamaç döner ve atlanır.
            let pencere = unsafe { GetConsoleWindow() };
            if !pencere.is_null() {
                // SAFETY: tutamaç bu sürecin konsol penceresidir; gizleme yan etkisizdir.
                unsafe {
                    ShowWindow(pencere, SW_HIDE);
                }
            }
        }
        // SAFETY: argümansız Win32 çağrısı; başarısızlığı (konsol yok) zararsızdır.
        unsafe {
            FreeConsole();
        }
    }
}

#[cfg(not(windows))]
fn yalniz_kendi_konsolumuzsa_birak(_gizle: bool) {}

async fn servis_komutu(yollar: &CetvelYollar, args: ServisArgs) -> ExitCode {
    if !args.tek_tur {
        yalniz_kendi_konsolumuzsa_birak(args.gizli);
    }
    match servis_calistir(yollar, args.tek_tur).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(cetvel_servis::ServisHatasi::ZatenCalisiyor) => {
            eprintln!("Cetvel servisi zaten çalışıyor.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Servis hatası: {e}");
            ExitCode::from(1)
        }
    }
}

async fn mcp_komutu(yollar: CetvelYollar) -> ExitCode {
    match cetvel_mcp::mcp_stdio_sunucusu(yollar).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("MCP sunucu hatası: {e}");
            ExitCode::from(1)
        }
    }
}

fn kur_komutu(yollar: &CetvelYollar, args: KurArgs) -> ExitCode {
    let Ok(exe_yolu) = std::env::current_exe() else {
        eprintln!("Yürütülebilir dosya yolu çözülemedi.");
        return ExitCode::from(1);
    };
    let secenekler = kurulum::KurSecenekleri {
        exe_yolu: &exe_yolu,
        uygulama: args.uygulama.as_deref(),
        baslat: args.baslat,
        kuru_calistir: args.kuru_calistir,
    };
    match kurulum::kur_calistir(yollar, secenekler) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

async fn durdur_komutu(yollar: &CetvelYollar) -> ExitCode {
    match kurulum::durdur_calistir(yollar).await {
        kurulum::DurdurmaSonucu::Calismiyordu => {
            println!("Cetvel servisi çalışmıyor.");
            println!(
                "{}",
                kurulum::servis_kaydi_ozeti(&kurulum::servis_kaydi_oku())
            );
            ExitCode::SUCCESS
        }
        kurulum::DurdurmaSonucu::DuzgunDurdu => {
            println!("Cetvel servisi durduruldu.");
            ExitCode::SUCCESS
        }
        kurulum::DurdurmaSonucu::Sonlandirildi { pid } => {
            println!("Cetvel servisi yanıt vermedi; süreç {pid} sonlandırıldı.");
            ExitCode::SUCCESS
        }
        kurulum::DurdurmaSonucu::Durmadi { neden } => {
            eprintln!("Cetvel servisi durdurulamadı: {neden}");
            ExitCode::from(2)
        }
    }
}

async fn kaldir_komutu(yollar: &CetvelYollar, ayarlar: &Ayarlar, args: KaldirArgs) -> ExitCode {
    match kurulum::kaldir_calistir(
        yollar,
        ayarlar,
        args.veri_sil,
        args.kuru_calistir,
        args.statusline_birak,
    )
    .await
    {
        Ok(rapor) if rapor.uyarilar.is_empty() => ExitCode::SUCCESS,
        Ok(rapor) => {
            for uyari in &rapor.uyarilar {
                eprintln!("Uyarı: {uyari}");
            }
            // 2: kısmi kaldırma (diğer adımlar tamamlandı).
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn dogrula_komutu(yollar: &CetvelYollar, args: DogrulaArgs) -> ExitCode {
    if let Some(dizin) = args.sema_yaz {
        let v2_yol = dizin.join("anlik-v2.schema.json");
        let v1_yol = dizin.join("anlik-v1-uyum.schema.json");
        if let Err(e) = atomik_yaz_json(&v2_yol, &anlik_v2_json_schema()) {
            eprintln!("v2 şema yazılamadı: {e}");
            return ExitCode::from(1);
        }
        if let Err(e) = atomik_yaz_json(&v1_yol, &orkestra_uyum_v1_json_schema()) {
            eprintln!("v1 şema yazılamadı: {e}");
            return ExitCode::from(1);
        }
        println!(
            "Şemalar yazıldı: {} ve {}",
            v2_yol.display(),
            v1_yol.display()
        );
    }

    if let Some(dosya) = args.dosya {
        let Ok(icerik) = std::fs::read_to_string(&dosya) else {
            eprintln!("Dosya okunamadı: {}", dosya.display());
            return ExitCode::from(1);
        };
        if let Ok(v2) = serde_json::from_str::<AnlikV2>(&icerik) {
            if let Err(e) = anlik_dogrula(&v2) {
                eprintln!("v2 doğrulama hatası: {e}");
                return ExitCode::from(1);
            }
            println!("v2 doğrulaması geçti: {}", dosya.display());
            return ExitCode::SUCCESS;
        }
        if let Ok(v1) = serde_json::from_str::<OrkestraUyumV1>(&icerik) {
            if let Err(e) = uyum_dogrula(&v1) {
                eprintln!("v1 uyum doğrulama hatası: {e}");
                return ExitCode::from(1);
            }
            println!("v1 uyum doğrulaması geçti: {}", dosya.display());
            return ExitCode::SUCCESS;
        }
        eprintln!("Dosya ne Anlık v2 ne de Orkestra v1 uyum biçimine uyuyor.");
        return ExitCode::from(1);
    }

    let mut sorun = false;
    let anlik_yol = yollar.anlik_dosyasi();
    if anlik_yol.exists() {
        match std::fs::read_to_string(&anlik_yol)
            .ok()
            .and_then(|s| serde_json::from_str::<AnlikV2>(&s).ok())
        {
            Some(v2) => match anlik_dogrula(&v2) {
                Ok(()) => println!("anlik.json doğrulandı (sira={}).", v2.sira),
                Err(e) => {
                    eprintln!("anlik.json doğrulama hatası: {e}");
                    sorun = true;
                }
            },
            None => {
                eprintln!("anlik.json ayrıştırılamadı: {}", anlik_yol.display());
                sorun = true;
            }
        }
    } else {
        println!("anlik.json henüz yok (öz-test şema üretimi kontrol edildi).");
    }

    let uyum_yol = yollar.orkestra_uyum_dosyasi();
    if uyum_yol.exists() {
        match std::fs::read_to_string(&uyum_yol)
            .ok()
            .and_then(|s| serde_json::from_str::<OrkestraUyumV1>(&s).ok())
        {
            Some(v1) => match uyum_dogrula(&v1) {
                Ok(()) => println!("orkestra-uyum.json doğrulandı."),
                Err(e) => {
                    eprintln!("orkestra-uyum.json doğrulama hatası: {e}");
                    sorun = true;
                }
            },
            None => {
                eprintln!("orkestra-uyum.json ayrıştırılamadı: {}", uyum_yol.display());
                sorun = true;
            }
        }
    }

    if sorun {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// `cetvel kopru statusline`: Claude Code'un stdin JSON'unu önceki statusLine komutuna iletir,
/// çıktısını aynen basar; `rate_limits` varsa servise gönderir. Hata durumunda sessiz kalır
/// (statusLine'ı asla bozmaz).
async fn statusline_komutu(args: StatuslineArgs) -> ExitCode {
    use std::io::{Read, Write};

    let mut girdi = Vec::new();
    let _ = std::io::stdin().take(1_048_576).read_to_end(&mut girdi);

    // Kota köprüsü önce gönderilir: takılan önceki statusLine komutu köprüyü durdurmasın.
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&girdi)
        && let Some(yuk) = cetvel_servis::bagla::statusline_kota_yuku(&json)
        && servis_calisiyor_mu()
        && let Ok(yollar) = CetvelYollar::ortamdan()
    {
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(800),
            servis_pipe_istegi(&yollar, "kopru.claude", Some(yuk)),
        )
        .await;
    }

    if let Some(onceki) = args
        .onceki_hex
        .as_deref()
        .and_then(cetvel_servis::bagla::hex_coz)
    {
        // Önceki komut, Claude Code'un kullandığı kabukla çalıştırılır (Git Bash varsa `bash -c`,
        // yoksa PowerShell): `~/.claude/x.sh`, tek tırnaklı `jq` vb. aynen çalışır.
        let mut komut = match cetvel_servis::bagla::statusline_kabugu() {
            cetvel_servis::bagla::StatuslineKabugu::Bash(bash) => {
                let mut k = std::process::Command::new(bash);
                k.arg("-c").arg(&onceki);
                k
            }
            cetvel_servis::bagla::StatuslineKabugu::PowerShell(ps) => {
                let mut k = std::process::Command::new(ps);
                k.args(["-NoProfile", "-NonInteractive", "-Command"])
                    .arg(&onceki);
                k
            }
        };
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            komut.creation_flags(CREATE_NO_WINDOW);
        }
        komut
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        if let Some(cikti) = onceki_komutu_calistir(komut, girdi, ONCEKI_STATUSLINE_SURESI) {
            let _ = std::io::stdout().write_all(&cikti);
        }
    }

    ExitCode::SUCCESS
}

/// Zincirlenen önceki statusLine komutuna tanınan en uzun süre.
const ONCEKI_STATUSLINE_SURESI: std::time::Duration = std::time::Duration::from_secs(3);

/// Önceki statusLine komutunu zaman aşımıyla çalıştırır: stdin ayrı iş parçacığından yazılır
/// (çocuk stdin tüketmeden büyük çıktı yazarsa kilitlenmesin), süre dolunca süreç ağacı Job ile
/// öldürülür. Zaman aşımında `None`.
fn onceki_komutu_calistir(
    mut komut: std::process::Command,
    girdi: Vec<u8>,
    sure: std::time::Duration,
) -> Option<Vec<u8>> {
    use std::io::{Read, Write};
    let job = cetvel_baglayici::JobSuresKorumasi::yeni().ok();
    let mut cocuk = komut.spawn().ok()?;
    #[cfg(windows)]
    if let Some(j) = &job {
        use std::os::windows::io::AsRawHandle;
        let _ = j.sureci_ekle(cocuk.as_raw_handle());
    }
    if let Some(mut stdin) = cocuk.stdin.take() {
        std::thread::spawn(move || {
            let _ = stdin.write_all(&girdi);
        });
    }
    let mut stdout = cocuk.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut tampon = Vec::new();
        let _ = stdout.by_ref().take(1_048_576).read_to_end(&mut tampon);
        let _ = tx.send(tampon);
    });
    let sonuc = rx.recv_timeout(sure).ok();
    if sonuc.is_none() {
        if let Some(j) = &job {
            j.sonlandir();
        }
        let _ = cocuk.kill();
    }
    let _ = cocuk.wait();
    sonuc
}

fn tablo_yazdir(anlik: &AnlikV2, servis_aktif: bool, dogrudan: bool) {
    if dogrudan {
        println!("Cetvel (doğrudan tanı okuması) · {}", anlik.zaman);
    } else if servis_aktif {
        println!(
            "Cetvel servisi çalışıyor · sira={} · {}",
            anlik.sira, anlik.zaman
        );
    } else {
        println!(
            "UYARI: Cetvel servisi çalışmıyor; son kayıt gösteriliyor (sira={}, {}).",
            anlik.sira, anlik.zaman
        );
    }
    println!(
        "{:<14} {:<11} {:<16} {:<13} {:<24} KAYNAK / YAS",
        "HAVUZ", "DURUM", "ETKIN KALAN", "KULLANILIR", "PENCERELER"
    );
    println!("{}", "-".repeat(94));

    for (id, havuz) in &anlik.havuzlar {
        let durum_str = serde_json::to_value(havuz.durum)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "?".to_string());
        // Kullanım sayacı (OpenCode, dsh) kota değildir: yüzde ya da "kullanılır" yazılmaz.
        let sayac = havuz.kullanim_sayaci_mi();
        let etkin_str = match (havuz.etkin_kalan, &havuz.baglayici_pencere) {
            _ if sayac => havuz
                .ek
                .as_ref()
                .and_then(|e| e.kullanim.as_ref())
                .map(|k| format!("{} oturum/24sa", k.oturum_24s))
                .unwrap_or_else(|| "sayaç".to_string()),
            (Some(k), Some(p)) => format!("{:.1}% ({p})", k * 100.0),
            (Some(k), None) => format!("{:.1}%", k * 100.0),
            (None, _) => havuz
                .bakiye
                .as_ref()
                .map(|b| format!("{:.2} {}", b.toplam, b.para_birimi))
                .unwrap_or_else(|| "-".to_string()),
        };
        let kullanilir_str = match havuz.kullanilabilir {
            _ if sayac => "kota yok",
            Some(true) => "evet",
            Some(false) => "HAYIR",
            None => "-",
        };
        let pencere_ozet = if havuz.pencereler.is_empty() {
            "-".to_string()
        } else {
            havuz
                .pencereler
                .iter()
                .map(|(ad, p)| format!("{ad}:{:.0}%", p.kalan * 100.0))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let kaynak_ozet = match &havuz.kaynak {
            Some(k) => {
                let tur = serde_json::to_value(k.tur)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_else(|| "?".to_string());
                format!("{tur} ({} sn)", k.yas_sn)
            }
            None => "-".to_string(),
        };

        println!(
            "{:<14} {:<11} {:<16} {:<13} {:<24} {}",
            id, durum_str, etkin_str, kullanilir_str, pencere_ozet, kaynak_ozet
        );
        if !havuz.notlar.is_empty() {
            println!("  not: {}", havuz.notlar.join("; "));
        }
    }
}
