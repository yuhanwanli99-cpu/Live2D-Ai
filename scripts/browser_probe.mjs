#!/usr/bin/env node
/**
 * 零依赖 CDP 浏览器探针（WSL2 headless Chrome + CDP，Node 22 自带全局 WebSocket）。
 *
 * 入口（唯一）：
 *   node scripts/browser_probe.mjs [scenario]         # 默认 all
 *   scenario 逗号分隔，可选项见 SCENARIOS
 *
 * 幂等、可一键重跑：每个 scenario 自建 page（Target.createTarget → 用完 close），
 * 不改仓库任何源码，只写 docs/verification/evidence-2026-10-06/**。
 * 环境变量覆盖：PROBE_BASE / PROBE_CDP / PROBE_OUT。
 *
 * 设计纪律：
 * - 只写证据，不"顺手修"页面；页面里跑的全是真实浏览器行为。
 * - 断不了言的写 manual-only，不写 pass（本项目教训：自检说谎比没有自检更坏）。
 * - 每个 scenario 的原始事件（网络 / 控制台）落 JSON，判定字符串落在 run.json。
 * - **截图纪律**（2026-10-06 实测修正，见 SHOT_PARAMS 头注）：一律整页截图
 *   （带 captureBeyondViewport），**不传 clip**，裁剪交给 png_stats 的 --crop。
 *   拿到「整帧单色平帧（纯白等）」= 无头合成器的空白帧 ⇒ 该条判 **blocked（环境）**，
 *   **不得**降级成产品 fail。
 * - 背景注入走**真字节库**（IndexedDB，产品 schema），不是往 localStorage 塞 dataUrl
 *   （v0.2.0 水合只认 id，塞 dataUrl 会被水合按「旧档搬运 + 剪枝」的时序吃掉）。
 */
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '..');
const OUT = process.env.PROBE_OUT || join(ROOT, 'docs/verification/evidence-2026-10-06');
const BASE = process.env.PROBE_BASE || 'http://127.0.0.1:18080';
const CDP_HTTP = process.env.PROBE_CDP || 'http://127.0.0.1:9222';
const APP = BASE + '/app/';
const VW = 1440, VH = 900;
// ── 截图参数（判据真源；2026-10-06 实测，原始矩阵见 evidence-2026-10-06/screenshot-mode-matrix.json）──
//
// **两条都实测过；缺任一条都会拿到「全白帧」，而全白帧不是产品状态。**
//
// 1. captureBeyondViewport: true —— 同一时刻、同一页面，本环境下不带它时
//    Page.captureScreenshot 可能回一整帧纯白（实测 5851 B / 100% ffffff），
//    带上它则是真画面（51899 B；主导色 ffffff 71.6% 恰好是舞台 iframe 那块
//    WebGPU canvas，其余 28% 是壳）。它把「回放合成器上一帧」换成「强制重新光栅化」，
//    无头 swiftshader 下前者会给空白帧。
// 2. **不传 clip** —— 实测在「整页截图有内容」的同一时刻（t=3000ms），带 clip 的
//    截图仍是 100% 纯白（340x848 全白，2434 B），而整页帧是内容帧（53131 B，
//    主导色 000000 77%）。clip 走的是另一条裁剪捕获路径。所以裁剪一律在
//    拿到整页帧之后用 png_stats.py --crop 做（脚本里的 py([shot,'--crop',...])）。
const SHOT_PARAMS = { format: 'png', captureBeyondViewport: true };
// 壳那一半（**不含**舞台 iframe）：舞台 canvas 在无头截图里恒为纯白，
// 背景 / 主题只能在这一半上量。x 1100..1440 = 聊天面板列，y 52..900。
const SHELL_CROP = '1100,52,1440,900';
const PREFS_KEY = 'live2d-ai.display-prefs';
// 像素统计器：优先用**本轮证据目录**里的那份（自包含），没有就回落到
// docs/verification/evidence-2026-10-05/png_stats.py（零依赖 stdlib 脚本，
// 两处是同一份；10-05 那份**不属于本轮写权限**，所以只读、不改）。
const PNG_STATS = (() => {
  const local = join(OUT, 'png_stats.py');
  if (existsSync(local)) return local;
  const shared = join(ROOT, 'docs/verification/evidence-2026-10-05/png_stats.py');
  if (existsSync(shared)) return shared;
  throw new Error('找不到 png_stats.py：' + OUT + ' 与 ' + shared + ' 都没有');
})();

for (const d of ['', 'bg', 'themes', 'stage', 'ui', 'fonts', 'settings', 'stage-class', 'audio']) mkdirSync(join(OUT, d), { recursive: true });

// ───────────────────────────── 结果记录 ─────────────────────────────
const RESULTS = [];
function rec(id, title, verdict, evidence, confidence = 'high', repro = '') {
  RESULTS.push({ id, title, verdict, evidence, confidence, repro });
  const mark = { pass: 'PASS', fail: 'FAIL', blocked: 'BLOCKED', 'manual-only': 'MANUAL' }[verdict] || verdict;
  console.log('[' + mark + '] ' + id + ' ' + title + ' — ' + evidence);
}
function save(name, obj) { writeFileSync(join(OUT, name), JSON.stringify(obj, null, 2)); }
function py(args) { return JSON.parse(execFileSync('python3', [PNG_STATS, ...args, '--json'], { encoding: 'utf8' })); }

// ─────────────────── 截图「可判读性」判据（唯一真源） ───────────────────
/** 整帧是不是**单色平帧**：主导色占比 > 99.9%。这种帧没有任何信息量。 */
function flatFrame(st) { return !!(st && st.dominant && st.dominant[0] && st.dominant[0].share > 0.999); }
/** 整帧 100% 纯白：本环境「无头合成器空白帧」的签名。
 *
 * 为什么纯白就是空白帧而不是产品状态：本探针的判据场景都在**默认黑主题**下取像素
 *（loadApp({clear:true}) 清掉偏好后 theme 回落 black），壳区不可能 100% 纯白；
 * 而实测在「同一时刻整页帧有内容」时，带 clip 的捕获偏偏回的就是 100% 纯白
 *（见 SHOT_PARAMS 头注 2）。所以「壳区 100% 纯白平帧」= 这一枪不可判读
 * ⇒ 判 **blocked（环境）**，不是产品缺陷。
 *
 * 注意：白主题下壳区本来就白，但那是**主题判据**（themes 场景用 12 条面带量），
 * 不走这条；这条只在默认黑主题的截图上用。 */
function blankWhite(st) { return flatFrame(st) && st.dominant[0].hex === 'ffffff'; }
/** 一次像素统计：整页截图 → 进程内裁剪（不依赖 CDP clip）。 */
async function shotStats(page, name, crop = null) {
  const p = await page.shot(name);
  const st = crop ? py([p, '--crop', crop]) : py([p]);
  return { path: p, bytes: statSync(p).size, stats: st, blank: blankWhite(st) };
}

/** 一张背景图的 id —— 与 Dart 的 backgroundIdOf **同算法**（djb2 变体，
 * h = (h*33 + codeUnit) mod (2^31-1)，初值 5381，输出 'bg' + 8 位十六进制）。
 * 真源：shell/flutter/lib/design/background_item.dart 的 backgroundFingerprint。
 * 为什么必须一致：id 同时是 IndexedDB 的键与偏好里的身份；不一致就会被水合
 * 当成「两个不同的项」，或留下读不回来的孤儿记录。 */
function backgroundIdOf(dataUrl) {
  let h = 5381;
  for (let i = 0; i < dataUrl.length; i++) h = (h * 33 + dataUrl.charCodeAt(i)) % 0x7fffffff;
  return 'bg' + h.toString(16).padStart(8, '0');
}

/** 在每个新文档里记录 WebSocket 帧（目标 B 的 WS 判据）。
 *
 * **只留投影**：audio 帧记 start/end/sentence_seq/样本数，正文帧记字符数，
 * error 帧记 code/stage/message。整段 base64 PCM **不进记录**（几 MB 会把
 * 页面和证据 JSON 一起撑爆）。帧 schema 真源：
 * crates/live2d-ai-desktop/src/web_api/ws/audio.rs（{"type":"audio","data":{...}}）。
 *
 * 用 patch 构造函数而不是 Network.webSocketFrameReceived 的理由：前者一定拿到
 * **文本原文**（Network 域的 payload 在大帧上可能给不完整数据），且与页面同栈。
 * 代价是必须在页面任何脚本之前注入 —— spyWs() 走
 * Page.addScriptToEvaluateOnNewDocument，即在真正 loadApp 之前。 */
const WS_NL = String.fromCharCode(10);
const WS_SPY_SOURCE = [
  '(function () {',
  "  window.__ws = { opened: 0, url: null, frames: [], closes: [], errors: [] };",
  "  var Orig = window.WebSocket;",
  "  function Spy(url, protocols) {",
  "    var ws = protocols === undefined ? new Orig(url) : new Orig(url, protocols);",
  "    window.__ws.opened += 1; window.__ws.url = String(url);",
  "    ws.addEventListener('message', function (ev) {",
  "      var d = String(ev.data); var rec = { len: d.length };",
  "      try {",
  "        var j = JSON.parse(d); var x = j.data || {}; rec.type = j.type;",
  "        if (j.type === 'audio') {",
  "          var chars = (typeof x.audio === 'string') ? x.audio.length : -1;",
  "          var bytes = 0;",
  "          if (chars > 0) { try { bytes = atob(x.audio).length; } catch (e) { bytes = -1; } }",
  "          rec.audioChars = chars; rec.audioBytes = bytes; rec.samples = bytes > 0 ? bytes / 2 : 0;",
  "          rec.start = x.start === true; rec.end = x.end === true;",
  "          rec.sentence_seq = (typeof x.sentence_seq === 'number') ? x.sentence_seq : null;",
  "          rec.sample_rate = x.sample_rate || null; rec.slice_ms = x.slice_ms || null;",
  "          rec.volume = (typeof x.volume === 'number') ? x.volume : null; rec.muted = x.muted === true;",
  "        } else if (j.type === 'error') {",
  "          rec.error = { code: x.code, stage: x.stage, message: String(x.message || '').slice(0, 200), fatal: x.fatal === true };",
  "        } else if (j.type === 'text_delta' || j.type === 'reasoning_delta' || j.type === 'text_fallback' || j.type === 'text') {",
  "          rec.textChars = (typeof x.text === 'string') ? x.text.length : -1;",
  "        }",
  "      } catch (e) { rec.raw = d.slice(0, 160); }",
  "      window.__ws.frames.push(rec);",
  "    });",
  "    ws.addEventListener('close', function () { window.__ws.closes.push(Date.now()); });",
  "    ws.addEventListener('error', function () { window.__ws.errors.push(Date.now()); });",
  "    return ws;",
  "  }",
  "  Spy.prototype = Orig.prototype;",
  "  var K = ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED'];",
  "  for (var i = 0; i < K.length; i++) Spy[K[i]] = Orig[K[i]];",
  "  window.WebSocket = Spy;",
  "})();",
].join(WS_NL);

/** 把一次会话的 WS 音频帧**归并成句子**（纯函数，给判定与证据共用）。
 *
 * 为什么按 sentence_seq 归并而不是整体计数：task 书上那条老缺陷正是
 * 「0 个 start、N 个 end」——总数看不出这种事，只有**逐句**才看得出。 */
function summarizeAudioFrames(frames) {
  const audio = frames.filter((f) => f && f.type === 'audio');
  const bySeq = new Map();
  for (const f of audio) {
    const seq = f.sentence_seq === null || f.sentence_seq === undefined ? 'null' : f.sentence_seq;
    if (!bySeq.has(seq)) bySeq.set(seq, { seq, frames: 0, starts: 0, ends: 0, samples: 0, maxVolume: 0, muted: false });
    const g = bySeq.get(seq);
    g.frames += 1;
    if (f.start) g.starts += 1;
    if (f.end) g.ends += 1;
    g.samples += f.samples || 0;
    if (typeof f.volume === 'number' && f.volume > g.maxVolume) g.maxVolume = f.volume;
    if (f.muted) g.muted = true;
  }
  const sentences = [...bySeq.values()].sort((a, b) => String(a.seq).localeCompare(String(b.seq), 'en', { numeric: true }));
  const seqs = sentences.map((s) => s.seq).filter((s) => s !== 'null');
  const numeric = seqs.map(Number).filter((n) => Number.isFinite(n));
  const strictlyIncreasingFrom1 = numeric.length > 0 && numeric[0] === 1 &&
    numeric.every((n, i) => i === 0 || n > numeric[i - 1]);
  return {
    audioFrames: audio.length,
    sentences,
    totalStarts: audio.filter((f) => f.start).length,
    totalEnds: audio.filter((f) => f.end).length,
    totalSamples: audio.reduce((a, f) => a + (f.samples || 0), 0),
    anyMuted: audio.some((f) => f.muted === true),
    seqs, strictlyIncreasingFrom1,
    everySentenceOneStartOneEnd: sentences.length > 0 && sentences.every((s) => s.starts === 1 && s.ends === 1),
    otherTypes: [...new Set(frames.map((f) => f.type || 'raw'))],
  };
}


// ───────────────────────────── 小工具 ─────────────────────────────
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const J = (v) => JSON.stringify(v);

