import {
  Box, CircleHelp, Code2, FileText, FolderOpen, Gauge, Layers, Minus, Moon, PackagePlus, Play, Power,
  RefreshCw, RotateCw, Save, Settings, Square, Stethoscope, Sun, Trash2, X, Keyboard,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type MouseEvent } from "react";
import type { AppController } from "./useAppController";
import { APP_VERSION, type PageId } from "./model";
import { closeWindow, minimizeWindow, startWindowDrag, toggleMaximize } from "../api";
import appIcon from "../assets/app-icon.png";
import type { Command } from "./commands";
import { CommandPalette, ShortcutSheet } from "./CommandPalette";
import { useTheme } from "./useTheme";
import { Kbd } from "../shared/ui";
import { AboutPage } from "../features/about/AboutPage";
import { ChannelsPage } from "../features/channels/ChannelsPage";
import { DiagnosticsPage } from "../features/diagnostics/DiagnosticsPage";
import { DllPage } from "../features/dlls/DllPage";
import { GamesPage } from "../features/games/GamesPage";
import { LogsPage } from "../features/logs/LogsPage";
import { OverviewPage } from "../features/overview/OverviewPage";
import { SettingsPage } from "../features/settings/SettingsPage";

const NAV: Array<{ id: PageId; label: string; detail: string; icon: typeof Gauge }> = [
  { id: "overview", label: "状态", detail: "环境与快捷操作", icon: Gauge },
  { id: "channels", label: "数据通道", detail: "构建对应的通道数据", icon: Layers },
  { id: "dlls", label: "组件", detail: "DLL 与加载器", icon: Box },
  { id: "games", label: "游戏配置", detail: "Lua 与 Depot", icon: Code2 },
  { id: "logs", label: "日志", detail: "运行记录与过滤", icon: FileText },
  { id: "diagnostics", label: "诊断", detail: "崩溃、缺失与隔离", icon: Stethoscope },
  { id: "settings", label: "设置", detail: "TOML 与数据源", icon: Settings },
  { id: "about", label: "关于", detail: "版本与许可", icon: CircleHelp },
];

