import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppMetadata, BuildInspection, ChannelFile, CrashDump, DepotDiscovery, GameConfig, LogFile,
  ManagerSettings, ScanState,
} from "./types";
import { PREVIEW_STEAM_DIR, previewBuild, previewGames, previewLogs, previewScan, previewSettings } from "./app/preview";

const hasTauri = () => "__TAURI_INTERNALS__" in window;

export async function detectSteamDir(): Promise<string | null> {
  if (!hasTauri()) return PREVIEW_STEAM_DIR;
  return invoke<string | null>("detect_steam_dir");
}

export async function pickSteamDir(): Promise<string | null> {
  if (!hasTauri()) return null;
  const picked = await open({ directory: true, multiple: false, title: "选择 Steam 根目录" });
  return typeof picked === "string" ? picked : null;
}

export async function scanState(steamDir: string): Promise<ScanState> {
  if (!hasTauri()) return previewScan(steamDir || PREVIEW_STEAM_DIR);
  return invoke<ScanState>("scan_state", { steamDir });
}

export async function installDlls(steamDir: string): Promise<void> {
  if (!hasTauri()) return;
  return invoke("install_dlls", { steamDir });
}

export async function removeDlls(steamDir: string): Promise<void> {
  if (!hasTauri()) return;
  return invoke("remove_dlls", { steamDir });
}

export async function loadSettings(steamDir: string): Promise<ManagerSettings> {
  if (!hasTauri()) return previewSettings;
  return invoke<ManagerSettings>("load_settings", { steamDir });
}

export async function saveSettings(steamDir: string, settings: ManagerSettings): Promise<void> {
  if (!hasTauri()) return;
  return invoke("save_settings", { steamDir, settings });
}

export async function listGames(steamDir: string): Promise<GameConfig[]> {
  if (!hasTauri()) return previewGames();
  return invoke<GameConfig[]>("list_games", { steamDir });
}

export async function importLuaFile(steamDir: string): Promise<GameConfig | null> {
  if (!hasTauri()) return null;
  return invoke<GameConfig | null>("import_lua_file", { steamDir });
}

export async function openLuaDir(steamDir: string): Promise<void> {
  if (!hasTauri()) return;
  return invoke("open_lua_dir", { steamDir });
}

export async function upsertGame(steamDir: string, game: GameConfig): Promise<void> {
  if (!hasTauri()) return;
  return invoke("upsert_game", { steamDir, game });
}

export async function deleteGame(steamDir: string, appid: number): Promise<void> {
  if (!hasTauri()) return;
  return invoke("delete_game", { steamDir, appid });
}

export async function setGameEnabled(steamDir: string, appid: number, enabled: boolean): Promise<void> {
  if (!hasTauri()) return;
  return invoke("set_game_enabled", { steamDir, appid, enabled });
}

export async function fetchAppMetadata(appid: number): Promise<AppMetadata> {
  if (!hasTauri()) return { appid, name: `App ${appid}`, source: "preview" };
  return invoke<AppMetadata>("fetch_app_metadata", { appid });
}

export async function readLogs(steamDir: string): Promise<LogFile[]> {
  if (!hasTauri()) return previewLogs();
  return invoke<LogFile[]>("read_logs", { steamDir });
}

export async function closeSteam(): Promise<void> {
  if (!hasTauri()) return;
  return invoke("close_steam");
}

export async function restartSteam(steamDir: string): Promise<void> {
  if (!hasTauri()) return;
  return invoke("restart_steam", { steamDir });
}

export async function minimizeWindow(): Promise<void> {
  if (!hasTauri()) return;
  return invoke("minimize_window");
}

export async function toggleMaximize(): Promise<void> {
  if (!hasTauri()) return;
  return invoke("toggle_maximize");
}

export async function closeWindow(): Promise<void> {
  if (!hasTauri()) return;
  return invoke("close_window");
}

export async function discoverDepots(steamDir: string, appid: number): Promise<DepotDiscovery> {
  if (!hasTauri()) {
    return {
      appid,
      name: `App ${appid}`,
      depots: [
        { depot_id: appid + 1, depot_key: "7d8f3c1a9e5b2046d1c8f7a3b6e90d24c5f1a8b7", manifest_gid: "4820981377404920371" },
        { depot_id: appid + 2, depot_key: null, manifest_gid: "7391024751209384756" },
      ],
      app_source: "preview",
      config_found: true,
    };
  }
  return invoke<DepotDiscovery>("discover_depots", { steamDir, appid });
}

export async function startWindowDrag(): Promise<void> {
  if (!hasTauri()) return;
  return invoke("start_window_drag");
}

export async function inspectBuild(steamDir: string): Promise<BuildInspection> {
  if (!hasTauri()) return previewBuild(steamDir || PREVIEW_STEAM_DIR);
  return invoke<BuildInspection>("inspect_build", { steamDir });
}

export async function installChannelFile(
  steamDir: string, channel: string, component: string, source: string,
): Promise<ChannelFile> {
  if (!hasTauri()) throw new Error("预览模式无法安装通道文件");
  return invoke<ChannelFile>("install_channel_file", { steamDir, channel, component, source });
}

export async function generateChannelFile(
  steamDir: string, channel: string, component: string,
): Promise<ChannelFile> {
  if (!hasTauri()) throw new Error("预览模式无法生成通道文件");
  return invoke<ChannelFile>("generate_channel_file", { steamDir, channel, component });
}
export async function removeChannelFile(
  steamDir: string, channel: string, component: string,
): Promise<ChannelFile> {
  if (!hasTauri()) throw new Error("预览模式无法移除通道文件");
  return invoke<ChannelFile>("remove_channel_file", { steamDir, channel, component });
}

export async function listCrashDumps(steamDir: string): Promise<CrashDump[]> {
  if (!hasTauri()) return [];
  return invoke<CrashDump[]>("list_crash_dumps", { steamDir });
}

export async function pickTomlFile(): Promise<string | null> {
  if (!hasTauri()) return null;
  const picked = await open({
    multiple: false,
    title: "选择通道数据文件（<sha256>.toml）",
    filters: [{ name: "OpenSteamTool data", extensions: ["toml"] }],
  });
  return typeof picked === "string" ? picked : null;
}
