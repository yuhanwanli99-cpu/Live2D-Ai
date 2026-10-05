#!/usr/bin/env node
/**
 * 字体回落「结构性离线化」验证据（R4-T4）。零依赖：只用 Node 22 自带的全局
 * WebSocket + 本机已有的无头 Chrome（CDP）。写法的参照系是 scripts/browser_probe.mjs。
 *
 * 验的是什么（判据来自 docs/architecture/font-fallback-offline.md）：
 *   页面加载并注入子集外字符（𠮷 U+20BB7 / 🀄 U+1F004 / 𝄞 U+1D11E）后，
 *   引擎的字体回落**不产生任何跨源请求**，且命中的是**同源** font-fallback/**。
 *
 * 用法：
 *   node scripts/font_offline_check.mjs
 *   PROBE_CDP=http://127.0.0.1:9222 \
 *   FONT_CHECK_URL=http://127.0.0.1:18099/app/ \
 *   node scripts/font_offline_check.mjs
 *
 * 环境变量：
 *   PROBE_CDP         CDP 端点，默认 http://127.0.0.1:9222
 *   FONT_CHECK_URL    被验页面，默认 http://127.0.0.1:18080/app/
 *   FONT_CHECK_SETTLE_MS  首帧后等待毫秒数（默认 25000）
 *   FONT_CHECK_OUT    非空则把原始事件落 JSON 到该路径
 *
 * 退出码：全部断言通过 = 0；否则 = 1（**不伪造绿灯**：拿不到证据就判红）。
 *
 * 纪律：
 * - 只读页面、只注入文本，不改仓库任何源码；
 * - 阳性对照（脚本故意跨源请求一次）单独标注，**不计入断言窗口**——它证明的是
 *   「探测器本身看得见跨源请求」，否则 external=0 可能是探测器瞎了。
 */
import { writeFileSync } from 'node:fs';

const CDP_HTTP = process.env.PROBE_CDP || 'http://127.0.0.1:9222';
const CHECK_URL = process.env.FONT_CHECK_URL || 'http://127.0.0.1:18080/app/';
const OUT = process.env.FONT_CHECK_OUT || '';
const SETTLE_MS = Number(process.env.FONT_CHECK_SETTLE_MS || 25000);
const VW = 1440;
const VH = 900;

