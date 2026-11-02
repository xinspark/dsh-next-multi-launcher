// 浏览器预览用的假数据（只在 `import.meta.env.DEV` 且没有 Tauri 运行时的时候生效）。
// 目的：调界面时不必每次起真应用；生产构建里这段会被 tree-shake 掉。
import type { Instance, InstanceView, Snapshot, VersionInfo } from './types';

const now = new Date();
const iso = (offsetMin = 0) => new Date(now.getTime() - offsetMin * 60_000).toISOString();

const versions: VersionInfo[] = [
  {
    tag: '2.0.21-next',
    dir: 'E:\\FXS\\Data\\DSH000\\DSH-NEXT Multi-Launcher\\versions\\2.0.21-next',
    exe: 'E:\\FXS\\Data\\DSH000\\DSH-NEXT Multi-Launcher\\versions\\2.0.21-next\\DSH NEXT.exe',
    asar: 'E:\\FXS\\Data\\DSH000\\DSH-NEXT Multi-Launcher\\versions\\2.0.21-next\\resources\\app.asar',
    shellVersion: '2.0.21-next',
    kernelVersion: '0.2.1-rc.4',
    valid: true,
    isLink: true,
    linkTarget: 'E:\\FXS\\APP\\DSH NEXT'
  },
  {
    tag: '2.0.20-next',
    dir: 'E:\\FXS\\Data\\DSH000\\DSH-NEXT Multi-Launcher\\versions\\2.0.20-next',
    shellVersion: '2.0.20-next',
    kernelVersion: '0.2.0-rc.9',
    valid: false,
    invalidReason: '缺少 resources\\app.asar',
    isLink: false
  }
];

function inst(id: string, name: string, home: string, extra: Partial<Instance> = {}): Instance {
  return {
    schemaVersion: 1,
    id,
    name,
    version: '2.0.21-next',
    home,
    archived: false,
    createdAt: iso(600),
    lastLaunchAt: iso(12),
    lastExitClean: true,
    notes: '',
    safeMode: false,
    ...extra
  };
}

const instances: Instance[] = [
  inst('ziyong', '自用', 'C:\\Users\\FXS\\.dsh'),
  inst('lingshu', '灵枢', 'C:\\Users\\FXS\\.dsh-lingshu', { notes: '专门用来试新插件' }),
  inst('lab', '实验室', 'C:\\Users\\FXS\\.dsh-lab', { version: '2.0.21-next', archived: false }),
  inst('old', '旧环境', 'C:\\Users\\FXS\\.dsh-old', { archived: true })
];

export function mockSnapshot(): Snapshot {
  const running = instances.filter((i) => i.id === 'lingshu').map(() => ({
    home: 'C:\\Users\\FXS\\.dsh-lingshu',
    shellPid: 40752,
    hostPid: 40388,
    startedAt: Math.floor(now.getTime() / 1000) - 12 * 60,
    safeMode: false
  }));

  const views: InstanceView[] = instances.map((i) => {
    const run = running.find((r) => r.home.toLowerCase() === i.home.toLowerCase());
    return {
      instance: i,
      running: run,
      launch: {
        instanceId: i.id,
        phase: run ? 'running' : i.id === 'lab' ? 'failed' : 'idle',
        elapsedMs: 0,
        shellPid: run?.shellPid,
        hostPid: run?.hostPid,
        error: i.id === 'lab' ? { code: 'E1005', message: '启动超时（90 秒内未看到该目录的进程）', detail: 'C:\\Users\\FXS\\.dsh-lab' } : undefined
      },
      version: versions[0],
      isDefault: i.id === 'ziyong',
      homeExists: true
    };
  });

  return {
    config: {
      schemaVersion: 1,
      defaultHome: 'C:\\Users\\FXS\\.dsh',
      homeRoot: 'C:\\Users\\FXS',
      launchTimeoutSec: 90,
      pollIntervalMs: 500,
      restorePointerAfterLaunch: true,
      downloadMirror: '',
      lastPointer: { home: 'C:\\Users\\FXS\\.dsh-lingshu', at: iso(12) }
    },
    pointer: {
      path: 'C:\\Users\\FXS\\AppData\\Roaming\\DSH NEXT\\home\\desktop-next-location.json',
      home: 'C:\\Users\\FXS\\.dsh',
      raw: '{ "home": "C:\\\\Users\\\\FXS\\\\.dsh" }',
      exists: true,
      writable: true
    },
    versions,
    instances: views,
    running,
    queue: [],
    dataDir: 'C:\\Users\\FXS\\AppData\\Local\\DSH-NEXT-Multi',
    versionsDir: 'E:\\FXS\\Data\\DSH000\\DSH-NEXT Multi-Launcher\\versions',
    logDir: 'C:\\Users\\FXS\\AppData\\Local\\DSH-NEXT-Multi\\logs',
    appVersion: '1.0.0',
    userProfile: 'C:\\Users\\FXS'
  };
}

