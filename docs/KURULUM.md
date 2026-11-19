# Cetvel Kurulumu (Windows)

> English: [INSTALL.md](INSTALL.md)

Cetvel'in İngilizce yayın adı Burette'tir; Windows'ta uygulama, kurulum dosyası, Başlat menüsü girdisi, servis ve komut satırı aracı **Cetvel** / `cetvel.exe` adını taşır.

Cetvel normal bir Windows uygulaması gibi kurulur: tek bir kurulum dosyası (`Cetvel_<SÜRÜM>_x64-setup.exe`), yönetici yetkisi gerekmez, Başlat menüsünde ve Windows aramasında "Cetvel" olarak çıkar, Ayarlar > Uygulamalar'dan kaldırılır.

| | |
|---|---|
| Sistem | Windows 10 / 11, 64 bit |
| Yetki | Yönetici gerekmez (yalnız geçerli kullanıcı için kurulur) |
| Kurulum yeri | `%LOCALAPPDATA%\Programs\Cetvel` |
| Dil | Kurulum başında Türkçe / English seçilir |
| Bileşen | Microsoft WebView2 (Windows 11'de hazır; yoksa kurulum Microsoft'tan indirir) |

## 1. Elle kurulum

1. Bu deponun [Sürümler (Releases) sayfasını](../../../releases) açın.
2. En son sürümün altındaki **`Cetvel_<SÜRÜM>_x64-setup.exe`** dosyasını indirin.
3. (Önerilir) Dosyanın bozulmadığını ve değiştirilmediğini doğrulayın. PowerShell'de:

   ```powershell
   Get-FileHash "$env:USERPROFILE\Downloads\Cetvel_<SÜRÜM>_x64-setup.exe" -Algorithm SHA256
   ```

   Çıkan değer sürüm sayfasında yayımlanan SHA-256 değeriyle aynı olmalı. Değilse dosyayı çalıştırmayın.
4. Dosyayı çift tıklayıp çalıştırın. SmartScreen uyarısı çıkarsa bkz. [Sorun giderme](#smartscreen-uyarısı).
5. Dili seçin → **İleri** → kurulum klasörünü onaylayın → **Yükle**.
6. Son sayfada **Cetvel'i çalıştır** işaretliyse uygulama açılır; tepside Cetvel simgesi görünür.

### Kurulum ne yapar

| Adım | Ayrıntı |
|---|---|
| Dosyaları kopyalar | `cetvel-masaustu.exe` (uygulama), `cetvel.exe` (arka plan servisi ve komut satırı), `uninstall.exe` |
| Başlat menüsü kısayolu | `Cetvel`: Windows aramasında çıkar, uygulamayı açar ve bildirimlerin Cetvel adıyla görünmesini sağlar |
| Kaldırma girdisi | Ayarlar > Uygulamalar ve Denetim Masası > Programlar ve Özellikler'de "Cetvel" |
| Servis kaydı | Görev Zamanlayıcı kökünde `CetvelServis` görevi: bu kullanıcı oturum açınca kullanıcı yetkisiyle `cetvel.exe servis --gizli` çalışır, çökerse yeniden başlatılır. Görev kurulumu yapan kullanıcının SID'ine bağlandığı için yönetici gerekmez. Görev kaydı yine de reddedilirse (ör. ilke Görev Zamanlayıcı'yı kısıtlıyorsa: "Erişim engellendi") yedek olarak `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` altına `CetvelServis` = `"<kurulum>\cetvel.exe" servis --gizli` yazılır: servis oturum açılışında pencere açmadan başlar ama çökerse kendiliğinden yeniden başlamaz. Kurulum servisi her iki yolda da hemen başlatır. Önceki sürümlerin `Cetvel\CetvelServis` görevi ve boş kalan `Cetvel` görev klasörü silinir |

Kurulumun **yapmadıkları:** yönetici yetkisi istemez, `PATH` değişkenini değiştirmez, API anahtarı istemez, Claude Code `settings.json` dosyasına dokunmaz (statusLine bağlantısı yalnız uygulamadaki **Bağla** ekranından, sizin onayınızla yapılır).

### Yükseltme

