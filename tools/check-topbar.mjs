#!/usr/bin/env node
// check-topbar.mjs —— 顶部 chrome 的静态门（不需要真窗口，CI 可跑）。
//
// 守三件事：
//   T1 提示条与指针条：必须 flex:none + 固定 height、padding "0 20px"，且两条高度相等
//   T2 提示条不得再有 min-height 兜底（它允许被压到 30px，正是漂移的来源）
//   T3 横幅上限还在（App.svelte 的 MAX_BANNERS + slice(0, MAX_BANNERS)）
//
// 判据来自实测：同一构建在三个视图里出现过 44 / 40 / 30 三种高度，
// 指针条因为没有下限，甚至可能被压到裁掉路径。
// 用法：node tools/check-topbar.mjs

import { readFileSync } from 'node:fs';

const CSS = 'src/styles/chrome.css';
const APP = 'src/App.svelte';

const css = readFileSync(CSS, 'utf8');
const app = readFileSync(APP, 'utf8');
const failures = [];

function block(source, selector) {
  const idx = source.indexOf(selector);
  if (idx < 0) return null;
  const end = source.indexOf('}', idx);
  return end < 0 ? null : source.slice(idx, end);
}

const notice = block(css, '.notice {');
const pointer = block(css, '.pointerbar {');

function heightOf(text) {
  const m = text?.match(/(?:^|\s)height:\s*([\d.]+px);/);
  return m ? m[1] : null;
}

for (const [name, text] of [
  ['.notice', notice],
  ['.pointerbar', pointer]
]) {
  if (!text) {
    failures.push(`T1 ${name} 块不存在`);
    continue;
  }
  if (!/flex:\s*none;/.test(text)) failures.push(`T1 ${name} 缺 flex: none（会被 flex 列压缩）`);
  if (!/height:\s*[\d.]+px;/.test(text)) failures.push(`T1 ${name} 缺固定 height`);
  const pad = text.match(/padding:\s*([^;]+);/);
  if (!pad) {
    failures.push(`T1 ${name} 缺 padding`);
  } else if (pad[1].trim() !== '0 20px') {
    failures.push(`T1 ${name} 的 padding 应为 "0 20px"（通栏 + 垂直居中），实际 "${pad[1].trim()}"`);
  }
}

const hn = heightOf(notice);
const hp = heightOf(pointer);
if (hn && hp && hn !== hp) failures.push(`T1 提示条与指针条高度不一致：${hn} vs ${hp}`);

if (/\.notice-ticker\s*\{[^}]*min-height/.test(css)) {
  failures.push('T2 .notice-ticker 又出现 min-height 兜底（会允许被压到该值）');
}

const cap = app.match(/const MAX_BANNERS = (\d+);/);
if (!cap) failures.push('T3 找不到 MAX_BANNERS');
if (!/\.slice\(0, MAX_BANNERS\)/.test(app)) failures.push('T3 MAX_BANNERS 没有被用于截断（上限失效）');

if (failures.length) {
  for (const f of failures) console.error(`[topbar] FAIL  ${f}`);
  process.exit(1);
}
console.log(
  `[topbar] PASS：两条 chrome 均 flex:none / ${hn} 高 / padding 0 20px；横幅上限 ${cap[1]} 条`
);