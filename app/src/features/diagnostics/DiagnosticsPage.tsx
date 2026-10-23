import { AlertTriangle, RefreshCw, Stethoscope } from "lucide-react";
import type { AppController } from "../../app/useAppController";
import type { CrashDump } from "../../types";
import { RecentLogCard } from "../../shared/panels";
import { Badge, Card, CardHead, Empty, KV, KVRow, Verdict } from "../../shared/ui";

export function DiagnosticsPage({ controller }: { controller: AppController }) {
  const { state, actions } = controller;
  const build = state.build;
  if (!build) {
    return <Empty title="尚无诊断数据" description="连接 Steam 目录后可以查看构建、崩溃转储与配置冲突。" />;
  }

  const missing = collectMissingEntries(state.logs);
  const suppressed = countMatches(state.logs, /exception 0x[0-9A-Fa-f]+ suppressed/g);
  const legacyMirror = build.toml.legacy_pattern_mirror && !build.toml.remote_url_template ? build.toml.legacy_pattern_mirror : "";
  const [layoutOffset, layoutSize] = build.dll.layout_signature?.split(":") ?? [];
  const crash = summarizeCrashes(build.crash_dumps);

  return (
    <div className="page-split">
      <div className="col-main">
        <Verdict
          tone={crash ? (crash.ours ? "warn" : "flat") : "ok"}
          text={crash
            ? <>{crash.total} 份崩溃中 {crash.count} 份落在 <code>{crash.module}</code>{crash.offset ? <>，偏移集中在 <code>+0x{crash.offset.toString(16).toUpperCase()}</code></> : null}</>
            : "没有崩溃转储；DLL 结构与当前构建匹配"}
          action={
            <button className="btn btn-sm" onClick={() => actions.setPage("logs")}>查看日志</button>
          }
        />

        <Card>
          <CardHead
            title="构建与 DLL"
            sub="DLL 自带的结构布局标记，决定这份构建对应哪套 CNetPacket 偏移"
            accent={Boolean(build.dll.layout_signature)}
            right={<Badge tone={build.dll.present ? "ok" : "bad"}>{build.dll.present ? "DLL 在位" : "缺失"}</Badge>}
          />
          <KV>
            <KVRow label="结构布局"><code>{layoutOffset ? `CNetPacket +0x${layoutOffset} / size +0x${layoutSize}` : "未识别（旧构建）"}</code></KVRow>
            <KVRow label="DLL SHA-256"><code>{build.dll.present ? build.dll.sha256 : "缺失"}</code></KVRow>
            <KVRow label="ipc 通道"><Badge tone={build.dll.has_ipc_channel ? "ok" : "warn"}>{build.dll.has_ipc_channel ? "支持" : "不支持"}</Badge></KVRow>
            <KVRow label="缺条目静默"><Badge tone={build.dll.has_suppress_support ? "ok" : "warn"}>{build.dll.has_suppress_support ? "支持" : "不支持"}</Badge></KVRow>
          </KV>
          {!build.dll.layout_signature && build.dll.present && (
            <p className="inline-warning"><AlertTriangle size={13} />未找到布局标记：该 DLL 可能仍是硬编码偏移的旧构建，RecvPkt 侧会反复抛异常。</p>
          )}
        </Card>

        <Card>
          <CardHead
            title="崩溃转储"
            sub="读取 Steam\dumps 下的 minidump；偏移列悬停看完整结论"
            right={<Badge tone={build.crash_dumps.length ? "warn" : "ok"}>{build.crash_dumps.length} 份</Badge>}
          />
          {build.crash_dumps.length === 0 ? (
            <p className="note">没有崩溃记录。</p>
          ) : (
            <table className="table">
              <thead>
                <tr><th>文件</th><th className="num">异常</th><th>故障模块</th><th>偏移</th></tr>
              </thead>
              <tbody>
                {build.crash_dumps.map((dump) => <CrashRow key={dump.path} dump={dump} />)}
              </tbody>
            </table>
          )}
        </Card>
      </div>

      <div className="col-side">
        <Card>
          <CardHead
            title="待处理"
            sub="缺条目 / 旧键 / 被抑制的异常"
            right={<Badge tone={missing.length || suppressed ? "warn" : "ok"}>{missing.length + (suppressed ? 1 : 0)}</Badge>}
          />
          {missing.length > 0
            ? <ul className="tag-list">{missing.map((name) => <li key={name}><code>{name}</code></li>)}</ul>
            : <p className="note">没有缺失条目。</p>}
          <p className="note">
            {suppressed > 0
              ? `本次会话已抑制 ${suppressed} 次访问冲突；集中在 RecvPkt 即布局偏移过期。`
              : "没有需要抑制的异常。"}
          </p>
          {crash && (
            <p className="note">
              {crash.ours
                ? "归因落在 OpenSteamTool.dll：钩子内部异常，重编前先确认 pattern 是最新的。"
                : "归因在 Steam 侧：多为布局偏移过期，重新观测后跑 cli layout 再重编 DLL。"}
            </p>
          )}
          {legacyMirror && (
            <p className="inline-warning"><AlertTriangle size={13} />检测到旧键 <code>[pattern] mirror</code>（DLL 不读它）。到「设置」改用 <code>[remote] url_template</code>。</p>
          )}
          <div className="btn-row spaced">
            <button className="btn btn-sm" data-kbd="Ctrl R" onClick={actions.refreshBuild}>
              <RefreshCw size={11} />重新体检
            </button>
            <button className="btn btn-ghost btn-sm" onClick={() => actions.setPage("settings")}>
              <Stethoscope size={11} />去设置
            </button>
          </div>
        </Card>

        <RecentLogCard logs={state.logs} onOpen={() => actions.setPage("logs")} />
      </div>
    </div>
  );
}