export function AppShell({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const { theme, toggle: toggleTheme } = useTheme();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [sheetOpen, setSheetOpen] = useState(false);
  const selected = NAV.find((item) => item.id === state.page) ?? NAV[0];
  const busy = state.activity !== "idle";
  const noDir = !state.steamDir;

  function startDrag(event: MouseEvent<HTMLElement>) {
    if (event.button === 0 && !(event.target as HTMLElement).closest("button")) void startWindowDrag();
  }

  const commands = useMemo<Command[]>(() => {
    const pages = NAV.map<Command>((item, index) => ({
      id: `page-${item.id}`,
      title: `跳转到 ${item.label}`,
      group: "跳转",
      icon: item.icon,
      hint: `Ctrl ${index + 1}`,
      keywords: item.detail,
      run: () => actions.setPage(item.id),
    }));
    return [
      ...pages,
      {
        id: "init", title: "一键初始化环境", group: "环境", icon: PackagePlus,
        keywords: "安装 dll 创建 toml",
        disabled: noDir || busy || state.scan?.dll_resources_ready === false,
        run: actions.initialize,
      },
      {
        id: "refresh", title: "刷新全部状态", group: "环境", icon: RefreshCw, hint: "Ctrl R",
        disabled: noDir || busy, run: actions.refreshAll,
      },
      {
        id: "dir", title: "选择 Steam 根目录", group: "环境", icon: FolderOpen,
        disabled: busy, run: () => void actions.chooseSteamDir(),
      },
      {
        id: "restart", title: "重启 Steam", group: "环境", icon: RotateCw,
        disabled: noDir || busy, run: actions.restartSteam,
      },
      {
        id: "close", title: "关闭 Steam", group: "环境", icon: Power,
        disabled: !state.scan?.steam_running || busy, run: actions.closeSteam,
      },
      {
        id: "install-dlls", title: "安装或修复组件", group: "组件", icon: Save,
        disabled: noDir || busy || state.scan?.dll_resources_ready === false, run: actions.installDlls,
      },
      {
        id: "remove-dlls", title: "安全移除组件", group: "组件", icon: Trash2,
        disabled: noDir || busy, run: actions.removeDlls,
      },
      {
        id: "import-lua", title: "导入 Lua 配置", group: "数据", icon: Play,
        disabled: noDir || busy, run: () => void actions.importLua(),
      },
      {
        id: "open-lua", title: "打开 Lua 目录", group: "数据", icon: FolderOpen,
        disabled: noDir, run: () => void actions.openLuaDir(),
      },
      {
        id: "theme",
        title: theme === "dark" ? "切换到浅色主题" : "切换到深色主题",
        group: "环境",
        icon: theme === "dark" ? Sun : Moon,
        hint: "Ctrl ⇧ L",
        keywords: "theme dark light 深色 浅色 主题",
        run: toggleTheme,
      },
      {
        id: "shortcuts", title: "查看快捷键", group: "数据", icon: Keyboard, hint: "?",
        run: () => setSheetOpen(true),
      },
    ];
  }, [actions, busy, noDir, state.scan, theme, toggleTheme]);

  const focusSearch = useCallback(() => {
    const input = document.querySelector<HTMLInputElement>(".search input");
    if (input) { input.focus(); input.select(); }
    else setPaletteOpen(true);
  }, []);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const target = event.target;
      const typing = target instanceof HTMLElement && target.closest("input, textarea, select") !== null;
      const mod = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();

      if (mod && key === "k") { event.preventDefault(); setPaletteOpen((open) => !open); setSheetOpen(false); return; }
      if (event.key === "Escape") { setPaletteOpen(false); setSheetOpen(false); return; }
      if (mod && key === "r") { event.preventDefault(); if (state.steamDir && !busy) actions.refreshAll(); return; }
      if (mod && key === "f") { event.preventDefault(); focusSearch(); return; }
      if (mod && event.shiftKey && key === "l") { event.preventDefault(); toggleTheme(); return; }
      if (mod && /^[1-8]$/.test(event.key)) { event.preventDefault(); const target = NAV[Number(event.key) - 1]; if (target) actions.setPage(target.id); return; }
      if (!typing && event.key === "?") { event.preventDefault(); setSheetOpen((open) => !open); }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [actions, busy, focusSearch, state.steamDir, toggleTheme]);


  // 按住 Alt 时导航行全部显快捷键，松开即收
  useEffect(() => {
    const root = document.documentElement;
    const show = (event: KeyboardEvent) => { if (event.key === "Alt") root.dataset.alt = "1"; };
    const hide = (event: KeyboardEvent) => { if (event.key === "Alt") delete root.dataset.alt; };
    const clear = () => { delete root.dataset.alt; };
    window.addEventListener("keydown", show);
    window.addEventListener("keyup", hide);
    window.addEventListener("blur", clear);
    return () => {
      window.removeEventListener("keydown", show);
      window.removeEventListener("keyup", hide);
      window.removeEventListener("blur", clear);
      delete root.dataset.alt;
    };
  }, []);

  return (
    <div className="frame">
      <header className="titlebar" data-tauri-drag-region onMouseDown={startDrag}>
        <div className="brand">
          <img className="brand-logo" src={appIcon} alt="" />
          <span className="brand-name">G-OpenSteamTool</span>
          <span className="brand-ver">{APP_VERSION}</span>
        </div>
        <div className="titlebar-right">
          <span className="conn">
            <span className={`dot ${state.scan?.steam_running ? "on" : ""}`} />
            {state.scan?.steam_running ? "Steam 运行中" : "Steam 未运行"}
          </span>
          <button className="kbd-btn" data-kbd="Ctrl K" onClick={() => setPaletteOpen(true)} title="打开命令面板（Ctrl K）">
            命令
          </button>
          <button
            className="icon-toggle"
            onClick={toggleTheme}
            aria-label={theme === "dark" ? "切换到浅色主题" : "切换到深色主题"}
            title={`${theme === "dark" ? "浅色" : "深色"}主题（Ctrl ⇧ L）`}
          >
            {theme === "dark" ? <Sun size={12} /> : <Moon size={12} />}
          </button>
          <div className="window-controls">
            <button className="win-btn" aria-label="最小化" onClick={() => void minimizeWindow()}><Minus size={14} /></button>
            <button className="win-btn" aria-label="最大化或还原" onClick={() => void toggleMaximize()}><Square size={11} /></button>
            <button className="win-btn close" aria-label="关闭" onClick={() => void closeWindow()}><X size={14} /></button>
          </div>
        </div>
      </header>

      <div className="body">
        <aside className="rail">
          <div className="rail-label">工作区</div>
          <nav className="rail-nav" aria-label="主导航">
            {NAV.map((item, index) => {
              const Icon = item.icon;
              return (
                <button
                  key={item.id}
                  className={`rail-item ${state.page === item.id ? "active" : ""}`}
                  onClick={() => actions.setPage(item.id)}
                  title={`${item.label} · ${item.detail}`}
                  aria-current={state.page === item.id}
                >
                  <span className="rail-icon"><Icon size={14} /></span>
                  <span className="rail-text">{item.label}</span>
                  <span className="rail-kbd"><Kbd keys={`${index + 1}`} /></span>
                </button>
              );
            })}
          </nav>
          <div className="rail-foot">
            <div className="rail-foot-row">
              <span>环境</span>
              <span className={state.scan?.steam_valid ? "pos" : ""}>{state.scan?.steam_valid ? "已连接" : "未连接"}</span>
            </div>
            <div className="rail-foot-row">
              <span>配置</span>
              <span className="tnum">{state.games.length} 个 Lua</span>
            </div>
          </div>
        </aside>

        <main className="main">
          <div className="page-head">
            <h1>{selected.label}</h1>
            <span className="crumb">{selected.detail}</span>
            <div className="head-actions">
              <button className="btn btn-ghost btn-sm" data-kbd="Ctrl R" onClick={actions.refreshAll} disabled={!state.steamDir || busy} title="刷新全部状态（Ctrl R）">
                <RefreshCw size={12} className={busy ? "spin" : ""} />刷新
              </button>
            </div>
          </div>

          <div className="content">
            {state.page === "overview" && <OverviewPage controller={controller} />}
            {state.page === "channels" && <ChannelsPage controller={controller} />}
            {state.page === "dlls" && <DllPage controller={controller} />}
            {state.page === "games" && <GamesPage controller={controller} />}
            {state.page === "logs" && <LogsPage controller={controller} />}
            {state.page === "diagnostics" && <DiagnosticsPage controller={controller} />}
            {state.page === "settings" && <SettingsPage controller={controller} />}
            {state.page === "about" && <AboutPage controller={controller} />}
          </div>
        </main>
      </div>

      <footer className="statusbar">
        <span className="path" title={state.steamDir}>{middleEllipsis(state.steamDir || "未选择 Steam 目录", 32)}</span>
        <span className={`sb-notice ${state.notice.tone}`} role="status" title={state.notice.message}>
          <span className="notice-dot" />
          <span className="sb-label">{noticeLabel(state.notice.tone)}</span>
          <span className="sb-msg">{state.notice.message}</span>
          <span className="sb-time mono">{state.notice.time}</span>
        </span>
        <span className="spacer" />
        <span className="sb-item"><StatusClock /></span>
        <span className="sb-item"><Kbd keys="Ctrl K" /> 命令</span>
        <span className="sb-item"><Kbd keys="?" /> 快捷键</span>
      </footer>

      <CommandPalette open={paletteOpen} commands={commands} onClose={() => setPaletteOpen(false)} />
      <ShortcutSheet open={sheetOpen} onClose={() => setSheetOpen(false)} />
    </div>
  );
}

function noticeLabel(tone: string) {
  if (tone === "success") return "完成";
  if (tone === "error") return "需要处理";
  if (tone === "working") return "处理中";
  return "状态";
}

/** 状态栏路径用中间省略：保留盘符与末级目录，中间缩掉。 */
function middleEllipsis(text: string, max: number) {
  if (text.length <= max) return text;
  const keepHead = Math.ceil((max - 1) / 2);
  const keepTail = Math.floor((max - 1) / 2);
  return `${text.slice(0, keepHead)}…${text.slice(-keepTail)}`;
}

/** 时钟独立成组件：每秒只重渲染这一小块，不带动整页。 */
function StatusClock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  return <span className="mono">{now.toLocaleTimeString("zh-CN", { hour12: false })}</span>;
}
