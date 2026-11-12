# 创意工坊下载

## 为什么 Steam 会提示「内容仍然处于加密状态」

Steam 的创意工坊物品不是直接解压到 `steamapps/workshop/content` 的：

- 新版物品（SteamPipe）会被打包成一个 **工坊 Depot** 的 manifest，Depot ID 通常等于游戏 AppId（`appinfo.vdf` 里的 `depots.workshopdepot`）。下载分块和文件名都用这个 Depot 的 **Depot Key** 加密。
- 旧版物品（legacy UGC）是 CDN 上的单个文件，Steam 客户端会从 `file_url` 直接下载。

解锁工具只负责让 Steam 认为「已拥有」，并不会凭空得到工坊 Depot 的 Key。Lua 里常见的 `addappid(<AppId>)` 没有 Key，Steam 仍然能取到 manifest 取码，但初始化 Depot 时会报：

```
Failed to initialize depot <AppId>, manifest ... (Missing decryption key)
```

界面上就是「内容仍然处于加密状态」。这不是工坊内容坏了，而是工坊 Depot 的 Key 没进 Lua。

## 本工具怎么处理

「创意工坊」页不经过 Steam 的下载队列：

1. 解析工坊链接 / 物品 ID，得到 `consumer_app_id`、工坊 Depot、manifest GID、文件大小与旧版 `file_url`。
2. 按以下顺序找工坊 Depot Key：
   - 当前 `G-<AppId>.lua` 里 Depot 对应的 `addappid` Key；
   - 本机 `config.vdf` 的 `Software/Valve/Steam/depots/<Depot>/DecryptionKey`（拥有游戏时通常有）；
   - `opensteamtool/workshop_keys.json` 缓存；
   - 设置页 `[workshop] key_url` 指定的密钥库（默认一个公开 JSON 库，可留空禁用）。
3. 从 Steam CDN 取 manifest（取码优先用 `manifest.steam.run`、`gmrc.wudrm.com`、ManifestDeX，也支持设置里的自建端点）。
4. 解压 manifest 的 `z` 条目，解出文件名 / 分块表；分块按 Depot Key 解密（AES-CBC + VZstd / Zip），校验 Adler32 与文件 SHA1。
5. 写入 `steamapps/workshop/content/<AppId>/<物品ID>`，并更新同库的 `appworkshop_<AppId>.acf`：

```
WorkshopItemsInstalled/<物品ID> = { size, timeupdated, manifest }
WorkshopItemDetails/<物品ID>   = { manifest, timeupdated, timetouched, latest_* }
```

重启 Steam 或重新启动游戏后，游戏通过 `ISteamUGC` 枚举已安装物品时就能看到它。

### 游戏内模组菜单看不到？

有些游戏（例如 **Probably Stolen**）的模组菜单只扫描游戏目录下的 `Mods/<模组名>/manifest.xml`，不会把 `steamapps/workshop/content` 当作已安装模组。工坊包里的 `README` 也通常要求把文件复制到 `Mods/<模组名>`。

下载页检测到 `<游戏目录>/Mods` 时会显示「同时安装到游戏 Mods 文件夹」开关（默认开启）：

- 从工坊包的 `manifest.xml` 读取 `<Assembly>`（如 `SmartInventoryArrange.Mono.dll`）或 `<ID>`，生成 `Mods/SmartInventoryArrange.Mono`；
- 只覆盖包内同名文件，保留用户自己放的配置；
- 覆盖前把旧目录备份到游戏目录外的 `Mods.gost-backup/<模组名>`，避免备份被模组加载器当成新模组；
- 重启游戏后，在模组菜单的「未激活」列表里启用。

如果游戏正在运行，复制 DLL 可能被占用而失败；先退出游戏再重试。

## Key 缺失怎么办

- **拥有游戏**：先到「游戏配置」点「本地自动获取」；如果本机 `config.vdf` 里有工坊 Depot Key，会显示在 Depot 列表里。
- **没有本地 Key**：在工坊下载面板的「手动 Key」填入 64 位十六进制；或者保持设置里的 `key_url` 可用，让工具联网查询。
- 工具不会生成、猜测或爆破 Key。密钥库里没有的 Depot 仍然会报错，这是预期行为。

## 限制

- 合集（collection）不会自动展开，请分别下载合集里的物品。
- 部分很老的工坊物品使用 LZMA 分块（`VZa`），当前版本只解 VZstd（`VSZa`）与 Zip（`PK`）分块；遇到时会明确提示。
- 旧版 UGC 物品只写单个文件，不需要 Key；不会创建额外的目录层级。
- 下载期间不要启动游戏，也不要让 Steam 同时写同一个 `appworkshop_*.acf`。工具会先替换目标物品目录；如果目录被占用会提示先关闭游戏 / Steam。
- 工坊内容仍受 Steam 协议与所在地区法律约束，请只下载你有权使用的内容。

## 相关设置

`opensteamtool.toml`：

```toml
[workshop]
key_url = "https://api.993499094.xyz/depotkeys.json"
```

留空则只使用 Lua / `config.vdf` / 本地缓存，不发起密钥库请求。
