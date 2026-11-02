#!/usr/bin/env node
// gui-smoke.mjs —— 启动真实 GUI（release 构建），抓一张窗口截图，用来做视觉验收。
// 为了绕过本机沙箱的写盘边界（见 docs/env-check.md），用环境变量把数据目录 / 版本目录 /
// 指针文件全部指到 workspace 内；替身版本树用 tools/e2e-stub 的 exe。
//
// 用法：node tools/gui-smoke.mjs [--exe <path>] [--out target/ui.png] [--keep]

import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const args = process.argv.slice(2);
function arg(name, fallback) {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
}

const root = resolve('.');
const exe = resolve(arg('exe', 'src-tauri/target/release/dsh-next-multi-launcher.exe'));
const out = resolve(arg('out', 'target/ui.png'));
const keep = args.includes('--keep');
const base = join(root, 'src-tauri', 'target', 'gui-test');
const dataDir = join(base, 'data');
const versionsDir = join(base, 'versions');
const pointerFile = join(base, 'pointer.json');
const stubExe = join(root, 'tools', 'e2e-stub', 'target', 'release', 'e2e-stub.exe');
const TAG = '2.0.21-next';

function fakeAsar(version, kernel) {
  const body = Buffer.from(JSON.stringify({ version, dependencies: { '@deepseek-ai/dsh': kernel } }), 'utf8');
  const index = Buffer.from(
    JSON.stringify({ files: { 'package.json': { size: body.length, offset: '0' } } }),
    'utf8'
  );
  const header = Buffer.alloc(16);
  header.writeUInt32LE(4, 0);
  header.writeUInt32LE(index.length + 8, 4);
  header.writeUInt32LE(index.length, 8);
  header.writeUInt32LE(index.length, 12);
  return Buffer.concat([header, index, body]);
}

function rmTree(dir) {
  for (let i = 0; i < 6; i++) {
    try {
      rmSync(dir, { recursive: true, force: true, maxRetries: 3, retryDelay: 120 });
      return;
    } catch (err) {
      if (i === 5) throw err;
      killLauncher();
      spawnSync('powershell', ['-NoProfile', '-Command', 'Start-Sleep -Milliseconds 400']);
    }
  }
}

function prepare() {
  killLauncher();
  rmTree(base);
  mkdirSync(join(dataDir, 'instances'), { recursive: true });
  mkdirSync(join(base, 'homes', 'ziyong'), { recursive: true });
  mkdirSync(join(base, 'homes', 'lingshu'), { recursive: true });
  mkdirSync(join(base, 'homes', 'lab'), { recursive: true });

  const vdir = join(versionsDir, TAG);
  mkdirSync(join(vdir, 'resources'), { recursive: true });
  copyFileSync(stubExe, join(vdir, 'DSH NEXT.exe'));
  writeFileSync(join(vdir, 'resources', 'app.asar'), fakeAsar(TAG, '0.2.1-rc.4'));

  const defaultHome = join(base, 'homes', 'ziyong');
  writeFileSync(pointerFile, JSON.stringify({ home: defaultHome }, null, 2) + '\n', 'utf8');

  const now = new Date().toISOString();
  const instances = [
    { id: 'ziyong', name: '自用', home: defaultHome },
    { id: 'lingshu', name: '灵枢', home: join(base, 'homes', 'lingshu') },
    { id: 'lab', name: '实验室', home: join(base, 'homes', 'lab') }
  ];
  for (const inst of instances) {
    writeFileSync(
      join(dataDir, 'instances', `${inst.id}.json`),
      JSON.stringify(
        {
          schemaVersion: 1,
          id: inst.id,
          name: inst.name,
          version: TAG,
          home: inst.home,
          archived: false,
          createdAt: now,
          lastLaunchAt: now,
          lastExitClean: true,
          notes: inst.id === 'lab' ? '专门用来试新插件' : ''
        },
        null,
        2
      ) + '\n',
      'utf8'
    );
  }

  writeFileSync(
    join(dataDir, 'app.json'),
    JSON.stringify(
      {
        schemaVersion: 1,
        defaultHome,
        launchTimeoutSec: 30,
        pollIntervalMs: 200,
        restorePointerAfterLaunch: true
      },
      null,
      2
    ) + '\n',
    'utf8'
  );
}

function screenshot() {
  const res = spawnSync(
    'powershell',
    ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', join(root, 'tools', 'capture-window.ps1'), '-TitleLike', 'DSH-NEXT 应用多开启动器', '-Out', out],
    { encoding: 'utf8' }
  );
  return { status: res.status, stdout: res.stdout, stderr: res.stderr };
}

function killLauncher() {
  spawnSync(
    'powershell',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      "Get-Process | Where-Object { $_.ProcessName -in @('dsh-next-multi-launcher','DSH-NEXT-Multi') } | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 300"
    ],
    { encoding: 'utf8' }
  );
}

async function main() {
  if (!existsSync(exe)) {
    console.error(`找不到可执行文件：${exe}`);
    process.exit(2);
  }
  prepare();
  killLauncher();

  const env = {
    ...process.env,
    DSH_MULTI_DATA_DIR: dataDir,
    DSH_MULTI_VERSIONS: versionsDir,
    DSH_MULTI_POINTER: pointerFile,
    DSH_MULTI_WEBVIEW_DATA: join(base, 'webview2'),
    DSH_MULTI_WINDOW_SIZE: process.env.DSH_MULTI_WINDOW_SIZE ?? '1120x760'
  };
  const child = spawn(exe, [], { env, detached: true, stdio: 'ignore' });
  child.unref();

  await new Promise((r) => setTimeout(r, 6000));

  const shot = screenshot();
  console.log(shot.stdout?.trim() || shot.stderr?.trim() || `screenshot exit=${shot.status}`);
  console.log(
    JSON.stringify(
      {
        exe,
        out,
        dataDir,
        log: join(dataDir, 'logs', 'launcher-' + new Date().toISOString().slice(0, 10).replace(/-/g, '') + '.log'),
        shotStatus: shot.status,
        kept: keep
      },
      null,
      2
    )
  );

  killLauncher();
  if (!keep) {
    // 截图保留，运行数据保留一份方便复跑；不删 base
  }
}

main().catch((err) => {
  console.error(err);
  killLauncher();
  process.exit(1);
});
