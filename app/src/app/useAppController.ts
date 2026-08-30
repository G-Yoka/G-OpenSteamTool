import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  closeSteam, deleteGame, detectSteamDir, discoverDepots, importLuaFile, installDlls,
  listGames, loadSettings, openLuaDir, pickSteamDir, readLogs, removeDlls, restartSteam,
  saveSettings, scanState, setGameEnabled, upsertGame,
} from "../api";
import type { GameConfig, ManagerSettings } from "../types";
import { defaultSettings, emptyGame, errorMessage, normalizeGame, type NoticeTone, type PageId } from "./model";

const STEAM_DIR_KEY = "gost.steamDirectory";

export function useAppController() {
  const [page, setPage] = useState<PageId>("overview");
  const [steamDir, setSteamDir] = useState("");
  const [scan, setScan] = useState<Awaited<ReturnType<typeof scanState>> | null>(null);
  const [settings, setSettings] = useState<ManagerSettings>(defaultSettings);
  const [games, setGames] = useState<GameConfig[]>([]);
  const [logs, setLogs] = useState<Awaited<ReturnType<typeof readLogs>>>([]);
  const [selectedLog, setSelectedLog] = useState("");
  const [selectedGame, setSelectedGame] = useState<GameConfig>(emptyGame);
  const [activity, setActivity] = useState<"idle" | "loading" | "working">("loading");
  const [notice, setNotice] = useState({ tone: "neutral" as NoticeTone, message: "正在检测 Steam 环境…", time: now() });
  const bootStarted = useRef(false);

  const notify = useCallback((tone: NoticeTone, message: string) => setNotice({ tone, message, time: now() }), []);

  const refreshScan = useCallback(async (dir = steamDir) => {
    if (dir) setScan(await scanState(dir));
  }, [steamDir]);
  const refreshGames = useCallback(async (dir = steamDir) => {
    if (dir) setGames(await listGames(dir));
  }, [steamDir]);
  const refreshLogs = useCallback(async (dir = steamDir) => {
    if (!dir) return;
    const next = await readLogs(dir);
    setLogs(next);
    setSelectedLog((current) => next.some((item) => item.name === current) ? current : next[0]?.name ?? "");
  }, [steamDir]);
  const refreshAll = useCallback(async (dir = steamDir) => {
    if (!dir) return;
    const [nextScan, nextSettings, nextGames, nextLogs] = await Promise.all([
      scanState(dir), loadSettings(dir), listGames(dir), readLogs(dir),
    ]);
    setScan(nextScan); setSettings(nextSettings); setGames(nextGames); setLogs(nextLogs);
    setSelectedLog((current) => nextLogs.some((item) => item.name === current) ? current : nextLogs[0]?.name ?? "");
  }, [steamDir]);

  useEffect(() => {
    if (bootStarted.current) return;
    bootStarted.current = true;
    let active = true;
    void (async () => {
      try {
        const saved = localStorage.getItem(STEAM_DIR_KEY)?.trim();
        if (saved) {
          try {
            await refreshAll(saved);
            if (!active) return;
            setSteamDir(saved);
            notify("success", "已恢复上次使用的 Steam 目录");
            return;
          } catch {
            localStorage.removeItem(STEAM_DIR_KEY);
          }
        }
        const detected = await detectSteamDir();
        if (!active) return;
        if (detected) {
          await refreshAll(detected);
          if (!active) return;
          setSteamDir(detected);
          localStorage.setItem(STEAM_DIR_KEY, detected);
          notify("success", "Steam 环境已就绪");
        } else notify("neutral", "请选择 Steam 根目录开始使用");
      } catch (error) { notify("error", errorMessage(error)); }
      finally { if (active) setActivity("idle"); }
    })();
    return () => { active = false; };
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
    setPage, setSettings, setSelectedLog,
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
    initialize: () => run(async () => { await installDlls(steamDir); await saveSettings(steamDir, defaultSettings); }, "正在安装组件并创建配置…", "初始化完成", ["all"]).catch(() => undefined),
    installDlls: () => run(() => installDlls(steamDir), "正在安装 DLL…", "DLL 安装完成", ["scan"]).catch(() => undefined),
    removeDlls: () => run(() => removeDlls(steamDir), "正在安全移除 DLL…", "DLL 已移除", ["scan"]).catch(() => undefined),
    closeSteam: () => run(closeSteam, "正在关闭 Steam…", "Steam 已关闭", ["scan"]).catch(() => undefined),
    restartSteam: () => run(() => restartSteam(steamDir), "正在重启 Steam…", "Steam 已重启", ["scan"]).catch(() => undefined),
    saveSettings: (next: ManagerSettings) => run(async () => { await saveSettings(steamDir, next); setSettings(next); }, "正在保存设置…", "设置已保存", ["scan"]).catch(() => undefined),
    saveGame: async (game: GameConfig) => {
      const normalized = normalizeGame(game);
      try { await run(() => upsertGame(steamDir, normalized), "正在生成 Lua 配置…", `G-${normalized.appid}.lua 已保存`, ["games", "scan"]); setSelectedGame(emptyGame); }
      catch { /* run already reported the failure */ }
    },
    deleteGame: (appid: number) => run(() => deleteGame(steamDir, appid), "正在删除 Lua 配置…", `G-${appid}.lua 已删除`, ["games", "scan"]).catch(() => undefined),
    toggleGame: (game: GameConfig) => run(() => setGameEnabled(steamDir, game.appid, !game.enabled), "正在切换配置状态…", game.enabled ? "配置已禁用" : "配置已启用", ["games", "scan"]).catch(() => undefined),
    discoverDepots: async (appid: number) => {
      if (!appid) { notify("error", "请先输入有效 AppId"); return null; }
      setActivity("working"); notify("working", "正在读取本地 appinfo.vdf 与 config.vdf…");
      try {
        const result = await discoverDepots(steamDir, appid);
        const keyed = result.depots.filter((depot) => depot.depot_key).length;
        notify("success", `发现 ${result.depots.length} 个 Depot，其中 ${keyed} 个包含本地 Key`);
        return result;
      } catch (error) { notify("error", errorMessage(error)); return null; }
      finally { setActivity("idle"); }
    },
    importLua: async () => {
      setActivity("working");
      try {
        const game = await importLuaFile(steamDir);
        if (!game) { notify("neutral", "已取消导入"); return; }
        await Promise.all([refreshGames(), refreshScan()]); setSelectedGame(normalizeGame(game)); notify("success", `G-${game.appid}.lua 已导入`);
      } catch (error) { notify("error", errorMessage(error)); }
      finally { setActivity("idle"); }
    },
    openLuaDir: () => openLuaDir(steamDir).catch((error) => notify("error", errorMessage(error))),
    refreshLogs: () => run(async () => undefined, "正在读取日志…", "日志已刷新", ["logs"]).catch(() => undefined),
  }), [notify, refreshAll, refreshGames, refreshScan, run, steamDir]);

  return { state: { page, steamDir, scan, settings, games, logs, selectedLog, selectedGame, activity, notice }, actions };
}

export type AppController = ReturnType<typeof useAppController>;
function now() { return new Date().toLocaleTimeString("zh-CN", { hour12: false }); }
