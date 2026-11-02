import fs from 'node:fs';
import { openAsar } from './asar-lib.mjs';

const ASAR = process.env.ASAR ?? 'E:\\FXS\\APP\\DSH NEXT\\resources\\app.asar';
const asar = openAsar(ASAR);
const text = asar.readText('lib/main.js');

function grab(re, radius = 400, max = 4) {
  const out = [];
  const g = new RegExp(re.source, re.flags.includes('g') ? re.flags : re.flags + 'g');
  let m;
  while ((m = g.exec(text)) && out.length < max) {
    out.push({ at: m.index, snippet: text.slice(Math.max(0, m.index - radius), m.index + radius) });
  }
  return out;
}

const report = {
  safeConst: grab(/SAFE_ARGUMENT\s*=\s*"[^"]*"/g, 120, 3),
  recoveryConst: grab(/RECOVERY_ARGUMENT\s*=\s*"[^"]*"/g, 120, 3),
  relaunchArgs: grab(/function relaunchArguments/g, 900, 2),
  relaunchUsage: grab(/relaunchArguments\(/g, 300, 6)
};

fs.writeFileSync('target/asar-restart-probe3.json', JSON.stringify(report, null, 2));

for (const [k, v] of Object.entries(report)) {
  console.log(`\n===== ${k} (${v.length}) =====`);
  for (const item of v.slice(0, 3)) {
    console.log(`--- @${item.at} ---`);
    console.log(item.snippet.replace(/\s+/g, ' ').slice(0, 900));
  }
}
