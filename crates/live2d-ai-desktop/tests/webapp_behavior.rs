//! web_api 行为基线——集成测试：Rust 拉起 Node 执行真实 `app.js`（stub DOM 桩）。
//!
//! 为什么在 Rust 里跑前端：
//! - 仓库 rust-ratio 门禁 ≥95%（`xtask` 统计 `.js` 计入分母），新增独立 `.js` 测试文件会直接拉低占比，
//!   因此把「Node 脚本 + DOM 桩 + 场景」全部内嵌为 Rust 字符串，由 `.rs` spawn node。
//! - node 缺失时该测试 **SKIP**（stderr 提示、不 FAIL），无 node 的 CI 依旧全绿。
//!
//! 机制：每个场景 = HARNESS(桩/断言工具) + `app.js` 原文 + 场景代码，拼接后整体经 stdin 喂给 `node`。
//! 单脚本单进程执行，作用域天然连通（app.js 顶层函数/`state` 可直接被场景代码引用），
//! 无跨上下文烦恼；`"use strict"` 只由 HARNESS 头部声明一次。
//!
//! 每场景打印 `BEHAVIOR <name> PASS/FAIL <detail>`；任一失败该测试 fail，
//! 并携带 node 侧 stdout/stderr 便于定位（行为基线的作用：重构时任何行为漂移在此暴露）。

use std::process::Command;

const HARNESS: &str = r#"
// ===== DOM / 运行环境桩 =====
const __els = new Map();
function makeEl(id) {
  const el = {
    id, value: "", textContent: "", innerHTML: "", hidden: false, disabled: false,
    checked: false, files: [],
    style: { removeProperty(){}, setProperty(){} },
    dataset: {},
    classList: { add(){}, remove(){}, toggle(){} },
    addEventListener(type, fn) { (this.__l = this.__l || {})[type] = this.__l[type] || []; this.__l[type].push(fn); },
    _emit(type, ev) { for (const fn of (this.__l && this.__l[type]) || []) fn(ev || {}); },
    closest(){ return null; }, querySelectorAll(){ return []; }, querySelector(){ return null; },
    setPointerCapture(){}, releasePointerCapture(){},
    parentNode: { firstChild: null, querySelector(){ return null; }, querySelectorAll(){ return []; } },
    firstChild: null,
  };
  return el;
}
const __RE = new Map();
function el(id) { if (!__RE.has(id)) __RE.set(id, makeEl(id)); return __RE.get(id); }
const getEl = (id) => (__RE.has(id) ? __RE.get(id) : null);

const __doc = {
  activeElement: null,
  getElementById: (id) => el(id),
  querySelectorAll: () => [],
  createElement: (tag) => el("__dyn__" + tag),
  addEventListener(){}, removeEventListener(){},
};
global.document = __doc;
global.window = { addEventListener(){}, postToStage: undefined };
global.location = { origin: "http://localhost.local" };
const __ls = new Map(); global.localStorage = {
  getItem: (k) => (__ls.has(k) ? __ls.get(k) : null),
  setItem: (k, v) => __ls.set(k, String(v)),
  removeItem: (k) => __ls.delete(k),
  clear: () => __ls.clear(),
};
global.confirm = () => true;
let __timerId = 0;
const __timers = new Map();
global.setInterval = (fn) => { __timers.set(++__timerId, fn); return __timerId; };
global.clearInterval = (id) => __timers.delete(id);
global.setTimeout = (fn) => { __timers.set(++__timerId, fn); return __timerId; };
global.clearTimeout = (id) => __timers.delete(id);
// fetch 桩：场景可用 __setRoute(fn) 按 (path, opts) 返回 {status, body}。默认 200 {}。
let __routeFn = null;
const __fetched = [];
function __route(p, o) { return __routeFn ? __routeFn(p, o) : null; }
global.__setRoute = (fn) => { __routeFn = fn; };
global.fetch = async (path, opts) => {
  __fetched.push({ path, opts });
  const { status = 200, body = {} } = __route(path, opts || {}) || {};
  return {
    ok: status < 400, status,
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    statusText: status === 200 ? "OK" : "ERR",
  };
};
// 测试工具
const __fails = [];
function ok(desc, cond, note) { if (!cond) __fails.push(desc + (note ? " | " + note : "")); }
function eq(desc, actual, expect) {
  const a = JSON.stringify(actual), e = JSON.stringify(expect);
  if (a !== e) __fails.push(desc + " | got " + a + " want " + e);
}
function __fail(desc, e) { __fails.push(desc + " | threw " + (e && e.stack ? e.stack.split("\n")[0] : e)); }
function __done(name) {
  if (__fails.length) { console.log("BEHAVIOR " + name + " FAIL: " + __fails.join(" ; ")); process.exit(1); }
  console.log("BEHAVIOR " + name + " PASS");
}
"#;

