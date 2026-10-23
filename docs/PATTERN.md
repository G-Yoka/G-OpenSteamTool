# 数据通道生成（pattern / ipc）

> 面向维护者。**pattern 通道现在可以在应用内直接生成**：数据通道页每行的「生成」按钮会读取当前 Steam 构建的 DLL 并在本地完成解析，本页的 CLI 仍适用于 ipc 通道、CI 与离线批处理。

Steam 每次更新都会改变 `steamui.dll` / `steamclient64.dll` 的内容，三个数据通道因此都需要重新生成：

| 通道 | 文件名依据 | 作用 |
| --- | --- | --- |
| `pattern/steamui` | `steamui.dll` 的 SHA-256 | UI 侧函数 RVA + 字节签名（10 + 25 个钩子） |
| `pattern/steamclient` | `steamclient64.dll` 的 SHA-256 | package / manifest / netpacket / IPC 入口 |
| `ipc/steamclient` | `steamclient64.dll` 的 SHA-256 | 接口 vtable + 方法 wire 常量 |

## 生成

工具位于 `tools/pattern-tracker`，纯本地运行，零种子从 Steam 二进制里解析；第三方 feed 只用于回归校验。

```bash
# pattern（10 + 25 个钩子）
python -m ost_patterns.cli resolve <Steam>\steamui.dll --rules rules\steamui.toml --out-dir out --require-all
python -m ost_patterns.cli resolve <Steam>\steamclient64.dll --rules rules\steamclient.toml --out-dir out --require-all

# ipc（接口 vtable + 方法 wire 常量）
python -m ost_patterns.cli ipc <Steam>\steamclient64.dll --idl <src>\Steam\IPCMessages.steamd --out-dir out-ipc
```

产物文件名由对应 DLL 的 SHA-256 决定，例如 `out/steamui/<sha256>.toml`。

## 安装

1. 打开应用「通道」页，确认顶部显示的三个哈希与当前 Steam 构建一致。
2. 对每个通道点「安装 / 替换」，选择上一步生成的 TOML。
3. 应用按当前 `OpenSteamTool.dll` 的 SHA-256 把文件放进
   `<Steam>/opensteamtool/<channel>/<component>/`。
4. 重启 Steam 生效。

命令行参数、规则文件格式与测试见 [tools/pattern-tracker/README.md](../tools/pattern-tracker/README.md)。