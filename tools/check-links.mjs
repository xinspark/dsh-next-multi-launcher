import fs from 'node:fs';
import path from 'node:path';

const ROOT = '.';
const files = [];

function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (['node_modules', '.git', 'target', 'dist', 'src-tauri'].includes(entry.name)) continue;
      walk(p);
    } else if (entry.name.endsWith('.md')) {
      files.push(p);
    }
  }
}

// 只检查公开文档
files.push('README.md');
walk('docs');

const missing = [];
let checked = 0;

for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  const dir = path.dirname(file);
  // [label](target) —— 排除 http(s)、mailto、纯锚点
  const re = /\[[^\]]*\]\(([^)\s]+)\)/g;
  let m;
  while ((m = re.exec(text))) {
    let target = m[1];
    if (/^(https?:|mailto:|#)/.test(target)) continue;
    target = target.split('#')[0];
    if (!target) continue;
    checked++;
    const resolved = path.resolve(dir, target);
    if (!fs.existsSync(resolved)) {
      missing.push({ file, target });
    }
  }
}

console.log(`checked ${checked} relative links in ${files.length} files`);
if (missing.length === 0) {
  console.log('no missing links');
} else {
  for (const m of missing) console.log(`MISSING  ${m.file}  ->  ${m.target}`);
  process.exitCode = 1;
}
