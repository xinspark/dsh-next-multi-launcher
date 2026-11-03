#!/usr/bin/env node
// extract-tokens.mjs —— 阶段 1 取证工具（设计说明 §7.1 / 实施计划 §2）
//
// 从 DSH-NEXT 的 resources/app.asar 里抽取原生设计变量（`--dsw-*` / `--ds-*` / `--dsh-*` / `--json-*`），
// 连同来源（包/lib/文件.js:符号）与主题域（light / dark）一起固化到 design/tokens.lock.json；
// 同时把可复用的原生片段原样复制到 design/parts/（逐份算 sha256），并生成 controls.md。
//
// 用法：
//   node tools/extract-tokens.mjs --out design/tokens.lock.json [--parts design/parts]
//                                 [--controls design/controls.md] [--css src/styles/tokens.css]
//                                 [--asar "<DSH-NEXT 安装目录>\\resources\\app.asar"]
//
// 约等于（验收）：两次运行输出逐字节相同 —— 所以本文件不写时间戳、全量排序、只写下确定性内容。

import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync, readFileSync, existsSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve } from 'node:path';
import { openAsar } from './asar-lib.mjs';

/**
 * 解析 DSH-NEXT 的 app.asar：--asar（调用处优先）→ DSH_ASAR → 常见安装目录。
 * 装到别处就显式指路；这里不猜路径，也绝不写死某台机器的安装位置。
 */
function resolveAsar() {
  const candidates = [
    process.env.DSH_ASAR,
    process.env.LOCALAPPDATA && join(process.env.LOCALAPPDATA, 'Programs', 'DSH NEXT', 'resources', 'app.asar'),
    process.env.ProgramFiles && join(process.env.ProgramFiles, 'DSH NEXT', 'resources', 'app.asar'),
    process.env['ProgramFiles(x86)'] && join(process.env['ProgramFiles(x86)'], 'DSH NEXT', 'resources', 'app.asar'),
  ].filter(Boolean);
  return candidates.find((p) => existsSync(p)) ?? null;
}

const DEFAULT_ASAR = resolveAsar();

/**
 * 记录出处时写**可移植**路径：仓内文件写相对路径，仓外写 <install>/…，
 * 免得把某台机器的安装位置固化进 lock 文件（它会随仓库发布）。
 */
function portablePath(p) {
  const rel = relative(process.cwd(), p);
  if (rel && !rel.startsWith('..') && !isAbsolute(rel)) return rel.replace(/\\/g, '/');
  return `<install>/${p.split(/[\\/]/).slice(-2).join("/")}`;
}

// ---------------------------------------------------------------- CLI

function parseArgs(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith('--')) {
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith('--')) {
        out[key] = next;
        i++;
      } else out[key] = true;
    } else out._.push(a);
  }
  return out;
}

const args = parseArgs(process.argv.slice(2));
const asarPath = args.asar ?? DEFAULT_ASAR;
if (!asarPath) {
  console.error('[tokens] 找不到 DSH-NEXT 的 app.asar：请用 --asar <路径> 或设置 DSH_ASAR');
  process.exit(2);
}
const outPath = resolve(args.out ?? 'design/tokens.lock.json');
const partsDir = resolve(args.parts ?? 'design/parts');
const controlsPath = resolve(args.controls ?? 'design/controls.md');
const cssOut = args.css ? resolve(args.css) : null;

if (!existsSync(asarPath)) {
  console.error(`[extract] app.asar 不存在：${asarPath}`);
  process.exit(2);
}

// ---------------------------------------------------------------- 工具

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');

/** 来源文件 → { pkg, file } */
function describeSource(entryPath) {
  const m = entryPath.match(/^node_modules\/((?:@[^/]+\/)?[^/]+)\/(.*)$/);
  if (m) return { pkg: m[1], file: m[2] };
  const top = entryPath.split('/')[0];
  return { pkg: top, file: entryPath.slice(top.length + 1) };
}

