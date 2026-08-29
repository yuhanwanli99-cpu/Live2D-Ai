"use strict";
// Live2D-Ai 配置面板前端（W8 任务）。
// 数据源：同源 /api/v1/* 端点（capabilities / status / settings / logs / test）。
// 密钥语义：api_key_env 字段只承载「环境变量名」（不接触密钥本身）。

const state = {
  capabilities: null,
  status: null,
  settingsView: null,
  llmKeyTouched: false,
  ttsKeyTouched: false,
};

const $ = (id) => document.getElementById(id);
const els = {
  ver: $("ver"), conn: $("conn"), dot: $("dot"),
  llmBadge: $("llmBadge"), ttsBadge: $("ttsBadge"),
  llmBaseUrl: $("llmBaseUrl"), llmModel: $("llmModel"), llmApiKeyEnv: $("llmApiKeyEnv"),
  ttsBaseUrl: $("ttsBaseUrl"), ttsVoice: $("ttsVoice"), ttsApiKeyEnv: $("ttsApiKeyEnv"),
  systemPrompt: $("systemPrompt"), devMode: $("devMode"),
  btnSave: $("btnSave"), btnReload: $("btnReload"), btnClearKey: $("btnClearKey"),
  btnTestLlm: $("btnTestLlm"), btnTestTts: $("btnTestTts"),
  llmTestResult: $("llmTestResult"), ttsTestResult: $("ttsTestResult"),
  saveStatus: $("saveStatus"),
  cfgPath: $("cfgPath"), startedAt: $("startedAt"), uptime: $("uptime"),
  activeModel: $("activeModel"), audio: $("audio"),
  devModeStatus: $("devModeStatus"), epoch: $("epoch"),
  llmBase: $("llmBase"), llmHasKey: $("llmHasKey"),
  ttsBase: $("ttsBase"), ttsHasKey: $("ttsHasKey"),
  logsCard: $("logsCard"), logsFile: $("logsFile"), logsPre: $("logs"),
  logsStatus: $("logsStatus"), btnReloadLogs: $("btnReloadLogs"),
  btnRefresh: $("btnRefresh"), toast: $("toast"),
};

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

// ---- 加载 ----
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

// 同步 LLM/TTS 顶栏徽章（status 拉取也用，节省一次 renderAll）
function renderBadges() {
  const s = state.status;
  if (!s) return;
  if (s.llm) {
    els.llmBadge.textContent = s.llm.configured
      ? (s.llm.has_api_key ? "已配置" : "未配密钥") : "未配置";
    els.llmBadge.className = "status " + (s.llm.configured
      ? (s.llm.has_api_key ? "ok" : "warn") : "err");
  }
  if (s.tts) {
    els.ttsBadge.textContent = s.tts.configured
      ? (s.tts.has_api_key ? "已配置" : "未配密钥") : "未配置";
    els.ttsBadge.className = "status " + (s.tts.configured
      ? (s.tts.has_api_key ? "ok" : "warn") : "err");
  }
  els.uptime.textContent = s.uptime_s + "s";
  if (s.config_path) els.cfgPath.textContent = s.config_path;
  els.epoch.textContent = s.current_epoch;
  els.devModeStatus.textContent = s.dev_mode ? "true" : "false";
  els.devModeStatus.className = s.dev_mode ? "ok" : "";
}
function renderAll() {
  if (state.capabilities) els.ver.textContent = "v" + state.capabilities.version;
  if (state.status) {
    const s = state.status;
    els.cfgPath.textContent = s.config_path || "—";
    els.startedAt.textContent = s.started_at || "—";
    els.uptime.textContent = s.uptime_s + "s";
    els.activeModel.textContent = s.active_model_id || "—";
    els.audio.textContent = s.audio
      ? (s.audio.backend + (s.audio.available ? " ✓" : " (unavailable)")) : "—";
    els.devModeStatus.textContent = s.dev_mode ? "true" : "false";
    els.devModeStatus.className = s.dev_mode ? "ok" : "";
    els.epoch.textContent = s.current_epoch;
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
    if (!state.llmKeyTouched && document.activeElement !== els.llmApiKeyEnv) {
      els.llmApiKeyEnv.value = sv.llm.has_api_key ? "(已绑定)" : "";
    }
    if (document.activeElement !== els.ttsBaseUrl) els.ttsBaseUrl.value = sv.tts.base_url || "";
    if (document.activeElement !== els.ttsVoice) els.ttsVoice.value = sv.tts.voice || "";
    if (!state.ttsKeyTouched && document.activeElement !== els.ttsApiKeyEnv) {
      els.ttsApiKeyEnv.value = sv.tts.has_api_key ? "(已绑定)" : "";
    }
    if (document.activeElement !== els.systemPrompt) {
      els.systemPrompt.value = sv.persona.system_prompt || "";
    }
    if (document.activeElement !== els.devMode) els.devMode.checked = !!sv.dev_mode;
    if (sv.dev_mode) {
      els.logsCard.classList.remove("hidden");
      refreshLogs();
    } else {
      els.logsCard.classList.add("hidden");
    }
  }
}

