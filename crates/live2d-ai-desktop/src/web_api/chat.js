"use strict";
// Live2D-Ai 对话区前端（D5-W2 任务；P1WS-2 升级重连/订阅/ping 协议对齐）。
//
// 数据源：
// - POST /api/v1/chat          发送用户消息（{text}，200/429/503）
// - POST /api/v1/chat/stop     停止当前 turn（幂等）
// - WS  /ws/state              接收 subscribe_ack + 业务事件
//
// WS 事件（来自 web_api::ws::app_event_to_ws_frame；P1WS-2 升级）：
// - {type:"subscribe_ack",  data:{topics:[...]}}     服务端首帧（契约 §4.4）
// - {type:"turn_state",     data:{epoch, status: "completed"|"failed"}}
// - {type:"runtime_status", data:{event:"voice_started"|"voice_ended"|"new_epoch"|"shutdown_ready", epoch?}}
// - {type:"text_delta",     data:{epoch, text, ts_ms?}}        LLM 流式片段
// - {type:"text_delta",     data:{epoch, completed: bool}}     完成锚点
// - {type:"action_state",   data:{epoch, kind, action?}}       P1WS-2 新增
// - {type:"pong",           data:{}}                            文本 pong
// - {type:"error",          data:{code, message}}               服务端 error 帧
//
// 重连（P1WS-2 对齐契约 §4.5）：
// - 指数退避 1s → 2s → 4s → 8s → 上限 30s；
// - 收到 runtime_status.event="shutdown_ready" 后**不**重连；
// - ping 15s（契约 §4.5；服务端当前不主动检测，文本 ping 服务端会回
//   文本 pong，**不**作 idle 心跳——因 server 端 actor 暂不 read
//   客户端帧，契约 partial）。

