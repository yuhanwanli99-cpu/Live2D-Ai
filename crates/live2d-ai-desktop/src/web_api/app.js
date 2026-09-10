"use strict";
// Live2D-Ai 配置面板前端（W8 任务）。数据源：同源 /api/v1/* 端点。

const state = {
  capabilities: null, status: null, settingsView: null,
  llmKeyTouched: false, ttsKeyTouched: false,
  settingsOpen: false, settingsLoaded: false,
  actionStrengths: {}, // 动作强度缓存（actionId -> 1..3，默认 2）
  appearance: { scale: 1.0, dark: true, lipSync: true, tier: 4096, }, // 外观与舞台（纯本地）
  models: null, modelsError: null,
  stageAck: null, // R5：渲染端的 stage-ack 快照（收到 ACK 才算真正生效）
};

const $ = (id) => document.getElementById(id);
// 仅当元素存在且未聚焦时写入值（避免覆盖用户正在编辑的输入框）。
const setIf = (el, v) => { if (el && document.activeElement !== el) el.value = v; };
 // 元素表（全量索引，键名即 DOM id——tests_html 锚点锁定，勿删改）
const els = {
  ver: $("ver"), conn: $("conn"), dot: $("dot"),
  llmBadge: $("llmBadge"), ttsBadge: $("ttsBadge"),
  llmBaseUrl: $("llmBaseUrl"), llmModel: $("llmModel"), llmApiKeyEnv: $("llmApiKeyEnv"),
  ttsBaseUrl: $("ttsBaseUrl"), ttsVoice: $("ttsVoice"), ttsApiKeyEnv: $("ttsApiKeyEnv"),
  systemPrompt: $("systemPrompt"), devMode: $("devMode"),
  personaName: $("f-p-name"), personaDesc: $("f-p-desc"),
  personaPersona: $("f-p-persona"), personaScenario: $("f-p-scenario"), personaFirst: $("f-p-first"),
  appIdleToggle: $("f-idle"), appClickToggle: $("f-click"),
  bgFileBtn: $("f-bgimg-btn"), bgFileInput: $("f-bgimg-input"), bgClearBtn: $("f-bgimg-clear"),
  btnSave: $("btnSave"), btnReload: $("btnReload"), btnClearKey: $("btnClearKey"),
  btnTestLlm: $("btnTestLlm"), btnTestTts: $("btnTestTts"),
  llmTestResult: $("llmTestResult"), ttsTestResult: $("ttsTestResult"), saveStatus: $("saveStatus"),
  cfgPath: $("cfgPath"), startedAt: $("startedAt"), uptime: $("uptime"),
  activeModel: $("activeModel"), audio: $("audio"), devModeStatus: $("devModeStatus"), epoch: $("epoch"),
  llmBase: $("llmBase"), llmHasKey: $("llmHasKey"),
  ttsBase: $("ttsBase"), ttsHasKey: $("ttsHasKey"),
  logsCard: $("logsCard"), logsFile: $("logsFile"), logsPre: $("logs"),
  logsStatus: $("logsStatus"), btnReloadLogs: $("btnReloadLogs"), logLevel: $("logLevel"),
  btnRefresh: $("btnRefresh"), toast: $("toast"),
  btnSettings: $("btnSettings"), settingsModal: $("settings-modal"),
  settingsRail: $("settingsRail"), settingsSectionArea: $("settingsSectionArea"),
  settingsOverlay: $("settings-overlay"), btnSettingsClose: $("btnSettingsClose"),
  grpLogs: $("grp-logs"), actionsList: $("actionsList"),
  appScale: $("appScale"), appBgDark: $("appBgDark"), appRenderTier: $("appRenderTier"),
  appLipSyncToggle: $("appLipSyncToggle"), live2dFrame: $("live2d-frame"),
  modelsList: $("modelsList"), btnRefreshModels: $("btnRefreshModels"), modelsStatus: $("modelsStatus"),
  stageToolbar: $("stageToolbar"), btnZoomIn: $("btnZoomIn"),
  btnZoomOut: $("btnZoomOut"), btnResetView: $("btnResetView"), btnActions: $("btnActions"),
};

