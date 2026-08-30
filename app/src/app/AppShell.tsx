import {
  Activity, Box, CircleHelp, Code2, Gauge, Minus, RefreshCw, Settings, X,
} from "lucide-react";
import type { MouseEvent } from "react";
import type { AppController } from "./useAppController";
import type { PageId } from "./model";
import { closeWindow, minimizeWindow, startWindowDrag } from "../api";
import appIcon from "../assets/app-icon.png";
import { AboutPage } from "../features/about/AboutPage";
import { DllPage } from "../features/dlls/DllPage";
import { GamesPage } from "../features/games/GamesPage";
import { LogsPage } from "../features/logs/LogsPage";
import { OverviewPage } from "../features/overview/OverviewPage";
import { SettingsPage } from "../features/settings/SettingsPage";

const navigation: Array<{ id: PageId; label: string; detail: string; icon: typeof Gauge }> = [
  { id: "overview", label: "概览", detail: "环境与快捷操作", icon: Gauge },
  { id: "games", label: "游戏配置", detail: "Lua 与 Depot", icon: Code2 },
  { id: "dlls", label: "组件", detail: "DLL 状态与安装", icon: Box },
  { id: "settings", label: "设置", detail: "OpenSteamTool", icon: Settings },
  { id: "logs", label: "日志", detail: "运行记录", icon: Activity },
  { id: "about", label: "关于", detail: "版本与更新", icon: CircleHelp },
];

export function AppShell({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const selected = navigation.find((item) => item.id === state.page) ?? navigation[0];
  function startDrag(event: MouseEvent<HTMLElement>) {
    if (event.button === 0 && !(event.target as HTMLElement).closest("button")) {
      void startWindowDrag();
    }
  }

  return (
    <div className="app-frame">
      <header className="titlebar" data-tauri-drag-region onMouseDown={startDrag}>
        <div className="brand-mark"><img src={appIcon} alt="" /></div>
        <div className="titlebar-name">
          <strong>G-OpenSteamTool</strong>
        </div>
        <div className="titlebar-context">
          <span className={`live-dot ${state.scan?.steam_running ? "online" : ""}`} />
          {state.scan?.steam_running ? "Steam 运行中" : "Steam 未运行"}
        </div>
        <div className="window-controls">
          <button aria-label="最小化" onClick={() => void minimizeWindow()}><Minus size={17} /></button>
          <button className="close" aria-label="关闭" onClick={() => void closeWindow()}><X size={17} /></button>
        </div>
      </header>

      <div className="app-layout">
        <aside className="sidebar">
          <nav aria-label="主导航">
            {navigation.map((item) => {
              const Icon = item.icon;
              return (
                <button key={item.id} className={state.page === item.id ? "active" : ""} onClick={() => actions.setPage(item.id)}>
                  <span className="nav-icon"><Icon size={20} /></span>
                  <span><strong>{item.label}</strong><small>{item.detail}</small></span>
                </button>
              );
            })}
          </nav>
          <div className="sidebar-foot">
            <span>v0.3.0</span>
            <span className={state.scan?.steam_valid ? "healthy" : ""}>{state.scan?.steam_valid ? "环境已连接" : "等待连接"}</span>
          </div>
        </aside>

        <main className="workspace-new">
          <div className="page-heading">
            <div><span className="eyebrow">WORKSPACE</span><h1>{selected.label}</h1><p>{selected.detail}</p></div>
            <button className="button ghost" onClick={actions.refreshAll} disabled={!state.steamDir || state.activity !== "idle"}>
              <RefreshCw size={17} className={state.activity !== "idle" ? "spin" : ""} />刷新
            </button>
          </div>

          <div className={`notice ${state.notice.tone}`} role="status">
            <span className="notice-pulse" />
            <strong>{noticeLabel(state.notice.tone)}</strong>
            <p>{state.notice.message}</p>
            <time>{state.notice.time}</time>
          </div>

          <div className="page-content">
            {state.page === "overview" && <OverviewPage controller={controller} />}
            {state.page === "games" && <GamesPage controller={controller} />}
            {state.page === "dlls" && <DllPage controller={controller} />}
            {state.page === "settings" && <SettingsPage controller={controller} />}
            {state.page === "logs" && <LogsPage controller={controller} />}
            {state.page === "about" && <AboutPage controller={controller} />}
          </div>
        </main>
      </div>
    </div>
  );
}

function noticeLabel(tone: string) {
  if (tone === "success") return "完成";
  if (tone === "error") return "需要处理";
  if (tone === "working") return "处理中";
  return "状态";
}
