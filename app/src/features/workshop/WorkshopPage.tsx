import {
  AlertTriangle, CheckCircle2, Download, FolderOpen, KeyRound, PackageSearch, RefreshCw, Search, ShieldCheck, X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { AppController } from "../../app/useAppController";
import type { WorkshopApp } from "../../types";
import { Badge, Field, Toggle } from "../../shared/ui";

const PHASE_LABEL: Record<string, string> = {
  resolve: "解析物品",
  key: "准备 Depot Key",
  manifest: "获取 manifest",
  download: "下载分块",
  verify: "校验文件",
  installed: "写入工坊记录",
  done: "完成",
};

export function WorkshopPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const [apps, setApps] = useState<WorkshopApp[]>([]);
  const [selectedAppid, setSelectedAppid] = useState<number | null>(() => {
    const saved = Number(localStorage.getItem("gost.workshopAppid") ?? "");
    return Number.isFinite(saved) && saved > 0 ? saved : null;
  });
  const [input, setInput] = useState("");
  const [keyOverride, setKeyOverride] = useState("");
  const [installMods, setInstallMods] = useState(true);
  const [error, setError] = useState("");
  const [loadingApps, setLoadingApps] = useState(false);
  const item = state.workshopItem;
  const progress = state.workshopProgress;
  const report = state.workshopReport;

  const selected = apps.find((app) => app.appid === selectedAppid) ?? null;
  const busy = state.activity !== "idle";

  useEffect(() => {
    if (!state.steamDir) return;
    let cancelled = false;
    setLoadingApps(true);
    void actions.loadWorkshopApps().then((list) => {
      if (cancelled) return;
      setApps(list);
      const preferred = state.selectedGame.appid;
      setSelectedAppid((current) => {
        if (current && list.some((app) => app.appid === current)) return current;
        return list.some((app) => app.appid === preferred) ? preferred : list[0]?.appid ?? null;
      });
    }).finally(() => { if (!cancelled) setLoadingApps(false); });
    return () => { cancelled = true; };
    // 只在 Steam 目录 / 配置库变化时重新拉取；actions 每次渲染都会换对象。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.steamDir, state.games.length, state.selectedGame.appid]);

  useEffect(() => {
    if (!selectedAppid) return;
    localStorage.setItem("gost.workshopAppid", String(selectedAppid));
    if (item && item.appid !== selectedAppid) actions.clearWorkshopSelection();
    setInput(localStorage.getItem(`gost.workshopInput.${selectedAppid}`) ?? "");
    setError("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedAppid]);

  useEffect(() => {
    if (item && selectedAppid && item.appid !== selectedAppid) actions.clearWorkshopSelection();
    // actions 每次渲染都会换对象；只跟随物品归属变化。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [item?.appid, selectedAppid]);



  useEffect(() => {
    setInstallMods(Boolean(item?.mods_available && !item?.legacy));
  }, [item?.published_file_id, item?.mods_available, item?.legacy]);

  const percent = useMemo(() => {
    if (!progress) return 0;
    if (progress.bytes_total > 0) return Math.min(100, (progress.bytes_done / progress.bytes_total) * 100);
    if (progress.chunks_total > 0) return Math.min(100, (progress.chunks_done / progress.chunks_total) * 100);
    return 0;
  }, [progress]);
  // 物品卡和结果卡都会带 notes；结果卡已经展示 Mods 目录，避免同一句出现两次。
  const itemNotes = item ? item.notes.filter((note) => !report?.notes.includes(note)) : [];
  const reportNotes = report
    ? report.notes.filter((note) => !note.startsWith("已安装到游戏 Mods："))
    : [];
  // 进度/结果只属于它自己的 App；切到别的游戏时不显示，切回后仍然能看到。
  const showProgress = Boolean(progress && (!state.workshopTaskAppid || state.workshopTaskAppid === selectedAppid));
  const showReport = Boolean(report && (!selectedAppid || report.item.appid === selectedAppid));

  async function parseItem() {
    if (!selected) { setError("请先在左侧选择一个带创意工坊的游戏"); return; }
    setError("");
    const next = await actions.resolveWorkshop(selected.appid, input);
    if (!next) return;
  }

  async function downloadItem() {
    if (!selected || !item) return;
    setError("");
    const next = await actions.downloadWorkshop(item.appid, input || String(item.published_file_id), keyOverride, installMods);
    if (next) void actions.loadWorkshopApps().then(setApps);
  }

  async function updateKeyDb() {
    await actions.refreshWorkshopKeyDb();
    const list = await actions.loadWorkshopApps();
    setApps(list);
  }

  return (
    <div className="games workshop-page">
      <div className="lib">
        <div className="lib-head">
          <h2>工坊游戏</h2>
          <span className="count">{apps.length}</span>
          <span className="right">
            <button
              className="icon-btn xxs"
              title="刷新工坊游戏列表"
              disabled={loadingApps || busy}
              onClick={() => void actions.loadWorkshopApps().then((list) => {
                setApps(list);
                setSelectedAppid((current) => (current && list.some((app) => app.appid === current) ? current : list[0]?.appid ?? null));
              })}
            >
              <RefreshCw size={11} className={loadingApps ? "spin" : ""} />
            </button>
          </span>
        </div>
        <div className="lib-body">
          {apps.map((app) => (
            <button
              key={app.appid}
              className={`g-item ${selectedAppid === app.appid ? "active" : ""}`}
              onClick={() => setSelectedAppid(app.appid)}
              title={`App ${app.appid} · 工坊 Depot ${app.workshop_depot}`}
            >
              <span className={`g-dot ${app.key_ready ? "ok" : "warn"}`} />
              <span className="g-name">{app.name}</span>
              <span className="g-id">{app.installed_items ? `${app.installed_items} 件` : app.appid}</span>
            </button>
          ))}
          {!apps.length && (
            <p className="note">
              配置库里还没有带创意工坊的游戏。先在「游戏配置」里保存该游戏的 Lua；
              如果它确实有工坊，列表会自动出现。
            </p>
          )}
        </div>
        <div className="lib-foot">
          <button className="btn btn-sm" disabled={busy} onClick={() => void updateKeyDb()}>
            <KeyRound size={11} />更新密钥库
          </button>
        </div>
      </div>

      <div className="editor">
        <div className="editor-head">
          <span className="file"><PackageSearch size={12} />创意工坊{selected ? ` · ${selected.name}` : ""}</span>
          <span className="right">
            {selected && (
              <Badge tone={selected.key_ready ? "ok" : "warn"}>
                {selected.key_ready ? `Key 就绪 · ${selected.key_source}` : "Key 缺失"}
              </Badge>
            )}
          </span>
        </div>

        <div className="editor-body">
          <section className="sec">
            <div className="sec-head">
              <span className="n">01</span><h3>工坊物品</h3>
              <span className="sub">支持创意工坊链接、steam:// 链接或纯数字 ID</span>
            </div>
            <div className="ws-input-row">
              <input
                className="input mono"
                value={input}
                onChange={(event) => {
                  setInput(event.target.value);
                  if (selectedAppid) localStorage.setItem(`gost.workshopInput.${selectedAppid}`, event.target.value);
                }}
                onKeyDown={(event) => { if (event.key === "Enter") void parseItem(); }}
                placeholder="https://steamcommunity.com/sharedfiles/filedetails/?id=..."
                spellCheck={false}
              />
              <button className="btn btn-sm" disabled={busy || !selected || !input.trim()} onClick={() => void parseItem()}>
                <Search size={11} />解析
              </button>
              {input && (
                <button className="icon-btn xxs" title="清空" onClick={() => {
                  setInput("");
                  if (selectedAppid) localStorage.removeItem(`gost.workshopInput.${selectedAppid}`);
                  actions.clearWorkshopItem();
                }}><X size={11} /></button>
              )}
            </div>
            {!selected && <p className="note">左侧还没有可用的工坊游戏。</p>}
            {error && <p className="inline-warning">{error}</p>}
            {item && (
              <div className="ws-item">
                <div className="ws-item-title">{item.title}</div>
                <div className="ws-meta">
                  <span>App <b className="mono">{item.appid}</b></span>
                  <span>Depot <b className="mono">{item.workshop_depot}</b></span>
                  <span>Manifest <b className="mono">{short(item.manifest_gid)}</b></span>
                  <span>大小 <b className="mono">{formatBytes(item.size_bytes)}</b></span>
                  <span>更新 <b className="mono">{formatTime(item.time_updated)}</b></span>
                </div>
                <div className="ws-badges">
                  {item.legacy && <Badge tone="idle">旧版 UGC · 直接下载</Badge>}
                  {item.installed && <Badge tone="ok">已安装</Badge>}
                  <Badge tone={item.key_ready ? "ok" : "warn"}>
                    {item.key_ready ? `Key 就绪 · ${item.key_source}` : "Key 缺失"}
                  </Badge>
                </div>
                {itemNotes.length > 0 && (
                  <div className="ws-notes">
                    {itemNotes.map((note) => <span key={note}><AlertTriangle size={10} />{note}</span>)}
                  </div>
                )}
              </div>
            )}
          </section>

          <section className="sec">
            <div className="sec-head">
              <span className="n">02</span><h3>Depot Key</h3>
              <span className="sub">优先使用 Lua / 本机 config.vdf；都没有时才查密钥库</span>
            </div>
            <Field
              label="手动 Key（可选）"
              hint="64 位十六进制。解锁游戏通常少了工坊 Depot 的 Key，Steam 才会提示「内容仍然处于加密状态」。"
            >
              <input
                className="input mono"
                value={keyOverride}
                onChange={(event) => setKeyOverride(event.target.value)}
                placeholder="留空则自动查找"
                spellCheck={false}
              />
            </Field>
            <div className="ws-key-line">
              <ShieldCheck size={12} />
              <span>
                {item?.key_ready
                  ? `已找到 Key（${item.key_source}），可直接下载。`
                  : "本地没有 Key 时，点「下载并安装」会自动查询设置里的密钥库；也可以先在这里填入。"}
              </span>
            </div>
          </section>

          <section className="sec">
            <div className="sec-head">
              <span className="n">03</span><h3>下载</h3>
              <span className="sub">直接写入 steamapps/workshop/content，不经过 Steam 的加密下载队列</span>
              <span className="right">
                <button className="btn btn-primary btn-sm" disabled={busy || !item} onClick={() => void downloadItem()}>
                  <Download size={11} />{item?.installed ? "重新下载" : "下载并安装"}
                </button>
              </span>
            </div>
            {item && (
              item.mods_available ? (
                <Toggle
                  checked={installMods}
                  onChange={setInstallMods}
                  label="同时安装到游戏 Mods 文件夹"
                  description={`${item.game_mods_dir ?? ""}；游戏内模组菜单从这里读取，重启游戏后生效`}
                />
              ) : (
                <p className="note">游戏目录下没有 `Mods` 文件夹：只写入 Steam 工坊目录，游戏内模组菜单可能不会显示。</p>
              )
            )}
            {state.notice.tone === "error" && <p className="inline-warning">{state.notice.message}</p>}
            {showProgress && progress ? (
              <div className="ws-progress-box">
                <div className="ws-progress-head">
                  <span>{PHASE_LABEL[progress.phase] ?? progress.phase}</span>
                  <span className="mono">{percent.toFixed(1)}%</span>
                </div>
                <div className="ws-progress"><i style={{ width: `${percent}%` }} /></div>
                <div className="ws-progress-sub">
                  <span>{formatBytes(progress.bytes_done)} / {formatBytes(progress.bytes_total)}</span>
                  <span>{progress.chunks_total ? `${progress.chunks_done}/${progress.chunks_total} 分块` : ""}</span>
                  <span className="ws-current">{progress.current_file || progress.message}</span>
                </div>
              </div>
            ) : (
              <p className="note">
                {item
                  ? item.legacy
                    ? "旧版 UGC 物品会直接下载原始文件，不需要 Depot Key。"
                    : "下载前请退出游戏；写入完成后重启 Steam 或重新启动游戏即可识别。"
                  : "先解析一个工坊物品，下载按钮才会启用。"}
              </p>
            )}
            {showReport && report && (
              <div className="ws-result">
                <div className="ws-result-head"><CheckCircle2 size={12} />App {report.item.appid} · {report.item.title} · 已安装 {report.files} 个文件 · {formatBytes(report.bytes)} · {report.elapsed_ms} ms</div>
                <div className="ws-path mono" title={report.content_dir}>{report.content_dir}</div>
                {report.mods_dir && <div className="ws-path mono" title={report.mods_dir}>游戏 Mods：{report.mods_dir}（{report.mods_files} 个文件）；重启游戏后在模组菜单启用。</div>}
                {reportNotes.map((note) => <div className="ws-notes" key={note}><span><AlertTriangle size={10} />{note}</span></div>)}
              </div>
            )}
          </section>

          <section className="sec">
            <div className="sec-head">
              <span className="n">04</span><h3>说明</h3>
            </div>
            <div className="ws-help">
              <p><FolderOpen size={11} /> 工坊内容按 Steam 的目录结构写入：<code>steamapps/workshop/content/&lt;AppId&gt;/&lt;物品ID&gt;</code>。</p>
              <p><ShieldCheck size={11} /> 新版物品走 SteamPipe：取工坊 Depot 的 manifest、按分块解密后落盘，并同步 <code>appworkshop_&lt;AppId&gt;.acf</code>。</p>
              <p><AlertTriangle size={11} /> 密钥库只用于补齐工坊 Depot Key；游戏本体仍然依赖你自己的 Lua 配置。没有 Key 的物品无法解密，工具不会猜测。</p>
            </div>
          </section>
        </div>

        <div className="editor-foot">
          <span className="where">{selected ? `Depot ${selected.workshop_depot} · ${selected.content_dir}` : "未选择游戏"}</span>
          <span className="right">
            <button className="btn btn-ghost btn-sm" disabled={!selected} onClick={() => void navigator.clipboard?.writeText(selected?.content_dir ?? "")}>
              复制目录
            </button>
          </span>
        </div>
      </div>
    </div>
  );
}

function short(value: string) {
  if (!value) return "—";
  return value.length > 16 ? `${value.slice(0, 10)}…${value.slice(-4)}` : value;
}

function formatBytes(bytes: number) {
  if (!bytes) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function formatTime(seconds: number) {
  if (!seconds) return "—";
  return new Date(seconds * 1000).toLocaleDateString("zh-CN");
}
