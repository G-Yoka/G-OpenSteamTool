import { ChevronDown, ChevronRight, Download, FileCode2, FolderOpen, Plus, Save, ScanSearch, Search, Trash2, X } from "lucide-react";
import { Fragment, useEffect, useMemo, useState } from "react";
import type { AppController } from "../../app/useAppController";
import { emptyGame } from "../../app/model";
import type { DepotDiscovery, DepotOwnership, DiscoveredDepot, GameConfig } from "../../types";
import { Badge, Field, Toggle } from "../../shared/ui";

export function GamesPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const [query, setQuery] = useState("");
  const [form, setForm] = useState<GameConfig>(state.selectedGame);
  const [advanced, setAdvanced] = useState(false);
  const [discovery, setDiscovery] = useState<DepotDiscovery | null>(null);
  const [dlcSelected, setDlcSelected] = useState<Record<number, boolean>>({});
  // 已保存配置的 Lua 只有扁平的 `addappid` 行，归属得靠本地 appinfo 现查
  const [ownership, setOwnership] = useState<Record<number, DepotOwnership>>({});
  useEffect(() => { setForm(state.selectedGame); }, [state.selectedGame]);
  // 换了 AppId 之后旧的发现结果不再适用，避免把别的游戏的 DLC 写进来
  useEffect(() => {
    if (discovery && discovery.appid !== form.appid) setDiscovery(null);
  }, [discovery, form.appid]);
  useEffect(() => {
    let cancelled = false;
    setOwnership({});
    // 只给已保存的配置标注：输入 AppId 的过程中不该每敲一位就解析一遍 appinfo
    const saved = state.games.some((game) => game.appid === form.appid);
    if (!state.steamDir || !form.appid || !saved) return;
    void actions.classifyDepots(form.appid).then((rows) => {
      if (cancelled || !rows) return;
      const next: Record<number, DepotOwnership> = {};
      for (const row of rows) {
        // 同一 Depot 若同时被标成主体与 DLC，以 DLC 为准
        if (!next[row.appid] || row.group === "dlc") next[row.appid] = row;
      }
      setOwnership(next);
    });
    return () => { cancelled = true; };
  }, [form.appid, state.steamDir, state.games]);
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

  function newGame() {
    actions.newGame();
    setForm(emptyGame);
    setDiscovery(null);
    setDlcSelected({});
  }

  /**
   * 主内容 + 勾选的 DLC 合并成同一份条目表；DLC 先给拥有标记行，再给内容 depot。
   * 发现结果只带「本地 config.vdf 里存在的 Key」，所以已有行上的 Key（导入或手工填的）
   * 必须沿用：否则一次重新获取就会把别人 Lua 里的 Key 清空。
   */
  function buildEntries(
    result: DepotDiscovery,
    selected: Record<number, boolean>,
    currentEntries: NonNullable<GameConfig["appid_entries"]> | undefined,
    currentManifests: NonNullable<GameConfig["manifest_entries"]> | undefined,
  ) {
    const existingKeys = new Map<number, string>();
    const existingFlags = new Map<number, number>();
    for (const entry of currentEntries ?? []) {
      const key = entry.depot_key?.trim();
      if (key) existingKeys.set(entry.appid, key);
      if (entry.unlock_flag != null) existingFlags.set(entry.appid, entry.unlock_flag);
    }
    const existingGids = new Map<number, string>();
    for (const entry of currentManifests ?? []) {
      if (entry.manifest_gid.trim()) existingGids.set(entry.depot_id, entry.manifest_gid.trim());
    }

    const makeEntry = (appid: number, discoveryKey?: string | null) => {
      const key = (discoveryKey ?? "").trim() || existingKeys.get(appid) || "";
      return { appid, unlock_flag: key ? existingFlags.get(appid) ?? 0 : null, depot_key: key };
    };
    const manifestEntries: { depot_id: number; manifest_gid: string }[] = [];
    const addManifest = (depotId: number, gid?: string | null) => {
      const value = (gid ?? "").trim() || existingGids.get(depotId) || "";
      if (value && !manifestEntries.some((item) => item.depot_id === depotId)) {
        manifestEntries.push({ depot_id: depotId, manifest_gid: value });
      }
    };

    const appidEntries = result.depots.map((depot) => makeEntry(depot.depot_id, depot.depot_key));
    for (const depot of result.depots) addManifest(depot.depot_id, depot.manifest_gid);
    if (!appidEntries.some((entry) => entry.appid === result.appid)) {
      appidEntries.push(makeEntry(result.appid));
    }
    for (const dlc of result.dlc) {
      if (!selected[dlc.appid]) continue;
      if (!appidEntries.some((entry) => entry.appid === dlc.appid)) {
        appidEntries.push(makeEntry(dlc.appid));
      }
      for (const depot of dlc.depots) {
        const entry = makeEntry(depot.depot_id, depot.depot_key);
        const existing = appidEntries.find((item) => item.appid === depot.depot_id);
        if (existing) Object.assign(existing, entry);
        else appidEntries.push(entry);
        addManifest(depot.depot_id, depot.manifest_gid);
      }
    }
    // 发现结果覆盖不到的旧 Manifest 也保留，避免一次获取把已有锁定清掉
    for (const [depotId, gid] of existingGids) addManifest(depotId, gid);
    return { appidEntries, manifestEntries };
  }

  function applyDiscovery(result: DepotDiscovery, selected: Record<number, boolean>) {
    setForm((prev) => {
      const { appidEntries, manifestEntries } = buildEntries(
        result,
        selected,
        prev.appid_entries,
        prev.manifest_entries,
      );
      return {
        ...prev,
        name: prev.name || result.name,
        appid_entries: appidEntries,
        manifest_entries: manifestEntries,
      };
    });
  }

  async function autoDiscoverDepots() {
    const result = await actions.discoverDepots(form.appid);
    if (!result) return;
    // 默认勾选：本地有 Key 的 DLC，或这份配置里本来就有它的行（导入来的 Key 不能丢）
    const present = new Set((form.appid_entries ?? []).map((entry) => entry.appid));
    const selection: Record<number, boolean> = {};
    for (const dlc of result.dlc) {
      selection[dlc.appid] =
        dlc.key_found || present.has(dlc.appid) || dlc.depots.some((depot) => present.has(depot.depot_id));
    }
    setDiscovery(result);
    setDlcSelected(selection);
    applyDiscovery(result, selection);
  }

  function toggleDlc(appid: number) {
    if (!discovery) return;
    const next = { ...dlcSelected, [appid]: !dlcSelected[appid] };
    setDlcSelected(next);
    applyDiscovery(discovery, next);
  }

  const appEntries = form.appid_entries ?? [];
  const manifestEntries = form.manifest_entries ?? [];
  // 显示顺序按归属整理（主体在前、DLC 在后），但数组本身不动：编辑仍按原下标写回。
  const appEntryOrder = useMemo(() => {
    const rank = (index: number) =>
      ownerGroupOf(ownership[appEntries[index].appid], appEntries[index].appid)?.isDlc ? 1 : 0;
    return appEntries.map((_, index) => index).sort((a, b) => rank(a) - rank(b) || a - b);
  }, [appEntries, ownership]);
  const manifestEntryOrder = useMemo(() => {
    const rank = (index: number) =>
      ownerGroupOf(ownership[manifestEntries[index].depot_id], manifestEntries[index].depot_id)?.isDlc ? 1 : 0;
    return manifestEntries.map((_, index) => index).sort((a, b) => rank(a) - rank(b) || a - b);
  }, [manifestEntries, ownership]);

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
          <button className="btn btn-sm" onClick={actions.importLua} disabled={disabled} title="导入 Lua 配置（可按住 Ctrl / Shift 多选）"><Download size={11} />导入</button>
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
            {discovery && (
              <div className="discovery">
                <div className="discovery-head">
                  <span className="discovery-title">本地发现</span>
                  <span className="row-sub" title="只统计本地 appinfo.vdf 里已有的 DLC；商店页有、但本地未缓存的 DLC 不会出现在这里">
                    {discovery.dlc.length
                      ? `${discovery.dlc.length} 个 DLC，其中 ${discovery.dlc.filter((dlc) => dlc.key_found).length} 个含本地 Key`
                      : "appinfo.vdf 里没有 DLC 记录；可先在 Steam 打开该游戏的 DLC 商店页，再重新获取"}
                  </span>
                </div>
                <DiscoveryRow
                  label="主内容"
                  appid={discovery.appid}
                  name={discovery.name}
                  depots={discovery.depots}
                  fixed
                />
                {discovery.dlc.map((dlc, index) => (
                  <DiscoveryRow
                    key={dlc.appid}
                    label={`DLC ${index + 1}`}
                    appid={dlc.appid}
                    name={dlc.name ?? "未命名 DLC"}
                    depots={dlc.depots}
                    checked={Boolean(dlcSelected[dlc.appid])}
                    onToggle={() => toggleDlc(dlc.appid)}
                  />
                ))}
              </div>
            )}
            <div className="entries cols-4">
              <div className="entries-head"><span>归属</span><span>App / Depot</span><span>解锁标记</span><span>64 位 Depot Key</span><span /></div>
              {appEntryOrder.map((index, position) => {
                const entry = appEntries[index];
                const group = ownerGroupOf(ownership[entry.appid], entry.appid);
                const previousIndex = position > 0 ? appEntryOrder[position - 1] : -1;
                const previousGroup = previousIndex >= 0
                  ? ownerGroupOf(ownership[appEntries[previousIndex].appid], appEntries[previousIndex].appid)
                  : null;
                const boundary = group !== null && group.key !== (previousGroup?.key ?? "");
                const groupKey = group?.key;
                const groupRows = groupKey
                  ? appEntries.filter((item) => ownerGroupOf(ownership[item.appid], item.appid)?.key === groupKey)
                  : [];
                const groupKeys = groupRows.filter((item) => item.depot_key?.trim()).length;
                return (
                  <Fragment key={index}>
                    {boundary && group && (
                      <div className={`entry-group ${group.isDlc ? "is-dlc" : "is-base"}`}>
                        {group.label}
                        <span className="entry-group-meta">{groupKeys}/{groupRows.length} 含 Key</span>
                      </div>
                    )}
                    <div className="entry-row">
                      <span className={`owner-tag ${group ? (group.isDlc ? "is-dlc" : "is-base") : "is-unknown"}`} title={group?.dlcAppid ? `DLC AppId ${group.dlcAppid}` : undefined}>
                        {group ? (group.isDlc ? "DLC" : "主体") : "—"}
                      </span>
                      <input className="input mono" inputMode="numeric" value={entry.appid || ""} onChange={(event) => updateAppEntry(form, setForm, index, { appid: Number(event.target.value) || 0 })} placeholder="AppId" />
                      <input className="input mono" inputMode="numeric" value={entry.unlock_flag ?? ""} onChange={(event) => updateAppEntry(form, setForm, index, { unlock_flag: event.target.value ? Number(event.target.value) : null })} placeholder="可选" />
                      <input className="input mono" value={entry.depot_key ?? ""} onChange={(event) => updateAppEntry(form, setForm, index, { depot_key: event.target.value })} placeholder="64 位十六进制" />
                      <button className="icon-btn xxs" onClick={() => setForm({ ...form, appid_entries: appEntries.filter((_, itemIndex) => itemIndex !== index) })} title="删除该行"><Trash2 size={11} /></button>
                    </div>
                  </Fragment>
                );
              })}
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
            <div className="entries cols-3m">
              <div className="entries-head"><span>归属</span><span>Depot ID</span><span>Manifest GID</span><span /></div>
              {manifestEntryOrder.map((index, position) => {
                const entry = manifestEntries[index];
                const group = ownerGroupOf(ownership[entry.depot_id], entry.depot_id);
                const previousIndex = position > 0 ? manifestEntryOrder[position - 1] : -1;
                const previousGroup = previousIndex >= 0
                  ? ownerGroupOf(ownership[manifestEntries[previousIndex].depot_id], manifestEntries[previousIndex].depot_id)
                  : null;
                const boundary = group !== null && group.key !== (previousGroup?.key ?? "");
                const groupKey = group?.key;
                const groupCount = groupKey
                  ? manifestEntries.filter((item) => ownerGroupOf(ownership[item.depot_id], item.depot_id)?.key === groupKey).length
                  : 0;
                return (
                  <Fragment key={index}>
                    {boundary && group && (
                      <div className={`entry-group ${group.isDlc ? "is-dlc" : "is-base"}`}>
                        {group.label}
                        <span className="entry-group-meta">{groupCount} 条</span>
                      </div>
                    )}
                    <div className="entry-row">
                      <span className={`owner-tag ${group ? (group.isDlc ? "is-dlc" : "is-base") : "is-unknown"}`} title={group?.dlcAppid ? `DLC AppId ${group.dlcAppid}` : undefined}>
                        {group ? (group.isDlc ? "DLC" : "主体") : "—"}
                      </span>
                      <input className="input mono" inputMode="numeric" value={entry.depot_id || ""} onChange={(event) => updateManifestEntry(form, setForm, index, { depot_id: Number(event.target.value) || 0 })} placeholder="Depot ID" />
                      <input className="input mono" value={entry.manifest_gid} onChange={(event) => updateManifestEntry(form, setForm, index, { manifest_gid: event.target.value })} placeholder="数字 GID" />
                      <button className="icon-btn xxs" onClick={() => setForm({ ...form, manifest_entries: manifestEntries.filter((_, itemIndex) => itemIndex !== index) })} title="删除该行"><Trash2 size={11} /></button>
                    </div>
                  </Fragment>
                );
              })}
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

