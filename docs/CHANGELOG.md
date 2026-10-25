# 版本变更记录

## v0.13.6

- 修复 DLC 的内容 Depot 被同时标成「主体」和「DLC」：主内容列表不再收录带 `dlcappid` 的 Depot（`4575721` / `4575722` 这类行此前会先以主体身份出现）。
- 归属映射改为「DLC 覆盖主体」：同一 AppId 在两处出现时以 DLC 为准，02 / 03 两张表都生效。
- 两张表的显示顺序按归属整理（主体在前、DLC 在后），底层数组顺序不变，保存结果与原来一致。
- 发现面板补充说明：DLC 列表只统计本地 `appinfo.vdf` 里已有的条目，商店页有但未缓存的 DLC 不会凭空出现。

## v0.13.5

- 「03 Manifest」表格同样按归属区分：新增「归属」列（主体 / DLC）与分组条（`主内容` / `DLC · 名称`），并在分组条右侧显示该组条目数。
- Manifest 与 Depot 共用同一份 `depot_ownership` 映射，选配置时只查一次；`—` 表示本地查不到归属。

## v0.13.4

- 修复「本地自动获取」会抹掉已有 Key 的问题：重新获取时按 AppId 继承当前配置里已填的 Depot Key 与 Manifest GID，只补空、不覆盖。
- DLC 默认勾选条件增加「这份配置里本来就有该 DLC 或其 Depot 的行」：导入来的 DLC 配置不会被一次本地获取清掉。
- 分组条补充 Key 统计（`2/3 含 Key`），DLC 行与它的内容 Depot 行不再混为一谈。
- 新增命令行自检 `--ownership` 已在 v0.13.3 提供；本版无接口变化。

## v0.13.3

- 已保存的 `G-<AppId>.lua` 在编辑器里不再是一排看不出归属的行：Depot 表新增「归属」列，逐行标注「主体 / DLC」，并在切换归属处插入分组条（主内容、DLC · 名称）。
- 归属来自只读标注接口 `depot_ownership`：选中配置时静默读取本地 `appinfo` / `packageinfo`，不写文件、不弹提示；查不到的行显示 `—`。
- 命令行自检可用 `cargo run --example dump_discovery -- "<Steam 目录>" <AppId> --ownership` 查看同一份逐行归属。

## v0.13.2

- 修复「本地自动获取」在没有 `extended.listofdlc` 的 App 上完全列不出 DLC：枚举改为「`listofdlc` + 全量 appinfo 扫描」双路，子条目按 `common.parent` / `extended.dlcforappid` 归属到当前 App。Escape Simulator 2（2879840）此前显示「没有 DLC 记录」，现在能列出 Apocalypse DLC 及其内容 Depot。
- DLC 空状态提示改为可操作引导：先在 Steam 打开该游戏的 DLC 商店页刷新 `appinfo.vdf`，再重新获取。

## v0.13.0

- 「本地自动获取」现在同时枚举 DLC：从 `appcache/appinfo.vdf` 的 `appinfo.extended.listofdlc` 读出该 App 的全部 DLC AppId。
- DLC 内容 Depot 三源合并：DLC 自身的 `appinfo` 条目、主 App 上带 `dlcappid` 的 Depot、`appcache/packageinfo.vdf` 中已购包（`appids` → `depotids`）。
- Key 仍只取本地 `config.vdf`：每个 Depot 单独匹配 `DecryptionKey`，`key_found` 表示该 DLC 已有本地内容，结果按「有 Key 优先」排序。
- 发现结果在「Depot 解密」区按「主内容 / DLC 1..n」分组显示，每个 DLC 可单独加入或取消；加入时先写 `addappid(<DlcAppId>)`，再写内容 Depot 与 `setManifestid`。
- 「本地自动获取」提示语补充 DLC 统计（总数 / 已有内容数）。
- 新增命令行自检 `cargo run --example dump_discovery -- "<Steam 目录>" <AppId>`，与界面走同一段发现逻辑。
- 下载通道 `manifest.lua` 随包提供：设置页可写入 / 更新 / 移除，默认 ManifestDeX 取码链，支持自建端点与用户自管保护（v0.12.0 引入）。
- 「下载通道自检」改为跟随 Steam 下载地区：读 `config.vdf` 的 `CurrentCellID`，经 `IContentServerDirectoryService/GetServersForSteamPipe` 取同区域 SteamCache 逐个尝试，失败再回退最近 `content_log.txt` 节点与 provider 提示。
- 游戏配置导入支持一次多选多个 `.lua`：文件选择器可 `Ctrl` / `Shift` 多选，逐个解析后一次性写入；单个文件识别不出 AppId 只跳过它并在提示里给出文件名与原因，其余文件照常导入。

## v0.12.0

- 新增「下载通道 manifest.lua」：设置页可写入 / 更新 / 移除 `<Steam>/config/lua/manifest.lua`，默认内置 ManifestDeX 取码链（旧 `opensteamtool` / `wudrm` / `steamrun` 三源已因 Steam 归属校验失效）。
- 新增「下载通道自检」：按当前游戏探测自建端点、ManifestDeX 与三个旧源，再向 CDN 请求一次清单，直接给出 HTTP 状态、耗时与清单字节数。
- `opensteamtool.toml` 新增 `[manager]` 段保存托管开关与自建端点；DLL 忽略未知段。
- 默认 `manifest_url` 改为 `opensteamtool`，`validate_settings` 放行内置三个取值并校验自建端点必须是 http(s)。
- 只管理带 `-- GOST-MANIFEST-LUA v1` 标记的文件：用户自管的 `manifest.lua` 不会被覆盖或删除。

