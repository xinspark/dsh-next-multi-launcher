// lint-css.mjs — 样式门的唯一实现（A0）。
//
// 四条规则，全部来自对原生 DSH 的取证（见 docs-private/样式改造.md）：
//   R1 禁裸颜色      —— 只许 var(--dsw-*)，禁 #hex / rgb() / hsl()
//   R2 圆角只用 token —— 只许 var(--dsw-radius-*)，白名单 0 / 999px / 50%（胶囊与小圆点）
//   R3 字号成对      —— 字面量 font-size 必须落在原生确有、且无 token 的五档
//                      {10, 11.5, 12.5, 13.5, 15}，并且同一规则里必须带 line-height（或 font: 简写）
//   R3b 字重不出界   —— 原生只用 400/500/600，700 及以上报红
//
// 为什么 R3 只罚"落单"而不罚"写字面量"：原生详情页自己也写字面量（11 个字号档里 5 档没有 token），
// 真正的共同点是**每一处字号都跟着行高**。判据要跟着原生的事实走，而不是跟着"应该用 token"的直觉走。
//
// 用法：node tools/lint-css.mjs [--json]
// 退出码：0 = 全绿；1 = 有违规。

import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = process.cwd();
const SCAN_DIRS = ['src'];
const TOKEN_FILE = 'src/styles/tokens.css'; // 自动抽取产物，不参与本门
// 原生确有、且没有对应 token 的字号档（取证见 docs-private/样式改造.md §2.2）：
// 10 / 11.5 / 12.5 / 13.5 / 15 是详情页实测档位；18 是 wizardTitle 的 18/26
// （token --dsw-font-m-18 实际是 16/28，名字对不上值，所以只能写字面量）。
const PX_WHITELIST = new Set(['10', '11.5', '12.5', '13.5', '15', '18']);
const RADIUS_WHITELIST = new Set(['0', '999px', '50%', 'inherit', 'unset', 'auto']);
const WEIGHT_ALLOWED = new Set(['400', '500', '600']);
const COLOR_LITERAL = /#[0-9a-fA-F]{3,8}\b|\brgba?\s*\(|\bhsla?\s*\(/;

function walk(dir, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name.startsWith('.')) continue;
      walk(full, out);
    } else if (/\.(css|svelte)$/.test(entry.name)) {
      out.push(full);
    }
  }
  return out;
}

/** 取出一个文件的样式文本：css 整份，svelte 只取 <style>。 */
function styleText(file) {
  const raw = readFileSync(file, 'utf8');
  if (file.endsWith('.css')) return raw;
  const out = [];
  for (const m of raw.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)) out.push(m[1]);
  return out.join('\n');
}

/**
 * 极简 CSS 块扫描：维护块栈，返回每个"带声明的块"（选择器 + 声明 + 祖先 @规则）。
 * 引号内的 {} 与注释都跳过 —— 否则 content:"{" 会撕裂块结构。
 */
function scanRules(css) {
  const rules = [];
  const stack = [];
  let buf = '';
  let quote = null;
  let comment = false;

  for (let i = 0; i < css.length; i += 1) {
    const ch = css[i];
    const next = css[i + 1];

    if (comment) {
      if (ch === '*' && next === '/') {
        comment = false;
        i += 1;
      }
      continue;
    }
    if (quote) {
      if (ch === '\\') i += 1;
      else if (ch === quote) quote = null;
      else if (stack.length) stack[stack.length - 1].pending += ch;
      continue;
    }
    if (ch === '/' && next === '*') {
      comment = true;
      i += 1;
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      if (stack.length) stack[stack.length - 1].pending += ch;
      continue;
    }

    if (ch === '{') {
      stack.push({ prelude: buf.trim().replace(/\s+/g, ' '), decls: [], pending: '' });
      buf = '';
      continue;
    }
    if (ch === ';') {
      if (stack.length) {
        const frame = stack[stack.length - 1];
        const decl = (buf + frame.pending).trim().replace(/\s+/g, ' ');
        frame.pending = '';
        if (decl) frame.decls.push(decl);
      }
      buf = '';
      continue;
    }
    if (ch === '}') {
      if (stack.length) {
        const frame = stack.pop();
        const tail = buf.trim().replace(/\s+/g, ' ');
        if (tail && frame.decls.length) frame.decls.push(tail);
        if (frame.decls.length) {
          rules.push({
            selector: frame.prelude,
            decls: frame.decls,
            at: stack.map((f) => f.prelude),
          });
        }
      }
      buf = '';
      continue;
    }
    buf += ch;
  }
  return rules;
}

