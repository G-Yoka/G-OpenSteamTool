import { ChevronDown, ChevronRight, Download, FileCode2, FolderOpen, Plus, Save, ScanSearch, Search, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { AppController } from "../../app/useAppController";
import { emptyGame } from "../../app/model";
import type { GameConfig } from "../../types";
import { Field, Toggle } from "../../shared/ui";

export function GamesPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const [query, setQuery] = useState("");
  const [form, setForm] = useState<GameConfig>(state.selectedGame);
  const [advanced, setAdvanced] = useState(false);
  useEffect(() => { setForm(state.selectedGame); }, [state.selectedGame]);
  // Keep the editor in sync with the on-disk state: toggling a config renames
  // the file, and a stale `form.enabled` would write the opposite copy back.
  useEffect(() => {
    if (!form.appid || state.activity !== "idle") return;
    const current = state.games.find((game) => game.appid === form.appid);
    if (current && current.enabled !== form.enabled) {
      setForm((prev) => ({ ...prev, enabled: current.enabled }));
    }
  }, [state.games, form.appid, form.enabled, state.activity]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return needle ? state.games.filter((game) => `${game.appid} ${game.name}`.toLowerCase().includes(needle)) : state.games;
  }, [query, state.games]);
  const disabled = !state.steamDir || state.activity !== "idle";
  const onDisk = state.games.find((game) => game.appid === form.appid);

  function newGame() { actions.newGame(); setForm(emptyGame); }

  async function autoDiscoverDepots() {
    const result = await actions.discoverDepots(form.appid);
    if (!result) return;
    const depots = result.depots.map((depot) => ({
      appid: depot.depot_id,
      unlock_flag: depot.depot_key ? 0 : null,
      depot_key: depot.depot_key ?? "",
    }));
    const appEntries = depots.some((entry) => entry.appid === result.appid)
      ? depots
      : [...depots, { appid: result.appid, unlock_flag: null, depot_key: "" }];
    setForm({
      ...form,
      name: form.name || result.name,
      appid_entries: appEntries,
      manifest_entries: result.depots
        .filter((depot) => depot.manifest_gid)
        .map((depot) => ({ depot_id: depot.depot_id, manifest_gid: depot.manifest_gid ?? "" })),
    });
  }

  const appEntries = form.appid_entries ?? [];
  const manifestEntries = form.manifest_entries ?? [];

  return (
    <div className="games">
      <div className="lib">
        <div className="lib-head">
          <h2>配置库</h2>
          <span className="count">{state.games.length}</span>
          <span className="right"><button className="icon-btn xxs" onClick={newGame} title="新建配置"><Plus size={11} /></button></span>
        </div>
        <div className="search">
          <Search size={11} />
          <input data-search value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索名称或 AppId" spellCheck={false} />
          {query && <button className="clear" onClick={() => setQuery("")} aria-label="清空"><X size={11} /></button>}
        </div>
        <div className="lib-body">
          {visible.map((game) => (
            <button
              key={game.appid}
              className={`g-item ${form.appid === game.appid ? "active" : ""} ${game.enabled ? "" : "off"}`}
              onClick={() => actions.setSelectedGame(game)}
              title={`G-${game.appid}.lua`}
            >
              <span className="g-dot" />
              <span className="g-name">{game.name || `App ${game.appid}`}</span>
              <span className="g-id">{game.appid}</span>
            </button>
          ))}
          {!visible.length && <p className="note">{query ? "没有匹配的配置。" : "还没有游戏配置：新建或导入已有 Lua。"}</p>}
        </div>
        <div className="lib-foot">
          <button className="btn btn-sm" onClick={actions.importLua} disabled={disabled}><Download size={11} />导入</button>
          <button className="btn btn-sm" onClick={actions.openLuaDir} disabled={disabled}><FolderOpen size={11} />目录</button>
        </div>
      </div>

      <div className="editor">
        <div className="editor-head">
          <span className="file"><FileCode2 size={12} />{form.appid ? `G-${form.appid}.lua` : "新建配置"}</span>
          <span className="right">
            <Toggle
              checked={form.enabled}
              onChange={(enabled) => {
                setForm({ ...form, enabled });
                // 已存在的配置：启停就是改文件名，交给后端；新建配置只是草稿状态
                if (onDisk) void actions.setGameEnabled(form.appid, enabled);
              }}
              label={form.enabled ? "启用" : "禁用"}
            />
          </span>
        </div>

        <div className="editor-body">
          <section className="sec">
            <div className="sec-head"><span className="n">01</span><h3>游戏信息</h3><span className="sub">用于文件命名与配置识别</span></div>
            <div className="form-two">
              <Field label="AppId">
                <input className="input mono" inputMode="numeric" value={form.appid || ""} onChange={(event) => setForm({ ...form, appid: Number(event.target.value) || 0 })} placeholder="例如 730" />
              </Field>
              <Field label="显示名称">
                <input className="input" value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} placeholder="例如 Counter-Strike 2" />
              </Field>
            </div>
          </section>

          <section className="sec">
            <div className="sec-head">
              <span className="n">02</span><h3>Depot 解密</h3><span className="sub">自动关联 Depot ID、Key 与当前 App ID</span>
              <span className="right">
                <button className="btn btn-sm" disabled={disabled || !form.appid} onClick={() => void autoDiscoverDepots()}>
                  <ScanSearch size={11} />本地自动获取
                </button>
                <button className="btn btn-ghost btn-sm" onClick={() => setForm({ ...form, appid_entries: [...appEntries, { appid: form.appid || 0, unlock_flag: null, depot_key: "" }] })}>
                  <Plus size={11} />加一行
                </button>
              </span>
            </div>
            <div className="entries cols-3">
              <div className="entries-head"><span>App / Depot</span><span>解锁标记</span><span>64 位 Depot Key</span><span /></div>
              {appEntries.map((entry, index) => (
                <div className="entry-row" key={index}>
                  <input className="input mono" inputMode="numeric" value={entry.appid || ""} onChange={(event) => updateAppEntry(form, setForm, index, { appid: Number(event.target.value) || 0 })} placeholder="AppId" />
                  <input className="input mono" inputMode="numeric" value={entry.unlock_flag ?? ""} onChange={(event) => updateAppEntry(form, setForm, index, { unlock_flag: event.target.value ? Number(event.target.value) : null })} placeholder="可选" />
                  <input className="input mono" value={entry.depot_key ?? ""} onChange={(event) => updateAppEntry(form, setForm, index, { depot_key: event.target.value })} placeholder="64 位十六进制" />
                  <button className="icon-btn xxs" onClick={() => setForm({ ...form, appid_entries: appEntries.filter((_, itemIndex) => itemIndex !== index) })} title="删除该行"><Trash2 size={11} /></button>
                </div>
              ))}
              {!appEntries.length && (
                <button className="entry-empty" onClick={() => setForm({ ...form, appid_entries: [{ appid: form.appid || 0, unlock_flag: null, depot_key: "" }] })}>
                  <Plus size={11} />添加 Depot 条目
                </button>
              )}
            </div>
          </section>

          <section className="sec">
            <div className="sec-head">
              <span className="n">03</span><h3>Manifest</h3><span className="sub">Depot ID → Manifest GID</span>
              <span className="right">
                <button className="btn btn-ghost btn-sm" onClick={() => setForm({ ...form, manifest_entries: [...manifestEntries, { depot_id: form.appid || 0, manifest_gid: "" }] })}>
                  <Plus size={11} />加一行
                </button>
              </span>
            </div>
            <div className="entries cols-2">
              <div className="entries-head"><span>Depot ID</span><span>Manifest GID</span><span /></div>
              {manifestEntries.map((entry, index) => (
                <div className="entry-row" key={index}>
                  <input className="input mono" inputMode="numeric" value={entry.depot_id || ""} onChange={(event) => updateManifestEntry(form, setForm, index, { depot_id: Number(event.target.value) || 0 })} placeholder="Depot ID" />
                  <input className="input mono" value={entry.manifest_gid} onChange={(event) => updateManifestEntry(form, setForm, index, { manifest_gid: event.target.value })} placeholder="数字 GID" />
                  <button className="icon-btn xxs" onClick={() => setForm({ ...form, manifest_entries: manifestEntries.filter((_, itemIndex) => itemIndex !== index) })} title="删除该行"><Trash2 size={11} /></button>
                </div>
              ))}
              {!manifestEntries.length && (
                <button className="entry-empty" onClick={() => setForm({ ...form, manifest_entries: [{ depot_id: form.appid || 0, manifest_gid: "" }] })}>
                  <Plus size={11} />添加 Manifest 条目
                </button>
              )}
            </div>
          </section>

          <section className="sec">
            <div className="adv">
              <button className="adv-head" onClick={() => setAdvanced(!advanced)} aria-expanded={advanced}>
                {advanced ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                <span className="t">高级凭据</span>
                <span className="s">Access Token、Ticket 与统计 Steam ID</span>
              </button>
              {advanced && (
                <div className="adv-body">
                  <Field label="Access Token"><input className="input mono" value={form.access_token ?? ""} onChange={(event) => setForm({ ...form, access_token: event.target.value })} /></Field>
                  <Field label="Stat Steam ID"><input className="input mono" value={form.stat_steam_id ?? ""} onChange={(event) => setForm({ ...form, stat_steam_id: event.target.value })} /></Field>
                  <Field label="AppTicket Hex"><textarea className="textarea mono" rows={3} value={form.app_ticket_hex ?? ""} onChange={(event) => setForm({ ...form, app_ticket_hex: event.target.value })} /></Field>
                  <Field label="ETicket Hex"><textarea className="textarea mono" rows={3} value={form.e_ticket_hex ?? ""} onChange={(event) => setForm({ ...form, e_ticket_hex: event.target.value })} /></Field>
                </div>
              )}
            </div>
          </section>

          <section className="sec">
            <div className="sec-head">
              <span className="n">04</span><h3>生成结果</h3>
              <span className="sub">保存后写入 {form.appid ? `G-${form.appid}.lua` : "G-<AppId>.lua"} 的内容（内部元数据行已省略）</span>
            </div>
            <pre className="code">{previewLua(form)}</pre>
          </section>
        </div>

        <div className="editor-foot">
          <span className="where">{form.appid ? `→ G-${form.appid}.lua${form.enabled ? "" : ".disabled"}` : "请输入有效 AppId"}</span>
          <span className="right">
            {form.appid > 0 && state.games.some((game) => game.appid === form.appid) && (
              <button className="btn btn-danger btn-sm" onClick={() => actions.deleteGame(form.appid)}><Trash2 size={11} />删除</button>
            )}
            <button className="btn btn-ghost btn-sm" onClick={newGame}>清空</button>
            <button className="btn btn-primary btn-sm" disabled={disabled || !form.appid} onClick={() => actions.saveGame(form)}>
              <Save size={11} />保存
            </button>
          </span>
        </div>
      </div>
    </div>
  );
}

