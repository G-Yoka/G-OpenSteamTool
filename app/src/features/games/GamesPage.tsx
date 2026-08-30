import { Download, FileCode2, FolderOpen, Plus, Save, ScanSearch, Search, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { AppController } from "../../app/useAppController";
import { emptyGame } from "../../app/model";
import type { GameConfig } from "../../types";
import { Card, EmptyState, Field, Toggle } from "../../shared/ui";

export function GamesPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const [query, setQuery] = useState("");
  const [form, setForm] = useState<GameConfig>(state.selectedGame);
  const [advanced, setAdvanced] = useState(false);
  useEffect(() => { setForm(state.selectedGame); }, [state.selectedGame]);
  const visibleGames = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return needle ? state.games.filter((game) => `${game.appid} ${game.name}`.toLowerCase().includes(needle)) : state.games;
  }, [query, state.games]);
  const disabled = !state.steamDir || state.activity !== "idle";

  function newGame() { actions.newGame(); setForm(emptyGame); }
  async function autoDiscoverDepots() {
    const result = await actions.discoverDepots(form.appid);
    if (!result) return;
    setForm({
      ...form,
      name: form.name || result.name,
      appid_entries: result.depots.map((depot) => ({
        appid: depot.depot_id,
        unlock_flag: depot.depot_key ? 0 : null,
        depot_key: depot.depot_key ?? "",
      })),
      manifest_entries: result.depots
        .filter((depot) => depot.manifest_gid)
        .map((depot) => ({ depot_id: depot.depot_id, manifest_gid: depot.manifest_gid ?? "" })),
    });
  }
  return <div className="games-layout">
    <Card className="game-library">
      <div className="library-head"><div><h2>配置库</h2><p>{state.games.length} 个托管配置</p></div><button className="square-button primary" onClick={newGame} title="新建配置"><Plus size={19} /></button></div>
      <div className="search-box"><Search size={17} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索名称或 AppId" />{query && <button onClick={() => setQuery("")}><X size={15} /></button>}</div>
      <div className="library-actions"><button onClick={actions.importLua} disabled={disabled}><Download size={16} />导入</button><button onClick={actions.openLuaDir} disabled={disabled}><FolderOpen size={16} />打开目录</button></div>
      <div className="game-items">
        {visibleGames.map((game) => <div key={game.appid} className={`game-item ${form.appid === game.appid ? "active" : ""} ${game.enabled ? "" : "disabled"}`} onClick={() => actions.setSelectedGame(game)}>
          <button className={`mini-toggle ${game.enabled ? "on" : ""}`} title={game.enabled ? "禁用" : "启用"} onClick={(event) => { event.stopPropagation(); actions.toggleGame(game); }}><span /></button>
          <div><strong>{game.name || `App ${game.appid}`}</strong><small>G-{game.appid}.lua{game.enabled ? "" : ".disabled"}</small></div>
          <span className="appid">{game.appid}</span>
        </div>)}
        {!visibleGames.length && <EmptyState title={query ? "没有匹配结果" : "还没有游戏配置"} description={query ? "换一个名称或 AppId 搜索。" : "新建配置或导入已有 Lua 文件。"} />}
      </div>
    </Card>

    <Card className="game-editor">
      <header className="editor-head"><div className="editor-title"><span><FileCode2 size={21} /></span><div><h2>{form.appid ? `编辑 G-${form.appid}.lua` : "新建游戏配置"}</h2><p>常用选项在上，高级凭据按需展开</p></div></div><Toggle checked={form.enabled} onChange={(enabled) => setForm({ ...form, enabled })} label={form.enabled ? "启用" : "禁用"} /></header>
      <div className="editor-scroll">
        <section className="form-section"><div className="form-section-title"><span>01</span><div><h3>游戏信息</h3><p>用于文件命名和配置识别</p></div></div><div className="form-grid two"><Field label="AppId"><input inputMode="numeric" value={form.appid || ""} onChange={(event) => setForm({ ...form, appid: Number(event.target.value) || 0 })} placeholder="例如 730" /></Field><Field label="显示名称"><input value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} placeholder="例如 Counter-Strike 2" /></Field></div></section>
        <section className="form-section"><div className="form-section-title"><span>02</span><div><h3>Depot 解密</h3><p>从 Steam 本地缓存自动关联 Depot ID、Key 和 Manifest</p></div><button className="text-button" disabled={disabled || !form.appid} onClick={() => void autoDiscoverDepots()}><ScanSearch size={15} />本地自动获取</button><button className="text-button" onClick={() => setForm({ ...form, appid_entries: [...(form.appid_entries ?? []), { appid: form.appid || 0, unlock_flag: null, depot_key: "" }] })}><Plus size={15} />添加一行</button></div>
          <div className="entry-table"><div className="entry-head"><span>App / Depot ID</span><span>解锁标记</span><span>64 位 Depot Key</span><span /></div>{(form.appid_entries ?? []).map((entry, index) => <div className="entry-row" key={index}><input inputMode="numeric" value={entry.appid || ""} onChange={(event) => updateAppEntry(form, setForm, index, { appid: Number(event.target.value) || 0 })} placeholder="AppId" /><input inputMode="numeric" value={entry.unlock_flag ?? ""} onChange={(event) => updateAppEntry(form, setForm, index, { unlock_flag: event.target.value ? Number(event.target.value) : null })} placeholder="可选" /><input className="mono" value={entry.depot_key ?? ""} onChange={(event) => updateAppEntry(form, setForm, index, { depot_key: event.target.value })} placeholder="可选，64 位十六进制" /><button className="icon-danger" onClick={() => setForm({ ...form, appid_entries: form.appid_entries?.filter((_, itemIndex) => itemIndex !== index) })}><Trash2 size={16} /></button></div>)}{!(form.appid_entries ?? []).length && <button className="empty-row" onClick={() => setForm({ ...form, appid_entries: [{ appid: form.appid || 0, unlock_flag: null, depot_key: "" }] })}><Plus size={16} />添加 Depot 条目</button>}</div>
        </section>
        <section className="form-section"><div className="form-section-title"><span>03</span><div><h3>Manifest</h3><p>将 Depot ID 映射到 Manifest GID</p></div><button className="text-button" onClick={() => setForm({ ...form, manifest_entries: [...(form.manifest_entries ?? []), { depot_id: form.appid || 0, manifest_gid: "" }] })}><Plus size={15} />添加一行</button></div>
          <div className="entry-table manifest"><div className="entry-head"><span>Depot ID</span><span>Manifest GID</span><span /></div>{(form.manifest_entries ?? []).map((entry, index) => <div className="entry-row" key={index}><input inputMode="numeric" value={entry.depot_id || ""} onChange={(event) => updateManifestEntry(form, setForm, index, { depot_id: Number(event.target.value) || 0 })} placeholder="Depot ID" /><input className="mono" value={entry.manifest_gid} onChange={(event) => updateManifestEntry(form, setForm, index, { manifest_gid: event.target.value })} placeholder="数字 GID" /><button className="icon-danger" onClick={() => setForm({ ...form, manifest_entries: form.manifest_entries?.filter((_, itemIndex) => itemIndex !== index) })}><Trash2 size={16} /></button></div>)}{!(form.manifest_entries ?? []).length && <button className="empty-row" onClick={() => setForm({ ...form, manifest_entries: [{ depot_id: form.appid || 0, manifest_gid: "" }] })}><Plus size={16} />添加 Manifest 条目</button>}</div>
        </section>
        <section className="advanced-section"><button className="advanced-trigger" onClick={() => setAdvanced(!advanced)}><span><strong>高级凭据</strong><small>Access Token、Ticket 与统计 Steam ID</small></span><b>{advanced ? "收起" : "展开"}</b></button>{advanced && <div className="form-grid two advanced-fields"><Field label="Access Token"><input className="mono" value={form.access_token ?? ""} onChange={(event) => setForm({ ...form, access_token: event.target.value })} /></Field><Field label="Stat Steam ID"><input className="mono" value={form.stat_steam_id ?? ""} onChange={(event) => setForm({ ...form, stat_steam_id: event.target.value })} /></Field><Field label="AppTicket Hex"><textarea className="mono" rows={3} value={form.app_ticket_hex ?? ""} onChange={(event) => setForm({ ...form, app_ticket_hex: event.target.value })} /></Field><Field label="ETicket Hex"><textarea className="mono" rows={3} value={form.e_ticket_hex ?? ""} onChange={(event) => setForm({ ...form, e_ticket_hex: event.target.value })} /></Field></div>}</section>
      </div>
      <footer className="editor-footer"><span>{form.appid ? `将写入 G-${form.appid}.lua${form.enabled ? "" : ".disabled"}` : "请输入有效 AppId"}</span><div>{form.appid && state.games.some((game) => game.appid === form.appid) && <button className="button danger-soft" onClick={() => actions.deleteGame(form.appid)}><Trash2 size={17} />删除</button>}<button className="button" onClick={newGame}>清空</button><button className="button primary" disabled={disabled || !form.appid} onClick={() => actions.saveGame(form)}><Save size={17} />保存配置</button></div></footer>
    </Card>
  </div>;
}

function updateAppEntry(form: GameConfig, setForm: (game: GameConfig) => void, index: number, patch: Partial<NonNullable<GameConfig["appid_entries"]>[number]>) { setForm({ ...form, appid_entries: (form.appid_entries ?? []).map((entry, itemIndex) => itemIndex === index ? { ...entry, ...patch } : entry) }); }
function updateManifestEntry(form: GameConfig, setForm: (game: GameConfig) => void, index: number, patch: Partial<NonNullable<GameConfig["manifest_entries"]>[number]>) { setForm({ ...form, manifest_entries: (form.manifest_entries ?? []).map((entry, itemIndex) => itemIndex === index ? { ...entry, ...patch } : entry) }); }
