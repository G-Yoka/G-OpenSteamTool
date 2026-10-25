import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppMetadata, BuildInspection, ChannelFile, CrashDump, DepotDiscovery, DepotOwnership, GameConfig, GenerateReport, ImportReport, LogFile,
  ManagerSettings, ManifestChannelProbe, ManifestLuaStatus, ScanState,
} from "./types";
import {
  PREVIEW_STEAM_DIR, previewBuild, previewGames, previewLogs, previewManifestLua, previewProbe, previewScan, previewSettings,
} from "./app/preview";

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

export async function manifestLuaStatus(steamDir: string): Promise<ManifestLuaStatus> {
  if (!hasTauri()) return previewManifestLua;
  return invoke<ManifestLuaStatus>("manifest_lua_status", { steamDir });
}

export async function installManifestLua(steamDir: string): Promise<ManifestLuaStatus> {
  if (!hasTauri()) return previewManifestLua;
  return invoke<ManifestLuaStatus>("install_manifest_lua", { steamDir });
}

export async function removeManifestLua(steamDir: string): Promise<ManifestLuaStatus> {
  if (!hasTauri()) return { ...previewManifestLua, present: false, managed: false, bytes: 0 };
  return invoke<ManifestLuaStatus>("remove_manifest_lua", { steamDir });
}

export async function probeManifestChannel(steamDir: string, appid?: number | null): Promise<ManifestChannelProbe> {
  if (!hasTauri()) return previewProbe;
  return invoke<ManifestChannelProbe>("probe_manifest_channel", { steamDir, appid: appid ?? null });
}

export async function listGames(steamDir: string): Promise<GameConfig[]> {
  if (!hasTauri()) return previewGames();
  return invoke<GameConfig[]>("list_games", { steamDir });
}

/** 打开文件选择器（支持多选）并把选中的 Lua 一次性导入；取消返回 null。 */
export async function importLuaFiles(steamDir: string): Promise<ImportReport | null> {
  if (!hasTauri()) return null;
  return invoke<ImportReport | null>("import_lua_files", { steamDir });
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
      dlc: [
        {
          appid: appid + 10,
          name: "Preview DLC A",
          depots: [
            { depot_id: appid + 11, depot_key: "1f0c4a7e9b2d5836ca14e7f0b3d9685c2a7e41f9d6b0835c4e2f1a9d7b3c6e50", manifest_gid: "1122334455667788990" },
          ],
          key_found: true,
        },
        { appid: appid + 12, name: "Preview DLC B", depots: [], key_found: false },
      ],
      app_source: "preview",
      config_found: true,
    };
  }
  return invoke<DepotDiscovery>("discover_depots", { steamDir, appid });
}

/** 只读的归属标注：给已保存 Lua 的每一行标出「主内容 / DLC」。 */
export async function depotOwnership(steamDir: string, appid: number): Promise<DepotOwnership[]> {
  if (!hasTauri()) {
    return [
      { appid, group: "base", dlc_appid: null, dlc_name: null },
      { appid: appid + 1, group: "base", dlc_appid: null, dlc_name: null },
      { appid: appid + 2, group: "base", dlc_appid: null, dlc_name: null },
      { appid: appid + 10, group: "dlc", dlc_appid: appid + 10, dlc_name: "Preview DLC A" },
      { appid: appid + 11, group: "dlc", dlc_appid: appid + 10, dlc_name: "Preview DLC A" },
      { appid: appid + 12, group: "dlc", dlc_appid: appid + 12, dlc_name: "Preview DLC B" },
    ];
  }
  return invoke<DepotOwnership[]>("depot_ownership", { steamDir, appid });
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
  steamDir: string, channel: string, component: string, idl?: string | null,
): Promise<ChannelFile> {
  if (!hasTauri()) throw new Error("预览模式无法生成通道文件");
  return invoke<ChannelFile>("generate_channel_file", { steamDir, channel, component, idl: idl ?? null });
}

export async function generateAllChannels(steamDir: string, idl?: string | null): Promise<GenerateReport> {
  if (!hasTauri()) throw new Error("预览模式无法生成通道文件");
  return invoke<GenerateReport>("generate_all_channels", { steamDir, idl: idl ?? null });
}
export async function pickIdlFile(): Promise<string | null> {
  if (!hasTauri()) return null;
  const picked = await open({
    multiple: false,
    title: "选择上游 IPC 定义文件（IPCMessages.steamd）",
    filters: [{ name: "IPC definition", extensions: ["steamd", "idl", "h"] }],
  });
  return typeof picked === "string" ? picked : null;
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
