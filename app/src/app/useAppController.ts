import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  closeSteam, deleteGame, detectSteamDir, depotOwnership, discoverDepots, importLuaFiles, inspectBuild,
  generateAllChannels, generateChannelFile, installChannelFile, installDlls, listGames, loadSettings, openLuaDir, pickSteamDir,
  pickTomlFile, readLogs, removeChannelFile, removeDlls, restartSteam,
  saveSettings, scanState, setGameEnabled, upsertGame,
  installManifestLua as installManifestLuaApi,
  removeManifestLua as removeManifestLuaApi,
  manifestLuaStatus,
  probeManifestChannel,
  workshopApps, resolveWorkshopItem, downloadWorkshopItem, refreshWorkshopKeys, onWorkshopProgress,
} from "../api";
import type { BuildInspection, GameConfig, ManagerSettings, ManifestLuaStatus, WorkshopDownloadReport, WorkshopItem, WorkshopProgress } from "../types";
import { defaultSettings, emptyGame, errorMessage, normalizeGame, type ConfirmRequest, type NoticeTone, type PageId } from "./model";

const STEAM_DIR_KEY = "gost.steamDirectory";

export function useAppController() {
  const [page, setPage] = useState<PageId>("overview");
  const [steamDir, setSteamDir] = useState("");
  const [scan, setScan] = useState<Awaited<ReturnType<typeof scanState>> | null>(null);
  const [build, setBuild] = useState<BuildInspection | null>(null);
  const [manifestLua, setManifestLua] = useState<ManifestLuaStatus | null>(null);
  const [settings, setSettings] = useState<ManagerSettings>(defaultSettings);
  const [games, setGames] = useState<GameConfig[]>([]);
  const [logs, setLogs] = useState<Awaited<ReturnType<typeof readLogs>>>([]);
  const [selectedLog, setSelectedLog] = useState("");
  const [selectedGame, setSelectedGame] = useState<GameConfig>(emptyGame);
  const [workshopItem, setWorkshopItem] = useState<WorkshopItem | null>(null);
  const [workshopProgress, setWorkshopProgress] = useState<WorkshopProgress | null>(null);
  const [workshopReport, setWorkshopReport] = useState<WorkshopDownloadReport | null>(null);
  const [workshopTaskAppid, setWorkshopTaskAppid] = useState<number | null>(null);
  const [activity, setActivity] = useState<"idle" | "loading" | "working">("loading");
  const [notice, setNotice] = useState({ tone: "neutral" as NoticeTone, message: "正在检测 Steam 环境…", time: now() });
  const [confirm, setConfirm] = useState<ConfirmRequest | null>(null);
  const confirmResolver = useRef<((ok: boolean) => void) | null>(null);
  const bootStarted = useRef(false);
  const gamesHydrated = useRef(false);

  const notify = useCallback((tone: NoticeTone, message: string) => setNotice({ tone, message, time: now() }), []);

  const askConfirm = useCallback((request: ConfirmRequest) => new Promise<boolean>((resolve) => {
    confirmResolver.current = resolve;
    setConfirm(request);
  }), []);

  const resolveConfirm = useCallback((ok: boolean) => {
    setConfirm(null);
    const resolve = confirmResolver.current;
    confirmResolver.current = null;
    resolve?.(ok);
  }, []);

  // 进度挂到 controller：工坊页被切走/切换游戏后回来仍能看到正在进行的下载。
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    void onWorkshopProgress((payload) => {
      if (!alive) return;
      setWorkshopProgress(payload);
      if (payload.phase === "done") {
        window.setTimeout(() => {
          if (alive) setWorkshopProgress((current) => (current?.phase === "done" ? null : current));
        }, 900);
      }
    }).then((fn) => {
      if (alive) unlisten = fn;
      else fn();
    }).catch(() => undefined);
    return () => { alive = false; unlisten?.(); };
  }, []);

  const refreshScan = useCallback(async (dir = steamDir) => {
    if (dir) setScan(await scanState(dir));
  }, [steamDir]);
  const refreshGames = useCallback(async (dir = steamDir) => {
    if (!dir) return;
    const next = await listGames(dir);
    setGames(next);
    pickFirstGame(next);
  }, [steamDir]);
  const refreshLogs = useCallback(async (dir = steamDir) => {
    if (!dir) return;
    const next = await readLogs(dir);
    setLogs(next);
    setSelectedLog((current) => next.some((item) => item.name === current) ? current : next[0]?.name ?? "");
  }, [steamDir]);
  const refreshBuild = useCallback(async (dir = steamDir) => {
    if (!dir) return;
    setBuild(await inspectBuild(dir));
  }, [steamDir]);
  const refreshAll = useCallback(async (dir = steamDir) => {
    if (!dir) return;
    const [nextScan, nextSettings, nextGames, nextLogs, nextBuild, nextManifestLua] = await Promise.all([
      scanState(dir), loadSettings(dir), listGames(dir), readLogs(dir), inspectBuild(dir), manifestLuaStatus(dir),
    ]);
    setScan(nextScan); setSettings(nextSettings); setGames(nextGames); setLogs(nextLogs); setBuild(nextBuild);
    setManifestLua(nextManifestLua);
    pickFirstGame(nextGames);
    setSelectedLog((current) => nextLogs.some((item) => item.name === current) ? current : nextLogs[0]?.name ?? "");
  }, [steamDir]);

  /** 首次载入配置库时选中第一条，避免编辑区空着；用户主动「新建」不会被抢回来。 */
  const pickFirstGame = useCallback((next: GameConfig[]) => {
    if (gamesHydrated.current) return;
    gamesHydrated.current = true;
    setSelectedGame((current) => (current.appid === 0 && next.length ? normalizeGame(next[0]) : current));
  }, []);

  useEffect(() => {
    // StrictMode 下 effect 会挂载/卸载各跑一次；用 ref 保证只真正启动一次，
    // 且不因「第一次的清理函数」把已经发出的一次性启动结果丢掉。
    if (bootStarted.current) return;
    bootStarted.current = true;
    void (async () => {
      try {
        const saved = localStorage.getItem(STEAM_DIR_KEY)?.trim();
        if (saved) {
          try {
            await refreshAll(saved);
            setSteamDir(saved);
            notify("success", "已恢复上次使用的 Steam 目录");
            return;
          } catch {
            localStorage.removeItem(STEAM_DIR_KEY);
          }
        }
        const detected = await detectSteamDir();
        if (detected) {
          await refreshAll(detected);
          setSteamDir(detected);
          localStorage.setItem(STEAM_DIR_KEY, detected);
          notify("success", "Steam 环境已就绪");
        } else notify("neutral", "请选择 Steam 根目录开始使用");
      } catch (error) {
        notify("error", errorMessage(error));
      } finally {
        setActivity("idle");
      }
    })();
  }, [notify, refreshAll]);

  useEffect(() => {
    if (!steamDir || activity !== "idle") return;
    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") void refreshScan(steamDir).catch(() => undefined);
    }, 8000);
    return () => window.clearInterval(timer);
  }, [activity, refreshScan, steamDir]);

  const run = useCallback(async (
    task: () => Promise<void>, pending: string, success: string,
    refresh: Array<"scan" | "games" | "logs" | "all"> = [],
  ) => {
    setActivity("working"); notify("working", pending);
    try {
      await task();
      for (const target of refresh) {
        if (target === "all") await refreshAll();
        else if (target === "scan") await refreshScan();
        else if (target === "games") await refreshGames();
        else await refreshLogs();
      }
      notify("success", success);
    } catch (error) { notify("error", errorMessage(error)); throw error; }
    finally { setActivity("idle"); }
  }, [notify, refreshAll, refreshGames, refreshLogs, refreshScan]);

  const actions = useMemo(() => ({
    setPage, setSettings, setSelectedLog, resolveConfirm,
    setSelectedGame: (game: GameConfig) => setSelectedGame(normalizeGame(game)),
    newGame: () => setSelectedGame(emptyGame),
    chooseSteamDir: async () => {
      const picked = await pickSteamDir(); if (!picked) return;
      setActivity("loading");
      try {
        await refreshAll(picked);
        setSteamDir(picked);
        localStorage.setItem(STEAM_DIR_KEY, picked);
        notify("success", "Steam 目录已保存，下次启动将自动恢复");
      }
      catch (error) { notify("error", errorMessage(error)); }
      finally { setActivity("idle"); }
    },
    refreshAll: () => run(async () => undefined, "正在刷新全部状态…", "状态已刷新", ["all"]).catch(() => undefined),
    initialize: async () => {
      setActivity("working");
      try {
        const before = await scanState(steamDir);
        const wasRunning = before.steam_running;
        if (wasRunning) {
          const proceed = await askConfirm({
            title: "一键初始化",
            message: "初始化会更新 Steam 目录中的组件和通道数据。需要先完全退出 Steam，完成后会自动重新启动 Steam。是否继续？",
            okLabel: "关闭 Steam 并继续",
            cancelLabel: "取消",
            tone: "warning",
          });
          if (!proceed) {
            notify("neutral", "已取消初始化");
            return;
          }
          notify("working", "正在关闭 Steam…");
          await closeSteam();
        }

        const failures: string[] = [];
        let report: Awaited<ReturnType<typeof generateAllChannels>> | null = null;
        notify("working", "正在安装组件、创建配置并生成通道数据…");

        try {
          await installDlls(steamDir);
        } catch (error) {
          failures.push(`组件未更新：${errorMessage(error)}`);
        }
        if (!failures.length) {
          try {
            await saveSettings(steamDir, defaultSettings);
          } catch (error) {
            failures.push(`配置未写入：${errorMessage(error)}`);
          }
        }
        if (!failures.length) {
          try {
            report = await generateAllChannels(steamDir, localStorage.getItem("gost.idl"));
          } catch (error) {
            failures.push(`通道未生成：${errorMessage(error)}`);
          }
        }

        let restarted = false;
        if (wasRunning) {
          notify("working", "正在重新启动 Steam…");
          try {
            await restartSteam(steamDir);
            restarted = true;
          } catch (error) {
            failures.push(`Steam 启动失败：${errorMessage(error)}`);
          }
        }

        try {
          await refreshAll();
        } catch (error) {
          failures.push(`状态刷新失败：${errorMessage(error)}`);
        }

        if (failures.length) {
          const suffix = wasRunning ? (restarted ? "；Steam 已重新启动" : "") : "；Steam 未启动";
          notify("error", `初始化未完成：${failures.join("；")}${suffix}`);
        } else {
          const done = report?.files.map((file) => `${file.channel}/${file.component}`).join("、") ?? "";
          const skipped = report?.skipped.length ? `；跳过 ${report.skipped.join("，")}` : "";
          const suffix = wasRunning ? (restarted ? "；Steam 已重新启动" : "") : "；启动 Steam 后生效";
          notify(report?.skipped.length ? "neutral" : "success", `初始化完成：已生成 ${done || "0 个通道"}${skipped}${suffix}`);
        }
      } catch (error) {
        notify("error", errorMessage(error));
      } finally {
        setActivity("idle");
      }
    },
    generateAll: async () => {
      setActivity("working");
      notify("working", "正在为当前 Steam 构建生成全部通道…");
      try {
        const report = await generateAllChannels(steamDir, localStorage.getItem("gost.idl"));
        await refreshAll();
        const skipped = report.skipped.length ? `；跳过 ${report.skipped.join("，")}` : "";
        notify(report.skipped.length ? "neutral" : "success", `已生成 ${report.files.length} 个通道${skipped}；重启 Steam 生效`);
      } catch (error) {
        notify("error", errorMessage(error));
      } finally {
        setActivity("idle");
      }
    },
    installDlls: () => run(() => installDlls(steamDir), "正在安装 DLL…", "DLL 安装完成", ["scan"]).catch(() => undefined),
    removeDlls: () => run(() => removeDlls(steamDir), "正在安全移除 DLL…", "DLL 已移除", ["scan"]).catch(() => undefined),
    closeSteam: () => run(closeSteam, "正在关闭 Steam…", "Steam 已关闭", ["scan"]).catch(() => undefined),
    restartSteam: () => run(() => restartSteam(steamDir), "正在重启 Steam…", "Steam 已重启", ["scan"]).catch(() => undefined),
    saveSettings: (next: ManagerSettings) => run(async () => { await saveSettings(steamDir, next); setSettings(next); }, "正在保存设置…", "设置已保存", ["all"]).catch(() => undefined),
    installManifestLua: async () => {
      setActivity("working");
      try {
        const status = await installManifestLuaApi(steamDir);
        setManifestLua(status);
        notify("success", "manifest.lua 已写入，重启 Steam 生效");
        return status;
      } catch (error) { notify("error", errorMessage(error)); return null; }
      finally { setActivity("idle"); }
    },
    removeManifestLua: async () => {
      setActivity("working");
      try {
        const status = await removeManifestLuaApi(steamDir);
        setManifestLua(status);
        notify("success", "manifest.lua 已移除");
        return status;
      } catch (error) { notify("error", errorMessage(error)); return null; }
      finally { setActivity("idle"); }
    },
    probeManifest: async (appid?: number | null) => {
      setActivity("working");
      notify("working", "正在测试 manifest 取码与 CDN…");
      try {
        const result = await probeManifestChannel(steamDir, appid ?? null);
        const usable = result.download?.status === 200;
        notify(usable ? "success" : "error", usable
          ? `下载通道可用：${result.download?.provider} → CDN ${result.download?.status}`
          : "下载通道不可用：没有源返回可用的 code，或 CDN 拒绝");
        return result;
      } catch (error) { notify("error", errorMessage(error)); return null; }
      finally { setActivity("idle"); }
    },
    saveGame: async (game: GameConfig) => {
      const normalized = normalizeGame(game);
      try { await run(() => upsertGame(steamDir, normalized), "正在生成 Lua 配置…", `G-${normalized.appid}.lua 已保存`, ["games", "scan"]); setSelectedGame(emptyGame); }
      catch { /* run already reported the failure */ }
    },
    deleteGame: (appid: number) => run(() => deleteGame(steamDir, appid), "正在删除 Lua 配置…", `G-${appid}.lua 已删除`, ["games", "scan"]).catch(() => undefined),
    setGameEnabled: (appid: number, enabled: boolean) => run(
      () => setGameEnabled(steamDir, appid, enabled),
      "正在切换配置状态…",
      enabled ? "配置已启用" : "配置已禁用",
      ["games", "scan"],
    ).catch(() => undefined),
    toggleGame: (game: GameConfig) => run(() => setGameEnabled(steamDir, game.appid, !game.enabled), "正在切换配置状态…", game.enabled ? "配置已禁用" : "配置已启用", ["games", "scan"]).catch(() => undefined),
    discoverDepots: async (appid: number) => {
      if (!appid) { notify("error", "请先输入有效 AppId"); return null; }
      setActivity("working"); notify("working", "正在读取本地 appinfo.vdf 与 config.vdf…");
      try {
        const result = await discoverDepots(steamDir, appid);
        const keyed = result.depots.filter((depot) => depot.depot_key).length;
        const ownedDlc = result.dlc.filter((dlc) => dlc.key_found).length;
        notify(
          "success",
          result.dlc.length
            ? `发现 ${result.depots.length} 个 Depot（${keyed} 个含本地 Key），${result.dlc.length} 个 DLC 中 ${ownedDlc} 个已有内容`
            : `发现 ${result.depots.length} 个 Depot，其中 ${keyed} 个包含本地 Key`,
        );
        return result;
      } catch (error) { notify("error", errorMessage(error)); return null; }
      finally { setActivity("idle"); }
    },
    // 静默的归属标注：给已保存配置的每一行标「主内容 / DLC」，失败就当没有。
    classifyDepots: (appid: number) =>
      appid ? depotOwnership(steamDir, appid).catch(() => null) : Promise.resolve(null),
    loadWorkshopApps: async (): Promise<import("../types").WorkshopApp[]> => {
      try {
        return await workshopApps(steamDir);
      } catch (error) {
        notify("error", errorMessage(error));
        return [];
      }
    },
    selectWorkshopItem: (next: WorkshopItem | null) => {
      setWorkshopItem(next);
      setWorkshopReport(null);
      setWorkshopProgress(null);
    },
    clearWorkshopItem: () => {
      setWorkshopItem(null);
      setWorkshopReport(null);
    },
    clearWorkshopSelection: () => {
      setWorkshopItem(null);
    },
    resolveWorkshop: async (appid: number | null, item: string): Promise<WorkshopItem | null> => {
      if (!item.trim()) { notify("error", "请先粘贴创意工坊链接或物品 ID"); return null; }
      setActivity("working");
      notify("working", "正在解析创意工坊物品…");
      try {
        const next = await resolveWorkshopItem(steamDir, appid, item);
        setWorkshopItem(next);
        setWorkshopReport(null);
        setWorkshopProgress(null);
        return next;
      } catch (error) {
        notify("error", errorMessage(error));
        return null;
      } finally {
        setActivity("idle");
      }
    },
    downloadWorkshop: async (
      appid: number | null, item: string, depotKey?: string | null, installMods = true,
    ): Promise<WorkshopDownloadReport | null> => {
      if (!item.trim()) { notify("error", "请先解析要下载的工坊物品"); return null; }
      setActivity("working");
      setWorkshopTaskAppid(appid);
      setWorkshopReport(null);
      setWorkshopProgress(null);
      notify("working", "正在下载工坊内容，请保持 Steam 关闭或不要启动游戏…");
      try {
        const report = await downloadWorkshopItem(steamDir, appid, item, depotKey, installMods);
        setWorkshopReport(report);
        setWorkshopItem(report.item);
        const mods = report.mods_dir ? `，并安装到游戏 Mods（${report.mods_files} 个文件）` : "";
        const warning = report.notes.length ? `；${report.notes[0]}` : "";
        notify(report.notes.length ? "neutral" : "success", `已下载 ${report.files} 个文件，${formatBytes(report.bytes)}${mods}${warning}`);
        return report;
      } catch (error) {
        setWorkshopProgress(null);
        notify("error", errorMessage(error));
        return null;
      } finally {
        setActivity("idle");
      }
    },
    refreshWorkshopKeyDb: async (): Promise<number | null> => {
      setActivity("working");
      notify("working", "正在更新创意工坊密钥库…");
      try {
        const count = await refreshWorkshopKeys(steamDir);
        notify("success", `密钥库已更新：${count} 个 Depot`);
        return count;
      } catch (error) {
        notify("error", errorMessage(error));
        return null;
      } finally {
        setActivity("idle");
      }
    },
    // 一次多选导入：单个文件解析失败只跳过它，其余照常写入并刷新配置库。
    importLua: async () => {
      setActivity("working");
      try {
        const report = await importLuaFiles(steamDir);
        if (!report) { notify("neutral", "已取消导入"); return; }
        const { imported, failed } = report;
        const reason = failed.length ? `${failed[0].file_name}：${failed[0].error}` : "";
        if (!imported.length) {
          notify("error", failed.length ? `导入失败（${reason}）` : "没有可导入的 Lua 文件");
          return;
        }
        await Promise.all([refreshGames(), refreshScan()]);
        setSelectedGame(normalizeGame(imported[0]));
        const suffix = failed.length
          ? `；${failed.length} 个失败（${reason}${failed.length > 1 ? " 等" : ""}）`
          : "";
        notify(
          failed.length ? "neutral" : "success",
          imported.length === 1
            ? `G-${imported[0].appid}.lua 已导入${suffix}`
            : `已导入 ${imported.length} 个配置${suffix}`,
        );
      } catch (error) { notify("error", errorMessage(error)); }
      finally { setActivity("idle"); }
    },
    openLuaDir: () => openLuaDir(steamDir).catch((error) => notify("error", errorMessage(error))),
    refreshLogs: () => run(async () => undefined, "正在读取日志…", "日志已刷新", ["logs"]).catch(() => undefined),
    refreshBuild: () => run(async () => undefined, "正在检查构建与数据通道…", "体检完成", ["all"]).catch(() => undefined),
    installChannel: async (channel: string, component: string) => {
      const picked = await pickTomlFile();
      if (!picked) { notify("neutral", "已取消选择数据文件"); return; }
      await run(
        async () => { await installChannelFile(steamDir, channel, component, picked); },
        `正在安装 ${channel}/${component} 数据…`,
        `${channel}/${component} 已安装，重启 Steam 生效`,
        ["all"],
      ).catch(() => undefined);
    },
    generateChannel: async (channel: string, component: string) => {
      // ipc 默认用内置的上游 IDL；只有用户在设置里指定过才走覆盖文件。
      const idl = channel === "ipc" ? localStorage.getItem("gost.idl") : null;
      await run(
        async () => { await generateChannelFile(steamDir, channel, component, idl); },
        channel === "ipc" ? "正在从 IDL 与 DLL 提取 ipc 常量…" : "正在按当前 Steam 构建生成通道数据…",
        `${channel}/${component} 已生成，重启 Steam 生效`,
        ["all"],
      ).catch(() => undefined);
    },    removeChannel: (channel: string, component: string) => run(
      async () => { await removeChannelFile(steamDir, channel, component); },
      "正在移除通道数据…",
      `${channel}/${component} 已移除`,
      ["all"],
    ).catch(() => undefined),
  }), [notify, refreshAll, refreshGames, refreshScan, resolveConfirm, run, steamDir]);

  return {
    state: {
      page, steamDir, scan, build, settings, games, logs, selectedLog, selectedGame, activity, notice, manifestLua, confirm,
      workshopItem, workshopProgress, workshopReport, workshopTaskAppid,
    },
    actions,
  };
}

export type AppController = ReturnType<typeof useAppController>;
function now() { return new Date().toLocaleTimeString("zh-CN", { hour12: false }); }

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