const INJECT = [
  { cp: 'U+20BB7', ch: '\u{20BB7}', note: 'CJK 扩展 B（未镜像 ⇒ 预期同源 404 → 豆腐块）' },
  { cp: 'U+1F004', ch: '\u{1F004}', note: '麻将牌红中 emoji（预期同源 notocoloremoji 200）' },
  { cp: 'U+1D11E', ch: '\u{1D11E}', note: '乐谱 G 谱号（预期同源 notomusic 200）' },
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const isLoopbackHost = (h) =>
  h === '127.0.0.1' || h === 'localhost' || h === '::1' || h === '[::1]' || h.endsWith('.localhost');
function hostOf(u) { try { return new URL(u).host; } catch { return '?'; } }
function isLoopbackUrl(u) { try { return isLoopbackHost(new URL(u).hostname); } catch { return true; } }
function isHttpUrl(u) { return /^https?:\/\//i.test(u); }
function pad(s, n) { s = String(s); return s.length >= n ? s : s + ' '.repeat(n - s.length); }
function padl(s, n) { s = String(s); return s.length >= n ? s : ' '.repeat(n - s.length) + s; }

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
    for (const h of this._handlers) if (h.method === m.method) { try { h.fn(m); } catch { /* 记录器不打断主流程 */ } }
  }
  send(method, params, sessionId) {
    if (params === undefined) params = {};
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
  close() { try { this.ws.close(); } catch { /* 已关 */ } }
}

class Page {
  constructor(browser, sessionId, targetId) {
    this.browser = browser; this.sessionId = sessionId; this.targetId = targetId;
    this.phase = 'load';
    this.requests = []; this.responses = []; this.failures = []; this.console = []; this.logs = [];
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
    browser.on('Network.requestWillBeSent', (m) => {
      page.requests.push({ phase: page.phase, url: m.params.request.url, method: m.params.request.method, type: m.params.type });
    });
    browser.on('Network.responseReceived', (m) => {
      page.responses.push({ phase: page.phase, url: m.params.response.url, status: m.params.response.status, mime: m.params.response.mimeType, fromDiskCache: !!m.params.response.fromDiskCache });
    });
    browser.on('Network.loadingFailed', (m) => {
      page.failures.push({ phase: page.phase, errorText: m.params.errorText, blockedReason: m.params.blockedReason || null, type: m.params.type });
    });
    browser.on('Runtime.consoleAPICalled', (m) => {
      page.console.push({ phase: page.phase, level: m.params.type, text: m.params.args.map((x) => x.value ?? x.description ?? x.type).join(' ').slice(0, 500) });
    });
    browser.on('Log.entryAdded', (m) => {
      page.logs.push({ phase: page.phase, level: m.params.entry.level, source: m.params.entry.source, text: m.params.entry.text.slice(0, 500) });
    });
    return page;
  }
  async evaluate(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture: true });
    if (r.exceptionDetails) throw new Error('page eval 抛错：' + r.exceptionDetails.text);
    return r.result.value;
  }
  async waitFor(expr, timeoutMs, label) {
    const t0 = Date.now();
    while (Date.now() - t0 < timeoutMs) {
      try { if (await this.evaluate(expr)) return true; } catch { /* 导航中 */ }
      await sleep(300);
    }
    throw new Error('等待超时：' + label);
  }
  async textarea() {
    return this.evaluate("(function () { var h = document.querySelector('flt-text-editing-host'); var root = h && h.shadowRoot ? h.shadowRoot : document; var el = root.querySelector('textarea'); if (!el) return null; var b = el.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; })()");
  }
  async typedText() {
    return this.evaluate("(function () { var h = document.querySelector('flt-text-editing-host'); var root = h && h.shadowRoot ? h.shadowRoot : document; var el = root.querySelector('textarea'); return el ? el.value : null; })()");
  }
  async click(x, y) {
    for (const type of ['mousePressed', 'mouseReleased']) {
      await this.send('Input.dispatchMouseEvent', { type, x: Math.round(x), y: Math.round(y), button: 'left', clickCount: 1, buttons: type === 'mousePressed' ? 1 : 0 });
      await sleep(70);
    }
  }
  async close() { await this.browser.send('Target.closeTarget', { targetId: this.targetId }).catch(() => {}); }
}

// ───────────────────────────── 主流程 ─────────────────────────────
const ver = await (await fetch(CDP_HTTP + '/json/version')).json();
const browser = await Cdp.connect(ver.webSocketDebuggerUrl);
const page = await Page.create(browser);
const problems = [];
let result = null;

