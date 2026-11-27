// fc85emu.js : fc85 のエミュレータの WebAssembly（DR-104）を JavaScript から使う糊。ブラウザでも Node でも動く
// ES のモジュール。WebAssembly は C の ABI（emulator/crates/capi、見出し fc85emu.h）を wasm32 に組んだもの
// （py emulator/embed/build.py）で、取り込む関数はない。説明は sdk/docs/emulator-api.md §08。
//
//   import { load, AREA, PAD } from "./fc85emu.js";
//   const fc85 = await load("build/fc85emu.wasm");          // URL、Response、バイト列、Module
//   const m = fc85.create({ boot, chars });                  // Uint8Array（chars は省いてよい）
//   m.loadCart(cart);                                        // 省けば Boot ROM だけ
//   m.setInput(PAD.RIGHT | PAD.A);
//   m.runFrames(1);
//   ctx.putImageData(new ImageData(m.screenRGBA(AREA.VIEW), 320, 240), 0, 0);
//   const pcm = m.audio();                                   // Int16Array（L・R の順、48 kHz）
//   m.midiIn([0x90, 60, 100]);                               // MIDI IN（ACIA の線）
//   const { bytes, us } = m.midiOut();                       // MIDI OUT と送り終えた時刻（µs）
//
// 誤りは Error を投げる。WebAssembly の中の panic は止まる（RuntimeError）ので、その仮想機は捨てる。
// ライセンスは MIT（リポジトリの LICENSE）。

export const AREA = Object.freeze({ ACTIVE: 0, VIEW: 1, RASTER: 2 });
export const SPACE = Object.freeze({
  CPU: 0, VRAM: 1, WAVE: 2, DSP_PROG: 3, TEMPORAL: 4, CHAR_ROM: 5, PALETTE: 6,
});
export const PAD = Object.freeze({
  UP: 0x01, DOWN: 0x02, LEFT: 0x04, RIGHT: 0x08, A: 0x10, B: 0x20, C: 0x40, START: 0x80,
});
export const FLAGS = Object.freeze({ NO_DOC: 0x01, DIGITAL_AUDIO: 0x02 });

/**
 * MIDI の byte の列をメッセージに組み立てる（ランニングステータスを開く。リアルタイムはほかのメッセージの途中にも
 * 入る。SysEx は F0〜F7 で 1 つ）。Web MIDI の send は完全なメッセージしか受けないので、送る前に通す。
 * ホストの midi.rs の Assembler と同じ規則。
 */
export class MidiAssembler {
  constructor() {
    this.status = 0;
    this.data = [];
    this.sysex = null;
  }

  /** 1 byte を足し、メッセージがそろえば配列で返す（なければ null）。 */
  push(b) {
    if (b >= 0xf8) return [b];
    if (this.sysex) {
      this.sysex.push(b);
      if (b === 0xf7) {
        const s = this.sysex;
        this.sysex = null;
        return s;
      }
      if (b < 0x80) return null;
      // F7 の前に別のステータス：SysEx を捨てて読み直す
      this.sysex = null;
    }
    if (b === 0xf0) {
      this.sysex = [b];
      this.status = 0;
      return null;
    }
    if (b >= 0x80) {
      this.data = [];
      if (b >= 0xf0) {
        this.status = 0;
        if (b === 0xf6) return [b];
        if (b >= 0xf1 && b <= 0xf3) this.status = b;
        return null;
      }
      this.status = b;
      return null;
    }
    if (!this.status) return null;
    this.data.push(b);
    const s = this.status;
    const need = (s & 0xf0) === 0xc0 || (s & 0xf0) === 0xd0 ? 1 : s === 0xf2 ? 2 : s >= 0xf0 ? 1 : 2;
    if (this.data.length < need) return null;
    const msg = [s, ...this.data];
    this.data = [];
    if (s >= 0xf0) this.status = 0;
    return msg;
  }
}

/** WebAssembly を読む。source は URL（文字列か URL）、Response、ArrayBuffer・型付き配列、WebAssembly.Module。 */
export async function load(source) {
  let mod;
  if (source instanceof WebAssembly.Module) {
    mod = source;
  } else if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
    mod = await WebAssembly.compile(source);
  } else {
    const r = source instanceof Response ? source : await fetch(source);
    if (!r.ok) throw new Error(`fc85emu.wasm を読めない: ${r.status} ${r.url}`);
    mod = await WebAssembly.compile(await r.arrayBuffer());
  }
  const inst = await WebAssembly.instantiate(mod, {});
  return new Fc85Module(inst.exports);
}

