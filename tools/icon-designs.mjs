#!/usr/bin/env node
// icon-designs.mjs —— 三个图标方向的设计稿（1024×1024 PNG）+ 一张并排对比图。
// 纯 Node（zlib + 手写 PNG chunk），不依赖 canvas / sharp。
// 用法：node tools/icon-designs.mjs
import { mkdirSync, writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

const SIZE = 1024;
const OUT_DIR = 'design/icons';

// ---------------------------------------------------------------- 画布工具
function canvas(bg0, bg1) {
  const px = Buffer.alloc(SIZE * SIZE * 4);
  for (let y = 0; y < SIZE; y++) {
    for (let x = 0; x < SIZE; x++) {
      const i = (y * SIZE + x) * 4;
      const t = (x / SIZE) * 0.35 + (y / SIZE) * 0.65;
      px[i] = Math.round(bg0[0] + (bg1[0] - bg0[0]) * t);
      px[i + 1] = Math.round(bg0[1] + (bg1[1] - bg0[1]) * t);
      px[i + 2] = Math.round(bg0[2] + (bg1[2] - bg0[2]) * t);
      px[i + 3] = 255;
    }
  }
  return px;
}

function insideRoundRect(x, y, x0, y0, x1, y1, r) {
  if (x < x0 || x > x1 || y < y0 || y > y1) return false;
  const cx = Math.min(Math.max(x, x0 + r), x1 - r);
  const cy = Math.min(Math.max(y, y0 + r), y1 - r);
  const dx = x - cx;
  const dy = y - cy;
  return dx * dx + dy * dy <= r * r;
}

function blend(px, x, y, color, alpha) {
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

/** 圆角矩形，4×4 超采样抗锯齿。stroke>0 时只画描边。 */
function roundRect(px, x0, y0, x1, y1, r, color, alpha = 1, stroke = 0) {
  const S = 4;
  for (let y = Math.floor(y0 - 2); y <= Math.ceil(y1 + 2); y++) {
    for (let x = Math.floor(x0 - 2); x <= Math.ceil(x1 + 2); x++) {
      let hits = 0;
      for (let sy = 0; sy < S; sy++) {
        for (let sx = 0; sx < S; sx++) {
          const fx = x + (sx + 0.5) / S;
          const fy = y + (sy + 0.5) / S;
          const inside = insideRoundRect(fx, fy, x0, y0, x1, y1, r);
          if (!inside) continue;
          if (stroke > 0) {
            const inner = insideRoundRect(
              fx,
              fy,
              x0 + stroke,
              y0 + stroke,
              x1 - stroke,
              y1 - stroke,
              Math.max(0, r - stroke)
            );
            if (inner) continue;
          }
          hits++;
        }
      }
      if (hits) blend(px, x, y, color, (hits / (S * S)) * alpha);
    }
  }
}

function circle(px, cx, cy, radius, color, alpha = 1) {
  const S = 4;
  for (let y = Math.floor(cy - radius - 2); y <= Math.ceil(cy + radius + 2); y++) {
    for (let x = Math.floor(cx - radius - 2); x <= Math.ceil(cx + radius + 2); x++) {
      let hits = 0;
      for (let sy = 0; sy < S; sy++) {
        for (let sx = 0; sx < S; sx++) {
          const dx = x + (sx + 0.5) / S - cx;
          const dy = y + (sy + 0.5) / S - cy;
          if (dx * dx + dy * dy <= radius * radius) hits++;
        }
      }
      if (hits) blend(px, x, y, color, (hits / (S * S)) * alpha);
    }
  }
}

/** 圆角遮罩：把方形裁成圆角方块（透明边） */
function maskRound(px, radius) {
  for (let y = 0; y < SIZE; y++) {
    for (let x = 0; x < SIZE; x++) {
      if (!insideRoundRect(x + 0.5, y + 0.5, 0, 0, SIZE, SIZE, radius)) {
        px[(y * SIZE + x) * 4 + 3] = 0;
      }
    }
  }
}

// ---------------------------------------------------------------- PNG 编码
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

function encode(px, size = SIZE) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0;
    px.copy(raw, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0))
  ]);
}

// ---------------------------------------------------------------- 配色
const BG0 = [9, 17, 31]; // #09111F
const BG1 = [18, 38, 66]; // #122642
const BLUE = [59, 130, 246]; // #3B82F6
const BLUE_LIGHT = [96, 165, 250];
const WHITE = [233, 241, 251];
const GREEN = [34, 197, 94];

// ---------------------------------------------------------------- A：堆叠实例
function designStacked() {
  const px = canvas(BG0, BG1);
  // 三张卡片，从后往前错位
  roundRect(px, 214, 214, 654, 654, 52, [58, 116, 184], 0.92);
  roundRect(px, 286, 286, 726, 726, 52, [96, 158, 226], 0.95);
  roundRect(px, 358, 358, 798, 798, 52, WHITE, 1);
  // 前卡标题条
  roundRect(px, 358, 358, 798, 470, 52, [206, 222, 245], 1);
  roundRect(px, 358, 430, 798, 470, 0, [206, 222, 245], 1);
  // 内容线
  for (let i = 0; i < 3; i++) {
    const y = 530 + i * 70;
    roundRect(px, 418, y, 418 + (i === 2 ? 220 : 320), y + 26, 13, [150, 178, 216], 0.95);
  }
  // 运行中绿点
  circle(px, 742, 700, 34, GREEN, 1);
  maskRound(px, 232);
  return px;
}

