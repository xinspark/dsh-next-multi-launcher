// IPC 包装：统一在边界把 Rust 的 {code,message,detail} 变成可展示的错误。
import { invoke } from '@tauri-apps/api/core';
import { mockCall } from './mock';
import type {
  Config,
  PermissionReport,
  ConfigPatch,
  Instance,
  InstancePayload,
  Snapshot,
  StopOutcome,
  VersionInfo,
  DiscoverCandidate
} from './types';

/** 浏览器里预览界面时没有 Tauri 运行时，走 mock（只在 dev 构建里生效）。 */
const previewMode =
  import.meta.env.DEV && typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);

export interface UiError {
  code: string;
  message: string;
  detail?: string;
}

function toUiError(raw: unknown): UiError {
  if (raw && typeof raw === 'object') {
    const obj = raw as Record<string, unknown>;
    if (typeof obj.code === 'string' && typeof obj.message === 'string') {
      return {
        code: obj.code,
        message: obj.message,
        detail: typeof obj.detail === 'string' ? obj.detail : undefined
      };
    }
    if (typeof obj.message === 'string') return { code: 'E1000', message: obj.message };
  }
  return { code: 'E1000', message: String(raw ?? '未知错误 / unknown error') };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (previewMode) {
    return mockCall(cmd, args) as T;
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw toUiError(err);
  }
}

export const api = {
  snapshot: () => call<Snapshot>('snapshot'),
  health: () => call<Record<string, unknown>>('health'),

  saveInstance: (payload: InstancePayload) => call<Instance>('instance_save', { payload }),
  archiveInstance: (id: string, archived: boolean) =>
    call<Instance>('instance_archive', { id, archived }),
  deleteInstance: (id: string, deleteData: boolean) =>
    call<void>('instance_delete', { id, deleteData }),

  launch: (id: string) => call<void>('instance_launch', { id }),
  cancel: (id: string) => call<void>('instance_cancel', { id }),
  stop: (id: string, force: boolean) => call<StopOutcome>('instance_stop', { id, force }),
  restartInstance: (id: string, mode: 'normal' | 'safe' | 'exitSafe') =>
    call<void>('instance_restart', { id, mode }),

  restorePointer: () => call<Snapshot['pointer']>('pointer_restore_default'),
  setConfig: (patch: ConfigPatch) => call<Config>('config_set', { patch }),

  scanVersions: () => call<VersionInfo[]>('version_scan'),
  importVersion: (dir: string) => call<VersionInfo>('version_import', { dir }),
  removeVersion: (tag: string) => call<void>('version_remove', { tag }),
  discoverVersions: () => call<DiscoverCandidate[]>('version_discover'),

  permissionReport: () => call<PermissionReport>('permission_report'),
  relaunchElevated: () => call<boolean>('relaunch_elevated'),

  pickDirectory: () => call<string | null>('pick_directory'),
  openPath: (path: string, create = false) => call<void>('open_path', { path, create })
};

/** 错误文案：中文人话在前，错误码附后并与 detail 一起进 title。 */
export function errorText(err: UiError): string {
  const extra = err.detail ? `（${err.detail}）` : '';
  return `${err.message}${extra}`;
}

/** 45 字符截断（设计 §8 文案规则）。 */
export function truncate(text: string, max = 45): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}