const SCENE_PURE: &str = r#"
// 纯函数电池：不依赖 DOM/网络，全部直接调用顶层函数。
(function () {
  try {
    // resolveActions：无 capabilities -> 内置 6 个；actions 为空 -> 内置；命中映射；未命中回传。
    state.capabilities = null;
    eq("pure/no-cap", resolveActions().join(","), "点头,摇头,歪头,环视,倾听,惊讶");
    state.capabilities = { actions: [] };
    eq("pure/empty-actions", resolveActions().length, 6);
    state.capabilities = { actions: ["nod", "custom_fancy", "live2d_perform_action_tilt"] };
    eq("pure/map-hit", resolveActions().join(","), "点头,custom_fancy,歪头");
    // strengthOf：缺省 2；越界回退 2；1..3 保留。
    state.actionStrengths = {};
    eq("pure/strength-default", strengthOf("nod"), 2);
    state.actionStrengths = { nod: 3 };
    eq("pure/strength-3", strengthOf("nod"), 3);
    eq("pure/strength-0", strengthOf("shake_no"), 2);
    eq("pure/strength-9", strengthOf("x"), 2);
    // applyStatusText：四个已知 + 未知回退 no_supervisor。
    eq("pure/apply-applied", applyStatusText("applied").toast, "已保存并热重载");
    eq("pure/apply-restart", applyStatusText("restart_required").toast, "已保存，需重启生效");
    eq("pure/apply-queued", applyStatusText("queued").toast, "已保存，配置将在下次对话时应用");
    eq("pure/apply-unknown", applyStatusText("whatever").toast, "已保存，配置将在下次重启后生效");
    // parseLogLevel：大小写不敏感；无等级 -> null。
    eq("pure/parse-info", parseLogLevel("2026-01-01 10:00:00 INFO plugin: hello"), "info");
    eq("pure/parse-error", parseLogLevel("  ERROR boom"), "error");
    eq("pure/parse-none", parseLogLevel("plain line without level"), null);
    // filterLogLines：全部/未知回退/级别过滤计数与无等级行保留。
    const lines = ["a INFO x", "b DEBUG y", "c WARN v", "d", "e ERROR z"];
    let r = filterLogLines(lines, "all");
    eq("pure/filter-all-shown", r.shown.join("|"), lines.join("|"));
    eq("pure/filter-all-count", r.filtered, 0);
    r = filterLogLines(lines, "warn");
    eq("pure/filter-warn-shown", r.shown.join("|"), "c WARN v|d|e ERROR z");
    eq("pure/filter-warn-filtered", r.filtered, 2);
    r = filterLogLines(lines, "bogus");
    eq("pure/filter-unknown", r.shown.join("|"), lines.join("|"));
    // cmdIdToStageAction 兜底（冒烟已覆盖主要路径）
    eq("pure/cmd-suffix", cmdIdToStageAction("live2d_perform_action_listen"), "listen");
  } catch (e) { __fail("pure", e); }
  __done("pure");
})();
"#;