// ---- 设置模态 + 左导航 rail ----
// section 注册（按 deepseekharness slots 思想）——规格 C3 字面。
const SETTINGS_SECTIONS = [
  { id:"character", label:"角色与人设", icon:"i-user" },
  { id:"appearance", label:"外观与舞台", icon:"i-palette" },
  { id:"ai", label:"AI 与语音", icon:"i-mic" },
  { id:"actions", label:"动作与互动", icon:"i-motion" },
  { id:"models", label:"模型管理", icon:"i-box" },
  { id:"dev", label:"开发者", icon:"i-tools" },
  { id:"mods", label:"Mod 管理", icon:"i-puzzle" },
];
let activeSection = "character";
function renderRail() {
  if (!els.settingsRail) return;
  // rail 已在 HTML 静态存在（含 SVG 图标与 data-s）——动态渲染仅兜底/保持兼容。
  const exists = els.settingsRail.querySelectorAll(".rail-item").length;
  if (exists >= SETTINGS_SECTIONS.length) return; // 静态 rail 已就绪
  els.settingsRail.innerHTML = SETTINGS_SECTIONS.map((s) =>
    '<button class="rail-item" data-s="' + s.id + '">' +
    '<svg class="ico-svg"><use href="#' + s.icon + '"/></svg>' +
    '<span class="lbl-t">' + s.label + '</span></button>'
  ).join("");
}
// rail 点击委托：点击任一 .rail-item → setActiveSection。
if (els.settingsRail) {
  els.settingsRail.addEventListener("click", (e) => {
    const item = e.target.closest(".rail-item");
    if (item) setActiveSection(item.dataset.s);
  });
}
function setActiveSection(id) {
  activeSection = id;
  document.querySelectorAll(".rail-item").forEach((r) =>
    r.classList.toggle("active", r.dataset.s === id)
  );
  document.querySelectorAll(".settings-section").forEach((s) =>
    s.classList.toggle("active", s.dataset.p === id)
  );
  if (id === "dev" || id === "mods") renderGroups();
  if (id === "mods") renderMods();
  if (id === "actions") renderActions();
  if (id === "models") loadModels();
  if (id === "appearance") bindAppearance();
}
let actStrength = 2;
function segSel(btn) {
  const seg = btn.closest(".seg");
  if (seg) seg.querySelectorAll(".seg-btn").forEach(b => b.classList.remove("active"));
  btn.classList.add("active");
  const v = parseInt(btn.dataset.v || btn.textContent, 10);
  if (!isNaN(v)) actStrength = v;
}
function actFire(actName) {
  const log = $("actLog");
  if (log) {
    const line = document.createElement("div");
    line.textContent = new Date().toLocaleTimeString() + " " + actName + " (强度" + actStrength + ")";
    log.insertBefore(line, log.firstChild);
  }
  // 触发真实动作：go to stage + commands invoke
  const cmds = { nod:"live2d_perform_action_nod", shake_no:"live2d_perform_action_shake_no",
    tilt:"live2d_perform_action_tilt", look_around:"live2d_perform_action_look_around",
    listen:"live2d_perform_action_listen", surprise:"live2d_perform_action_surprise" };
  const cmdId = cmds[actName] || ("live2d_perform_action_" + actName);
  invokeAction(actName, cmdId);
}
function toggleActPanel() {
  const p = $("actPanel");
  if (p) p.hidden = !p.hidden;
}
// 动作浮窗交互委托
(function bindActPanel() {
  const panel = $("actPanel");
  if (!panel) return;
  const head = $("actDrag");
  if (head) {
    let sx=0, sy=0, ox=0, oy=0, drag=false;
    head.addEventListener("pointerdown", (e) => {
      if (e.target.closest("button")) return;
      drag=true; sx=e.clientX-ox; sy=e.clientY-oy;
      head.setPointerCapture(e.pointerId);
    });
    head.addEventListener("pointermove", (e) => {
      if (!drag) return;
      ox=e.clientX-sx; oy=e.clientY-sy;
      panel.style.right="auto"; panel.style.bottom="auto";
      panel.style.left=ox+"px"; panel.style.top=oy+"px";
    });
    head.addEventListener("pointerup", () => drag=false);
  }
  panel.addEventListener("click", (e) => {
    const b = e.target.closest(".act-btn[data-act]");
    if (b) actFire(b.dataset.act);
    const sg = e.target.closest(".seg-btn[data-v]");
    if (sg) segSel(sg);
  });
  const close = $("btnActClose");
  if (close) close.addEventListener("click", () => panel.hidden = true);
})();
// 设置面板「动作与互动 → 基础动作试演」网格按钮（.act-grid .act-btn[data-act]）复用 actFire，
// 与 #actPanel 浮窗一致；全局委托避免 per-button 绑定。
document.addEventListener("click", (e) => {
  const b = e.target.closest(".act-grid .act-btn[data-act]");
  if (b) actFire(b.dataset.act);
});

function openSettings() {
  if (!state.settingsLoaded) { state.settingsLoaded = true; loadSettingsLazy(); }
  els.settingsModal.hidden = false;
  els.settingsOverlay.hidden = false;
  renderRail();
  setActiveSection(activeSection);
}
function closeSettings() {
  els.settingsModal.hidden = true;
  els.settingsOverlay.hidden = true;
  state.settingsOpen = false;
}
function loadSettingsLazy() {
  if (!state.capabilities) {
    api("GET", "/api/v1/app/capabilities").then((cap) => {
      state.capabilities = cap; renderGroups();
    }).catch(() => {});
  } else {
    renderGroups();
  }
  renderMods();
  loadModels();
  if (!state.settingsView) {
    api("GET", "/api/v1/settings").then((sv) => {
      state.settingsView = sv; renderAll();
    }).catch(() => {});
  }
}
const STATUS_POLL_MS = 5000;
const LOGS_POLL_MS = 8000;

// ---- Toast ----
let toastTimer = null;
function toast(msg, kind) {
  els.toast.textContent = msg;
  els.toast.className = kind || "";
  els.toast.style.display = "block";
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { els.toast.style.display = "none"; }, 3500);
}
// ---- HTTP ----
async function api(method, path, body) {
  const opts = { method, headers: {} };
  if (body !== undefined) {
    opts.headers["Content-Type"] = "application/json";
    opts.body = JSON.stringify(body);
  }
  let resp;
  try {
    resp = await fetch(path, opts);
  } catch (e) {
    throw { network: true, message: "网络错误：" + e };
  }
  const text = await resp.text();
  let data = null;
  if (text) {
    try { data = JSON.parse(text); } catch (_) { data = { raw: text }; }
  }
  if (!resp.ok) {
    const err = (data && data.error) || { code: "http_" + resp.status, message: text || resp.statusText };
    throw { status: resp.status, error: err };
  }
  return data;
}

