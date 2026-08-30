# G-OpenSteamTool

🌐 Language: [简体中文](../README.md) · **English**

G-OpenSteamTool is a Windows desktop manager for Steam-local OpenSteamTool components, per-game Lua files, TOML settings, Depot metadata, and logs.

- Current version: `v0.3.0`
- Platform: Windows 10/11 x64
- Stack: Tauri 2 / Rust / React / TypeScript
- Updates: manual downloads from [GitHub Releases](https://github.com/G-Yoka/G-OpenSteamTool/releases)

![G-OpenSteamTool 0.3.0 overview](../screenshots/overview-v0.3.0.png)

## Highlights

- Detects and remembers the Steam installation directory.
- Installs, scans, and safely removes managed OpenSteamTool DLLs with SHA-256 verification.
- Manages `opensteamtool.toml` and `G-<AppId>.lua` files.
- Reads App names, Depot IDs, and public manifests from local `appinfo.vdf`.
- Merges Depot keys already present in the local Steam `config.vdf`.
- Imports existing Lua configurations and preserves multiple Depot/Manifest entries.
- Reads local OpenSteamTool logs and restarts Steam without blocking the UI.
- Contains no in-app updater, GitHub network optimization, or custom DNS/DoT feature.

The application cannot create or retrieve Depot keys that are not already available in the current local Steam configuration.

## Install

Download the MSI installer or portable EXE from the [latest release](https://github.com/G-Yoka/G-OpenSteamTool/releases/latest). See [INSTALL.md](INSTALL.md) for detailed instructions.

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
