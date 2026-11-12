# 版本变更记录

## v0.15.4

- 修复 Steam 客户端 `1794424036`（`steamclient64.dll` SHA `ebf46f…4096c`）更新后「全部生成」失败、Steam 同时弹出「Unsupported Steam Version」和「IPC spec missing」的问题：新版编译器把部分函数拆成带 `UNW_FLAG_CHAININFO` 的前导/函数体片段，解析器现在会沿 pdata 链还原完整函数，并从正确的函数入口生成 RVA；同时更新 `steamclient` 的 `GetOrAddAppData`、`KeyValues_FindOrCreateKey`、`ProcessPendingLicenseUpdates`、`SendCallbackToPipe` 规则。
- 「全部生成」改为各通道独立执行：某个 pattern 规则失效时只记入「跳过」，不再中止整轮，`ipc` 和其余通道仍会写入，避免两个弹窗同时出现。
- 通道页的 IPC 方法数不再把文件头里的 `funcHash` 字样算成一个方法。
- 工坊清单索引（`manifest.steam.run/api/workshop/<id>`）超时/502 时不再弹出「工坊清单索引不可用」警告：该接口只补充标题、Depot、大小等元数据，Steam `PublishedFileDetails` 可用时下载和安装仍然正常。
- 版本号在 `package.json`、`tauri.conf.json`、`Cargo.toml`、界面显示四处统一为 `0.15.4`。

## v0.15.3

- 修复「切到别的游戏还显示上一个游戏的工坊物品与进度」：切换左侧游戏时清掉上一游戏的物品卡与输入框，输入框内容按 App 分开记忆、切回可恢复；下载任务与结果仍保存在全局，但**只在对应 App 下显示**。底部状态栏照旧提示「正在下载工坊内容」，不再把别的游戏的物品挂到当前游戏下面。
- 修复「已安装到游戏 Mods」提示出现两次：同一条 note 之前被后端同时写进 `item.notes` 与 `report.notes`，物品卡和结果卡各渲染一次。现在物品卡过滤掉结果卡已有的 note，结果卡只保留一条（`游戏 Mods：<路径>（N 个文件）；重启游戏后在模组菜单启用。`）。
- 版本号在 `package.json`、`tauri.conf.json`、`Cargo.toml`、界面显示四处统一为 `0.15.3`。

## v0.15.2

- 修复下载进度「切走再回来就没了」：进度、当前物品与下载结果原来存在工坊页的局部 state，切换游戏或页面后组件重建就丢；现在统一放到 `useAppController`，`workshop-progress` 事件也在全局监听，不再依赖工坊页是否打开。后台下载照常继续，切回来进度条、百分比与结果卡都在，`done` 后自动收起。
- 修复「无法解密工坊文件名：Depot Key 不正确或 manifest 已损坏」的一类误报（实测物品 `3829696854`）：
  - manifest 里文件名的 base64 每 64 字符带硬换行，旧代码直接解码失败——解码前先剥掉所有空白；
  - manifest 中有 `flags & 0x40` 的目录条目（如 `Textures`、`Textures\Items`），旧代码把它们当文件 `File::create`，报「拒绝访问」——现在识别为目录并跳过。
  该物品实测 `files=228 bytes=13623703`，并自动安装到游戏 `Mods\CutePortraits`。
- 切换左侧游戏时清掉上一游戏的物品卡与输入框（输入按 App 分开记忆）；下载任务和上次结果仍然保留，但只在对应 App 下显示，切回即可看到，避免在 `Steam Input Configs` 下仍显示 `Probably Stolen` 的物品。
- 下载结果里的「已安装到游戏 Mods」提示去重：物品卡不再重复同一句，结果卡只保留一条目录/文件数说明。
- 版本号在 `package.json`、`tauri.conf.json`、`Cargo.toml`、界面显示四处统一为 `0.15.2`。

## v0.15.1

- 工坊下载支持**同时安装到游戏 `Mods` 文件夹**：部分游戏（实测 Probably Stolen）的模组菜单只扫描 `<游戏目录>/Mods`，不认 `steamapps/workshop/content`，下载完游戏里照样看不到。检测到 `Mods` 目录时下载面板默认勾选「同时安装到游戏 Mods 文件夹」，下载后自动复制过去。
- 模组目录名从包内 `manifest.xml` 的 `<Assembly>` / `<ID>` 推导（去掉 `.dll` 后缀），所以 `SmartInventoryArrange.Mono.dll` 这类会落到 `Mods\SmartInventoryArrange.Mono`，与游戏加载器期望的目录名一致。
- 覆盖前把已有文件备份到 `<游戏目录>/Mods.gost-backup/<模组名>`——放在 `Mods` **外面**，否则加载器会把备份目录当成第二个模组扫描（日志里的 `Duplicate mod ID` 就是这么来的）。只覆盖包内同名文件，用户自己的配置保留。
- 下载结果新增 `mods_dir` / `mods_files`：面板直接显示安装到哪个目录、复制了几个文件；没装到 Mods 时给出一条 note 说明原因。
- 工坊下载进度改由全局 controller 保存：下载中切到别的页面、或点了左侧别的游戏再回来，进度条和结果不会丢失。
- 工坊 manifest 的文件名 base64 先去掉换行再解码，修复部分物品（如 3829696854）报「无法解密工坊文件名：Depot Key 不正确或 manifest 已损坏」。
- 版本号在 `package.json`、`tauri.conf.json`、`Cargo.toml`、界面显示四处统一为 `0.15.1`。

## v0.15.0

- 新增「创意工坊」页：对有工坊的游戏粘贴链接或物品 ID，解析 App / 工坊 Depot / Manifest 后直接下载到 `steamapps/workshop/content/<AppId>/<物品ID>`，不再经过 Steam 的加密下载队列。
- 新增工坊 Depot Key 链路：优先读当前 Lua、本机 `config.vdf` 和工具缓存；缺失时可从可配置的密钥库查询，也可以在下载面板手动填入。密钥库只补工坊 Depot，不生成、不猜测 Key。
- 自动同步 `appworkshop_<AppId>.acf` 的 `WorkshopItemsInstalled` / `WorkshopItemDetails`，重启 Steam 或重新启动游戏后即可识别。
- 「本地自动获取」现在会把 `workshopdepot` 补进 Depot 列表：拥有游戏但 Lua 漏了工坊 Depot 时，Steam 自带的工坊下载也能正常解密。
- 支持旧版 UGC 工坊物品（`file_url` 直接下载）和新版 SteamPipe 工坊物品（manifest + 分块解密，支持 VZstd / Zip 分块）。
- 设置页新增 `[workshop] key_url`，可留空禁用联网密钥库；下载页显示进度、文件数、字节数与写入目录。
- 下载完成后可自动复制到游戏 `Mods/<模组名>`（读取 `manifest.xml` 的 `Assembly` / `ID` 决定目录名），解决游戏内模组菜单只扫描本地 `Mods`、看不到 Steam 工坊目录的问题。
- 版本号在 `package.json`、`tauri.conf.json`、`Cargo.toml`、界面显示四处统一为 `0.15.0`。

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