const MAX_PIXELS = 342 * 262;
const AUDIO_CHUNK = 4096;

class Fc85Module {
  constructor(x) {
    this.x = x;
    if (x.fc85_abi_version() !== 1) throw new Error(`C の ABI の版が違う: ${x.fc85_abi_version()}`);
    this.audioRate = x.fc85_audio_rate();
    this.frameRate = x.fc85_frame_rate();
    this.version = this.cstr(x.fc85_version());
  }

  /** 今の WebAssembly のメモリのバイト列（領域を取ると作り直されるので、そのつど取る）。 */
  get u8() {
    return new Uint8Array(this.x.memory.buffer);
  }

  cstr(p) {
    const m = this.u8;
    let e = p;
    while (m[e] !== 0) e++;
    return new TextDecoder().decode(m.subarray(p, e));
  }

  error(what) {
    return new Error(`${what}: ${this.cstr(this.x.fc85_last_error())}`);
  }

  /** バイト列を WebAssembly の中に写して f(ptr, len) を呼ぶ。null なら (0, 0)。 */
  withBytes(bytes, f) {
    if (bytes == null || bytes.length === 0) return f(0, 0);
    const p = this.x.fc85_alloc(bytes.length);
    if (p === 0) throw new Error("領域を取れない");
    try {
      this.u8.set(bytes, p);
      return f(p, bytes.length);
    } finally {
      this.x.fc85_dealloc(p, bytes.length);
    }
  }

  screenWidth(area) {
    return this.x.fc85_screen_width(area);
  }

  screenHeight(area) {
    return this.x.fc85_screen_height(area);
  }

  /** 仮想機を作って電源を入れる。boot・chars は Uint8Array（なくてもよい）。 */
  create({ boot = null, chars = null, flags = 0 } = {}) {
    const m = this.withBytes(boot, (bp, bl) =>
      this.withBytes(chars, (cp, cl) => this.x.fc85_new(bp, bl, cp, cl, flags)));
    if (m === 0) throw this.error("fc85_new");
    return new Machine(this, m);
  }
}

class Machine {
  constructor(mod, ptr) {
    this.mod = mod;
    this.x = mod.x;
    this.ptr = ptr;
    this.pic = this.x.fc85_alloc(MAX_PIXELS * 4);
    this.pcm = this.x.fc85_alloc(AUDIO_CHUNK * 4);
  }

  check(r, what) {
    if (r < 0) throw this.mod.error(what);
    return r;
  }

  /** カートリッジを挿して電源を入れ直す（null で抜く）。 */
  loadCart(bytes) {
    this.mod.withBytes(bytes, (p, n) => this.check(this.x.fc85_load_cart(this.ptr, p, n), "fc85_load_cart"));
  }

  reset() {
    this.check(this.x.fc85_reset(this.ptr), "fc85_reset");
  }

  /** PORT_A・PORT_B（PAD のビットの OR）。 */
  setInput(portA, portB = 0) {
    this.check(this.x.fc85_set_input(this.ptr, portA & 0xffff, portB & 0xffff), "fc85_set_input");
  }

  runFrames(n = 1) {
    this.check(this.x.fc85_run_frames(this.ptr, n), "fc85_run_frames");
  }

  get frame() {
    return Number(this.x.fc85_frame(this.ptr));
  }

  /** 仮想機の今の時刻（電源を入れてからの µs）。 */
  get timeUs() {
    return Number(this.x.fc85_time_us(this.ptr));
  }

  /** MIDI IN の線に byte の列（配列か Uint8Array）を置く。今から delayUs µs 後に来たとする（DR-126）。 */
  midiIn(bytes, delayUs = 0) {
    this.mod.withBytes(Uint8Array.from(bytes), (p, n) =>
      this.check(this.x.fc85_midi_in(this.ptr, p, n, delayUs >>> 0), "fc85_midi_in"));
  }

