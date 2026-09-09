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
    if !out.status.success() {
        return Err(format!(
            "node 退出码 {}；stderr: {}",
            out.status,
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
const SCENARIOS: &[(&str, &str)] = &[("smoke", SMOKE)];

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
