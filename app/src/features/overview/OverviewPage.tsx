import { Box, CircleCheck, FolderOpen, Play, Power, RefreshCw, Rocket, Settings2 } from "lucide-react";
import type { AppController } from "../../app/useAppController";
import appIcon from "../../assets/app-icon.png";
import { Card, CardHeader, Stat } from "../../shared/ui";

export function OverviewPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const readyDlls = state.scan?.dlls.filter((dll) => dll.state === "Managed").length ?? 0;
  const loadedDlls = state.scan?.dlls.filter((dll) => dll.loaded_by_steam).length ?? 0;
  const initialized = readyDlls === 3 && Boolean(state.scan?.config_exists);
  const disabled = state.activity !== "idle";

  return <div className="overview-page">
    <Card className="hero-card">
      <div className="hero-copy">
        <span className={`status-chip ${initialized ? "ready" : ""}`}><CircleCheck size={15} />{initialized ? "环境已就绪" : "等待初始化"}</span>
        <h2>{state.steamDir ? "管理你的 OpenSteamTool 环境" : "先连接 Steam 安装目录"}</h2>
        <p>{state.steamDir ? "安装状态、游戏配置和运行日志集中在一个工作区中。" : "选择包含 steam.exe 的目录，应用会自动完成环境扫描。"}</p>
        <div className="path-box"><FolderOpen size={19} /><span>{state.steamDir || "尚未选择 Steam 根目录"}</span><button onClick={actions.chooseSteamDir}>选择目录</button></div>
        <div className="hero-actions">
          <button className="button primary" disabled={!state.steamDir || disabled || state.scan?.dll_resources_ready === false} onClick={actions.initialize}><Rocket size={18} />一键初始化</button>
          <button className="button" disabled={!state.steamDir || disabled} onClick={actions.restartSteam}><RefreshCw size={18} />重启 Steam</button>
          <button className="button danger-soft" disabled={!state.scan?.steam_running || disabled} onClick={actions.closeSteam}><Power size={18} />关闭 Steam</button>
        </div>
      </div>
      <div className="hero-visual" aria-hidden="true"><div className="core-ring ring-one" /><div className="core-ring ring-two" /><div className="core-logo"><img src={appIcon} alt="" /></div><span className="orbit-node node-one" /><span className="orbit-node node-two" /><span className="orbit-node node-three" /></div>
    </Card>

    <div className="stats-grid">
      <Stat label="Steam" value={state.scan?.steam_running ? "运行中" : "未运行"} hint={`版本 ${state.scan?.steam_version ?? "未检测"}`} tone={state.scan?.steam_running ? "good" : "default"} />
      <Stat label="DLL 组件" value={`${readyDlls} / 3`} hint={`${loadedDlls} 个已由 Steam 加载`} tone={readyDlls === 3 ? "good" : "warn"} />
      <Stat label="游戏配置" value={`${state.games.length} 个`} hint={`${state.games.filter((game) => game.enabled).length} 个已启用`} tone="accent" />
      <Stat label="TOML 配置" value={state.scan?.config_exists ? "已创建" : "未创建"} hint={`日志等级 ${state.settings.log_level}`} tone={state.scan?.config_exists ? "good" : "warn"} />
    </div>

    <div className="overview-columns">
      <Card>
        <CardHeader icon={Box} title="组件状态" description="Steam 根目录中的运行组件" actions={<button className="text-button" onClick={() => actions.setPage("dlls")}>详细管理</button>} />
        <div className="compact-list">
          {(state.scan?.dlls ?? []).map((dll) => <div key={dll.name}><span className={`component-dot ${dll.state.toLowerCase()}`} /><div><strong>{dll.name}</strong><small>{dll.loaded_by_steam ? "Steam 已加载" : dll.state === "Managed" ? "文件校验通过" : dll.state === "Foreign" ? "检测到不同版本" : "尚未安装"}</small></div><b className={`badge ${dll.state.toLowerCase()}`}>{dll.state === "Managed" ? "正常" : dll.state === "Foreign" ? "不一致" : "缺失"}</b></div>)}
          {!state.scan && <div className="skeleton-row">连接 Steam 后显示组件状态</div>}
        </div>
      </Card>
      <Card>
        <CardHeader icon={Play} title="建议操作" description="根据当前环境生成的快捷入口" />
        <div className="recommendations">
          {!state.steamDir && <button onClick={actions.chooseSteamDir}><FolderOpen size={20} /><span><strong>连接 Steam 目录</strong><small>开始扫描本机环境</small></span></button>}
          {state.steamDir && !initialized && <button onClick={actions.initialize}><Rocket size={20} /><span><strong>完成初始化</strong><small>安装 DLL 并写入默认设置</small></span></button>}
          {initialized && state.games.length === 0 && <button onClick={() => actions.setPage("games")}><Settings2 size={20} /><span><strong>创建第一个游戏配置</strong><small>填写 AppId、Depot 和 Manifest</small></span></button>}
          {initialized && state.games.length > 0 && <button onClick={() => actions.setPage("games")}><Settings2 size={20} /><span><strong>继续管理游戏</strong><small>{state.games.length} 个配置可编辑</small></span></button>}
          <button onClick={() => actions.setPage("logs")}><Box size={20} /><span><strong>检查运行日志</strong><small>{state.logs.length ? `${state.logs.length} 个日志文件` : "暂无日志"}</small></span></button>
        </div>
      </Card>
    </div>
  </div>;
}
