import { AlertTriangle, X } from "lucide-react";
import { useEffect, useRef } from "react";
import type { ConfirmRequest } from "./model";

/** 应用内确认框：跟随主题变量，替代系统原生 MessageBox。 */
export function ConfirmDialog({ request, onResolve }: { request: ConfirmRequest | null; onResolve: (ok: boolean) => void }) {
  const okRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (!request) return;
    const timer = window.setTimeout(() => okRef.current?.focus(), 0);
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onResolve(false);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("keydown", onKey);
    };
  }, [request, onResolve]);

  if (!request) return null;

  return (
    <div
      className="palette-scrim confirm-scrim"
      onMouseDown={(event) => { if (event.target === event.currentTarget) onResolve(false); }}
    >
      <div
        className={`confirm ${request.tone === "danger" ? "danger" : ""}`}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        aria-describedby="confirm-body"
      >
        <div className="confirm-head">
          <span className="confirm-icon"><AlertTriangle size={16} /></span>
          <span id="confirm-title" className="confirm-title">{request.title}</span>
          <button className="confirm-close" onClick={() => onResolve(false)} aria-label="关闭">
            <X size={14} />
          </button>
        </div>
        <div id="confirm-body" className="confirm-body">{request.message}</div>
        <div className="confirm-actions">
          <button className="btn" onClick={() => onResolve(false)}>{request.cancelLabel}</button>
          <button ref={okRef} className="btn btn-primary" onClick={() => onResolve(true)}>{request.okLabel}</button>
        </div>
      </div>
    </div>
  );
}