// ---- 保存 ----
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
  if (Object.keys(persona).length) patch.persona = persona;
  patch.dev_mode = els.devMode.checked ? true : false;
  return patch;
}

// D-P0C（2026-08-29）：apply_status 文案映射。
// **前端字符串锚点**：后端 [`ApplyStatus`] 序列化是 snake_case
//（"applied" / "restart_required" / "queued" / "no_supervisor"）；
// 改动需同步本表与 `dto.rs::ApplyStatus`。
//
// 三档语义：
// - applied：动态装配 / 热重载已生效 → 可立即对话。
// - restart_required / queued：需重启/稍后生效 → 提示但不报红。
// - no_supervisor：纯控制平面模式（dev-only）→ 提示「不装对话引擎」。
const APPLY_STATUS_TEXT = {
  applied: { toast: "已保存并热重载", status: "已保存于 " },
  restart_required: { toast: "已保存，需重启生效", status: "已保存（需重启）" },
  queued: { toast: "已保存，配置将在下次对话时应用", status: "已保存（排队中）" },
  no_supervisor: { toast: "已保存，配置将在下次重启后生效", status: "已保存（控制平面）" },
};

function applyStatusText(applyStatus) {
  // 容错：未知状态 = 当作 no_supervisor 提示，不让前端崩。
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
    // D-P0C：按 `apply_status` 字段显示不同文案（避免「已热重载」与
    // 「实际 503」错位——旧版无 apply_status 时退回旧文案）。
    const applyStatus = resp.apply_status || "no_supervisor";
    const text = applyStatusText(applyStatus);
    toast(text.toast, applyStatus === "applied" ? "ok" : "warn");
    els.saveStatus.textContent =
      text.status + new Date().toLocaleTimeString();
    // apply_status=applied 时，刷新 status 让对话区立即可用（不一定
    // 要等下一次 poll）；其他状态不强拉——避免误导。
    if (applyStatus === "applied") {
      await loadAll();
    } else {
      // 至少刷新 settings 视图（不刷新 status；supervisor 槽位空。
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
    // 段级 clear_api_key：每段独立，避免误伤（与 v0 顶层标志的差异）。
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

// ---- 日志 ----
let logsTimer = null;
async function refreshLogs() {
  if (!state.settingsView || !state.settingsView.dev_mode) {
    els.logsCard.classList.add("hidden");
    return;
  }
  els.logsCard.classList.remove("hidden");
  try {
    const r = await api("GET", "/api/v1/logs");
    els.logsFile.textContent = r.file ? "（" + r.file + "）" : "";
    const lines = r.lines || [];
    if (!lines.length) {
      els.logsPre.textContent = "（无日志）";
    } else {
      els.logsPre.textContent = lines.join("\n");
      if (r.truncated) {
        els.logsPre.textContent = "…（已截断，仅显示最近 200 行）\n" + els.logsPre.textContent;
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
els.btnReloadLogs.addEventListener("click", refreshLogs);

// ---- 启动 ----
loadAll().then(() => { startLogsPoll(); });
setInterval(() => {
  api("GET", "/api/v1/app/status").then((s) => {
    state.status = s;
    renderBadges();
  }).catch(() => {});
}, STATUS_POLL_MS);
