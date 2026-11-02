// 只读取证：DSH-NEXT 的重启 / 安全模式 / 指针相关实现。
import { openAsar } from './asar-lib.mjs';

const ASAR = process.env.ASAR ?? 'E:\\FXS\\APP\\DSH NEXT\\resources\\app.asar';
const asar = openAsar(ASAR);

const files = asar
  .list()
  .filter((e) => e.type === 'file' && /\.(js|cjs|mjs|json)$/i.test(e.path) && !e.path.includes('.map'));

// 主进程入口
let mainFile = null;
try {
  const pkg = JSON.parse(asar.readText('package.json'));
  mainFile = pkg.main;
  console.log('main =', mainFile, ' version =', pkg.version);
} catch (e) {
  console.log('package.json 读取失败:', e.message);
}

const want = [/relaunch/gi, /safe[-_]?mode/gi, /recovery/gi, /desktop-next-location/gi, /--user-data-dir/g];
const names = ['relaunch', 'safe', 'recovery', 'pointer', 'userdata'];

const hits = [];
for (const f of files) {
  if (f.path.startsWith('node_modules/')) continue;
  let text;
  try {
    text = asar.readText(f.path);
  } catch {
    continue;
  }
  if (!text) continue;
  const counts = want.map((re) => (text.match(re) ?? []).length);
  if (counts.some((c) => c > 0)) {
    hits.push({ file: f.path, counts, size: text.length });
  }
}

hits.sort((a, b) => b.counts[0] + b.counts[1] + b.counts[2] - (a.counts[0] + a.counts[1] + a.counts[2]));
for (const h of hits.slice(0, 20)) {
  console.log(
    h.file.padEnd(60),
    names.map((n, i) => `${n}=${h.counts[i]}`).join(' '),
    `(${h.size}B)`
  );
}
