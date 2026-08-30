import type { GameConfig, LogFile, ManagerSettings, ScanState } from "../types";

export type PageId = "overview" | "games" | "dlls" | "settings" | "logs" | "about";
export type ActivityState = "idle" | "loading" | "working";
export type NoticeTone = "neutral" | "working" | "success" | "error";

export type Notice = { tone: NoticeTone; message: string; time: string };

export type AppViewState = {
  page: PageId;
  steamDir: string;
  scan: ScanState | null;
  settings: ManagerSettings;
  games: GameConfig[];
  logs: LogFile[];
  selectedLog: string;
  selectedGame: GameConfig;
  activity: ActivityState;
  notice: Notice;
};

export const defaultSettings: ManagerSettings = {
  log_level: "info",
  manifest_url: "wudrm",
  timeout_resolve_ms: 5000,
  timeout_connect_ms: 5000,
  timeout_send_ms: 10000,
  timeout_recv_ms: 10000,
  lua_paths: [],
  pattern_mirror: "",
};

export const emptyGame: GameConfig = {
  appid: 0,
  name: "",
  enabled: true,
  depot_key: "",
  access_token: "",
  manifest_gid: "",
  app_ticket_hex: "",
  e_ticket_hex: "",
  stat_steam_id: "",
  appid_entries: [],
  manifest_entries: [],
};

export function normalizeGame(game: GameConfig): GameConfig {
  const appid = Number(game.appid) || 0;
  return {
    appid,
    name: game.name?.trim() ?? "",
    enabled: game.enabled ?? true,
    depot_key: game.depot_key?.trim() ?? "",
    access_token: game.access_token?.trim() ?? "",
    manifest_gid: game.manifest_gid?.trim() ?? "",
    app_ticket_hex: game.app_ticket_hex?.trim() ?? "",
    e_ticket_hex: game.e_ticket_hex?.trim() ?? "",
    stat_steam_id: game.stat_steam_id?.trim() ?? "",
    appid_entries: (game.appid_entries ?? []).map((entry) => ({
      appid: Number(entry.appid) || 0,
      unlock_flag: entry.unlock_flag == null ? null : Number(entry.unlock_flag),
      depot_key: entry.depot_key?.trim() ?? "",
    })),
    manifest_entries: (game.manifest_entries ?? []).map((entry) => ({
      depot_id: Number(entry.depot_id) || 0,
      manifest_gid: entry.manifest_gid?.trim() ?? "",
    })),
  };
}

export function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  try { return JSON.stringify(error); } catch { return "发生未知错误"; }
}
