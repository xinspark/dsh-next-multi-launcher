import { openAsar } from './asar-lib.mjs';

const ASAR = process.env.ASAR ?? 'E:\\FXS\\APP\\DSH NEXT\\resources\\app.asar';
const asar = openAsar(ASAR);

const all = asar.list();
console.log('entries =', all.length);
const files = all.filter((e) => e.type === 'file');
console.log('files =', files.length);

const mainText = asar.readText('lib/main.js');
console.log('lib/main.js length =', mainText?.length ?? 0);
if (mainText) {
  for (const re of [/relaunch/g, /safe[-_]?mode/gi, /recovery/gi, /desktop-next-location/gi]) {
    console.log(String(re), '=>', (mainText.match(re) ?? []).length);
  }
  const idx = mainText.search(/relaunch|safe[-_]?mode|recovery/i);
  if (idx >= 0) console.log('context:', mainText.slice(Math.max(0, idx - 200), idx + 300));
}