export function mockCall(cmd: string, args?: Record<string, unknown>): unknown {
  switch (cmd) {
    case 'snapshot':
      return mockSnapshot();
    case 'instance_stop':
      return { stopped: true, needsForce: false, pid: 40752 };
    case 'instance_delete':
    case 'instance_cancel':
      return null;
    case 'instance_launch':
      return null;
    case 'instance_archive':
      return instances[0];
    case 'instance_save':
      return instances[0];
    case 'pointer_restore_default':
      return mockSnapshot().pointer;
    case 'config_set':
      return mockSnapshot().config;

    case 'open_url':
      return null;
    case 'update_check':
      return {
        current: '1.0.2',
        latest: 'v1.0.2',
        hasUpdate: false,
        url: 'https://github.com/xinspark/dsh-next-multi-launcher/releases/tag/v1.0.2'
      };
    case 'catalog_list': {
      const catalog: Record<string, unknown[]> = {
        next: [
          { channel: 'next', tag: 'v2.0.21-next', publishedAt: '2026-10-29T23:27:36Z', size: 323798424, assetName: 'DSH-NEXT-2.0.21-next-x64-Setup.exe', kind: 'installer', url: 'https://example.com/a.exe', digest: 'sha256:abc', prerelease: false },
          { channel: 'next', tag: 'v2.0.20-next', publishedAt: '2026-10-27T00:11:23Z', size: 324264074, assetName: 'DSH-NEXT-2.0.20-next-x64-Setup.exe', kind: 'installer', url: 'https://example.com/b.exe', digest: 'sha256:def', prerelease: false }
        ],
        'next-preview': [
          { channel: 'next-preview', tag: 'v2.0.21-beta.1', publishedAt: '2026-10-29T22:00:00Z', size: 334547330, assetName: 'DSH-Desktop-Beta-2.0.21-beta.1-x64-Setup.exe', kind: 'installer', url: 'https://example.com/c.exe', digest: 'sha256:ghi', prerelease: true }
        ],
        native: [
          { channel: 'native', tag: 'latest', size: 0, assetName: 'dsh-latest-windows-x64.exe', kind: 'installer', url: 'https://download.deepseek.com/desktop/dsh-latest-windows-x64.exe', prerelease: false }
        ]
      };
      return catalog[String(args?.channel ?? 'next')] ?? [];
    }
    case 'version_scan':
      return versions;
    case 'version_discover':
      return [
        {
          path: 'E:\\FXS\\APP\\DSH NEXT',
          source: 'shortcut',
          valid: true,
          shellVersion: '2.0.21-next',
          kernelVersion: '0.2.1-rc.4',
          registered: false
        },
        {
          path: 'C:\\Users\\demo\\AppData\\Local\\Programs\\DSH Desktop',
          source: 'registry',
          valid: true,
          shellVersion: '2.0.20-next',
          kernelVersion: '0.2.0-rc.9',
          registered: false
        },
        {
          path: 'D:\\Somewhere\\DSH',
          source: 'common',
          valid: false,
          registered: false
        }
      ];
    case 'version_import':
      return versions[0];
    case 'version_remove':
      return null;
    case 'pick_directory':
      return 'E:\\FXS\\APP\\DSH NEXT';
    case 'open_path':
      return null;
    case 'health':
      return { ok: true };
    default:
      void args;
      return null;
  }
}


