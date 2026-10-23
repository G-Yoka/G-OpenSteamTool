# Third-party notice: IPCMessages.steamd

`IPCMessages.steamd` is copied verbatim from the OpenSteamTool upstream source
tree (`src/Steam/IPCMessages.steamd`) and is distributed under the same licence
as that project (GPL-3.0). It is embedded into the application only so the `ipc`
channel can be generated offline; it is not modified.

This repository already redistributes the OpenSteamTool binary
(`resources/dlls/OpenSteamTool.dll`), so the same obligations apply to this file.

Users can override the embedded copy from the settings page (the override path is
stored locally and never written into `opensteamtool.toml`).