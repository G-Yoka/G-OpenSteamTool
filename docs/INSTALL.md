# 安装、升级与卸载

## 系统要求

- Windows 10 或 Windows 11 x64
- 已安装 Steam
- 对 Steam 安装目录具有读写权限

## 安装版

1. 从 [GitHub Releases](https://github.com/G-Yoka/G-OpenSteamTool/releases/latest) 下载 `G-OpenSteamTool_0.3.0_x64_en-US.msi`。
2. 退出 Steam。
3. 运行 MSI 并完成安装。
4. 启动 G-OpenSteamTool，确认 Steam 根目录中存在 `steam.exe`。

## 便携版

下载 `g-opensteamtool.exe` 到独立目录后直接运行。不要从压缩包内部直接启动，也不要把程序本体放入 Steam 根目录。

## 首次初始化

1. 在概览页选择 Steam 根目录。
2. 点击“一键初始化”。
3. 在组件页确认三个托管 DLL 状态正常。
4. 在设置页检查 `opensteamtool.toml` 参数。
5. 添加游戏配置后重启 Steam。

## 从旧版本升级

0.3.0 不再包含应用内更新功能，请从 Releases 手动下载安装。

- MSI：退出旧版程序后运行新 MSI。
- 便携版：退出旧版程序后替换 EXE。
- Steam 目录选择保存在应用本地存储中；安装环境变化后可能需要重新选择一次。
- 现有 `opensteamtool.toml`、`config/lua/G-*.lua` 与日志不会因更新管理器而自动删除。

## 卸载

卸载 G-OpenSteamTool 本体不会自动删除 Steam 目录中的 OpenSteamTool 配置。若需要移除托管 DLL，请先在应用“组件”页执行安全移除，再卸载程序。

不要直接批量删除 Steam 根目录文件；应用只会移除哈希匹配的托管 DLL，以避免误删其他软件安装的同名文件。

## 下载校验

在 PowerShell 中执行：

```powershell
Get-FileHash .\g-opensteamtool.exe -Algorithm SHA256
Get-FileHash .\G-OpenSteamTool_0.3.0_x64_en-US.msi -Algorithm SHA256
```

将结果与 Release 中的 `SHA256SUMS.txt` 对比。
