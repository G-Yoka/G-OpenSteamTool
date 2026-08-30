# 安装、升级与卸载

## 系统要求

- Windows 10 或 Windows 11 x64
- 已安装 Steam
- 对 Steam 安装目录具有读写权限

## 便携版

1. 从 [GitHub Releases](https://github.com/G-Yoka/G-OpenSteamTool/releases/latest) 下载 `g-opensteamtool.exe`。
2. 退出 Steam。
3. 将 EXE 放到独立目录后直接运行；托管 DLL 资源已内嵌，不需要安装器或额外资源目录。

不要把程序本体放入 Steam 根目录。

## 首次初始化

1. 在概览页选择 Steam 根目录。
2. 点击“一键初始化”。
3. 在组件页确认三个托管 DLL 状态正常。
4. 在设置页检查 `opensteamtool.toml` 参数。
5. 添加游戏配置后重启 Steam。

## 从旧版本升级

0.3.0 不再包含应用内更新功能，请从 Releases 手动下载。

- 便携版：退出旧版程序后替换 EXE。
- Steam 目录选择保存在应用本地存储中；安装环境变化后可能需要重新选择一次。
- 现有 `opensteamtool.toml`、`config/lua/G-*.lua` 与日志不会因更新管理器而自动删除。

## 卸载

删除 G-OpenSteamTool EXE 不会自动删除 Steam 目录中的 OpenSteamTool 配置。若需要移除托管 DLL，请先在应用“组件”页执行安全移除，再删除程序。

不要直接批量删除 Steam 根目录文件；应用只会移除哈希匹配的托管 DLL，以避免误删其他软件安装的同名文件。

## 下载校验

在 PowerShell 中执行：

```powershell
Get-FileHash .\g-opensteamtool.exe -Algorithm SHA256
```

将结果与 Release 中的 `SHA256SUMS.txt` 对比。
