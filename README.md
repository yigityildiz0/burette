**English** | [Türkçe](README.tr.md)

<p align="center">
  <img src="apps/arayuz/logo.png" alt="Burette logo" width="96" height="96">
</p>

# Burette

**AI usage limits, measured.**

Burette is a local Windows app that shows how much of your AI coding tool limits you have left: Claude Code's 5-hour and weekly windows, Codex, the three Antigravity pools, and optionally a DeepSeek balance and local usage counters. It sits in the system tray, warns you before a limit runs out, and publishes the same numbers as JSON for your own scripts and agents.

It is for people who work with several AI coding tools at once and want to see, at a glance, which one still has room, without opening each tool's own usage page.

**Quick start:** download `Cetvel_<VERSION>_x64-setup.exe` from the [Releases page](https://github.com/yigityildiz0/burette/releases), check its SHA-256, run it (no administrator rights needed), and look for the icon in the system tray. Details: [Install](#install).

Burette is the English name of the project; its Turkish name is **Cetvel**. The crates, binaries (`cetvel.exe`, `cetvel-masaustu.exe`), Windows service and data folders use the name `cetvel`.

Status: version 0.1.0, an early release for Windows 10 and 11 (64-bit) only.

Burette is independent and is not affiliated with or endorsed by Anthropic, OpenAI, Google or DeepSeek. Their product names are used only to identify them.

## Supported platforms

| Platform | Pools | How it is read | Default |
|---|---|---|---|
| Claude (Claude Code) | `claude_max`: 5-hour and weekly windows | Claude Code passes `rate_limits` to its statusLine command; `cetvel kopru statusline` forwards them to the local service and still runs your previous statusLine command unchanged | Linked from the Connect tab, after a preview and your approval |
| Codex | `codex_plus`: 5-hour and weekly windows | Runs the local `codex.exe app-server` and calls `account/rateLimits/read`; falls back to the rate-limit events in `~/.codex/sessions` | On when Codex is installed and signed in |
| Antigravity | `agy_gemini`, `agy_claude_gpt`, `agy_argon` (three separate pools) | Runs `agy -p /usage` and parses its output | **Off**; you choose "on request" or "automatic" |
| DeepSeek (optional) | `deepseek`: account balance, not a quota | `GET https://api.deepseek.com/user/balance` with your API key | Off |
| OpenCode (optional) | `opencode`: usage counter, no quota | Session and token totals from the local `opencode.db` (or the older `storage/` JSON) | Off |
| DeepSeek Harness (optional) | `dsh`: usage counter, no quota | Session and token totals from `~/.dsh/sessions` | Off |

OpenCode and DeepSeek Harness have no official quota, so Burette only counts local usage and labels these cards "no quota · usage counter".

## What the percentages mean

- By default every bar and the tray icon show **remaining** capacity (`15% left`). Settings > General > *Primary percentage mode* switches the bars and tray to **used** (`85% used`).
- A pool can have several windows (for example 5-hour and weekly). Its headline number is the **effective remaining**: the lowest remaining value among its windows. The window that sets it is the **binding window**.
- If any window is at 0%, the whole pool is marked unusable, even when another window still shows 100%. Example: weekly at 0% and 5-hour at 100% means the pool is closed until the weekly reset.
- In the JSON files, `kalan` (remaining) and `kullanilan` (used) are fractions between 0 and 1, and `etkin_kalan` is the effective remaining.

## Features

- **Tray, popover and window:** a two-bar tray icon and its tooltip show the tightest pool; the popover lists every pool with thin bars and the time to reset; the expanded window adds per-platform detail and history. `Ctrl+Alt+K` shows or hides the popover.
- **Strip mode:** a thin, draggable bar at the top or side of the screen, with an optional click-through mode (preview).
- **Notifications:** Windows toasts when a pool falls below its threshold (defaults: Claude 5-hour 15%, Claude weekly 30%, others 20% remaining; adjustable per platform), when the current pace would run a window out before it resets, and when data goes stale. Quiet hours and per-platform mute are supported.
- **Forecasts:** consumption rate (EWMA) and an estimated depletion time per window, shown only when there are enough readings.
- **History:** readings are kept in a local SQLite database (raw samples for 90 days) and shown as charts.
- **Customisable cards:** reorder, pin or hide pools.
- **11 languages:** Turkish, English, German, French, Spanish, Portuguese (Brazil), Russian, Japanese, Korean, Simplified and Traditional Chinese.
- **Themes:** four colour themes, each in light, dark or follow-system mode.
- **For scripts and agents:**
  - `anlik.json` (snapshot schema v2) and `orkestra-uyum.json` (compact compatibility schema v1, read by [Orkestra](https://github.com/yigityildiz0/orkestra), a router that picks a model for each task by difficulty and remaining quota) are written atomically to `%LOCALAPPDATA%\Cetvel`. JSON Schemas are in [`schema/`](schema/).
  - A local HTTP API on `127.0.0.1` (default port 43117, Bearer token required), a per-user named pipe, and an MCP stdio server (`cetvel mcp`, tools `kota_anlik`, `havuz_detay`, `uygun_havuzlar`).
  - A command-line tool (see [Command line](#command-line)).

## Install

Requirements: Windows 10 or 11, 64-bit. No administrator rights are needed.

1. Download `Cetvel_<VERSION>_x64-setup.exe` from the [Releases page](https://github.com/yigityildiz0/burette/releases).
2. Compare its SHA-256 with the value on the release page, then run it.

The installer is not code-signed yet, so SmartScreen may warn you. Full instructions, silent install (`/S`), a ready-made prompt that lets an AI coding agent install it for you, upgrading and uninstalling: [docs/INSTALL.md](docs/INSTALL.md).

## First run

1. Setup registers a background service that starts when you sign in. If **Run Cetvel** is ticked on the last page, the app opens and its icon appears in the system tray; click the icon for the popover, or press `Ctrl+Alt+K`.
2. Codex shows up on its own if it is installed and signed in.
3. For Claude, open the **Connect** tab and approve the statusLine link (you see a preview first and a dated backup is made). Then use Claude Code once so it sends its first reading.
4. Antigravity, DeepSeek and the local usage counters stay off until you enable them in the app.

## Uninstall

Settings > Apps > Installed apps > Cetvel > Uninstall. Your settings and history are kept unless you tick **Delete the application data**. Silent uninstall and what is removed: [docs/INSTALL.md](docs/INSTALL.md#3-uninstalling).

## Troubleshooting

Service not running, port conflicts, SmartScreen, missing notifications or WebView2: [docs/INSTALL.md](docs/INSTALL.md#4-troubleshooting). Short answers are in the [FAQ](#faq).

## Build from source

You need the Rust toolchain (edition 2024, so Rust 1.85 or newer) with the MSVC target. The UI in `apps/arayuz` is plain HTML, CSS and JavaScript; there is no npm build step.

```powershell
cargo build --release                                  # cetvel.exe and cetvel-masaustu.exe
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

To build the installer, install the Tauri CLI (`cargo install tauri-cli --version "^2" --locked`) and run:

```powershell
pwsh -File paketleme/kurulum-dosyasi-uret.ps1   # -> target/release/bundle/nsis/Cetvel_<VERSION>_x64-setup.exe
```

The script remaps local build paths (`--remap-path-prefix`, `/d1trimfile:`) so your user folder does not end up in the binaries, then scans the output for it and prints the SHA-256. A plain `cargo tauri build` also works, but its output contains local paths and should not be published.

## Command line

The command-line tool uses Turkish command names.

```powershell
cetvel durum                 # status: reads the latest snapshot (exit code 3 if the service is not running)
cetvel oku --codex           # read once from a connector and print the snapshot JSON
cetvel servis                # run the background service in the foreground
cetvel kur --kuru-calistir   # preview service registration without changing anything
cetvel durdur                # stop the running service
cetvel dogrula               # validate the snapshot files against the schema
cetvel mcp                   # start the MCP stdio server
```

## Privacy

- **Runs locally.** No account, no telemetry, no analytics. The HTTP API listens only on `127.0.0.1` (and `[::1]` when available), accepts only loopback `Host` headers and requires a token; the named pipe is restricted to your Windows account.
- **No model requests.** Burette never sends prompts to any model. The only network request it makes itself is the optional DeepSeek balance call. Codex and Antigravity are queried through their own local command-line tools.
- **What it reads:**
  - Claude: the `rate_limits` data Claude Code passes to the statusLine command, and `~/.claude/settings.json` (to show and, with your approval, change the statusLine link; a dated backup is made first).
  - Codex: the output of `codex.exe app-server`, the rate-limit events in `~/.codex/sessions`, and whether `~/.codex/auth.json` exists (its contents are never read).
  - Antigravity: the output of `agy -p /usage`, only after you allow it. Its `hub_*.json` files are never read.
  - OpenCode and DeepSeek Harness: session counts and token totals only; never prompts, replies, titles or `auth.json`.
  - For the local agent API (`/v1/ajanlar`): Codex session logs and Antigravity's `conversation_summaries.db`, reduced to type, count, time and a hashed ID. No prompt text, command lines or file paths are kept.
- **What it writes:** settings in `%APPDATA%\Cetvel\ayarlar.toml`; snapshots, history, logs and connection info in `%LOCALAPPDATA%\Cetvel` (user-only file permissions). API keys go to Windows Credential Manager (`Cetvel/<name>`), never to plain-text files.
- **Uninstalling** keeps your data unless you tick *Delete the application data*.

## FAQ

**Why does a pool say "unusable" when one bar is full?**
Another window of the same pool is at 0%. The pool is limited by its tightest window; see [What the percentages mean](#what-the-percentages-mean).

**Claude shows no data.**
Claude data arrives only while Claude Code runs and refreshes its statusLine. Link the statusLine from the Connect tab, then use Claude Code once.

**Why is Antigravity off?**
Reading it means running `agy -p /usage`, which may conflict with third-party tool policies. You decide whether to allow it, on request or automatically.

**Does it work on macOS or Linux?**
No. Burette uses Windows-only APIs (Task Scheduler, Credential Manager, named pipes, toast notifications).

**Is my API key sent anywhere?**
The DeepSeek key is sent only to the DeepSeek balance endpoint. It is not written to logs or settings files.

## Contributing

Bug reports and ideas are welcome as [issues](https://github.com/yigityildiz0/burette/issues). Build, test and style rules for contributors and AI coding agents are in [AGENTS.md](AGENTS.md).

## License

Dual-licensed under `MIT OR Apache-2.0`: see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Licenses of bundled Rust components are listed in [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt), generated from `Cargo.lock`:

```powershell
python paketleme/ucuncu_taraf_bildirimleri_uret.py            # regenerate
python paketleme/ucuncu_taraf_bildirimleri_uret.py --denetle  # exit code 1 if outdated
```

The bundled fonts (Inter, Source Serif 4) are under the SIL Open Font License; see `apps/arayuz/fontlar/`.
