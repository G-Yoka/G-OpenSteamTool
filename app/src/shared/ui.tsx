import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <section className={`card ${className}`}>{children}</section>;
}

export function CardHeader({ icon: Icon, title, description, actions }: { icon: LucideIcon; title: string; description?: string; actions?: ReactNode }) {
  return <header className="card-header"><span className="card-icon"><Icon size={19} /></span><div><h2>{title}</h2>{description && <p>{description}</p>}</div>{actions && <div className="card-actions">{actions}</div>}</header>;
}

export function Stat({ label, value, hint, tone = "default" }: { label: string; value: string; hint: string; tone?: "default" | "good" | "warn" | "accent" }) {
  return <div className={`stat ${tone}`}><span>{label}</span><strong>{value}</strong><small>{hint}</small></div>;
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return <label className="field"><span>{label}</span>{children}{hint && <small>{hint}</small>}</label>;
}

export function Toggle({ checked, onChange, label, description }: { checked: boolean; onChange: (next: boolean) => void; label: string; description?: string }) {
  return <button type="button" className={`toggle-row ${checked ? "checked" : ""}`} onClick={() => onChange(!checked)} aria-pressed={checked}>
    <span><strong>{label}</strong>{description && <small>{description}</small>}</span><i><b /></i>
  </button>;
}

export function EmptyState({ title, description, action }: { title: string; description: string; action?: ReactNode }) {
  return <div className="empty-state"><div className="empty-orbit"><span /></div><strong>{title}</strong><p>{description}</p>{action}</div>;
}