let CRC = null;
function crc32(buf) {
  if (!CRC) {
    CRC = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      CRC[n] = c;
    }
  }
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}
/** 生成一张纯 stdlib PNG（无依赖），返回 dataURL。 */
function makePngDataUrl(w, h, pixel) {
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) {
    const o = y * (w * 3 + 1);
    raw[o] = 0;
    for (let x = 0; x < w; x++) {
      const rgb = pixel(x, y);
      const i = o + 1 + x * 3;
      raw[i] = rgb[0]; raw[i + 1] = rgb[1]; raw[i + 2] = rgb[2];
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 2;
  const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0)),
  ]);
  return 'data:image/png;base64,' + png.toString('base64');
}
const IMG = {
  red: makePngDataUrl(64, 64, () => [255, 0, 0]),
  blue: makePngDataUrl(64, 64, () => [0, 0, 255]),
  green: makePngDataUrl(64, 64, () => [0, 255, 0]),
  // 非方形 + 上下两色：cover / contain / stretch / tile 四档在像素上必然不同
  tall: makePngDataUrl(40, 160, (x, y) => (y < 80 ? [0, 255, 0] : [255, 0, 255])),
};

// ───────────────────────────── CDP 客户端 ─────────────────────────────
class Cdp {
  constructor(wsUrl) { this.wsUrl = wsUrl; this._id = 0; this._pending = new Map(); this._handlers = []; }
  static async connect(wsUrl) { const c = new Cdp(wsUrl); await c._open(); return c; }
  _open() {
    return new Promise((res, rej) => {
      const ws = new WebSocket(this.wsUrl);
      this.ws = ws;
      ws.addEventListener('open', () => res());
      ws.addEventListener('error', () => rej(new Error('CDP 连接失败：' + this.wsUrl)));
      ws.addEventListener('message', (ev) => this._onMessage(String(ev.data)));
    });
  }
  _onMessage(text) {
    let m; try { m = JSON.parse(text); } catch { return; }
    if (m.id !== undefined && this._pending.has(m.id)) {
      const p = this._pending.get(m.id); this._pending.delete(m.id);
      if (m.error) p.reject(new Error(m.error.message + ' (' + m.error.code + ')')); else p.resolve(m.result);
      return;
    }
    for (const h of this._handlers) if (h.method === m.method) { try { h.fn(m); } catch { /* 记录器不许打断主流程 */ } }
  }
  send(method, params = {}, sessionId) {
    const id = ++this._id;
    const msg = { id, method, params };
    if (sessionId) msg.sessionId = sessionId;
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => { if (this._pending.has(id)) { this._pending.delete(id); reject(new Error('CDP 超时 ' + method)); } }, 60000);
      this._pending.set(id, { resolve: (v) => { clearTimeout(t); resolve(v); }, reject: (e) => { clearTimeout(t); reject(e); } });
      this.ws.send(JSON.stringify(msg));
    });
  }
  on(method, fn) { this._handlers.push({ method, fn }); }
  once(method, timeoutMs = 30000) {
    return new Promise((resolve, reject) => {
      const h = { method, fn: (m) => { this._handlers = this._handlers.filter((x) => x !== h); clearTimeout(t); resolve(m); } };
      this._handlers.push(h);
      const t = setTimeout(() => { this._handlers = this._handlers.filter((x) => x !== h); reject(new Error('事件超时 ' + method)); }, timeoutMs);
    });
  }
  close() { try { this.ws.close(); } catch { /* 已关 */ } }
}

class Page {
  constructor(browser, sessionId, targetId) {
    this.browser = browser; this.sessionId = sessionId; this.targetId = targetId;
    this.recording = false;
    this.net = { requests: [], responses: [], failures: [] };
    this.console = []; this.exceptions = []; this.logs = [];
  }
  send(m, p) { return this.browser.send(m, p, this.sessionId); }
  static async create(browser) {
    const t = await browser.send('Target.createTarget', { url: 'about:blank' });
    const a = await browser.send('Target.attachToTarget', { targetId: t.targetId, flatten: true });
    const page = new Page(browser, a.sessionId, t.targetId);
    await page.send('Page.enable');
    await page.send('Runtime.enable');
    await page.send('Network.enable');
    await page.send('Network.setCacheDisabled', { cacheDisabled: true });
    await page.send('Network.setBypassServiceWorker', { bypass: true });
    await page.send('Emulation.setDeviceMetricsOverride', { width: VW, height: VH, deviceScaleFactor: 1, mobile: false });
    await page.send('Log.enable').catch(() => {});
    browser.on('Network.requestWillBeSent', (m) => { if (page.recording) page.net.requests.push({ url: m.params.request.url, method: m.params.request.method, type: m.params.type }); });
    browser.on('Network.responseReceived', (m) => { if (page.recording) page.net.responses.push({ url: m.params.response.url, status: m.params.response.status, mime: m.params.response.mimeType, fromDiskCache: !!m.params.response.fromDiskCache }); });
    browser.on('Network.loadingFailed', (m) => { if (page.recording) page.net.failures.push({ errorText: m.params.errorText, blockedReason: m.params.blockedReason || null }); });
    browser.on('Runtime.consoleAPICalled', (m) => {
      if (!page.recording) return;
      const frames = (m.params.stackTrace && m.params.stackTrace.callFrames) || [];
      const frameUrl = frames.length ? frames[0].url : '';
      page.console.push({
        level: m.params.type,
        text: m.params.args.map((x) => x.value ?? x.description ?? x.type).join(' ').slice(0, 400),
        frame: frameUrl || null,
        renderer: /\/render\//.test(frameUrl),
      });
    });
    browser.on('Runtime.exceptionThrown', (m) => { if (page.recording) page.exceptions.push({ text: m.params.exceptionDetails.text, desc: (m.params.exceptionDetails.exception || {}).description }); });
    browser.on('Log.entryAdded', (m) => { if (page.recording) page.logs.push({ level: m.params.entry.level, source: m.params.entry.source, text: m.params.entry.text.slice(0, 400) }); });
    return page;
  }
  async evaluate(expression, awaitPromise = true) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise, userGesture: true });
    if (r.exceptionDetails) throw new Error('page eval 抛错：' + r.exceptionDetails.text + ' ' + ((r.exceptionDetails.exception || {}).description || ''));
    return r.result.value;
  }
  /** **整页**截图（不带 clip，见 SHOT_PARAMS 头注）。opts 保留只为兼容旧调用点，
   *  **不再**接受 clip —— 带 clip 的捕获在本环境会稳定回纯白帧。裁剪请用 png_stats --crop。 */
  async shot(name, opts = {}) {
    if (opts.clip) throw new Error('shot() 不再支持 clip（见 SHOT_PARAMS 头注）：请用 png_stats --crop');
    const r = await this.send('Page.captureScreenshot', SHOT_PARAMS);
    const p = join(OUT, name);
    writeFileSync(p, Buffer.from(r.data, 'base64'));
    return p;
  }
  /** 等「真的画出来了」。
   *
   * 判据：**整页**截图（captureBeyondViewport）→ png_stats `--crop` 取壳区 →
   * 不是「100% 纯白平帧」（见 blankWhite）。最多等 timeoutMs。
   *
   * 返回 {paintedAfterMs, dominant, bytes, crop, samples}；一直判不出来时
   * paintedAfterMs = null 并带 why —— 调用方据此判 **blocked（环境）**，
   * 不是产品 fail（task-7 目标 A2 的硬要求）。*/
  async waitForPaint(timeoutMs = 40000, crop = SHELL_CROP) {
    const samples = [];
    const t0 = Date.now();
    while (Date.now() - t0 < timeoutMs) {
      try {
        const r = await this.send('Page.captureScreenshot', SHOT_PARAMS);
        const buf = Buffer.from(r.data, 'base64');
        writeFileSync('/tmp/probe-paint.png', buf);
        const st = py(['/tmp/probe-paint.png', '--crop', crop]);
        const white = blankWhite(st);
        samples.push({ at: Date.now() - t0, bytes: buf.length, dominant: st.dominant[0], blank: white });
        if (!white) {
          return { paintedAfterMs: Date.now() - t0, dominant: st.dominant[0], bytes: buf.length, crop, samples };
        }
      } catch (e) { samples.push({ at: Date.now() - t0, error: String(e.message).slice(0, 120) }); }
      await sleep(1500);
    }
    return {
      paintedAfterMs: null, dominant: null, bytes: null, crop, samples,
      why: timeoutMs + ' ms 内整页截图在壳区(' + crop + ')始终是 100% 纯白平帧（无头合成器空白帧）',
    };
  }
  async waitFor(expr, timeoutMs = 30000, label = expr) {
    const t0 = Date.now();
    while (Date.now() - t0 < timeoutMs) {
      try { if (await this.evaluate(expr)) return true; } catch { /* 导航中 */ }
      await sleep(300);
    }
    throw new Error('等待超时：' + label);
  }
  async reload() {
    const loaded = this.browser.once('Page.loadEventFired').catch(() => null);
    await this.send('Page.reload', { ignoreCache: true });
    await loaded;
    await this.waitFor("!!document.querySelector('flt-glass-pane') || !!document.querySelector('canvas')", 30000, 'Flutter 首帧容器');
  }
  /** 在每个新文档（含 iframe）里记录 window 'message' 事件，用于观测**父页下发的协议帧**。 */
  async spyFrames() {
    await this.send('Page.addScriptToEvaluateOnNewDocument', {
      source: "window.__msgs = []; var _add = window.addEventListener; window.addEventListener = function (t, f, o) { if (t === 'message') { _add.call(window, 'message', function (ev) { try { window.__msgs.push(String(ev.data)); } catch (e) {} }); } return _add.call(this, t, f, o); };",
    });
  }
  /** 在建页之前注入 WS 帧记录器（见 WS_SPY_SOURCE 头注）。 */
  async spyWs() {
    await this.send('Page.addScriptToEvaluateOnNewDocument', { source: WS_SPY_SOURCE });
  }
  /** 读 WS 记录器（需要先 spyWs 且页面已加载过）。 */
  wsFrames() {
    return this.evaluate('window.__ws || null');
  }
  /** 读 iframe 收到的帧（需要先 spyFrames）。 */
  frames() {
    return this.evaluate("(function () { var f = document.querySelector('iframe'); try { return f && f.contentWindow.__msgs ? f.contentWindow.__msgs.slice() : ['<no-spy>']; } catch (e) { return ['ERR:' + e.message]; } })()").then((list) => list.map((s) => { try { return JSON.parse(s); } catch { return s; } }));
  }
  async loadApp(opts = {}) {
    const url = opts.url || APP;
    const loaded = this.browser.once('Page.loadEventFired').catch(() => null);
    await this.send('Page.navigate', { url });
    await loaded;
    await this.waitFor("!!document.querySelector('flt-glass-pane') || !!document.querySelector('canvas')", 30000, 'Flutter 首帧容器');
    if (opts.clear || opts.prefs) {
      await this.evaluate('localStorage.clear()');
      if (opts.prefs) {
        await this.evaluate('localStorage.setItem(' + J(PREFS_KEY) + ',' + J(J(opts.prefs)) + ')');
      }
      // 必须重新加载：清完 localStorage 不刷新的话，**已经在内存里的**偏好
      // 还是上一轮那套（本轮实测：clear 之后主题仍是上一个场景选的白）。
      await this.reload();
    }
    await sleep(opts.settleMs === undefined ? 6000 : opts.settleMs);
    this.lastPaint = await this.waitForPaint();
  }
  /** 写偏好并重载。
   *
   * **背景库走真字节库**（task-7 目标 A3）：patch.backgrounds 里的 dataUrl 会被
   * 写进 IndexedDB（产品 schema），localStorage 里只留 {kind:'image', id} ——
   * 与产品「导入图片」之后落盘的样子**逐字段一致**。为什么不能只往 localStorage
   * 塞 dataUrl：v0.2.0 的水合会认 id 不认 dataUrl（DisplayPrefs.toJson 只写 id），
   * 而注入与产品水合谁先跑不确定，旧写法读回 dataUrl=null（len:0）就是这么来的。 */
  async setPrefs(patch) {
    const raw = await this.evaluate('localStorage.getItem(' + J(PREFS_KEY) + ')');
    const cur = raw ? JSON.parse(raw) : {};
    const next = { ...cur, ...patch };
    if (Array.isArray(patch.backgrounds)) next.backgrounds = await this.storeBackgroundBytes(patch.backgrounds);
    await this.evaluate('localStorage.setItem(' + J(PREFS_KEY) + ',' + J(J(next)) + ')');
    await this.reload();
    await sleep(4000);
    this.lastPaint = await this.waitForPaint();
  }
  /** 把 [{kind:'image', dataUrl}] 写进 IndexedDB 字节库，返回偏好该留的 id-only 清单。
   *
   * schema 真源：shell/flutter/lib/data/background_store_web.dart
   *   db = 'live2d-ai'（version 1），store = 'backgrounds'（**out-of-line key**），
   *   key = 背景 id（bg + 8 位十六进制），value = dataURL 字符串。
   * id 由内容算：backgroundIdOf() 与 Dart 的 backgroundFingerprint 同算法
   *   （djb2：h = (h*33 + codeUnit) mod (2^31-1)，初值 5381）。
   * 若不是图片项 / 没有 dataUrl，则**原样保留**（只当搬运工，不替产品做判断）。 */
  async storeBackgroundBytes(items) {
    const plan = items
      .filter((it) => it && it.kind === 'image' && typeof it.dataUrl === 'string' && it.dataUrl.length > 0)
      .map((it) => ({ id: it.id || backgroundIdOf(it.dataUrl), dataUrl: it.dataUrl }));
    if (!plan.length) return items;
    const ids = await this.evaluate(
      '(async () => {' +
      '  const plan = ' + J(plan) + ';' +
      "  const db = await new Promise((res, rej) => { const req = indexedDB.open('live2d-ai', 1);" +
      "    req.onupgradeneeded = () => { const d = req.result; if (!d.objectStoreNames.contains('backgrounds')) d.createObjectStore('backgrounds'); };" +
      '    req.onsuccess = () => res(req.result); req.onerror = () => rej(new Error(\'idb open error\')); req.onblocked = () => rej(new Error(\'idb open blocked\')); });' +
      "  const out = await new Promise((res, rej) => { const tx = db.transaction(['backgrounds'], 'readwrite'); const s = tx.objectStore('backgrounds');" +
      '    for (const p of plan) s.put(p.dataUrl, p.id);' +
      "    tx.oncomplete = () => res(plan.map((p) => p.id)); tx.onabort = () => rej(new Error('tx abort')); tx.onerror = () => rej(new Error('tx error')); });" +
      '  db.close(); return out; })()');
    let k = 0;
    return items.map((it) => {
      if (it && it.kind === 'image' && typeof it.dataUrl === 'string' && it.dataUrl.length > 0) {
        const id = ids[k++] || backgroundIdOf(it.dataUrl);
        const out = { kind: 'image', id };
        for (const f of ['opacity', 'fit', 'align']) if (it[f] !== undefined) out[f] = it[f];
        return out;
      }
      return it;
    });
  }
  /** 字节库里现有哪些 id（证明字节真的落进了 IndexedDB，而不是只写在偏好里）。 */
  async idbKeys() {
    return this.evaluate(
      '(async () => {' +
      "  const db = await new Promise((res, rej) => { const req = indexedDB.open('live2d-ai', 1);" +
      "    req.onupgradeneeded = () => { const d = req.result; if (!d.objectStoreNames.contains('backgrounds')) d.createObjectStore('backgrounds'); };" +
      '    req.onsuccess = () => res(req.result); req.onerror = () => rej(new Error(\'idb open error\')); });' +
      "  const keys = await new Promise((res, rej) => { const tx = db.transaction(['backgrounds'], 'readonly'); const s = tx.objectStore('backgrounds');" +
      "    const r = s.getAllKeys(); r.onsuccess = () => res(r.result); r.onerror = () => rej(new Error('keys error')); });" +
      '  db.close(); return keys; })()');
  }
  /** 页面上真实存在的媒体元素（目标 B 的「前端真的建了 <audio> 并播放」判据）。 */
  audioElements() {
    return this.evaluate("[...document.querySelectorAll('audio')].map(function (a) { var s = a.currentSrc || a.src || ''; return { src: s.slice(0, 60), blob: s.indexOf('blob:') === 0, duration: a.duration, currentTime: a.currentTime, paused: a.paused, readyState: a.readyState, muted: a.muted, volume: a.volume }; })");
  }
  async wipePrefs() {
    await this.evaluate('localStorage.clear()');
    await this.reload();
    await sleep(4000);
    this.lastPaint = await this.waitForPaint();
  }
  async prefsNow() {
    const raw = await this.evaluate('localStorage.getItem(' + J(PREFS_KEY) + ')');
    return raw ? JSON.parse(raw) : null;
  }
  async enableSemantics() {
    const r = await this.evaluate("(() => { const p = document.querySelector('flt-semantics-placeholder'); if (p) { p.click(); return 'clicked'; } return document.querySelector('flt-semantics') ? 'already' : 'none'; })()");
    await sleep(1500);
    const n = await this.evaluate("document.querySelectorAll('flt-semantics').length");
    return { how: r, count: n };
  }
  async semantics() {
    // 注意：Flutter Web 的语义 DOM **不一定**把文案放 aria-label——有些节点是
    // 纯文本子节点。只读 aria-label 会漏掉大半（本轮实测：31 个 flt-semantics
    // 里只有 4 个有 aria-label）。
    const expr = "[...document.querySelectorAll('flt-semantics')].map(function (e) { var b = e.getBoundingClientRect(); return { label: e.getAttribute('aria-label'), text: (e.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 120), role: e.getAttribute('role'), x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) }; }).filter(function (n) { return (n.label || n.text) && n.w > 0 && n.h > 0; })";
    return this.evaluate(expr);
  }
  /** 语义节点的可见文案（aria-label 优先，回落到文本子节点）。 */
  static nodeText(n) { return (n && (n.label || n.text)) || ''; }
  /** Flutter 的文本编辑宿主（shadow root）里的 **textarea**——聊天输入框就是它。 */
  async chatInput() {
    return this.evaluate("(function () { var h = document.querySelector('flt-text-editing-host'); var root = h && h.shadowRoot ? h.shadowRoot : document; var el = root.querySelector('textarea'); if (!el) return null; var b = el.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; })()");
  }
  /** 等聊天输入框（shadow root 里的 textarea）出现；等不到返回 null。 */
  async waitForChatInput(timeoutMs = 20000) {
    const t0 = Date.now();
    while (Date.now() - t0 < timeoutMs) {
      const r = await this.chatInput().catch(() => null);
      if (r) return r;
      await sleep(500);
    }
    return null;
  }
  /** 聊天输入框里**真实**的字符串（用来证明 insertText 真的进去了）。 */
  async typedText() {
    return this.evaluate("(function () { var h = document.querySelector('flt-text-editing-host'); var root = h && h.shadowRoot ? h.shadowRoot : document; var el = root.querySelector('textarea'); return el ? el.value : null; })()");
  }
  async click(x, y) {
    for (const type of ['mousePressed', 'mouseReleased']) {
      await this.send('Input.dispatchMouseEvent', { type, x: Math.round(x), y: Math.round(y), button: 'left', clickCount: 1, buttons: type === 'mousePressed' ? 1 : 0 });
      await sleep(70);
    }
    await sleep(900);
  }
  async clickLabel(substr, opts = {}) {
    // 取**面积最小**的命中节点：大容器的 textContent 会把子节点文案全拼进来，
    // 直接取第一个会点到整块面板上（本轮实测踩过）。
    const list = (await this.semantics())
      .filter((n) => Page.nodeText(n).includes(substr))
      .sort((a, b) => a.w * a.h - b.w * b.h);
    if (!list.length) return null;
    const node = list[Math.min(opts.index || 0, list.length - 1)];
    await this.click(node.x + node.w / 2, node.y + node.h / 2);
    return node;
  }
  async drag(x1, y1, x2, y2, steps = 14) {
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: Math.round(x1), y: Math.round(y1), button: 'left', clickCount: 1, buttons: 1 });
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(x1 + (x2 - x1) * t), y: Math.round(y1 + (y2 - y1) * t), button: 'left', buttons: 1 });
      await sleep(45);
    }
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: Math.round(x2), y: Math.round(y2), button: 'left', buttons: 0 });
    await sleep(1000);
  }
  stageRect() {
    return this.evaluate("(function () { var f = document.querySelector('iframe'); if (!f) return null; var b = f.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; })()");
  }
  hud() {
    return this.evaluate("(function () { var f = document.querySelector('iframe'); try { var d = f && f.contentDocument; var el = d && d.getElementById('status'); return el ? el.textContent : null; } catch (e) { return 'ERR:' + e.message; } })()");
  }
  async close() { await this.browser.send('Target.closeTarget', { targetId: this.targetId }).catch(() => {}); }
}

