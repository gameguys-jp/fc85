// fc85 の音を鳴らす AudioWorklet（DR-107）。試しの頁（index.html）が読み、Web Worker（worker.js）が
// MessagePort で送る音（Int16Array、L・R の順、48 kHz）を順に鳴らす。AudioContext の標本化の周波数が
// 48 kHz でなければ、隣り合う標本の間を直線で補う。
//
// - 溜まりが尽きたら（Worker が実時間に追いつかない）、最後の値から 0 へ下げて待ち、2 フレーム分
//   （約 1602 組）溜まってから鳴らし直す（細かく途切れ続けないように）。尽きた回数を数える。
// - 約 21 ms ごとに、取り出した組の数（consumed）と溜まっている組の数（queued）と尽きた回数を Worker に
//   返す。Worker はこれで進める速さを決める（音の時計に合わせる）。
// - { flush: true } を受けたら、それより前に受けた音を捨てる（捨てた分も取り出した数に入れる）。
// 同じファイルの fc85-in は Audio IN（DR-129、DR-130）：頁がつないだマイクの音を Worker へ送る（下）。
// ライセンスは MIT（リポジトリの LICENSE）。

const RESUME = 1602;
const REPORT = 1024;

class Fc85Out extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const o = (options && options.processorOptions) || {};
    this.step = (o.sourceRate || sampleRate) / sampleRate;
    this.queue = [];
    this.head = null;
    this.at = 0;
    this.queued = 0;
    this.consumed = 0;
    this.underruns = 0;
    // 補う 2 点（cur → nxt）と、出している値（level）。配列を作らないよう L・R を別に持つ
    this.curL = 0;
    this.curR = 0;
    this.nxtL = 0;
    this.nxtR = 0;
    this.levL = 0;
    this.levR = 0;
    this.frac = 0;
    this.starved = true;
    this.since = 0;
    this.link = null;
    this.port.onmessage = (e) => {
      if (e.data && e.data.link) {
        this.link = e.data.link;
        this.link.onmessage = (m) => this.take(m.data);
      }
    };
  }

  take(d) {
    if (d.flush) {
      this.consumed += this.queued;
      this.queue = [];
      this.head = null;
      this.at = 0;
      this.queued = 0;
      this.starved = true;
      this.report();
    }
    if (d.pcm && d.pcm.length >= 2) {
      this.queue.push(d.pcm);
      this.queued += d.pcm.length >> 1;
    }
  }

  /** 次の 1 組を nxt に取る。なければ false。 */
  pop() {
    while (!this.head || this.at >= this.head.length) {
      if (this.queue.length === 0) return false;
      this.head = this.queue.shift();
      this.at = 0;
    }
    this.nxtL = this.head[this.at] / 32768;
    this.nxtR = this.head[this.at + 1] / 32768;
    this.at += 2;
    this.queued -= 1;
    this.consumed += 1;
    return true;
  }

  report() {
    if (this.link) {
      this.link.postMessage({ consumed: this.consumed, queued: this.queued, underruns: this.underruns });
    }
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    const l = out[0];
    const r = out[1] || out[0];
    for (let i = 0; i < l.length; i++) {
      if (this.starved && this.queued >= RESUME) {
        // 鳴らし直す：下げた値から続ける
        this.starved = false;
        this.curL = this.levL;
        this.curR = this.levR;
        this.frac = 0;
        this.pop();
      }
      if (this.starved) {
        this.levL *= 0.995;
        this.levR *= 0.995;
      } else {
        const f = this.frac;
        this.levL = this.curL + (this.nxtL - this.curL) * f;
        this.levR = this.curR + (this.nxtR - this.curR) * f;
        this.frac += this.step;
        while (this.frac >= 1) {
          this.frac -= 1;
          this.curL = this.nxtL;
          this.curR = this.nxtR;
          if (!this.pop()) {
            this.starved = true;
            this.underruns += 1;
            break;
          }
        }
      }
      l[i] = this.levL;
      r[i] = this.levR;
    }
    this.since += l.length;
    if (this.since >= REPORT) {
      this.since = 0;
      this.report();
    }
    return true;
  }
}

registerProcessor("fc85-out", Fc85Out);

/**
 * Audio IN（DR-129、DR-130）：頁がつないだ音（マイク・ライン入力）を 48 kHz の L・R の Int16Array にして、
 * 512 組（約 10.7 ms）ごとに Worker へ送る。AudioContext が 48 kHz でなければ、隣り合う標本の間を直線で補う
 * （ホストの audio_in.rs と同じ）。1 チャネルの入力は L・R の両方に。出力は無音（グラフに残すためにつなぐ）。
 */
class Fc85In extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const o = (options && options.processorOptions) || {};
    // 出力の 1 標本の間に進む入力の標本の数と、次の出力の位置（前の入力を 0、今の入力を 1）
    this.step = sampleRate / (o.targetRate || 48000);
    this.pos = 1;
    this.prevL = 0;
    this.prevR = 0;
    this.buf = new Int16Array(2 * 512);
    this.n = 0;
    this.link = null;
    this.port.onmessage = (e) => {
      if (e.data && e.data.link) this.link = e.data.link;
    };
  }

  emit(l, r) {
    this.buf[2 * this.n] = Math.max(-32768, Math.min(32767, Math.round(l * 32768)));
    this.buf[2 * this.n + 1] = Math.max(-32768, Math.min(32767, Math.round(r * 32768)));
    this.n += 1;
    if (this.n === 512) {
      if (this.link) {
        const pcm = this.buf;
        this.link.postMessage({ pcm }, [pcm.buffer]);
        this.buf = new Int16Array(2 * 512);
      }
      this.n = 0;
    }
  }

  process(inputs) {
    const inp = inputs[0];
    if (!inp || inp.length === 0) return true;
    const l = inp[0];
    const r = inp[1] || inp[0];
    for (let i = 0; i < l.length; i++) {
      while (this.pos <= 1) {
        const f = this.pos;
        this.emit(this.prevL + (l[i] - this.prevL) * f, this.prevR + (r[i] - this.prevR) * f);
        this.pos += this.step;
      }
      this.pos -= 1;
      this.prevL = l[i];
      this.prevR = r[i];
    }
    return true;
  }
}

registerProcessor("fc85-in", Fc85In);
