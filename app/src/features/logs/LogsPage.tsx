import { FileText, RefreshCw, Search, WrapText } from "lucide-react";
import { useMemo, useState } from "react";
import type { AppController } from "../../app/useAppController";
import { Card, EmptyState } from "../../shared/ui";

export function LogsPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const [query, setQuery] = useState("");
  const [level, setLevel] = useState("all");
  const [wrap, setWrap] = useState(true);
  const active = state.logs.find((log) => log.name === state.selectedLog) ?? state.logs[0];
  const rows = useMemo(() => (active?.content ?? "").split(/\r?\n/).map(parseLine).filter((row) => (level === "all" || row.level === level) && (!query || row.raw.toLowerCase().includes(query.toLowerCase()))), [active, level, query]);
  return <div className="logs-layout">
    <Card className="log-files"><div className="library-head"><div><h2>日志文件</h2><p>{state.logs.length} 个可用文件</p></div><button className="square-button" onClick={actions.refreshLogs}><RefreshCw size={18} /></button></div><div className="log-file-list">{state.logs.map((log) => <button key={log.name} className={active?.name === log.name ? "active" : ""} onClick={() => actions.setSelectedLog(log.name)}><FileText size={18} /><span><strong>{log.name}</strong><small>{formatBytes(log.size_bytes)} · {log.line_count} 行</small></span></button>)}</div></Card>
    <Card className="log-viewer"><header className="log-toolbar"><div><h2>{active?.name ?? "日志查看器"}</h2><p>{active ? `${rows.length} 行匹配结果` : "选择日志文件查看内容"}</p></div><div className="log-filters"><div className="search-box compact"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索日志" /></div><select value={level} onChange={(event) => setLevel(event.target.value)}><option value="all">全部等级</option><option value="error">Error</option><option value="warn">Warn</option><option value="info">Info</option><option value="debug">Debug</option><option value="trace">Trace</option></select><button className={`square-button ${wrap ? "active" : ""}`} onClick={() => setWrap(!wrap)} title="自动换行"><WrapText size={18} /></button></div></header>
      {!active ? <EmptyState title="暂无运行日志" description="启动 Steam 并运行 OpenSteamTool 后，日志会显示在这里。" /> : <div className={`log-lines ${wrap ? "wrap" : ""}`}>{rows.map((row, index) => <div className={`log-line ${row.level}`} key={`${index}-${row.raw}`}><span className="line-number">{index + 1}</span><time>{row.time || "--:--:--"}</time><b>{row.level || "log"}</b><code>{row.message}</code></div>)}{!rows.length && <EmptyState title="没有匹配日志" description="调整等级或搜索关键词后重试。" />}</div>}
    </Card>
  </div>;
}

function parseLine(raw: string) { const match = raw.match(/^\[?([^\]]*\d{2}:\d{2}:\d{2}[^\]]*)\]?\s*\[?(trace|debug|info|warn|warning|error)\]?\s*[:\-]?\s*(.*)$/i); return { raw, time: match?.[1]?.match(/\d{2}:\d{2}:\d{2}/)?.[0] ?? "", level: (match?.[2] ?? "").toLowerCase().replace("warning", "warn"), message: match?.[3] ?? raw }; }
function formatBytes(bytes: number) { if (bytes < 1024) return `${bytes} B`; if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`; return `${(bytes / 1024 / 1024).toFixed(1)} MB`; }
