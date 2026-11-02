/** 名称 → 实例 ID（统一哈希）。
 *
 * 为什么不用拼音：`ling-shu-xi-2` 这类 ID 会让 home 路径变长
 * （`<homeRoot>\.dsh-ling-shu-xi-2`），而 ID 的职责只是“稳定、唯一、短”。
 * 可读性由实例名称承担，ID 可在新建对话框里手改。
 *
 * 算法：64 位 FNV-1a → base36 取 8 位（≈2.8e12 组合），名称空则返回空串。
 */
export function slugify(input: string): string {
  if (!input.trim()) return '';

  let hash = 0xcbf29ce484222325n;
  for (const ch of input) {
    hash ^= BigInt(ch.codePointAt(0) ?? 0);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return `inst-${hash.toString(36).slice(0, 8)}`;
}
