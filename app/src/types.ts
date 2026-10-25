export type DllState = "Missing" | "Managed" | "Foreign";
export type DllLoadState = "Loaded" | "NotLoaded" | "SteamNotRunning" | "VerifyFailed";

export type DllStatus = {
  name: string;
  state: DllState;
  target_path: string;
  resource_hash?: string | null;
  target_hash?: string | null;
  hash_matched: boolean;
  loaded_by_steam: boolean;
  load_state: DllLoadState;
};

export type ScanState = {
  steam_dir: string;
  steam_valid: boolean;
  steam_running: boolean;
  steam_version: string | null;
  config_exists: boolean;
  lua_count: number;
  dlls: DllStatus[];
  dll_resources_ready: boolean;
  missing_dll_resources: string[];
  log_files: string[];
};

export type ManagerSettings = {
  log_level: string;
  manifest_url: string;
  timeout_resolve_ms: number;
  timeout_connect_ms: number;
  timeout_send_ms: number;
  timeout_recv_ms: number;
  lua_paths: string[];
  /** `[remote] url_template` — must contain {channel}, {component}, {sha256}. */
  remote_url_template: string;
  /** `[ui] suppress_missing_popup` — only honoured by our own DLL build. */
  suppress_missing_popup: boolean;
  /** Legacy `[pattern] mirror`, read-only migration source. */
  pattern_mirror: string;
  /** Write `<Steam>/config/lua/manifest.lua` (manifest-code fallback chain). */
  manage_manifest_lua: boolean;
  /** Optional self-hosted manifest-code endpoint prefix. */
  manifest_lua_endpoint: string;
};

export type ManifestLuaStatus = {
  path: string;
  present: boolean;
  managed: boolean;
  bytes: number;
  modified_time?: number | null;
};

export type ManifestProbe = {
  provider: string;
  url: string;
  status?: number | null;
  latency_ms: number;
  body: string;
  error?: string | null;
  code_ok: boolean;
  cdn_hint?: string | null;
};

export type DownloadProbe = {
  provider: string;
  code: string;
  cdn_host: string;
  /** Steam cache group for this host (hkg1 = 33, tyo3 = 32 …). */
  cdn_cell?: number | null;
  /** steam-cell | steam-log | provider-hint | default */
  cdn_source: string;
  url: string;
  status?: number | null;
  bytes: number;
  error?: string | null;
};

export type ManifestChannelProbe = {
  appid: number;
  game_name: string;
  depot_id: number;
  manifest_gid: string;
  cell_id?: number | null;
  /** Hosts Steam itself used most recently (may include token-gated CDNs). */
  steam_hosts: string[];
  providers: ManifestProbe[];
  download?: DownloadProbe | null;
};

export type GenerateReport = {
  files: ChannelFile[];
  skipped: string[];
};

export type ChannelFile = {
  channel: string;
  component: string;
  path: string;
  present: boolean;
  bytes: number;
  entries: number;
  entry_label: string;
  expected_name: string;
};

export type DllProvenance = {
  present: boolean;
  path: string;
  sha256: string;
  size: number;
  layout_signature: string | null;
  has_suppress_support: boolean;
  has_ipc_channel: boolean;
};

export type TomlSnapshot = {
  log_level: string;
  manifest_url: string;
  remote_url_template: string;
  suppress_missing_popup: boolean;
  legacy_pattern_mirror: string;
};

export type CrashDump = {
  name: string;
  path: string;
  size: number;
  modified_unix: number;
  exception_code: number | null;
  faulting_module: string | null;
  faulting_offset: number | null;
  fault_address: number | null;
  verdict: string;
};

export type BuildInspection = {
  steamui_sha256: string | null;
  steamclient_sha256: string | null;
  steam_version: string | null;
  dll: DllProvenance;
  channels: ChannelFile[];
  toml: TomlSnapshot;
  crash_dumps: CrashDump[];
};

export type GameConfig = {
  appid: number;
  name: string;
  enabled: boolean;
  depot_key?: string | null;
  access_token?: string | null;
  manifest_gid?: string | null;
  app_ticket_hex?: string | null;
  e_ticket_hex?: string | null;
  stat_steam_id?: string | null;
  appid_entries?: AppIdEntry[];
  manifest_entries?: ManifestEntry[];
};

export type AppIdEntry = {
  appid: number;
  unlock_flag?: number | null;
  depot_key?: string | null;
};

export type ManifestEntry = {
  depot_id: number;
  manifest_gid: string;
};

/** 批量导入里单个文件的失败原因；其余文件照常导入。 */
export type ImportFailure = {
  file_name: string;
  path: string;
  error: string;
};

export type ImportReport = {
  imported: GameConfig[];
  failed: ImportFailure[];
};

export type AppMetadata = {
  appid: number;
  name: string;
  source: string;
};

export type DiscoveredDepot = {
  depot_id: number;
  depot_key?: string | null;
  manifest_gid?: string | null;
};

export type DiscoveredDlc = {
  appid: number;
  name?: string | null;
  depots: DiscoveredDepot[];
  key_found: boolean;
};

export type DepotDiscovery = {
  appid: number;
  name: string;
  depots: DiscoveredDepot[];
  dlc: DiscoveredDlc[];
  app_source: string;
  config_found: boolean;
};

/** 已保存配置里每一行属于主内容还是某个 DLC。 */
export type DepotOwnership = {
  appid: number;
  group: "base" | "dlc" | string;
  dlc_appid?: number | null;
  dlc_name?: string | null;
};

export type LogFile = {
  name: string;
  content: string;
  size_bytes: number;
  modified_time?: number | null;
  line_count: number;
};
