# 「无互联网连接」诊断（2026-10-24 已定位到根因）

Steam 下载页把「拿不到 manifest request code」显示成 **无互联网连接**。
所以这不是网速或 DNS 的问题：先定位取码链断在哪一层。

**现在的结论**：v0.12–v0.13 生成的 `manifest.lua` 和 OpenSteamTool DLL 的「逐行分块」Lua 加载器
不兼容 —— 文件里的顶层 `local` 在块结束时就被销毁，`fetch_manifest_code_ex` 每次调用都在
`attempt to index a nil value (global 'memo')` 上失败，DLL 于是回落 `[manifest] url`（已死的
`wudrm`），Valve 返回 `Access Denied`，Steam 显示「无互联网连接」。已复现、已修（见第 2 节）。

## 1. 便携版带得走什么，带不走什么

`g-opensteamtool.exe` 只是管理器和 DLL 资源包。真正让下载跑起来的是 Steam 目录里的四项，它们都在目标机器上，不会被 exe 带过去。

| 内容 | 位置 | 便携版是否自带 |
| --- | --- | --- |
| 管理器本体 | exe 所在目录 | 是 |
| `OpenSteamTool.dll` / `dwmapi.dll` / `xinput1_4.dll` | `<Steam>\` 根目录 | 资源内嵌，需要在目标机点「一键初始化」释放 |
| `opensteamtool.toml` | `<Steam>\` | 否 |
| `manifest.lua`（取码链） | `<Steam>\config\lua\` | 否 |
| `G-<AppId>.lua`（`addappid` + `setManifestid`） | `<Steam>\config\lua\` | 否 |
| `pattern` / `ipc` 通道 | `<Steam>\opensteamtool\{pattern,ipc}\<组件>\<sha256>.toml` | 否，且必须按**他那台机器**上 `steamclient64.dll` / `steamui.dll` 的 SHA-256 生成 |

## 2. 根因：`manifest.lua` 的写法与 DLL 的 Lua 加载器不兼容（已复现）

### 2.1 DLL 怎么读 Lua

`LuaConfig::ParseFile` 不是 `luaL_loadfile`，而是**逐行累积、能编译就立刻执行并清空**：

```cpp
while (std::getline(file, line)) {
    chunk += line;
    if (luaL_loadstring(L, chunk) == LUA_OK) { lua_pcall(L, 0, 0, 0); chunk.clear(); }
}
```

于是每个顶层语句各自成为一个 chunk。v0.13 之前生成的模板是这样的：

```lua
local SOURCES = { ... }          -- 块 1：执行完就丢
local memo = {}                  -- 块 2：执行完就丢
local function parse_code(body)  -- 块 3：执行完就丢
local function fetch(src, gid)   -- 块 4：执行完就丢
function fetch_manifest_code_ex(app_id, depot_id, gid)
    local key = ...              -- 块 5：这里引用的 memo/SOURCES/fetch 已经是 nil 全局
```

`fetch_manifest_code_ex` 本身是全局函数（能被 DLL 找到），但它引用的 upvalue 全是前几个 chunk 的
局部变量，调用时统统是 `nil`。

### 2.2 复现（用 Lua VM 跑同一套逐行加载逻辑）

```
generated manifest.lua (top-level locals + global function)
  top-level chunks executed: 18, parse/runtime errors: 0
  fetch_manifest_code=function, fetch_manifest_code_ex=function
  fetch_manifest_code_ex(...) -> ERROR: ...:3: attempt to index a nil value (global 'memo')
```

换成「所有状态都在函数体内 / 全局表」的版本，同一套加载逻辑返回真实 code：

```
self-contained version
  fetch_manifest_code_ex(...) -> 8164849757364629124   (ManifestDeX 实际返回值)
MATCH
```

### 2.3 失败链

```
fetch_manifest_code_ex(...) 报错 → 返回 false
  → fetch_manifest_code(...) 同样报错 → false
    → FetchActive(): [manifest] url = "wudrm" → http://gmrc.wudrm.com/manifest/<gid> 502
      → 交回 Steam 原生结果：未拥有 → Access Denied
        → content_log: Failed to get manifest request code, 'Access Denied'
          → 下载队列显示「无互联网连接」