## v0.11.0

- 内置上游 `IPCMessages.steamd`，ipc 通道零输入即可生成。
- 设置页「IPC 定义文件」默认显示内置版本，指定覆盖后可一键恢复内置。
- 新增 `resources/ipc/NOTICE.md` 记录该文件的上游来源与许可。

## v0.10.0

- 一键初始化现在会同时生成通道数据：pattern 两条，ipc 在已记住 IDL 时一并生成。
- 通道页新增「全部生成」，一次刷新三条通道并报告跳过项。
- ipc 缺少 IDL 时只跳过该通道，不影响 pattern 写入与初始化结果。
- 生成时 steamclient64.dll 只解析一次，pattern 与 ipc 共用同一份 PE 映像。

## v0.9.1

- 修复禁用配置后 `G-<AppId>.lua` 与 `G-<AppId>.lua.disabled` 并存：写入时删除对侧文件，一个 AppId 只保留一份。
- `set_game_enabled` 在目标已存在时改为自愈，不再报错。
- 删除会清掉两种状态的 Lua。
- 游戏编辑器跟随磁盘状态刷新 `enabled`，避免保存把禁用配置写回启用。
- 编辑器里的启用/禁用开关改为直接调用后端改名：之前点了会被同步逻辑立刻弹回，看起来像点不动。

## v0.9.0

- 新增应用内 ipc 通道生成：从 `steamclient64.dll` 的 wrapper 提取 `funcHash` / `fencepost`，接口号与 `argc` 取自上游 IDL。
- `ipc.py` 的 IDL 解析、wrapper 定位与 TOML 输出移植为 Rust。
- 设置页新增「IPC 定义文件」，记录 `IPCMessages.steamd` 路径（仅本机）。
- 首次对 ipc 点「生成」自动弹出文件选择器并记忆路径。
- 输出与上游 Python 流水线逐字节一致。
## v0.8.0

- 新增应用内 pattern 通道生成：数据通道页「生成」按钮直接读当前 Steam 构建的 `steamui.dll` / `steamclient64.dll`，无需 Python 与仓库。
- `tools/pattern-tracker` 的 PE 读取、签名合成、规则引擎、RTTI 与 TOML 读写移植为 Rust，规则集编译期内嵌。
- 生成结果与 Python 流水线逐字节一致；写入采用临时文件 + 替换。
- 生成在后台线程执行，界面不阻塞。
- 通道页侧栏缺文件时提供「生成缺失通道」。
- ipc 通道仍需上游 `IPCMessages.steamd`，应用内会明确提示。
## v0.7.3

- 新增数据通道页：按当前 Steam 构建的 SHA-256 管理 `pattern` / `ipc` 三个通道文件。
- 新增诊断页：崩溃转储归因、缺失条目与被抑制异常汇总。
- 新增命令面板（`Ctrl K`）与 `Ctrl 1…8` / `Ctrl R` / `Ctrl F` / `Ctrl ⇧ L` / `?` 快捷键。
- 新增明暗主题，跟随系统并可记忆，首帧前生效。
- 界面重做：指纹条、指标条、结论条与右侧「下一步 + 最近日志」统一骨架；数据通道页改为与组件页一致的左右双栏。
- 组件页区分「已加载 / 未加载 · 备用」，结论只以 `OpenSteamTool.dll` 是否被加载为准。
- 设置页支持 `[remote] url_template`、`[ui] suppress_missing_popup` 与四项网络超时。
- 界面文案分层：机制说明下沉到悬浮提示与文档，术语改为中文状态词。
- 托管 DLL 资源体积 11.0 MB → 1.44 MB。
- 版本号在 `package.json`、`tauri.conf.json`、`Cargo.toml`、界面显示四处统一。

> v0.4.0 – v0.7.2 为本地迭代版本，未单独发布。
## v0.3.1

- 本地自动获取 Depot 时自动追加当前游戏 App ID，并生成 `addappid(<AppId>)`。
- 当前 App ID 已存在于自动发现结果时不会重复添加。

## v0.3.0

- 完成 Tauri/React/Rust 工程重构，前端按 `app / features / shared` 分层，Rust 命令通过 `services` 连接领域实现。
- 全新应用图标，并同步到窗口、概览、关于页与 EXE。
- 将三项托管 DLL 资源直接内嵌到便携 EXE，不再发布 MSI 安装器。
- 自动从 Steam 本地 `appinfo.vdf` 和 `config.vdf` 发现 App、Depot、Manifest 与已有 Depot Key。
- 修复窗口拖动、最小化、关闭按钮和 Steam 目录持久化。
- 修复关闭状态下重启 Steam 导致重复启动，以及同步等待造成界面无响应的问题。
- 移除应用内在线更新、更新通道、GitHub 网络优化和 DNS/DoT 优化。
- 配置与安装清单改用带回滚的安全写入。
- 仓库整理为 `app/`、`docs/`、`screenshots/` 发布型结构。

## v0.2.3

- 重构桌面 UI，完善窗口尺寸、控件交互和在线更新流程。

## v0.2.2

- 更新发布签名配置与版本产物。

更早版本请查看 Git 历史与 [GitHub Releases](https://github.com/G-Yoka/G-OpenSteamTool/releases)。