(function () {
  const chatEls = {
    input: document.getElementById("chat-input"),
    send: document.getElementById("chat-send"),
    stop: document.getElementById("chat-stop"),
    messages: document.getElementById("chat-messages"),
    empty: document.getElementById("chat-empty"),
    status: document.getElementById("chat-status"),
    hint: document.getElementById("chat-hint"),
    wsBadge: document.getElementById("wsBadge"),
  };

  // 当前 assistant 消息（流式进行中）。
  const chatState = {
    currentEpoch: null,        // 当前 turn 的 epoch（用于按 epoch 分组 text_delta）
    currentBubble: null,       // 当前 assistant <div class="chat-msg assistant">
    streaming: false,          // 是否有 turn 在跑
    sendInFlight: false,       // POST /api/v1/chat 是否在飞
    ws: null,                  // WebSocket 实例
    wsBackoffMs: 1000,         // P1WS-2：当前退避（1s → 2s → 4s → 8s → 30s 上限；契约 §4.5）
    wsTimer: null,
    reconnecting: false,
    subscribed: false,         // P1WS-2：服务端已发 subscribe_ack 后置 true
    noReconnect: false,        // P1WS-2：收到 shutdown_ready 后置 true，连接关闭不再重连
  };

  // ---- DOM helpers ----
  function appendUserBubble(text) {
    if (chatEls.empty) chatEls.empty.remove();
    const el = document.createElement("div");
    el.className = "chat-msg user";
    const meta = document.createElement("div");
    meta.className = "meta";
    meta.textContent = "你 · " + nowHms();
    const body = document.createElement("div");
    body.className = "body";
    body.textContent = text;
    el.appendChild(meta);
    el.appendChild(body);
    chatEls.messages.appendChild(el);
    scrollMessagesToEnd();
  }

  function beginAssistantBubble(epoch) {
    if (chatEls.empty) chatEls.empty.remove();
    const el = document.createElement("div");
    el.className = "chat-msg assistant streaming";
    el.dataset.epoch = String(epoch == null ? "" : epoch);
    const meta = document.createElement("div");
    meta.className = "meta";
    meta.textContent = "助手 · epoch " + (epoch == null ? "—" : epoch) + " · " + nowHms();
    const body = document.createElement("div");
    body.className = "body";
    body.textContent = "";
    el.appendChild(meta);
    el.appendChild(body);
    chatEls.messages.appendChild(el);
    chatState.currentBubble = el;
    chatState.currentEpoch = epoch;
    scrollMessagesToEnd();
  }

  function appendAssistantDelta(text) {
    // P1WS-1：后端 text_delta **现在**带真实 text payload
    // （`{"type":"text_delta","data":{"epoch":N,"ts_ms":T,"text":"..."}}`）。
    // 这是 LLM 流式正文片段的逐字追加；按字累加到当前气泡 body。
    if (!text) return;
    if (!chatState.currentBubble) {
      // 兜底：若 text_delta 先于 turn_state / voice_started 到达
      // （极端网络序），先开一个空气泡承接；epoch 走 payload.epoch。
      beginAssistantBubble(null);
    }
    const body = chatState.currentBubble.querySelector(".body");
    body.textContent += text;
    scrollMessagesToEnd();
  }

  function finalizeAssistantBubble(success) {
    if (!chatState.currentBubble) return;
    chatState.currentBubble.classList.remove("streaming");
    if (!success) chatState.currentBubble.classList.add("failed");
    const body = chatState.currentBubble.querySelector(".body");
    if (!body.textContent) {
      // 没有任何 payload 到达 → 留一个占位提示，便于 UI 区分"空响应 vs 未生成"。
      body.textContent = success ? "（空响应）" : "（生成失败）";
    }
    chatState.currentBubble = null;
    chatState.currentEpoch = null;
  }

  function scrollMessagesToEnd() {
    chatEls.messages.scrollTop = chatEls.messages.scrollHeight;
  }

  function nowHms() {
    const d = new Date();
    const p = (n) => (n < 10 ? "0" + n : "" + n);
    return p(d.getHours()) + ":" + p(d.getMinutes()) + ":" + p(d.getSeconds());
  }

  function setChatStatus(label, kind) {
    chatEls.status.textContent = label;
    chatEls.status.className = "chat-status status " + (kind || "");
  }

  function setHint(text, kind) {
    chatEls.hint.textContent = text;
    chatEls.hint.className = "status " + (kind || "");
  }

  function setStreaming(on) {
    chatState.streaming = !!on;
    chatEls.send.disabled = !!on;
    chatEls.stop.disabled = !on;
    chatEls.input.disabled = !!on;
  }

  // ---- 发送 / 停止 ----
  async function sendChat() {
    const text = (chatEls.input.value || "").trim();
    if (!text) return;
    if (chatState.streaming) {
      setHint("生成中…请先停止当前轮", "warn");
      return;
    }
    appendUserBubble(text);
    chatEls.input.value = "";
    setStreaming(true);
    setChatStatus("生成中", "warn");
    setHint("已发送，等待核心链路…");
    chatState.sendInFlight = true;
    try {
      const resp = await fetch("/api/v1/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text: text }),
      });
      const bodyText = await resp.text();
      let data = null;
      try { data = bodyText ? JSON.parse(bodyText) : null; } catch (_) {}
      if (resp.status === 200) {
        const epoch = (data && data.epoch) || null;
        beginAssistantBubble(epoch);
        setHint("已接受 (epoch " + (epoch == null ? "—" : epoch) + ")");
      } else if (resp.status === 429) {
        setChatStatus("忙", "warn");
        setHint("忙，请稍候（429）", "warn");
        appendUserBubble("⚠ " + ((data && data.error && data.error.message) || "核心链路忙，请稍候"));
        setStreaming(false);
      } else if (resp.status === 503) {
        setChatStatus("未启动", "err");
        setHint("核心链路未启动（503）", "err");
        appendUserBubble("⚠ " + ((data && data.error && data.error.message) || "核心链路未启动，请先在 live2d-ai.toml 配 LLM/TTS 后重启"));
        setStreaming(false);
      } else {
        const msg = (data && data.error && data.error.message) || (bodyText || ("HTTP " + resp.status));
        setChatStatus("错误", "err");
        setHint(msg, "err");
        appendUserBubble("⚠ " + msg);
        setStreaming(false);
      }
    } catch (e) {
      setChatStatus("网络错误", "err");
      setHint("网络错误：" + e, "err");
      setStreaming(false);
    } finally {
      chatState.sendInFlight = false;
    }
  }

  async function stopChat() {
    setHint("请求停止…");
    try {
      await fetch("/api/v1/chat/stop", { method: "POST" });
      setChatStatus("已请求停止", "warn");
    } catch (e) {
      setHint("停止请求失败：" + e, "err");
    }
  }

  // ---- WebSocket ----
  function wsUrl() {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    return proto + "//" + location.host + "/ws/state";
  }

  function setWsBadge(label, kind) {
    chatEls.wsBadge.textContent = label;
    chatEls.wsBadge.className = "status " + (kind || "");
  }

  function handleWsMessage(ev) {
    let data = null;
    try { data = JSON.parse(ev.data); } catch (_) { return; }
    const type = data && data.type;
    const payload = (data && data.data) || {};
    if (type === "subscribe_ack") {
      // P1WS-2 契约 §4.4：服务端首帧。topics 是客户端发的订阅列表 echo。
      chatState.subscribed = true;
      setWsBadge("已订阅", "ok");
      setHint("WS 已就绪（topics: " + (Array.isArray(payload.topics) ? payload.topics.length : 0) + "）");
      return;
    }
    if (type === "turn_state") {
      const epoch = payload.epoch;
      const status = payload.status;
      // 仅当完成的 epoch 与当前流式气泡匹配时收口。
      if (chatState.currentBubble && (chatState.currentEpoch == null || chatState.currentEpoch === epoch)) {
        finalizeAssistantBubble(status === "completed");
        setStreaming(false);
        setChatStatus(status === "completed" ? "已完成" : "失败", status === "completed" ? "ok" : "err");
        setHint("turn " + (epoch == null ? "?" : epoch) + " " + status);
      } else {
        setChatStatus(status === "completed" ? "已完成" : "失败", status === "completed" ? "ok" : "err");
      }
    } else if (type === "text_delta") {
      // P1WS-1：后端 text_delta 双形态——
      //   1) {epoch, text, ts_ms}  → LLM 流式正文片段（**新**）
      //   2) {epoch, completed}    → RootAudit(GenerationFinished) 代理的完成锚点
      // 两者按 epoch 独立处理：先追加 text（若有），再判 completed 收口。
      if (typeof payload.text === "string" && payload.text.length > 0) {
        appendAssistantDelta(payload.text);
      }
      if (payload.completed) {
        if (chatState.currentBubble && (chatState.currentEpoch == null || chatState.currentEpoch === payload.epoch)) {
          finalizeAssistantBubble(true);
          setStreaming(false);
          setChatStatus("已完成", "ok");
          setHint("text_delta 完成 (epoch " + (payload.epoch == null ? "?" : payload.epoch) + ")");
        }
      }
    } else if (type === "runtime_status") {
      const evt = payload.event;
      if (evt === "voice_started") {
        setChatStatus("播放中", "ok");
        if (!chatState.currentBubble) beginAssistantBubble(payload.epoch);
      } else if (evt === "voice_ended") {
        setChatStatus("播放结束", "");
      } else if (evt === "new_epoch") {
        // 新一轮 → 开新 assistant 气泡（如果用户已发送但 WS 还没收到 turn_state 之前）。
        if (!chatState.currentBubble) beginAssistantBubble(payload.epoch);
      } else if (evt === "shutdown_ready") {
        // P1WS-2 契约 §4.5：shutdown_ready 后**不**重连。
        setChatStatus("收口", "warn");
        setStreaming(false);
        if (chatState.currentBubble) finalizeAssistantBubble(false);
        chatState.noReconnect = true;
        if (chatState.wsTimer) { clearTimeout(chatState.wsTimer); chatState.wsTimer = null; }
        setHint("server 已进入收口，WS 不再重连");
      }
    } else if (type === "action_state") {
      // P1WS-2 新增事件：模型动作表演。
      // P1 占位：仅记录到 hint，便于后续接入 cap-aware UI（不冒泡 UI）。
      setHint("动作 " + (payload.kind || "?") + " " + (payload.action && payload.action.action ? payload.action.action : ""));
    } else if (type === "pong") {
      // P1WS-2 文本 pong（契约 §4.5）；不做事，UI 不变。
      setHint("pong (seq=" + (data.seq || "?") + ")");
    } else if (type === "error") {
      // P1WS-2：服务端 error 帧（契约 §5 风格）。
      setChatStatus("错误", "err");
      setHint("server error: " + (payload.code || "?") + " " + (payload.message || ""), "err");
    }
  }

  function scheduleReconnect() {
    // P1WS-2 契约 §4.5：
    //   - 收到 shutdown_ready 后**不**重连（noReconnect = true）。
    //   - 退避 1s → 2s → 4s → 8s → 上限 30s。
    if (chatState.noReconnect) {
      setWsBadge("已收口", "");
      return;
    }
    if (chatState.reconnecting) return;
    chatState.reconnecting = true;
    const delay = chatState.wsBackoffMs;
    setWsBadge("重连中 " + Math.round(delay / 1000) + "s", "warn");
    chatState.wsTimer = setTimeout(() => {
      chatState.reconnecting = false;
      // 退避：1s → 2s → 4s → 8s → 上限 30s。
      chatState.wsBackoffMs = Math.min(chatState.wsBackoffMs * 2, 30000);
      connectWs();
    }, delay);
  }

  function connectWs() {
    if (chatState.wsTimer) { clearTimeout(chatState.wsTimer); chatState.wsTimer = null; }
    // P1WS-2：shutdown_ready 后不再发起新连接。
    if (chatState.noReconnect) {
      setWsBadge("已收口", "");
      return;
    }
    let ws;
    try {
      ws = new WebSocket(wsUrl());
    } catch (e) {
      setWsBadge("WS 失败", "err");
      scheduleReconnect();
      return;
    }
    chatState.ws = ws;
    chatState.subscribed = false;
    setWsBadge("连接中", "warn");
    ws.onopen = () => {
      setWsBadge("已连接", "ok");
      // P1WS-2 退避重置：下次断开时从 1s 起。
      chatState.wsBackoffMs = 1000;
      // P1WS-2 契约 §4.4：连接建立后**立即**发 subscribe 帧（首帧）。
      // topics v1 = 空（"全部接受"语义；runtime.* 透传）。
      try {
        ws.send(JSON.stringify({ type: "subscribe", topics: [] }));
      } catch (e) {
        // send 失败 → 走 onclose 路径
      }
      // 重连成功拉一次 status，确保 epoch/运行态对齐。
      fetch("/api/v1/app/status").then((r) => r.json()).then((s) => {
        if (s && typeof s.current_epoch !== "undefined") {
          const ep = document.getElementById("epoch");
          if (ep) ep.textContent = s.current_epoch;
        }
      }).catch(() => {});
    };
    ws.onmessage = handleWsMessage;
    ws.onerror = () => { setWsBadge("错误", "err"); };
    ws.onclose = () => {
      setWsBadge("已断开", "err");
      chatState.ws = null;
      chatState.subscribed = false;
      // P1WS-2：仅在非 shutdown_ready 时重连。
      if (!chatState.noReconnect) {
        scheduleReconnect();
      }
    };
  }

  // ---- 输入行为 ----
  chatEls.input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendChat();
    }
  });
  chatEls.send.addEventListener("click", sendChat);
  chatEls.stop.addEventListener("click", stopChat);

  // ---- 启动 ----
  connectWs();
  setChatStatus("空闲", "");

  // 暴露给 app.js / 测试（供调试 / 集成检查使用）。
  // P1WS-2：增加 reconnect backoff / noReconnect / subscribed API 便于测试。
  window.Live2DAiChat = {
    send: sendChat,
    stop: stopChat,
    state: chatState,
    scheduleReconnect: scheduleReconnect,
    backoffSeconds: function () { return Math.round(chatState.wsBackoffMs / 1000); },
  };
})();
