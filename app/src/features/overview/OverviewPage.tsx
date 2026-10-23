import { FolderOpen, Layers, PackagePlus, Power, RotateCw, Stethoscope } from "lucide-react";
import { useState } from "react";
import { APP_VERSION } from "../../app/model";
import type { AppController } from "../../app/useAppController";
import { RecentLogCard, SideAction } from "../../shared/panels";
import { Badge, Card, CardHead, KV, KVRow, Metric, Metrics, RailVerdict } from "../../shared/ui";

type Kind = "通道" | "组件";
type SurfaceRow = {
  key: string;
  kind: Kind;
  name: string;
  dep: string;
  size: string;
  state: string;
  tone: "ok" | "warn" | "idle";
};

const CHANNEL_HOST: Record<string, string> = { steamui: "steamui.dll", steamclient: "steamclient64.dll" };

export function OverviewPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const dlls = state.scan?.dlls ?? [];
  const managed = dlls.filter((dll) => dll.state === "Managed").length;
  const loaded = dlls.filter((dll) => dll.loaded_by_steam).length;
  const initialized = managed === 3 && Boolean(state.scan?.config_exists);
  const busy = state.activity !== "idle";
  const channels = state.build?.channels ?? [];
  const installed = channels.filter((file) => file.present).length;
  const missing = channels.filter((file) => !file.present);
  const dumps = state.build?.crash_dumps.length ?? 0;
  const enabledGames = state.games.filter((game) => game.enabled).length;
  const build = state.build?.steam_version ?? state.scan?.steam_version ?? "未检测";
  const layout = state.build?.dll.layout_signature?.split(":")[0] ?? null;
  const [kindFirst, setKindFirst] = useState<Kind>("通道");

  const conclusion = !state.steamDir
    ? "尚未连接 Steam 目录，页面只显示预览状态。"
    : missing.length
      ? `环境已连接；${missing.length} 个数据通道缺失，IPC 相关功能本次会话不可用。`
      : "环境已连接；三个通道齐备，重启 Steam 后生效。";
  const conclusionTone = !state.steamDir || missing.length ? "warn" : "ok";

  // 通道与组件合成一张表：同类项合并，行高仍 27，不再并排两张等高空卡
  const rows: SurfaceRow[] = [
    ...channels.map<SurfaceRow>((file) => ({
      key: `channel-${file.channel}-${file.component}`,
      kind: "通道",
      name: `${file.channel} · ${file.component}`,
      dep: CHANNEL_HOST[file.component] ?? file.component,
      size: file.present ? `${file.bytes} B` : "—",
      state: file.present ? file.entry_label : "缺失",
      tone: file.present ? "ok" : "warn",
    })),
    ...dlls.map<SurfaceRow>((dll) => ({
      key: `dll-${dll.name}`,
      kind: "组件",
      name: dll.name,
      dep: "Steam 进程",
      size: dll.name === "OpenSteamTool.dll" && state.build?.dll.size ? `${(state.build.dll.size / 1048576).toFixed(2)} MB` : "—",
      state: dll.state === "Managed"
        ? (dll.loaded_by_steam ? "校验通过 · 已加载" : "校验通过")
        : dll.state === "Foreign" ? "版本不一致" : "未安装",
      tone: dll.state === "Managed" ? "ok" : dll.state === "Foreign" ? "warn" : "idle",
    })),
  ];
  const sorted = [...rows].sort((a, b) => (a.kind === b.kind ? 0 : a.kind === kindFirst ? -1 : 1));

  return (
    <div className="page-split">
      <div className="col-main">
        {/* 签名元素：只放不变量（build / 布局偏移 / 版本），可变量交给下面的指标条 */}
        <div className="fingerprint" title="当前构建指纹">
          <span>build <b>{build}</b></span>
          <span className="fp-sep">·</span>
          <span>CNP <b>{layout ? `+0x${layout}` : "—"}</b></span>
          <span className="fp-sep">·</span>
          <span>v<b>{APP_VERSION}</b></span>
        </div>

        <Metrics>
          <Metric
            label="Steam"
            value={state.scan?.steam_running ? "运行中" : "未运行"}
            hint={`build ${build}`}
            tone={state.scan?.steam_running ? "good" : "flat"}
          />
          <Metric
            label="数据通道"
            value={`${installed}/${channels.length || 3}`}
            hint={layout ? `布局 +0x${layout}` : "布局标记未识别"}
            tone={channels.length > 0 && installed === channels.length ? "good" : "warn"}
          />
          <Metric label="组件" value={`${managed}/3`} hint={`${loaded} 个已被 Steam 加载`} tone="focus" />
          <Metric label="游戏配置" value={`${state.games.length} 个`} hint={`${enabledGames} 启用 · ${state.games.length - enabledGames} 禁用`} />
          <Metric label="崩溃转储" value={`${dumps} 份`} hint={dumps ? "到「诊断」看结论" : "近期无崩溃"} tone={dumps ? "warn" : "flat"} />
        </Metrics>

        <Card>
          <CardHead
            title="环境"
            sub={initialized ? "已就绪" : "待初始化"}
            right={<Badge tone={initialized ? "ok" : "warn"}>{initialized ? "ready" : "setup"}</Badge>}
            accent={initialized}
          />
          <KV>
            <KVRow label="Steam 目录">
              <code title={state.steamDir}>{state.steamDir || "尚未选择"}</code>
              <button className="btn btn-ghost btn-sm" onClick={actions.chooseSteamDir} disabled={busy}>
                <FolderOpen size={11} />选择
              </button>
            </KVRow>
            <KVRow label="配置文件">
              {state.scan?.config_exists ? <code>opensteamtool.toml</code> : <span className="faint">未创建</span>}
              {state.scan?.config_exists && <Badge tone="ok">存在</Badge>}
            </KVRow>
            <KVRow label="DLL 布局">
              <code>{state.build?.dll.layout_signature ? `CNetPacket +0x${state.build.dll.layout_signature.replace(":", " / size +0x")}` : "未识别"}</code>
            </KVRow>
            <KVRow label="日志文件">
              <span className="tnum">{state.logs.length} 个</span>
              {state.logs.length > 0 && <button className="btn btn-ghost btn-sm" onClick={() => actions.setPage("logs")}>查看</button>}
            </KVRow>
          </KV>
          <div className="btn-row spaced">
            <button className="btn btn-primary" onClick={actions.initialize} disabled={!state.steamDir || busy || state.scan?.dll_resources_ready === false}>
              <PackagePlus size={12} />一键初始化
            </button>
            <button className="btn" onClick={actions.restartSteam} disabled={!state.steamDir || busy}>
              <RotateCw size={12} />重启 Steam
            </button>
            <button className="btn btn-danger" onClick={actions.closeSteam} disabled={!state.scan?.steam_running || busy}>
              <Power size={12} />关闭 Steam
            </button>
          </div>
        </Card>

        <Card>
          <CardHead
            title="通道与组件"
            sub="3 个 DLL · 2 类通道数据"
            right={
              <div className="btn-row">
                <button className="btn btn-ghost btn-sm" onClick={() => actions.setPage("channels")} title="打开数据通道页">通道管理</button>
                <button className="btn btn-ghost btn-sm" onClick={() => actions.setPage("dlls")} title="打开组件页">组件管理</button>
              </div>
            }
          />
          <table className="table">
            <thead>
              <tr>
                <th
                  className="sortable"
                  onClick={() => setKindFirst((current) => (current === "通道" ? "组件" : "通道"))}
                  title="按类型排序"
                >
                  类型<span className="sort-mark">{kindFirst === "通道" ? "↑" : "↓"}</span>
                </th>
                <th>名称</th>
                <th>依赖</th>
                <th className="num">大小</th>
                <th>状态</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((row) => (
                <tr key={row.key}>
                  <td className="dim">{row.kind}</td>
                  <td className="strong nowrap">{row.name}</td>
                  <td><code>{row.dep}</code></td>
                  <td className="num">{row.size}</td>
                  <td><Badge tone={row.tone}>{row.state}</Badge></td>
                </tr>
              ))}
              {!sorted.length && <tr><td colSpan={5}><span className="faint">连接 Steam 后显示通道与组件状态</span></td></tr>}
            </tbody>
          </table>
        </Card>
      </div>

      <div className="col-side">
        <RailVerdict tone={conclusionTone} text={conclusion} />

        <Card>
          <CardHead title="下一步" sub="按当前环境排序" />
          <div className="ov-actions">
            {!state.steamDir && <SideAction icon={<FolderOpen size={12} />} title="连接 Steam 目录" sub="扫描本机环境" onClick={actions.chooseSteamDir} />}
            {state.steamDir && !initialized && <SideAction icon={<PackagePlus size={12} />} title="完成初始化" sub="安装 DLL 并写入默认 TOML" onClick={actions.initialize} />}
            {missing.length > 0 && <SideAction icon={<Layers size={12} />} title="补齐数据通道" sub={`${missing.length} 个通道缺失`} onClick={() => actions.setPage("channels")} />}
            {state.games.length > 0 && <SideAction icon={<Layers size={12} />} title="继续管理游戏" sub={`${state.games.length} 个配置可编辑`} onClick={() => actions.setPage("games")} />}
            <SideAction icon={<Stethoscope size={12} />} title="查看诊断" sub="崩溃转储、缺失条目与故障隔离" onClick={() => actions.setPage("diagnostics")} />
          </div>
        </Card>

        <RecentLogCard logs={state.logs} onOpen={() => actions.setPage("logs")} />
      </div>
    </div>
  );
}
