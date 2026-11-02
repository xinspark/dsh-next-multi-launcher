#!/usr/bin/env node
// icon-linked.mjs —— 与 DSH-NEXT 联动的图标：复用官方鲸鱼剪影 + 同构的右下角胶囊。
// 纯 Node：自带最小 PNG 解码器（8-bit RGB/RGBA，无隔行）+ 编码器。
// 用法：node tools/icon-linked.mjs
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { inflateSync, deflateSync } from 'node:zlib';

/**
 * 官方 app-icon.png 的位置：DSH_APP_ICON → 常见安装目录的 unpacked 副本。
 * 官方图标只用来做"鲸鱼剪影"的取材，缺了就直接报错，不去猜别的位置。
 */
function resolveSource() {
  const candidates = [
    process.env.DSH_APP_ICON,
    process.env.LOCALAPPDATA &&
      join(process.env.LOCALAPPDATA, 'Programs', 'DSH NEXT', 'resources', 'app.asar.unpacked', 'build', 'app-icon.png'),
    process.env.ProgramFiles &&
      join(process.env.ProgramFiles, 'DSH NEXT', 'resources', 'app.asar.unpacked', 'build', 'app-icon.png'),
  ].filter(Boolean);
  const hit = candidates.find((p) => existsSync(p));
  if (!hit) throw new Error('找不到官方 app-icon.png：请设置 DSH_APP_ICON 指向它');
  return hit;
}

const SRC = resolveSource();
const OUT_DIR = 'design/icons';
const SIZE = 1024;

// ---------------------------------------------------------------- PNG 解码
function decodePng(buf) {
  if (buf.readUInt32BE(0) !== 0x89504e47) throw new Error('not png');
  let pos = 8;
  let width = 0;
  let height = 0;
  let colorType = 0;
  let bitDepth = 0;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString('ascii', pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      if (data[12] !== 0) throw new Error('interlaced png unsupported');
    } else if (type === 'IDAT') {
      idat.push(data);
    } else if (type === 'IEND') {
      break;
    }
    pos += 12 + len;
  }
  if (bitDepth !== 8 && bitDepth !== 16) throw new Error(`bit depth ${bitDepth} unsupported`);
  const channels = colorType === 6 ? 4 : colorType === 2 ? 3 : 0;
  if (!channels) throw new Error(`color type ${colorType} unsupported`);
  const sampleBytes = bitDepth === 16 ? 2 : 1;
  const bpp = channels * sampleBytes; // 反滤波的“每像素字节数”

  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * bpp;
  const out = Buffer.alloc(width * height * 4);
  let prev = Buffer.alloc(stride);

  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const cur = Buffer.alloc(stride);
    for (let i = 0; i < stride; i++) {
      const a = i >= bpp ? cur[i - bpp] : 0;
      const b = prev[i];
      const c = i >= bpp ? prev[i - bpp] : 0;
      const x = line[i];
      let v;
      switch (filter) {
        case 0:
          v = x;
          break;
        case 1:
          v = x + a;
          break;
        case 2:
          v = x + b;
          break;
        case 3:
          v = x + ((a + b) >> 1);
          break;
        case 4: {
          const p = a + b - c;
          const pa = Math.abs(p - a);
          const pb = Math.abs(p - b);
          const pc = Math.abs(p - c);
          v = x + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c);
          break;
        }
        default:
          throw new Error(`filter ${filter}`);
      }
      cur[i] = v & 0xff;
    }
    for (let x = 0; x < width; x++) {
      const si = x * bpp;
      const di = (y * width + x) * 4;
      const step = sampleBytes;
      out[di] = cur[si];
      out[di + 1] = cur[si + step];
      out[di + 2] = cur[si + step * 2];
      out[di + 3] = channels === 4 ? cur[si + step * 3] : 255;
    }
    prev = cur;
  }
  return { width, height, rgba: out };
}

// ---------------------------------------------------------------- 画布工具
const insideRoundRect = (x, y, x0, y0, x1, y1, r) => {
  if (x < x0 || x > x1 || y < y0 || y > y1) return false;
  const cx = Math.min(Math.max(x, x0 + r), x1 - r);
  const cy = Math.min(Math.max(y, y0 + r), y1 - r);
  const dx = x - cx;
  const dy = y - cy;
  return dx * dx + dy * dy <= r * r;
};

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

