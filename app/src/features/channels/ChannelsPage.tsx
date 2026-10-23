import { Box, FolderInput, FolderOpen, Layers, RefreshCw, ShieldAlert, Sparkles, Trash2 } from "lucide-react";
import type { AppController } from "../../app/useAppController";
import { RecentLogCard, SideAction } from "../../shared/panels";
import { Badge, Card, CardHead, Empty, Hash, KV, KVRow, Verdict } from "../../shared/ui";

const CHANNEL_TITLES: Record<string, { title: string; hint: string }> = {
  "pattern/steamui": { title: "steamui.dll 函数位置", hint: "RVA + 字节签名，决定 UI 侧钩子能否装上" },
  "pattern/steamclient": { title: "steamclient64.dll 函数位置", hint: "package / manifest / netpacket / IPC 入口" },
  "ipc/steamclient": { title: "IPC 线路常量", hint: "接口与方法 funcHash / fencepost" },
};

export function ChannelsPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const build = state.build;
  if (!build) {
    return <Empty title="尚未检测到构建信息" description="选择 Steam 根目录后，这里会显示三个数据通道与当前构建的匹配情况。" />;
  }

  const dllReady = build.dll.present;
  const ipcSupported = build.dll.has_ipc_channel;
  const busy = state.activity !== "idle";
  const installed = build.channels.filter((file) => file.present).length;
  const missing = build.channels.filter((file) => !file.present);
  const missingPattern = missing.find((file) => file.channel === "pattern");

  return (
    <div className="page-split">
      <div className="col-main">
        <Verdict
          tone={!dllReady ? "bad" : missing.length ? "warn" : "ok"}
          text={
            !dllReady
              ? <>OpenSteamTool.dll 不在 Steam 根目录；通道装了也不会被读取，先到「组件」页安装</>
              : missing.length
                ? <>{missing.length} 个通道缺失（{missing.map((file) => `${file.channel}/${file.component}`).join("、")}）；补齐后重启 Steam 生效</>
                : <>三个通道与当前构建匹配，重启 Steam 生效</>
          }
          action={
            <button
              className="btn btn-sm"
              onClick={() => (dllReady ? void actions.refreshBuild() : actions.setPage("dlls"))}
            >
              {dllReady ? <><RefreshCw size={11} />重新体检</> : <><Layers size={11} />去组件页</>}
            </button>
          }
        />

        <Card>
          <CardHead
            title="当前构建"
            sub="DLL 与当前 Steam 构建的对应关系"
            accent={dllReady}
            right={<Badge tone={dllReady ? "ok" : "bad"}>{dllReady ? "DLL 在位" : "DLL 缺失"}</Badge>}
          />
          <KV>
            <KVRow label="Steam 客户端"><code>{build.steam_version ?? "未知"}</code></KVRow>
            <KVRow label="steamui.dll"><Hash value={build.steamui_sha256} truncate={false} /></KVRow>
            <KVRow label="steamclient64"><Hash value={build.steamclient_sha256} truncate={false} /></KVRow>
            <KVRow label="OpenSteamTool">
              {build.dll.present
                ? <><Hash value={build.dll.sha256} truncate={false} /><span className="dim"> · {(build.dll.size / 1048576).toFixed(2)} MB</span></>
                : <span className="faint">缺失</span>}
            </KVRow>
          </KV>
          {!dllReady && (
            <p className="inline-warning"><ShieldAlert size={13} />OpenSteamTool.dll 不在 Steam 根目录，数据通道装了也不会被读取。</p>
          )}
          {dllReady && !ipcSupported && (
            <p className="inline-warning"><ShieldAlert size={13} />这份 DLL 不支持 ipc 通道，装好的 ipc 数据会被忽略。</p>
          )}
        </Card>

        <Card>
          <CardHead title="通道文件" sub={`${installed}/${build.channels.length} 已安装`} />
          <table className="table">
            <thead>
              <tr>
                <th>通道</th>
                <th>作用</th>
                <th>状态</th>
                <th className="num">条目</th>
                <th className="num">大小</th>
                <th>期望文件名</th>
                <th className="actions">操作</th>
              </tr>
            </thead>
            <tbody>
              {build.channels.map((file) => {
                const meta = CHANNEL_TITLES[`${file.channel}/${file.component}`] ?? { title: file.component, hint: "" };
                return (
                  <tr key={`${file.channel}-${file.component}`}>
                    <td className="strong nowrap"><code>{file.channel}/{file.component}</code></td>
                    <td><span title={file.path}>{meta.title}</span></td>
                    <td><Badge tone={file.present ? "ok" : "warn"}>{file.present ? file.entry_label : "缺失"}</Badge></td>
                    <td className="num">{file.entries || "—"}</td>
                    <td className="num">{file.present ? `${file.bytes} B` : "—"}</td>
                    <td><code>{file.expected_name ? `${file.expected_name.slice(0, 12)}…toml` : "（缺少 DLL）"}</code></td>
                    <td className="actions">
                      <div className="btn-row end">
                        <button
                          className="btn btn-sm"
                          disabled={busy || file.channel !== "pattern"}
                          title={file.channel === "pattern" ? "按当前 Steam 构建在应用内生成" : "ipc 通道需要上游 IPCMessages.steamd"}
                          onClick={() => void actions.generateChannel(file.channel, file.component)}
                        >
                          <Sparkles size={11} />生成
                        </button>
                        <button className="btn btn-sm" disabled={busy} onClick={() => void actions.installChannel(file.channel, file.component)}>
                          <FolderInput size={11} />{file.present ? "替换" : "安装"}
                        </button>
                        <button className="icon-btn xs" disabled={!file.present || busy} onClick={() => void actions.removeChannel(file.channel, file.component)} title="移除">
                          <Trash2 size={11} />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <p className="note row">
          <span>路径：</span>
          <code className="clip" title={build.channels[0]?.path}>{build.channels[0]?.path ?? "—"}</code>
          <span className="tail">安装后重启 Steam 生效</span>
        </p>
        </Card>
      </div>

      <div className="col-side">
        <Card>
          <CardHead title="下一步" sub="按当前状态排序" />
          <div className="ov-actions">
            {!dllReady && (
              <SideAction icon={<Layers size={12} />} title="先装 DLL" sub="通道依赖 OpenSteamTool.dll" onClick={() => actions.setPage("dlls")} />
            )}
            {missing.length > 0 && (
              <SideAction
                icon={<Sparkles size={12} />}
                title={missingPattern ? "生成缺失通道" : "安装通道文件"}
                sub={missingPattern ? `${missing.length} 个待补齐` : "从本地选择 TOML"}
                onClick={() => missingPattern ? void actions.generateChannel(missingPattern.channel, missingPattern.component) : void actions.installChannel("pattern", "steamui")}
              />
            )}
            {dllReady && (
              <SideAction icon={<Box size={12} />} title="查看组件" sub="DLL 在位与加载状态" onClick={() => actions.setPage("dlls")} />
            )}
            <SideAction icon={<FolderOpen size={12} />} title="打开日志" sub="看通道是否被真正读取" onClick={() => actions.setPage("logs")} />
          </div>
        </Card>

        <RecentLogCard logs={state.logs} onOpen={() => actions.setPage("logs")} />
      </div>
    </div>
  );
}