function setConn(kind, label) {
  els.dot.className = "dot " + kind;
  els.conn.textContent = label;
}
async function loadAll() {
  setConn("loading", "加载中…");
  try {
    const [cap, st, sv] = await Promise.all([
      api("GET", "/api/v1/app/capabilities"),
      api("GET", "/api/v1/app/status"),
      api("GET", "/api/v1/settings"),
    ]);
    state.capabilities = cap;
    state.status = st;
    state.settingsView = sv;
    renderAll();
    setConn("ok", "已连接");
  } catch (e) {
    setConn("err", "连接失败");
    toast("加载失败：" + (e.error ? e.error.message : e.message), "err");
  }
}

// ---- capability 驱动：分组可见性（字段清单参考 dto::AppInfo）----
function renderGroups() {
  const cap = state.capabilities;
  if (!cap) return;
  const sv = state.settingsView || { dev_mode: false };

  // #grp-logs：仅 dev_mode 开启时显示，否则提示。
  // v2.5 HTML 里日志区没有独立的 grp-logs 容器，直接以 logsCard 显示/隐藏（grpLogs 兼容旧 HTML，防空引用）。
  const grp = els.grpLogs;
  if (sv.dev_mode) {
    if (grp) grp.style.removeProperty("display");
    if (els.logsCard) els.logsCard.classList.remove("hidden");
    if (els.btnActions) els.btnActions.classList.remove("hidden");
  } else {
    if (grp) grp.style.display = "none";
    if (els.logsCard) els.logsCard.classList.add("hidden");
    if (els.btnActions) els.btnActions.classList.add("hidden");
    if (els.logsPre) els.logsPre.textContent = "（需开启开发者模式）";
    if (els.logsStatus) {
      els.logsStatus.textContent = "dev_mode 关闭";
      els.logsStatus.className = "status warn";
    }
  }
  // 动作与互动：按能力驱动渲染（actions 为空则用内置 6 个）。
  renderActions();
}

// ---- 动作与互动 ----
const BUILTIN_ACTIONS = [
  ["点头","live2d_perform_action_nod"],["摇头","live2d_perform_action_shake_no"],
  ["歪头","live2d_perform_action_tilt"],["环视","live2d_perform_action_look_around"],
  ["倾听","live2d_perform_action_listen"],["惊讶","live2d_perform_action_surprise"],
];
// cmdId → iframe bridge action id（英文 id）：取 command id 后缀，未命中原样回传。
function cmdIdToStageAction(cmdId) {
  if (!cmdId) return "";
  const m = cmdId.match(/live2d_perform_action_(.+)$/);
  return m ? m[1] : cmdId;
}
function resolveActions() {
  const cap = state.capabilities;
  const names = cap && cap.actions && cap.actions.length ? cap.actions : null;
  if (!names) return BUILTIN_ACTIONS.map((a) => a[0]);
  return names.map((n) => {
    const hit = BUILTIN_ACTIONS.find((a) => a[0] === n || a[1].endsWith(n));
    return hit ? hit[0] : n;
  });
}
function strengthOf(name) {
  const v = state.actionStrengths[name];
  return v >= 1 && v <= 3 ? v : 2;
}
function renderActions() {
  const list = els.actionsList;
  if (!list) return;
  const names = resolveActions();
  list.innerHTML = names.map((name) => {
    const cmdId = (BUILTIN_ACTIONS.find((a) => a[0] === name) || ["", ""])[1];
    return '<div class="row"><span style="flex:1">' + name + '</span>'
      + '<label class="field-meta" style="min-width:7ch;text-align:right">强度：<input type="number" min="1" max="3" value="' + strengthOf(name) + '" data-action="' + name + '" style="width:3ch"></label>'
      + '<button class="secondary" data-try="' + name + '" data-cmd="' + cmdId + '">试演</button></div>';
  }).join("");
  list.oninput = (e) => {
    const inp = e.target.closest('input[data-action]');
    if (!inp) return;
    let v = parseInt(inp.value, 10);
    if (!v || v < 1) v = 1; if (v > 3) v = 3;
    inp.value = String(v); state.actionStrengths[inp.dataset.action] = v;
    postStageConfig();
  };
  list.onclick = (e) => {
    const b = e.target.closest("button[data-try]");
    if (!b) return;
    invokeAction(b.dataset.try, b.dataset.cmd);
  };
}
async function invokeAction(name, cmdId) {
  const strength = strengthOf(name);
  const actionId = cmdIdToStageAction(cmdId); // 映射到 iframe bridge action id（英文 id）
  try {
    await api("POST", "/api/v1/commands/" + encodeURIComponent(cmdId) + "/invoke", { strength });
    // bridge 通知：动作开始 → wasm 端按 1500ms 自然结束（不再发送 end 指令）。
    // end 消息保留供外部强制停止，但试演不再提前发送 end。
    postToStage({ type: "action-state", action: actionId, state: "start", strength });
    toast("试演 " + name + "：已提交（等待渲染端 ACK）", "ok");
  } catch (e) {
    toast("试演 " + name + " 失败：" + (e.error ? e.error.code + "：" + e.error.message : e.message), "err");
  }
}
// ---- iframe bridge 发送 helper ----
// 统一 postMessage 到 live2d-frame（判存在 + try/catch）；wasm 只收 String 必须 JSON.stringify，
// 同时传 location.origin 同源校验；chat.js 经 window.postToStage 复用。
function postToStage(msg) {
  try {
    const frame = els.live2dFrame;
    if (!frame || !frame.contentWindow) return false;
    frame.contentWindow.postMessage(JSON.stringify(msg), location.origin);
    return true;
  } catch (_) {
    return false;
  }
}
window.postToStage = postToStage; // 供 chat.js（独立 IIFE）复用，避免硬依赖

