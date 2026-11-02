#!/usr/bin/env node
// verify-tokens.mjs —— 原生对齐门（设计说明 §7.3 / 实施计划 §7）。
//
// 四条判据，任一不过 → 退出码 1：
//   A. src/**/*.{css,svelte} 里每个 var(--x) 必须出现在 design/tokens.lock.json 的 token 名册；
//   B. tokens.css 必须与锁里记录的 sha256 一致（也就是“重新抽取不会有变化”）；
//   C. design/parts/*.css 逐份 sha256 必须与锁一致（改一个字节就红）；
//   D. 手写样式里不允许出现 hex / rgb() / hsl()（值只能来自 token）。
//
// 用法：node tools/verify-tokens.mjs [--lock design/tokens.lock.json] [--src src]

import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, relative, resolve, extname, basename } from 'node:path';

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i].startsWith('--')) out[argv[i].slice(2)] = argv[i + 1];
  }
  return out;
}

const args = parseArgs(process.argv.slice(2));
const root = process.cwd();
const lockPath = resolve(args.lock ?? 'design/tokens.lock.json');
const srcDir = resolve(args.src ?? 'src');
const partsDir = resolve('design/parts');

const failures = [];
const warnings = [];
const stats = { files: 0, refs: 0, tokens: 0, parts: 0 };

function fail(rule, message) {
  failures.push({ rule, message });
}

function sha256(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

if (!existsSync(lockPath)) {
  console.error(`[verify] 缺少基线：${lockPath}`);
  process.exit(2);
}

const lock = JSON.parse(readFileSync(lockPath, 'utf8'));
const tokenNames = new Set((lock.tokens ?? []).map((t) => t.name));
stats.tokens = tokenNames.size;

// 名册完整性：每个 token 必须有 pkg/file/symbol 出处
if (!Array.isArray(lock.tokens) || lock.tokens.length === 0) {
  fail('roster', 'tokens.lock.json 里没有 token');
}
for (const token of lock.tokens ?? []) {
  if (!token.sources || token.sources.length === 0) {
    fail('roster', `${token.name} 没有来源（pkg/file/symbol）`);
    continue;
  }
  for (const s of token.sources) {
    if (!s.pkg || !s.file || !s.symbol) {
      fail('roster', `${token.name} 的来源字段不全：${JSON.stringify(s)}`);
    }
  }
}

const undefinedRefs = new Set((lock.undefinedRefs ?? []).map((r) => r.name));

// ---------------------------------------------------------------- A / D

const GENERATED_CSS = resolve('src/styles/tokens.css');
const files = existsSync(srcDir)
  ? walk(srcDir).filter((f) => ['.css', '.svelte'].includes(extname(f).toLowerCase()))
  : [];

/** 只看 <style> 块，避免把 TS/模板文本当成样式 */
function styleSegments(file, text) {
  if (extname(file).toLowerCase() === '.css') return [{ css: text, offset: 0 }];
  const out = [];
  const re = /<style[^>]*>([\s\S]*?)<\/style>/gi;
  let m;
  while ((m = re.exec(text)) !== null) {
    out.push({ css: m[1], offset: m.index });
  }
  return out;
}

for (const file of files) {
  const rel = relative(root, file).replace(/\\/g, '/');
  if (resolve(file) === GENERATED_CSS) continue; // 生成文件单独校验（规则 B）
  stats.files++;
  const text = readFileSync(file, 'utf8');
  for (const seg of styleSegments(file, text)) {
    // 先剥掉注释，避免把注释里的示例当成真实用法
    const css = seg.css.replace(/\/\*[\s\S]*?\*\//g, '');
    // A：var() 必须来自名册
    for (const m of css.matchAll(/var\(\s*(--[a-zA-Z0-9-]+)/g)) {
      stats.refs++;
      const name = m[1];
      if (tokenNames.has(name)) continue;
      if (undefinedRefs.has(name)) {
        warnings.push({ rule: 'A', message: `${rel} 用了原生未定义的 ${name}（锁里已记录，靠运行时注入）` });
        continue;
      }
      fail('A', `${rel} 用了自造变量 ${name}（不在 tokens.lock.json）`);
    }
    // D：禁 hex / rgb / hsl
    for (const m of css.matchAll(/#[0-9a-fA-F]{3,8}\b/g)) {
      fail('D', `${rel} 出现 hex 颜色 ${m[0]}（只允许 var(--…)）`);
    }
    for (const m of css.matchAll(/\b(rgba?|hsla?)\(/g)) {
      fail('D', `${rel} 出现函数式颜色 ${m[0]}（只允许 var(--…)）`);
    }
  }
}

// ---------------------------------------------------------------- B

function verifyTokensCss() {
  const expected = lock.generatedCss;
  if (!expected) {
    fail('B', 'tokens.lock.json 没有 generatedCss 记录（先跑 tools/extract-tokens.mjs --css src/styles/tokens.css）');
    return;
  }
  if (!existsSync(GENERATED_CSS)) {
    fail('B', '缺少 src/styles/tokens.css（用 extract-tokens.mjs --css 生成）');
    return;
  }
  const actual = sha256(readFileSync(GENERATED_CSS));
  if (actual !== expected.sha256) {
    fail('B', `src/styles/tokens.css 与基线不一致（期望 ${expected.sha256.slice(0, 12)}…，实际 ${actual.slice(0, 12)}…）`);
  }
}

verifyTokensCss();

// ---------------------------------------------------------------- C

function verifyParts() {
  const parts = lock.parts ?? [];
  if (parts.length === 0) {
    fail('C', '锁里没有 parts');
    return;
  }
  const expected = new Map(parts.map((p) => [p.name, p.sha256]));
  for (const [name, sha] of expected) {
    const file = join(partsDir, name);
    if (!existsSync(file)) {
      fail('C', `缺少 design/parts/${name}`);
      continue;
    }
    const actual = sha256(readFileSync(file));
    if (actual !== sha) fail('C', `design/parts/${name} 被改动（期望 ${sha.slice(0, 12)}…，实际 ${actual.slice(0, 12)}…）`);
    stats.parts++;
  }
  if (existsSync(partsDir)) {
    for (const file of readdirSync(partsDir)) {
      if (!expected.has(file)) fail('C', `design/parts/${file} 不在基线里（多出来的文件）`);
    }
  }
}

verifyParts();

// ---------------------------------------------------------------- 报告

const report = {
  ok: failures.length === 0,
  lock: relative(root, lockPath).replace(/\\/g, '/'),
  tokens: stats.tokens,
  scannedFiles: stats.files,
  varRefs: stats.refs,
  parts: stats.parts,
  failures,
  warnings
};

console.log(JSON.stringify(report, null, 2));

if (failures.length > 0) {
  console.error(`\n[verify] FAIL：${failures.length} 处不合格`);
  process.exit(1);
}
console.log(`\n[verify] PASS：${stats.refs} 个 var() 全部来自名册；${stats.parts} 份 parts 未改动`);
