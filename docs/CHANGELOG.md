# 版本变更记录

## v0.3.0

- 完成 Tauri/React/Rust 工程重构，前端按 `app / features / shared` 分层，Rust 命令通过 `services` 连接领域实现。
- 全新应用图标，并同步到窗口、概览、关于页、EXE 与 MSI。
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
