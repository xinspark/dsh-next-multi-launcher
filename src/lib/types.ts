// 前端类型：与 Rust 侧的 serde camelCase 结构逐字对应。

export type LaunchPhase =
  | 'queued'
  | 'preparing'
  | 'spawning'
  | 'waiting'
  | 'running'
  | 'failed'
  | 'idle';

export interface AppError {
  code: string;
  message: string;
  detail?: string;
}

export interface LastPointer {
  home: string;
  at: string;
}

export interface Config {
  schemaVersion: number;
  defaultHome: string;
  /** 数据目录树根：新建实例 home 的默认值前缀（`<homeRoot>\.dsh-<id>`） */
  homeRoot: string;
  launchTimeoutSec: number;
  pollIntervalMs: number;
  restorePointerAfterLaunch: boolean;
  lastPointer?: LastPointer;
}

export interface Instance {
  schemaVersion: number;
  id: string;
  name: string;
  version: string;
  home: string;
  archived: boolean;
  createdAt: string;
  lastLaunchAt?: string;
  lastExitClean: boolean;
  notes: string;
  /** 上次启动是否带 --next-safe-mode */
  safeMode: boolean;
}

export interface VersionInfo {
  tag: string;
  dir: string;
  exe?: string;
  asar?: string;
  shellVersion: string;
  kernelVersion: string;
  valid: boolean;
  invalidReason?: string;
  /** versions\<tag> 是否是指向安装树的目录链接 */
  isLink: boolean;
  linkTarget?: string;
}

/** 自动扫描到的安装目录候选 */
export interface DiscoverCandidate {
  path: string;
  source: 'shortcut' | 'registry' | 'common' | 'process' | string;
  valid: boolean;
  shellVersion?: string;
  kernelVersion?: string;
  registered: boolean;
}

export interface RunningInfo {
  home: string;
  shellPid: number;
  hostPid?: number;
  startedAt: number;
  /** 是否以安全模式运行（命令行含 --next-safe-mode） */
  safeMode: boolean;
}

export interface LaunchStatus {
  instanceId: string;
  phase: LaunchPhase;
  position?: number;
  startedAt?: string;
  elapsedMs: number;
  shellPid?: number;
  hostPid?: number;
  error?: AppError;
}

export interface PointerSnapshot {
  path: string;
  home?: string;
  raw: string;
  exists: boolean;
  writable: boolean;
}

export interface InstanceView {
  instance: Instance;
  running?: RunningInfo;
  launch: LaunchStatus;
  version?: VersionInfo;
  isDefault: boolean;
  homeExists: boolean;
}

export interface Snapshot {
  config: Config;
  pointer: PointerSnapshot;
  versions: VersionInfo[];
  instances: InstanceView[];
  running: RunningInfo[];
  queue: string[];
  dataDir: string;
  versionsDir: string;
  logDir: string;
  appVersion: string;
  userProfile: string;
}

export interface StopOutcome {
  stopped: boolean;
  needsForce: boolean;
  pid?: number;
}

export interface InstancePayload {
  id: string;
  name: string;
  version: string;
  home: string;
  notes: string;
  isNew: boolean;
  archived?: boolean;
}

export interface ConfigPatch {
  defaultHome?: string;
  homeRoot?: string;
  launchTimeoutSec?: number;
  pollIntervalMs?: number;
  restorePointerAfterLaunch?: boolean;
}

export interface PermissionReport {
  isAdmin: boolean;
  elevatedByEnv: boolean;
  dataDir: string;
  dataWritable: boolean;
  pointerPath: string;
  pointerWritable: boolean;
  needsElevation: boolean;
}

/** 启动器自身的更新检查结果（只提示，不下载） */
export interface UpdateInfo {
  current: string;
  latest?: string;
  hasUpdate: boolean;
  url?: string;
  publishedAt?: string;
  error?: string;
}

/** 版本目录条目（三渠道共用） */
export interface CatalogEntry {
  channel: 'next' | 'next-preview' | 'native' | string;
  tag: string;
  publishedAt?: string;
  size: number;
  assetName: string;
  url: string;
  digest?: string;
  prerelease: boolean;
}