```

朋友的诊断包里这条链完整出现（268 次 `Access Denied`、0 次成功），而 `addappid` / `setManifestid`
是生效的（`content_log` 有 `AppID 286160 config changed : added depots 286161`）——说明 DLL 装了、
通道命中了、Lua 也跑了，只是取码函数抛异常。

### 2.4 修复

`render_manifest_lua` 改为自包含写法（v0.13 起）：

- 只有两个全局表：`GOST_MANIFEST_SOURCES`、`GOST_MANIFEST_MEMO`；
- 所有 `local` 都在函数体内；
- 单元测试 `render_has_no_top_level_local` 守住这条规则。

已生成好的文件：`outputs/manifest.self-contained.lua`（1488 B，
sha256 `3cb4b5cbe83e426d6bc98382bbb2f4dee5015586cee9497d61e9a72ed7a8d488`）。

替换时两个硬性要求：

1. **UTF-8 无 BOM**。带 BOM 时第一行 `-- GOST...` 就编译失败（`unexpected symbol near '<\239>'`），
   整份文件永远凑不成完整 chunk。
2. 用工具「写入 / 更新」生成，或原样复制文件，不要经过 GBK 编辑器（复制后变双编码会在注释里留下乱码）。

```powershell
# 校验落地成功
Get-FileHash "<Steam>\config\lua\manifest.lua" -Algorithm SHA256
# 第一行必须是 -- GOST-MANIFEST-LUA v1，且首字节是 2D 2D
[IO.File]::ReadAllBytes("<Steam>\config\lua\manifest.lua")[0..2] | % { $_.ToString('X2') }
```

### 2.5 为什么有的机器「本地一切正常」

因为那些机器根本没用上这条 Lua 兜底链 —— 取码这一关由客户端自己过了，坏掉的模板不会被调用。

判据在自己的 `content_log.txt` 里：

| 观察 | 含义 |
| --- | --- |
| 完全搜不到 `Failed to get manifest request code` | 客户端每次都拿到了 code（Valve 直接签发，或 provider 仍可用），Lua 没参与 |
| 成功行带 `?t=…&k=…` 或 `?auth_key=…`，并且有 `AuthenticateDepotID (<depot>) - Success!` | 客户端手上有该 depot 的 key/授权，取码不走兜底 |
| 有 `AppID <appid> failed to update ownership ticket (Access Denied)` | 这个 App 没有归属，**只有它**才会落到 Lua/provider 兜底 |

典型对照：出问题的机器 `Failed to get manifest request code` 出现 268 次、`failed to update ownership
ticket` 紧跟其后；正常的机器一次都没有。同一份 `manifest.lua`、同一个 DLL 哈希，在两边表现不同，
差别不在模板，而在「这次下载到底需不需要兜底取码」。

所以「我本地没问题」不能证明模板没问题 —— 只证明你本机没走到这条分支。

### 2.6 验证修好没有

```powershell
Select-String -Path "<Steam>\logs\content_log.txt" -Pattern "manifest request" | Select-Object -Last 10
```

出现 `manifest request received 200 (OK)` 就是通了；仍是 `Access Denied` 说明还停在旧文件（Steam 没重启 / 文件被写回旧模板）。

## 3. 其余四层（修好上面之后仍失败再查）

### 第 1 层：`manifest.lua` 不在他机器上

没有 `config\lua\manifest.lua` 时 DLL 只剩三个已死的内置源 → 必现「无互联网连接」。

- 证据：文件不存在，或没有 `-- GOST-MANIFEST-LUA v1` 首行。
- 修：设置页「写入 / 更新」。

### 第 2 层：自建端点跟着 toml 一起被带过去

`SOURCES` 里自建端点排在前面。`http://127.0.0.1:8787/manifest/` 只有你自己那台机器在跑；
他那边每次取码都会先打一个死端口，防火墙 DROP 时表现为等超时。