/** 值类型判定（只做形状分类，不改值） */
function valueTypeOf(value) {
  const v = value.trim();
  if (/^#[0-9a-fA-F]{3,8}$/.test(v)) return 'color';
  if (/^(rgb|rgba|hsl|hsla|color-mix|color|lab|lch|oklch)\(/i.test(v)) return 'color';
  if (/^var\(--[\w-]+\)$/.test(v)) return 'alias';
  if (/^-?\d*\.?\d+(px|rem|em|vw|vh|%|ch|pt)$/i.test(v)) return 'length';
  if (/^-?\d*\.?\d+(ms|s)$/i.test(v)) return 'duration';
  if (/^(cubic-bezier|steps)\(/i.test(v) || /^(ease|ease-in|ease-out|ease-in-out|linear)$/i.test(v)) return 'easing';
  if (/-apple-system|BlinkMacSystemFont|"Segoe UI"|PingFang|sans-serif|monospace/.test(v)) return 'font';
  if (/^-?\d*\.?\d+$/.test(v)) return 'number';
  if (/^(superellipse|inset|drop-shadow)/.test(v)) return 'other';
  return 'other';
}

/** 解开 JS 字符串字面量（Vite/ESBuild 产物里的 CSS 就是这种形态） */
function decodeJsString(literal) {
  try {
    return JSON.parse(`"${literal.replace(/\n/g, '\\n')}"`);
  } catch {
    return literal
      .replace(/\\"/g, '"')
      .replace(/\\'/g, "'")
      .replace(/\\\\/g, '\\')
      .replace(/\\n/g, '\n')
      .replace(/\\t/g, '\t');
  }
}

/**
 * 解析一段 CSS，产出 [{ name, value, scope }]。
 * scope: 'light' | 'dark'（选择器或 @media 命中暗色即 dark）。
 * 只认自定义属性声明 `--x: v;`，不展开 var()。
 */
function parseCssDeclarations(css) {
  const out = [];
  // 逐字符扫描，维护选择器栈（支持 @media / @supports 嵌套）
  const stack = []; // { selector, dark }
  let buf = '';
  let i = 0;
  const n = css.length;
  while (i < n) {
    const c = css[i];
    if (c === '{') {
      const selector = buf.trim();
      buf = '';
      const parentDark = stack.some((s) => s.dark);
      const isDark = parentDark || inferDark(selector);
      stack.push({ selector, dark: isDark, isAtRule: selector.startsWith('@') });
      i++;
      continue;
    }
    if (c === '}') {
      // 块结束：先把块内剩余内容当作无冒号尾巴丢弃
      buf = '';
      stack.pop();
      i++;
      continue;
    }
    if (c === ';') {
      const decl = buf.trim();
      buf = '';
      i++;
      if (!decl.startsWith('--')) continue;
      const idx = decl.indexOf(':');
      if (idx <= 2) continue;
      const name = decl.slice(0, idx).trim();
      const value = decl.slice(idx + 1).trim();
      if (!/^--[a-z0-9-]+$/i.test(name) || value === '') continue;
      const dark = stack.some((s) => s.dark);
      out.push({ name, value, scope: dark ? 'dark' : 'light' });
      continue;
    }
    // `{}` 内的选择器/声明都往 buf 里堆
    buf += c;
    i++;
  }
  return out;
}

function inferDark(selector) {
  if (!selector) return false;
  if (/data-ds-dark-theme|data-theme=["']?dark|prefers-color-scheme\s*:\s*dark|\.dark\b/i.test(selector)) return true;
  return false;
}

// ---------------------------------------------------------------- 取源

const asar = openAsar(asarPath);
const asarSha = asar.sha256File();
const allEntries = asar.list('').sort();

const CSS_ENTRY = /\.css$/i;
const JS_ENTRY = /\.(js|mjs|cjs)$/i;
// 只扫“可能定义变量”的源：全部 CSS + 客户端 UI 包 / 宿主 native-ui 的 JS。
const JS_ALLOW = /(dsh-client-ui-theme|dsh-client-ui-primitives|dsh-client-ui-[\w-]*|lib\/native-ui|lib\/web-document\.js)/;

const entries = allEntries.filter((k) => {
  const e = asar.entries.get(k);
  if (!e || e.size === 0 || e.size > 8 * 1024 * 1024) return false; // 单文件上限 8MB，防误扫大包
  if (CSS_ENTRY.test(k)) return true;
  if (JS_ENTRY.test(k) && JS_ALLOW.test(k)) return true;
  return false;
});

const occurrences = [];
const skipped = [];

for (const entryPath of entries) {
  let text;
  try {
    text = asar.readText(entryPath);
  } catch (err) {
    skipped.push({ path: entryPath, reason: `读取失败：${err.message}` });
    continue;
  }
  const src = describeSource(entryPath);

  if (CSS_ENTRY.test(entryPath)) {
    const decls = parseCssDeclarations(text);
    for (const d of decls) occurrences.push({ ...d, source: { ...src, symbol: entryPath.split('/').pop() } });
    continue;
  }

  // JS：抽出内联 CSS 字符串（含 `--x: v;` 的字符串字面量），并用区域注释命名符号
  const regionRe = /\/\/#region [^\n]*?([\w.-]+)\.css\.mjs/g;
  const regions = [...text.matchAll(regionRe)].map((m) => ({ index: m.index, name: m[1] }));
  const strRe = /"((?:[^"\\\n]|\\.)*)"/g;
  let m;
  while ((m = strRe.exec(text)) !== null) {
    const raw = m[1];
    if (!raw.includes('--')) continue;
    if (!/--[a-z0-9-]+\s*:/.test(raw)) continue;
    const css = decodeJsString(raw);
    if (!/--[a-z0-9-]+\s*:/.test(css)) continue;
    const pos = m.index;
    let symbol = null;
    for (let r = regions.length - 1; r >= 0; r--) {
      if (regions[r].index < pos) {
        symbol = regions[r].name + '.css';
        break;
      }
    }
    const decls = parseCssDeclarations(css);
    if (decls.length === 0) continue;
    // 同一段 CSS 里通常 light/dark 各一块
    for (const d of decls) occurrences.push({ ...d, source: { ...src, symbol: symbol ?? 'inline-css' } });
  }
}

// ---------------------------------------------------------------- 归并

/** 来源优先级：主题包 > primitives > 其它，用于同名 token 冲突时取主值 */
function sourceRank(s) {
  if (s.pkg === '@deepseek-ai/dsh-client-ui-theme') return 0;
  if (s.pkg === '@deepseek-ai/dsh-client-ui-primitives') return 1;
  if (s.pkg === 'lib') return 2;
  return 3;
}

const byName = new Map();
for (const occ of occurrences) {
  let t = byName.get(occ.name);
  if (!t) {
    t = { name: occ.name, valueType: valueTypeOf(occ.value), scopes: new Set(), values: {}, sources: [] };
    byName.set(occ.name, t);
  }
  t.scopes.add(occ.scope);
  const key = `${occ.scope}:${sourceRank(occ.source)}:${occ.source.file}:${occ.source.symbol}`;
  const prev = t.values[occ.scope];
  if (!prev || key < prev.key) {
    t.values[occ.scope] = { key, value: occ.value, source: occ.source };
  }
  t.sources.push(occ.source);
}

const tokens = [...byName.values()]
  .map((t) => ({
    name: t.name,
    valueType: t.valueType,
    scopes: [...t.scopes].sort(),
    value: {
      light: t.values.light?.value ?? null,
      dark: t.values.dark?.value ?? null
    },
    sources: dedupeSources(t.sources)
  }))
  .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));

function dedupeSources(list) {
  const seen = new Map();
  for (const s of list) {
    const k = `${s.pkg}|${s.file}|${s.symbol}`;
    if (!seen.has(k)) seen.set(k, { pkg: s.pkg, file: s.file, symbol: s.symbol });
  }
  return [...seen.values()].sort((a, b) =>
    `${a.pkg}|${a.file}|${a.symbol}` < `${b.pkg}|${b.file}|${b.symbol}` ? -1 : 1
  );
}

if (tokens.length === 0) {
  console.error('[extract] 没有抽到任何 token，拒绝写出空基线');
  process.exit(3);
}

// ---------------------------------------------------------------- parts

const PART_SOURCES = [
  // [输出名, 来源 entry 或 {inline: 包路径}, 说明]
  { name: 'scrollbar.css', from: 'node_modules/@deepseek-ai/dsh-client-ui-theme/lib/client.js', inline: 'scrollbar', note: '原生滚动条' },
  { name: 'focus.css', from: 'node_modules/@deepseek-ai/dsh-client-ui-theme/lib/client.js', inline: 'focus', note: '焦点环' },
  { name: 'corner-shape.css', from: 'node_modules/@deepseek-ai/dsh-client-ui-theme/lib/client.js', inline: 'corner-shape', note: '超椭圆圆角' },
  { name: 'Menu.module.css', from: 'node_modules/@deepseek-ai/dsh-client-ui-primitives/lib/Menu.module.css', note: '菜单表面' },
  { name: 'HoverCard.module.css', from: 'node_modules/@deepseek-ai/dsh-client-ui-primitives/lib/HoverCard.module.css', note: '悬浮卡表面' }
];

function extractInlineCss(clientJs, symbolFragment) {
  const text = asar.readText(clientJs);
  const re = /\/\/#region [^\n]*?([\w.-]+)\.css\.mjs[\s\S]*?var \w+ = "((?:[^"\\]|\\.)*)";/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    if (m[1] === symbolFragment) return decodeJsString(m[2]);
  }
  return null;
}

mkdirSync(partsDir, { recursive: true });
const parts = [];
for (const p of PART_SOURCES) {
  let content;
  try {
    content = p.inline ? extractInlineCss(p.from, p.inline) : asar.readText(p.from);
  } catch (err) {
    skipped.push({ path: p.from, reason: `parts 读取失败：${err.message}` });
    continue;
  }
  if (!content) {
    skipped.push({ path: p.from, reason: `未找到内联片段 ${p.inline}` });
    continue;
  }
  const bytes = Buffer.from(content, 'utf8');
  writeFileSync(join(partsDir, p.name), bytes);
  parts.push({ name: p.name, sha256: sha256(bytes), bytes: bytes.length, note: p.note, source: p.from });
}

// ---------------------------------------------------------------- controls（控件规格）

function findToken(re) {
  return tokens.find((t) => re.test(t.name));
}

function tokenSpec(label, re, note) {
  const t = findToken(re);
  if (!t) {
    skipped.push({ path: 'controls', reason: `未找到控件规格 token ${label}（${re}）` });
    return null;
  }
  return {
    label,
    token: t.name,
    value: t.value.light ?? t.value.dark,
    dark: t.value.dark,
    note,
    source: t.sources[0]
  };
}

const radiusScale = tokens
  .filter((t) => /^--dsw-radius-/.test(t.name))
  .sort((a, b) => String(a.value.light).localeCompare(String(b.value.light), 'en', { numeric: true }))
  .map((t) => ({ label: t.name.replace('--dsw-radius-', ''), token: t.name, value: t.value.light, source: t.sources[0] }));

const controls = [
  tokenSpec('卡片圆角', /^--dsw-radius-lg$/, '设置页卡片圆角 16px'),
  tokenSpec('行圆角', /^--dsw-radius-md$/, '列表行 / 输入框圆角 12px'),
  tokenSpec('面板圆角', /^--dsw-radius-panel$/, '浮层 / 面板圆角 28px'),
  tokenSpec('小圆角', /^--dsw-radius-xs$/, '徽标 / 标签圆角 4px')
].filter(Boolean);

// 按钮高度 / chevron 尺寸：从 primitives CSS 里量（量不到就进 skipped，不猜）
const metricSources = [
  'node_modules/@deepseek-ai/dsh-client-ui-primitives/lib/Menu.module.css',
  'node_modules/@deepseek-ai/dsh-client-ui-primitives/lib/HoverCard.module.css',
  'node_modules/@deepseek-ai/dsh-client-ui-primitives/lib/FileTypeIcon.module.css'
];
const metrics = [];
for (const path of metricSources) {
  let text;
  try {
    text = asar.readText(path);
  } catch {
    continue;
  }
  const heights = [...text.matchAll(/height:\s*(\d+)px/g)].map((m) => Number(m[1]));
  const rads = [...text.matchAll(/border-radius:\s*([\w.-]+)/g)].map((m) => m[1]);
  if (heights.length) metrics.push({ file: path.split('/').pop(), heights: [...new Set(heights)].sort((a, b) => a - b) });
  if (rads.length) metrics.push({ file: path.split('/').pop(), radii: [...new Set(rads)].sort() });
}

if (metrics.length === 0) skipped.push({ path: 'controls', reason: '未从 primitives CSS 量到按钮高度 / 圆角' });

// ---------------------------------------------------------------- 未定义引用（诚实记录）

const tokenNames = new Set(tokens.map((t) => t.name));
const undefinedRefs = new Map();
for (const t of tokens) {
  for (const v of [t.value.light, t.value.dark]) {
    if (!v) continue;
    for (const m of String(v).matchAll(/var\((--[a-z0-9-]+)/gi)) {
      if (!tokenNames.has(m[1])) undefinedRefs.set(m[1], (undefinedRefs.get(m[1]) ?? 0) + 1);
    }
  }
}
const undefinedList = [...undefinedRefs.entries()]
  .map(([name, refs]) => ({ name, refs }))
  .sort((a, b) => (a.name < b.name ? -1 : 1));

// ---------------------------------------------------------------- 写 lock

const lock = {
  schemaVersion: 1,
  tool: 'tools/extract-tokens.mjs@1',
  sourceAsar: { path: portablePath(asarPath), sha256: asarSha, entries: allEntries.length },
  counts: {
    cssSources: entries.filter((e) => CSS_ENTRY.test(e)).length,
    jsSources: entries.filter((e) => !CSS_ENTRY.test(e)).length,
    tokens: tokens.length,
    parts: parts.length
  },
  tokens,
  controls: { radiusScale, controls, metrics },
  parts,
  skipped
};

// 先攒内容，等 CSS 生成后再一起落盘（见文件末尾）

// ---------------------------------------------------------------- 生成 CSS（可选）

if (cssOut) {
  const light = tokens.filter((t) => t.value.light !== null);
  const dark = tokens.filter((t) => t.value.dark !== null);
  const decl = (t) => `  ${t.name}: ${t.value.light ?? t.value.dark};`;
  const darkDecl = (t) => `  ${t.name}: ${t.value.dark};`;
  const lines = [];
  lines.push('/* 由 tools/extract-tokens.mjs 从 DSH-NEXT app.asar 生成，请勿手改。');
  lines.push(`   来源：${portablePath(asarPath)}`);
  lines.push(`   asar sha256：${asarSha}`);
  lines.push(`   token 数：${tokens.length}（light ${light.length} / dark ${dark.length}） */`);
  lines.push('');
  lines.push(':root {');
  lines.push(...light.map(decl));
  lines.push('}');
  lines.push('');
  lines.push('[data-theme="dark"] {');
  lines.push(...dark.map(darkDecl));
  lines.push('}');
  lines.push('');
  lines.push('@media (prefers-color-scheme: dark) {');
  lines.push('  :root:not([data-theme="light"]) {');
  lines.push(...dark.map((t) => `    ${t.name}: ${t.value.dark};`));
  lines.push('  }');
  lines.push('}');
  lines.push('');
  const cssText = lines.join('\n');
  mkdirSync(dirname(cssOut), { recursive: true });
  writeFileSync(cssOut, cssText, 'utf8');
  lock.generatedCss = {
    path: portablePath(cssOut),
    sha256: sha256(Buffer.from(cssText, 'utf8')),
    bytes: Buffer.byteLength(cssText, 'utf8')
  };
}

// ---------------------------------------------------------------- controls.md

function mdTable(rows, headers) {
  const esc = (v) => String(v ?? '').replace(/\|/g, '\\|');
  const head = `| ${headers.join(' | ')} |`;
  const sep = `| ${headers.map(() => '---').join(' | ')} |`;
  return [head, sep, ...rows.map((r) => `| ${r.map(esc).join(' | ')} |`)].join('\n');
}

const controlsMd = [
  '# 控件规格（阶段 1 产物）',
  '',
  '> 由 `tools/extract-tokens.mjs` 从 DSH-NEXT `app.asar` 自动生成，请勿手改。',
  `> 基线 asar：\`${asarPath}\``,
  `> asar sha256：\`${asarSha}\``,
  '',
  '## 1. 圆角刻度',
  '',
  mdTable(
    radiusScale.map((r) => [r.label, `\`${r.token}\``, r.value, `${r.source.pkg}/${r.source.file}:${r.source.symbol}`]),
    ['档位', 'token', '值', '来源']
  ),
  '',
  '## 2. 控件规格',
  '',
  mdTable(
    controls.map((c) => [c.label, `\`${c.token}\``, c.value, c.note, `${c.source.pkg}/${c.source.file}:${c.source.symbol}`]),
    ['用途', 'token', 'light 值', '说明', '来源']
  ),
  '',
  '## 3. 实测尺寸（primitives CSS）',
  '',
  ...(metrics.length
    ? metrics.map((m) =>
        m.heights
          ? `- \`${m.file}\` 高度：${m.heights.map((h) => `${h}px`).join(' / ')}`
          : `- \`${m.file}\` 圆角：${m.radii.map((r) => `\`${r}\``).join(' / ')}`
      )
    : ['- （未量到，见 tokens.lock.json 的 skipped）']),
  '',
  '## 4. 复用片段（design/parts）',
  '',
  mdTable(
    parts.map((p) => [p.name, p.bytes, `\`${p.sha256}\``, p.note]),
    ['文件', '字节', 'sha256', '说明']
  ),
  '',
  '## 5. 抽不到的项（显式记录，不静默丢弃）',
  '',
  ...(skipped.length ? skipped.map((s) => `- \`${s.path}\`：${s.reason}`) : ['- 无']),
  ''
].join('\n');

mkdirSync(dirname(controlsPath), { recursive: true });
writeFileSync(controlsPath, controlsMd, 'utf8');

lock.undefinedRefs = undefinedList;

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, `${JSON.stringify(lock, null, 2)}\n`, 'utf8');

asar.close();

console.log(
  JSON.stringify(
    {
      out: outPath,
      css: cssOut,
      controls: controlsPath,
      parts: parts.length,
      tokens: tokens.length,
      skipped: skipped.length,
      undefinedRefs: undefinedList.length,
      cssSources: lock.counts.cssSources,
      jsSources: lock.counts.jsSources
    },
    null,
    2
  )
);