// ---------------------------------------------------------------- B：四宫格窗口
function designGrid() {
  const px = canvas(BG0, BG1);
  const cells = [
    [200, 200, 500, 500],
    [524, 200, 824, 500],
    [200, 524, 500, 824],
    [524, 524, 824, 824]
  ];
  cells.forEach((c, idx) => {
    const active = idx === 0;
    if (active) {
      roundRect(px, c[0], c[1], c[2], c[3], 44, BLUE, 1);
      roundRect(px, c[0], c[1], c[2], c[1] + 74, 44, BLUE_LIGHT, 1);
      for (let i = 0; i < 3; i++) {
        const y = c[1] + 150 + i * 56;
        roundRect(px, c[0] + 44, y, c[0] + 44 + (i === 2 ? 120 : 200), y + 22, 11, WHITE, 0.95);
      }
    } else {
      roundRect(px, c[0], c[1], c[2], c[3], 44, BLUE, 0.55, 14);
    }
  });
  maskRound(px, 232);
  return px;
}

// ---------------------------------------------------------------- C：启动器
function designLauncher() {
  const px = canvas(BG0, BG1);
  // 主窗口（描边）
  roundRect(px, 206, 236, 818, 760, 56, BLUE, 0.9, 16);
  // 标题条
  roundRect(px, 206, 236, 818, 356, 56, BLUE, 0.28);
  roundRect(px, 206, 300, 818, 356, 0, BLUE, 0.28);
  // 窗口内三条内容线
  for (let i = 0; i < 3; i++) {
    const y = 420 + i * 72;
    roundRect(px, 270, y, 270 + (i === 2 ? 240 : 420), y + 26, 13, BLUE, 0.35);
  }
  // 右下角启动按钮
  circle(px, 700, 700, 128, [30, 64, 175], 1);
  circle(px, 700, 700, 116, BLUE, 1);
  // 白色三角形（播放）
  const cx = 700;
  const cy = 700;
  const tri = [
    [cx - 34, cy - 52],
    [cx - 34, cy + 52],
    [cx + 56, cy]
  ];
  for (let y = Math.floor(cy - 60); y <= Math.ceil(cy + 60); y++) {
    for (let x = Math.floor(cx - 50); x <= Math.ceil(cx + 60); x++) {
      if (pointInTriangle(x + 0.5, y + 0.5, tri)) blend(px, x, y, WHITE, 1);
    }
  }
  maskRound(px, 232);
  return px;
}

function pointInTriangle(x, y, [a, b, c]) {
  const d1 = side(x, y, a, b);
  const d2 = side(x, y, b, c);
  const d3 = side(x, y, c, a);
  const hasNeg = d1 < 0 || d2 < 0 || d3 < 0;
  const hasPos = d1 > 0 || d2 > 0 || d3 > 0;
  return !(hasNeg && hasPos);
}

/** 点相对有向边 p1→p2 的侧向符号 */
function side(x, y, p1, p2) {
  return (x - p2[0]) * (p1[1] - p2[1]) - (p1[0] - p2[0]) * (y - p2[1]);
}

// ---------------------------------------------------------------- 对比图（3×512 并排）
function downscale(px, size, factor) {
  const out = Buffer.alloc((size / factor) * (size / factor) * 4);
  const os = size / factor;
  for (let y = 0; y < os; y++) {
    for (let x = 0; x < os; x++) {
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;
      for (let sy = 0; sy < factor; sy++) {
        for (let sx = 0; sx < factor; sx++) {
          const i = ((y * factor + sy) * size + (x * factor + sx)) * 4;
          r += px[i];
          g += px[i + 1];
          b += px[i + 2];
          a += px[i + 3];
        }
      }
      const n = factor * factor;
      const o = (y * os + x) * 4;
      out[o] = Math.round(r / n);
      out[o + 1] = Math.round(g / n);
      out[o + 2] = Math.round(b / n);
      out[o + 3] = Math.round(a / n);
    }
  }
  return out;
}

function contactSheet(images) {
  const cell = 512;
  const gap = 32;
  const width = images.length * cell + (images.length + 1) * gap;
  const height = cell + gap * 2;
  const buf = Buffer.alloc(width * height * 4);
  // 深色背景
  for (let i = 0; i < width * height; i++) {
    buf[i * 4] = 24;
    buf[i * 4 + 1] = 24;
    buf[i * 4 + 2] = 28;
    buf[i * 4 + 3] = 255;
  }
  images.forEach((img, idx) => {
    const ox = gap + idx * (cell + gap);
    const oy = gap;
    for (let y = 0; y < cell; y++) {
      for (let x = 0; x < cell; x++) {
        const si = (y * cell + x) * 4;
        const a = img[si + 3] / 255;
        if (a <= 0) continue;
        const di = ((oy + y) * width + ox + x) * 4;
        for (let c = 0; c < 3; c++) {
          buf[di + c] = Math.round(img[si + c] * a + buf[di + c] * (1 - a));
        }
      }
    }
  });
  return { buf, width, height };
}

// ---------------------------------------------------------------- 输出
mkdirSync(OUT_DIR, { recursive: true });

const designs = [
  { name: 'A-stacked', px: designStacked() },
  { name: 'B-grid', px: designGrid() },
  { name: 'C-launcher', px: designLauncher() }
];

for (const d of designs) {
  const png = encode(d.px);
  writeFileSync(`${OUT_DIR}/${d.name}.png`, png);
  console.log(`${OUT_DIR}/${d.name}.png`, png.length, 'bytes');
}

const half = designs.map((d) => downscale(d.px, SIZE, 2));
const sheet = contactSheet(half);
writeFileSync(`${OUT_DIR}/preview.png`, encodeSheet(sheet));
console.log(`${OUT_DIR}/preview.png`, `${sheet.width}x${sheet.height}`);

function encodeSheet({ buf, width, height }) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const raw = Buffer.alloc(height * (width * 4 + 1));
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0;
    buf.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0))
  ]);
}
