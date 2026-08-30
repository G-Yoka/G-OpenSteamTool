import { Box, Download, FileCheck2, RefreshCw, ShieldAlert, Trash2 } from "lucide-react";
import type { AppController } from "../../app/useAppController";
import type { DllStatus } from "../../types";
import { Card, CardHeader, EmptyState } from "../../shared/ui";

export function DllPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const disabled = state.activity !== "idle" || !state.steamDir;
  return <div className="stack-page">
    <Card className="section-intro">
      <div><span className="section-symbol"><ShieldAlert size={24} /></span><div><h2>DLL 组件管理</h2><p>通过 SHA-256 校验来源与目标文件，并检查 Steam 进程中的实际加载状态。</p></div></div>
      <div className="button-row"><button className="button primary" disabled={disabled || state.scan?.dll_resources_ready === false} onClick={actions.installDlls}><Download size={17} />安装或修复</button><button className="button danger-soft" disabled={disabled} onClick={actions.removeDlls}><Trash2 size={17} />安全移除</button><button className="button" disabled={disabled} onClick={actions.refreshAll}><RefreshCw size={17} />重新扫描</button></div>
    </Card>
    {!state.scan ? <Card><EmptyState title="尚未连接 Steam" description="选择 Steam 根目录后即可查看组件状态。" action={<button className="button primary" onClick={actions.chooseSteamDir}>选择目录</button>} /></Card> : <div className="dll-grid">
      {state.scan.dlls.map((dll) => <DllCard key={dll.name} dll={dll} />)}
    </div>}
    {state.scan && <Card><CardHeader icon={FileCheck2} title="校验说明" description="工具不会把文件存在等同于组件可用" /><div className="explain-grid"><p><b className="badge managed">正常</b><span>目标文件与应用内置资源哈希完全一致。</span></p><p><b className="badge foreign">不一致</b><span>文件存在，但并非当前版本提供的组件。</span></p><p><b className="badge missing">缺失</b><span>Steam 根目录中没有找到对应文件。</span></p></div></Card>}
  </div>;
}

function DllCard({ dll }: { dll: DllStatus }) {
  const stateLabel = dll.state === "Managed" ? "校验通过" : dll.state === "Foreign" ? "版本不一致" : "尚未安装";
  return <Card className={`dll-card ${dll.state.toLowerCase()}`}>
    <div className="dll-card-top"><span className="dll-symbol"><Box size={22} /></span><b className={`badge ${dll.state.toLowerCase()}`}>{stateLabel}</b></div>
    <h3>{dll.name}</h3><p className="mono path-clip" title={dll.target_path}>{dll.target_path}</p>
    <dl><div><dt>资源哈希</dt><dd className="mono">{shortHash(dll.resource_hash)}</dd></div><div><dt>目标哈希</dt><dd className="mono">{shortHash(dll.target_hash)}</dd></div><div><dt>Steam 加载</dt><dd className={dll.loaded_by_steam ? "positive" : ""}>{dll.loaded_by_steam ? "已加载" : dll.load_state === "SteamNotRunning" ? "Steam 未运行" : dll.load_state === "VerifyFailed" ? "无法验证" : "未加载"}</dd></div></dl>
  </Card>;
}
function shortHash(value?: string | null) { return value ? `${value.slice(0, 10)}…${value.slice(-6)}` : "—"; }