- 证据：`opensteamtool.toml` 里 `manifest_lua_endpoint = "http://127.0.0.1:8787/..."`。
- 修：清空端点 → 重新写入。分发时不要带这一项。

### 第 3 层：取码的账号归属校验

2026-09 起 Valve 对 manifest 取码做账号归属校验。ManifestDeX 返回 200 只说明它给了码。

- 证据：`content_log.txt` 里 `manifest request received 403/401`；自检表 `manifestdex` 200 但 CDN 4xx。
- 这一层的判定要在第 2 节修好之后再做，否则永远看到 Access Denied。

### 第 4 层：`pattern` / `ipc` 不匹配他的 Steam 构建

通道文件按组件 DLL 的 SHA-256 命名，Steam 每次更新都会变。

- 证据：数据通道页缺失；`<Steam>\opensteamtool\pattern\steamclient\<sha256>.toml` 与本地 DLL 哈希不符。
- 修：数据通道页「全部生成」。

### 第 5 层：DLL 没被 Steam 加载

`dwmapi.dll` 是注入点。先开 Steam 再装 DLL、或从另一个 Steam 根目录启动，都是「装了但没加载」。

- 证据：`content_log` 里连 `addappid` 的痕迹都没有（没有 `config changed : added depots`）。
- 修：完全退出 Steam → 装 DLL → 重启。

## 4. 需要哪些日志，在哪

Steam 根目录 = 含 `steam.exe` 的目录（默认 `C:\Program Files (x86)\Steam`，你截图里是 `D:\Steam`）。

| 优先级 | 文件 | 看什么 |
| --- | --- | --- |
| ★★★ | `<Steam>\logs\content_log.txt` | `manifest request received <状态码>`、`Failed to get manifest request code`、`config changed : added depots` |
| ★★★ | `<Steam>\config\lua\manifest.lua`、`G-*.lua` | 取码函数写法、编码/BOM、条目 |
| ★★ | `<Steam>\opensteamtool.toml` | `[manifest] url`、`[log] level`、`manifest_lua_endpoint` |
| ★★ | `<Steam>\logs\console_log.txt` | `Client version:` |
| ★★ | `<Steam>\logs\connection_log.txt` | CM 是否连上 |
| ★ | `<Steam>\opensteamtool\*.log`（`main.log` / `manifest.log` / …） | **只有 DLL 的 debug 构建才写**；便携版里那份 release 构建不写文件日志，目录里没有 `.log` 是正常的 |
| ★ | `<Steam>\dumps\*.dmp`、`steamclient64.dll`/`steamui.dll` 的 SHA-256 | 环境差异 |

## 5. 一次交齐：一键收集

```powershell
# 双击 tools\collect-diagnostics.cmd 也可以
powershell -NoProfile -ExecutionPolicy Bypass -File tools\collect-diagnostics.ps1 -SteamDir "D:\Steam"
```

产出 `桌面\gost-diag-<机器名>-<时间>.zip`：`MANIFEST.txt`（已摘关键行）、`opensteamtool\*.log`、
`logs\*.txt`、`config-lua\*.lua`、`opensteamtool.toml`、`config.vdf.redacted`、最近 dmp、proxy/hosts/dns。
Depot Key、`addtoken`、`setAppTicket`、`setETicket`、`setStat`、SteamID、AccountName 脱敏为 `<REDACTED-*>`。

## 6. 判读表

| 现象 | 结论 | 下一步 |
| --- | --- | --- |
| `Access Denied` + `manifest.lua` 是旧模板（顶层 `local`） | 取码函数抛异常 | 第 2 节：换自包含版本 |
| 换成新模板后仍 `Access Denied`，但文件哈希正确 | 码被归属校验挡下 | 第 3 层：换取码源 / 账号 |
| 完全没有 `manifest request` 行 | Steam 没走到取码 | 第 4/5 层：通道、DLL 加载 |
| `manifest request received 403` | 有码但 CDN 拒绝 | 码与账号/清单不匹配 |
| `manifest request received 200` | 通了 | 看下载进度 |