function DiscoveryRow({ label, appid, name, depots, fixed = false, checked = true, onToggle }: {
  label: string;
  appid: number;
  name: string;
  depots: DiscoveredDepot[];
  fixed?: boolean;
  checked?: boolean;
  onToggle?: () => void;
}) {
  const keyCount = depots.filter((depot) => depot.depot_key).length;
  return (
    <div className={`discovery-row${checked ? "" : " is-off"}`}>
      <span className="discovery-tag">{label}</span>
      <span className="discovery-name" title={name}>{name}</span>
      <span className="mono discovery-appid">{appid}</span>
      <span className="discovery-meta">{depots.length ? `${depots.length} depot · ${keyCount} key` : "无 depot 数据"}</span>
      {fixed
        ? <Badge tone="idle">固定</Badge>
        : <button className="btn btn-sm" aria-pressed={checked} onClick={onToggle}>{checked ? "已加入" : "加入"}</button>}
    </div>
  );
}

type OwnerGroup = { key: string; label: string; isDlc: boolean; dlcAppid?: number };

/** 把后端给的「主体 / DLC」整理成可直接渲染的分组标签；找不到归属时返回 null。 */
function ownerGroupOf(owner: DepotOwnership | undefined, fallbackId: number): OwnerGroup | null {
  if (!owner) return null;
  if (owner.group !== "dlc") return { key: "base", label: "主内容", isDlc: false };
  const dlcAppid = owner.dlc_appid ?? fallbackId;
  return {
    key: `dlc-${dlcAppid}`,
    label: `DLC · ${owner.dlc_name ?? dlcAppid}`,
    isDlc: true,
    dlcAppid,
  };
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
