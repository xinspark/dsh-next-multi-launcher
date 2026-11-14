#!/usr/bin/env node
// check-i18n.mjs —— 文案字典门禁（中英对照不许漂移）。
//
// 为什么需要它：字典是"手写两遍"的结构，任何一次新增文案都可能只补一边；
// 缺键不会报错（translate 会回落到 zh 或直接显示 key），只是一个别扭的中文/英文混排，
// 直到有用户截图来问。这类漂移必须变成门，而不是靠人记得。
//
// 五条规则（任一 FAIL 即退出码 1）：
//   R1 键集对称：zh 有而 en 无 / en 有而 zh 无
//   R2 同一字典内键不重复（后者会静默覆盖前者）
//   R3 占位符一致：同一键的 {var} 集合必须相同（否则 t(key, {x}) 只替换一半）
//   R4 en 值零中日韩字符（en 里出现中文 = 漏译，回落到中文不易被发现）
//      —— 白名单见 CJK_ALLOWED：语言切换按钮在英文界面里本来就该写「中文」
//   R5 代码里 t('...') 用到的键必须存在（拼错 key 会原样显示在界面上）
//
// 只做静态读取，不执行 TypeScript。
//
// 用法：node tools/check-i18n.mjs [--verbose]

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

const root = resolve('.');
const src = join(root, 'src');
const i18nFile = join(src, 'lib', 'i18n.ts');
const verbose = process.argv.includes('--verbose');

const failures = [];
const warnings = [];

// ---------------------------------------------------------------- 解析字典

/** 从 `const <name>: Dict = {` 到匹配的 `};`，认单行与「键一行、值在下一行」两种写法。 */
function parseDict(text, dictName) {
  const start = text.indexOf(`const ${dictName}: Dict = {`);
  if (start < 0) throw new Error(`找不到字典 const ${dictName}: Dict = {`);
  const body = text.slice(start);
  const end = body.indexOf('\n};');
  if (end < 0) throw new Error(`字典 ${dictName} 没有以 }; 结束`);
  const inner = body.slice(body.indexOf('{') + 1, end);

  const entries = [];
  // 键后面可以换行、缩进，再用单引号包住值（值内允许 \' 与 \" 转义）
  const re = /'((?:[^'\\]|\\.)+)'\s*:\s*'((?:[^'\\]|\\.)*)'/g;
  let m;
  while ((m = re.exec(inner))) {
    entries.push({ key: m[1].replace(/\\'/g, "'"), raw: m[2] });
  }
  // 值里面如果还有 'key': 'value' 形状（例如文案里出现英文引号对），会多算一条；
  // 用「键必须在行首缩进处」的约束复查一次（真实的键都写在缩进开头）。
  const keyRe = /^[ \t]*'((?:[^'\\]|\\.)+)'\s*:/gm;
  const declared = [...inner.matchAll(keyRe)].length;
  if (declared !== entries.length) {
    warnings.push(`${dictName}: 正则命中 ${entries.length} 条、行首键 ${declared} 条，以行首为准`);
  }
  return entries;
}

function placeholders(raw) {
  return [...raw.matchAll(/\{([a-zA-Z0-9_]+)\}/g)].map((m) => m[1]).sort().join(',');
}

const CJK = /[\u3000-\u303f\u3400-\u4dbf\u4e00-\u9fff\uff00-\uffef]/;

/** 英文界面里本来就该出现中文的键（语言切换按钮显示「中文」，不是漏译）。 */
const CJK_ALLOWED = new Set(['header.lang']);

const i18nText = readFileSync(i18nFile, 'utf8');
const zh = parseDict(i18nText, 'zh');
const en = parseDict(i18nText, 'en');

// R2 重复键
for (const [name, entries] of [
  ['zh', zh],
  ['en', en]
]) {
  const seen = new Map();
  for (const e of entries) {
    if (seen.has(e.key)) failures.push(`R2 ${name} 重复键 "${e.key}"（第 ${seen.get(e.key)} 与后一次出现）`);
    else seen.set(e.key, entries.indexOf(e) + 1);
  }
}

const zhMap = new Map(zh.map((e) => [e.key, e]));
const enMap = new Map(en.map((e) => [e.key, e]));

// R1 键集对称
for (const key of zhMap.keys()) if (!enMap.has(key)) failures.push(`R1 en 缺键 "${key}"`);
for (const key of enMap.keys()) if (!zhMap.has(key)) failures.push(`R1 zh 缺键 "${key}"`);

// R3 占位符一致 / R4 en 零中文
for (const [key, z] of zhMap) {
  const e = enMap.get(key);
  if (!e) continue;
  const pz = placeholders(z.raw);
  const pe = placeholders(e.raw);
  if (pz !== pe) {
    failures.push(`R3 占位符不一致 "${key}"：zh{${pz}} vs en{${pe}}`);
  }
  if (!z.raw.trim()) failures.push(`R1 zh 空值 "${key}"`);
  if (!e.raw.trim()) failures.push(`R1 en 空值 "${key}"`);
  if (CJK.test(e.raw) && !CJK_ALLOWED.has(key)) {
    failures.push(`R4 en 值含中文 "${key}"：${e.raw.slice(0, 40)}`);
  }
}

// ---------------------------------------------------------------- R5 代码里的键

function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) out.push(...walk(full));
    else if (/\.(svelte|ts)$/.test(name)) out.push(full);
  }
  return out;
}

const used = new Map(); // key -> 第一次出现的文件
for (const file of walk(src)) {
  const text = readFileSync(file, 'utf8');
  for (const m of text.matchAll(/\bt\(\s*'([a-zA-Z0-9_.]+)'/g)) {
    if (!used.has(m[1])) used.set(m[1], relative(root, file));
  }
}
for (const [key, file] of used) {
  if (!zhMap.has(key)) failures.push(`R5 代码引用了不存在的键 "${key}"（${file}）`);
}

const unused = [...zhMap.keys()].filter((k) => !used.has(k));

// ---------------------------------------------------------------- 输出

const summary = {
  zhKeys: zhMap.size,
  enKeys: enMap.size,
  referencedKeys: used.size,
  unusedKeys: unused.length,
  failures,
  warnings
};

if (verbose) console.log(JSON.stringify({ ...summary, unused }, null, 2));

if (failures.length) {
  console.error('[check-i18n] FAIL');
  for (const f of failures) console.error('  ' + f);
  if (warnings.length) for (const w of warnings) console.error('  warn: ' + w);
  process.exit(1);
}

// 未使用的键只提示：有些是为将来预留的，也可能是删代码后的残渣
if (unused.length) {
  console.log(`[check-i18n] 提示：${unused.length} 个键未被 t('…') 直接引用（可能是动态拼接或残渣）：`);
  console.log('  ' + unused.slice(0, 12).join(', ') + (unused.length > 12 ? ' …' : ''));
}
for (const w of warnings) console.log('  warn: ' + w);

console.log(
  `[check-i18n] PASS：zh ${zhMap.size} 键 / en ${enMap.size} 键一一对应，占位符一致，en 零中文，` +
    `代码引用 ${used.size} 键全部存在`
);
