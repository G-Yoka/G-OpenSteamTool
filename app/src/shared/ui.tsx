import { useState, type ReactNode } from "react";

/** 面板：所有内容都装在 1px 描边的卡片里，不用阴影分层。 */
export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <section className={`card ${className}`}>{children}</section>;
}

export function CardHead({ title, sub, right, accent = false }: { title: string; sub?: string; right?: ReactNode; accent?: boolean }) {
  return (
    <header className={`card-head ${accent ? "accent" : ""}`}>
      <h2>{title}</h2>
      {sub && <span className="sub">{sub}</span>}
      {right && <div className="right">{right}</div>}
    </header>
  );
}

/** 指标条：一行贴在一起的数字，比四张独立卡省一半纵向空间。 */
export function Metrics({ children }: { children: ReactNode }) {
  return <div className="metrics">{children}</div>;
}

export function Metric({ label, value, hint, tone = "flat" }: { label: string; value: ReactNode; hint?: string; tone?: "flat" | "good" | "warn" | "bad" | "focus" }) {
  return (
    <div className={`metric ${tone === "flat" ? "" : tone}`}>
      <div className="m-label">{label}</div>
      <div className="m-value">{value}</div>
      {hint && <div className="m-hint">{hint}</div>}
    </div>
  );
}

export function KV({ children }: { children: ReactNode }) {
  return <dl className="kv">{children}</dl>;
}

export function KVRow({ label, children }: { label: string; children: ReactNode }) {
  return <><dt>{label}</dt><dd>{children}</dd></>;
}

export function Badge({ tone = "idle", children }: { tone?: "ok" | "warn" | "bad" | "idle"; children: ReactNode }) {
  return <span className={`badge ${tone}`}>{children}</span>;
}

/** 快捷键提示：空格分隔的每一段单独成一枚键帽，读起来像 Raycast 的快捷键标签。 */
export function Kbd({ keys }: { keys: string }) {
  return <>{keys.split(" ").map((key) => <span className="kbd" key={key}>{key}</span>)}</>;
}

export function Toggle({ checked, onChange, label, description }: { checked: boolean; onChange: (next: boolean) => void; label: string; description?: string }) {
  return (
    <button type="button" className={`toggle ${checked ? "on" : ""}`} onClick={() => onChange(!checked)} aria-pressed={checked}>
      <span className="toggle-ind"><i /></span>
      <span className="toggle-text"><strong>{label}</strong>{description && <small>{description}</small>}</span>
    </button>
  );
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return <label className="field"><span>{label}</span>{children}{hint && <small>{hint}</small>}</label>;
}

export function Empty({ title, description, action }: { title: string; description: string; action?: ReactNode }) {
  return <div className="empty"><div className="empty-title">{title}</div><p className="empty-sub">{description}</p>{action}</div>;
}

/**
 * 结论块：一句结论 + 一个动作。表格页永远先给判断，再给明细。
 * stacked 用于 320px 侧栏：点与首行文字对齐，句子在 v-text 内自然换行。
 * compact 用于「正常」态：收成一行状态条，不占卡片位。
 */
const VERDICT_WORD: Record<string, string> = { ok: "正常", warn: "注意", bad: "异常" };

export function Verdict({ tone = "flat", text, action, stacked = false, compact = false, word }: {
  tone?: "flat" | "ok" | "warn" | "bad";
  text: ReactNode;
  action?: ReactNode;
  stacked?: boolean;
  compact?: boolean;
  word?: string;
}) {
  const label = word ?? VERDICT_WORD[tone];
  return (
    <div className={`verdict ${stacked ? "stacked" : ""} ${compact ? "compact" : ""} ${tone === "flat" ? "" : "tone-" + tone}`}>
      <span className="v-dot" />
      {label ? <span className="verdict-word">{label}</span> : null}
      <span className="v-text">{text}</span>
      {action}
    </div>
  );
}

/** 右栏判决块：正常态只是一行状态条；只有「注意 / 异常」才升成一张卡。 */
export function RailVerdict({ tone = "flat", text, action }: {
  tone?: "flat" | "ok" | "warn" | "bad";
  text: ReactNode;
  action?: ReactNode;
}) {
  return <Verdict tone={tone} text={text} action={action} stacked compact={tone === "ok"} />;
}

/** 复制到剪贴板：优先 Clipboard API，失败时退回临时 textarea + execCommand。 */
async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const helper = document.createElement("textarea");
      helper.value = text;
      helper.setAttribute("readonly", "");
      helper.style.position = "fixed";
      helper.style.opacity = "0";
      document.body.appendChild(helper);
      helper.select();
      const ok = document.execCommand("copy");
      document.body.removeChild(helper);
      return ok;
    } catch {
      return false;
    }
  }
}

/** hash 单元格：中间省略显示，title 保留全文，单击复制（复制后短暂显示「已复制」）。 */
export function Hash({ value, title, truncate = true }: { value?: string | null; title?: string; truncate?: boolean }) {
  const [copied, setCopied] = useState(false);
  const full = (value ?? "").trim();
  if (!full) return <span className="faint">—</span>;
  const short = truncate && full.length > 20 ? `${full.slice(0, 12)}…${full.slice(-6)}` : full;
  async function copy() {
    if (!(await copyText(full))) return;
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  }
  return (
    <code
      className="hash"
      title={title ?? `${full}\n（单击复制）`}
      data-copied={copied ? "" : undefined}
      onClick={() => void copy()}
    >
      {copied ? "已复制" : short}
    </code>
  );
}