// R5：监听渲染端 stage-ack 快照，收到 ACK 才 toast「已应用」。
window.addEventListener("message", (ev) => {
  // 同源校验（wasm 仅回 postMessage("*")，此处用 origin 过滤）
  if (ev.origin !== location.origin) return;
  let msg;
  try { msg = typeof ev.data === "string" ? JSON.parse(ev.data) : ev.data; } catch (_) { return; }
  if (msg && msg.type === "stage-ack") {
    state.stageAck = {
      applied: msg.applied,
      scale: msg.scale,
      offset_x: msg.offset_x,
      offset_y: msg.offset_y,
      msg_type: msg.msg_type,
    };
    toast("渲染端已应用", "ok");
  }
});

// ---- 外观与舞台 ----
function postStageConfig() {
  const cfg = {
    type: "stage-config",
    scale: state.appearance.scale,
    dark: state.appearance.dark,
    lipSync: state.appearance.lipSync,
  };
  // B1：互动/空闲开关（f-idle / f-click），缺元素则不发该字段。
  if (els.appIdleToggle) cfg.idleEnabled = els.appIdleToggle.checked;
  if (els.appClickToggle) cfg.clickEnabled = els.appClickToggle.checked;
  // ADR §3.4：渲染档位（4096/8192/16384），与前几项一致走本地预览。
  if (els.appRenderTier) cfg.tier = state.appearance.tier;
  postToStage(cfg);
}
function bindAppearance() {
  // P0-5：iframe bridge 已真实接通（stage-config/audio-volume/action-state/load-model）。
  // 移除过时标记，不再呈现"成功但未生效"假象。
  if (els.appLipSyncToggle && els.appLipSyncToggle.parentNode) {
    const parent = els.appLipSyncToggle.parentNode;
    const meta = parent.querySelector(".field-meta");
    if (meta) meta.textContent = "本地预览（不写入后端）";
  }
  // bindAppearance 四控件 loop 化（appScale/appBgDark/appLipSyncToggle/appRenderTier）：
  // 共享「读 LS → 同步 state.appearance + el → 监听改变 → 写 LS + postStageConfig」。
  // 行为基线保护：`if (!el) return` 等价原 `if (els.x){...}` 早返——缺元素不影响其余控件。
  const clamp = (v) => Math.max(0.5, Math.min(1.5, v));
  const setLs = (k, v) => localStorage.setItem("dsh.app." + k, String(v));
  const getLs = (k) => localStorage.getItem("dsh.app." + k);
  APPEARANCE_CONTROLS.forEach((c) => {
    const el = els[c.el];
    if (!el) return;
    const ls = getLs(c.k);
    if (ls !== null) {
      if (c.kin === "range") {
        const v = clamp(parseFloat(ls) || 1.0); el.value = v.toFixed(2); state.appearance[c.k] = v;
      } else if (c.kin === "bool") {
        const v = ls === "true"; el.checked = v; state.appearance[c.k] = v;
      } else if (c.kin === "tier") {
        if (c.opts.includes(ls)) { const v = parseInt(ls, 10); el.value = ls; state.appearance[c.k] = v; }
        else { el.value = String(state.appearance[c.k]); }
      }
    }
    el.addEventListener(c.kin === "range" ? "input" : "change", () => {
      if (c.kin === "range") {
        let v = clamp(parseFloat(el.value) || 1.0); el.value = v.toFixed(2); state.appearance[c.k] = v; setLs(c.k, v);
      } else if (c.kin === "bool") {
        const v = el.checked; state.appearance[c.k] = v; setLs(c.k, v);
      } else if (c.kin === "tier") {
        const v = parseInt(el.value, 10) || 4096; state.appearance[c.k] = v; setLs(c.k, v);
      }
      postStageConfig();
    });
  });
  // 初始同步一次 stage-config，保证后端/bridge 与 UI 一致（含 idle/click 开关）。
  postStageConfig();
}
// bindAppearance 四控件 loop 配置：kind(range/bool/tier)决定 parse+序列化方式。
const APPEARANCE_CONTROLS = [
  { el: "appScale", k: "scale", kin: "range" },
  { el: "appBgDark", k: "dark", kin: "bool" },
  { el: "appLipSyncToggle", k: "lipSync", kin: "bool" },
  { el: "appRenderTier", k: "tier", kin: "tier", opts: ["4096", "8192", "16384"] },
];
// ---- B1/C2：互动开关 + 自定义背景（本地，postToStage 即时生效）----
function bindStageInteractions() {
  // B1：空闲随机动作 / 点击互动开关 → stage-config（不持久化）。
  if (els.appIdleToggle) {
    els.appIdleToggle.addEventListener("change", () => postStageConfig());
  }
  if (els.appClickToggle) {
    els.appClickToggle.addEventListener("change", () => postStageConfig());
  }
  // C2：自定义背景图（本地 FileReader → dataURL → stage-bg；localStorage 记忆）。
  const BG_LS_KEY = "dsh.stage.bg";
  function clearBg() {
    localStorage.removeItem(BG_LS_KEY);
    postToStage({ type: "stage-bg", dataUrl: null });
    toast("已清除背景图", "ok");
  }
  if (els.bgFileBtn && els.bgFileInput) {
    els.bgFileBtn.addEventListener("click", () => els.bgFileInput.click());
    els.bgFileInput.addEventListener("change", () => {
      const f = els.bgFileInput.files && els.bgFileInput.files[0];
      if (!f) return;
      const reader = new FileReader();
      reader.onload = () => {
        const dataUrl = String(reader.result || "");
        let persisted = true;
        try { localStorage.setItem(BG_LS_KEY, dataUrl); } catch (_) { persisted = false; }
        const sent = postToStage({ type: "stage-bg", dataUrl });
        // localStorage 超限（如 >5MB jpg 的 dataURL）：本次会话仍生效，但刷新不记忆。
        if (sent) {
          toast(persisted ? "背景图已应用（本地）" : "背景图已应用；图片过大，刷新后不会保留", persisted ? "ok" : "warn");
        } else {
          toast("背景图已保存，但渲染端未加载（刷新后生效）", "warn");
        }
      };
      reader.onerror = () => toast("背景图读取失败", "err");
      reader.readAsDataURL(f);
      els.bgFileInput.value = "";
    });
  }
  if (els.bgClearBtn) els.bgClearBtn.addEventListener("click", clearBg);
  // 加载时恢复：有记忆 → 补发 stage-bg；无 → 不发（wasm 默认无背景）。
  try {
    const saved = localStorage.getItem(BG_LS_KEY);
    if (saved) postToStage({ type: "stage-bg", dataUrl: saved });
  } catch (_) { /* 忽略 */ }
}
// ---- 模型管理 ----
async function loadModels() {
  const st = els.modelsStatus;
  if (st) { st.textContent = "加载中…"; st.className = "status"; }
  try {
    const data = await api("GET", "/api/v1/models");
    state.models = data; state.modelsError = null;
    if (st) { st.textContent = "共 " + (data.models || []).length + " 个"; st.className = "status ok"; }
  } catch (e) {
    state.models = null; state.modelsError = e;
    if (st) { st.textContent = "加载失败：" + (e.error ? e.error.code + "：" + e.error.message : e.message); st.className = "status err"; }
  }
  renderModels();
}
function renderModels() {
  const list = els.modelsList;
  if (!list) return;
  const models = (state.models && state.models.models) || [];
  if (state.modelsError || !models.length) {
    list.innerHTML = '<span class="field-meta">' + (state.modelsError ? "加载失败 / 暂无模型" : "暂无模型（本地 assets/models/bai 未导入时显示此提示）") + '</span>';
    return;
  }
  list.innerHTML = models.map((m) =>
    '<div class="row"><span style="flex:1">' + m.id + ' / ' + (m.display_name || m.id) + '</span>'
    + (m.active ? '<span class="status ok">当前</span>' : '<span class="status">闲置</span>')
    + '<button class="secondary" data-activate="' + m.id + '">设为当前</button></div>'
  ).join("");
  list.onclick = (e) => {
    const b = e.target.closest("button[data-activate]");
    if (!b) return;
    activateModel(b.dataset.activate);
  };
}
async function activateModel(id) {
  try {
    const resp = await api("POST", "/api/v1/models/" + encodeURIComponent(id) + "/activate", {});
    toast("已设为当前：" + id, "ok");
    // bridge 通知：加载模型。url 构造规则 v1：/models/<id>/runtime/<id>.model3.json
    // 若 activate 响应含路径字段（model_url / url），优先用响应值。
    const modelUrl = (resp && (resp.model_url || resp.url)) || "/models/" + encodeURIComponent(id) + "/runtime/" + encodeURIComponent(id) + ".model3.json";
    postToStage({ type: "load-model", url: modelUrl });
  } catch (e) {
    toast("激活失败：" + (e.error ? e.error.code + "：" + e.error.message : e.message), "err");
  }
  await loadModels();
}
const MOD_STATUS = {running:["运行中","ok"],disabled:["已禁用","warn"],failed:["失败","err"],starting:["启动中","warn"],stopping:["停止中","warn"]};
const MOD_ACTIONS = {enable:"启用",disable:"禁用",restart:"重启"};
async function renderMods() {
  const list = $("modsList");
  if (!list) return; list.innerHTML = "";
  let mods, err = null;
  try { mods = await api("GET", "/api/v1/mods"); } catch (e) { err = e; }
  if (err) {
    const m = err.status === 404 || err.network ? "Mod API 不可用（后端未就绪）"
      : "加载失败：" + (err.error ? err.error.message : err.message);
    list.innerHTML = '<span class="field-meta">' + m + '</span>';
    return;
  }
  const arr = (mods && mods.mods) || [];
  if (!arr.length) { list.innerHTML = '<span class="field-meta">（无 Mod）</span>'; return; }
  list.innerHTML = arr.map((m) => {
    const [sLabel, sCls] = MOD_STATUS[m.status] || ["未知", "dim"];
    const bd = (a) => !m.enabled || a === "enable" ? "starting|running".includes(m.status) : a === "disable" ? m.status === "stopping" : "starting|stopping".includes(m.status);
    return '<div class="row mods-row"><span class="mods-label">'
      + m.name + " (" + m.id + ") v" + m.version + "</span>"
      + '<span class="status ' + sCls + '">' + sLabel + "</span>"
      + ["enable", "disable", "restart"].map((a) =>
        '<button class="secondary" data-mod="' + m.id + '" data-action="' + a + '" '
        + (bd(a) ? "" : "disabled") + ">" + MOD_ACTIONS[a] + "</button>").join("")
      + "</div>";
  }).join("");
  list.onclick = (e) => {
    const b = e.target.closest("button[data-action]");
    if (b) modAction(b.dataset.mod, b.dataset.action);
  };
}
async function modAction(modId, action) {
  try {
    const data = await api("POST", "/api/v1/mods/" + encodeURIComponent(modId) + "/" + action, {});
    if (data && data.ok === false) {
      toast("Mod 操作失败：" + (data.error ? data.error.message : "未知"), "err");
    } else {
      toast("Mod 已" + (MOD_ACTIONS[action] || action), "ok");
    }
    renderMods();
  } catch (e) {
    toast("Mod 操作失败：" + (e.error ? e.error.message : e.message), "err");
  }
}