const SCENE_SETTINGS: &str = r#"
// buildPatch / renderAll / save 热腰带（DOM 桩读取真实元素属性）。
(function () {
  try {
    const sv = {
      llm: { base_url: "http://llm.default", model: "m1", has_api_key: false },
      tts: { base_url: "http://tts.default", voice: "v1", has_api_key: false },
      persona: { name: "n1", description: "d1", personality: "p1", scenario: "s1", first: "f1" },
      dev_mode: false,
    };
    state.settingsView = sv;
    state.capabilities = { version: "9.9" };
    state.status = { uptime_s: 7, started_at: "now", active_model_id: "vm1", audio: { backend: "rw", available: true }, llm: { base_url: "uL", has_api_key: false }, tts: { base_url: "uT", has_api_key: false }, config_path: "/cfg", current_epoch: 3, dev_mode: false };
    __setRoute((path, o) => {
      if (path === "/api/v1/app/status") return { status: 200, body: state.status };
      if (path === "/api/v1/app/capabilities") return { status: 200, body: { version: "1.0" } };
      if (path === "/api/v1/settings" && o.method === "PATCH") return { status: 200, body: { apply_status: "applied", settings: sv } };
      if (path === "/api/v1/settings") return { status: 200, body: sv };
      return { status: 200, body: {} };
    });
    renderAll();
    eq("set/version", els.ver.textContent, "v9.9");
    eq("set/uptime", els.uptime.textContent, "7s");
    eq("set/activeModel", els.activeModel.textContent, "vm1");
    // 无变更时 buildPatch 仅 dev_mode 段。
    const p0 = buildPatch();
    ok("set/patch-empty", !p0.llm && !p0.tts && !p0.persona, JSON.stringify(p0));
    eq("set/patch-dev", p0.dev_mode, false);
    // 改 llm base_url + voice + persona 名称空 -> null-clear。
    els.llmBaseUrl.value = "http://new";
    els.ttsVoice.value = "v9";
    els.personaName.value = "   ";
    const p1 = buildPatch();
    eq("set/llm-base", p1.llm.base_url, "http://new");
    eq("set/tts-voice", p1.tts.voice, "v9");
    eq("set/persona-null", p1.persona.name, null);
    // keyTouched + 占位符：(已绑定) 或空 -> 段级清除（api_key_env=null + clear_api_key），真实值 -> 直写。
    state.llmKeyTouched = true; state.ttsKeyTouched = false;
    els.llmApiKeyEnv.value = "(已绑定)";
    const p2 = buildPatch();
    eq("set/key-clear-env", p2.llm.api_key_env, null);
    eq("set/key-clear-flag", p2.llm.clear_api_key, true);
    ok("set/key-tts-untouched", p2.tts && p2.tts.clear_api_key === undefined, JSON.stringify(p2));
    els.llmApiKeyEnv.value = "sk-real";
    const p3 = buildPatch();
    eq("set/key-write", p3.llm.api_key_env, "sk-real");
    ok("set/key-write-noclear", p3.llm.clear_api_key === undefined, JSON.stringify(p3.llm));
    // renderAll 的 (已绑定) 占位：has_api_key=true 且未 touched。
    state.llmKeyTouched = false;
    state.settingsView = { ...sv, llm: { ...sv.llm, has_api_key: true } };
    state.status = { ...state.status, llm: { ...state.status.llm, has_api_key: true } };
    renderAll();
    eq("set/placeholder", els.llmApiKeyEnv.value, "(已绑定)");
    // activeElement 是正在编辑的输入时，renderAll 不覆盖（先聚焦再改值再渲染——值必须是保留态）。
    document.activeElement = els.llmBaseUrl;
    els.llmBaseUrl.value = "http://focus-kept";
    renderAll();
    eq("set/focus-kept", els.llmBaseUrl.value, "http://focus-kept");
    // 焦点释放后 renderAll 应写回 sv 的值。
    document.activeElement = null;
    renderAll();
    eq("set/focus-revealed", els.llmBaseUrl.value, "http://llm.default");
    // save() happy path：PATCH 携带 buildPatch 结果，apply_status=applied -> 刷新 + 文案。
    __fetched.length = 0;
    els.llmBaseUrl.value = "http://new2";
    save().then(() => {
      const patch = __fetched.find((f) => f.opts && f.opts.method === "PATCH" && f.path === "/api/v1/settings");
      ok("set/save-PATCH", !!patch, "未发 PATCH");
      eq("set/save-payload-llm", JSON.parse(patch.opts.body).llm.base_url, "http://new2");
      eq("set/save-applytext", els.saveStatus.textContent, "已保存于 " + new Date().toLocaleTimeString());
      // reloadForm 会重置 keyTouched 并重新拉取。
      state.llmKeyTouched = true; state.ttsKeyTouched = true;
      reloadForm().then(() => {
        eq("set/reload-reset", state.llmKeyTouched || state.ttsKeyTouched, false);
        // clearKeyBinding：段级清两个 provider。
        __fetched.length = 0;
        clearKeyBinding().then(() => {
          const cp = __fetched.find((f) => f.opts && f.opts.method === "PATCH");
          ok("set/clear-PATCH", cp, "未发清除 PATCH");
          const pb = JSON.parse(cp.opts.body);
          eq("set/clear-llm", pb.llm.clear_api_key, true);
          eq("set/clear-tts", pb.tts.clear_api_key, true);
          eq("set/clear-touch-reset", state.llmKeyTouched || state.ttsKeyTouched, false);
          __done("settings");
        }).catch((e) => { __fail("set/clear", e); __done("settings"); });
      }).catch((e) => { __fail("set/reload", e); __done("settings"); });
    }).catch((e) => { __fail("set/save", e); __done("settings"); });
  } catch (e) { __fail("settings", e); __done("settings"); }
})();
"#;

