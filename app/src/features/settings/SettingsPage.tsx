import { FileSliders, Save, TimerReset } from "lucide-react";
import { useEffect, useState } from "react";
import type { AppController } from "../../app/useAppController";
import type { ManagerSettings } from "../../types";
import { Card, CardHeader, Field } from "../../shared/ui";

export function SettingsPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const [draft, setDraft] = useState<ManagerSettings>(state.settings);
  const [luaPaths, setLuaPaths] = useState(state.settings.lua_paths.join("\n"));
  useEffect(() => { setDraft(state.settings); setLuaPaths(state.settings.lua_paths.join("\n")); }, [state.settings]);
  const save = () => actions.saveSettings({ ...draft, lua_paths: luaPaths.split(/\r?\n/).map((item) => item.trim()).filter(Boolean) });
  return <div className="settings-layout">
    <div className="settings-main">
      <Card><CardHeader icon={FileSliders} title="基础设置" description="写入 Steam 根目录下的 opensteamtool.toml" /><div className="form-grid two settings-form"><Field label="日志等级"><select value={draft.log_level} onChange={(event) => setDraft({ ...draft, log_level: event.target.value })}><option value="trace">Trace</option><option value="debug">Debug</option><option value="info">Info</option><option value="warn">Warn</option><option value="error">Error</option></select></Field><Field label="Manifest 来源"><select value={draft.manifest_url} onChange={(event) => setDraft({ ...draft, manifest_url: event.target.value })}><option value="wudrm">wudrm</option><option value="steamrun">steamrun</option></select></Field><Field label="Pattern Mirror"><input value={draft.pattern_mirror} onChange={(event) => setDraft({ ...draft, pattern_mirror: event.target.value })} placeholder="留空使用默认源" /></Field><Field label="额外 Lua 路径" hint="每行填写一个绝对路径"><textarea rows={4} value={luaPaths} onChange={(event) => setLuaPaths(event.target.value)} placeholder={"D:\\SteamLua\nE:\\SharedLua"} /></Field></div></Card>
      <Card><CardHeader icon={TimerReset} title="网络超时" description="单位为毫秒；网络不稳定时可以适当调高" /><div className="form-grid four settings-form">{timeoutFields.map(([key, label]) => <Field label={label} key={key}><input type="number" min={100} step={100} value={draft[key]} onChange={(event) => setDraft({ ...draft, [key]: Math.max(0, Number(event.target.value)) })} /></Field>)}</div></Card>
    </div>
    <aside className="settings-side"><Card className="save-card"><p>修改 TOML 设置后需要重启 Steam 才能完全生效。</p><button className="button primary full" disabled={!state.steamDir || state.activity !== "idle"} onClick={save}><Save size={17} />保存所有设置</button></Card></aside>
  </div>;
}

const timeoutFields: Array<[keyof Pick<ManagerSettings, "timeout_resolve_ms" | "timeout_connect_ms" | "timeout_send_ms" | "timeout_recv_ms">, string]> = [
  ["timeout_resolve_ms", "DNS 解析"], ["timeout_connect_ms", "建立连接"], ["timeout_send_ms", "发送请求"], ["timeout_recv_ms", "接收响应"],
];