// 同步 LLM/TTS 顶栏徽章（status 拉取也复用，节省一次 renderAll）。
function renderBadges() {
  const s = state.status;
  if (!s) return;
  ["llm","tts"].forEach((k) => {
    const svc = s[k];
    if (!svc) return;
    els[k + "Badge"].textContent = svc.configured
      ? (svc.has_api_key ? "已配置" : "未配密钥") : "未配置";
    els[k + "Badge"].className = "status " + (svc.configured
      ? (svc.has_api_key ? "ok" : "warn") : "err");
  });
  els.uptime.textContent = s.uptime_s + "s";
  if (s.config_path) els.cfgPath.textContent = s.config_path;
  els.epoch.textContent = s.current_epoch;
  els.devModeStatus.textContent = s.dev_mode ? "true" : "false";
  els.devModeStatus.className = s.dev_mode ? "status ok" : "status";
}
// persona 五字段 loop 配置：el id ↔ persona 字段映射。
const PERSONA_FIELDS = [
  ["personaName", "name"], ["personaDesc", "description"],
  ["personaPersona", "personality"], ["personaScenario", "scenario"],
  ["personaFirst", "first"],
];
function renderAll() {
  if (state.capabilities) els.ver.textContent = "v" + state.capabilities.version;
  if (state.status) {
    const s = state.status;
    els.startedAt.textContent = s.started_at || "—";
    els.activeModel.textContent = s.active_model_id || "—";
    els.audio.textContent = s.audio
      ? (s.audio.backend + (s.audio.available ? " ✓" : " (unavailable)")) : "—";
    els.llmBase.textContent = s.llm.base_url || "—";
    els.llmHasKey.textContent = s.llm.has_api_key ? "true" : "false";
    els.llmHasKey.className = s.llm.has_api_key ? "ok" : "err";
    els.ttsBase.textContent = s.tts.base_url || "—";
    els.ttsHasKey.textContent = s.tts.has_api_key ? "true" : "false";
    els.ttsHasKey.className = s.tts.has_api_key ? "ok" : "err";
    renderBadges();
  }
  if (state.settingsView) {
    const sv = state.settingsView;
    if (document.activeElement !== els.llmBaseUrl) els.llmBaseUrl.value = sv.llm.base_url || "";
    if (document.activeElement !== els.llmModel) els.llmModel.value = sv.llm.model || "";
    if (!state.llmKeyTouched) setIf(els.llmApiKeyEnv, sv.llm.has_api_key ? "(已绑定)" : "");
    if (document.activeElement !== els.ttsBaseUrl) els.ttsBaseUrl.value = sv.tts.base_url || "";
    if (document.activeElement !== els.ttsVoice) els.ttsVoice.value = sv.tts.voice || "";
    if (!state.ttsKeyTouched) setIf(els.ttsApiKeyEnv, sv.tts.has_api_key ? "(已绑定)" : "");
    setIf(els.systemPrompt, sv.persona.system_prompt || "");
    const p = sv.persona || {};
    // persona 五字段 loop 化（behavioral-equivalent）。
    PERSONA_FIELDS.forEach(([e, k]) => setIf(els[e], p[k] || ""));
    if (document.activeElement !== els.devMode) els.devMode.checked = !!sv.dev_mode;
    if (sv.dev_mode) {
      els.logsCard.classList.remove("hidden");
      refreshLogs();
    } else {
      els.logsCard.classList.add("hidden");
    }
  }
  renderGroups();
  if (state.models) renderModels();
}
function buildPatch() {
  const sv = state.settingsView || { llm: {}, tts: {}, persona: {} };
  const patch = {};
  // LLM 段
  const llm = {};
  if (els.llmBaseUrl.value !== (sv.llm.base_url || "")) llm.base_url = els.llmBaseUrl.value;
  if (els.llmModel.value !== (sv.llm.model || "")) llm.model = els.llmModel.value;
  if (state.llmKeyTouched) {
    const v = els.llmApiKeyEnv.value.trim();
    if (v === "" || v === "(已绑定)") {
      // 段级清除：api_key_env=null + clear_api_key=true 同段。
      // 旧版顶层 clear_api_key 已废弃（误伤另一 provider）。
      llm.api_key_env = null;
      llm.clear_api_key = true;
    } else {
      llm.api_key_env = v;
    }
  }
  if (Object.keys(llm).length) patch.llm = llm;
  // TTS 段
  const tts = {};
  if (els.ttsBaseUrl.value !== (sv.tts.base_url || "")) tts.base_url = els.ttsBaseUrl.value;
  if (els.ttsVoice.value !== (sv.tts.voice || "")) tts.voice = els.ttsVoice.value;
  if (state.ttsKeyTouched) {
    const v = els.ttsApiKeyEnv.value.trim();
    if (v === "" || v === "(已绑定)") {
      tts.api_key_env = null;
      tts.clear_api_key = true;
    } else {
      tts.api_key_env = v;
    }
  }
  if (Object.keys(tts).length) patch.tts = tts;
  // persona 段
  const persona = {};
  if (els.systemPrompt.value !== (sv.persona.system_prompt || "")) {
    persona.system_prompt = els.systemPrompt.value;
  }
  // 角色卡：空输入 = null（清除字段，与后端「非空才写入」语义一致）。persona 五字段 loop 化。
  PERSONA_FIELDS.forEach(([e, k]) => {
    const el = els[e];
    if (el) {
      const v = el.value.trim();
      if (v !== (sv.persona[k] || "")) persona[k] = v === "" ? null : v;
    }
  });
  if (Object.keys(persona).length) patch.persona = persona;
  patch.dev_mode = els.devMode.checked ? true : false;
  return patch;
}

