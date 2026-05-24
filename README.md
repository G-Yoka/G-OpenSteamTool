# OpenSteamTool Manager

这是一个基于 `.NET 8 WPF` 的 OpenSteamTool 可视化管理器，用于在 Windows 上管理 Steam 根目录中的 OpenSteamTool 部署、配置、Lua 脚本和日志状态。

## 功能

- 选择并校验 Steam 根目录。
- 安装、移除和校验 `OpenSteamTool.dll`、`dwmapi.dll`、`xinput1_4.dll`。
- 使用 SHA-256 对比 Payload DLL 与 Steam 目录 DLL。
- 检查 `OpenSteamTool.dll` 是否已真实加载到 `steam.exe`。
- 编辑 `opensteamtool.toml`。
- 按“每游戏一个 Lua 文件”管理 `<Steam>\config\lua\ost_<appid>.lua`。
- 导入现有 Lua 文件并接管为管理器配置。
- 查看 `<Steam>\opensteamtool\*.log` 日志。
- 快速重启 Steam。

## 项目结构

```text
manager/
  OpenSteamTool.Manager/
    Views/
    ViewModels/
    Models/
    Services/
    Helpers/
    Converters/
    Payload/
```

当前保持单项目结构，不拆分类库。WPF 界面使用 MVVM 组织，核心第三方依赖仅为 `CommunityToolkit.Mvvm`。

## 构建

```powershell
dotnet restore manager\OpenSteamTool.Manager\OpenSteamTool.Manager.csproj
dotnet build manager\OpenSteamTool.Manager\OpenSteamTool.Manager.csproj -c Release
```

构建产物位于：

```text
manager\OpenSteamTool.Manager\bin\Release\net8.0-windows\
```

## Payload DLL

应用安装 DLL 时会读取：

```text
manager\OpenSteamTool.Manager\Payload\
```

需要包含：

- `OpenSteamTool.dll`
- `dwmapi.dll`
- `xinput1_4.dll`

如果仓库不适合分发二进制 DLL，可以删除 Payload 中的 DLL，只保留 `Payload\README.md`，由使用者自行放入对应文件。

## 管理的 Steam 文件

- DLL：`<Steam>\OpenSteamTool.dll`
- DLL：`<Steam>\dwmapi.dll`
- DLL：`<Steam>\xinput1_4.dll`
- TOML：`<Steam>\opensteamtool.toml`
- Lua：`<Steam>\config\lua\ost_<appid>.lua`
- 日志：`<Steam>\opensteamtool\*.log`

## 注意

- 安装或移除 DLL 前需要关闭 Steam。
- Lua 和 TOML 可以在 Steam 运行时编辑，但某些配置需要重启 Steam 后才会重新加载。
- “DLL 已加载”状态来自 `steam.exe` 当前模块列表，不等同于文件是否存在。
