import { FileText, RefreshCw, Search, WrapText, X } from "lucide-react";
import { useMemo, useState } from "react";
import type { AppController } from "../../app/useAppController";
import { formatBytes, parseLine } from "../../shared/logLine";
import { Badge, Empty, Verdict } from "../../shared/ui";

const LEVELS: Array<[string, string]> = [
  ["all", "全部"],
  ["error", "error"],
  ["warn", "warn"],
  ["info", "info"],
  ["debug", "debug"],
  ["trace", "trace"],
];

export function LogsPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const [query, setQuery] = useState("");
  const [level, setLevel] = useState("all");
  const [wrap, setWrap] = useState(false);
  const active = state.logs.find((log) => log.name === state.selectedLog) ?? state.logs[0];
  const all = useMemo(() => (active?.content ?? "").split(/\r?\n/).filter(Boolean).map(parseLine), [active]);
  const rows = useMemo(
    () => all.filter((row) => (level === "all" || row.level === level) && (!query || row.raw.toLowerCase().includes(query.toLowerCase()))),
    [all, level, query],
  );

  return (
    <div className="logs">
      <Verdict
        tone={all.some((row) => row.level === "error") ? "bad" : all.some((row) => row.level === "warn") ? "warn" : "ok"}
        text={all.length
          ? <>{`${active?.name ?? "日志"}：`}{all.filter((row) => row.level === "warn").length} 个 warn、{all.filter((row) => row.level === "error").length} 个 error{topSignal(all)}</>
          : "当前没有可读日志"}
        action={
          <button className="btn btn-sm" data-kbd="Ctrl R" onClick={actions.refreshLogs}>
            <RefreshCw size={11} />刷新日志
          </button>
        }
      />
      <div className="log-files">
        <div className="lib-head">
          <h2>日志文件</h2>
          <span className="count">{state.logs.length}</span>
          <span className="right"><button className="icon-btn xxs" onClick={actions.refreshLogs} title="重新读取日志"><RefreshCw size={11} /></button></span>
        </div>
        <div className="list">
          {state.logs.map((log) => (
            <button
              key={log.name}
              className={`g-item ${active?.name === log.name ? "active" : ""}`}
              onClick={() => actions.setSelectedLog(log.name)}
              title={log.name}
            >
              <FileText size={11} />
              <span className="g-name">{log.name}</span>
              <span className="g-id">{log.line_count}</span>
            </button>
          ))}
          {!state.logs.length && <Empty title="暂无日志" description="启动 Steam 并运行 OpenSteamTool 后，日志会出现在这里。" />}
        </div>
      </div>

      <div className="log-viewer">
        <div className="log-toolbar">
          <span className="file mono">{active?.name ?? "未选择文件"}</span>
          <span className="meta">{rows.length}/{all.length} 行</span>
          <div className="right">
            <div className="search inline">
              <Search size={11} />
              <input data-search value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索日志" spellCheck={false} />
              {query && <button className="clear" onClick={() => setQuery("")} aria-label="清空"><X size={11} /></button>}
            </div>
            <div className="seg">
              {LEVELS.map(([value, label]) => (
                <button key={value} className={level === value ? "on" : ""} onClick={() => setLevel(value)}>{label}</button>
              ))}
            </div>
            <button className={`icon-btn xs ${wrap ? "on" : ""}`} onClick={() => setWrap(!wrap)} title="自动换行">
              <WrapText size={11} />
            </button>
          </div>
        </div>

        {!active ? (
          <Empty title="暂无运行日志" description="日志文件出现在选中的 Steam 目录后即可查看与过滤。" />
        ) : (
          <div className="log-body">
            {rows.map((row, index) => (
              <div className={`log-line ${row.level} ${wrap ? "" : "nw"}`} key={`${index}-${row.raw}`}>
                <span className="ln">{index + 1}</span>
                <span className="lt">{row.time || "--:--:--"}</span>
                <span className="lv">{row.level || "log"}</span>
                <span className="lm">{row.message}</span>
              </div>
            ))}
            {!rows.length && <div className="palette-empty">没有匹配的日志行</div>}
          </div>
        )}
        <div className="log-foot">
          <Badge tone={rows.some((row) => row.level === "error") ? "bad" : "idle"}>error {all.filter((row) => row.level === "error").length}</Badge>
          <Badge tone={rows.some((row) => row.level === "warn") ? "warn" : "idle"}>warn {all.filter((row) => row.level === "warn").length}</Badge>
          <span className="meta">{active ? `${formatBytes(active.size_bytes)}` : ""}</span>
        </div>
      </div>
    </div>
  );
}

/** 从日志内容里指出「问题集中在哪」——只做词频归并，不猜结论。 */
function topSignal(rows: Array<{ message: string }>) {
  const buckets: Array<[string, number]> = [
    ["RecvPkt 抑制", rows.filter((row) => /RecvPkt/.test(row.message)).length],
    ["pattern 缺条目", rows.filter((row) => /no entry for/.test(row.message)).length],
    ["ipc 通道", rows.filter((row) => /\bipc\b/i.test(row.message)).length],
  ];
  const [label, count] = buckets.sort((a, b) => b[1] - a[1])[0];
  return count > 0 ? `，集中在 ${label}（${count} 行）` : "";
}
