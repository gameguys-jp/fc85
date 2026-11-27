// fc85 の仮想機を回す Web Worker（DR-107）。試しの頁（index.html）が module の Worker として作る。頁の主の
// スレッドは釦と描画だけにし、仮想機はここで回す（描画や頁の作業で仮想機が待たない）。
//
// 進め方：音を鳴らす AudioWorklet（audio.js）が取り出した組の数を返すので、まだ鳴っていない音（先行き）が
// 目標（約 50 ms から、尽きるたびに 1 フレーム分ずつ増やし、最大約 170 ms）を下回る間フレームを進める
// （音の時計に合わせる。仮想機のフレームは約 59.92 Hz で、画面の 60 Hz とは合わせない）。音がなければ
// performance.now() の時刻に合わせる。1 回に進めるのは 4 フレームまでで、間に知らせ（入力など）を受ける。
// 実時間に追いつかない機械では、実時間より遅く進む（音は途切れる）。
//
// 画面は、渡された OffscreenCanvas があればここで描き、なければ RGBA を主のスレッドへ送る。
//
// 受ける知らせ（type）：init（wasm の URL、canvas）、audio（AudioWorklet への MessagePort）、boot（ROM、
// DOC の子基板）、run（on）、input（pad）、reset、save、load（state）、bench（frames）、bend（cmd。仕様書 §11
// のベンド点の命令を窓口の JSON の手順の bend で。DR-124）。
// 送る知らせ：ready、frame（canvas がないとき）、stat（1 秒ごと）、saved、loaded、bench、bent（summary、mode）、
// error。
// ライセンスは MIT（リポジトリの LICENSE）。
import { load, AREA, FLAGS } from "./fc85emu.js";

const W = 320;
const H = 240;
const LEAD_MIN = 2400;
const LEAD_MAX = 8000;
const BATCH = 4;

let fc85 = null;
let m = null;
let roms = null;
let canvas = null;
let image = null;
let link = null;
let running = false;
let pad = 0;
// 前のフレームの後に押された釦（1 フレームより短い押下も、次のフレームでは押したことにする）
let tapped = 0;
let timer = 0;
// 音の時計：送った組の数と、AudioWorklet が取り出した組の数
let produced = 0;
let consumed = 0;
let underruns = 0;
let queued = 0;
let lead = LEAD_MIN;
// 時刻の時計（音がないとき）
let last = 0;
let owed = 0;
// 統計
let statAt = 0;
let statFrames = 0;
let busy = 0;

const post = (msg, transfer = []) => self.postMessage(msg, transfer);

// 待たずに次の pump を呼ぶ（setTimeout(0) は重ねると 4 ms に切り上がる）。知らせ（入力など）は間に受ける
const soon = new MessageChannel();
soon.port1.onmessage = () => pump();
const again = (ms) => (ms > 0 ? setTimeout(pump, ms) : (soon.port2.postMessage(0), -1));

function fail(e) {
  running = false;
  if (m) {
    try {
      m.free();
    } catch {
      // WebAssembly の中で止まった仮想機は、返せなくても捨てる
    }
  }
  m = null;
  post({ type: "error", message: e && e.message ? e.message : String(e), fatal: true });
}

function build() {
  if (m) m.free();
  m = null;
  m = fc85.create({ boot: roms.boot, chars: roms.chars, flags: roms.doc ? 0 : FLAGS.NO_DOC });
  if (roms.cart) m.loadCart(roms.cart);
  flush();
}

/** AudioWorklet の溜まりを捨てる（リセット、状態を戻す、一時停止）。 */
function flush() {
  if (link) link.postMessage({ flush: true });
  consumed = produced;
}

function frame() {
  m.setInput(pad | tapped);
  tapped = 0;
  m.runFrames(1);
  const pcm = m.audio();
  if (link && pcm.length > 0) {
    produced += pcm.length >> 1;
    link.postMessage({ pcm }, [pcm.buffer]);
  }
  statFrames += 1;
}

function present() {
  if (canvas) {
    m.screenRGBA(AREA.VIEW, image.data);
    canvas.putImageData(image, 0, 0);
  } else {
    const px = m.screenRGBA(AREA.VIEW);
    post({ type: "frame", px }, [px.buffer]);
  }
}

function pump() {
  timer = 0;
  if (!running || !m) return;
  const t0 = performance.now();
  let n = 0;
  try {
    if (link) {
      while (n < BATCH && produced - consumed < lead) {
        frame();
        n += 1;
      }
    } else {
      owed = Math.min(owed + ((t0 - last) / 1000) * fc85.frameRate, BATCH);
      last = t0;
      while (n < BATCH && owed >= 1) {
        owed -= 1;
        frame();
        n += 1;
      }
    }
    if (n > 0) present();
  } catch (e) {
    fail(e);
    return;
  }
  const t1 = performance.now();
  busy += t1 - t0;
  if (t1 - statAt >= 1000) stat(t1);
  timer = again(n >= BATCH ? 0 : 4);
}

