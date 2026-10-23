# 版本变更记录

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