function CrashRow({ dump }: { dump: CrashDump }) {
  const module = dump.faulting_module?.split("\\").pop() ?? "未知模块";
  const ours = /OpenSteamTool/i.test(module);
  const offset = dump.faulting_offset != null ? `+0x${dump.faulting_offset.toString(16).toUpperCase()}` : "—";
  return (
    <tr>
      <td><code>{dump.name}</code></td>
      <td className="num"><code>{dump.exception_code != null ? `0x${dump.exception_code.toString(16).toUpperCase()}` : "—"}</code></td>
      <td><code>{module}</code></td>
      <td>
        <span className={`crash-cell ${ours ? "warn" : ""}`} title={dump.verdict}>
          <span className="c-dot" />
          <code>{offset}</code>
        </span>
      </td>
    </tr>
  );
}

/** 把「N 份崩溃」压成一句归因：最常出现的故障模块 + 该模块里最常见的偏移。 */
function summarizeCrashes(dumps: CrashDump[]) {
  if (!dumps.length) return null;
  const byModule = new Map<string, CrashDump[]>();
  for (const dump of dumps) {
    const module = dump.faulting_module?.split("\\").pop() ?? "未知模块";
    byModule.set(module, [...(byModule.get(module) ?? []), dump]);
  }
  const [module, group] = [...byModule.entries()].sort((a, b) => b[1].length - a[1].length)[0];
  const offsets = group.map((dump) => dump.faulting_offset).filter((value): value is number => value != null);
  const counts = new Map<number, number>();
  for (const offset of offsets) counts.set(offset, (counts.get(offset) ?? 0) + 1);
  const offset = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0];
  return { module, count: group.length, total: dumps.length, offset, ours: /OpenSteamTool/i.test(module) };
}

function collectMissingEntries(logs: Array<{ content: string }>): string[] {
  const names = new Set<string>();
  for (const log of logs) {
    for (const match of log.content.matchAll(/no entry for '([^']+)'/g)) names.add(match[1]);
  }
  return [...names].sort();
}

function countMatches(logs: Array<{ content: string }>, pattern: RegExp): number {
  let total = 0;
  for (const log of logs) total += [...log.content.matchAll(pattern)].length;
  return total;
}
