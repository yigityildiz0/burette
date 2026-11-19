# AGENTS.md

Guidance for AI coding agents and contributors working in this repository.

Burette (Turkish name: Cetvel) is a Windows-only AI usage-limit monitor: Rust 2024 workspace, Tauri 2 desktop app, plain HTML/CSS/JS UI. Product name in English docs and non-Turkish UI: **Burette**; in Turkish: **Cetvel**. Crate, binary, service and folder names stay `cetvel*`; do not rename them.

## Layout

| Path | Contents |
|---|---|
| `crates/cetvel-model` | Data model (`AnlikV2`, `HavuzAnlik`), effective-remaining logic, v1 compatibility output, JSON Schema generation |
| `crates/cetvel-tahmin` | Consumption rate (EWMA), depletion forecast, pressure labels |
| `crates/cetvel-baglayici` | Quota connectors: Codex (`app-server`), Antigravity (`agy -p /usage`), DeepSeek balance, OpenCode / DeepSeek Harness usage counters; Job Object helper |
| `crates/cetvel-ajan` | Read-only agent event reader (Codex session logs, Antigravity summary DB) for `/v1/ajanlar` |
| `crates/cetvel-depo` | Settings (`ayarlar.toml`), paths, atomic writes, user-only DACL, Credential Manager, SQLite history |
| `crates/cetvel-servis` | Service loop, per-pool state machine, HTTP API (`127.0.0.1`), named pipe, notifications, Claude statusLine bridge, Connect (`bagla`) flow |
| `crates/cetvel-mcp` | MCP stdio server |
| `crates/cetvel` | CLI binary `cetvel.exe` (`durum`, `oku`, `servis`, `mcp`, `kur`, `kaldir`, `durdur`, `dogrula`, `kopru`) |
| `crates/cetvel-masaustu` | Tauri 2 desktop app `cetvel-masaustu.exe`: tray icon, popover, window, strip; serves `apps/arayuz` |
| `apps/arayuz` | UI: `index.html`, `uygulama.js`, `stil.css`, `diller/*.json` (11 locales), `ornek-anlik.json` (demo data), `TASARIM.md` (design notes) |
| `apps/arayuz/araclar` | `dil_denetle.py` (locale check), `tasma_testi.mjs` (overflow test, needs Chrome or Edge) |
| `fixtures/` | Sample connector outputs used by tests |
| `schema/` | Generated JSON Schemas for `anlik.json` (v2) and `orkestra-uyum.json` (v1) |
| `paketleme/` | Installer build script, NSIS hooks, third-party notice generator |
| `docs/` | User install guides (`INSTALL.md`, `KURULUM.md`) |

## Build and check

Set `CARGO_TARGET_DIR` outside the repository if you work in a copy that must stay clean.

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node --check apps/arayuz/uygulama.js
python apps/arayuz/araclar/dil_denetle.py
```

All five must pass before a change is done. Optional:

```powershell
node apps/arayuz/araclar/tasma_testi.mjs                      # UI overflow test across widths, languages, scales
python paketleme/ucuncu_taraf_bildirimleri_uret.py --denetle  # THIRD-PARTY-NOTICES.txt up to date
cetvel dogrula --sema-yaz schema                              # regenerate schema/ after model changes
pwsh -File paketleme/kurulum-dosyasi-uret.ps1                 # installer (needs Tauri CLI 2)
```

Open `apps/arayuz/index.html?demo=1` from a local static server to see the UI with demo data.

## Rules

- **Language:** identifiers, comments and log text are Turkish. Keep that style in existing code.
- **UI strings:** every key exists in all 11 files under `apps/arayuz/diller/` with the same structure and `{placeholder}` tokens; `tr.json` is the reference. Turkish fallback strings also live in `YEDEK_TR` in `uygulama.js`. Run `dil_denetle.py` after any change.
- **Layout:** no text may be clipped or overlap at any supported width, language or scale.
- **Percentages:** the model stores `kalan` (remaining) and `kullanilan` (used) as 0..1 fractions; `etkin_kalan` is the lowest remaining across a pool's windows. The UI defaults to remaining and can switch to used.
- **Privacy:** never read or store prompt/response text, command lines, file paths, OAuth tokens or cookies. Codex `auth.json` is checked for existence only; Antigravity `hub_*.json` is never touched. Antigravity reading stays off unless the user allows it.
- **Secrets:** no hardcoded URLs, keys or user paths. Keys come from environment variables or Windows Credential Manager, are held in `GizliDeger` and never logged.
- **Network:** the HTTP API binds to loopback only and requires the Bearer token. Do not add outbound calls beyond the existing connectors.
- **Files:** write data with the atomic helpers in `cetvel-depo`; settings changes must not overwrite an unreadable `ayarlar.toml`.
- **Claude settings:** `~/.claude/settings.json` is changed only after a preview, a dated backup and explicit user approval.
- **Dependencies:** if `Cargo.lock` changes, regenerate `THIRD-PARTY-NOTICES.txt`.
- **Tests:** add or update tests with every behaviour change; tests must not touch the real user profile, Credential Manager or registry values outside test names.
