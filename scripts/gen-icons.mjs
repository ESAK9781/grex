import fs from "node:fs";
import zlib from "node:zlib";
import path from "node:path";

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const t = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([t, data])), 0);
  return Buffer.concat([len, t, data, crc]);
}

function makePng(size) {
  const raw = Buffer.alloc(size * (size * 4 + 1));
  const r = size * 0.22;
  for (let y = 0; y < size; y++) {
    const row = y * (size * 4 + 1);
    raw[row] = 0;
    for (let x = 0; x < size; x++) {
      const idx = row + 1 + x * 4;
      const dx = Math.max(r - x, 0, x - (size - 1 - r));
      const dy = Math.max(r - y, 0, y - (size - 1 - r));
      const inside = dx * dx + dy * dy <= r * r;
      if (!inside) {
        raw[idx] = 0;
        raw[idx + 1] = 0;
        raw[idx + 2] = 0;
        raw[idx + 3] = 0;
        continue;
      }
      const nx = x / size;
      const ny = y / size;
      const border = nx < 0.08 || nx > 0.92 || ny < 0.08 || ny > 0.92;
      const glyph =
        nx > 0.24 &&
        nx < 0.76 &&
        ny > 0.24 &&
        ny < 0.76 &&
        (ny < 0.36 ||
          ny > 0.64 ||
          nx < 0.36 ||
          (nx > 0.64 && ny > 0.48) ||
          (ny > 0.46 && ny < 0.56 && nx > 0.48));
      if (glyph) {
        raw[idx] = 249;
        raw[idx + 1] = 115;
        raw[idx + 2] = 22;
        raw[idx + 3] = 255;
      } else if (border) {
        raw[idx] = 234;
        raw[idx + 1] = 88;
        raw[idx + 2] = 12;
        raw[idx + 3] = 255;
      } else {
        raw[idx] = 15;
        raw[idx + 1] = 19;
        raw[idx + 2] = 28;
        raw[idx + 3] = 255;
      }
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const idat = zlib.deflateSync(raw);
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", ihdr),
    chunk("IDAT", idat),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function makeIco(pngBuf, size) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(1, 4);
  const entry = Buffer.alloc(16);
  entry[0] = size >= 256 ? 0 : size;
  entry[1] = size >= 256 ? 0 : size;
  entry.writeUInt16LE(1, 4);
  entry.writeUInt16LE(32, 6);
  entry.writeUInt32LE(pngBuf.length, 8);
  entry.writeUInt32LE(22, 12);
  return Buffer.concat([header, entry, pngBuf]);
}

function makeIcns(png256) {
  const entry = Buffer.alloc(8 + png256.length);
  entry.write("ic08", 0, 4, "ascii");
  entry.writeUInt32BE(8 + png256.length, 4);
  png256.copy(entry, 8);
  const hdr = Buffer.alloc(8);
  hdr.write("icns", 0, 4, "ascii");
  hdr.writeUInt32BE(8 + entry.length, 4);
  return Buffer.concat([hdr, entry]);
}

const dir = path.join("src-tauri", "icons");
if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
const p32 = makePng(32);
const p128 = makePng(128);
const p256 = makePng(256);
fs.writeFileSync(path.join(dir, "32x32.png"), p32);
fs.writeFileSync(path.join(dir, "128x128.png"), p128);
fs.writeFileSync(path.join(dir, "128x128@2x.png"), p256);
fs.writeFileSync(path.join(dir, "icon.ico"), makeIco(p256, 256));
fs.writeFileSync(path.join(dir, "icon.icns"), makeIcns(p256));
console.log("Generated Tauri icons in src-tauri/icons");