// D-P0C：apply_status 文案映射（snake_case，需与 dto.rs::ApplyStatus 同步）。
const APPLY_STATUS_TEXT = {
  applied: { toast: "已保存并热重载", status: "已保存于 " },
  restart_required: { toast: "已保存，需重启生效", status: "已保存（需重启）" },
  queued: { toast: "已保存，配置将在下次对话时应用", status: "已保存（排队中）" },
  no_supervisor: { toast: "已保存，配置将在下次重启后生效", status: "已保存（控制平面）" },
};

function applyStatusText(applyStatus) {
  // 容错：未知状态 = 当作 no_supervisor 提示。
  return APPLY_STATUS_TEXT[applyStatus] || APPLY_STATUS_TEXT.no_supervisor;
}

async function save() {
  els.btnSave.disabled = true;
  els.saveStatus.textContent = "保存中…";
  try {
    const patch = buildPatch();
    const resp = await api("PATCH", "/api/v1/settings", patch);
    state.settingsView = resp.settings;
    state.llmKeyTouched = false;
    state.ttsKeyTouched = false;
    // D-P0C：按 `apply_status` 显示不同文案；applied 时刷新让对话区可用于。
    const applyStatus = resp.apply_status || "no_supervisor";
    const text = applyStatusText(applyStatus);
    toast(text.toast, applyStatus === "applied" ? "ok" : "warn");
    els.saveStatus.textContent = text.status + new Date().toLocaleTimeString();
    if (applyStatus === "applied") {
      await loadAll();
    } else {
      // 至少刷新 settings 视图（不刷新 status；supervisor 槽位空）。
      await api("GET", "/api/v1/settings").then((sv) => {
        state.settingsView = sv;
      }).catch(() => {});
    }
  } catch (e) {
    const msg = e.error ? (e.error.code + "：" + e.error.message) : e.message;
    els.saveStatus.textContent = msg;
    toast("保存失败：" + msg, "err");
  } finally {
    els.btnSave.disabled = false;
  }
}