  /** MIDI OUT：ACIA が送り終えた byte と、その時刻（µs）。{ bytes: Uint8Array, us: Float64Array }。 */
  midiOut() {
    const cap = 256;
    const p = this.x.fc85_alloc(cap * 9);
    try {
      const bytes = [];
      const us = [];
      for (;;) {
        const n = this.check(this.x.fc85_midi_out(this.ptr, p + cap * 8, p, cap), "fc85_midi_out");
        const view = new DataView(this.x.memory.buffer);
        for (let i = 0; i < n; i++) {
          us.push(Number(view.getBigUint64(p + 8 * i, true)));
          bytes.push(view.getUint8(p + cap * 8 + i));
        }
        if (n < cap) break;
      }
      return { bytes: Uint8Array.from(bytes), us: Float64Array.from(us) };
    } finally {
      this.x.fc85_dealloc(p, cap * 9);
    }
  }

  /** 画面を 0x00RRGGBB の Uint32Array で（写し）。 */
  screenXRGB(area = AREA.VIEW) {
    const n = this.check(this.x.fc85_screen(this.ptr, area, this.pic, MAX_PIXELS), "fc85_screen");
    return new Uint32Array(this.x.memory.buffer, this.pic, n).slice();
  }

  /** 画面を RGBA（ImageData の形）で。out を渡せばそこに書く。並べ替えは WebAssembly の中（fc85_screen_rgba）。 */
  screenRGBA(area = AREA.VIEW, out = null) {
    const n = this.check(this.x.fc85_screen_rgba(this.ptr, area, this.pic, MAX_PIXELS * 4), "fc85_screen_rgba");
    const src = new Uint8ClampedArray(this.x.memory.buffer, this.pic, 4 * n);
    if (out) {
      out.set(src);
      return out;
    }
    return src.slice();
  }

  /** 溜まった音をすべて取り出す（Int16Array、L・R の順）。 */
  audio() {
    const parts = [];
    let total = 0;
    for (;;) {
      const n = this.check(this.x.fc85_audio(this.ptr, this.pcm, AUDIO_CHUNK), "fc85_audio");
      parts.push(new Int16Array(this.x.memory.buffer, this.pcm, 2 * n).slice());
      total += 2 * n;
      if (n < AUDIO_CHUNK) break;
    }
    const out = new Int16Array(total);
    let o = 0;
    for (const p of parts) {
      out.set(p, o);
      o += p.length;
    }
    return out;
  }

  /** 状態を保存する（Uint8Array。同じ版のエミュレータで読める）。 */
  saveState() {
    const lenp = this.x.fc85_alloc(8);
    try {
      const p = this.x.fc85_save_state(this.ptr, lenp);
      if (p === 0) throw this.mod.error("fc85_save_state");
      const len = new Uint32Array(this.x.memory.buffer, lenp, 1)[0];
      const out = this.mod.u8.slice(p, p + len);
      this.x.fc85_free_bytes(p, len);
      return out;
    } finally {
      this.x.fc85_dealloc(lenp, 8);
    }
  }

  loadState(bytes) {
    this.mod.withBytes(bytes, (p, n) => this.check(this.x.fc85_load_state(this.ptr, p, n), "fc85_load_state"));
  }

  /** メモリを読む（バスを通さない）。 */
  peek(space, addr, len) {
    const p = this.x.fc85_alloc(len);
    try {
      this.check(this.x.fc85_peek(this.ptr, space, addr >>> 0, p, len), "fc85_peek");
      return this.mod.u8.slice(p, p + len);
    } finally {
      this.x.fc85_dealloc(p, len);
    }
  }

  poke(space, addr, bytes) {
    this.mod.withBytes(bytes, (p, n) =>
      this.check(this.x.fc85_poke(this.ptr, space, addr >>> 0, p, n), "fc85_poke"));
  }

  /** JSON の手順（sdk/docs/emulator-api.md）。method と params を渡し、答え（result か error）を返す。 */
  call(method, params = {}, id = 1) {
    const req = new TextEncoder().encode(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\0");
    const out = this.mod.withBytes(req, (p) => {
      const r = this.x.fc85_call(this.ptr, p);
      if (r === 0) throw this.mod.error("fc85_call");
      const s = this.mod.cstr(r);
      this.x.fc85_free_string(r);
      return s;
    });
    return JSON.parse(out);
  }

  free() {
    if (this.ptr === 0) return;
    this.x.fc85_dealloc(this.pic, MAX_PIXELS * 4);
    this.x.fc85_dealloc(this.pcm, AUDIO_CHUNK * 4);
    this.x.fc85_free(this.ptr);
    this.ptr = 0;
  }
}
