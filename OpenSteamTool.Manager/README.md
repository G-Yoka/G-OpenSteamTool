# OpenSteamTool Manager

OpenSteamTool Manager 是一个基于 .NET 8 WPF 的 Windows 桌面管理器，用于简化 OpenSteamTool 的部署、配置和状态查看。

它不会修改 OpenSteamTool 的 C++ Hook 逻辑，只负责管理 Steam 根目录下的 DLL、`opensteamtool.toml`、Lua 配置文件和日志。

## 功能

- 选择并校验 Steam 根目录，要求目录中存在 `steam.exe`。
- 安装或移除 OpenSteamTool 所需 DLL。
- 安装前自动备份 Steam 根目录中已有的同名 DLL。
- 移除时优先恢复备份；没有备份时只删除与内置 Payload 匹配的 DLL。
- Steam 正在运行时禁止 DLL 安装/移除，但允许编辑 Lua 和 TOML。
- 使用表单编辑 `opensteamtool.toml`。
- 按“每个游戏一个文件”的方式管理 Lua 配置。
- 支持启用/禁用单个游戏 Lua，禁用时改名为 `.lua.disabled`。
- 查看 DLL、TOML、Lua、日志和 Steam 进程状态。
- 读取 `<Steam>\opensteamtool\*.log`，支持按日志模块筛选。

## 构建

在仓库根目录运行：

```powershell
dotnet restore OpenSteamTool.Manager\OpenSteamTool.Manager.csproj
dotnet build OpenSteamTool.Manager\OpenSteamTool.Manager.csproj -c Release
```

构建产物位于：

```text
OpenSteamTool.Manager\bin\Release\net8.0-windows\
```

## Payload DLL

发布或使用 DLL 安装功能前，需要将 OpenSteamTool 的发布 DLL 放入：

```text
OpenSteamTool.Manager\Payload\
```

需要包含：

- `OpenSteamTool.dll`
- `dwmapi.dll`
- `xinput1_4.dll`

项目会把 `Payload\*.dll` 复制到输出目录。若缺少这些 DLL，应用仍可启动，但状态页会显示 `Payload 缺失`，安装操作会被阻止。

## 管理的文件

应用只会写入用户选择的 Steam 根目录：

- DLL：`<Steam>\OpenSteamTool.dll`
- DLL：`<Steam>\dwmapi.dll`
- DLL：`<Steam>\xinput1_4.dll`
- 备份：`<Steam>\opensteamtool-manager\backup\`
- TOML：`<Steam>\opensteamtool.toml`
- Lua：`<Steam>\config\lua\ost_<appid>.lua`
- 禁用 Lua：`<Steam>\config\lua\ost_<appid>.lua.disabled`
- 日志：`<Steam>\opensteamtool\*.log`

## Lua 管理规则

管理器创建的 Lua 文件会以以下标记开头：

```lua
-- OpenSteamTool Manager
```

自动编辑只处理带有该标记的文件。没有该标记的用户手写 Lua 文件会被忽略，避免误改。

每个游戏默认生成一个独立文件：

```text
ost_<appid>.lua
```

禁用该游戏时，文件会改名为：

```text
ost_<appid>.lua.disabled
```

## TOML 配置

管理器提供表单编辑以下字段：

- `[log].level`
- `[manifest].url`
- `timeout_resolve_ms`
- `timeout_connect_ms`
- `timeout_send_ms`
- `timeout_recv_ms`
- `[lua].paths`

首次保存 TOML 前，如果 Steam 根目录中已存在 `opensteamtool.toml`，原文件会备份到：

```text
<Steam>\opensteamtool-manager\backup\opensteamtool.toml.original
```

当前实现会生成规范化 TOML，不保留原文件注释布局。

## 使用流程

1. 构建或发布 OpenSteamTool，取得三个 DLL。
2. 将三个 DLL 放入 `OpenSteamTool.Manager\Payload\`。
3. 构建并运行 OpenSteamTool Manager。
4. 选择 Steam 根目录。
5. 确认 Steam 未运行后安装 DLL。
6. 在 TOML 页面保存基础配置。
7. 在 Lua 游戏页面为每个游戏创建独立配置。
8. 在状态页和日志页检查运行状态。

## 注意事项

- DLL 安装/移除要求 Steam 关闭。
- Lua/TOML 编辑可以在 Steam 运行时进行。
- OpenSteamTool 当前只扫描 `<Steam>\config\lua` 目录顶层的 `.lua` 文件，因此管理器默认不把单个游戏 Lua 放入子目录。
- 若 Steam 更新导致 OpenSteamTool Hook 签名失效，管理器只能显示日志和文件状态，不能修复 Hook 兼容性问题。
