# OpenSteamTool Manager Payload

Place release payload DLLs here before publishing the manager:

- `OpenSteamTool.dll`
- `dwmapi.dll`
- `xinput1_4.dll`

The WPF project copies `Payload\*.dll` to the output directory. If these files are missing, the UI shows `Payload 缺失` and install is blocked.
