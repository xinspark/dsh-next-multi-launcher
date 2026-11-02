// 端到端 UI 验证：打开「新建实例」，输入中文名，读取 ID / home 实际值。
// 用法：先 `pnpm dev`，再 `node tools/ui-check.mjs`
import { spawn } from 'node:child_process';
import { setTimeout as sleep } from 'node:timers/promises';

const EDGE = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const URL = process.env.APP_URL ?? 'http://localhost:1420';
const PORT = 9340;
const NAME = process.env.CHECK_NAME ?? '灵枢-汐';

const edge = spawn(
  EDGE,
  [
    '--headless=new',
    '--disable-gpu',
    `--remote-debugging-port=${PORT}`,
    '--user-data-dir=' + process.env.TEMP + '\\dshla-ui-check',
    '--window-size=1120,760',
    URL
  ],
  { stdio: 'ignore' }
);

async function wsUrl() {
  for (let i = 0; i < 60; i++) {
    try {
      const res = await fetch(`http://127.0.0.1:${PORT}/json/list`);
      const list = await res.json();
      const page = list.find((t) => t.type === 'page' && t.url.includes('localhost:1420'));
      if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
    } catch {
      /* not ready */
    }
    await sleep(250);
  }
  throw new Error('CDP 没有就绪（先启动 pnpm dev）');
}

const ws = new WebSocket(await wsUrl());
let seq = 0;
const pending = new Map();
ws.addEventListener('message', (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  }
});
await new Promise((r) => ws.addEventListener('open', r, { once: true }));

function send(method, params = {}) {
  const id = ++seq;
  return new Promise((resolve) => {
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });
}
async function ev(expression) {
  const res = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (res.result?.exceptionDetails) throw new Error(res.result.exceptionDetails.text);
  return res.result?.result?.value ?? null;
}

await send('Runtime.enable');
await sleep(4000);

await ev(`(() => {
  const btn = [...document.querySelectorAll('button')].find((b) => (b.textContent || '').includes('新建实例'));
  btn?.click();
  return !!btn;
})()`);
await sleep(900);

const read = `(() => {
  const modal = document.querySelector('.modal');
  if (!modal) return { error: 'no-modal' };
  return [...modal.querySelectorAll('input')].map((i) => i.value);
})()`;

const before = await ev(read);

await ev(`(() => {
  const modal = document.querySelector('.modal');
  const nameInput = modal?.querySelector('input');
  if (!nameInput) return 'no-input';
  const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
  setter.call(nameInput, ${JSON.stringify(NAME)});
  nameInput.dispatchEvent(new Event('input', { bubbles: true }));
  return 'typed';
})()`);
await sleep(700);

const after = await ev(read);

const ok = Array.isArray(after) && /^inst-[a-z0-9]{8}$/.test(after[1] ?? '') && (after[2] ?? '').endsWith(after[1]);
console.log(JSON.stringify({ name: NAME, before, after, ok }, null, 2));

ws.close();
edge.kill();
process.exit(ok ? 0 : 1);
