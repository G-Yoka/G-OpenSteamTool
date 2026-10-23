import type { ReactNode } from "react";
import type { LogFile } from "../types";
import { parseLine } from "./logLine";
import { Card, CardHead } from "./ui";

/** 侧栏动作行：图标 + 标题 + 说明，整行可点。 */
export function SideAction({ icon, title, sub, onClick }: { icon: ReactNode; title: string; sub: string; onClick: () => void }) {
  return (
    <button className="act-row" onClick={onClick}>
      <span className="act-icon">{icon}</span>
      <span className="row-main">
        <span className="row-title">{title}</span>
        <span className="row-sub">{sub}</span>
      </span>
    </button>
  );
}

/**
 * 侧栏第三块：最近日志尾 N 行（mono、只读、单行省略）。
 * 三个满高双栏页共用，避免各自复制一份解析逻辑。
 */
export function RecentLogCard({ logs, onOpen, lines = 5 }: { logs: LogFile[]; onOpen?: () => void; lines?: number }) {
  // 真实安装里 DLL 按模块各写一份日志（main / ipc / netpacket…）：
  // 侧栏优先看主模块日志，其次本工具日志，最后退回第一份。
  const active = logs.find((log) => /^main\.log$/i.test(log.name))
    ?? logs.find((log) => /^opensteamtool/i.test(log.name))
    ?? logs[0];
  const rows = (active?.content ?? "").split(/\r?\n/).filter(Boolean).slice(-lines).map(parseLine);
  return (
    <Card>
      <CardHead
        title="最近日志"
        sub={active ? `${active.name} · 末 ${rows.length} 行` : "暂无日志"}
        right={onOpen ? <button className="btn btn-ghost btn-sm" onClick={onOpen}>打开</button> : undefined}
      />
      <div className="recent">
        {rows.map((row, index) => (
          <div className={`log-line mini nw ${row.level}`} key={`${index}-${row.raw}`}>
            <span className="lt">{row.time || "--:--:--"}</span>
            <span className="lv">{row.level || "log"}</span>
            <span className="lm">{row.message}</span>
          </div>
        ))}
        {!rows.length && <p className="note">启动 Steam 后这里会显示最近的日志。</p>}
      </div>
    </Card>
  );
}