Yeni sürümün kurulum dosyasını çalıştırın. Önceki kurulum bulunursa pencerede iki seçenek çıkar ve **ilki, "Yükleme yapmadan önce kaldırın", önceden işaretlidir.** Ayarlarınızın ve bağlantılarınızın olduğu gibi kalması için **"Kaldırma işlemini gerçekleştirme"yi seçip** İleri'ye basın. Eski bir sürüme dönerken de pencere ve varsayılan aynıdır.

| | Kaldırma işlemini gerçekleştirme (önerilen) | Yükleme yapmadan önce kaldırın (önceden işaretli) |
|---|---|---|
| Nasıl işler | Servis durdurulur, yeni dosyalar eskilerin üzerine yazılır, servis yeniden başlatılır | Önce eski sürümün kaldırıcısı açılır ve tam kaldırma yapar, sonra yeni sürüm kurulur |
| Ayarlar, ölçüm geçmişi, API anahtarları | korunur | korunur; ama kaldırıcıdaki **Uygulama verilerini sil** kutusunu işaretlerseniz **silinir** |
| Claude statusLine bağlantısı | korunur | geri alınır; gerekiyorsa Bağla ekranından yeniden uygulayın |
| "Windows ile başlat" | korunur | kapanır; uygulama ayarlarından yeniden açın |
| Servis görevi, Başlat menüsü kısayolu | korunur | silinir, yeni kurulum yeniden oluşturur |

Aynı sürümü yeniden çalıştırırsanız önceden işaretli seçenek **Bileşen Ekle/Yeniden Yükle**'dir ve üzerine yazar; **Cetvel'i kaldır** seçeneği tam kaldırma yapar.

Sessiz kurulum (`/S`) bu pencereyi göstermez ve her zaman üzerine yazarak yükseltir. Pasif kurulum (`/P`) yeni sürümde de üzerine yazar, ama **aynı sürümü** `/P` ile yeniden kurmak önce tam kaldırma yapar (statusLine bağlantısı ve "Windows ile başlat" geri alınır); aynı sürümü onarmak için `/S` kullanın.

## 2. YZ ajanıyla kurulum (kopyala-yapıştır)

Aşağıdaki metni bir YZ kodlama ajanına (Claude Code, Codex vb.) verin. `[RELEASES_URL]` (bu deponun sürüm adresi, `https://github.com/<sahip>/burette/releases`) ve `<SÜRÜM>` yerlerini doldurun.

```text
Windows bilgisayarıma Cetvel'i kur. PowerShell kullan.

Kurallar:
- Yönetici yetkisi isteme, yükseltilmiş terminal açma.
- API anahtarı isteme, okuma ya da yazma; Credential Manager'a dokunma.
- SmartScreen, Defender ya da antivirüs ayarlarını değiştirme; dışlama ekleme.
- Claude Code settings.json dosyasına dokunma.
- Bir adım başarısız olursa dur, çıktıyı bana göster ve ne yapmamı önerdiğini söyle.

Adımlar:
1. Kurulum dosyasını indir:
   $dosya = Join-Path $env:TEMP "Cetvel_<SÜRÜM>_x64-setup.exe"
   Invoke-WebRequest -Uri "[RELEASES_URL]/download/v<SÜRÜM>/Cetvel_<SÜRÜM>_x64-setup.exe" -OutFile $dosya
2. SHA-256 değerini hesapla ve sürüm sayfasındaki değerle karşılaştır:
   (Get-FileHash $dosya -Algorithm SHA256).Hash
   Eşleşmiyorsa kurma ve bana bildir.
3. Sessiz kur (yönetici gerekmez; kurulum yeri %LOCALAPPDATA%\Programs\Cetvel):
   Start-Process -FilePath $dosya -ArgumentList "/S" -Wait
4. Doğrula:
   $cetvel = Join-Path $env:LOCALAPPDATA "Programs\Cetvel\cetvel.exe"
   & $cetvel durum; "çıkış kodu: $LASTEXITCODE"
   Beklenen: çıkış kodu 0 (servis çalışıyor). Çıkış kodu 3 ise servis çalışmıyordur:
   & $cetvel kur --baslat; Start-Sleep -Seconds 5; & $cetvel durum; "çıkış kodu: $LASTEXITCODE"
5. Servis kaydını göster (görev ya da oturum açılışı yedeği):
   & $cetvel kur --kuru-calistir | Select-Object -First 15
   schtasks /Query /TN "CetvelServis"   # görev yolu; yedek yolda "bulunamıyor" der
   (Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run).CetvelServis   # yedek yol
6. Kurulum dosyasını sil: Remove-Item $dosya
7. Bana kısa bir özet ver: kurulum yeri, `cetvel durum` çıkış kodu ve çıktısı, servis kaydının yolu (görev ya da oturum açılışı kaydı).
```

