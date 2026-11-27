// WebAssembly の仮想機を Node で動かし、試験（test_embed.py）が C の ABI の結果と突き合わせる値を 1 行の JSON で
// 出す。使い方：node check.mjs <fc85emu.wasm> <boot.bin> <char.bin> [cart.bin]
import { readFileSync } from "node:fs";
import { load, AREA, PAD, MidiAssembler } from "./fc85emu.js";

const [wasm, boot, chars, cart] = process.argv.slice(2);
const fnv = (bytes) => {
  let h = 0xcbf29ce484222325n;
  for (const b of bytes) h = BigInt.asUintN(64, (h ^ BigInt(b)) * 0x100000001b3n);
  return h.toString(16).padStart(16, "0");
};
const bytesOf = (typed) => new Uint8Array(typed.buffer, typed.byteOffset, typed.byteLength);

const fc85 = await load(readFileSync(wasm));
const m = fc85.create({ boot: readFileSync(boot), chars: readFileSync(chars) });
if (cart) m.loadCart(readFileSync(cart));
m.runFrames(120);
const out = {
  version: fc85.version,
  frame_rate: fc85.frameRate,
  frame: m.frame,
  screen: fnv(bytesOf(m.screenXRGB(AREA.VIEW))),
  audio: fnv(bytesOf(m.audio())),
};
const rgba = m.screenRGBA(AREA.ACTIVE);
out.rgba_len = rgba.length;
// RGBA（fc85_screen_rgba）は 0x00RRGGBB（fc85_screen）を並べ替えたもの
const xrgb = m.screenXRGB(AREA.ACTIVE);
out.rgba_ok = xrgb.every((p, i) =>
  rgba[4 * i] === p >>> 16 && rgba[4 * i + 1] === ((p >>> 8) & 0xff) &&
  rgba[4 * i + 2] === (p & 0xff) && rgba[4 * i + 3] === 255);

// 状態：保存して進め、戻して同じ入力で進めると同じ
const st = m.saveState();
out.state_len = st.length;
const go = () => {
  m.setInput(PAD.RIGHT);
  m.runFrames(30);
  return [fnv(bytesOf(m.screenXRGB(AREA.RASTER))), fnv(bytesOf(m.audio())), m.frame].join(" ");
};
const a = go();
m.loadState(st);
out.round_trip = a === go();
out.after = a;

m.poke(0, 0x100800, new Uint8Array([0x12, 0x34]));
out.peek = Array.from(m.peek(0, 0x100800, 2));
out.bios_version = Array.from(m.peek(0, 0x480, 2));
const r = m.call("regs");
out.pc = r.result.pc;

// MIDI（DR-126）：BIOS は受けた byte を受信のリング（bios_midi_head・tail・buf、$100612〜）に入れる
m.midiIn([0x90, 60, 100]);
m.runFrames(1);
out.midi_ring = Array.from(m.peek(0, 0x100612, 4 + 8));
out.midi_out_bios = m.midiOut().bytes.length;
// 来た byte を送り返す小さな Boot ROM：送り終えた時刻が付く
const echo = new Uint8Array(0x10000).fill(0xff);
echo.set([0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00]);
[0x13fc, 0x0003, 0x0026, 0x0014, 0x13fc, 0x0015, 0x0026, 0x0014, 0x0839, 0x0000, 0x0026, 0x0014, 0x67f6,
  0x1039, 0x0026, 0x0016, 0x0839, 0x0001, 0x0026, 0x0014, 0x67f6, 0x13c0, 0x0026, 0x0016, 0x60de]
  .forEach((w, i) => echo.set([w >> 8, w & 0xff], 0x100 + 2 * i));
const e = fc85.create({ boot: echo });
e.runFrames(1);
const t0 = e.timeUs;
e.midiIn([0x90, 64, 1, 0x80, 64, 0]);
e.runFrames(1);
const eo = e.midiOut();
out.midi_echo = { bytes: Array.from(eo.bytes), us: Array.from(eo.us, (u) => u - t0) };
e.free();
const asm = new MidiAssembler();
out.midi_assembled = [0x90, 60, 100, 62, 0xf8, 90, 0xc1, 5, 0xf0, 0x7e, 0xf7, 0xf2, 0x10, 0x20]
  .map((b) => asm.push(b)).filter(Boolean);
try {
  m.loadState(new Uint8Array([1, 2, 3]));
  out.bad_state = "通った";
} catch (e) {
  out.bad_state = e.message;
}
m.free();
console.log(JSON.stringify(out));