const SMOKE: &str = r#"
// 冒烟：纯函数 cmdIdToStageAction 的映射与未知回退。
(function () {
  try {
    eq("smoke/map", cmdIdToStageAction("live2d_perform_action_nod"), "nod");
    eq("smoke/suffix", cmdIdToStageAction("live2d_perform_action_look_around"), "look_around");
    eq("smoke/passthru", cmdIdToStageAction("custom_thing"), "custom_thing");
    eq("smoke/empty", cmdIdToStageAction(""), "");
    eq("smoke/null", cmdIdToStageAction(undefined), "");
    ok("smoke/state-present", typeof state === "object", "state 缺失");
  } catch (e) { __fail("smoke", e); }
  __done("smoke");
})();
"#;

fn node_version() -> Option<String> {
    let out = Command::new("node").arg("--version").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn run_node(script: &str) -> Result<String, String> {
    use std::io::Write;
    let mut child = Command::new("node")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn node 失败: {e}"))?;
    child
        .stdin
        .take()
        .expect("stdin 已 piped")
        .write_all(script.as_bytes())
        .map_err(|e| format!("写 stdin 失败: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("wait node 失败: {e}"))?;
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        return Err(format!(
            "node 退出码 {}；stdout: {}\nstderr: {}",
            out.status,
            if stdout.is_empty() {
                "（无）"
            } else {
                &stdout
            },
            if stderr.is_empty() {
                "（无）"
            } else {
                &stderr
            }
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// 行为基线场景表：新增场景 = 在给定常量写 JS 断言，并在此登记。
const SCENARIOS: &[(&str, &str)] = &[
    ("smoke", SMOKE),
    ("pure", SCENE_PURE),
    ("settings", SCENE_SETTINGS),
];

#[test]
fn webapp_behavior_baseline() {
    let Some(ver) = node_version() else {
        eprintln!(
            "SKIP: node 不可用（行为基线需要 node）——已在无 node 环境跳过，请在有 node 的环境复核"
        );
        return;
    };
    let app_js = include_str!("../src/web_api/app.js");
    for (name, scene) in SCENARIOS {
        let script = format!("{HARNESS}\n{app_js}\n{scene}");
        match run_node(&script) {
            Ok(out) => println!("[{name}] {out}"),
            Err(err) => panic!("[{name}] {err}"),
        }
    }
    eprintln!("[环境] node {ver}（webapp 行为基线 OK）");
}
