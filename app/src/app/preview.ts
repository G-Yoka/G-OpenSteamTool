import type {
  BuildInspection, ChannelFile, GameConfig, LogFile, ManagerSettings, ManifestChannelProbe, ManifestLuaStatus, ScanState,
} from "../types";

/**
 * 浏览器预览数据（仅 `npm run dev` 在没有 Tauri 运行时的环境里使用）。
 * 打包后的 EXE 一定带 `__TAURI_INTERNALS__`，永远不会走到这里，
 * 所以这些固定值不会影响真实用户看到的状态。
 * 存在的意义：不装 Steam 也能对着真实密度校对排版、截图与演示键盘操作。
 */
export const PREVIEW_STEAM_DIR = "D:\\Steam";

const DLLS = ["OpenSteamTool.dll", "dwmapi.dll", "xinput1_4.dll"];

export function previewScan(steamDir = PREVIEW_STEAM_DIR): ScanState {
  return {
    steam_dir: steamDir,
    steam_valid: true,
    steam_running: true,
    steam_version: "1731202840",
    config_exists: true,
    lua_count: 4,
    dlls: DLLS.map((name, index) => {
      const state = index === 2 ? "Missing" : "Managed";
      return {
        name,
        state,
        target_path: `${steamDir}\\${name}`,
        resource_hash: state === "Missing" ? "9f2c41ab77d0e3519c8a04d0f2f0c4b1d7c1a6f1c6b30d4e8b7a2e19f4c3d1a6" : "3ab8d1f7c2e4905b61f7a3c8d2e6b1904f7c2a5d8e1b3064f9a7c2d5e8b1f430",
        target_hash: state === "Missing" ? null : "3ab8d1f7c2e4905b61f7a3c8d2e6b1904f7c2a5d8e1b3064f9a7c2d5e8b1f430",
        hash_matched: state !== "Missing",
        loaded_by_steam: index === 0,
        load_state: index === 0 ? "Loaded" : "NotLoaded",
      } as ScanState["dlls"][number];
    }),
    dll_resources_ready: true,
    missing_dll_resources: [],
    log_files: ["opensteamtool.log"],
  };
}

const CHANNELS: ChannelFile[] = [
  { channel: "pattern", component: "steamui", path: "", present: true, bytes: 2841, entries: 10, entry_label: "10 项", expected_name: "3ab8d1f7c2e4905b.toml" },
  { channel: "pattern", component: "steamclient", path: "", present: true, bytes: 6412, entries: 25, entry_label: "25 项", expected_name: "3ab8d1f7c2e4905b.toml" },
  { channel: "ipc", component: "steamclient", path: "", present: false, bytes: 0, entries: 0, entry_label: "缺失", expected_name: "3ab8d1f7c2e4905b.toml" },
];

export function previewBuild(steamDir = PREVIEW_STEAM_DIR): BuildInspection {
  return {
    steamui_sha256: "cd9226db4e6c28942df70c22c0475d91fdd1c1a2b1c70e30d7b4c9a1e6f04d2b",
    steamclient_sha256: "e9045710ec7ee47d7b7488d1c5dda2d9b8a1c3e7f0d24a6b9c5e8d1f3a7b2c40",
    steam_version: "1731202840",
    dll: {
      present: true,
      path: `${steamDir}\\OpenSteamTool.dll`,
      sha256: "3ab8d1f7c2e4905b61f7a3c8d2e6b1904f7c2a5d8e1b3064f9a7c2d5e8b1f430",
      size: 486_912,
      layout_signature: "1A0:2E8",
      has_suppress_support: true,
      has_ipc_channel: true,
    },
    channels: CHANNELS.map((file) => ({ ...file, path: `${steamDir}\\opensteamtool\\${file.channel}\\${file.component}\\${file.expected_name}` })),
    toml: { log_level: "info", manifest_url: "wudrm", remote_url_template: "", suppress_missing_popup: false, legacy_pattern_mirror: "" },
    crash_dumps: [
      {
        name: "steam_20260912_174512.dmp",
        path: `${steamDir}\\dumps\\steam_20260912_174512.dmp`,
        size: 1_486_233,
        modified_unix: 1_757_687_512,
        exception_code: 3221225477,
        faulting_module: "C:\\Steam\\steamclient64.dll",
        faulting_offset: 4_362_118,
        fault_address: 18,
        verdict: "faulting module 是 steamclient64.dll：RecvPkt 侧布局偏移过期，重新观测后跑 cli layout。",
      },
      {
        name: "steam_20260903_091201.dmp",
        path: `${steamDir}\\dumps\\steam_20260903_091201.dmp`,
        size: 1_204_994,
        modified_unix: 1_756_869_121,
        exception_code: 3221225477,
        faulting_module: "C:\\Steam\\OpenSteamTool.dll",
        faulting_offset: 128_450,
        fault_address: 0,
        verdict: "与本工具组件相关：钩子内部空指针，检查 pattern 是否全新。",
      },
    ],
  };
}

export const previewSettings: ManagerSettings = {
  log_level: "info",
  manifest_url: "opensteamtool",
  timeout_resolve_ms: 5000,
  timeout_connect_ms: 5000,
  timeout_send_ms: 10000,
  timeout_recv_ms: 10000,
  lua_paths: ["D:\\SteamLua", "E:\\SharedLua"],
  remote_url_template: "",
  suppress_missing_popup: false,
  pattern_mirror: "",
  manage_manifest_lua: true,
  manifest_lua_endpoint: "",
};

