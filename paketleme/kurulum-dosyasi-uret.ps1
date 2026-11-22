<#
.SYNOPSIS
  Cetvel Windows kurulum dosyasını üretir ve dağıtılacak ikililerde derleyen makinenin
  kullanıcı klasörü yolunun kalmadığını doğrular.

.DESCRIPTION
  Cargo, kayıt defteri (registry) crate'lerinin mutlak yollarını panic ve hata konumu
  dizgilerine gömer: `<kullanıcı klasörü>\.cargo\registry\src\...`. aws-lc-sys'in C kodu da
  `__FILE__` ile aynı yolu (uzun ve 8.3 kısa biçimiyle) taşır. Kurulum dosyası herkese açık
  dağıtılacağı için bu yollar Windows kullanıcı adını sızdırır.

  Bu betik:
    1. rustc'ye `--remap-path-prefix` verir (kullanıcı klasörü, CARGO_HOME, depo kökü) ve
       PE içindeki PDB yolunu yalnız dosya adına indirir (`/PDBALTPATH:%_PDB%`);
    2. MSVC C derleyicisine `/d1trimfile:` verir (CARGO_HOME\registry\src, uzun ve kısa biçim);
    3. `cargo tauri build` çalıştırır;
    4. `cetvel.exe`, `cetvel-masaustu.exe` ve (7-Zip varsa) kurulum dosyasının açılmış içeriğinde
       kullanıcı klasörü yolunu arar (ASCII/UTF-8 ve UTF-16, büyük/küçük harf duyarsız);
       bulursa 1 koduyla çıkar.

  Yollar çalışma anında hesaplanır; depoya kişisel yol yazılmaz. Ayarlar yalnız bu betiğin
  ortamında geçerlidir, bitince eski değerler geri yüklenir.

.PARAMETER YalnizDogrula
  Derlemeden yalnız mevcut çıktıları tarar.

.PARAMETER OnbellekTut
  Verilmezse betik başarıyla bitince target klasöründeki büyük ara çıktılar (debug, deps, build,
  incremental) silinir; kurulum dosyası (target\release\bundle) kalır. Aynı sürümü tekrar derlemek
  gerekirse bu bayrakla hızlı derleme için önbellek korunur.

.EXAMPLE
  pwsh -File paketleme\kurulum-dosyasi-uret.ps1
#>
[CmdletBinding()]
param([switch]$YalnizDogrula, [switch]$OnbellekTut)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3