try {
  console.log('# CDP ' + ver.Browser + ' @ ' + CDP_HTTP);
  console.log('# 被验页面 ' + CHECK_URL);
  console.log('# 注入字符 ' + INJECT.map((x) => x.cp).join(' / '));
  console.log('');

  page.phase = 'load';
  const loaded = new Promise((res) => {
    const h = { method: 'Page.loadEventFired', fn: () => { browser._handlers = browser._handlers.filter((x) => x !== h); clearTimeout(t); res(true); } };
    browser._handlers.push(h);
    const t = setTimeout(() => { browser._handlers = browser._handlers.filter((x) => x !== h); res(false); }, 30000);
  });
  await page.send('Page.navigate', { url: CHECK_URL });
  await loaded;
  await page.waitFor("!!document.querySelector('flt-glass-pane') || !!document.querySelector('canvas')", 30000, 'Flutter 首帧容器');
  await sleep(SETTLE_MS);

  // ── 注入前快照 ──
  const before = {
    requests: page.requests.filter((r) => r.phase === 'load'),
    responses: page.responses.filter((r) => r.phase === 'load'),
  };
  const externalBefore = before.requests.filter((r) => isHttpUrl(r.url) && !isLoopbackUrl(r.url));

  // ── 注入 ──
  page.phase = 'inject';
  let input = await page.textarea();
  let clicked = null;
  if (!input) {
    // 输入宿主是惰性创建的：先点一下聊天输入区（右下角），再等它出现。
    clicked = 'point(' + (VW - 160) + ',' + (VH - 30) + ')';
    await page.click(VW - 160, VH - 30);
    for (let i = 0; i < 30 && !input; i++) { await sleep(500); input = await page.textarea(); }
  }
  let injected = null;
  if (!input) {
    problems.push('找不到 Flutter 的文本输入宿主（flt-text-editing-host/textarea）⇒ 无法注入字符，本次不出「注入后」结论');
    console.log('!! 注入失败：找不到输入宿主（clicked=' + clicked + '）');
  } else {
    await page.click(input.x + Math.min(40, Math.max(4, input.w / 4)), input.y + input.h / 2);
    await sleep(600);
    for (const it of INJECT) {
      await page.send('Input.insertText', { text: it.ch });
      await sleep(400);
    }
    await sleep(3000);
    injected = await page.typedText();
    const ok = typeof injected === 'string' && INJECT.every((it) => injected.includes(it.ch));
    console.log('# 注入实证：输入框实测 = ' + JSON.stringify(injected) + '（' + (ok ? '三个字符都在' : '**字符没有全部进去**') + '）');
    if (!ok) problems.push('注入实证失败：输入框里没有三个子集外字符，注入后结论不成立');
  }

  // 给回落字体下载留时间（404/200 都要落表）
  await sleep(12000);
  page.phase = 'after';
  await sleep(500);
  const after = {
    requests: page.requests.filter((r) => r.phase === 'inject' || r.phase === 'after'),
    responses: page.responses.filter((r) => r.phase === 'inject' || r.phase === 'after'),
  };
  const externalAfter = after.requests.filter((r) => isHttpUrl(r.url) && !isLoopbackUrl(r.url));

  // ── 阳性对照：证明探测器看得见跨源请求（不计入断言窗口） ──
  page.phase = 'control';
  await page.evaluate("fetch('https://fonts.gstatic.com/s/notosanssc/v1/__probe__.woff2').catch(function () { return null; })");
  await sleep(2500);
  const control = page.requests.filter((r) => r.phase === 'control' && isHttpUrl(r.url) && !isLoopbackUrl(r.url));
  const controlResponses = page.responses.filter((r) => r.phase === 'control');
  page.phase = 'done';

  // ── 按 host 分组 ──
  const hosts = {};
  for (const r of page.requests) {
    if (!isHttpUrl(r.url)) continue;
    const h = hostOf(r.url);
    if (!hosts[h]) hosts[h] = { load: 0, inject: 0, after: 0, control: 0, total: 0, loopback: isLoopbackUrl(r.url) };
    if (hosts[h][r.phase] === undefined) hosts[h][r.phase] = 0;
    hosts[h][r.phase]++;
    hosts[h].total++;
  }

  // ── 同源 font-fallback 命中 ──
  const fbResponses = page.responses.filter((r) => /\/font-fallback\//.test(r.url));
  const fb200 = fbResponses.filter((r) => r.status === 200);
  const fb404 = fbResponses.filter((r) => r.status === 404);
  const fbOther = fbResponses.filter((r) => r.status !== 200 && r.status !== 404);
  const fam = (u) => { const m = /\/font-fallback\/([^/]+)\//.exec(u); return m ? m[1] : '?'; };
  const famHits = {};
  for (const r of fbResponses) {
    const k = fam(r.url) + ' ' + r.status;
    famHits[k] = (famHits[k] || 0) + 1;
  }

  const fontWarn = [...page.console, ...page.logs]
    .filter((e) => /font|Font|woff|glyph/i.test(e.text))
    .map((e) => e.phase + '/' + e.level + ': ' + e.text);

  // ── 输出 ──
  console.log('');
  console.log('== 按 host 分组的请求数（load=页面加载期 / inject=注入期 / after=注入后 / control=阳性对照）==');
  console.log(pad('host', 34) + padl('load', 6) + padl('inject', 8) + padl('after', 7) + padl('control', 9) + padl('total', 7) + '   loopback');
  for (const h of Object.keys(hosts).sort((a, b) => hosts[b].total - hosts[a].total)) {
    const v = hosts[h];
    console.log(pad(h, 34) + padl(v.load || 0, 6) + padl(v.inject || 0, 8) + padl(v.after || 0, 7) + padl(v.control || 0, 9) + padl(v.total, 7) + '   ' + (v.loopback ? 'yes' : 'NO'));
  }
  console.log('');
  console.log('== 跨源（非 loopback）请求 ==');
  console.log('  注入前 = ' + externalBefore.length + '  注入后 = ' + externalAfter.length + '  阳性对照（脚本故意） = ' + control.length);
  for (const r of externalBefore.concat(externalAfter)) console.log('    !! ' + r.phase + ' ' + r.method + ' ' + r.url);
  for (const r of control) {
    const resp = controlResponses.find((x) => x.url === r.url);
    console.log('    (对照，不计入断言) ' + r.url + ' → ' + (resp ? resp.status : 'no-response'));
  }
  console.log('');
  console.log('== 同源 font-fallback/** 请求（' + fbResponses.length + ' 条）==');
  for (const r of fbResponses) {
    console.log('  ' + r.status + '  ' + r.phase + '  ' + fam(r.url) + '  ' + r.url);
  }
  if (fbResponses.length === 0) console.log('  （无——回落从未发生，或页面没有真的用到引擎回落）');
  console.log('');
  console.log('== 同源 font-fallback 按 字族×状态 聚合 ==');
  for (const k of Object.keys(famHits).sort()) console.log('  ' + pad(k, 34) + famHits[k]);
  console.log('');
  console.log('== 控制台/日志里的字体相关行（' + fontWarn.length + ' 条）==');
  for (const t of fontWarn.slice(0, 40)) console.log('  ' + t);
  if (fontWarn.length === 0) console.log('  （无）');
  console.log('');
  console.log('== 断言 ==');
  const checks = [];
  const add = (name, ok, detail) => { checks.push({ name, ok: !!ok, detail }); console.log('  [' + (ok ? 'PASS' : 'FAIL') + '] ' + name + ' —— ' + detail); };

  add('注入前：非 loopback 请求 = 0（页面加载期不出网）', externalBefore.length === 0,
    '实测 ' + externalBefore.length + ' 条' + (externalBefore.length ? '：' + externalBefore.map((r) => r.url).join(',') : ''));
  add('注入后：非 loopback 请求 = 0（含字体回落窗口）', externalAfter.length === 0,
    '实测 ' + externalAfter.length + ' 条' + (externalAfter.length ? '：' + externalAfter.map((r) => r.url).join(',') : ''));
  add('同源命中 notocoloremoji（🀄 U+1F004）200',
    fb200.some((r) => fam(r.url) === 'notocoloremoji'),
    'notocoloremoji 200 条数 = ' + fb200.filter((r) => fam(r.url) === 'notocoloremoji').length);
  add('同源命中 notomusic（𝄞 U+1D11E）200',
    fb200.some((r) => fam(r.url) === 'notomusic'),
    'notomusic 200 条数 = ' + fb200.filter((r) => fam(r.url) === 'notomusic').length);
  if (injected !== null) {
    add('注入实证：三个子集外字符真的进了 Flutter 输入宿主',
      INJECT.every((it) => typeof injected === 'string' && injected.includes(it.ch)),
      '输入框实测 = ' + JSON.stringify(injected));
  }
  add('阳性对照：探测器确实看得见跨源请求（否则 external=0 无意义）', control.length > 0,
    '脚本故意请求 fonts.gstatic.com → 记录到 ' + control.length + ' 条');
  add('同源 font-fallback 无 5xx / 非 200|404 状态', fbOther.length === 0,
    '非 200|404 的状态=' + JSON.stringify(fbOther.map((r) => r.status + ' ' + r.url)));

  result = {
    ok: problems.length === 0 && checks.every((c) => c.ok),
    problems,
    checkUrl: CHECK_URL,
    cdp: ver.Browser,
    injected,
    checks,
    externalBefore: externalBefore.map((r) => r.url),
    externalAfter: externalAfter.map((r) => r.url),
    control: control.map((r) => r.url),
    fontFallback: fbResponses.map((r) => ({ status: r.status, phase: r.phase, family: fam(r.url), url: r.url })),
    familyStatus: famHits,
    fontWarn,
    hosts,
    requests: page.requests,
    responses: page.responses,
    failures: page.failures,
  };
  console.log('');
  console.log(result.ok ? 'RESULT: PASS' : 'RESULT: FAIL');
  if (problems.length) for (const p of problems) console.log('  !! ' + p);
} catch (e) {
  console.log('脚本异常：' + e.message);
  result = { ok: false, error: e.message, requests: page.requests, responses: page.responses, failures: page.failures };
  console.log('RESULT: FAIL');
} finally {
  if (OUT) { try { writeFileSync(OUT, JSON.stringify(result, null, 2)); console.log('# 原始事件已写入 ' + OUT); } catch (e) { console.log('!! 写 ' + OUT + ' 失败：' + e.message); } }
  await page.close();
  browser.close();
}
process.exit(result && result.ok ? 0 : 1);
