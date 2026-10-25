# G-OpenSteamTool

🌐 Language: [简体中文](../README.md) · **English**

G-OpenSteamTool is a Windows desktop manager for Steam-local OpenSteamTool components, per-game Lua files, TOML settings, Depot metadata, and logs.

- Current version: `v0.7.3`
- Platform: Windows 10/11 x64
- Stack: Tauri 2 / Rust / React / TypeScript
- Updates: manual downloads from [GitHub Releases](https://github.com/G-Yoka/G-OpenSteamTool/releases)

![G-OpenSteamTool overview](../screenshots/ui-v0.4-overview.png)

## Highlights

- Detects and remembers the Steam installation directory.
- Installs, scans, and safely removes managed OpenSteamTool DLLs with SHA-256 verification.
- Manages `opensteamtool.toml` and `G-<AppId>.lua` files.
- Reads App names, Depot IDs, and public manifests from local `appinfo.vdf`, and enumerates the App's DLC through `extended.listofdlc`.
- Merges Depot keys already present in the local Steam `config.vdf`.
- Auto-discovery groups results as “base content / DLC 1..n”: a DLC's depots come from its own `appinfo` entry, the base App's `dlcappid` depots, and the depots of owned packages in `packageinfo.vdf`; DLC that already has local keys is selected by default.
- Imports existing Lua configurations and preserves multiple Depot/Manifest entries.
- Reads local OpenSteamTool logs and restarts Steam without blocking the UI.
- Manages the three data channels (`pattern/steamui`, `pattern/steamclient`, `ipc/steamclient`) keyed by the current Steam build SHA-256.
- Diagnostics page summarizes crash dumps, missing entries, and suppressed exceptions, attributing offsets to `OpenSteamTool.dll` or the Steam side.
- Component page separates loaded from not loaded · standby; the verdict only depends on whether `OpenSteamTool.dll` is loaded by Steam.
- Command palette (`Ctrl K`) plus `Ctrl 1-8`, `Ctrl R`, `Ctrl F`, `Ctrl Shift L`, and `?` shortcuts.
- Dark and light themes that follow the system preference or can be pinned manually.
- Contains no in-app updater, GitHub network optimization, or custom DNS/DoT feature.

The application cannot create or retrieve Depot keys that are not already available in the current local Steam configuration.

## Install

Download the standalone `g-opensteamtool.exe` from the [latest release](https://github.com/G-Yoka/G-OpenSteamTool/releases/latest). The managed DLL resources are embedded, so no installer or companion resource directory is required. See [INSTALL.md](INSTALL.md) for detailed instructions.

## Build from source

Install Node.js, the Rust MSVC toolchain, and Visual Studio Build Tools with Desktop development with C++.

```powershell
cd app
npm ci
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run build:tauri-release
```

## Disclaimer

This is an unofficial community project and is not affiliated with Valve, Steam, or the upstream OpenSteamTool authors. Users are responsible for complying with applicable laws, platform agreements, and software licenses.
