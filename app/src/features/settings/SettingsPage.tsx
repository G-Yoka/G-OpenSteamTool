import { FileSliders, FolderOpen, Link2, Save, TimerReset } from "lucide-react";
import { useEffect, useState } from "react";
import { pickIdlFile } from "../../api";
import type { AppController } from "../../app/useAppController";
import type { ManagerSettings } from "../../types";
import { Badge, Card, CardHead, Field, Toggle } from "../../shared/ui";

export function SettingsPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const [draft, setDraft] = useState<ManagerSettings>(state.settings);
  const [luaPaths, setLuaPaths] = useState(state.settings.lua_paths.join("\n"));
  const [idlPath, setIdlPath] = useState(() => localStorage.getItem("gost.idl") ?? "");
  useEffect(() => {
    setDraft(state.settings);
    setLuaPaths(state.settings.lua_paths.join("\n"));
  }, [state.settings]);

  const template = draft.remote_url_template.trim();
  const missingPlaceholder = template !== "" && !["{channel}", "{component}", "{sha256}"].every((token) => template.includes(token));
  const dirty = JSON.stringify({ ...draft, lua_paths: luaPaths.split(/\r?\n/).map((item) => item.trim()).filter(Boolean) }) !== JSON.stringify(state.settings);
  const pickIdl = async () => {
    const picked = await pickIdlFile();
    if (!picked) return;
    localStorage.setItem("gost.idl", picked);
    setIdlPath(picked);
  };
  const save = () => actions.saveSettings({ ...draft, lua_paths: luaPaths.split(/\r?\n/).map((item) => item.trim()).filter(Boolean) });

  return (
    <div className="stack">
      <Card>
        <CardHead
          title="基础设置"
          sub="写入 Steam 根目录下的 opensteamtool.toml"
          accent
          right={<Badge tone={dirty ? "warn" : "idle"}>{dirty ? "未保存" : "已同步"}</Badge>}
        />
        <div className="field-row">
          <span className="f-label"><strong>日志等级</strong><small>DLL 写入 opensteamtool.log 的详细程度</small></span>
          <select className="select" value={draft.log_level} onChange={(event) => setDraft({ ...draft, log_level: event.target.value })}>
            <option value="trace">Trace</option>
            <option value="debug">Debug</option>
            <option value="info">Info</option>
            <option value="warn">Warn</option>
            <option value="error">Error</option>
          </select>
        </div>
        <div className="field-row">
          <span className="f-label"><strong>Manifest 来源</strong><small>自动发现 Manifest GID 时使用的数据源</small></span>
          <select className="select" value={draft.manifest_url} onChange={(event) => setDraft({ ...draft, manifest_url: event.target.value })}>
            <option value="wudrm">wudrm</option>
            <option value="steamrun">steamrun</option>
          </select>
        </div>
        <div className="field-row tall">
          <span className="f-label"><strong>额外 Lua 路径</strong><small>每行一个绝对路径</small></span>
          <textarea className="textarea mono" rows={3} value={luaPaths} onChange={(event) => setLuaPaths(event.target.value)} placeholder={"D:\\SteamLua\nE:\\SharedLua"} spellCheck={false} />
        </div>
      </Card>

      <Card>
        <CardHead title="数据源 [remote]" sub="DLL 真正读取的键；三个通道共用同一个模板" accent={template !== ""} />
        <Field label="url_template" hint="必须包含 {channel}、{component}、{sha256}；留空则使用内置的 GitHub → jsDelivr 回退链">
          <input
            className="input mono"
            value={draft.remote_url_template}
            onChange={(event) => setDraft({ ...draft, remote_url_template: event.target.value })}
            placeholder="https://raw.githubusercontent.com/<owner>/steam-monitor/{channel}/{component}/{sha256}.toml"
            spellCheck={false}
          />
        </Field>
        {missingPlaceholder && (
          <p className="inline-warning">缺少占位符：模板必须同时包含 {"{channel}"}、{"{component}"}、{"{sha256}"}，否则 DLL 会直接禁用远程取数。</p>
        )}
        {draft.pattern_mirror && !template && (
          <p className="note">检测到旧键 <code>[pattern] mirror</code>（DLL 不识别），保存后会迁移成上面的模板。</p>
        )}
        <div className="divider" />
        <Toggle
          checked={draft.suppress_missing_popup}
          onChange={(next) => setDraft({ ...draft, suppress_missing_popup: next })}
          label="缺条目时静默"
          description="[ui] suppress_missing_popup：缺钩子只写日志、不弹窗（需要自建 DLL 才生效）"
        />
      </Card>

      <Card>
        <CardHead
          title="IPC 定义文件"
          sub="默认使用内置的上游定义；需要时可指定更新的文件"
          accent
        />
        <Field
          label="IPCMessages.steamd"
          hint="内置版本随应用分发（OpenSteamTool 上游 GPL-3.0 源码）；覆盖路径只记录在本机，不写入 opensteamtool.toml"
        >
          <div className="btn-row">
            <input className="input mono" value={idlPath} readOnly placeholder="内置（随应用分发）" />
            <button className="btn btn-sm" onClick={() => void pickIdl()}>
              <FolderOpen size={11} />选择…
            </button>
            {idlPath && (
              <button
                className="btn btn-ghost btn-sm"
                onClick={() => { localStorage.removeItem("gost.idl"); setIdlPath(""); }}
              >
                恢复内置
              </button>
            )}
          </div>
        </Field>
      </Card>

      <Card>
        <CardHead title="网络超时" sub="单位毫秒；网络不稳定时可适当调高" />
        <div className="grid-4">
          {timeoutFields.map(([key, label]) => (
            <Field label={label} key={key}>
              <input className="input mono" type="number" min={100} step={100} value={draft[key]} onChange={(event) => setDraft({ ...draft, [key]: Math.max(0, Number(event.target.value)) })} />
            </Field>
          ))}
        </div>
        <div className="btn-row spaced">
          <button className="btn btn-primary" disabled={!state.steamDir || state.activity !== "idle" || missingPlaceholder} onClick={save}>
            <Save size={12} />保存所有设置
          </button>
          <span className="note inline">改完需要重启 Steam 才完全生效。</span>
        </div>
      </Card>

      <Card>
        <CardHead title="TOML 快照" sub="磁盘上当前生效的值（来源：opensteamtool.toml）" />
        <table className="table keys">
          <thead><tr><th>键</th><th>值</th></tr></thead>
          <tbody>
            <tr><td><code>log_level</code></td><td><code>{state.build?.toml.log_level ?? "—"}</code></td></tr>
            <tr><td><code>manifest_url</code></td><td><code>{state.build?.toml.manifest_url ?? "—"}</code></td></tr>
            <tr><td><code>remote.url_template</code></td><td><code>{state.build?.toml.remote_url_template || "（空）"}</code></td></tr>
            <tr><td><code>ui.suppress_missing_popup</code></td><td><code>{String(state.build?.toml.suppress_missing_popup ?? false)}</code></td></tr>
          </tbody>
        </table>
      </Card>
    </div>
  );
}

const timeoutFields: Array<[keyof Pick<ManagerSettings, "timeout_resolve_ms" | "timeout_connect_ms" | "timeout_send_ms" | "timeout_recv_ms">, string]> = [
  ["timeout_resolve_ms", "DNS 解析"],
  ["timeout_connect_ms", "建立连接"],
  ["timeout_send_ms", "发送请求"],
  ["timeout_recv_ms", "接收响应"],
];