async function reloadForm() {
  state.llmKeyTouched = false;
  state.ttsKeyTouched = false;
  await loadAll();
  toast("已重新载入", "ok");
}

async function clearKeyBinding() {
  if (!confirm("确认清除 LLM 与 TTS 的 api_key_env 绑定？此操作立即写盘。")) return;
  els.saveStatus.textContent = "清除中…";
  try {
    // 段级 clear_api_key：每段独立，避免误伤。
    const patch = {
      llm: { api_key_env: null, clear_api_key: true },
      tts: { api_key_env: null, clear_api_key: true },
    };
    const resp = await api("PATCH", "/api/v1/settings", patch);
    state.settingsView = resp.settings;
    state.llmKeyTouched = false;
    state.ttsKeyTouched = false;
    // D-P0C：apply_status 决定后续动作——applied 直接刷新，restart_required
    // 提示用户需重启（不立即刷新 status，因对话可能仍 503）。
    const applyStatus = resp.apply_status || "no_supervisor";
    const text = applyStatusText(applyStatus);
    toast("已清除密钥绑定（" + text.toast + "）",
      applyStatus === "applied" ? "ok" : "warn");
    await loadAll();
  } catch (e) {
    const msg = e.error ? (e.error.code + "：" + e.error.message) : e.message;
    toast("清除失败：" + msg, "err");
  }
}

