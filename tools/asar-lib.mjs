// asar-lib.mjs — minimal read-only asar reader (8-byte header + JSON index + data area).
// Format: uint32 LE [0..4) = 4 (header size field size), uint32 LE [4..8) = pickle length,
// uint32 LE [8..12) = json length (for 16-byte headers), uint32 LE [12..16) = json string length.
// The data area begins at 8 + pickleLength.
import { createHash } from 'node:crypto';
import { openSync, readSync, closeSync, fstatSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export function openAsar(path) {
  const fd = openSync(path, 'r');
  const fileSize = fstatSync(fd).size;
  const head = Buffer.alloc(16);
  readSync(fd, head, 0, 16, 0);
  const pickleLen = head.readUInt32LE(4);
  const jsonLen = head.readUInt32LE(12);
  const indexStart = 16;
  const dataStart = 8 + pickleLen;
  const raw = Buffer.alloc(jsonLen);
  readSync(fd, raw, 0, jsonLen, indexStart);
  const index = JSON.parse(raw.toString('utf8'));

  function walk(node, prefix, out) {
    for (const [name, child] of Object.entries(node.files ?? {})) {
      const full = prefix ? `${prefix}/${name}` : name;
      if (child.files) walk(child, full, out);
      else out.set(full, child);
    }
    return out;
  }
  const entries = walk(index, '', new Map());

  return {
    path,
    fd,
    fileSize,
    index,
    entries,
    list(prefix = '') {
      return [...entries.keys()].filter((k) => k.startsWith(prefix));
    },
    read(entryPath) {
      const e = entries.get(entryPath);
      if (!e) throw new Error(`asar entry not found: ${entryPath}`);
      if (e.size === 0) return Buffer.alloc(0);
      // Entries flagged "unpacked" live beside the archive in <asar>.unpacked/.
      if (e.unpacked) return readFileSync(join(`${path}.unpacked`, ...entryPath.split('/')));
      const buf = Buffer.alloc(e.size);
      readSync(fd, buf, 0, e.size, dataStart + Number(e.offset));
      return buf;
    },
    readText(entryPath) {
      return this.read(entryPath).toString('utf8');
    },
    sha256File() {
      const h = createHash('sha256');
      const chunk = Buffer.alloc(4 * 1024 * 1024);
      let pos = 0;
      while (pos < fileSize) {
        const n = readSync(fd, chunk, 0, Math.min(chunk.length, fileSize - pos), pos);
        if (n <= 0) break;
        h.update(chunk.subarray(0, n));
        pos += n;
      }
      return h.digest('hex');
    },
    close() {
      closeSync(fd);
    },
  };
}
