/**
 * chat-ui.ts — PC 端对话面板（毛玻璃 + design token 统一风格）
 *
 * 相对旧版的观感/体验升级（对齐 docs/research/neko-ui-alignment-and-gaps.md）：
 *   - P0-4：样式全部走 `chat.css` + `theme.css` 的 token，主色与设置中心统一
 *   - P1-1：消息渐入、面板滑入、状态胶囊脉冲
 *   - P1-2：欢迎空状态 + 「正在思考」三点指示
 *   - P1-3：SVG 图标替换 emoji（⚙/🎤/🔊）
 *   - P1-8：aria-live 消息区、按钮 aria-label、Enter 发送 + 焦点可见
 *   - P2-2：头像 + 时间戳 + 毛玻璃气泡；外部注入消息带来源角标
 *
 * 协议不变：`initChatPanel(container, { onSend }) → ChatPanel`，
 * 由 main.ts 接 ws-bridge（本模块不认识 WebSocket）。
 */
import { icon } from './icons';
import type { VoiceCapture } from './voice-input';
import './theme.css';
import './chat.css';

export interface ChatMessage {
  role: 'user' | 'assistant' | 'error' | 'system';
  content: string;
  /** 消息来源标签（如「外部接入」）；仅用于展示角标 */
  sourceTag?: string;
}

/** 语音输入的可见状态（P0-2） */
export type VoiceUiState = 'idle' | 'listening' | 'transcribing' | 'interrupted' | 'error';

export interface ChatPanel {
  /** 追加一条消息（自动滚动到底部，自动收起思考指示） */
  addMessage(msg: ChatMessage): void;
  /** 用历史消息整段替换列表（P0-3 回看/切换会话；空列表 → 回到欢迎态） */
  restoreMessages(messages: ChatMessage[]): void;
  /** 连接状态（头部胶囊：已连接 / 未连接） */
  setConnected(connected: boolean): void;
  /** TTS 播放状态（头部胶囊） */
  setTtsStatus(status: string): void;
  /** 「正在思考」指示开关（发送后打开，首句回复到达时自动关闭） */
  setThinking(thinking: boolean): void;
  /**
   * 流式增量缓冲（P1-2）：首个增量创建"正在说…"气泡并缓存文本，
   * 但【不】立即上屏——文字随语音逐句揭示（音画同步），
   * 权威文本由 revealStream 按句上屏、finalizeStream 收尾。
   */
  streamDelta(text: string): void;
  /**
   * 按句揭示权威文本（句子级字幕拼接）：在该轮的同一个气泡里
   * 上屏已开播句子的干净文本——文字与声音同步出现。
   */
  revealStream(text: string): void;
  /** 思维链增量上屏（折叠面板；受「对话内展示思考」开关控制） */
  streamReasoning(text: string): void;
  /** 是否正在流式输出（调用方据此跳过句子级字幕上屏） */
  isStreaming(): boolean;
  /**
   * 结束流式：有权威最终文本（句子级字幕拼接）时用它替换，避免流式残留
   * TTS 过滤前的噪声；无则保留已流出的内容。
   */
  finalizeStream(finalText?: string): void;
  /** 停止按钮可见性：生成 / 播放期间显示，点击回调 onStopRequest */
  setStopVisible(visible: boolean): void;
  /** 语音输入状态（P0-2：聆听 / 识别中 / 已打断，对用户可见） */
  setVoiceState(state: VoiceUiState, detail?: string): void;
  /** 外部接入开关状态（头部胶囊：外部接入中） */
  setExternalGate(enabled: boolean): void;
  /** 清空消息列表并回到欢迎空状态 */
  clear(): void;
  /** 面板根元素（供 history 抽屉等挂载） */
  root(): HTMLElement;
  /** 头部操作区（供 main.ts 追加按钮） */
  headerActions(): HTMLElement | null;
  destroy(): void;
}

/** 去除 [emotion] / {json} 等控制标签，只显示纯净文本 */
function stripTags(text: string): string {
  return text
    .replace(/\[(?:emo:)?[a-zA-Z_]+(?:_\d+)?\]/g, '')
    .replace(/\{.*?\}/g, '')
    .trim();
}

/** HH:MM 时间戳（本地时区，24 小时制） */
function timestamp(): string {
  return new Date().toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });
}