function stat(t) {
  const s = (t - statAt) / 1000;
  post({
    type: "stat",
    frame: m ? m.frame : 0,
    fps: statFrames / s,
    speed: statFrames / s / fc85.frameRate,
    load: busy / (t - statAt),
    // AudioWorklet が最後に知らせた溜まり（鳴らしていない音）
    lead: link ? (queued / fc85.audioRate) * 1000 : null,
    target: link ? (lead / fc85.audioRate) * 1000 : null,
    underruns: link ? underruns : null,
  });
  statAt = t;
  statFrames = 0;
  busy = 0;
}

function setRunning(on) {
  running = on && m !== null;
  if (running) {
    last = statAt = performance.now();
    owed = 0;
    statFrames = 0;
    busy = 0;
    if (!timer) timer = again(0);
  } else {
    flush();
  }
}

/** 同じ ROM で別の仮想機を作り、今の場面（動かしている仮想機の状態を写す。まだなければ電源投入）から
 *  frames フレームを止まらずに進めた 1 フレームあたりの時間。釦は今のまま。毎フレーム音と画面（テレビの
 *  模型の 320×240）を取り出す（speed.mjs と同じ手順）。動かしている仮想機は変えない。 */
function bench(frames) {
  const b = fc85.create({ boot: roms.boot, chars: roms.chars, flags: roms.doc ? 0 : FLAGS.NO_DOC });
  try {
    if (roms.cart) b.loadCart(roms.cart);
    if (m) b.loadState(m.saveState());
    b.setInput(pad);
    const px = new Uint8ClampedArray(W * H * 4);
    const t = performance.now();
    for (let i = 0; i < frames; i++) {
      b.runFrames(1);
      b.audio();
      b.screenRGBA(AREA.VIEW, px);
    }
    return (performance.now() - t) / frames;
  } finally {
    b.free();
  }
}

self.onmessage = async (e) => {
  const d = e.data;
  try {
    switch (d.type) {
      case "init": {
        fc85 = await load(new URL(d.wasm, d.base));
        if (d.canvas) {
          canvas = d.canvas.getContext("2d");
          image = canvas.createImageData(W, H);
        }
        post({ type: "ready", version: fc85.version, frameRate: fc85.frameRate, audioRate: fc85.audioRate });
        break;
      }
      case "audio":
        link = d.link;
        link.onmessage = (a) => {
          consumed = Math.max(consumed, a.data.consumed);
          queued = a.data.queued;
          if (a.data.underruns > underruns && running) {
            lead = Math.min(lead + Math.round(fc85.audioRate / fc85.frameRate), LEAD_MAX);
          }
          underruns = a.data.underruns;
        };
        // 時刻の時計から音の時計へ（動いている途中でもよい）
        produced = consumed = queued = 0;
        break;
      case "boot":
        roms = { boot: d.boot, chars: d.chars, cart: d.cart, doc: d.doc };
        lead = LEAD_MIN;
        build();
        break;
      case "run":
        setRunning(d.on);
        break;
      case "input":
        tapped |= d.pad & ~pad;
        pad = d.pad;
        break;
      case "reset":
        if (m) {
          m.reset();
          flush();
        }
        break;
      case "save":
        if (m) post({ type: "saved", state: m.saveState() });
        break;
      case "load":
        if (m) {
          m.loadState(d.state);
          flush();
          if (!running) present();
          post({ type: "loaded" });
        }
        break;
      case "bench": {
        if (!roms) throw new Error("ROM を読んでいない");
        const was = running;
        setRunning(false);
        const ms = bench(d.frames);
        post({ type: "bench", ms, frames: d.frames });
        setRunning(was);
        break;
      }
      case "bend": {
        if (!m) throw new Error("仮想機がない");
        const r = m.call("bend", { cmd: d.cmd });
        if (r.error) throw new Error(r.error.message ?? JSON.stringify(r.error));
        post({ type: "bent", summary: r.result.summary, mode: r.result.mode });
        break;
      }
    }
  } catch (err) {
    // WebAssembly の中で止まった（panic）仮想機は捨てる。戻り値の誤り（読めない状態など）なら仮想機は続ける
    if (err instanceof WebAssembly.RuntimeError || !m) fail(err);
    else post({ type: "error", message: err.message, fatal: false });
  }
};