$kok = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$profil = [Environment]::GetFolderPath('UserProfile')
$cargoEv = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $profil '.cargo' }
$cargoEv = $cargoEv.TrimEnd('\')

function Kisa-Yol([string]$yol) {
    try { return (New-Object -ComObject Scripting.FileSystemObject).GetFolder($yol).ShortPath }
    catch { return $yol }
}

# --- 1-2. Derleme ortamı -------------------------------------------------------------------
$kayitKaynak = Join-Path $cargoEv 'registry\src'
$izlenen = 'CARGO_ENCODED_RUSTFLAGS', 'CC_SHELL_ESCAPED_FLAGS'
$hostSatiri = (& rustc -vV | Select-String '^host:').ToString()
$hedef = $hostSatiri.Substring(5).Trim()
$cflagsAdi = 'CFLAGS_' + ($hedef -replace '-', '_')
$izlenen += $cflagsAdi
$eski = @{}
foreach ($ad in $izlenen) { $eski[$ad] = [Environment]::GetEnvironmentVariable($ad) }

# rustc birden çok eşleşmede SONUNCUYU uygular: genelden özele sıralanır.
$rustBayraklari = @()
if ($env:RUSTFLAGS) { $rustBayraklari += $env:RUSTFLAGS -split '\s+' | Where-Object { $_ } }
$rustBayraklari += "--remap-path-prefix=$profil=~"
$rustBayraklari += "--remap-path-prefix=$cargoEv=/cargo"
$rustBayraklari += "--remap-path-prefix=$kok=/cetvel"
$rustBayraklari += '-Clink-arg=/PDBALTPATH:%_PDB%'

$cBayraklari = @()
if ($eski[$cflagsAdi]) { $cBayraklari += $eski[$cflagsAdi] }
if ($hedef -like '*-msvc') {
    # Sondaki ters bölü tırnağı kaçırmasın diye önek ters bölüsüz verilir.
    foreach ($onek in @($kayitKaynak, (Kisa-Yol $kayitKaynak)) | Select-Object -Unique) {
        $cBayraklari += "`"/d1trimfile:$onek`""
    }
}

function Ortami-Uygula {
    $env:CARGO_ENCODED_RUSTFLAGS = $rustBayraklari -join [char]0x1f
    $env:CC_SHELL_ESCAPED_FLAGS = '1'
    [Environment]::SetEnvironmentVariable($cflagsAdi, ($cBayraklari -join ' '))
}

function Ortami-Geri-Al {
    foreach ($ad in $izlenen) { [Environment]::SetEnvironmentVariable($ad, $eski[$ad]) }
}

# --- 4. Tarama -----------------------------------------------------------------------------
$aranan = @($profil, (Kisa-Yol $profil), ($profil -replace '\\', '/')) | Select-Object -Unique
$latin1 = [Text.Encoding]::GetEncoding(28591)

function Say([string]$metin, [string]$igne) {
    $n = 0; $i = 0
    while (($i = $metin.IndexOf($igne, $i, [StringComparison]::OrdinalIgnoreCase)) -ge 0) { $n++; $i += $igne.Length }
    return $n
}

function Dosyayi-Tara([string]$yol) {
    $b = [IO.File]::ReadAllBytes($yol)
    $tek = $latin1.GetString($b)
    $cift0 = [Text.Encoding]::Unicode.GetString($b)
    $cift1 = if ($b.Length -gt 1) { [Text.Encoding]::Unicode.GetString($b, 1, $b.Length - 1) } else { '' }
    $toplam = 0
    foreach ($igne in $aranan) {
        $toplam += Say $tek ($latin1.GetString([Text.Encoding]::UTF8.GetBytes($igne)))
        $toplam += Say $cift0 $igne
        $toplam += Say $cift1 $igne
    }
    return $toplam
}

function Ciktilari-Tara {
    $sorun = 0
    $hedefler = @(
        (Join-Path $kok 'target\release\cetvel.exe'),
        (Join-Path $kok 'target\release\cetvel-masaustu.exe')
    )
    $kurulum = Get-ChildItem (Join-Path $kok 'target\release\bundle\nsis') -Filter '*-setup.exe' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $gecici = $null
    $yediZip = @("$env:ProgramFiles\7-Zip\7z.exe", "${env:ProgramFiles(x86)}\7-Zip\7z.exe") |
        Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    if ($kurulum -and $yediZip) {
        $gecici = Join-Path ([IO.Path]::GetTempPath()) ("cetvel-kurulum-tarama-" + [Guid]::NewGuid().ToString('N'))
        & $yediZip x $kurulum.FullName "-o$gecici" -y | Out-Null
        $hedefler += Get-ChildItem $gecici -Recurse -File | ForEach-Object FullName
    } elseif ($kurulum) {
        Write-Warning "7-Zip bulunamadı; kurulum dosyasının içi taranmadı (yalnız target\release ikilileri)."
    }
    try {
        foreach ($dosya in $hedefler) {
            if (-not (Test-Path $dosya)) { Write-Warning "Bulunamadı: $dosya"; $sorun++; continue }
            $n = Dosyayi-Tara $dosya
            $gorunen = if ($gecici -and $dosya.StartsWith($gecici)) { 'kurulum:' + $dosya.Substring($gecici.Length) } else { $dosya.Substring($kok.Length + 1) }
            if ($n -gt 0) { Write-Host "  SIZINTI  $gorunen : kullanıcı klasörü yolu $n kez" -ForegroundColor Red; $sorun++ }
            else { Write-Host "  temiz    $gorunen" }
        }
    } finally {
        if ($gecici -and (Test-Path $gecici)) { Remove-Item $gecici -Recurse -Force }
    }
    if ($kurulum) {
        $ozet = (Get-FileHash $kurulum.FullName -Algorithm SHA256).Hash
        Write-Host ("SHA-256  {0}  {1}" -f $ozet, $kurulum.Name)
    }
    return $sorun
}

# --- Akış ----------------------------------------------------------------------------------
if (-not $YalnizDogrula) {
    Ortami-Uygula
    try {
        Push-Location (Join-Path $kok 'crates\cetvel-masaustu')
        try {
            & cargo tauri build
            if ($LASTEXITCODE -ne 0) { throw "cargo tauri build $LASTEXITCODE koduyla bitti." }
        } finally { Pop-Location }
    } finally { Ortami-Geri-Al }
}

Write-Host 'Dağıtılacak ikililerde kullanıcı klasörü yolu taranıyor...'
$sorunSayisi = Ciktilari-Tara
if ($sorunSayisi -gt 0) {
    Write-Host "$sorunSayisi dosyada kullanıcı klasörü yolu ya da eksik dosya var; kurulum dosyasını yayımlamayın." -ForegroundColor Red
    exit 1
}
Write-Host 'Temiz: dağıtılacak ikililerde kullanıcı klasörü yolu yok.' -ForegroundColor Green

if (-not $OnbellekTut -and -not $YalnizDogrula) {
    $hedef = Join-Path $kok 'target'
    $silinecek = @('debug', 'tmp', 'release\deps', 'release\build', 'release\incremental', 'release\.fingerprint', 'release\examples', 'release\wix', 'release\resources') |
        ForEach-Object { Join-Path $hedef $_ }
    foreach ($yol in $silinecek) {
        if (Test-Path $yol) { Remove-Item $yol -Recurse -Force -ErrorAction SilentlyContinue }
    }
    Write-Host 'Büyük ara derleme çıktıları silindi (kurulum dosyası kaldı).' -ForegroundColor Green
}
