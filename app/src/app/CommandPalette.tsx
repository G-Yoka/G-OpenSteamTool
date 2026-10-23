import { CornerDownLeft, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { rankCommands, type Command } from "./commands";
import { Kbd } from "../shared/ui";

/**
 * 命令面板：Raycast 的核心手感——一个入口覆盖全部跳转与操作，
 * 全程可键盘操作（↑↓ 选择 / ↵ 执行 / esc 关闭），鼠标只是备选路径。
 */
export function CommandPalette({ open, commands, onClose }: { open: boolean; commands: Command[]; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const ranked = useMemo(() => rankCommands(commands, query), [commands, query]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActive(0);
    window.setTimeout(() => inputRef.current?.focus(), 0);
  }, [open]);

  useEffect(() => {
    if (active > ranked.length - 1) setActive(Math.max(0, ranked.length - 1));
  }, [active, ranked.length]);

  useEffect(() => {
    listRef.current?.querySelector('[data-active="true"]')?.scrollIntoView({ block: "nearest" });
  }, [active, ranked]);

  if (!open) return null;

  function commit(command: Command | undefined) {
    if (!command || command.disabled) return;
    onClose();
    command.run();
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "ArrowDown" || (event.key === "n" && event.ctrlKey)) {
      event.preventDefault();
      setActive((current) => (ranked.length ? (current + 1) % ranked.length : 0));
    } else if (event.key === "ArrowUp" || (event.key === "p" && event.ctrlKey)) {
      event.preventDefault();
      setActive((current) => (ranked.length ? (current - 1 + ranked.length) % ranked.length : 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      commit(ranked[active]);
    } else if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "Tab") {
      event.preventDefault();
    }
  }

  let lastGroup = "";
  return (
    <div className="palette-scrim" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div className="palette" role="dialog" aria-label="命令面板" onKeyDown={onKeyDown}>
        <div className="palette-input">
          <Search size={14} />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => { setQuery(event.target.value); setActive(0); }}
            placeholder="搜索页面、操作或路径…"
            spellCheck={false}
          />
          {query && <button className="clear" onClick={() => setQuery("")} aria-label="清空"><X size={13} /></button>}
        </div>
        <div className="palette-list" ref={listRef}>
          {!ranked.length && <div className="palette-empty">没有匹配的命令</div>}
          {ranked.map((command, index) => {
            const Icon = command.icon;
            const header = command.group !== lastGroup ? command.group : "";
            lastGroup = command.group;
            return (
              <div key={command.id}>
                {header && <div className="palette-group">{header}</div>}
                <button
                  className={`palette-item ${index === active ? "active" : ""}`}
                  data-active={index === active}
                  disabled={command.disabled}
                  onMouseMove={() => setActive(index)}
                  onClick={() => commit(command)}
                >
                  <span className="pi-icon"><Icon size={13} /></span>
                  <span className="pi-title">{command.title}</span>
                  {command.hint && <span className="pi-hint">{command.hint}</span>}
                </button>
              </div>
            );
          })}
        </div>
        <div className="palette-foot">
          <span><Kbd keys="↑ ↓" /> 选择</span>
          <span><Kbd keys="↵" /> 执行</span>
          <span><Kbd keys="esc" /> 关闭</span>
          <span className="spacer"><CornerDownLeft size={11} /></span>
        </div>
      </div>
    </div>
  );
}

export const SHORTCUTS: Array<[string, string]> = [
  ["Ctrl K", "命令面板：跳转页面 / 触发操作"],
  ["Ctrl 1…8", "按顺序直达左侧导航"],
  ["Ctrl R", "刷新全部状态"],
  ["Ctrl F", "聚焦当前视图的搜索框"],
  ["Ctrl ⇧ L", "切换浅色 / 深色主题"],
  ["?", "打开这张快捷键表"],
  ["Esc", "关闭浮层 / 退出输入框"],
  ["↑ ↓", "在列表与命令之间移动"],
  ["↵", "执行选中的命令"],
];

export function ShortcutSheet({ open, onClose }: { open: boolean; onClose: () => void }) {
  if (!open) return null;
  return (
    <div className="palette-scrim" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div className="sheet" role="dialog" aria-label="快捷键">
        <div className="sheet-head">
          快捷键
          <span className="right"><Kbd keys="esc" /></span>
        </div>
        <div className="sheet-body">
          {SHORTCUTS.map(([keys, label]) => (
            <div className="sheet-row" key={keys}>
              <span>{label}</span>
              <span className="keys"><Kbd keys={keys} /></span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