### Kurulum parametreleri

| Parametre | Anlamı |
|---|---|
| `/S` | Sessiz kurulum: pencere yok, soru yok, dil sistem diline göre seçilir |
| `/P` | Pasif kurulum: yalnız ilerleme çubuğu |
| `/D=<klasör>` | Kurulum klasörü. **En sonda** ve tırnaksız yazılır: `... /S /D=C:\Araclar\Cetvel` |
| `/R` | `/S` ya da `/P` ile: kurulum bitince uygulamayı başlat |

Kurulum dosyası, oturum açmış kullanıcı olarak çalıştırılmalıdır; yükseltilmiş (yönetici) terminalden çalıştırmayın, aksi hâlde servis görevi o hesap için kaydolabilir.

## 3. Kaldırma

**Ayarlar > Uygulamalar > Yüklü uygulamalar > Cetvel > Kaldır** (ya da Denetim Masası > Programlar ve Özellikler).

Kaldırma penceresinde **Uygulama verilerini sil** kutusu bulunur; **varsayılan olarak işaretsizdir.**

| | Kutu işaretsiz (varsayılan) | Kutu işaretli |
|---|---|---|
| Program dosyaları, Başlat menüsü kısayolu, kaldırma girdisi | silinir | silinir |
| Servis kaydı (`CetvelServis` görevi, eski `Cetvel\CetvelServis` görevi, `Run\CetvelServis` yedeği), çalışan servis | durdurulur ve silinir | durdurulur ve silinir |
| "Windows ile başlat" kaydı | silinir | silinir |
| Claude Code statusLine bağlantısı | özgün komuta geri alınır (tarihli yedekle) | özgün komuta geri alınır |
| Ayarlar (`%APPDATA%\Cetvel`) ve ölçüm geçmişi (`%LOCALAPPDATA%\Cetvel`) | **korunur** | silinir |
| Credential Manager'daki `Cetvel/*` API anahtarları | **korunur** | silinir |
| Arayüz önbelleği (`%LOCALAPPDATA%\dev.cetvel.masaustu`) | korunur | silinir |

Sessiz kaldırma (YZ ajanı için):

```powershell
$kaldirici = Join-Path $env:LOCALAPPDATA "Programs\Cetvel\uninstall.exe"
Start-Process -FilePath $kaldirici -ArgumentList "/S" -Wait            # verileri korur
Start-Process -FilePath $kaldirici -ArgumentList "/S /VERISIL" -Wait   # verileri ve anahtarları da siler
```

Kaldırıcı kendini geçici klasöre kopyalayıp oradan çalıştığı için komut erken dönebilir. Doğrulama: birkaç saniye sonra `Test-Path (Join-Path $env:LOCALAPPDATA "Programs\Cetvel\cetvel.exe")` sonucu `False`, `schtasks /Query /TN "CetvelServis"` görev bulunamadığı için hata vermeli ve `(Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run).CetvelServis` boş dönmeli.

## 4. Sorun giderme

### Servis çalışmıyor

Belirti: uygulamada "servis kapalı" şeridi, `cetvel durum` çıkış kodu 3.

1. Uygulamadaki **Servisi başlat** düğmesine basın (tepsi menüsünde de var).
2. Ya da terminalde:

   ```powershell
   $cetvel = Join-Path $env:LOCALAPPDATA "Programs\Cetvel\cetvel.exe"
   & $cetvel kur --baslat     # servisi yeniden kaydeder (görev, olmazsa oturum açılışı kaydı) ve başlatır
   & $cetvel durum            # servis kapalıysa nasıl kayıtlı olduğunu da yazar
   ```

3. Kayıt durumu: `& $cetvel durdur` servis kapalıysa kaydı özetler; görev yolunda ayrıntı için `schtasks /Query /TN "CetvelServis" /V /FO LIST`. Yedek yolda kayıt Görev Yöneticisi > Başlangıç uygulamaları'nda devre dışı bırakılmışsa oturum açılışında başlamaz; oradan etkinleştirin.
4. Günlükler: `%LOCALAPPDATA%\Cetvel\gunluk`
5. Servis yönetici terminalinden başlatılmışsa normal kullanıcı onu durduramaz; yönetici terminalinde `cetvel durdur` çalıştırın, sonra normal terminalde `cetvel kur --baslat`.

