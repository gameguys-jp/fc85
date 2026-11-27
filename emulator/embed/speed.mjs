// WebAssembly の仮想機の速さ：Boot ROM（とカートリッジ）で何フレームか進め、1 フレームあたりの時間を出す。
// フロントエンドと同じく、毎フレーム音と画面（テレビの模型、320×240）を取り出す。3 回走らせた中の最短。
// 使い方：node speed.mjs <fc85emu.wasm> <boot.bin> <char.bin> [cart.bin か -] [フレーム数]
import { readFileSync } from "node:fs";
import { load, AREA } from "./fc85emu.js";

const [wasm, boot, chars, cart, n] = process.argv.slice(2);
const frames = Number(n || 240);
const fc85 = await load(readFileSync(wasm));
let best = Infinity;
for (let k = 0; k < 3; k++) {
  const m = fc85.create({ boot: readFileSync(boot), chars: readFileSync(chars) });
  if (cart && cart !== "-") m.loadCart(readFileSync(cart));
  const t = performance.now();
  for (let f = 0; f < frames; f++) {
    m.runFrames(1);
    m.audio();
    m.screenRGBA(AREA.VIEW);
  }
  best = Math.min(best, (performance.now() - t) / frames);
  m.free();
}
console.log(`${best.toFixed(2)} ms/フレーム（実時間は ${(1000 / fc85.frameRate).toFixed(2)} ms）`);