export const previewManifestLua: ManifestLuaStatus = {
  path: "D:\\Steam\\config\\lua\\manifest.lua",
  present: true,
  managed: true,
  bytes: 2313,
  modified_time: 1_792_788_000,
};

export const previewProbe: ManifestChannelProbe = {
  appid: 730,
  game_name: "Counter-Strike 2",
  depot_id: 731,
  manifest_gid: "4820981377404920371",
  cell_id: 166,
  steam_hosts: ["dl.steam.clngaa.com", "st.dl.eccdnx.com"],
  providers: [
    { provider: "manifestdex", url: "https://manifest.manifestdex.com/4820981377404920371", status: 200, latency_ms: 640, body: "9767027774221603670", error: null, code_ok: true, cdn_hint: null },
    { provider: "opensteamtool", url: "https://manifest.opensteamtool.com/4820981377404920371", status: 403, latency_ms: 310, body: "", error: null, code_ok: false, cdn_hint: null },
    { provider: "wudrm", url: "http://gmrc.wudrm.com/manifest/4820981377404920371", status: 502, latency_ms: 220, body: "", error: null, code_ok: false, cdn_hint: null },
  ],
  download: {
    provider: "manifestdex",
    code: "9767027774221603670",
    cdn_host: "cache9-tyo3.steamcontent.com",
    cdn_cell: 32,
    cdn_source: "steam-cell",
    url: "https://cache9-tyo3.steamcontent.com/depot/731/manifest/4820981377404920371/5/9767027774221603670",
    status: 200,
    bytes: 134_456,
    error: null,
  },
};

export function previewGames(): GameConfig[] {
  const seed: Array<[number, string, boolean]> = [
    [730, "Counter-Strike 2", true],
    [570, "Dota 2", true],
    [1091500, "Cyberpunk 2077", true],
    [292030, "The Witcher 3: Wild Hunt", false],
    [1245620, "ELDEN RING", true],
    [236390, "War Thunder", false],
  ];
  return seed.map(([appid, name, enabled]) => ({
    appid,
    name,
    enabled,
    depot_key: "",
    access_token: "",
    manifest_gid: "",
    app_ticket_hex: "",
    e_ticket_hex: "",
    stat_steam_id: "",
    appid_entries: [
      { appid, unlock_flag: 0, depot_key: "" },
      { appid: appid + 1, unlock_flag: null, depot_key: "7d8f3c1a9e5b2046d1c8f7a3b6e90d24c5f1a8b7" },
    ],
    manifest_entries: [{ depot_id: appid + 1, manifest_gid: "4820981377404920371" }],
  }));
}

const LOG = `[2026-09-17 09:14:02] [info] opensteamtool: loaded pattern/steamui (10 entries)
[2026-09-17 09:14:02] [info] opensteamtool: loaded pattern/steamclient (25 entries)
[2026-09-17 09:14:02] [warn] opensteamtool: no entry for 'ipc/steamclient' — IPC handlers disabled
[2026-09-17 09:14:02] [info] opensteamtool: layout signature CNetPacket +0x1A0 / size +0x2E8
[2026-09-17 09:14:03] [debug] ctor: CPackageManager ctor hooked at 0x00007FF8A1C2D410
[2026-09-17 09:14:03] [debug] ctor: CPackageInfo ctor hooked at 0x00007FF8A1C31F20
[2026-09-17 09:14:03] [info] addappid(730) registered without depot key
[2026-09-17 09:14:03] [info] addappid(731) registered with depot key
[2026-09-17 09:14:04] [info] addappid(1091500) registered without depot key
[2026-09-17 09:14:04] [info] manifest 731 → 4820981377404920371
[2026-09-17 09:14:05] [info] steam ui ready (build 1731202840)
[2026-09-17 09:15:11] [warn] RecvPkt: exception 0xC0000005 suppressed at 0x00007FF8A1D214B0
[2026-09-17 09:15:11] [warn] RecvPkt: exception 0xC0000005 suppressed at 0x00007FF8A1D214B0
[2026-09-17 09:15:12] [warn] RecvPkt: exception 0xC0000005 suppressed at 0x00007FF8A1D214B0
[2026-09-17 09:15:40] [info] package change: 730 state 4 → 6
[2026-09-17 09:16:02] [error] pattern: no entry for 'GetAppOwnershipTicket' — falling back to native
[2026-09-17 09:16:02] [info] ticket: native call returned 0 bytes
[2026-09-17 09:17:19] [debug] ipc: handler dispatch skipped (channel missing)
[2026-09-17 09:18:00] [info] shutdown requested by ui
[2026-09-17 09:18:00] [info] opensteamtool: 2 warnings, 1 error this session`;

export function previewLogs(): LogFile[] {
  const lines = LOG.split("\n");
  return [
    { name: "opensteamtool.log", content: LOG, size_bytes: 2_184, modified_time: 1_757_998_680, line_count: lines.length },
    { name: "opensteamtool.1.log", content: LOG.split("\n").slice(0, 12).join("\n"), size_bytes: 1_622, modified_time: 1_757_912_280, line_count: 12 },
    { name: "pattern.log", content: "[2026-09-16 21:03:10] [info] resolve steamui.dll --require-all\n[2026-09-16 21:03:11] [info] 10/10 rules matched", size_bytes: 412, modified_time: 1_757_825_000, line_count: 2 },
  ];
}
