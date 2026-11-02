#!/usr/bin/env node
// e2e.mjs —— 启动管道端到端验收（A1/A2/A3/A4/A5/A6）。
//
// 做法：用 tools/e2e-stub 编一个替身 `DSH NEXT.exe`（行为与真身一致：读指针、起
// `--user-data-dir=<home>\electron-user-data` 子进程、再起 host），放到隔离的
// versions/ 目录里，用环境变量把启动器指到 workspace 内的数据目录与指针文件。
//
// 不使用真实 DSH 安装树、不碰 %APPDATA% 的真实指针（见 docs/env-check.md「写盘边界」）。
//
// 用法：node tools/e2e.mjs

import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = resolve('.');
const targetDir = join(root, 'src-tauri', 'target');
const runId = new Date().toISOString().replace(/[:.]/g, '-');
const e2e = join(targetDir, `e2e-${runId}`);
const dataDir = join(e2e, 'data');
const versionsDir = join(e2e, 'versions');
const pointerFile = join(e2e, 'pointer.json');
const stubCrate = join(root, 'tools', 'e2e-stub');
const stubExe = join(stubCrate, 'target', 'release', 'e2e-stub.exe');
const launcherExe = join(targetDir, 'debug', 'dsh-next-multi-launcher.exe');

const TAG = '2.0.99-next';
const BAD_TAG = '2.0.98-broken';
const KERNEL = '0.2.99-rc.1';
const stubPath = join(versionsDir, TAG, 'DSH NEXT.exe');

const results = [];
const log = (msg) => console.log(msg);

function ps(script) {
  const res = spawnSync('powershell', ['-NoProfile', '-NonInteractive', '-Command', script], {
    encoding: 'utf8'
  });
  return (res.stdout ?? '').trim();
}

