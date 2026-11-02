// 展示用的纯函数：路径中间省略、时间、运行时长。

/** 中间省略：保留盘符与最后一段，超长时插入 …（设计 §6.2「路径过长：中间省略」）。 */
export function shortPath(path: string, max = 46): string {
  if (!path) return '';
  if (path.length <= max) return path;
  const parts = path.split('\\').filter(Boolean);
  const head = parts[0] ?? '';
  const tailParts: string[] = [];
  let tailLen = 0;
  for (let i = parts.length - 1; i >= 1 && tailLen < max * 0.45; i--) {
    tailParts.unshift(parts[i]);
    tailLen += parts[i].length + 1;
  }
  return `${head}\\…\\${tailParts.join('\\')}`;
}

export function hhmm(iso?: string): string {
  if (!iso) return '—';
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '—';
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
}

/** 运行时长：秒 → “12 分钟 / 1 小时 3 分钟”。 */
export function uptime(startedAtSec: number): string {
  if (!startedAtSec) return '—';
  const startedMs = startedAtSec * 1000;
  const ms = Date.now() - startedMs;
  if (ms < 0) return '—';
  const total = Math.floor(ms / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours > 0) return `${hours} 小时 ${minutes} 分钟`;
  if (minutes > 0) return `${minutes} 分钟`;
  return `${Math.max(1, total)} 秒`;
}

export function elapsedSec(iso?: string): number {
  if (!iso) return 0;
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return 0;
  return Math.max(0, Math.floor((Date.now() - t) / 1000));
}

export function formatDate(iso?: string): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString();
}
