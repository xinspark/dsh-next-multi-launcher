#!/usr/bin/env node
// make-icon.mjs —— 生成启动器图标源图（1024×1024 PNG），随后用 `pnpm tauri icon` 派生各尺寸。
// 设计：深蓝渐变圆角方块 + 两扇重叠的“窗口”，表达“同一应用多开”。
// 纯 Node 实现（zlib + 手写 PNG chunk），不依赖 canvas / sharp。
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { deflateSync } from 'node:zlib';

const SIZE = 1024;
const out = resolve(process.argv[2] ?? 'src-tauri/icons/source.png');

const px = Buffer.alloc(SIZE * SIZE * 4);

function mix(a, b, t) {
  return [
    Math.round(a[0] + (b[0] - a[0]) * t),
    Math.round(a[1] + (b[1] - a[1]) * t),
    Math.round(a[2] + (b[2] - a[2]) * t)
  ];
}

function blend(x, y, color, alpha) {
  if (alpha <= 0) return;
  if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) return;
  const i = (y * SIZE + x) * 4;
  const a = Math.min(1, alpha);
  const dst = [px[i], px[i + 1], px[i + 2], px[i + 3] / 255];
  const outA = a + dst[3] * (1 - a);
  if (outA <= 0) return;
  for (let c = 0; c < 3; c++) {
    px[i + c] = Math.round((color[c] * a + dst[c] * dst[3] * (1 - a)) / outA);
  }
  px[i + 3] = Math.round(outA * 255);
}

// 圆角矩形（带 1px 抗锯齿，用 4×4 超采样）
function roundRect(x0, y0, x1, y1, r, color, alpha = 1) {
  const S = 4;
  for (let y = Math.floor(y0 - 1); y <= Math.ceil(y1 + 1); y++) {
    for (let x = Math.floor(x0 - 1); x <= Math.ceil(x1 + 1); x++) {
      let hits = 0;
      for (let sy = 0; sy < S; sy++) {
        for (let sx = 0; sx < S; sx++) {
          const fx = x + (sx + 0.5) / S;
          const fy = y + (sy + 0.5) / S;
          if (insideRoundRect(fx, fy, x0, y0, x1, y1, r)) hits++;
        }
      }
      if (hits) blend(x, y, color, (hits / (S * S)) * alpha);
    }
  }
}

function insideRoundRect(x, y, x0, y0, x1, y1, r) {
  if (x < x0 || x > x1 || y < y0 || y > y1) return false;
  const cx = Math.min(Math.max(x, x0 + r), x1 - r);
  const cy = Math.min(Math.max(y, y0 + r), y1 - r);
  const dx = x - cx;
  const dy = y - cy;
  return dx * dx + dy * dy <= r * r;
}

// 1) 背景渐变（左上深蓝 → 右下亮蓝）
const c0 = [37, 99, 235]; // #2563eb
const c1 = [77, 147, 248]; // #4d93f8
for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const i = (y * SIZE + x) * 4;
    const t = (x / SIZE) * 0.35 + (y / SIZE) * 0.65;
    const [r, g, b] = mix(c0, c1, t);
    px[i] = r;
    px[i + 1] = g;
    px[i + 2] = b;
    px[i + 3] = 255;
  }
}

// 2) 外层圆角遮罩（把方形裁成圆角方块）
const RADIUS = 232;
for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const i = (y * SIZE + x) * 4;
    if (!insideRoundRect(x + 0.5, y + 0.5, 0, 0, SIZE, SIZE, RADIUS)) px[i + 3] = 0;
  }
}

// 3) 两扇重叠的窗口：后窗（暗） + 前窗（亮），各自带标题栏
const white = [255, 255, 255];
// 后窗
roundRect(300, 250, 700, 560, 44, mix(c0, [0, 0, 0], 0.35), 0.85);
// 前窗
roundRect(360, 380, 780, 760, 44, white, 0.96);
// 前窗标题栏
roundRect(360, 380, 780, 470, 44, [220, 232, 255], 0.9);
// 前窗正文三行（浅蓝条）
for (let i = 0; i < 3; i++) {
  const y = 520 + i * 58;
  roundRect(410, y, 410 + (i === 2 ? 220 : 320), y + 24, 12, [150, 180, 235], 0.85);
}
// 前窗右侧主按钮块
roundRect(640, 520, 735, 700, 26, [59, 130, 246], 0.92);

// ---- PNG 编码
function crc32(buf) {
  let c = ~0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const t = Buffer.from(type, 'ascii');
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([t, data])), 0);
  return Buffer.concat([len, t, data, crcBuf]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA

// 过滤字节 0（None）+ 原始扫描线
const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
for (let y = 0; y < SIZE; y++) {
  raw[y * (SIZE * 4 + 1)] = 0;
  px.copy(raw, y * (SIZE * 4 + 1) + 1, y * SIZE * 4, (y + 1) * SIZE * 4);
}

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0))
]);

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, png);
console.log(JSON.stringify({ out, bytes: png.length, size: SIZE }));