function updateAppEntry(form: GameConfig, setForm: (game: GameConfig) => void, index: number, patch: Partial<NonNullable<GameConfig["appid_entries"]>[number]>) {
  setForm({ ...form, appid_entries: (form.appid_entries ?? []).map((entry, itemIndex) => itemIndex === index ? { ...entry, ...patch } : entry) });
}

function updateManifestEntry(form: GameConfig, setForm: (game: GameConfig) => void, index: number, patch: Partial<NonNullable<GameConfig["manifest_entries"]>[number]>) {
  setForm({ ...form, manifest_entries: (form.manifest_entries ?? []).map((entry, itemIndex) => itemIndex === index ? { ...entry, ...patch } : entry) });
}

/**
 * 与 Rust 侧 `render_game_lua` 保持同一套规则，方便在保存前看清将要写入的语句。
 * 元数据行（内部 JSON）刻意不预览：它只服务于再次导入。
 */
function previewLua(game: GameConfig): string {
  const lines = [`-- G-OpenSteamTool: ${game.appid || 0} ${game.name.trim()}`];
  for (const entry of game.appid_entries ?? []) {
    const flag = entry.unlock_flag ?? null;
    const key = entry.depot_key?.trim() || "";
    if (flag != null && key) lines.push(`addappid(${entry.appid}, ${flag}, "${key}")`);
    else if (flag != null) lines.push(`addappid(${entry.appid}, ${flag})`);
    else if (key) lines.push(`addappid(${entry.appid}, 0, "${key}")`);
    else lines.push(`addappid(${entry.appid || 0})`);
  }
  const token = game.access_token?.trim();
  if (token) lines.push(`addtoken(${game.appid}, "${token}")`);
  const manifests = game.manifest_entries ?? [];
  if (manifests.length) {
    for (const entry of manifests) lines.push(`setManifestid(${entry.depot_id}, "${entry.manifest_gid}")`);
  } else if (game.manifest_gid?.trim()) {
    lines.push(`setManifestid(${game.appid}, "${game.manifest_gid.trim()}")`);
  }
  if (game.app_ticket_hex?.trim()) lines.push(`setAppTicket(${game.appid}, "<${game.app_ticket_hex.trim().length} hex 字符>")`);
  if (game.e_ticket_hex?.trim()) lines.push(`setETicket(${game.appid}, "<${game.e_ticket_hex.trim().length} hex 字符>")`);
  if (game.stat_steam_id?.trim()) lines.push(`setStat(${game.appid}, "${game.stat_steam_id.trim()}")`);
  return lines.join("\n");
}
