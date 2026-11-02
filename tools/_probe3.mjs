import fs from 'node:fs';
import { openAsar } from './asar-lib.mjs';

const ASAR = process.env.ASAR ?? 'E:\\FXS\\APP\\DSH NEXT\\resources\\app.asar';
const asar = openAsar(ASAR);
const text = asar.readText('lib/main.js');

function around(re, radius = 260, max = 8) {
  const out = [];
  const g = new RegExp(re.source, re.flags.includes('g') ? re.flags : re.flags + 'g');
  let m;
  while ((m = g.exec(text)) && out.length < max) {
    out.push({
      at: m.index,
      snippet: text.slice(Math.max(0, m.index - radius), m.index + radius)
    });
  }
  return out;
}

const report = {};
for (const [name, re] of Object.entries({
  relaunch: /app\.relaunch|relaunch\(/g,
  safeMode: /safe[-_]?mode/gi,
  recovery: /recovery/gi,
  pointer: /desktop-next-location/gi
})) {
  report[name] = around(re, name === 'pointer' ? 400 : 240, 6);
}

fs.writeFileSync('target/asar-restart-probe.json', JSON.stringify(report, null, 2));
console.log('written target/asar-restart-probe.json');
for (const [k, v] of Object.entries(report)) {
  console.log(`\n===== ${k} (${v.length}) =====`);
  for (const item of v.slice(0, 3)) {
    console.log(`--- @${item.at} ---`);
    console.log(item.snippet.replace(/\s+/g, ' ').slice(0, 460));
  }
}
