# G-OpenSteamTool

[![Downloads](https://img.shields.io/github/downloads/G-Yoka/G-OpenSteamTool/latest/total?label=Downloads)](https://github.com/G-Yoka/G-OpenSteamTool/releases/latest)
[![往期下载量合计](https://img.shields.io/github/downloads/G-Yoka/G-OpenSteamTool/total?label=%E5%BE%80%E6%9C%9F%E4%B8%8B%E8%BD%BD%E9%87%8F%E5%90%88%E8%AE%A1)](https://github.com/G-Yoka/G-OpenSteamTool/releases)

🌐 语言 / Language：**简体中文** · [English](docs/README.en.md)

面向 Windows 的 OpenSteamTool 桌面管理器：把 Steam 目录、托管组件、游戏 Lua、Depot / DLC、数据通道、TOML 设置和运行日志收进一个工作区。

| 项目 | 说明 |
|---|---|
| 当前版本 | `v0.13.6` |
| 运行环境 | Windows 10 / 11 x64 |
| 技术栈 | Tauri 2 · Rust · React · TypeScript |
| 分发方式 | 便携单文件 EXE，DLL 资源已内嵌 |
| 更新方式 | 从 [Releases](https://github.com/G-Yoka/G-OpenSteamTool/releases) 手动下载覆盖 |

> 本项目只做桌面管理与本地资源整理，不编译 OpenSteamTool 上游源码，也不会生成、猜测或联网绕过授权获取当前账户本地不存在的 Depot Key。

## 界面预览

![概览（深色）](screenshots/ui-v0.4-overview.png)
![组件页](screenshots/ui-v0.4-components.png)

## 功能

### 工作区与组件

- 自动检测 Steam 安装目录，也可手动选择包含 `steam.exe` 的根目录；下次启动自动恢复。
- 扫描、安装、安全移除 `OpenSteamTool.dll`、`dwmapi.dll`、`xinput1_4.dll`，用 SHA-256 区分「托管 / 外部 / 缺失」，只删除哈希匹配的托管文件。
- 组件状态只以 `OpenSteamTool.dll` 是否被 Steam 进程加载为准；两个代理 DLL 未触发属于正常备用状态。
- 一键初始化：安装托管组件、写入默认 `opensteamtool.toml`、生成当前 Steam 构建的通道数据。

### 游戏配置

- 按 `G-<AppId>.lua` 管理配置，支持新建、编辑、启用 / 禁用（改文件名）、删除；导入支持一次多选多个 Lua。
- 「本地自动获取」读取 `appcache/appinfo.vdf` 与 `config/config.vdf`，一次补齐 Depot ID、Public Manifest GID 与本地已有的 Depot Key。
- **DLC 发现**：按 `extended.listofdlc` 枚举 DLC；缺失该字段的 App 会扫描整份 `appinfo`，按 `common.parent` / `extended.dlcforappid` 归属。DLC 的内容 Depot 依次取自：DLC 自身条目、主 App 上带 `dlcappid` 的 Depot、`packageinfo.vdf` 中已购包的 `depotids`。
- 发现结果按「主内容 / DLC 1..n」分组；已有本地 Key 的 DLC 默认勾选，加入时先写 `addappid(<DlcAppId>)` 再写内容 Depot 与 `setManifestid`。
- **重新获取只补空、不覆盖**：已填的 Depot Key、解锁标记与 Manifest 会按 AppId 继承，导入来的配置不会被一次获取清空。
- 已保存配置的两张表（02 Depot 解密、03 Manifest）都带「归属」列与分组条，逐行标出 `主体` / `DLC`；归属只读自本地 `appinfo` / `packageinfo`，不写文件。

### 数据通道

- 按当前 Steam 构建的 `steamclient64.dll` / `steamui.dll` SHA-256 管理 `pattern` 与 `ipc` 三个通道文件，缺失或不匹配时直接给结论。
- 应用内生成，不需要 Python 或外部仓库；上游 `IPCMessages.steamd` 已内置，输出与 `tools/pattern-tracker` 逐字节一致。
- 「全部生成」一次刷新三条通道并报告跳过项。

### 下载通道

- 按需写入 `<Steam>/config/lua/manifest.lua`（ManifestDeX 取码链，可前置自建端点），支持写入、更新、移除。
- 「下载通道自检」跟随当前下载区域：读取 `config.vdf` 的 `CurrentCellID`，取同区域 SteamCache 逐个尝试，失败再回退 `content_log.txt` 里的节点与 provider 提示，直接显示 HTTP 状态、耗时与清单字节数。
- 只接管带 `-- GOST-MANIFEST-LUA v1` 标记的文件，用户自管的 `manifest.lua` 不会被覆盖。

### 诊断与日志

- 汇总崩溃转储、缺失条目与被抑制异常，并把崩溃偏移归因到 `OpenSteamTool.dll` 或 Steam 侧。
- 读取并筛选 `<Steam>/opensteamtool/*.log`，按级别与关键字过滤。
- 后台线程安全关闭 / 重启 Steam，不阻塞界面、不重复启动。
- 遇到「无互联网连接」时见 [docs/NO_INTERNET.md](docs/NO_INTERNET.md)，可用 [tools/collect-diagnostics.ps1](tools/collect-diagnostics.ps1) 一键收集日志。

### 交互

- 命令面板 `Ctrl K`；`Ctrl 1…8` 切换页面，`Ctrl R` 重新扫描，`Ctrl F` 搜索，`Ctrl ⇧ L` 主题，`?` 快捷键总览。
- 明暗两套配色，跟随系统或手动固定并记忆。
- 纯手动更新：不含应用内更新、Release 查询、DoT / DNS 优化等网络功能。

## 下载与安装

前往 [Releases](https://github.com/G-Yoka/G-OpenSteamTool/releases/latest) 下载：

- `g-opensteamtool.exe`：完整便携版，无需安装，DLL 资源已内嵌。
- `SHA256SUMS.txt`：发布文件 SHA-256 校验值。

GitHub 自动生成的 `Source code (zip)` / `(tar.gz)` 是源码，不能直接运行。安装、升级与卸载细节见 [docs/INSTALL.md](docs/INSTALL.md)。

```powershell
# 校验下载文件（可选）
Get-FileHash .\g-opensteamtool.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

## 快速开始

1. 完全退出 Steam，启动 G-OpenSteamTool。
2. 确认自动检测到的 Steam 目录，或手动选择包含 `steam.exe` 的根目录。
3. 点击「一键初始化」：安装托管组件、创建默认 TOML、生成通道数据。
4. 进入「游戏配置」，输入 App ID，点「本地自动获取」。
5. 在「主内容 / DLC」分组里核对 Depot、Key、Manifest，勾选需要的 DLC，保存为 `G-<AppId>.lua`。
6. 重启 Steam，使组件、TOML、通道与 Lua 生效。
7. Steam 更新后，回到「数据通道」点「全部生成」刷新通道文件。

## Depot 与 DLC 的本地发现

本地有三个文件承担不同职责：

```text
<Steam>/appcache/appinfo.vdf     App → 名称 / Depot / Public Manifest / DLC 索引
<Steam>/appcache/packageinfo.vdf 已购包 → appids / depotids
<Steam>/config/config.vdf        Depot → DecryptionKey
```

合并规则：

- **Depot 列表**取自 App 的 `depots` 节点，带 `dlcappid` 的条目归给对应 DLC，不重复计入主内容。
- **DLC 列表**先读 `extended.listofdlc`，再扫全量 `appinfo` 补 `common.parent` / `extended.dlcforappid` 指向本 App 的条目。
- **Key** 只从 `config.vdf` 的 `depots` 映射读取，按 Depot ID 精确匹配；没有就是没有，不会联网补。
- 保存配置时，发现结果覆盖不到的旧 Key / Manifest 会保留，避免误清空。

DLC 的 Depot 若在本地既没有 `appinfo` 条目、账号也没有对应已购包记录，面板会显示「无 depot 数据」。想让它出现：在 Steam 里打开该 DLC 的商店页停留一下，让客户端缓存 `appinfo`，再重新获取。

没有 GUI 时可以直接查同一份结果：

```powershell
cd app
cargo run --example dump_discovery -- "D:\Steam" 2879840
cargo run --example dump_discovery -- "D:\Steam" 2879840 --ownership
```

## 常见问题

**DLC 列表比商店页少？**
本地 `appinfo.vdf` 只缓存客户端请求过的条目。商店页有、但本地没缓存的 DLC 不会凭空出现；打开该 DLC 页面刷新缓存后再获取。

**DLC 分组里有 Key，表格里却是空？**
Key 属于 DLC 的内容 Depot 行（例如 `4575721`），不在 DLC 的 AppId 行（`4575720`）上。分组条右侧的 `x/y 含 Key` 就是这一组的实际数量。

**重新点「本地自动获取」后 Key 没了？**
v0.13.4 起改为按 AppId 继承：发现结果没有的 Key / Manifest 会保留原值。

**组件显示「未加载 · 备用」？**
只有 `OpenSteamTool.dll` 必须进入 Steam 进程；两个代理 DLL 取决于本次启动解析到哪条通道，未触发不影响功能。

**Steam 更新后游戏打不开 / 下载报错？**
先到「数据通道」点「全部生成」，再重启 Steam；下载报「无互联网连接」见 [docs/NO_INTERNET.md](docs/NO_INTERNET.md)。

## 安全边界

- DLL 移除只处理本工具安装且哈希匹配的文件。
- Lua 管理只操作 `G-*.lua` 与 `G-*.lua.disabled`，不改写其他 Lua。
- TOML、Lua、安装清单与 `manifest.lua` 均采用临时文件 + 回滚写入。
- Depot Key 只在本地读取并写入用户选择的配置，不进入应用日志。
- 不含应用内更新、Release 查询、DoT 或 DNS 优化代码。

## 从源码构建

需要 Node.js、Rust MSVC 工具链与 Visual Studio Build Tools（Desktop development with C++）。

```powershell
cd app
npm ci
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run build:tauri-release
```

产物：

```text
app/src-tauri/target/release/g-opensteamtool.exe
```

测试基线：`cargo test` 覆盖 lib 26、集成 38、patterns 5；`npx tsc --noEmit` 通过。

## 仓库结构

```text
G-OpenSteamTool/
├── app/
│   ├── src/                     React / TypeScript 前端
│   ├── src-tauri/
│   │   ├── src/                 Rust 后端与 services 分层
│   │   ├── tests/               Rust 集成测试
│   │   ├── examples/            命令行自检（发现 / 归属 / 取码）
│   │   ├── resources/dlls/      编译进 EXE 的 DLL 资源
│   │   └── icons/               应用图标
│   ├── package.json
│   └── vite.config.ts
├── docs/                       安装、变更记录、设计与通道文档
├── tools/                      通道生成参考实现与诊断脚本
├── screenshots/
├── README.md
└── .gitignore
```

## 文档

- [安装、升级与卸载](docs/INSTALL.md)
- [版本变更记录](docs/CHANGELOG.md)
- [「无互联网连接」诊断与日志收集](docs/NO_INTERNET.md)
- [数据通道生成（pattern / ipc）](docs/PATTERN.md)
- [界面设计与文案规范](docs/DESIGN.md)
- 发布说明：[v0.13.6](docs/releases/v0.13.6.md) · [v0.13.5](docs/releases/v0.13.5.md) · [v0.13.4](docs/releases/v0.13.4.md) · [v0.13.3](docs/releases/v0.13.3.md) · [v0.13.2](docs/releases/v0.13.2.md) · [更多](docs/releases)
- [English README](docs/README.en.md)

## 说明

G-OpenSteamTool 是非官方社区项目，与 Valve、Steam 或 OpenSteamTool 上游作者不存在隶属关系。使用前请确认符合所在地区法律、Steam 协议及相关软件许可；用户对自己的配置与使用结果负责。