/** 杀掉历史 e2e 运行留下的替身进程（PowerShell 单引号字符串里反斜杠不转义）。 */
function killStubProcesses() {
  const prefix = join(targetDir, 'e2e-').replace(/'/g, "''");
  const script = `
$prefix = '${prefix}'
$victims = @(Get-CimInstance Win32_Process -Filter "Name='DSH NEXT.exe'" | Where-Object {
  ($_.ExecutablePath -and $_.ExecutablePath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase))
})
foreach ($v in $victims) { Stop-Process -Id $v.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Milliseconds 400
Write-Output ('killed=' + $victims.Count)
`;
  return ps(script);
}

function buildStub() {
  if (existsSync(stubExe)) return;
  log('[e2e] 编译替身进程…');
  const out = spawnSync('cargo', ['build', '--release', '--quiet', '--manifest-path', stubCrate], {
    stdio: 'inherit'
  });
  if (out.status !== 0) throw new Error('替身进程编译失败');
}

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

function prepare() {
  mkdirSync(join(dataDir, 'instances'), { recursive: true });

  const vdir = join(versionsDir, TAG);
  mkdirSync(join(vdir, 'resources'), { recursive: true });
  copyFileSync(stubExe, stubPath);
  writeFileSync(join(vdir, 'resources', 'app.asar'), fakeAsar(TAG, KERNEL));

  // 坏版本：只有 app.asar，没有 DSH NEXT.exe
  mkdirSync(join(versionsDir, BAD_TAG, 'resources'), { recursive: true });
  writeFileSync(join(versionsDir, BAD_TAG, 'resources', 'app.asar'), fakeAsar(BAD_TAG, KERNEL));

  const defaultHome = join(e2e, 'homes', 'default');
  mkdirSync(defaultHome, { recursive: true });
  writeFileSync(pointerFile, JSON.stringify({ home: defaultHome }, null, 2) + '\n', 'utf8');

  const now = new Date().toISOString();
  const writeInstance = (id, name, home, version) => {
    writeFileSync(
      join(dataDir, 'instances', `${id}.json`),
      JSON.stringify(
        {
          schemaVersion: 1,
          id,
          name,
          version,
          home,
          archived: false,
          createdAt: now,
          lastExitClean: true,
          notes: ''
        },
        null,
        2
      ) + '\n',
      'utf8'
    );
  };
  // 每个用例用独立实例，避免上一个用例的进程干扰
  writeInstance('lab1', '实验室一', join(e2e, 'homes', 'lab1'), TAG);
  writeInstance('lab2', '实验室二', join(e2e, 'homes', 'lab2'), TAG);
  writeInstance('dup', '重复目录', join(e2e, 'homes', 'lab1'), TAG);
  writeInstance('bad', '坏版本', join(e2e, 'homes', 'bad'), BAD_TAG);
  writeInstance('slow', '慢启动', join(e2e, 'homes', 'slow'), TAG);
  writeInstance('exit', '启动即退', join(e2e, 'homes', 'exit'), TAG);

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

function env(extra = {}) {
  return {
    ...process.env,
    DSH_MULTI_DATA_DIR: dataDir,
    DSH_MULTI_VERSIONS: versionsDir,
    DSH_MULTI_POINTER: pointerFile,
    ...extra
  };
}

/** 等“启动器进程退出”，不等管道 EOF（替身会活 120 秒并攥着句柄）。 */
function runCli(args, extraEnv = {}) {
  return new Promise((resolvePromise) => {
    const child = spawn(launcherExe, args, { env: env(extraEnv) });
    let textOut = '';
    let settled = false;
    const finish = (status) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      const start = textOut.indexOf('{');
      let json = null;
      if (start >= 0) {
        try {
          json = JSON.parse(textOut.slice(start));
        } catch {
          json = null;
        }
      }
      resolvePromise({ status, text: textOut, json });
    };
    const timer = setTimeout(() => finish(-1), 240_000);
    child.stdout.on('data', (chunk) => (textOut += chunk.toString('utf8')));
    child.stderr.on('data', (chunk) => (textOut += chunk.toString('utf8')));
    child.on('exit', (code) => finish(code));
    child.on('error', () => finish(-1));
  });
}

function pointerHome() {
  try {
    return JSON.parse(readFileSync(pointerFile, 'utf8')).home;
  } catch {
    return null;
  }
}

function record(name, ok, evidence) {
  results.push({ name, ok, evidence });
  log(`${ok ? 'PASS' : 'FAIL'}  ${name}${evidence ? ` — ${evidence}` : ''}`);
}

function pruneOldRuns(keep = 2) {
  try {
    const dirs = readdirSync(targetDir)
      .filter((name) => name.startsWith('e2e-'))
      .sort();
    for (const name of dirs.slice(0, Math.max(0, dirs.length - keep))) {
      try {
        rmSync(join(targetDir, name), { recursive: true, force: true });
      } catch {
        /* 旧目录可能被占用，忽略 */
      }
    }
  } catch {
    /* ignore */
  }
}

async function main() {
  buildStub();
  log(`[e2e] ${killStubProcesses()}`);
  pruneOldRuns();
  log(`[e2e] 本次目录 ${e2e}`);
  prepare();

  const originalPointer = readFileSync(pointerFile, 'utf8');

  // ---- A1：两个不同 home 的实例先后启动
  const r1 = await runCli(['--e2e-launch', 'lab1']);
  record(
    'A1-a lab1 启动成功',
    r1.json?.ok === true && r1.json?.phase === 'running',
    `exit=${r1.status} shell=${r1.json?.shellPid} host=${r1.json?.hostPid}`
  );

  const r2 = await runCli(['--e2e-launch', 'lab2']);
  const both = r2.json?.running ?? [];
  record(
    'A1-b lab2 启动成功且两个 home 同时在线',
    r2.json?.ok === true &&
      r2.json?.phase === 'running' &&
      both.some((r) => String(r.home).endsWith('lab1')) &&
      both.some((r) => String(r.home).endsWith('lab2')),
    `running=${JSON.stringify(both.map((r) => ({ home: r.home.split('\\').pop(), shell: r.shell, host: r.host })))}`
  );

  // ---- A2：同 home 被拒 E1003，指针不变
  const beforeDup = pointerHome();
  const r3 = await runCli(['--e2e-launch', 'dup']);
  record(
    'A2 同 home 被拒 E1003',
    r3.json?.ok === false && /E1003/.test(r3.text),
    `tail=${(r3.text || '').slice(-120).replace(/\n/g, ' ')}`
  );
  record('A2 指针未被改动', pointerHome() === beforeDup, `pointer=${pointerHome()}`);

  // ---- A3：版本目录缺 exe → E1004 + 指针回滚
  killStubProcesses();
  const beforeA3 = pointerHome();
  const r4 = await runCli(['--e2e-launch', 'bad']);
  record(
    'A3 版本无效 E1004',
    r4.json?.ok === false && /E1004/.test(r4.text),
    `tail=${(r4.text || '').slice(-120).replace(/\n/g, ' ')}`
  );
  record('A3 指针回滚', pointerHome() === beforeA3, `pointer=${pointerHome()}`);

  // ---- A4：启动后迟迟没有子进程 → E1005 超时 + 指针回滚
  killStubProcesses();
  const beforeA4 = pointerHome();
  const r5 = await runCli(['--e2e-launch', 'slow'], { STUB_START_DELAY_MS: '45000' });
  record(
    'A4 启动超时 E1005',
    r5.json?.ok === false && /E1005/.test(r5.text),
    `tail=${(r5.text || '').slice(-160).replace(/\n/g, ' ')}`
  );
  record('A4 指针回滚', pointerHome() === beforeA4, `pointer=${pointerHome()}`);

  // ---- A5：实例运行中改指针，原实例不搬家（观察项 Q1）
  killStubProcesses();
  const r6 = await runCli(['--e2e-launch', 'lab2']);
  const beforeA5 = pointerHome();
  const runningBefore = r6.json?.running ?? [];
  writeFileSync(
    pointerFile,
    JSON.stringify({ home: join(e2e, 'homes', 'somewhere-else') }, null, 2) + '\n',
    'utf8'
  );
  const status = await runCli(['--e2e-status']);
  const runningAfter = status.json?.running ?? [];
  const lab2Before = runningBefore.find((r) => String(r.home).endsWith('lab2'));
  const lab2After = runningAfter.find((r) => String(r.home).endsWith('lab2'));
  record(
    'A5 改指针后原实例仍指向自己的 home',
    Boolean(lab2Before) && lab2Before?.shell === lab2After?.shell && String(lab2After?.home).endsWith('lab2'),
    `before=${JSON.stringify(lab2Before)} after=${JSON.stringify(lab2After)}`
  );
  writeFileSync(pointerFile, JSON.stringify({ home: beforeA5 }, null, 2) + '\n', 'utf8');

  // ---- A6：主进程启动即退出 → E1006（越过 20 秒宽限期）+ 指针回滚
  killStubProcesses();
  const beforeA6 = pointerHome();
  const r7 = await runCli(['--e2e-launch', 'exit'], { STUB_EXIT_CODE: '7' });
  record(
    'A6 启动即退出 E1006',
    r7.json?.ok === false && /E1006/.test(r7.text),
    `tail=${(r7.text || '').slice(-160).replace(/\n/g, ' ')}`
  );
  record('A6 指针回滚', pointerHome() === beforeA6, `pointer=${pointerHome()}`);

  // ---- 停止：替身没有窗口，只能用强制结束（真实 DSH 走 15 秒优雅关闭）
  killStubProcesses();
  await runCli(['--e2e-launch', 'lab1']);
  await runCli(['--e2e-launch', 'lab2']);
  const stop1 = await runCli(['--e2e-stop', 'lab1', '--force']);
  const stop2 = await runCli(['--e2e-stop', 'lab2', '--force']);
  record('强制停止 lab1', stop1.json?.ok === true, JSON.stringify(stop1.json));
  record('强制停止 lab2', stop2.json?.ok === true, JSON.stringify(stop2.json));

  const settled = await runCli(['--e2e-status']);
  record(
    '停止后没有残留的 e2e 实例',
    (settled.json?.running ?? []).filter((r) => String(r.home).includes('homes')).length === 0,
    `running=${JSON.stringify((settled.json?.running ?? []).map((r) => r.home))}`
  );

  writeFileSync(pointerFile, originalPointer, 'utf8');
  record('指针文件恢复为测试前内容', readFileSync(pointerFile, 'utf8') === originalPointer);

  killStubProcesses();

  const failed = results.filter((r) => !r.ok);
  writeFileSync(
    join(dataDir, 'e2e-report.json'),
    JSON.stringify({ at: new Date().toISOString(), runDir: e2e, ok: failed.length === 0, results }, null, 2) + '\n',
    'utf8'
  );
  log(`\n[e2e] ${results.length - failed.length}/${results.length} 通过；报告 ${join(dataDir, 'e2e-report.json')}`);
  if (failed.length > 0) process.exit(1);
}

main().catch((err) => {
  console.error(err);
  killStubProcesses();
  process.exit(1);
});
