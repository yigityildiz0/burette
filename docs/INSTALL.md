# Installing Burette (Windows)

> Türkçe: [KURULUM.md](KURULUM.md)

Burette is the English name of Cetvel. On Windows the app, the setup file, the Start menu entry, the service and the command-line tool keep the original name **Cetvel** / `cetvel.exe`, so the steps below use that name.

Cetvel installs like a regular Windows app: a single setup file (`Cetvel_<VERSION>_x64-setup.exe`), no administrator rights, shows up as "Cetvel" in the Start menu and Windows search, and is removed from Settings > Apps.

| | |
|---|---|
| System | Windows 10 / 11, 64-bit |
| Rights | No administrator needed (installs for the current user only) |
| Install location | `%LOCALAPPDATA%\Programs\Cetvel` |
| Language | Turkish / English, chosen when setup starts |
| Dependency | Microsoft WebView2 (built into Windows 11; setup downloads it from Microsoft if missing) |

## 1. Manual install

1. Open the [Releases page](https://github.com/yigityildiz0/burette/releases) of this repository.
2. Download **`Cetvel_<VERSION>_x64-setup.exe`** from the latest release.
3. (Recommended) Check that the file is intact and unmodified. In PowerShell:

   ```powershell
   Get-FileHash "$env:USERPROFILE\Downloads\Cetvel_<VERSION>_x64-setup.exe" -Algorithm SHA256
   ```

   The value must match the SHA-256 published on the release page. If it does not, do not run the file.
4. Double-click the file. If SmartScreen warns you, see [Troubleshooting](#smartscreen-warning).
5. Pick a language → **Next** → confirm the install folder → **Install**.
6. If **Run Cetvel** is ticked on the last page, the app opens and the Cetvel icon appears in the tray.

### What setup does

| Step | Details |
|---|---|
| Copies files | `cetvel-masaustu.exe` (app), `cetvel.exe` (background service and command line), `uninstall.exe` |
| Start menu shortcut | `Cetvel`: found by Windows search, opens the app and lets notifications show under the Cetvel name |
| Uninstall entry | "Cetvel" in Settings > Apps and Control Panel > Programs and Features |
| Service registration | Task Scheduler task `CetvelServis` in the root folder: runs `cetvel.exe servis --gizli` with user rights when this user signs in and restarts it if it crashes. The task is bound to the installing user's SID, so no administrator rights are needed. If the task is still refused (e.g. a policy restricts Task Scheduler: "Access is denied"), setup falls back to `CetvelServis` = `"<install>\cetvel.exe" servis --gizli` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`: the service starts at sign-in without a window but is not restarted automatically if it crashes. Either way setup also starts it right away. A `Cetvel\CetvelServis` task from earlier versions and the empty `Cetvel` task folder it leaves are removed |

What setup **does not** do: it does not ask for administrator rights, does not change `PATH`, does not ask for API keys, and does not touch Claude Code `settings.json` (the statusLine link is only made from the app's **Connect** screen, with your approval).

### Upgrading

Run the setup file of the new version. If a previous install is found the window shows two choices, and **the first one, "Uninstall before installing", is preselected.** To keep your settings and links as they are, **select "Do not uninstall"** and click Next. Going back to an older version shows the same window with the same default.

| | Do not uninstall (recommended) | Uninstall before installing (preselected) |
|---|---|---|
| How it works | The service is stopped, new files overwrite the old ones, the service is started again | The old version's uninstaller opens and does a full uninstall, then the new version is installed |
| Settings, measurement history, API keys | kept | kept; but **deleted** if you tick **Delete the application data** in the uninstaller |
| Claude statusLine link | kept | reverted; re-apply it from the Connect screen if you need it |
| "Start with Windows" | kept | turned off; turn it on again in the app settings |
| Service task, Start menu shortcut | kept | removed, recreated by the new install |

If you run the same version again, the preselected choice is **Add/Reinstall components**, which overwrites; **Uninstall Cetvel** does a full uninstall.

A silent install (`/S`) shows no window and always upgrades by overwriting. A passive install (`/P`) also overwrites a newer version, but reinstalling **the same version** with `/P` does a full uninstall first (the statusLine link and "Start with Windows" are reverted); use `/S` to repair the same version.

## 2. Install with an AI agent (copy and paste)

Give the text below to an AI coding agent (Claude Code, Codex, etc.). Before you send it, set `$version` in step 1 to the version you want (see the [Releases page](https://github.com/yigityildiz0/burette/releases); `0.1.0` is the first release).

```text
Install Cetvel on my Windows computer. Use PowerShell.

Rules:
- Do not ask for administrator rights or open an elevated terminal.
- Do not ask for, read or write any API key; do not touch Credential Manager.
- Do not change SmartScreen, Defender or antivirus settings; do not add exclusions.
- Do not touch the Claude Code settings.json file.
- If a step fails, stop, show me the output and tell me what you suggest I do.

Steps:
1. Download the setup file:
   $version = "0.2.0"
   $file = Join-Path $env:TEMP "Cetvel_${version}_x64-setup.exe"
   Invoke-WebRequest -Uri "https://github.com/yigityildiz0/burette/releases/download/v$version/Cetvel_${version}_x64-setup.exe" -OutFile $file
2. Compute the SHA-256 and compare it with the value on the release page:
   (Get-FileHash $file -Algorithm SHA256).Hash
   If it does not match, do not install and tell me.
3. Install silently (no administrator needed; location %LOCALAPPDATA%\Programs\Cetvel):
   Start-Process -FilePath $file -ArgumentList "/S" -Wait
4. Verify:
   $cetvel = Join-Path $env:LOCALAPPDATA "Programs\Cetvel\cetvel.exe"
   & $cetvel durum; "exit code: $LASTEXITCODE"
   Expected: exit code 0 (service running). Exit code 3 means the service is not running:
   & $cetvel kur --baslat; Start-Sleep -Seconds 5; & $cetvel durum; "exit code: $LASTEXITCODE"
5. Show the service registration (task or sign-in fallback):
   & $cetvel kur --kuru-calistir | Select-Object -First 15
   schtasks /Query /TN "CetvelServis"   # task path; reports "cannot find" on the fallback path
   (Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run).CetvelServis   # fallback path
6. Delete the setup file: Remove-Item $file
7. Give me a short summary: install location, `cetvel durum` exit code and output, which registration path is used (task or sign-in entry).
```

The command-line tool is in Turkish: `durum` = status, `kur` = install/register, `--baslat` = start, `durdur` = stop, `kaldir` = uninstall, `--veri-sil` = delete data.

### Setup parameters

| Parameter | Meaning |
|---|---|
| `/S` | Silent install: no window, no questions, language follows the system language |
| `/P` | Passive install: progress bar only |
| `/D=<folder>` | Install folder. Must come **last** and without quotes: `... /S /D=C:\Tools\Cetvel` |
| `/R` | With `/S` or `/P`: start the app when setup finishes |

Run setup as the signed-in user; do not run it from an elevated (administrator) terminal, otherwise the service task may be registered for that account.

## 3. Uninstalling

**Settings > Apps > Installed apps > Cetvel > Uninstall** (or Control Panel > Programs and Features).

The uninstall window has a **Delete the application data** checkbox; it is **unticked by default.**

| | Unticked (default) | Ticked |
|---|---|---|
| Program files, Start menu shortcut, uninstall entry | removed | removed |
| Service registration (`CetvelServis` task, older `Cetvel\CetvelServis` task, `Run\CetvelServis` fallback), running service | stopped and removed | stopped and removed |
| "Start with Windows" entry | removed | removed |
| Claude Code statusLine link | reverted to the original command (with a dated backup) | reverted to the original command |
| Settings (`%APPDATA%\Cetvel`) and measurement history (`%LOCALAPPDATA%\Cetvel`) | **kept** | removed |
| `Cetvel/*` API keys in Credential Manager | **kept** | removed |
| UI cache (`%LOCALAPPDATA%\dev.cetvel.masaustu`) | kept | removed |

Silent uninstall (for an AI agent):

```powershell
$uninstaller = Join-Path $env:LOCALAPPDATA "Programs\Cetvel\uninstall.exe"
Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait              # keeps data
Start-Process -FilePath $uninstaller -ArgumentList "/S /DELETEDATA" -Wait  # also deletes data and keys
```

`/VERISIL` is the Turkish alias of `/DELETEDATA`. The uninstaller copies itself to a temporary folder and runs from there, so the command may return early. To verify: after a few seconds `Test-Path (Join-Path $env:LOCALAPPDATA "Programs\Cetvel\cetvel.exe")` should be `False`, and `schtasks /Query /TN "CetvelServis"` should fail because the task no longer exists, and `(Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run).CetvelServis` should be empty.

## 4. Troubleshooting

### The service is not running

Symptom: a "service is off" banner in the app, `cetvel durum` exits with code 3.

1. Press **Start service** in the app (also in the tray menu).
2. Or in a terminal:

   ```powershell
   $cetvel = Join-Path $env:LOCALAPPDATA "Programs\Cetvel\cetvel.exe"
   & $cetvel kur --baslat     # re-registers the service (task, else sign-in entry) and starts it
   & $cetvel durum            # when the service is off, also prints how it is registered
   ```

3. Registration status: `& $cetvel durdur` summarises it when the service is off; for details on the task path use `schtasks /Query /TN "CetvelServis" /V /FO LIST`. On the fallback path, if the entry is disabled in Task Manager > Startup apps it will not start at sign-in; enable it there.
4. Logs: `%LOCALAPPDATA%\Cetvel\gunluk`
5. If the service was started from an administrator terminal, a normal user cannot stop it; run `cetvel durdur` in an administrator terminal, then `cetvel kur --baslat` in a normal one.

### Setup says a file cannot be written

Setup stops the running service first. If another process is using `cetvel.exe` (the Claude Code statusLine bridge, or `cetvel mcp` started by an MCP client), setup moves the old copy aside in the same folder as `cetvel-eski-<number>.exe` and writes the new one; the moved copy is deleted by the next install or by uninstall. On uninstall, a `cetvel.exe` that is still in use is moved to the temp folder. If you still get an error, close Claude Code and any MCP clients and stop the service by hand:

```powershell
& (Join-Path $env:LOCALAPPDATA "Programs\Cetvel\cetvel.exe") durdur
```

Then press **Retry** in setup. If the app is open, close it from the tray with **Quit**.

### Port conflict

The local API listens on `127.0.0.1` only, default port `43117`. If the port is taken, the service picks a free port by itself and writes it to `%LOCALAPPDATA%\Cetvel\baglanti.json`; the app and clients read the port from there. For a fixed port, edit `%APPDATA%\Cetvel\ayarlar.toml`:

```toml
[http]
port = 43117
otomatik_port = true   # false: the service does not start if the port is taken
```

Restart the service afterwards: `cetvel durdur`, then `cetvel kur --baslat`.

### SmartScreen warning

Cetvel is not code-signed yet, so Windows may show "Windows protected your PC".

1. First compare the file's SHA-256 with the value on the release page (see [Manual install](#1-manual-install)).
2. If it matches: **More info** → **Run anyway**.
3. If it does not match, delete the file and download it again.

Do not turn off SmartScreen or your antivirus and do not add exclusions. If your antivirus flags the file wrongly, report a false positive to the vendor and report it as an [issue](https://github.com/yigityildiz0/burette/issues).

### Notifications do not appear

Notifications are tied to the `Cetvel` Start menu shortcut. If the shortcut was deleted, `cetvel kur` recreates it. Check that Cetvel is allowed in Windows Settings > System > Notifications.

### WebView2 is missing

If the app window does not open, Microsoft Edge WebView2 Runtime may be missing. Setup normally downloads it from Microsoft; for offline machines install the "Evergreen" runtime from Microsoft's website.

## 5. Privacy

- Cetvel runs locally; it sends no telemetry or usage statistics and needs no account.
- It reads quota data from tools on your computer (Codex, Claude Code statusLine; Antigravity only if you explicitly allow it) and only from the balance endpoints of providers you have entered a key for.
- The local API is reachable only from this computer (`127.0.0.1`) and only with a token; the named pipe is open only to your Windows account.
- API keys are stored in Windows Credential Manager (`Cetvel/*`), never in plain-text files.
- Data stays in your own user folders (`%LOCALAPPDATA%\Cetvel`, `%APPDATA%\Cetvel`); you choose whether to delete it when uninstalling.
- Setup downloads from Microsoft only when WebView2 is missing.