function roundRect(px, x0, y0, x1, y1, r, color, alpha = 1) {
  const S = 4;
  for (let y = Math.floor(y0 - 2); y <= Math.ceil(y1 + 2); y++) {
    for (let x = Math.floor(x0 - 2); x <= Math.ceil(x1 + 2); x++) {
      let hits = 0;
      for (let sy = 0; sy < S; sy++) {
        for (let sx = 0; sx < S; sx++) {
          if (insideRoundRect(x + (sx + 0.5) / S, y + (sy + 0.5) / S, x0, y0, x1, y1, r)) hits++;
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
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([t, data])), 0);
  return Buffer.concat([len, t, data, crc]);
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

// ---------------------------------------------------------------- 鲸鱼蒙版
const logo = decodePng(readFileSync(SRC));
console.log('logo:', logo.width, 'x', logo.height);

/** 亮度蒙版：官方图里鲸鱼是浅色、背景是近黑；取亮度 > 阈值且不透明的像素。 */
function whaleMask(threshold = 150) {
  const { width, height, rgba } = logo;
  const mask = new Uint8Array(width * height);
  // 只取鲸鱼所在区域：排除顶部高光细线（y<130）与右下角官方 NEXT 胶囊（y>770）
  const Y_MIN = 130;
  const Y_MAX = 770;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      if (y < Y_MIN || y > Y_MAX) continue;
      const r = rgba[i * 4];
      const g = rgba[i * 4 + 1];
      const b = rgba[i * 4 + 2];
      const a = rgba[i * 4 + 3];
      const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
      mask[i] = a > 40 && lum > threshold ? 1 : 0;
    }
  }
  return { width, height, mask };
}

const whale = whaleMask();

/** 把蒙版按目标矩形缩放绘制（双线性采样），用给定颜色填充。 */
function drawWhale(px, dx0, dy0, dw, dh, color, alpha = 1) {
  for (let y = 0; y < dh; y++) {
    for (let x = 0; x < dw; x++) {
      const sx = ((x + 0.5) / dw) * whale.width - 0.5;
      const sy = ((y + 0.5) / dh) * whale.height - 0.5;
      const x0 = Math.max(0, Math.min(whale.width - 1, Math.floor(sx)));
      const y0 = Math.max(0, Math.min(whale.height - 1, Math.floor(sy)));
      const x1 = Math.min(whale.width - 1, x0 + 1);
      const y1 = Math.min(whale.height - 1, y0 + 1);
      const fx = sx - x0;
      const fy = sy - y0;
      const v =
        whale.mask[y0 * whale.width + x0] * (1 - fx) * (1 - fy) +
        whale.mask[y0 * whale.width + x1] * fx * (1 - fy) +
        whale.mask[y1 * whale.width + x0] * (1 - fx) * fy +
        whale.mask[y1 * whale.width + x1] * fx * fy;
      if (v > 0.02) blend(px, dx0 + x, dy0 + y, color, v * alpha);
    }
  }
}

// ---------------------------------------------------------------- 合成
const BG = [21, 21, 21]; // 与 DSH 官方图标一致的近黑
const WHITE = [255, 255, 255];

function designLinked() {
  const px = Buffer.alloc(SIZE * SIZE * 4);
  for (let i = 0; i < SIZE * SIZE; i++) {
    px[i * 4] = BG[0];
    px[i * 4 + 1] = BG[1];
    px[i * 4 + 2] = BG[2];
    px[i * 4 + 3] = 255;
  }

  // 鲸鱼：略偏左上，给右下角胶囊留位置（与官方构图一致）
  drawWhale(px, 90, 70, 840, 840, WHITE, 1);

  // 右下角胶囊：白底 + 三个深色圆点（“多个实例”）
  const capX0 = 560;
  const capY0 = 790;
  const capX1 = 950;
  const capY1 = 930;
  roundRect(px, capX0, capY0, capX1, capY1, 70, WHITE, 1);
  const cy = (capY0 + capY1) / 2;
  for (let i = 0; i < 3; i++) {
    circle(px, 640 + i * 115, cy, 26, BG, 1);
  }

  maskRound(px, 232);
  return px;
}

// ---------------------------------------------------------------- E：四宫格 + 激活格里的鲸鱼
function designGridWhale() {
  const px = Buffer.alloc(SIZE * SIZE * 4);
  for (let i = 0; i < SIZE * SIZE; i++) {
    px[i * 4] = BG[0];
    px[i * 4 + 1] = BG[1];
    px[i * 4 + 2] = BG[2];
    px[i * 4 + 3] = 255;
  }

  // 四宫格：间距大一点，小尺寸下才分得开
  const cells = [
    [112, 112, 500, 500],
    [524, 112, 912, 500],
    [112, 524, 500, 912],
    [524, 524, 912, 912]
  ];
  const BLUE = [37, 99, 235]; // #2563EB，与 DSH 品牌蓝一致
  const CELL_BG = [24, 28, 36]; // 压暗的非激活格
  const CELL_EDGE = [59, 130, 246];

  cells.forEach((c, idx) => {
    const active = idx === 0;
    if (active) {
      roundRect(px, c[0], c[1], c[2], c[3], 52, BLUE, 1);
    } else {
      // 未激活格也画成“实例卡片”：标题条 + 内容线，不再是空块
      roundRect(px, c[0], c[1], c[2], c[3], 52, CELL_BG, 1);
      roundRect(px, c[0], c[1], c[2], c[1] + 62, 26, [40, 50, 66], 1);
      roundRect(px, c[0], c[1] + 44, c[2], c[1] + 62, 0, [40, 50, 66], 1);
      for (let i = 0; i < 3; i++) {
        const y = c[1] + 104 + i * 48;
        roundRect(px, c[0] + 36, y, c[0] + 36 + (i === 2 ? 130 : 220), y + 18, 9, [56, 68, 86], 1);
      }
      roundRect(px, c[0], c[1], c[2], c[3], 52, CELL_EDGE, 0.3);
    }
  });

  // 激活格里的小鲸鱼：整张蒙版缩放到格子内（蒙版里鲸鱼本就居中偏上）
  drawWhale(px, 96, 96, 420, 420, WHITE, 1);

  maskRound(px, 232);
  return px;
}

mkdirSync(OUT_DIR, { recursive: true });
const png = encode(designLinked());
writeFileSync(`${OUT_DIR}/D-linked-whale.png`, png);
console.log(`${OUT_DIR}/D-linked-whale.png`, png.length, 'bytes');

const pngE = encode(designGridWhale());
writeFileSync(`${OUT_DIR}/E-grid-whale.png`, pngE);
console.log(`${OUT_DIR}/E-grid-whale.png`, pngE.length, 'bytes');