### Kurulum "dosya yazılamıyor" diyor

Kurulum önce çalışan servisi durdurur. `cetvel.exe`'yi başka bir süreç kullanıyorsa (Claude Code statusLine köprüsü, bir MCP istemcisinin başlattığı `cetvel mcp`), eski kopyayı aynı klasörde `cetvel-eski-<sayı>.exe` adıyla kenara alıp yenisini yazar; kenara alınan kopya bir sonraki kurulumda ya da kaldırmada silinir. Kaldırmada kullanımdaki `cetvel.exe` geçici klasöre taşınır. Yine de hata çıkarsa Claude Code'u ve MCP istemcilerini kapatın ve servisi elle durdurun:

```powershell
& (Join-Path $env:LOCALAPPDATA "Programs\Cetvel\cetvel.exe") durdur
```

Sonra kurulumda **Yeniden dene**'ye basın. Uygulama açıksa tepsiden **Çıkış** ile kapatın.

### Port çakışması

Yerel API yalnız `127.0.0.1` üzerinde dinler, varsayılan port `43117`. Port doluysa servis kendiliğinden boş bir port seçer ve `%LOCALAPPDATA%\Cetvel\baglanti.json` dosyasına yazar; uygulama ve istemciler portu oradan okur. Sabit port isterseniz `%APPDATA%\Cetvel\ayarlar.toml` içinde:

```toml
[http]
port = 43117
otomatik_port = true   # false: port doluysa servis başlamaz
```

Değişiklikten sonra servisi yeniden başlatın: `cetvel durdur`, ardından `cetvel kur --baslat`.

### SmartScreen uyarısı

Cetvel şu an kod imzası taşımıyor; Windows bu yüzden "Windows kişisel bilgisayarınızı korudu" uyarısı gösterebilir.

1. Önce dosyanın SHA-256 değerini sürüm sayfasındakiyle karşılaştırın (bkz. [Elle kurulum](#1-elle-kurulum)).
2. Değer eşleşiyorsa **Ek bilgi** → **Yine de çalıştır**.
3. Eşleşmiyorsa dosyayı silin ve yeniden indirin.

SmartScreen'i ya da antivirüsü kapatmayın, dışlama eklemeyin. Antivirüs dosyayı yanlış işaretlerse üreticiye yanlış pozitif bildirimi yapın ve bize bildirin.

### Bildirimler görünmüyor

Bildirimler Başlat menüsündeki `Cetvel` kısayoluna bağlıdır. Kısayol silindiyse `cetvel kur` onu yeniden oluşturur. Windows Ayarlar > Sistem > Bildirimler'de Cetvel'in açık olduğunu denetleyin.

### WebView2 eksik

Uygulama penceresi açılmıyorsa Microsoft Edge WebView2 Runtime kurulu olmayabilir. Kurulum normalde onu Microsoft'tan indirir; çevrimdışı kurulumda Microsoft'un sitesinden "Evergreen" sürümünü kurun.

## 5. Gizlilik

- Cetvel yerelde çalışır; telemetri ya da kullanım istatistiği göndermez, hesap gerektirmez.
- Kota bilgisini bilgisayardaki araçlardan (Codex, Claude Code statusLine; açıkça izin verirseniz Antigravity) ve yalnız anahtar girdiğiniz sağlayıcıların bakiye uç noktalarından okur.
- Yerel API yalnız bu bilgisayardan (`127.0.0.1`) ve jetonla erişilebilir; adlandırılmış kanal yalnız sizin Windows hesabınıza açıktır.
- API anahtarları Windows Credential Manager'da (`Cetvel/*`) saklanır; düz metin dosyaya yazılmaz.
- Veriler yalnız sizin kullanıcı klasörlerinizde durur (`%LOCALAPPDATA%\Cetvel`, `%APPDATA%\Cetvel`); kaldırırken silmeyi siz seçersiniz.
- Kurulum, yalnız WebView2 eksikse Microsoft'tan indirme yapar.