const EMPTY_STATE_HTML = `
  <div class="chat-empty" data-empty>
    ${icon('sparkle')}
    <div class="chat-empty-title">和白打个招呼吧</div>
    <div class="chat-empty-hint">输入文字或按麦克风说话，她会用表情、口型和声音回应你。</div>
    <div class="chat-empty-tips">
      <span>Enter 发送 · 滚轮缩放角色 · 拖拽移动角色</span>
      <span>右上角齿轮可配置模型 / 音色 / 背景 / 外部接入</span>
    </div>
  </div>
`;

/**
 * 创建对话面板。
 * @param opts.onSend 发送回调；返回 false 表示未连接（面板给出错误气泡）
 * @param opts.onOpenSettings 设置按钮回调（缺省时按钮仍渲染但不接线）
 */
export function initChatPanel(
  container: HTMLElement,
  opts: {
    onSend: (text: string) => boolean;
    onOpenSettings?: () => void;
    /** 停止按钮点击回调（打断生成 + 停止播放，由 main.ts 接 wsBridge） */
    onStopRequest?: () => void;
    /** 后端 ASR 采集器（P0-2）；不传或不支持则回落浏览器 Web Speech */
    voice?: VoiceCapture;
  },
): ChatPanel {
  const panel = document.createElement('section');
  panel.className = 'chat-panel';
  panel.setAttribute('aria-label', '对话面板');
  panel.innerHTML = `
    <header class="chat-header">
      <div class="chat-avatar" aria-hidden="true">白</div>
      <div class="chat-headings">
        <div class="chat-title">白 · Live2D-Ai</div>
        <div class="chat-substatus">
          <span class="chat-pill chat-status bad"><span class="dot"></span><span class="chat-status-text">未连接</span></span>
          <span class="chat-pill chat-tts-status hidden"><span class="dot"></span><span class="chat-tts-text">TTS 空闲</span></span>
          <span class="chat-pill chat-voice-status hidden"></span>
          <span class="chat-pill chat-ext-status busy live hidden"><span class="dot"></span>外部接入中</span>
        </div>
      </div>
      <div class="chat-header-actions">
        <button class="chat-icon-btn chat-settings ui-lift" type="button" title="设置中心" aria-label="打开设置中心">${icon('settings')}</button>
      </div>
    </header>

    <div class="chat-messages" role="log" aria-live="polite" aria-label="对话记录">
      ${EMPTY_STATE_HTML}
    </div>

    <div class="chat-thinking" data-thinking aria-hidden="true">
      <span class="chat-thinking-dots"><i></i><i></i><i></i></span>
      <span>白正在思考…</span>
    </div>

    <div class="chat-input-row">
      <input class="chat-input" type="text" placeholder="说点什么…（Enter 发送）" autocomplete="off" aria-label="消息输入框" />
      <button class="chat-mic ui-lift" type="button" title="语音输入" aria-label="语音输入">${icon('mic')}</button>
      <button class="chat-stop ui-lift hidden" type="button" title="停止生成与播放" aria-label="停止生成与播放">${icon('stop')}</button>
      <button class="chat-send ui-lift" type="button" title="发送" aria-label="发送消息">${icon('send')}</button>
    </div>
  `;
  container.appendChild(panel);

  const messagesEl = panel.querySelector<HTMLElement>('.chat-messages')!;
  const inputEl = panel.querySelector<HTMLInputElement>('.chat-input')!;
  const sendBtn = panel.querySelector<HTMLButtonElement>('.chat-send')!;
  const micBtn = panel.querySelector<HTMLButtonElement>('.chat-mic')!;
  const stopBtn = panel.querySelector<HTMLButtonElement>('.chat-stop')!;
  const settingsBtn = panel.querySelector<HTMLButtonElement>('.chat-settings')!;
  const statusPill = panel.querySelector<HTMLElement>('.chat-status')!;
  const statusText = panel.querySelector<HTMLElement>('.chat-status-text')!;
  const ttsPill = panel.querySelector<HTMLElement>('.chat-tts-status')!;
  const ttsText = panel.querySelector<HTMLElement>('.chat-tts-text')!;
  const extPill = panel.querySelector<HTMLElement>('.chat-ext-status')!;
  const voicePill = panel.querySelector<HTMLElement>('.chat-voice-status')!;
  const thinkingEl = panel.querySelector<HTMLElement>('[data-thinking]')!;

  const dropEmptyState = (): void => {
    messagesEl.querySelector('[data-empty]')?.remove();
  };

  const setThinking = (thinking: boolean): void => {
    thinkingEl.classList.toggle('show', thinking);
    thinkingEl.setAttribute('aria-hidden', thinking ? 'false' : 'true');
  };

  const addMessage = (msg: ChatMessage): void => {
    dropEmptyState();
    // 助手/错误消息意味着本轮已有产出 → 收起思考指示
    if (msg.role === 'assistant' || msg.role === 'error') setThinking(false);

    const row = document.createElement('div');
    row.className = `chat-msg chat-msg-${msg.role}`;
    if (msg.sourceTag) row.classList.add('chat-msg-external');

    const bubble = document.createElement('div');
    bubble.className = 'chat-bubble';
    bubble.textContent = stripTags(msg.content);
    row.appendChild(bubble);

    // 系统提示不配元信息行（保持轻量）
    if (msg.role === 'user' || msg.role === 'assistant') {
      const meta = document.createElement('div');
      meta.className = 'chat-meta';
      if (msg.sourceTag) {
        const tag = document.createElement('span');
        tag.className = 'chat-meta-tag';
        tag.textContent = msg.sourceTag;
        meta.appendChild(tag);
      }
      const time = document.createElement('span');
      time.textContent = timestamp();
      meta.appendChild(time);
      row.appendChild(meta);
    }

    messagesEl.appendChild(row);
    messagesEl.scrollTop = messagesEl.scrollHeight;
  };

  /* ---- P1-2 流式上屏（音画同步版）----
   * LLM 增量只进缓冲、不直接上屏；句子级字幕随语音逐句开播时，
   * 由 revealStream 把已开播句子的干净权威文本揭示到同一个气泡——
   * 修复此前「整段文字提前暴露、声音还在后面」的不同步。 */
  let streamBubble: HTMLElement | null = null;
  let streamText = ''; // 原始增量缓冲（TTS 关闭 / 无字幕路径的兜底）
  let revealedText = ''; // 已随语音揭示的权威文本

  /* ---- 思维链展示（reasoning-delta；「对话内展示思考」开关控制，localStorage showThinking） ---- */
  let thinkPanel: HTMLDetailsElement | null = null;
  let thinkBody: HTMLElement | null = null;
  let thinkText = '';

  const thinkingEnabled = (): boolean => localStorage.getItem('showThinking') !== '0';

  const resetThinkPanel = (): void => {
    thinkPanel = null;
    thinkBody = null;
    thinkText = '';
  };

  const streamReasoning = (text: string): void => {
    if (!text || !thinkingEnabled()) return;
    dropEmptyState();
    if (!thinkPanel) {
      const details = document.createElement('details');
      details.className = 'chat-thinking';
      details.open = true; // 首次展开让用户看见在思考；收起状态由用户控制
      const summary = document.createElement('summary');
      summary.textContent = '思考中…';
      const body = document.createElement('div');
      body.className = 'chat-thinking-body';
      details.appendChild(summary);
      details.appendChild(body);
      messagesEl.appendChild(details);
      thinkPanel = details;
      thinkBody = body;
      thinkText = '';
    }
    // 首个正文 token 到达时自动折叠思考（答案才是主角）
    if (streamBubble && thinkPanel.open) thinkPanel.open = false;
    thinkText += text;
    thinkBody!.textContent = stripTags(thinkText);
    messagesEl.scrollTop = messagesEl.scrollHeight;
  };

  /** 确保本轮流式气泡存在（「正在说…」占位 + 流式样式） */
  const ensureStreamBubble = (): HTMLElement => {
    if (!streamBubble) {
      const row = document.createElement('div');
      row.className = 'chat-msg chat-msg-assistant chat-msg-streaming';
      const bubble = document.createElement('div');
      bubble.className = 'chat-bubble';
      row.appendChild(bubble);
      const meta = document.createElement('div');
      meta.className = 'chat-meta';
      const tag = document.createElement('span');
      tag.className = 'chat-stream-caret';
      tag.textContent = '正在说…';
      meta.appendChild(tag);
      row.appendChild(meta);
      messagesEl.appendChild(row);
      streamBubble = bubble;
      streamText = '';
      revealedText = '';
    }
    return streamBubble;
  };

  /** 流式开始/进行中的公共前置：收空态、收思考指示、折叠思维链 */
  const beginStream = (): void => {
    dropEmptyState();
    setThinking(false); // 已经开始出字，思考指示让位
    if (thinkPanel) thinkPanel.open = false; // 正文开始 → 折叠思维链
    ensureStreamBubble();
    messagesEl.scrollTop = messagesEl.scrollHeight;
  };

  const streamDelta = (text: string): void => {
    if (!text) return;
    beginStream();
    streamText += text; // 只缓冲：文字等对应语音开播时再揭示（音画同步）
  };

  const revealStream = (text: string): void => {
    const clean = (text || '').trim();
    if (!clean) return;
    beginStream();
    revealedText = clean;
    const bubble = ensureStreamBubble();
    bubble.textContent = stripTags(clean);
    // 已有正文 → 撤下「正在说…」占位（行内时间戳等 finalize 再补）
    const caret = bubble
      .closest<HTMLElement>('.chat-msg')
      ?.querySelector<HTMLElement>('.chat-stream-caret');
    if (caret) caret.textContent = '';
    messagesEl.scrollTop = messagesEl.scrollHeight;
  };

  const finalizeStream = (finalText?: string): void => {
    if (!streamBubble) return;
    // 优先级：整轮权威文本 > 已揭示文本 > 原始缓冲（TTS 关闭兜底）
    const authoritative = ((finalText || '').trim() || revealedText || streamText).trim();
    streamBubble.textContent = stripTags(authoritative);
    const row = streamBubble.closest<HTMLElement>('.chat-msg');
    if (row) {
      row.classList.remove('chat-msg-streaming');
      const caret = row.querySelector<HTMLElement>('.chat-stream-caret');
      if (caret) caret.textContent = timestamp();
    }
    streamBubble = null;
    streamText = '';
    revealedText = '';
    resetThinkPanel(); // 本轮结束 → 下轮新建思考块
    messagesEl.scrollTop = messagesEl.scrollHeight;
  };

  const doSend = (): void => {
    const text = inputEl.value.trim();
    if (!text) return;
    if (!opts.onSend(text)) {
      addMessage({ role: 'error', content: '未连接服务器（WS 断开），请稍候重试' });
      return;
    }
    addMessage({ role: 'user', content: text });
    setThinking(true);
    inputEl.value = '';
    inputEl.focus();
  };

  /* ---- P0-2 语音输入：优先走后端 ASR（把音频送回后端），不支持时回落浏览器 Web Speech ---- */
  const backendVoice = opts.voice;
  const useBackendVoice = !!backendVoice?.supported();

  // 浏览器语音输入（Web Speech API）—— 仅作降级路径
  const SpeechRecognitionCtor =
    (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
  let recognizing = false;
  let recognition: any = null;
  const startWebSpeech = (): void => {
    if (!SpeechRecognitionCtor) {
      addMessage({
        role: 'error',
        content: '当前环境无法录音（需要 https 或 localhost），也不支持浏览器语音识别',
      });
      return;
    }
    if (recognizing) {
      recognition?.stop();
      return;
    }
    recognition = new SpeechRecognitionCtor();
    recognition.lang = 'zh-CN';
    recognition.interimResults = false;
    recognition.maxAlternatives = 1;
    recognition.onstart = () => {
      recognizing = true;
      micBtn.classList.add('chat-mic-active');
      micBtn.setAttribute('aria-label', '正在聆听，点击停止');
    };
    recognition.onend = () => {
      recognizing = false;
      micBtn.classList.remove('chat-mic-active');
      micBtn.setAttribute('aria-label', '语音输入');
    };
    recognition.onerror = () => {
      recognizing = false;
      micBtn.classList.remove('chat-mic-active');
      addMessage({ role: 'error', content: '语音识别失败，请重试' });
    };
    recognition.onresult = (event: any) => {
      const text = (event.results?.[0]?.[0]?.transcript || '').trim();
      if (!text) return;
      if (opts.onSend(text)) {
        addMessage({ role: 'user', content: text });
        setThinking(true);
      } else {
        addMessage({ role: 'error', content: '未连接服务器（WS 断开），请稍候重试' });
      }
    };
    recognition.start();
  };

  const toggleVoice = (): void => {
    if (!useBackendVoice || !backendVoice) {
      startWebSpeech();
      return;
    }
    if (backendVoice.isListening()) {
      backendVoice.stop(); // 停止 → 送 mic-audio-end → 后端 ASR
      return;
    }
    void backendVoice.start();
  };

  micBtn.addEventListener('click', toggleVoice);
  micBtn.title = useBackendVoice ? '语音输入（后端识别）' : '语音输入（浏览器识别）';
  sendBtn.addEventListener('click', doSend);
  inputEl.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.isComposing) doSend();
  });
  if (opts.onOpenSettings) {
    settingsBtn.addEventListener('click', opts.onOpenSettings);
  // 停止按钮：打断 LLM 流式输出 + 停止 TTS 播放（main.ts 接 wsBridge.sendInterrupt）
  stopBtn.addEventListener('click', () => opts.onStopRequest?.());
  }

  return {
    addMessage,
    restoreMessages(messages: ChatMessage[]) {
      messagesEl.innerHTML = messages.length ? '' : EMPTY_STATE_HTML;
      setThinking(false);
      for (const msg of messages) {
        // 历史消息不重复播动画（整段恢复时逐条 slide-up 会像"刷屏"）
        const row = document.createElement('div');
        row.className = `chat-msg chat-msg-${msg.role} chat-msg-restored`;
        const bubble = document.createElement('div');
        bubble.className = 'chat-bubble';
        bubble.textContent = stripTags(msg.content);
        row.appendChild(bubble);
        messagesEl.appendChild(row);
      }
      messagesEl.scrollTop = messagesEl.scrollHeight;
    },
    setConnected(connected: boolean) {
      statusText.textContent = connected ? '已连接' : '未连接';
      statusPill.className = `chat-pill chat-status ${connected ? 'ok' : 'bad live'}`;
      inputEl.disabled = false; // 断线也允许输入（发送时给出明确提示）
    },
    setTtsStatus(status: string) {
      const speaking = status.includes('播放');
      ttsText.textContent = speaking ? '播放中' : status.replace(/^🔊\s*/, '');
      ttsPill.classList.toggle('hidden', !speaking);
      ttsPill.className = `chat-pill chat-tts-status ${speaking ? 'busy live' : 'hidden'}`;
      if (speaking) setThinking(false);
    },
    setThinking,
    streamDelta,
    streamReasoning,
    isStreaming: () => streamBubble !== null,
    revealStream,
    finalizeStream,
    setStopVisible(visible: boolean) {
      stopBtn.classList.toggle('hidden', !visible);
    },
    setVoiceState(state: VoiceUiState, detail?: string) {
      micBtn.classList.toggle('chat-mic-active', state === 'listening');
      micBtn.classList.toggle('chat-mic-busy', state === 'transcribing');
      micBtn.setAttribute(
        'aria-label',
        state === 'listening' ? '正在聆听，点击停止' : '语音输入',
      );
      const labels: Record<VoiceUiState, string> = {
        idle: '',
        listening: '🎙 聆听中…',
        transcribing: '识别中…',
        interrupted: '已打断',
        error: detail || '语音失败',
      };
      const text = labels[state];
      voicePill.textContent = text;
      voicePill.className = `chat-pill chat-voice-status ${
        text ? (state === 'error' ? 'bad' : 'busy live') : 'hidden'
      }`;
    },
    setExternalGate(enabled: boolean) {
      extPill.classList.toggle('hidden', !enabled);
    },
    clear() {
      messagesEl.innerHTML = EMPTY_STATE_HTML;
      setThinking(false);
      // 清掉可能残留的流式状态（否则后续增量会写进已被移除的孤儿气泡）
      streamBubble = null;
      streamText = '';
      revealedText = '';
      resetThinkPanel();
      stopBtn.classList.add('hidden');
    },
    root() {
      return panel;
    },
    headerActions() {
      return panel.querySelector<HTMLElement>('.chat-header-actions');
    },
    destroy() {
      panel.remove();
    },
  };
}
