/**
 * chat-ui.ts — PC 端聊天面板（spoq-fix：双端对话框，直播/微信风格）
 *
 * 布局：右侧半透明面板（消息流 + 底部输入框 + 发送按钮）。
 * 消息：用户右对齐（主色气泡）、AI 左对齐（灰气泡）、错误红色。
 * 协议：onSend(text) 由调用方接 ws-bridge.sendText；addMessage 由 WS 分发驱动。
 *
 * DOM 结构（注入容器）：
 *   .chat-panel
 *     .chat-messages   （滚动消息列表）
 *     .chat-input-row  （输入框 + 发送按钮）
 */
export interface ChatMessage {
  role: 'user' | 'assistant' | 'error' | 'system';
  content: string;
}

export interface ChatPanel {
  /** 追加一条消息（自动滚动到底部） */
  addMessage(msg: ChatMessage): void;
  /** 连接状态提示（顶部小字：已连接 / 未连接） */
  setConnected(connected: boolean): void;
  /** TTS 播放状态提示（如 "🔊 播放中" / "TTS 空闲"） */
  setTtsStatus(status: string): void;
  /** 销毁面板 DOM（可选） */
  destroy(): void;
}

/** 去除 [emotion] / [emo:xxx] 标签（显示纯净文本） */
function stripTags(text: string): string {
  return text
    .replace(/\[(?:emo:)?[a-zA-Z_]+(?:_\d+)?\]/g, '')
    .replace(/\{.*?\}/g, '')
    .trim();
}

/** 创建聊天面板。onSend 返回 false 表示未连接（输入框给出提示）。 */
export function initChatPanel(
  container: HTMLElement,
  opts: { onSend: (text: string) => boolean },
): ChatPanel {
  const panel = document.createElement('div');
  panel.className = 'chat-panel';
  panel.innerHTML = `
    <div class="chat-header">
      <span class="chat-title">白 · Live2Dai</span>
      <span class="chat-tts-status">TTS 空闲</span>
      <span class="chat-status">未连接</span>
      <button class="chat-settings" type="button" title="设置 / Dashboard">⚙</button>
    </div>
    <div class="chat-messages"></div>
    <div class="chat-input-row">
      <input class="chat-input" type="text" placeholder="输入消息，Enter 发送…" autocomplete="off" />
      <button class="chat-mic" type="button" title="语音输入">🎤</button>
      <button class="chat-send" type="button">发送</button>
    </div>
  `;
  container.appendChild(panel);

  const messagesEl = panel.querySelector<HTMLElement>('.chat-messages')!;
  const inputEl = panel.querySelector<HTMLInputElement>('.chat-input')!;
  const sendBtn = panel.querySelector<HTMLButtonElement>('.chat-send')!;
  const micBtn = panel.querySelector<HTMLButtonElement>('.chat-mic')!;
  const statusEl = panel.querySelector<HTMLElement>('.chat-status')!;
  const ttsStatusEl = panel.querySelector<HTMLElement>('.chat-tts-status')!;

  const addMessage = (msg: ChatMessage): void => {
    const row = document.createElement('div');
    row.className = `chat-msg chat-msg-${msg.role}`;
    const bubble = document.createElement('div');
    bubble.className = 'chat-bubble';
    bubble.textContent = stripTags(msg.content);
    row.appendChild(bubble);
    messagesEl.appendChild(row);
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
    inputEl.value = '';
    inputEl.focus();
  };

  // P0: 浏览器语音输入（Web Speech API，Chrome/Edge 可用）
  const SpeechRecognitionCtor =
    (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
  let recognizing = false;
  let recognition: any = null;
  const startVoice = (): void => {
    if (!SpeechRecognitionCtor) {
      addMessage({ role: 'error', content: '当前浏览器不支持语音输入，请使用 Chrome/Edge' });
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
    };
    recognition.onend = () => {
      recognizing = false;
      micBtn.classList.remove('chat-mic-active');
    };
    recognition.onerror = () => {
      recognizing = false;
      micBtn.classList.remove('chat-mic-active');
      addMessage({ role: 'error', content: '语音识别失败，请重试' });
    };
    recognition.onresult = (event: any) => {
      const text = (event.results?.[0]?.[0]?.transcript || '').trim();
      if (text && opts.onSend(text)) {
        addMessage({ role: 'user', content: text });
      } else if (text) {
        addMessage({ role: 'error', content: '未连接服务器（WS 断开），请稍候重试' });
      }
    };
    recognition.start();
  };
  micBtn.addEventListener('click', startVoice);

  sendBtn.addEventListener('click', doSend);
  inputEl.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') doSend();
  });

  return {
    addMessage,
    setConnected(connected: boolean) {
      statusEl.textContent = connected ? '已连接' : '未连接';
      statusEl.classList.toggle('chat-status-ok', connected);
    },
    setTtsStatus(status: string) {
      ttsStatusEl.textContent = status;
    },
    destroy() {
      panel.remove();
    },
  };
}