// ---- 测试连接 ----
async function testEndpoint(btn, resultEl, path) {
  btn.disabled = true;
  resultEl.textContent = "测试中…";
  resultEl.className = "status";
  try {
    const r = await api("POST", path, {});
    if (r.ok) {
      resultEl.className = "status ok";
      resultEl.textContent = "✓ 成功（" + (r.latency_ms || "?") + "ms）";
    } else {
      resultEl.className = "status err";
      const err = r.error || {};
      resultEl.textContent = "✗ " + (err.code || "fail") + "：" + (err.message || "");
    }
  } catch (e) {
    resultEl.className = "status err";
    resultEl.textContent = "✗ " + (e.message || "请求失败");
  } finally {
    btn.disabled = false;
  }
}
let logsTimer = null;
// 日志级别严重度排序（越高越严重；trace 最低）。用于「级别≥」过滤。
const LOG_LEVEL_RANK = { trace: 0, debug: 1, info: 2, warn: 3, error: 4 };
// 从 /api/v1/logs/levels 拉取已知级别，填充「级别≥」下拉（保留「全部」）。
async function populateLogLevels() {
  if (!els.logLevel || els.logLevel.dataset.loaded === "1") return;
  try {
    const r = await api("GET", "/api/v1/logs/levels");
    const lv = (r && r.levels) || [];
    // 只保留在默认下拉里还没有的级别（trace/debug/info/warn/error，按严重度升序）。
    const known = Object.keys(LOG_LEVEL_RANK);
    const merged = known.filter((k) => lv.includes(k));
    els.logLevel.innerHTML = '<option value="all">全部</option>' +
      merged.map((k) => '<option value="' + k + '">' + k + '</option>').join("");
    els.logLevel.dataset.loaded = "1";
  } catch (_) { /* levels 端点不可用（非 dev_mode 等）：保持仅「全部」 */ }
}
// 解析单行日志的级别 token（形如 "…  INFO target: msg"），无则 null。
function parseLogLevel(line) {
  const m = line.match(/\s(trace|debug|info|warn|error)\s/i);
  return m ? m[1].toLowerCase() : null;
}
// 按「级别≥」过滤显示：selected=all → 全部；否则只显示 rank ≥ 选中级别的行。
function filterLogLines(lines, selected) {
  if (!selected || selected === "all") return { shown: lines, filtered: 0 };
  const min = LOG_LEVEL_RANK[selected];
  if (min === undefined) return { shown: lines, filtered: 0 };
  const shown = lines.filter((l) => {
    const lv = parseLogLevel(l);
    if (lv === null) return true; // 无级别行（如空行/标题）始终保留
    return (LOG_LEVEL_RANK[lv] ?? 2) >= min;
  });
  return { shown, filtered: lines.length - shown.length };
}
async function refreshLogs() {
  if (!state.settingsView || !state.settingsView.dev_mode) {
    els.logsCard.classList.add("hidden");
    return;
  }
  els.logsCard.classList.remove("hidden");
  if (els.logLevel) await populateLogLevels();
  try {
    const r = await api("GET", "/api/v1/logs");
    els.logsFile.textContent = r.file ? "（" + r.file + "）" : "";
    const lines = r.lines || [];
    if (!lines.length) {
      els.logsPre.textContent = "（无日志）";
    } else {
      const sel = els.logLevel ? els.logLevel.value : "all";
      const { shown, filtered } = filterLogLines(lines, sel);
      els.logsPre.textContent = shown.join("\n");
      if (r.truncated) {
        els.logsPre.textContent = "…（已截断，仅显示最近 200 行）\n" + els.logsPre.textContent;
      }
      if (filtered > 0) {
        els.logsPre.textContent += "\n…（已隐藏 " + filtered + " 行，级别≥" + sel + "）";
      }
    }
    els.logsStatus.textContent = "已拉取 " + lines.length + " 行";
    els.logsStatus.className = "status ok";
  } catch (e) {
    if (e.status === 403) {
      els.logsStatus.textContent = "403 — dev_mode 已关闭";
      els.logsStatus.className = "status warn";
      els.logsPre.textContent = "（无权限；dev_mode 关闭）";
    } else {
      els.logsStatus.textContent = "拉取失败：" + (e.error ? e.error.message : e.message);
      els.logsStatus.className = "status err";
    }
  }
}

function startLogsPoll() {
  if (logsTimer) clearInterval(logsTimer);
  logsTimer = setInterval(() => {
    if (state.settingsView && state.settingsView.dev_mode) refreshLogs();
  }, LOGS_POLL_MS);
}
// ---- 事件 ----
els.llmApiKeyEnv.addEventListener("input", () => { state.llmKeyTouched = true; });
els.ttsApiKeyEnv.addEventListener("input", () => { state.ttsKeyTouched = true; });
els.btnSave.addEventListener("click", save);
els.btnReload.addEventListener("click", reloadForm);
els.btnClearKey.addEventListener("click", clearKeyBinding);
els.btnTestLlm.addEventListener("click", () => testEndpoint(els.btnTestLlm, els.llmTestResult, "/api/v1/settings/test/llm"));
els.btnTestTts.addEventListener("click", () => testEndpoint(els.btnTestTts, els.ttsTestResult, "/api/v1/settings/test/tts"));
els.btnRefresh.addEventListener("click", loadAll);
if (els.btnActions) els.btnActions.addEventListener("click", toggleActPanel);
els.btnReloadLogs.addEventListener("click", refreshLogs);
if (els.logLevel) els.logLevel.addEventListener("change", refreshLogs);
// 外观与舖控件绑定（本地状态 + postMessage）。
bindAppearance();
// B1/C2：互动开关 + 自定义背景绑定。
bindStageInteractions();
// A1: 舞台缩放/复位悬浮控制条 → postToStage({type:"stage-zoom",dir})。
if (els.btnZoomIn) els.btnZoomIn.addEventListener("click", () => postToStage({ type: "stage-zoom", dir: "in" }));
if (els.btnZoomOut) els.btnZoomOut.addEventListener("click", () => postToStage({ type: "stage-zoom", dir: "out" }));
if (els.btnResetView) els.btnResetView.addEventListener("click", () => postToStage({ type: "stage-zoom", dir: "reset" }));
// 模型管理按钮绑定。
if (els.btnRefreshModels) els.btnRefreshModels.addEventListener("click", loadModels);

// 设置模态开关事件（topbar ⚙ btnSettings → openSettings；遮罩/Esc/✕ → closeSettings）。
els.btnSettings.addEventListener("click", () => {
  state.settingsOpen = true; openSettings();
});
if (els.btnSettingsClose) els.btnSettingsClose.addEventListener("click", closeSettings);
els.settingsOverlay.addEventListener("click", closeSettings);
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && state.settingsOpen) closeSettings();
});

// ---- 启动 ----
globalThis.__booted = loadAll().then(() => { startLogsPoll(); }); // 暴露启动 promise：行为基线 await 它，避免与启动续体竞态
setInterval(() => {
  api("GET", "/api/v1/app/status").then((s) => {
    state.status = s;
    renderBadges();
  }).catch(() => {});
}, STATUS_POLL_MS);