/** 语义节点的可见文案：aria-label 优先，回落到文本子节点（Flutter Web 两种都用）。 */
const T = (n) => (n && (n.label || n.text)) || '';

// ───────────────────────────── 外部源判定 ─────────────────────────────
const EXTERNAL = /^https?:\/\/(?!127\.0\.0\.1|localhost)/i;
function externalUrls(urls) { return [...new Set(urls.filter((u) => EXTERNAL.test(u)))]; }
function hostOf(u) { try { return new URL(u).host; } catch { return '?'; } }
function fpsOf(s) { const m = /FPS ([0-9.]+)/.exec(s || ''); return m ? parseFloat(m[1]) : null; }

// ───────────────────────────── 场景 ─────────────────────────────
async function scenarioFiles() {
  const dir = join(ROOT, 'shell/flutter/build/web');
  const names = ['index.html', 'main.dart.js', 'flutter_bootstrap.js', 'flutter.js', 'flutter_service_worker.js', 'version.json', 'manifest.json'];
  const rows = [];
  for (const n of names) {
    let text = null;
    try { text = execFileSync('cat', [join(dir, n)], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }); } catch { /* 缺文件 */ }
    if (text === null) { rows.push({ file: n, exists: false }); continue; }
    const urls = [...new Set((text.match(/https?:\/\/[^\s"'\x60)\\]}]+/g) || []))];
    rows.push({
      file: n, exists: true, bytes: text.length,
      gstaticHits: (text.match(/gstatic/gi) || []).length,
      externalUrls: urls.filter((u) => EXTERNAL.test(u)),
      sample: (text.match(/[^\n]{0,60}gstatic[^\n]{0,60}/i) || [''])[0].slice(0, 160),
    });
  }
  // bootstrap 里真正的 CDN 决策
  const boot = rows.find((r) => r.file === 'flutter_bootstrap.js');
  const useLocal = boot && boot.exists;
  let cfg = null;
  if (useLocal) {
    const text = execFileSync('cat', [join(dir, 'flutter_bootstrap.js')], { encoding: 'utf8' });
    const m = /useLocalCanvasKit["']?\s*:\s*(true|false)/.exec(text);
    cfg = m ? m[1] : null;
  }
  save('build-artifacts.json', { dir, rows, useLocalCanvasKit: cfg });
  rec('F-0050-01a', '四个构建产物里 gstatic / 外部 URL 命中数',
    'pass',
    rows.map((r) => r.file + '=' + (r.exists ? r.gstaticHits : 'MISSING')).join(' ') +
    '；externalUrls=' + JSON.stringify(rows.map((r) => [r.file, (r.externalUrls || []).length])),
    'high', 'node scripts/browser_probe.mjs files');
  rec('F-0050-01b', 'gstatic 字符串是否可达（bootstrap 的 CDN 分支）',
    cfg === 'true' ? 'pass' : (cfg === 'false' ? 'fail' : 'blocked'),
    'flutter_bootstrap.js 的 _flutter.buildConfig.useLocalCanvasKit=' + cfg +
    '；引擎 helper 形如 n.canvasKitBaseUrl ? … : (engineRevision && !useLocalCanvasKit ? "https://www.gstatic.com/flutter-canvaskit/…" : "canvaskit")' +
    ' ⇒ useLocalCanvasKit=true 时该分支恒不可达，实际 base 为同源 "canvaskit"',
    cfg === null ? 'low' : 'high');
  // 产物新鲜度
  const mtime = (p) => { try { return statSync(p).mtimeMs; } catch { return null; } };
  const mainJs = join(dir, 'main.dart.js');
  const walk = (p, acc = []) => {
    for (const e of execFileSync('find', [p, '-name', '*.dart'], { encoding: 'utf8' }).trim().split('\n').filter(Boolean)) acc.push(e);
    return acc;
  };
  const darts = walk(join(ROOT, 'shell/flutter/lib'));
  let newest = 0, newestFile = '';
  for (const f of darts) { const t = mtime(f); if (t && t > newest) { newest = t; newestFile = f; } }
  const js = mtime(mainJs);
  const fresh = js !== null && newest > 0 && js >= newest;
  rec('8b', '前端产物新鲜度（main.dart.js 不早于最新的 lib/*.dart）',
    fresh ? 'pass' : 'fail',
    'main.dart.js=' + (js ? new Date(js).toISOString() : 'missing') + '；最新 dart=' +
    (newest ? new Date(newest).toISOString() + ' ' + newestFile.replace(ROOT + '/', '') : 'none') +
    '；dart 文件 ' + darts.length + ' 个',
    'high', 'PROBE 的 files 场景');
}

async function scenarioNet(browser) {
  const page = await Page.create(browser);
  try {
    page.recording = true;
    await page.loadApp({ clear: true, settleMs: 9000 });
    await page.shot('shell-after-load.png');
    const urls = page.net.requests.map((r) => r.url);
    const ext = externalUrls(urls);
    // 跨源请求按 host 分类（**只打印，不当判据**）：R4-T4 正在改字体回落
    // （web/flutter_bootstrap.js），落地后这条应当变成「0 条跨源」；
    // 最终判定由 Lead 在全量重建后跑，这里只如实给出分类与原始 URL。
    const appOrigin = new URL(APP).origin;
    const crossOrigin = page.net.requests.filter((r) => { try { return new URL(r.url).origin !== appOrigin; } catch { return false; } });
    const crossByHost = {};
    for (const r of crossOrigin) { const h = hostOf(r.url); crossByHost[h] = (crossByHost[h] || 0) + 1; }
    save('net-crossorigin.json', { appOrigin, count: crossOrigin.length, byHost: crossByHost, urls: crossOrigin.map((r) => r.url) });
    save('net.json', { requests: page.net.requests, responses: page.net.responses, failures: page.net.failures, console: page.console, exceptions: page.exceptions, logs: page.logs, crossOriginByHost: crossByHost });
    rec('1d', '跨源请求按 host 分类（只打印，不当判据）', 'manual-only',
      crossOrigin.length === 0 ? '0 条跨源请求（同源清单：' + [...new Set(urls.map(hostOf))].join(',') + '）'
        : crossOrigin.length + ' 条：' + JSON.stringify(crossByHost),
      'high', 'node scripts/browser_probe.mjs net（原始见 net-crossorigin.json）');
    rec('1a', '加载 /app/ 后零外部源请求', ext.length === 0 ? 'pass' : 'fail',
      '共 ' + urls.length + ' 条请求，外部源 ' + ext.length + ' 条' + (ext.length ? '：' + ext.join(', ') : '') +
      '；hosts=' + [...new Set(urls.map(hostOf))].join(','), 'high',
      'node scripts/browser_probe.mjs net（原始事件见 net.json）');
    const cv = page.net.responses.filter((r) => /canvaskit/.test(r.url)).map((r) => r.status + ' ' + r.url);
    rec('1b', 'CanvasKit 实际从同源 canvaskit/ 加载', cv.some((s) => s.startsWith('200 ') && s.includes('/app/canvaskit/')) ? 'pass' : 'fail',
      cv.join(' | ') || '没有 canvaskit 请求', 'high');
    const models = page.net.responses.filter((r) => /\/models\//.test(r.url)).map((r) => r.status + ' ' + r.url);
    rec('1c', '模型资产走本机静态路由且 200', models.length > 0 && models.every((s) => s.startsWith('2')) ? 'pass' : 'fail',
      models.length + ' 条：' + models.slice(0, 6).join(' | ') || '无', 'high');
    // 渲染面的 status() **所有**状态行都走 console.error（不止 HUD 行），
    // 所以按**调用栈来源帧**区分：来自 /render/ 的一律是渲染面自身日志。
    const rendererErr = page.console.filter((c) => c.level === 'error' && c.renderer);
    const shellErr = [...page.console.filter((c) => c.level === 'error' && !c.renderer).map((c) => c.text),
      ...page.exceptions.map((e) => 'exception: ' + e.text + ' ' + (e.desc || '')),
      ...page.logs.filter((l) => l.level === 'error').map((l) => l.source + ': ' + l.text)];
    const nonOk = page.net.responses.filter((r) => r.status >= 400);
    // /actions/field_map.json 与 /actions/presets.json 是**可选**覆盖表
    // （main.rs 头注：不存在就回落渲染面内建默认表）⇒ 404 属设计内。
    const optionalMiss = nonOk.filter((r) => /\/actions\/(field_map|presets)\.json$/.test(r.url));
    const unexpectedMiss = nonOk.filter((r) => !/\/actions\/(field_map|presets)\.json$/.test(r.url));
    // 已知环境噪声：CanvasKit 在无头 swiftshader 下偶发 WebGL CONTEXT_LOST。
    // 浏览器给的 network 日志只有状态码、没有 URL，所以按「可选 404 的条数」配平：
    // 有 N 条设计内 404，就核销掉 N 条 network 404 日志（多出来的仍算真错误）。
    let budget404 = optionalMiss.length;
    const benign = [];
    const realErr = [];
    for (const t of shellErr) {
      if (/field_map\.json|presets\.json|CONTEXT_LOST_WEBGL/.test(t)) { benign.push(t); continue; }
      if (budget404 > 0 && /Failed to load resource: the server responded with a status of 404/.test(t)) { budget404--; benign.push(t); continue; }
      realErr.push(t);
    }
    const glWarn = page.logs.filter((l) => /CONTEXT_LOST_WEBGL/.test(l.text));
    save('console.json', { rendererErrorLines: rendererErr, shellErrors: shellErr, benign, realErrors: realErr, glContextLost: glWarn, nonOk, all: page.console, logs: page.logs, exceptions: page.exceptions });
    rec('2', '控制台：渲染面自身日志 / 设计内 404 / 真错误 三者分开',
      realErr.length === 0 ? 'pass' : 'fail',
      '渲染面帧的 console.error ' + rendererErr.length + ' 条（status() 的所有状态行都走 console.error）；' +
      '设计内 404 ' + optionalMiss.length + ' 条（' + optionalMiss.map((r) => r.url).join(',') + '，可选覆盖表，回落内建默认）；' +
      '未预期 4xx/5xx ' + unexpectedMiss.length + ' 条；WebGL CONTEXT_LOST_WEBGL 噪声 ' + glWarn.length + ' 条（无头 swiftshader）；' +
      '其余真错误 ' + realErr.length + ' 条：' + JSON.stringify(realErr.slice(0, 3)), 'high',
      'node scripts/browser_probe.mjs net（原始见 console.json / net.json）');
  } finally { page.recording = false; await page.close(); }
}

async function scenarioOffline(browser) {
  const page = await Page.create(browser);
  try {
    await page.send('Network.setBlockedURLs', { urls: ['*://*.gstatic.com/*', '*://*.googleapis.com/*', '*://*.google.com/*', '*://*.gstatic.cn/*', '*://*.googletagmanager.com/*'] });
    page.recording = true;
    await page.loadApp({ clear: true, settleMs: 10000 });
    // 整页截图 + 进程内裁剪（不带 clip）；壳区像素才是「有没有画出来」的判据。
    const shot = await shotStats(page, 'offline-blocked-external.png', SHELL_CROP);
    const st = shot.stats;
    const shotStage = await page.shot('stage/offline-stage.png');
    const ext = externalUrls(page.net.requests.map((r) => r.url));
    const blocked = page.net.failures.filter((f) => f.blockedReason);
    const hudText = await page.hud();
    save('offline.json', { externalUrls: ext, failures: page.net.failures, pixelsShell: st, shotBytes: shot.bytes, paint: page.lastPaint, hud: hudText });
    // 「有没有真的画出来」的判据：壳区主导色的占比不接近 1（纯色/白屏 ⇒ 无 UI）。
    const drawn = 1 - st.dominant[0].share > 0.02;
    const theme = (await page.prefsNow() || {}).theme || 'black';
    // 判据不可用时**判 blocked（环境）**，不判 fail（task-7 目标 A2）：
    // 100% 纯白平帧 = 无头合成器空白帧，它说明不了产品好坏。
    // 「不可判读」= 本次判据用的那张帧是 100% 纯白平帧。它**就是** waitForPaint()
    // 返回 null 的同一个判据（同一个 blankWhite()，同一条 SHELL_CROP），所以
    // waitForPaint() 判不出来的场景在这里必然也判不出来 ⇒ 判 blocked（环境），
    // 并把 waitForPaint 的原始依据（采样次数 / 字节数 / why）打进证据。
    const unusable = shot.blank;
    const paintWhy = page.lastPaint && page.lastPaint.why ? page.lastPaint.why : null;
    const verdict = unusable ? 'blocked' : (ext.length === 0 && drawn ? 'pass' : 'fail');
    rec('offline', '外部主机全拦截后应用仍可用（断网不白屏）', verdict,
      (unusable ? '【环境不可判读】壳区截图是 100% 纯白平帧（' + shot.bytes + ' B），无头合成器没给内容帧；' : '') +
      '被 blocked 的外部请求 ' + blocked.length + ' 条；请求里外部 URL ' + ext.length + ' 条；主题=' + theme +
      '；壳区主导色=' + JSON.stringify(st.dominant.slice(0, 3)) + '（主导占比 ' + st.dominant[0].share + '，<0.98 视为有真实绘制）' +
      '；截图字节=' + shot.bytes +
      '；waitForPaint=' + (page.lastPaint ? (page.lastPaint.paintedAfterMs === null ? 'null（判不出）' : page.lastPaint.paintedAfterMs + ' ms') : 'n/a') +
      '（采样 ' + (page.lastPaint && page.lastPaint.samples ? page.lastPaint.samples.length : 0) + ' 次' + (paintWhy ? '；why=' + paintWhy : '') + '）；HUD=' + String(hudText).slice(0, 120),
      unusable ? 'low' : (drawn ? 'high' : 'medium'), 'node scripts/browser_probe.mjs offline');
  } finally { page.recording = false; await page.close(); }
}

async function scenarioStage(browser) {
  const page = await Page.create(browser);
  try {
    page.recording = true;
    await page.loadApp({ clear: true, settleMs: 10000 });
    const rect = await page.stageRect();
    const h1 = await page.hud();
    await sleep(3000);
    const h2 = await page.hud();
    const models = page.net.responses.filter((r) => /\/models\//.test(r.url)).map((r) => r.status + ' ' + r.url);
    // 整页截图 + **进程内裁剪**（不带 clip，见 SHOT_PARAMS 头注）。
    const stageCrop = rect && rect.w > 50
      ? [rect.x, rect.y, rect.x + rect.w, rect.y + rect.h].map((v) => Math.round(v)).join(',')
      : null;
    const shot = await page.shot('stage/stage.png');
    const cropArgs = stageCrop ? ['--crop', stageCrop] : [];
    let st = null; try { st = py([shot, ...cropArgs]); } catch (e) { st = { error: String(e.message) }; }
    // 「模型有没有真的画出来」：默认主题的舞台底色是 #000000，
    // 所以舞台裁剪区里**非纯黑像素的占比**就是角色的可见面积。
    const prefs0 = await page.prefsNow();
    const theme0 = (prefs0 && prefs0.theme) || 'black';
    const stageHex = (PALETTE[theme0] || PALETTE.black).stage;
    let modelPix = null;
    try { modelPix = py([shot, ...cropArgs, '--colors', stageHex]); } catch (e) { modelPix = { error: String(e.message) }; }
    const shotFull = await page.shot('stage/shell-with-stage.png');
    save('stage.json', { rect, stageCrop, hud1: h1, hud2: h2, modelResponses: models, pixelsStage: st, modelPixels: modelPix, stageShot: shot, fullShot: shotFull });
    // 舞台像素级验收在本环境**做不到**，而且必须当场证明它做不到（不许默认通过）：
    // 强制把 stageColor 改成 #ff0000，若截图仍与之前逐像素相同 ⇒ canvas 内容
    // 根本没进截图（本环境实测正是如此）。
    let canvasWhite = null;
    if (rect) {
      try {
        await page.evaluate("document.querySelector('iframe').contentWindow.postMessage(JSON.stringify({version:1,type:'sync',payload:{stageColor:'#ff0000'}}), '*')");
        await sleep(1500);
        const shotRed = await page.shot('stage/stage-forced-red.png');
        const d = py([shot, '--diff', shotRed]).diff.fraction;
        const redStats = py([shotRed, ...cropArgs]);
        canvasWhite = { diffFractionAfterForcingRed: d, forcedRedDominant: redStats.dominant.slice(0, 2), forcedRedChannels: redStats.channels };
      } catch (e) { canvasWhite = { error: String(e.message) }; }
    }
    save('stage/stage-canvas-white-evidence.json', { theme: theme0, stageHex, modelPixels: modelPix, canvasWhite });
    rec('3d', '截图能否用来验证舞台像素（角色 / 底色）', 'manual-only',
      '主题=' + theme0 + '；舞台裁剪区主导色=' + JSON.stringify((modelPix.dominant || []).slice(0, 2)) +
      '；把 stageColor 强制改成 #ff0000 后与改动前的逐像素差异=' + (canvasWhite && canvasWhite.diffFractionAfterForcingRed) +
      '，强制后主导色=' + JSON.stringify(canvasWhite && canvasWhite.forcedRedDominant) +
      ' ⇒ 无头截图**拍不到 WebGPU canvas 内容**（恒为纯白），舞台像素级验收必须人工',
      'high', 'node scripts/browser_probe.mjs stage（原始见 stage/stage-canvas-white-evidence.json）');
    rec('3a', '模型资产 HTTP 200（bai.model3.json 等）', models.length > 0 && models.every((s) => s.startsWith('2')) ? 'pass' : 'fail',
      models.length + ' 条：' + models.slice(0, 8).join(' | '), 'high');
    const hudOk = /GPU:/.test(h2 || '') && /bg: (solid|image)/.test(h2 || '');
    rec('3b', '渲染面 HUD 有 GPU 行 + bg: 字段', hudOk ? 'pass' : 'fail', String(h2).slice(0, 220), 'high');
    const f1 = fpsOf(h1), f2 = fpsOf(h2);
    rec('3c', 'FPS 在推进（两次采样相隔 3s）', f1 !== null && f2 !== null && f2 > 5 ? 'pass' : 'fail',
      'fps1=' + f1 + ' fps2=' + f2 + '（HUD: ' + String(h2).slice(0, 120) + '）', 'high');
  } finally { page.recording = false; await page.close(); }
}

async function scenarioUi(browser) {
  const page = await Page.create(browser);
  try {
    await page.loadApp({ clear: true, settleMs: 8000 });
    // Flutter(CanvasKit) 把画布放在 shadow root 里，document.querySelectorAll('canvas') 数不到 ⇒ 必须穿透
    const expr = "(function () { var n = 0; var hosts = []; var walk = function (root) { n += root.querySelectorAll('canvas').length; root.querySelectorAll('*').forEach(function (e) { if (e.shadowRoot) { hosts.push(e.tagName.toLowerCase()); walk(e.shadowRoot); } }); }; walk(document); var c = document.createElement('canvas'); var gl2 = c.getContext('webgl2'); var gl1 = c.getContext('webgl'); return { title: document.title, canvasesDeep: n, shadowHosts: hosts, iframes: [...document.querySelectorAll('iframe')].map(function (f) { var b = f.getBoundingClientRect(); return { src: f.getAttribute('src'), w: Math.round(b.width), h: Math.round(b.height) }; }), glassPane: !!document.querySelector('flt-glass-pane'), flutterView: !!document.querySelector('flutter-view'), semanticsPlaceholder: !!document.querySelector('flt-semantics-placeholder'), bodyChildren: [...document.body.children].map(function (e) { return e.tagName.toLowerCase(); }), viewport: { w: innerWidth, h: innerHeight }, dpr: devicePixelRatio, webgl2: !!gl2, webgl: !!gl1, ua: navigator.userAgent }; })()";
    const dom = await page.evaluate(expr);
    await page.shot('ui/shell.png');
    const sem = await page.enableSemantics();
    const nodes = await page.semantics();
    save('ui.json', { dom, semantics: sem, nodes, htmlLen: await page.evaluate('document.documentElement.outerHTML.length') });
    rec('4a', 'Flutter 壳渲染容器存在（glass-pane + canvas + iframe）',
      dom.glassPane && dom.canvasesDeep >= 1 && dom.iframes.length >= 1 ? 'pass' : 'fail', JSON.stringify(dom).slice(0, 400), 'high');
    const wanted = ['设置', '发送', '输入', '聊天'];
    const found = wanted.filter((w) => nodes.some((s) => T(s).includes(w)));
    rec('4b', '语义树（真实 DOM）里能找到壳控件',
      nodes.length > 0 ? 'pass' : 'blocked',
      '语义节点 ' + nodes.length + ' 个（enableSemantics=' + JSON.stringify(sem) + '）；命中词 ' + JSON.stringify(found) +
      '；样例 ' + JSON.stringify(nodes.slice(0, 8).map((s) => T(s))), nodes.length > 0 ? 'high' : 'low',
      'Flutter Web 语义树要 flt-semantics-placeholder 被点一下才建；本脚本已点');
    return { page, nodes };
  } finally { /* 由调用方关闭 */ }
}

// ───────────── 设置面板（check 7）与主题（check 6）+ 背景（check 5） ─────────────

const PALETTE = {
  black: { stage: '000000', bands: ['11121b', '1d1e2a', '252634'] },
  white: { stage: 'ffffff', bands: ['f7f5f3', 'edece9', 'e2e0de'] },
  blue: { stage: '061223', bands: ['161e30', '212a3f', '28324b'] },
  gray: { stage: '1c1c1f', bands: ['282623', '353330', '3e3c38'] },
};
const THEME_UI = [['black', '黑，'], ['white', '白，'], ['blue', '蓝，'], ['gray', '灰，']];
const ALL_BANDS = Object.values(PALETTE).flatMap((p) => p.bands);
const perceived = (hex) => {
  const r = parseInt(hex.slice(0, 2), 16), g = parseInt(hex.slice(2, 4), 16), b = parseInt(hex.slice(4, 6), 16);
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
};

async function openSettingsPanel(page) {
  const sem = await page.enableSemantics();
  let nodes = await page.semantics();
  const btn = nodes.find((n) => T(n) === '设置') || nodes.find((n) => T(n).includes('设置'));
  if (!btn) return { ok: false, why: '语义树里没有「设置」节点', sem, nodes };
  await page.click(btn.x + btn.w / 2, btn.y + btn.h / 2);
  await sleep(1500);
  nodes = await page.semantics();
  return { ok: true, btn, sem, nodes };
}

async function scenarioSettings(browser) {
  const page = await Page.create(browser);
  try {
    await page.loadApp({ clear: true, settleMs: 8000 });
    const opened = await openSettingsPanel(page);
    save('settings/open.json', { opened });
    rec('7a', '语义树里存在「设置」入口（AppBar 文字按钮）', opened.btn ? 'pass' : 'fail',
      opened.btn ? JSON.stringify(opened.btn) : opened.why + '（节点 ' + (opened.nodes || []).length + ' 个）', 'high',
      'node scripts/browser_probe.mjs settings');
    if (!opened.ok) return;
    await page.shot('settings/settings-open.png');
    const nodes = opened.nodes;
    const has = (s) => nodes.some((n) => T(n).includes(s));
    const marks = ['配色', '外观', '舞台与口型', '背景库', '设置'].filter(has);
    save('settings/nodes.json', { count: nodes.length, labels: nodes.map((n) => T(n)), nodes });
    rec('7b', '点「设置」后设置内容真的渲染出来（不是空壳）', marks.length >= 3 ? 'pass' : 'fail',
      '命中分区/字段 ' + JSON.stringify(marks) + '；语义节点 ' + nodes.length + ' 个；样例 ' +
      JSON.stringify(nodes.slice(0, 10).map((n) => T(n))), 'high');
    // 交互性实证：点一个真实控件并观察**落盘副作用**
    // （默认 DisplayPrefs 从不落盘，所以点之前 localStorage 是 null —— 这本身也是证据）
    const byArea = (a, b) => a.w * a.h - b.w * b.h;
    const before = await page.prefsNow();
    const whiteNode = nodes.filter((n) => T(n).startsWith('白，')).sort(byArea)[0];
    let interacted = null;
    if (whiteNode) {
      await page.click(whiteNode.x + whiteNode.w / 2, whiteNode.y + whiteNode.h / 2);
      await sleep(900);
      const after = await page.prefsNow();
      interacted = { kind: 'theme-card', before: before && before.theme, after: after && after.theme, node: whiteNode };
      await page.shot('settings/after-theme-click.png');
      rec('7c', '设置面板真的可交互（点「白」主题卡 → localStorage 的 theme 变化）',
        after && after.theme === 'white' ? 'pass' : 'fail',
        'theme：点击前=' + (before ? before.theme : 'null（无落盘）') + ' → 点击后=' + (after && after.theme) +
        '；节点=' + JSON.stringify(whiteNode), 'high', 'node scripts/browser_probe.mjs settings');
    } else {
      rec('7c', '设置面板真的可交互（点「白」主题卡 → localStorage 的 theme 变化）', 'blocked',
        '语义树里找不到以「白，」开头的主题卡节点（共 ' + nodes.length + ' 个节点）', 'low');
    }
    // 附加：描边强度滑条（语义节点是 48x48 的滑块本体，48 宽，选择器命中它的 aria-label）
    // 滑条本体是带 aria-label='描边强度' 的 48x48 节点；文本节点是它左边的标签
    const strokeNode = nodes.filter((n) => n.label === '描边强度').sort(byArea)[0] || nodes.filter((n) => T(n) === '描边强度').sort(byArea)[0];
    let slider = null;
    if (strokeNode) {
      const beforeEdge = ((await page.prefsNow()) || {}).edgeStrength;
      await page.drag(strokeNode.x + strokeNode.w / 2, strokeNode.y + strokeNode.h / 2, strokeNode.x + strokeNode.w / 2 + 160, strokeNode.y + strokeNode.h / 2);
      const afterEdge = ((await page.prefsNow()) || {}).edgeStrength;
      slider = { node: strokeNode, beforeEdge, afterEdge };
      rec('7d', '设置面板滑条可拖动（拖「描边强度」→ edgeStrength 变化）',
        afterEdge !== undefined && afterEdge !== beforeEdge ? 'pass' : 'manual-only',
        'edgeStrength ' + beforeEdge + ' → ' + afterEdge + '（节点 ' + JSON.stringify(strokeNode) + '；拖 +160px）',
        afterEdge !== beforeEdge ? 'medium' : 'low', 'node scripts/browser_probe.mjs settings');
    } else {
      rec('7d', '设置面板滑条可拖动（拖「描边强度」→ edgeStrength 变化）', 'blocked', '找不到 aria-label=描边强度 的滑条节点', 'low');
    }
    save('settings/interaction.json', { interacted, slider });
  } finally { await page.close(); }
}

async function scenarioThemes(browser) {
  // (a) 真 UI 点击切四套主题
  const page = await Page.create(browser);
  const uiSwitch = {};
  try {
    await page.loadApp({ clear: true, settleMs: 8000 });
    const opened = await openSettingsPanel(page);
    if (!opened.ok) {
      rec('6a', '四套配色经真实 UI 点击切换', 'blocked', opened.why, 'low');
    } else {
      for (const entry of THEME_UI) {
        const wire = entry[0], prefix = entry[1];
        const n = (await page.semantics()).find((x) => T(x).startsWith(prefix));
        if (!n) { uiSwitch[wire] = 'node-not-found'; continue; }
        await page.click(n.x + n.w / 2, n.y + n.h / 2);
        const prefs = await page.prefsNow();
        // 没落盘 = 还是默认主题（黑）：点「黑」卡是 no-op，不该写成 null
        uiSwitch[wire] = prefs ? prefs.theme : 'black';
        await page.shot('themes/panel-' + wire + '.png');
      }
      const allOk = THEME_UI.every((e) => uiSwitch[e[0]] === e[0]);
      rec('6a', '四套配色经真实 UI 点击切换（读回 localStorage）', allOk ? 'pass' : 'fail',
        JSON.stringify(uiSwitch), 'high', 'node scripts/browser_probe.mjs themes');
    }
  } finally { await page.close(); }
  // (b) 三级面：设置面板里 12 条面带在渲染像素上真的出现，级差用**实测色**算
  try {
    const shot = join(OUT, 'themes/panel-white.png');
    const st = py([shot, '--colors', ALL_BANDS.join(','), '--tol', '2']);
    const byHex = {};
    for (const c of st.colors) byHex[c.hex] = c;
    const missing = ALL_BANDS.filter((h) => byHex[h].hits < 50);
    const ladder = {};
    for (const w of Object.keys(PALETTE)) {
      const obs = PALETTE[w].bands.map((h) => (byHex[h].observed || h));
      const deltas = [perceived(obs[1]) - perceived(obs[0]), perceived(obs[2]) - perceived(obs[1])];
      ladder[w] = { observed: obs, deltas: deltas.map((d) => Math.round(d * 10000) / 10000) };
    }
    save('themes/ladder.json', { bands: st.colors, missing, ladder });
    const minDelta = Math.min(...Object.values(ladder).flatMap((v) => v.deltas.map(Math.abs)));
    rec('6b', '三级面在真实渲染像素里出现且级差 ≥0.03（用实测色算）',
      missing.length === 0 && minDelta >= 0.03 ? 'pass' : 'fail',
      '12 条面带缺失 ' + missing.length + ' 个（阈值 ≥50 像素命中）；最小级差=' + Math.round(minDelta * 10000) / 10000 +
      '；逐套实测 observed=' + JSON.stringify(ladder), 'high',
      'python3 docs/verification/evidence-2026-10-05/png_stats.py themes/panel-white.png --colors ' + ALL_BANDS.join(',') + ' --tol 2');
  } catch (e) {
    rec('6b', '三级面在真实渲染像素里出现且级差 ≥0.03（用实测色算）', 'blocked', String(e.message), 'low');
  }
  // (c) 舞台底色随主题变（渲染面帧缓冲的实色）
  const page2 = await Page.create(browser);
  try {
    const rows = [];
    await page2.loadApp({ clear: true, settleMs: 8000 });
    for (const wire of Object.keys(PALETTE)) {
      await page2.setPrefs({ theme: wire });
      const rect = await page2.stageRect();
      const full = await page2.shot('themes/stage-' + wire + '.png');
      let st = null;
      if (rect && rect.w > 50) {
        const crop = [rect.x, rect.y, rect.x + rect.w, rect.y + rect.h].map((v) => Math.round(v)).join(',');
        st = py([full, '--crop', crop, '--colors', PALETTE[wire].stage]);
      } else {
        st = py([full, '--colors', PALETTE[wire].stage]);
      }
      rows.push({ wire, stageRect: rect, expect: PALETTE[wire].stage, top: st.dominant[0], match: st.colors[0] });
    }
    save('themes/stage-colors.json', { rows });
    // 舞台那一块的截图像素在本环境恒为白（见 stage 场景 3d 的当场证明），
    // 所以这条只能给数据、不能给判定；协议级判定在 stagecolor 场景（6c）。
    const ok = rows.every((r) => r.match.hits > 1000 && r.top.hex === r.expect);
    rec('6c-pixel', '四套主题的舞台底色在截图像素上是否等于该套 stage 色', 'manual-only',
      '逐套：' + rows.map((r) => r.wire + ' 期望#' + r.expect + ' 截图主导#' + r.top.hex).join(' | ') +
      ' ⇒ 舞台区截图恒为纯白（无头 WebGPU canvas 不进截图），像素判定不可用；等价协议证据见 6c（sync.stageColor）',
      'high', 'node scripts/browser_probe.mjs themes（原始像素见 themes/stage-colors.json）');
  } finally { await page2.close(); }
}

async function scenarioBg(browser) {
  const page = await Page.create(browser);
  const base = { backgroundSource: 0, backgroundEnabled: true, backgroundOpacity: 1.0, backgroundBlur: 0.0, backgroundScrim: 1, imageAlign: 4, slideInterval: 0, slideRandom: false };
  try {
    // ── 判据阈值（唯一真源；数字与来由都在这里，不要散落到 evidence 字符串里）──
    //
    // REDNESS_ON / REDNESS_OFF 量的都是**壳区**（SHELL_CROP）的 redness：
    //   redness = 逐像素 (R - (G+B)/2) 的均值（png_stats.py 的 channels/redness）。
    // 取值依据（2026-10-06 实测，原始数字见 bg/visibility.json）：
    //   · 无图基线（黑主题，clear 后默认）：redness = -4.33
    //   · 背景库一张纯红 64x64 图 cover 铺满壳区：redness = 50.45
    //   · 纯蓝图（轮播第 2 张）：redness = -30.31
    //   · 同一页面重复采样是**逐像素相同**的（bg/carousel-0 与 -2 的 bytes 都是 52243）
    //     ⇒ 该量的噪声远小于 1 个点。
    // REDNESS_ON = 20 的来由：= 实测红信号的 40%（20 / 50.45），比基线高 24.3 个点。
    //   留这么大余量是因为壳区还有圆角/抗锯齿/半透明面板在稀释红色；反过来说，
    //   图只铺到壳区**一半**时 redness ≈ (50.45 - (-4.33))/2 + (-4.33) = 23.1，仍 > 20
    //   ⇒ 阈值不会把「部分铺到」误判成没上屏。
    // REDNESS_OFF = 8 的来由：清图后必须回到基线附近，取「基线 + 12 个点」为上限；
    //   实测清图后 = -4.33（与基线一毫不差）。
    const REDNESS_ON = 20;
    const REDNESS_OFF = 8;
    // 两张不同 fit 的**整页帧**至少要有 0.5% 的像素不同才算「可区分」：
    // 0.5% × 1440×900 = 6480 px，远高于 PNG 编解码噪声（同图重拍 diff = 0，实测），
    // 又远低于四档铺法实际差出来的比例（stretch/tile 的差别在整幅图上是一半以上）。
    const DIFF_MIN = 0.005;
    const pending = (list) => list.filter((x) => x.blank).map((x) => x.path.replace(OUT + '/', '') + '=' + x.bytes + 'B');
    // waitForPaint() 的**原始依据**逐步留档（每步的采样次数 / 字节数 / why）：
    // 它返回 null（40s 全窗口都判成 100% 纯白平帧）与「某张截图是 blankWhite」用的是
    // **同一个函数**，所以两条判据同源；这里把依据打出来，便于事后复核为什么判 blocked。
    const paints = {};
    const paintFull = {};
    const paintBrief = (k) => {
      const p = paintFull[k];
      if (!p) return 'n/a';
      return (p.paintedAfterMs === null ? 'null（判不出）' : p.paintedAfterMs + 'ms') + ' / ' + (p.samples ? p.samples.length : 0) + ' 次采样' +
        (p.why ? ' / why=' + p.why : '');
    };

    // 00 基线（无图）
    await page.loadApp({ clear: true, settleMs: 7000 });
    const none = await shotStats(page, 'bg/00-none.png', SHELL_CROP);
    paints.none = page.lastPaint && page.lastPaint.paintedAfterMs;
    paintFull.none = page.lastPaint;
    // 01 背景库一张红图 → 可见。**字节写进真字节库**（IndexedDB，产品 schema），
    //    偏好里只留 {kind:'image', id} —— 与产品「导入图片」之后落盘的样子一致。
    await page.setPrefs({ ...base, backgrounds: [{ kind: 'image', dataUrl: IMG.red }], imageFit: 0 });
    const red = await shotStats(page, 'bg/01-red-cover.png', SHELL_CROP);
    paints.red = page.lastPaint && page.lastPaint.paintedAfterMs;
    paintFull.red = page.lastPaint;
    const prefsAfterSet = await page.prefsNow();
    const idbKeysAfterSet = await page.idbKeys().catch((e) => ['ERR:' + e.message]);
    // 02 刷新后仍在（水合从字节库补字节；读不回来就会看不到图）
    await page.setPrefs({});
    const red2 = await shotStats(page, 'bg/02-red-persist.png', SHELL_CROP);
    paints.red2 = page.lastPaint && page.lastPaint.paintedAfterMs;
    paintFull.red2 = page.lastPaint;
    const prefsAfter = await page.prefsNow();
    const idbKeysAfterReload = await page.idbKeys().catch((e) => ['ERR:' + e.message]);
    // 03 清图
    await page.setPrefs({ backgrounds: [] });
    const cleared = await shotStats(page, 'bg/03-cleared.png', SHELL_CROP);
    paints.cleared = page.lastPaint && page.lastPaint.paintedAfterMs;
    paintFull.cleared = page.lastPaint;
    const idbKeysAfterClear = await page.idbKeys().catch((e) => ['ERR:' + e.message]);
    save('bg/visibility.json', {
      none, red, red2, cleared,
      injectedId: backgroundIdOf(IMG.red),
      prefsAfterSet, prefsAfter, idbKeysAfterSet, idbKeysAfterReload, idbKeysAfterClear,
      paints, paintFull,
      note: '背景字节住在 IndexedDB（db live2d-ai / store backgrounds / key=id），localStorage 只留 id',
    });
    const b5a = pending([none, red]);
    const b5b = pending([red2]);
    const b5c = pending([cleared]);
    rec('5a', '背景库选图 → 图真的上屏（红度像素级）',
      b5a.length ? 'blocked' : (red.stats.redness > REDNESS_ON ? 'pass' : 'fail'),
      (b5a.length ? '【环境不可判读】空白帧：' + b5a.join('、') + '；' : '') +
      '壳区 redness：无图 ' + none.stats.redness + ' → 有图 ' + red.stats.redness + '（阈值 >' + REDNESS_ON + '）；通道均值 ' +
      JSON.stringify(red.stats.channels) + '；裁剪=' + SHELL_CROP +
      '；偏好 backgrounds=' + JSON.stringify((prefsAfterSet || {}).backgrounds) +
      '；IndexedDB 键=' + JSON.stringify(idbKeysAfterSet) +
      '；waitForPaint（带原始依据）none=' + paintBrief('none') + '；red=' + paintBrief('red'),
      b5a.length ? 'low' : 'high', 'node scripts/browser_probe.mjs bg（原始像素见 bg/visibility.json）');
    rec('5b', '刷新后背景仍在（真字节库水合，不是靠 localStorage 里的 dataUrl）',
      b5b.length ? 'blocked' : (red2.stats.redness > REDNESS_ON ? 'pass' : 'fail'),
      (b5b.length ? '【环境不可判读】空白帧：' + b5b.join('、') + '；' : '') +
      '刷新后 redness=' + red2.stats.redness + '（阈值 >' + REDNESS_ON + '）；偏好里 backgrounds=' +
      JSON.stringify((prefsAfter || {}).backgrounds) + '；IndexedDB 键（与 redness 同一时刻）= ' + JSON.stringify(idbKeysAfterReload) +
      '；清图后 IndexedDB 键=' + JSON.stringify(idbKeysAfterClear) +
      '；waitForPaint red2=' + paintBrief('red2'),
      b5b.length ? 'low' : 'high');
    rec('5c', '清图后图真的消失（回到底色）',
      b5c.length ? 'blocked' : (cleared.stats.redness < REDNESS_OFF ? 'pass' : 'fail'),
      (b5c.length ? '【环境不可判读】空白帧：' + b5c.join('、') + '；' : '') +
      '清图后 redness=' + cleared.stats.redness + '（阈值 <' + REDNESS_OFF + '；基线 ' + none.stats.redness + '）' +
      '；waitForPaint cleared=' + paintBrief('cleared'),
      b5c.length ? 'low' : 'high');
    // 04 四档 fit：同一张非方形图，四张**整页**截图必须互不相同
    const fitShots = {};
    for (const fit of [0, 1, 2, 3]) {
      const name = ['cover', 'contain', 'stretch', 'tile'][fit];
      await page.setPrefs({ ...base, backgrounds: [{ kind: 'image', dataUrl: IMG.tall }], imageFit: fit, tileSize: 64 });
      fitShots[name] = await shotStats(page, 'bg/fit-' + name + '.png', SHELL_CROP);
    }
    const diffs = {};
    for (const a of ['cover', 'contain', 'stretch']) {
      for (const b of ['contain', 'stretch', 'tile']) {
        if (a >= b) continue;
        diffs[a + '_vs_' + b] = py([fitShots[a].path, '--diff', fitShots[b].path]).diff.fraction;
      }
    }
    save('bg/fits.json', {
      shots: Object.fromEntries(Object.entries(fitShots).map(([k, v]) => [k, { path: v.path, bytes: v.bytes, dominant: v.stats.dominant.slice(0, 2) }])),
      diffs, shellRedness: Object.fromEntries(Object.entries(fitShots).map(([k, v]) => [k, v.stats.redness])),
    });
    const allDiff = Object.values(diffs).every((d) => d > DIFF_MIN);
    const b5d = pending(Object.values(fitShots));
    rec('5d', '四档 fit 像素级可区分（cover/contain/stretch/tile）',
      b5d.length ? 'blocked' : (allDiff ? 'pass' : 'fail'),
      (b5d.length ? '【环境不可判读】空白帧：' + b5d.join('、') + '；' : '') +
      '整页逐像素差异（>=' + DIFF_MIN + ' 判可区分）：' + JSON.stringify(diffs), b5d.length ? 'low' : 'high',
      'python3 …/png_stats.py bg/fit-cover.png --diff bg/fit-contain.png');
    // 05 壳背景轮播：slideInterval=5 + 两张图，采样 4 次
    await page.setPrefs({ ...base, backgrounds: [{ kind: 'image', dataUrl: IMG.red }, { kind: 'image', dataUrl: IMG.blue }], slideInterval: 5 });
    const samples = [];
    for (let i = 0; i < 4; i++) {
      const s = await shotStats(page, 'bg/carousel-' + i + '.png', SHELL_CROP);
      samples.push({ i, redness: s.stats.redness, channels: s.stats.channels, bytes: s.bytes, blank: s.blank });
      await sleep(4000);
    }
    save('bg/carousel.json', { samples });
    const sawRed = samples.some((s) => s.redness > REDNESS_ON), sawBlue = samples.some((s) => s.redness < -REDNESS_OFF);
    const b5e = samples.filter((s) => s.blank).map((s) => 'carousel-' + s.i + '=' + s.bytes + 'B');
    rec('5e', '壳背景轮播真的在换（间隔 5s，采样 4 次）',
      b5e.length ? 'blocked' : (sawRed && sawBlue ? 'pass' : 'fail'),
      (b5e.length ? '【环境不可判读】空白帧：' + b5e.join('、') + '；' : '') + JSON.stringify(samples),
      b5e.length ? 'low' : 'high', 'node scripts/browser_probe.mjs bg');
    // 06 预览 + 拖动排序（UI 路径）
    await page.setPrefs({ ...base, backgrounds: [{ kind: 'image', dataUrl: IMG.red }, { kind: 'image', dataUrl: IMG.blue }], slideInterval: 0, imageFit: 0 });
    const before = await page.prefsNow();
    const opened = await openSettingsPanel(page);
    if (!opened.ok) {
      rec('5f', '点缩略图预览（当前标记移动）', 'blocked', opened.why, 'low');
      rec('5g', '拖动排序背景库', 'blocked', opened.why, 'low');
      return;
    }
    const beforeNodes = await page.semantics();
    const cur0 = beforeNodes.find((n) => T(n).includes('当前'));
    const item2 = beforeNodes.find((n) => T(n).startsWith('图片 2'));
    let cur1 = null, clickErr = null;
    if (item2) {
      await page.click(item2.x + Math.min(60, item2.w / 2), item2.y + item2.h / 2);
      await sleep(800);
      cur1 = (await page.semantics()).find((n) => T(n).includes('当前'));
    } else clickErr = '语义树找不到「图片 2」节点';
    await page.shot('settings/library-after-preview.png');
    rec('5f', '点缩略图预览：当前标记移到该张（可达 + 生效）',
      cur0 && cur1 && T(cur1) !== T(cur0) && T(cur1).startsWith('图片 2') ? 'pass' : 'fail',
      '点前「当前」=' + (cur0 && T(cur0)) + '；点后=' + (cur1 && T(cur1)) + (clickErr ? '；' + clickErr : ''), 'high');
    // 拖动排序：从第 1 行拖到第 2 行下方，读回偏好顺序（顺序 = 图 id 序列）
    const rowNodes = (await page.semantics()).filter((n) => /^图片 [12]/.test(T(n)));
    let reorder = null;
    if (rowNodes.length >= 2) {
      const r1 = rowNodes.find((n) => T(n).startsWith('图片 1'));
      const r2 = rowNodes.find((n) => T(n).startsWith('图片 2'));
      const idsBefore = (before.backgrounds || []).map((b) => b.id);
      await page.drag(r1.x + 12, r1.y + r1.h / 2, r2.x + 12, r2.y + r2.h + 10);
      await sleep(1200);
      const after = await page.prefsNow();
      reorder = { idsBefore, idsAfter: (after.backgrounds || []).map((b) => b.id), labels: rowNodes.map((n) => T(n)) };
      save('settings/reorder.json', reorder);
      rec('5g', '拖动排序背景库（CDP 指针输入）',
        reorder.idsAfter.length === 2 && reorder.idsBefore[0] === reorder.idsAfter[1] ? 'pass' : 'manual-only',
        JSON.stringify(reorder) + '；不成则需人工（ReorderableListView 的拖动手柄命中点未知，本脚本用行左侧 12px 起拖）',
        reorder.idsBefore[0] === reorder.idsAfter[1] ? 'medium' : 'low');
    } else {
      rec('5g', '拖动排序背景库（CDP 指针输入）', 'blocked', '语义树里找不到两行背景项', 'low');
    }
  } finally { await page.close(); }
}

// ───────────────────────────── 后端端点面（不依赖 Flutter 产物） ─────────────────────────────
function curlGet(url, extra = []) {
  const bodyFile = '/tmp/browser_probe_body';
  const out = execFileSync('curl', ['-sS', '-o', bodyFile, '-w', '%{http_code} %{redirect_url} %{size_download}', ...extra, url], { encoding: 'utf8' });
  let body = '';
  try { body = readFileSync(bodyFile, 'utf8'); } catch { /* 无 body */ }
  const p = out.trim().split(' ');
  return { url, status: parseInt(p[0], 10), redirect: p[1] || '', bytes: parseInt(p[2] || '0', 10), body };
}

async function scenarioApi() {
  const rows = {};
  rows.root = curlGet(BASE + '/');
  rows.app = curlGet(BASE + '/app/');
  rows.index = curlGet(BASE + '/app/index.html');
  rows.mainJs = curlGet(BASE + '/app/main.dart.js');
  rows.bootstrap = curlGet(BASE + '/app/flutter_bootstrap.js');
  rows.status = curlGet(BASE + '/api/v1/app/status');
  rows.capabilities = curlGet(BASE + '/api/v1/app/capabilities');
  rows.models = curlGet(BASE + '/api/v1/models');
  rows.settings = curlGet(BASE + '/api/v1/settings');
  rows.mods = curlGet(BASE + '/api/v1/mods');
  rows.env = curlGet(BASE + '/api/v1/env');
  const manifest = curlGet(BASE + '/models/bai/runtime/bai.model3.json');
  const refs = [];
  if (manifest.status === 200) {
    try {
      const j = JSON.parse(manifest.body);
      const walk = (v) => {
        if (typeof v === 'string') { if (/\.(moc3|png|jpe?g|webp|json|physics3|pose3|motion3|exp3|cdi3)$/i.test(v)) refs.push(v); }
        else if (Array.isArray(v)) v.forEach(walk);
        else if (v && typeof v === 'object') Object.values(v).forEach(walk);
      };
      walk(j.FileReferences || j);
    } catch { /* 非 JSON */ }
  }
  const dir = '/models/bai/runtime/';
  const refRows = [...new Set(refs)].slice(0, 60).map((r) => {
    const u = r.startsWith('/') ? BASE + r : BASE + dir + r.replace(/^\.\//, '');
    return curlGet(u);
  });
  const render = curlGet(BASE + '/render');
  const assets = [...new Set([...render.body.matchAll(/["'](\/render\/[^"']+)["']/g)].map((m) => m[1]))];
  const assetRows = assets.map((a) => curlGet(BASE + a));
  const keyLeak = /sk-[A-Za-z0-9]{8,}/.test(rows.settings.body + rows.env.body);
  save('api.json', {
    endpoints: Object.fromEntries(Object.entries(rows).map(([k, v]) => [k, { status: v.status, redirect: v.redirect, bytes: v.bytes }])),
    envBody: rows.env.body, modelRefs: refRows.map((r) => ({ url: r.url, status: r.status, bytes: r.bytes })),
    renderAssets: assetRows.map((r) => ({ url: r.url, status: r.status, bytes: r.bytes })),
  });
  rec('api-a', '托管层：/ 302→/app/，/app/ 与三个入口文件 200',
    rows.root.status === 302 && rows.root.redirect.includes('/app/') && rows.app.status === 200 &&
    rows.index.status === 200 && rows.mainJs.status === 200 && rows.mainJs.bytes > 1000000 && rows.bootstrap.status === 200 ? 'pass' : 'fail',
    'root=' + rows.root.status + '→' + rows.root.redirect + '；app=' + rows.app.status + '；index=' + rows.index.status +
    '；main.dart.js=' + rows.mainJs.status + ' ' + rows.mainJs.bytes + 'B；bootstrap=' + rows.bootstrap.status, 'high',
    'node scripts/browser_probe.mjs api');
  rec('api-b', 'API 面 200 且不泄密钥（/api/v1/settings、/api/v1/env）',
    rows.status.status === 200 && rows.settings.status === 200 && rows.mods.status === 200 && rows.env.status === 200 && !keyLeak ? 'pass' : 'fail',
    'app/status=' + rows.status.status + ' capabilities=' + rows.capabilities.status + ' models=' + rows.models.status + ' settings=' + rows.settings.status + ' mods=' + rows.mods.status + ' env=' + rows.env.status +
    '；env body=' + rows.env.body.slice(0, 120) + '；密钥样式泄漏=' + keyLeak, 'high');
  const refBad = refRows.filter((r) => r.status !== 200);
  rec('api-c', '模型 manifest 及它引用的每个资源可达（200）',
    manifest.status === 200 && refRows.length > 0 && refBad.length === 0 ? 'pass' : 'fail',
    'manifest=' + manifest.status + ' ' + manifest.bytes + 'B；引用 ' + refRows.length + ' 个，非 200 ' + refBad.length +
    (refBad.length ? '：' + JSON.stringify(refBad.slice(0, 5).map((r) => r.url + '=' + r.status)) : ''), 'high');
  const assetBad = assetRows.filter((r) => r.status !== 200);
  rec('api-d', '/render 渲染面资源可达（html + js + wasm）',
    render.status === 200 && assetRows.length > 0 && assetBad.length === 0 ? 'pass' : 'fail',
    'render=' + render.status + '；资源 ' + assetRows.length + ' 个：' + JSON.stringify(assetRows.map((r) => r.url.split('/').pop() + '=' + r.status)), 'high');
}

async function scenarioRender(browser) {
  const page = await Page.create(browser);
  try {
    page.recording = true;
    await page.loadApp({ url: BASE + '/render?hud=1', settleMs: 6000 });
    const hudOk = await page.waitFor("(function () { var e = document.getElementById('status'); return !!e && /GPU:/.test(e.textContent || ''); })()", 45000, 'HUD GPU 行').catch(() => false);
    const h1 = await page.evaluate("document.getElementById('status').textContent");
    await sleep(3000);
    const h2 = await page.evaluate("document.getElementById('status').textContent");
    const shot = await page.shot('stage/render-direct.png');
    const models = page.net.responses.filter((r) => /\/models\//.test(r.url)).map((r) => r.status + ' ' + r.url);
    save('render.json', { hudOk, hud1: h1, hud2: h2, modelResponses: models, requests: page.net.requests, failures: page.net.failures });
    rec('render-a', '不经过 Flutter 壳，直接打开 /render?hud=1 能跑起来',
      hudOk ? 'pass' : 'fail', 'HUD1=' + String(h1).slice(0, 160) + ' || HUD2=' + String(h2).slice(0, 160), 'high',
      'node scripts/browser_probe.mjs render');
    rec('render-b', '渲染面自己加载模型（/render 直接跑时的 /models/ 请求）',
      models.length > 0 && models.every((s) => s.startsWith('2')) ? 'pass' : 'fail', models.slice(0, 6).join(' | ') || '无', 'high');
  } finally { page.recording = false; await page.close(); }
}

// ───────────── 运行期字体兜底（Lead 指定的最高优先级实测） ─────────────
/** 子集外字符：故意注入，用来探「缺字时会不会真的去 fonts.gstatic.com」。 */
const OUT_OF_SUBSET = ['\u{20BB7}', '\u{1F004}', '\u{1D11E}', '\u{266A}', '\u{20AC}', '\u{0416}'];

async function scenarioFonts(browser) {
  const page = await Page.create(browser);
  try {
    page.recording = true;
    await page.loadApp({ clear: true, settleMs: 8000 });
    const steps = [];
    const gs = () => page.net.requests.filter((r) => /gstatic|googleapis/i.test(r.url)).map((r) => r.url);
    const mark = (label) => steps.push({ label, gstatic: gs(), reqTotal: page.net.requests.length });
    mark('01 首屏加载');

    // ① 主题四张卡（真 UI 点击，每张卡都画它自己那套三级面 + 中文标签）
    const opened = await openSettingsPanel(page);
    mark('02 设置面板打开' + (opened.ok ? '' : '（失败：' + opened.why + '）'));
    if (opened.ok) {
      for (const e of THEME_UI) {
        const n = await page.clickLabel(e[1]);
        await sleep(700);
        mark('03 主题卡 ' + e[0] + (n ? '' : '（节点缺失）'));
      }
      // ② 逐个分区（每个分区的中文标签、字段名、说明文字都过一遍）
      for (const s of ['人设', '模型库', 'LLM', '语音合成', '外观与互动', 'Mod', '诊断', '开发模式']) {
        const n = await page.clickLabel(s);
        await sleep(900);
        mark('04 分区 ' + s + (n ? '' : '（节点缺失）'));
      }
      // ③ 背景库 + 图案中文标签（外观与互动分区内）
      await page.clickLabel('外观与互动');
      await sleep(600);
      for (const p of ['渐变', '光晕', '网格', '斜纹']) {
        const n = await page.clickLabel(p);
        await sleep(400);
        mark('05 图案标签 ' + p + (n ? '' : '（节点缺失）'));
      }
      await sleep(600);
      await page.shot('fonts/after-settings-walk.png');
    }
    // ④ 会话列表（浮层）
    const savedNodes = await page.semantics();
    const closeBtn = savedNodes.find((n) => /关闭|完成|收起/.test(T(n)));
    if (closeBtn) await page.click(closeBtn.x + closeBtn.w / 2, closeBtn.y + closeBtn.h / 2);
    else await page.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 }).catch(() => {});
    await sleep(800);
    const sess = await page.clickLabel('会话');
    await sleep(1000);
    mark('06 会话列表' + (sess ? '' : '（节点缺失）'));
    await page.shot('fonts/after-sessions.png');
    await page.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 }).catch(() => {});
    await sleep(800);

    // ⑤ 聊天输入框：先打产品内的中文，再打**子集外字符**
    // 注意：语义树里「说点什么吧」是**空态提示文案**（在 370px 高的位置），
    // 不是输入框。真正的输入框是 flt-text-editing-host shadow root 里的 textarea
    // （本轮实测：按文案匹配会点到空态提示上，打进去的字一个都没落地）。
    const input = await page.chatInput();
    const typed = {};
    if (input) {
      await page.click(input.x + Math.min(80, input.w / 2), input.y + input.h / 2);
      await page.send('Input.insertText', { text: '中文界面文案测试：配色、舞台、口型、背景库、诊断' });
      await sleep(1200);
      typed.productDom = await page.typedText();
      mark('07 输入产品中文文案（DOM 实测=' + JSON.stringify(typed.productDom) + '）');
      await page.send('Input.insertText', { text: OUT_OF_SUBSET.join('') });
      await sleep(2500);
      typed.exoticDom = await page.typedText();
      mark('08 输入子集外字符 ' + OUT_OF_SUBSET.join(' ') + '（DOM 实测=' + JSON.stringify(typed.exoticDom) + '）');
      await page.shot('fonts/after-exotic-input.png');
    } else {
      mark('07/08 找不到聊天输入框（节点缺失）');
    }

    // ⑥ 阳性对照：证明「探测 gstatic 的能力」本身没瞎（故意发一条请求）
    await page.evaluate("fetch('https://fonts.gstatic.com/s/notosanssc/v1/__probe__.woff2').catch(function () { return null; })");
    await sleep(1500);
    mark('99 阳性对照：脚本故意请求 fonts.gstatic.com');

    // 逐字符归因：每个字符一个**全新页面**（字体一旦加载就会被引擎缓存，
    // 同一页里后面的字符不会再发请求 ⇒ 必须一字符一页才归因得准）。
    const perChar = [];
    for (const ch of OUT_OF_SUBSET) {
      const p = await Page.create(browser);
      try {
        p.recording = true;
        await p.loadApp({ settleMs: 7000 });
        // 输入宿主是**惰性创建**的：全新页面里 flt-text-editing-host 还不存在，
        // 先点一下聊天输入区（右下角）让 Flutter 建出它，再等。
        let inp = await p.waitForChatInput(5000);
        if (!inp) {
          await p.click(VW - 160, VH - 30);
          inp = await p.waitForChatInput(15000);
        }
        if (!inp) { perChar.push({ ch, error: '找不到 textarea' }); continue; }
        await p.click(inp.x + 40, inp.y + inp.h / 2);
        const before = p.net.requests.length;
        await p.send('Input.insertText', { text: ch });
        await sleep(2500);
        perChar.push({
          ch: 'U+' + ch.codePointAt(0).toString(16).toUpperCase(),
          char: ch,
          dom: await p.typedText(),
          gstatic: p.net.requests.slice(before).map((r) => r.url).filter((u) => /gstatic|googleapis/i.test(u)),
        });
      } catch (err) {
        perChar.push({ ch, error: err.message });
      } finally { p.recording = false; await p.close(); }
    }
    save('fonts.json', { steps, typed, perChar, outOfSubset: OUT_OF_SUBSET, requests: page.net.requests, failures: page.net.failures });
    const triggered = perChar.filter((c) => c.gstatic && c.gstatic.length > 0);
    const perCharErrors = perChar.filter((c) => c.error);
    rec('F-fonts-03', '逐字符归因：哪些子集外字符会真的去 Google 拉字体',
      perCharErrors.length > 0 ? 'blocked' : (triggered.length === 0 ? 'pass' : 'fail'),
      perChar.map((c) => c.ch + (c.error ? '(错误:' + c.error + ')' : '→' + (c.gstatic.length ? c.gstatic.map((u) => u.split('/s/')[1].split('/')[0]).join(',') : '无请求'))).join(' | '),
      perCharErrors.length > 0 ? 'low' : 'high',
      'node scripts/browser_probe.mjs fonts（逐字符原始见 fonts.json.perChar）');
    // 产品文案走查 = 全部步骤**减去**「故意注入子集外字符」（08）与阳性对照（99）
    const productSteps = steps.filter((s) => !s.label.startsWith('99') && !s.label.startsWith('08'));
    const productHits = productSteps.filter((s) => s.gstatic.length > 0);
    const control = steps[steps.length - 1];
    const marks = productSteps.filter((s) => /节点缺失|失败/.test(s.label));
    rec('F-fonts-01', '完整 UI 走查（四主题 + 八分区 + 图案标签 + 会话列表 + 聊天输入的产品中文）全程 0 次 gstatic',
      productHits.length === 0 ? 'pass' : 'fail',
      '走查 ' + productSteps.length + ' 步；出现 gstatic 的步：' + (productHits.length ? JSON.stringify(productHits) : '无') +
      '；未命中节点的步 ' + marks.length + ' 个：' + JSON.stringify(marks.map((s) => s.label)) +
      '；阳性对照（脚本故意请求）gstatic 请求数=' + control.gstatic.length + ' ⇒ 探测器本身能看见 gstatic',
      productHits.length === 0 && control.gstatic.length > 0 && marks.length === 0 ? 'high' : 'medium',
      'node scripts/browser_probe.mjs fonts（原始 steps 见 fonts.json）');
    const exoticStep = steps.find((s) => s.label.startsWith('08'));
    rec('F-fonts-02', '故意注入子集外字符是否触发运行期字体兜底（fonts.gstatic.com）',
      exoticStep && exoticStep.gstatic.length > 0 ? 'fail' : 'pass',
      exoticStep ? '输入 ' + OUT_OF_SUBSET.join(' ') + ' 后 gstatic 请求=' + JSON.stringify(exoticStep.gstatic) +
        '；DOM 输入框实测=' + JSON.stringify(typed.exoticDom) : '未走到该步',
      typed.exoticDom ? 'high' : 'low',
      'node scripts/browser_probe.mjs fonts');
  } finally { page.recording = false; await page.close(); }
}

/** 协议级验证：四套主题下 Flutter 到底给渲染面发了什么 stageColor。
 *
 * 为什么不用像素：本环境的无头截图**拍不到 WebGPU canvas 的内容**——
 * 强制把 stageColor 改成 #ff0000 之后，canvas 区域仍是 100% 纯白
 * （见 stage/stage-canvas-white-evidence.json）。所以舞台底色的唯一可信
 * 证据是它下发的**协议帧**，而不是截图。 */
async function scenarioStageColor(browser) {
  const rows = [];
  for (const wire of Object.keys(PALETTE)) {
    const page = await Page.create(browser);
    try {
      await page.spyFrames();
      await page.loadApp({ clear: true, prefs: { theme: wire }, settleMs: 9000 });
      const frames = await page.frames();
      const syncs = frames.filter((f) => f && f.type === 'sync' && f.payload && f.payload.stageColor);
      rows.push({ wire, expect: '#' + PALETTE[wire].stage, sent: syncs.map((f) => f.payload.stageColor), frames: frames.length });
    } finally { await page.close(); }
  }
  save('stage-class/stagecolor.json', { rows });
  const ok = rows.every((r) => r.sent.length > 0 && r.sent.every((c) => c.toLowerCase() === r.expect));
  rec('6c', '四套主题的舞台底色确实按协议下发（sync.stageColor）', ok ? 'pass' : 'fail',
    rows.map((r) => r.wire + ': 期望 ' + r.expect + ' 实发 ' + JSON.stringify(r.sent)).join(' | '),
    'high', 'node scripts/browser_probe.mjs stagecolor（原始帧见 stage-class/stagecolor.json）');
}

// ───────────── 目标 B：真实音频链路（LLM → TTS → WS audio 帧 → <audio> 播放） ─────────────
/** 一轮对话的提示词：**只要一句话**，且以句号收尾 ——
 * 分句器按真实句读切，一句话 ⇒ 一个 sentence_seq ⇒ 边界断言最干净。 */
const AUDIO_PROMPT = '请只回一句话，以句号结尾：今天天气不错。';
/** 等音频链路的预算（毫秒）：LLM 首字 + TTS 合成（本机 CosyVoice 实测 5.4s/句）留足。 */
const AUDIO_WAIT_MS = 90000;

/** 真实音频链路验收（task-7 目标 B）。四条判据，每条都给原始数字：
 *
 *   a. WS 有 `audio` 帧，且**每句**恰好一个 `start` + 一个 `end`，
 *      `sentence_seq` 从 1 起严格递增（老缺陷正是「0 个 start、N 个 end」）；
 *   b. 非静音分支：`audio` 帧解出的样本数 > 0，且没有 `muted:true` 的帧；
 *   c. 前端真的建了媒体元素：`<audio>` 的 src 是 `blob:` 且 duration > 0；
 *   d. 真的在播：同一个元素的 currentTime 在采样窗口里前进 > 0.2s。
 *
 * 四条里任何一条拿不到**证据**（TTS 不可用 / 服务端 mute / 前端没建元素），
 * 该条一律判 **blocked** 并写清原因，**不许**写成 pass。
 * 判据真源：crates/live2d-ai-desktop/src/web_api/ws/audio.rs（帧 schema）、
 * shell/flutter/lib/audio/audio_player.dart（blob + <audio>）。 */
async function scenarioAudio(browser) {
  const page = await Page.create(browser);
  try {
    await page.spyWs();
    await page.loadApp({ clear: true, settleMs: 8000 });
    const wsLoaded = await page.wsFrames();
    // 真实 UI：点进输入框 → 打一句话 → 点「发送」（真指针事件 = 也给了页面用户激活，
    // 否则 <audio>.play() 会被 autoplay 策略拦下，那时判 blocked 而不是 fail）。
    let input = await page.waitForChatInput(8000);
    if (!input) { await page.click(VW - 160, VH - 30); input = await page.waitForChatInput(20000); }
    const typed = { input: !!input, text: null, sendNode: null, sendHow: null };
    if (input) {
      await page.click(input.x + Math.min(80, input.w / 2), input.y + input.h / 2);
      await page.send('Input.insertText', { text: AUDIO_PROMPT });
      await sleep(1200);
      typed.text = await page.typedText();
      await page.enableSemantics();
      const nodes = await page.semantics();
      const btn = nodes.filter((n) => T(n).includes('发送')).sort((a, b) => a.w * a.h - b.w * b.h)[0] || null;
      typed.sendNode = btn;
      if (btn) {
        typed.sendHow = 'click-语义节点';
        await page.click(btn.x + btn.w / 2, btn.y + btn.h / 2);
      } else {
        typed.sendHow = 'keydown-Enter';
        await page.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
      }
    }
    // 采样：直到「见过句尾 end」且「currentTime 前进过」，或超时。
    const timeline = [];
    const elSamples = [];
    const t0 = Date.now();
    let sawEnd = false, advanced = false;
    while (Date.now() - t0 < AUDIO_WAIT_MS) {
      await sleep(2000);
      const snap = await page.wsFrames();
      const fr = (snap && snap.frames) || [];
      const s = summarizeAudioFrames(fr);
      const els = await page.audioElements().catch(() => []);
      elSamples.push({ at: Date.now() - t0, els });
      timeline.push({
        at: Date.now() - t0, frames: fr.length, audioFrames: s.audioFrames,
        starts: s.totalStarts, ends: s.totalEnds, samples: s.totalSamples,
        sentences: s.sentences.map((g) => g.seq + ':' + g.starts + 'start/' + g.ends + 'end/' + g.samples + 'smp'),
        elements: els.length, elapsedSec: els.map((e) => e.currentTime),
        errs: fr.filter((f) => f.type === 'error').map((f) => f.error).slice(-2),
      });
      if (s.totalEnds > 0) sawEnd = true;
      for (let i = 0; i < els.length; i++) {
        const rows = elSamples.filter((x) => x.els[i]).map((x) => x.els[i].currentTime);
        if (rows.length > 1 && Math.max(...rows) - Math.min(...rows) > 0.2) advanced = true;
      }
      if (sawEnd && advanced) break;
    }
    const snap = await page.wsFrames();
    const frames = (snap && snap.frames) || [];
    const sum = summarizeAudioFrames(frames);
    const errs = frames.filter((f) => f.type === 'error').map((f) => f.error);
    const els = await page.audioElements().catch(() => []);
    // 逐元素（下标即身份）看 currentTime：只看到 >0 可能只是静态值，必须看**前进量**。
    const progressed = [];
    for (let i = 0; i < els.length; i++) {
      const rows = elSamples.filter((x) => x.els[i]).map((x) => ({ at: x.at, t: x.els[i].currentTime, dur: x.els[i].duration, blob: x.els[i].blob === true }));
      if (!rows.length) continue;
      progressed.push({
        index: i, blob: rows.some((r) => r.blob),
        duration: rows[rows.length - 1].dur,
        firstAt: rows[0].at, firstTime: rows[0].t, lastAt: rows[rows.length - 1].at, lastTime: rows[rows.length - 1].t,
        deltaSec: +(rows[rows.length - 1].t - rows[0].t).toFixed(3),
      });
    }
    save('audio/ws-frames.json', {
      wsUrl: snap && snap.url, socketsOpened: snap && snap.opened, framesTotal: frames.length,
      audioFrames: frames.filter((f) => f.type === 'audio'), errors: errs,
      otherTypes: sum.otherTypes, summary: sum, timeline,
    });
    save('audio/media-elements.json', { typed, wsAtLoad: wsLoaded && { opened: wsLoaded.opened, url: wsLoaded.url }, final: els, samples: elSamples, progressed });
    const ttsErrs = errs.filter((e) => e && (e.stage === 'tts' || /^tts_/.test(String(e.code))));
    const why = sum.audioFrames === 0
      ? (errs.length ? 'WS error 帧：' + JSON.stringify(errs.slice(0, 3)) : '整轮没有 audio 帧（LLM/TTS 没产出，或这一轮没发出去）')
      : null;
    // a. 句界
    const aOk = sum.audioFrames > 0 && sum.everySentenceOneStartOneEnd && sum.strictlyIncreasingFrom1;
    rec('audio-a', 'WS audio 帧有 start/end 边界且 sentence_seq 从 1 起严格递增',
      aOk ? 'pass' : (sum.audioFrames === 0 ? 'blocked' : 'fail'),
      'audio 帧 ' + sum.audioFrames + ' 条；start ' + sum.totalStarts + ' 个 / end ' + sum.totalEnds + ' 个；' +
      '句子 ' + JSON.stringify(sum.sentences.map((g) => ({ seq: g.seq, frames: g.frames, starts: g.starts, ends: g.ends, samples: g.samples }))) +
      '；sentence_seq=' + JSON.stringify(sum.seqs) + '（严格从 1 递增=' + sum.strictlyIncreasingFrom1 + '）' +
      (why ? '；原因：' + why : ''),
      sum.audioFrames === 0 ? 'low' : 'high', 'node scripts/browser_probe.mjs audio（原始帧见 audio/ws-frames.json）');
    // b. 样本数 / 静音
    rec('audio-b', '音频样本数 > 0（非静音分支）',
      sum.totalSamples > 0 && !sum.anyMuted ? 'pass' : 'blocked',
      '总样本数 ' + sum.totalSamples + '（audio 帧载荷 ' + sum.audioFrames + ' 条，逐帧 samples 见 audio/ws-frames.json）；' +
      '服务端 muted 标记=' + sum.anyMuted + (ttsErrs.length ? '；TTS 错误帧=' + JSON.stringify(ttsErrs.slice(0, 2)) : '') +
      (sum.anyMuted ? '；服务端 mute（LIVE2D_AI_MUTE_AUDIO=1）⇒ 样本被零填充，属 blocked 而不是 fail' : ''),
      sum.totalSamples > 0 ? 'high' : 'low');
    // c/d. 媒体元素 + 播放前进
    const mediaOk = progressed.some((p) => p.duration > 0 && p.deltaSec > 0.2);
    rec('audio-c', '前端建了 <audio>（src=blob:、duration>0）且 currentTime 真的前进',
      mediaOk ? 'pass' : 'blocked',
      '页面上的 <audio> 元素 ' + els.length + ' 个；逐元素：' + JSON.stringify(progressed) +
      '；末次快照=' + JSON.stringify(els.slice(0, 2)) +
      (mediaOk ? '' : '；没观察到「blob src + duration>0 + currentTime 前进 >0.2s」（无头 autoplay 策略 / 没建元素 / 还没播到）⇒ blocked'),
      mediaOk ? 'high' : 'low', 'node scripts/browser_probe.mjs audio（原始见 audio/media-elements.json）');
  } finally { await page.close(); }
}

const SCENARIOS = {
  files: scenarioFiles, api: scenarioApi, fonts: scenarioFonts, stagecolor: scenarioStageColor, net: scenarioNet, offline: scenarioOffline,
  render: scenarioRender, stage: scenarioStage, ui: scenarioUi,
  settings: scenarioSettings, themes: scenarioThemes, bg: scenarioBg,
  // audio **不进 `all`**：它要活端点（LLM + 本机 TTS）并会真的发一轮对话 ——
  // 与 AGENTS「端到端探针只在有活端点时跑，缺配置就明确跳过」是同一条纪律。
  // 要跑就显式：node scripts/browser_probe.mjs audio（或 all,audio）。
  audio: scenarioAudio,
};

async function main() {
  const which = (process.argv[2] || 'all').split(',').filter(Boolean);
  if (which.includes('summary')) {
    const ver = await (await fetch(CDP_HTTP + '/json/version')).json().catch(() => ({ Browser: 'n/a' }));
    const agg = await aggregate({ cdp: ver.Browser, head: execFileSync('git', ['-C', ROOT, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), base: BASE, cdpUrl: CDP_HTTP });
    console.log('# 汇总 ' + agg.total + ' 项，fail ' + agg.fail + ' 项，来自 ' + agg.scenarios + ' 个场景');
    return;
  }
  const list = which.includes('all')
    ? ['files', 'api', 'net', 'fonts', 'offline', 'render', 'stage', 'stagecolor', 'ui', 'settings', 'themes', 'bg']
    : which;
  if (which.includes('all') && !which.includes('audio')) console.log('# 注：audio 场景需要活端点（LLM+TTS），不在 all 里；要跑：node scripts/browser_probe.mjs audio');
  const ver = await (await fetch(CDP_HTTP + '/json/version')).json();
  const head = execFileSync('git', ['-C', ROOT, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  console.log('# CDP ' + ver.Browser + ' @ ' + CDP_HTTP + ' / ' + BASE);
  console.log('# HEAD ' + head);
  for (const name of list) {
    const fn = SCENARIOS[name];
    if (!fn) { console.log('!! 未实现场景 ' + name); continue; }
    console.log('\n## scenario ' + name);
    const browser = name === 'files' || name === 'api' ? null : await Cdp.connect(ver.webSocketDebuggerUrl);
    const start = RESULTS.length;
    try { await fn(browser); } catch (e) { rec(name, '场景 ' + name, 'blocked', '脚本异常：' + e.message, 'low'); }
    finally { if (browser) browser.close(); }
    // 每个场景单独落一份结果：这样「只重跑某个场景」不会把别的场景的证据抹掉。
    save('results-' + name + '.json', { scenario: name, at: new Date().toISOString(), results: RESULTS.slice(start) });
  }
  const agg = await aggregate({ cdp: ver.Browser, head, base: BASE, cdpUrl: CDP_HTTP });
  console.log('\n# 合计 ' + agg.total + ' 项，fail ' + agg.fail + ' 项（来自 ' + agg.scenarios + ' 个场景结果文件）→ run.json');
}

/** 把 OUT 下所有 results-*.json 汇总成 run.json（部分重跑也能拿到完整表）。 */
async function aggregate(meta = {}) {
  const files = readdirSync(OUT).filter((f) => /^results-.*\.json$/.test(f)).sort();
  const all = [];
  const scenarios = {};
  for (const f of files) {
    const d = JSON.parse(readFileSync(join(OUT, f), 'utf8'));
    scenarios[d.scenario] = { at: d.at, count: d.results.length };
    for (const r of d.results) all.push({ ...r, scenario: d.scenario, at: d.at });
  }
  save('run.json', { aggregatedAt: new Date().toISOString(), ...meta, scenarios, results: all });
  return { total: all.length, fail: all.filter((r) => r.verdict === 'fail').length, scenarios: Object.keys(scenarios).length };
}

await main();
