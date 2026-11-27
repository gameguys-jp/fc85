// WebAssembly の仮想機を Node で動かし、試験（test_embed.py）が C の ABI の結果と突き合わせる値を 1 行の JSON で
// 出す。使い方：node check.mjs <fc85emu.wasm> <boot.bin> <char.bin> [cart.bin]
import { readFileSync } from "node:fs";
import { load, AREA, PAD } from "./fc85emu.js";

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
try {
  m.loadState(new Uint8Array([1, 2, 3]));
  out.bad_state = "通った";
} catch (e) {
  out.bad_state = e.message;
}
m.free();
console.log(JSON.stringify(out));
