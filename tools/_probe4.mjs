import fs from 'node:fs';
import { openAsar } from './asar-lib.mjs';

const ASAR = process.env.ASAR ?? 'E:\\FXS\\APP\\DSH NEXT\\resources\\app.asar';
const asar = openAsar(ASAR);
const text = asar.readText('lib/main.js');

function grab(re, radius = 320, max = 6) {
  const out = [];
  const g = new RegExp(re.source, re.flags.includes('g') ? re.flags : re.flags + 'g');
  let m;
  while ((m = g.exec(text)) && out.length < max) {
    out.push({ at: m.index, snippet: text.slice(Math.max(0, m.index - radius), m.index + radius) });
  }
  return out;
}

const report = {};
const pats = {
  restartAppAction: /"restart-app"/g,
  safeModeFlag: /--safe-mode|"--safe"|safeModeFlag/g,
  safeModeAssign: /safeMode\s*[:=]/g,
  argvScan: /process\.argv/g,
  relaunchCall: /relaunch\(\{[^}]*args/gs,
  envHome: /DSH_DESKTOP_NEXT_HOME/g
};
for (const [k, re] of Object.entries(pats)) report[k] = grab(re, 300, 5);

fs.writeFileSync('target/asar-restart-probe2.json', JSON.stringify(report, null, 2));

for (const [k, v] of Object.entries(report)) {
  console.log(`\n===== ${k} (${v.length}) =====`);
  for (const item of v.slice(0, 3)) {
    console.log(`--- @${item.at} ---`);
    console.log(item.snippet.replace(/\s+/g, ' ').slice(0, 520));
  }
}
