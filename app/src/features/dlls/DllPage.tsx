import { Download, FileSearch, FolderOpen, ShieldAlert, Stethoscope, Trash2 } from "lucide-react";
import type { AppController } from "../../app/useAppController";
import type { DllStatus } from "../../types";
import { RecentLogCard, SideAction } from "../../shared/panels";
import { Badge, Card, CardHead, Empty, Hash, KV, KVRow, Verdict } from "../../shared/ui";

export function DllPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const busy = state.activity !== "idle" || !state.steamDir;
  if (!state.scan) {
    return <Empty title="尚未连接 Steam" description="选择 Steam 根目录后即可查看组件状态。" />;
  }
  const loaded = state.scan.dlls.filter((dll) => dll.loaded_by_steam).length;
  const managed = state.scan.dlls.filter((dll) => dll.state === "Managed").length;
  const ready = managed === 3;
  const steamRunning = state.scan.steam_running;
  // 只有 OpenSteamTool.dll 自己没进进程才算真问题：另外两个是代理，备用链未触发属于正常
  const ostLoaded = Boolean(state.scan.dlls.find((dll) => dll.name === "OpenSteamTool.dll")?.loaded_by_steam);

  return (
    <div className="page-split">
      <div className="col-main">
        <Verdict
          tone={!ready ? "warn" : !steamRunning ? "flat" : ostLoaded ? "ok" : "warn"}
          text={!ready
            ? <>{3 - managed} 个组件未安装或哈希不一致；先「安装或修复」，再启动 Steam</>
            : !steamRunning
              ? <>3 个组件校验通过；Steam 未运行，启动后这里会显示实际加载状态</>
              : ostLoaded
                ? <>3 个组件校验通过，{loaded} 个已被 Steam 加载，可直接启动 Steam</>
                : <>3 个组件校验通过，但 OpenSteamTool.dll 没被 Steam 加载；重启 Steam 后再复查</>}
          action={
            <button className="btn btn-sm" data-kbd="Ctrl R" onClick={actions.refreshAll} disabled={!state.steamDir || state.activity !== "idle"}>
              <FileSearch size={11} />重新扫描
            </button>
          }
        />

        <Card>
          <CardHead
            title="托管组件"
            sub="3 个组件，按哈希校验"
            accent={ready}
            right={
              <div className="btn-row">
                <button className="btn btn-primary btn-sm" disabled={busy || state.scan.dll_resources_ready === false} onClick={actions.installDlls}>
                  <Download size={11} />安装或修复
                </button>
                <button className="btn btn-danger btn-sm" disabled={busy} onClick={actions.removeDlls}>
                  <Trash2 size={11} />安全移除
                </button>
              </div>
            }
          />
          <table className="table">
            <thead>
              <tr><th>文件</th><th>状态</th><th>资源哈希</th><th>目标哈希</th><th>Steam 加载</th></tr>
            </thead>
            <tbody>
              {state.scan.dlls.map((dll) => <DllRow key={dll.name} dll={dll} />)}
            </tbody>
          </table>
        </Card>

        <Card>
          <CardHead title="资源状态" sub="内置 DLL 资源是否随 EXE 一起分发" />
          <KV>
            <KVRow label="资源就绪">
              <Badge tone={state.scan.dll_resources_ready ? "ok" : "bad"}>{state.scan.dll_resources_ready ? "是" : "否"}</Badge>
            </KVRow>
            <KVRow label="缺失资源">
              <code>{state.scan.missing_dll_resources.length ? state.scan.missing_dll_resources.join(" · ") : "无"}</code>
            </KVRow>
            <KVRow label="Steam 进程">
              <Badge tone={state.scan.steam_running ? "ok" : "idle"}>{state.scan.steam_running ? "运行中" : "未运行"}</Badge>
            </KVRow>
          </KV>
          {!state.scan.dll_resources_ready && (
            <p className="inline-warning"><ShieldAlert size={13} />EXE 里没有找到 DLL 资源，安装会失败。请使用完整便携版。</p>
          )}
        </Card>
      </div>

      <div className="col-side">
        <Card>
          <CardHead title="下一步" sub="按当前状态排序" />
          <div className="ov-actions">
            <SideAction icon={<Download size={12} />} title={ready ? "重新校验组件" : "安装或修复"} sub={ready ? "比对哈希与加载状态" : `${3 - managed} 个待补齐`} onClick={actions.installDlls} />
            <SideAction icon={<Trash2 size={12} />} title="安全移除" sub="只删除哈希匹配的托管文件" onClick={actions.removeDlls} />
            <SideAction icon={<Stethoscope size={12} />} title="查看诊断" sub="崩溃转储与缺失条目" onClick={() => actions.setPage("diagnostics")} />
            <SideAction icon={<FolderOpen size={12} />} title="打开日志" sub="看 DLL 是否被真正加载" onClick={() => actions.setPage("logs")} />
          </div>
        </Card>

        <RecentLogCard logs={state.logs} onOpen={() => actions.setPage("logs")} />
      </div>
    </div>
  );
}

const DLL_STATE_HINT: Record<DllStatus["state"], string> = {
  Managed: "与应用内置资源哈希完全一致",
  Foreign: "文件存在，但不是当前版本提供的组件",
  Missing: "Steam 根目录中没有找到该文件",
};

function DllRow({ dll }: { dll: DllStatus }) {
  const label = dll.state === "Managed" ? "校验通过" : dll.state === "Foreign" ? "版本不一致" : "未安装";
  return (
    <tr>
      <td>
        <div className="row-main">
          <span className="strong mono">{dll.name}</span>
          <span className="row-sub" title={dll.target_path}>{dll.target_path}</span>
        </div>
      </td>
      <td><span title={DLL_STATE_HINT[dll.state]}><Badge tone={dll.state === "Managed" ? "ok" : dll.state === "Foreign" ? "warn" : "idle"}>{label}</Badge></span></td>
      <td><Hash value={dll.resource_hash} /></td>
      <td><Hash value={dll.target_hash} /></td>
      <td className={dll.loaded_by_steam ? "pos" : ""} title={loadHint(dll)}>{loadLabel(dll)}</td>
    </tr>
  );
}

function loadLabel(dll: DllStatus) {
  if (dll.loaded_by_steam) return "已加载";
  if (dll.load_state === "SteamNotRunning") return "Steam 未运行";
  if (dll.load_state === "VerifyFailed") return "无法验证";
  // 托管但没进进程：多半是代理通道本次没被触发，不是故障
  return dll.state === "Managed" ? "未加载 · 备用" : "未加载";
}

function loadHint(dll: DllStatus) {
  if (dll.loaded_by_steam) return "已载入 steam.exe，注入链生效";
  if (dll.load_state === "SteamNotRunning") return "Steam 未运行，启动后再看这一列";
  if (dll.load_state === "VerifyFailed") return "读不到 Steam 进程模块表，可能权限不足";
  if (dll.state === "Managed") {
    return "这份代理只在 Steam 本次运行时解析到它才生效；当前注入由已加载的项承担，不影响功能";
  }
  return "文件不在位，先安装或修复";
}