const splitDecl = (decl) => {
  const idx = decl.indexOf(':');
  if (idx < 0) return null;
  return {
    prop: decl.slice(0, idx).trim().toLowerCase(),
    value: decl.slice(idx + 1).trim(),
  };
};

const isVar = (value) => /^var\(--dsw-[\w-]+\)$/.test(value.trim());
const isKeyframes = (at) => at.some((a) => /^@(-\w+-)?keyframes/.test(a));

const findings = [];

function report(file, rule, ruleId, message) {
  findings.push({
    file: relative(ROOT, file).replace(/\\/g, '/'),
    selector: rule.selector,
    rule: ruleId,
    message,
  });
}

for (const file of SCAN_DIRS.flatMap((d) => walk(join(ROOT, d)))) {
  if (relative(ROOT, file).replace(/\\/g, '/') === TOKEN_FILE) continue;
  const css = styleText(file);
  if (!css.trim()) continue;

  for (const rule of scanRules(css)) {
    if (isKeyframes(rule.at)) continue;
    const props = rule.decls.map(splitDecl).filter(Boolean);
    const hasLineHeight = props.some((d) => d.prop === 'line-height');
    const hasFontShorthand = props.some((d) => d.prop === 'font');

    for (const { prop, value } of props) {
      // R1 禁裸颜色
      if (/^(color|background|background-color|border|border-color|border-[a-z]+-color|outline|outline-color|box-shadow|fill|stroke|caret-color|text-decoration-color)$/.test(prop)) {
        if (COLOR_LITERAL.test(value) && !/var\(--dsw-/.test(value)) {
          report(file, rule, 'R1', `${prop}: ${value} —— 颜色必须是 var(--dsw-*)`);
        }
      }

      // R2 圆角只用 token
      if (/^border(-(top|bottom)-(left|right))?-radius$/.test(prop)) {
        const parts = value.split(/\s+/).filter(Boolean);
        for (const part of parts) {
          if (isVar(part) || RADIUS_WHITELIST.has(part)) continue;
          report(file, rule, 'R2', `${prop}: ${value} —— 只许 var(--dsw-radius-*) 或 0/999px/50%`);
        }
      }

      // R3 字号成对
      if (prop === 'font-size' && !isVar(value)) {
        const px = /^([\d.]+)px$/.exec(value.trim());
        if (!px) {
          report(file, rule, 'R3', `font-size: ${value} —— 字面量只许 px: 10/11.5/12.5/13.5/15`);
        } else if (!PX_WHITELIST.has(px[1])) {
          report(
            file,
            rule,
            'R3',
            `font-size: ${value} —— 该档有 token，请用 var(--dsw-font-*)，原生无 token 的只有 10/11.5/12.5/13.5/15`,
          );
        }
        if (!hasLineHeight && !hasFontShorthand) {
          report(file, rule, 'R3', `font-size: ${value} 没有配 line-height —— 原生每一处都成对出现`);
        }
      }

      // R3b 字重不出界
      if (prop === 'font-weight' && !isVar(value)) {
        const w = value.trim();
        if (/^\d+$/.test(w) && !WEIGHT_ALLOWED.has(w)) {
          report(file, rule, 'R3b', `font-weight: ${w} —— 原生只用 400/500/600`);
        }
      }
    }
  }
}

const byRule = findings.reduce((acc, f) => ({ ...acc, [f.rule]: (acc[f.rule] ?? 0) + 1 }), {});
const json = process.argv.includes('--json');

if (json) {
  console.log(JSON.stringify({ total: findings.length, byRule, findings }, null, 2));
} else if (findings.length === 0) {
  console.log('[lint-css] PASS：R1/R2/R3/R3b 全 0');
} else {
  for (const f of findings) console.log(`${f.file}  ${f.rule}  ${f.selector}  ${f.message}`);
  console.log(
    `\n[lint-css] FAIL：${findings.length} 处 ` +
      `(R1 ${byRule.R1 ?? 0} / R2 ${byRule.R2 ?? 0} / R3 ${byRule.R3 ?? 0} / R3b ${byRule.R3b ?? 0})`,
  );
}

process.exit(findings.length === 0 ? 0 : 1);
