[English](README.md) | **Türkçe**

<p align="center">
  <img src="apps/arayuz/logo.png" alt="Cetvel logosu" width="96" height="96">
</p>

# Cetvel

**Yapay zekâ kullanım sınırları, ölçülmüş hâliyle.**

Cetvel, yapay zekâ kodlama araçlarındaki sınırlarınızdan ne kadarının kaldığını gösteren yerel bir Windows uygulamasıdır: Claude Code'un 5 saatlik ve haftalık pencereleri, Codex, Antigravity'nin üç havuzu ve isteğe bağlı olarak DeepSeek bakiyesi ile yerel kullanım sayaçları. Sistem tepsisinde durur, bir sınır bitmeden önce uyarır ve aynı sayıları kendi betikleriniz ve ajanlarınız için JSON olarak yayımlar.

Aynı anda birkaç yapay zekâ kodlama aracı kullananlar için tasarlandı: her aracın kendi kullanım sayfasını açmadan, hangisinde hâlâ yer olduğunu tek bakışta görürsünüz.

**Hızlı başlangıç:** [Sürümler (Releases) sayfasından](https://github.com/yigityildiz0/burette/releases) `Cetvel_<SÜRÜM>_x64-setup.exe` dosyasını indirin, SHA-256 değerini denetleyin, çalıştırın (yönetici yetkisi gerekmez) ve sistem tepsisindeki simgeye bakın. Ayrıntılar: [Kurulum](#kurulum).

Projenin İngilizce yayın adı **Burette**'tir. Crate'ler, çalıştırılabilir dosyalar (`cetvel.exe`, `cetvel-masaustu.exe`), Windows servisi ve veri klasörleri `cetvel` adını kullanır.

Durum: 0.1.0 sürümü; yalnız Windows 10 ve 11 (64 bit) için erken bir yayın.

Cetvel bağımsız bir projedir; Anthropic, OpenAI, Google ya da DeepSeek ile bağı yoktur, bu şirketler tarafından desteklenmez ya da onaylanmaz. Ürün adları yalnız tanımlamak için geçer.

## Desteklenen platformlar

| Platform | Havuzlar | Nasıl okunur | Varsayılan |
|---|---|---|---|
| Claude (Claude Code) | `claude_max`: 5 saatlik ve haftalık pencere | Claude Code, statusLine komutuna `rate_limits` verisini geçirir; `cetvel kopru statusline` bunu yerel servise iletir ve önceki statusLine komutunuzu da olduğu gibi çalıştırır | Bağla sekmesinden, önizleme ve onayınızla bağlanır |
| Codex | `codex_plus`: 5 saatlik ve haftalık pencere | Yerel `codex.exe app-server`'ı çalıştırıp `account/rateLimits/read` çağırır; olmazsa `~/.codex/sessions` içindeki kota olaylarına düşer | Codex kurulu ve oturum açıksa açık |
| Antigravity | `agy_gemini`, `agy_claude_gpt`, `agy_argon` (üç ayrı havuz) | `agy -p /usage` çalıştırılır ve çıktısı ayrıştırılır | **Kapalı**; "istek üzerine" ya da "otomatik" seçimi sizde |
| DeepSeek (isteğe bağlı) | `deepseek`: hesap bakiyesi, kota değil | API anahtarınızla `GET https://api.deepseek.com/user/balance` | Kapalı |
| OpenCode (isteğe bağlı) | `opencode`: kullanım sayacı, kota yok | Yerel `opencode.db` (ya da eski `storage/` JSON) içinden oturum ve belirteç toplamları | Kapalı |
| DeepSeek Harness (isteğe bağlı) | `dsh`: kullanım sayacı, kota yok | `~/.dsh/sessions` içinden oturum ve belirteç toplamları | Kapalı |

OpenCode ve DeepSeek Harness'ın resmî kotası olmadığı için Cetvel yalnız yerel kullanımı sayar ve bu kartları "kota yok · kullanım sayacı" diye gösterir.

## Yüzdeler ne anlama gelir

- Varsayılan olarak her çubuk ve tepsi simgesi **kalanı** gösterir (`%15 kaldı`). Ayarlar > Genel > *Ana yüzde ölçütü* çubukları ve tepsiyi **kullanılana** (`%85 kullanıldı`) çevirir.
- Bir havuzun birden çok penceresi olabilir (ör. 5 saatlik ve haftalık). Öne çıkan sayı **etkin kalandır**: pencereler arasındaki en düşük kalan. Bu değeri belirleyen pencereye **bağlayıcı pencere** denir.
- Herhangi bir pencere %0'daysa, öteki pencere %100 gösterse bile havuzun tamamı kullanılamaz sayılır. Örnek: haftalık %0, 5 saatlik %100 ise havuz haftalık sıfırlamaya kadar kapalıdır.
- JSON dosyalarında `kalan` ve `kullanilan` 0 ile 1 arasında oranlardır; `etkin_kalan` etkin kalandır.

## Özellikler

- **Tepsi, açılır pencere ve geniş pencere:** iki çubuklu tepsi simgesi ve ipucu en dar havuzu gösterir; açılır pencere her havuzu ince çubuklarla ve sıfırlamaya kalan süreyle listeler; geniş pencere platform ayrıntısı ve geçmiş ekler. `Ctrl+Alt+K` açılır pencereyi açar ya da gizler.
- **Şerit kipi:** ekranın üstünde ya da yanında ince, sürüklenebilir bir kota şeridi; isteğe bağlı tıklama geçirgen kip (önizleme).
- **Bildirimler:** bir havuz eşiğin altına düşünce (varsayılan kalan eşikleri: Claude 5 saat %15, Claude haftalık %30, diğerleri %20; platform başına değiştirilebilir), mevcut hızla bir pencere sıfırlanmadan bitecekse ve veri bayatlayınca Windows bildirimi gelir. Sessiz saatler ve platform başına susturma vardır.
- **Tahmin:** tüketim hızı (EWMA) ve pencere başına tahmini bitiş saati; yalnız yeterli okuma varsa gösterilir.
- **Geçmiş:** okumalar yerel bir SQLite veritabanında tutulur (ham örnekler 90 gün) ve grafikle gösterilir.
- **Kart düzeni:** havuzları sıralayın, sabitleyin ya da gizleyin.
- **11 dil:** Türkçe, İngilizce, Almanca, Fransızca, İspanyolca, Portekizce (Brezilya), Rusça, Japonca, Korece, Basitleştirilmiş ve Geleneksel Çince.
- **Temalar:** dört renk teması; her biri açık, koyu ya da sistemi izleyen kipte.
- **Betikler ve ajanlar için:**
  - `anlik.json` (anlık şema v2) ve `orkestra-uyum.json` (sade uyum şeması v1; her işi zorluğa ve kalan kotaya göre bir modele yönlendiren [Orkestra](https://github.com/yigityildiz0/orkestra) bu dosyayı okur) `%LOCALAPPDATA%\Cetvel` altına atomik olarak yazılır. JSON şemaları [`schema/`](schema/) klasöründedir.
  - `127.0.0.1` üzerinde yerel HTTP API (varsayılan port 43117, Bearer jeton zorunlu), kullanıcıya özel adlandırılmış boru (named pipe) ve MCP stdio sunucusu (`cetvel mcp`; araçlar `kota_anlik`, `havuz_detay`, `uygun_havuzlar`).
  - Komut satırı aracı (bkz. [Komut satırı](#komut-satırı)).

## Kurulum

Gereksinim: Windows 10 ya da 11, 64 bit. Yönetici yetkisi gerekmez.

1. [Sürümler (Releases) sayfasından](https://github.com/yigityildiz0/burette/releases) `Cetvel_<SÜRÜM>_x64-setup.exe` dosyasını indirin.
2. SHA-256 değerini sürüm sayfasındakiyle karşılaştırın, sonra çalıştırın.

Kurulum dosyası henüz kod imzalı değildir; SmartScreen uyarı verebilir. Ayrıntılı adımlar, sessiz kurulum (`/S`), bir YZ kodlama ajanına kurulumu yaptırmak için hazır istem, yükseltme ve kaldırma: [docs/KURULUM.md](docs/KURULUM.md).

## İlk çalıştırma

1. Kurulum, oturum açtığınızda başlayan bir arka plan servisi kaydeder. Son sayfada **Cetvel'i çalıştır** işaretliyse uygulama açılır ve simgesi sistem tepsisinde görünür; açılır pencere için simgeye tıklayın ya da `Ctrl+Alt+K`'ya basın.
2. Codex kuruluysa ve oturum açıksa kendiliğinden görünür.
3. Claude için **Bağla** sekmesini açıp statusLine bağlantısını onaylayın (önce önizleme görürsünüz ve tarihli bir yedek alınır). Ardından Claude Code'u bir kez kullanın; ilk okuma böylece gelir.
4. Antigravity, DeepSeek ve yerel kullanım sayaçları, uygulamada siz açana kadar kapalı kalır.

## Kaldırma

Ayarlar > Uygulamalar > Yüklü uygulamalar > Cetvel > Kaldır. **Uygulama verilerini sil** kutusunu işaretlemezseniz ayarlarınız ve geçmişiniz korunur. Sessiz kaldırma ve nelerin silindiği: [docs/KURULUM.md](docs/KURULUM.md#3-kaldırma).

## Sorun giderme

Servis çalışmıyor, port çakışması, SmartScreen, bildirimlerin görünmemesi ya da WebView2 eksikliği: [docs/KURULUM.md](docs/KURULUM.md#4-sorun-giderme). Kısa yanıtlar [SSS](#sss) bölümünde.

## Kaynaktan derleme

MSVC hedefli Rust araç zinciri gerekir (edition 2024, yani Rust 1.85 ya da üstü). `apps/arayuz` içindeki arayüz düz HTML, CSS ve JavaScript'tir; npm derleme adımı yoktur.

```powershell
cargo build --release                                  # cetvel.exe ve cetvel-masaustu.exe
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Kurulum dosyası için Tauri CLI'yi kurun (`cargo install tauri-cli --version "^2" --locked`) ve şunu çalıştırın:

```powershell
pwsh -File paketleme/kurulum-dosyasi-uret.ps1   # -> target/release/bundle/nsis/Cetvel_<SÜRÜM>_x64-setup.exe
```

Betik yerel derleme yollarını yeniden eşler (`--remap-path-prefix`, `/d1trimfile:`); böylece kullanıcı klasörünüz çalıştırılabilir dosyalara girmez. Ardından çıktıda bu yolu arar ve SHA-256 değerini yazar. Düz `cargo tauri build` de çalışır ama çıktısında yerel yollar kalır, yayımlanmamalıdır.

## Komut satırı

```powershell
cetvel durum                 # son anlığı okur (servis çalışmıyorsa çıkış kodu 3)
cetvel oku --codex           # bir bağlayıcıdan tek seferlik okur, anlık JSON'unu yazar
cetvel servis                # arka plan servisini ön planda çalıştırır
cetvel kur --kuru-calistir   # servis kaydını hiçbir şeyi değiştirmeden önizler
cetvel durdur                # çalışan servisi durdurur
cetvel dogrula               # anlık dosyalarını şemaya göre doğrular
cetvel mcp                   # MCP stdio sunucusunu başlatır
```

## Gizlilik

- **Yerelde çalışır.** Hesap, telemetri ya da analitik yoktur. HTTP API yalnız `127.0.0.1` (varsa `[::1]`) üzerinde dinler, yalnız loopback `Host` başlığını kabul eder ve jeton ister; adlandırılmış boru yalnız sizin Windows hesabınıza açıktır.
- **Model isteği yok.** Cetvel hiçbir modele istem göndermez. Kendi yaptığı tek ağ isteği isteğe bağlı DeepSeek bakiye çağrısıdır. Codex ve Antigravity kendi yerel komut satırı araçlarıyla sorgulanır.
- **Neyi okur:**
  - Claude: Claude Code'un statusLine komutuna geçirdiği `rate_limits` verisi ve `~/.claude/settings.json` (statusLine bağlantısını göstermek ve onayınızla değiştirmek için; önce tarihli yedek alınır).
  - Codex: `codex.exe app-server` çıktısı, `~/.codex/sessions` içindeki kota olayları ve `~/.codex/auth.json` dosyasının var olup olmadığı (içeriği okunmaz).
  - Antigravity: yalnız izin verdikten sonra `agy -p /usage` çıktısı. `hub_*.json` dosyaları hiç okunmaz.
  - OpenCode ve DeepSeek Harness: yalnız oturum sayısı ve belirteç toplamları; istem, yanıt, başlık ya da `auth.json` asla.
  - Yerel ajan API'si (`/v1/ajanlar`) için: Codex oturum günlükleri ve Antigravity'nin `conversation_summaries.db` dosyası; yalnız tür, sayı, zaman ve karma kimliğe indirgenir. İstem metni, komut satırı ya da dosya yolu tutulmaz.
- **Neyi yazar:** ayarlar `%APPDATA%\Cetvel\ayarlar.toml`; anlıklar, geçmiş, günlükler ve bağlantı bilgisi `%LOCALAPPDATA%\Cetvel` (yalnız kullanıcıya açık dosya izinleriyle). API anahtarları düz metin dosyaya değil Windows Kimlik Bilgisi Yöneticisi'ne (`Cetvel/<ad>`) yazılır.
- **Kaldırma**, *Uygulama verilerini sil* kutusunu işaretlemezseniz verilerinizi korur.

## SSS

**Bir çubuk doluyken havuz neden "kullanılamaz" diyor?**
Aynı havuzun başka bir penceresi %0'dadır. Havuzu en dar penceresi sınırlar; bkz. [Yüzdeler ne anlama gelir](#yüzdeler-ne-anlama-gelir).

**Claude verisi görünmüyor.**
Claude verisi yalnız Claude Code çalışırken ve statusLine'ı yenilenirken gelir. Bağla sekmesinden statusLine bağlantısını kurun, sonra Claude Code'u bir kez kullanın.

**Antigravity neden kapalı?**
Okumak için `agy -p /usage` çalıştırmak gerekir; bu, üçüncü taraf araç politikalarıyla çelişebilir. İzni istek üzerine ya da otomatik olarak vermeye siz karar verirsiniz.

**macOS ya da Linux'ta çalışır mı?**
Hayır. Cetvel yalnız Windows'ta bulunan API'leri kullanır (Görev Zamanlayıcı, Kimlik Bilgisi Yöneticisi, adlandırılmış borular, bildirimler).

**API anahtarım bir yere gönderiliyor mu?**
DeepSeek anahtarı yalnız DeepSeek bakiye uç noktasına gönderilir. Günlüğe ya da ayar dosyasına yazılmaz.

## Katkı

Hata bildirimleri ve fikirler [issue](https://github.com/yigityildiz0/burette/issues) olarak açılabilir. Katkı verenler ve YZ kodlama ajanları için derleme, test ve stil kuralları [AGENTS.md](AGENTS.md) dosyasındadır (İngilizce).

## Lisans

`MIT OR Apache-2.0` çift lisanslıdır: bkz. [LICENSE-MIT](LICENSE-MIT) ve [LICENSE-APACHE](LICENSE-APACHE). Pakete giren Rust bileşenlerinin lisansları `Cargo.lock`'tan üretilen [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) dosyasındadır:

```powershell
python paketleme/ucuncu_taraf_bildirimleri_uret.py            # yeniden üretir
python paketleme/ucuncu_taraf_bildirimleri_uret.py --denetle  # güncel değilse çıkış kodu 1
```

Paketteki yazı tipleri (Inter, Source Serif 4) SIL Open Font License altındadır; bkz. `apps/arayuz/fontlar/`.
