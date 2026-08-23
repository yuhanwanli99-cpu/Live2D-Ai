/**
 * WebSocket 桥接（RM7）——对齐 contract-pc-render.md §RM7 + architecture-pc-render.md §3.7
 *
 * 消费的消息类型（对齐 types.ts WsMessage）：
 *   "set-model-and-conf" → onModelChange(model_info)           （RM7-01）
 *   "audio"              → lipSync.play(volumes, visemes, slice_length)（RM7-02）
 *                          + emotionConsumer.consume(actions.parameterOverrides)（RM7-03）
 *                          + onSubtitle(display_text)
 *                          + 半身动作：actions.motionTimeline 存在时挂播放进度调度器
 *                            （motion-timeline.ts，播放到对应位置才 bodyMotion.play），
 *                            否则退回旧行为（到达即播 actions.motion）
 *   "subtitle"           → onSubtitle(display_text)（LLM 句子切出后先于 audio 上屏）
 *   "emotion"            → emotionConsumer.consumeFrame(frame)（emotion+strength+overrides）
 *   未知 type            → 忽略（try/catch 兜底，不崩溃，RM7-04）
 *
 * 断线自动重连：指数退避（1s 起 ×2，封顶 30s，RM7-05）；手动 disconnect() 不重连。
 */
import { LipSync, VisemeFrame } from './lip-sync';
import { EmotionConsumer } from './emotion-consumer';
import { BodyActionPlayer } from './choreography';
import { MotionTimelineScheduler } from './motion-timeline';
import type { HistoryEntry, HistoryMessage } from './history';
import type { DisplayText, EmotionMessage, ModelInfo, MotionCue } from './types';
import type { LiveEventPayload } from './live-event';

/** 重连参数（RM7-05：断开后 5s 内触发首次重连；指数退避，最大间隔 30s） */
const RECONNECT_BASE_MS = 1000;
const RECONNECT_MAX_MS = 30_000;

/** WAV 播放队列项：音频元素 + 该块的括注动作时间轴 + 句首标记（字幕同步用） */
interface QueuedAudio {
  el: HTMLAudioElement;
  timeline: MotionCue[];
  startsSentence: boolean;
}

export class WsBridge {
  /** 模型切换回调（"set-model-and-conf" → model_info） */
  onModelChange: ((modelInfo: ModelInfo) => void) | null = null;
  /** 字幕回调（"audio" → display_text） */
  onSubtitle: ((text: DisplayText) => void) | null = null;
  /**
   * 外部文字注入回调（"external-input" → 后端把外部程序发来的那句话回显给前端，
   * 让用户看到"角色为什么突然开口"）。见 src/open_llm_vtuber/external_input.py。
   */
  onExternalInput: ((payload: { text: string; source: string }) => void) | null = null;
  /** 语音识别结果上屏（空文本 = 没听清，前端提示重说） */
  onTranscription: ((text: string) => void) | null = null;
  /**
   * 后端错误回调（`{"type":"error","message":...}`）。
   * 典型来源：未配置 API Key（占位 LLM 的配置指引）、LLM/TTS 上游失败。
   * 此前前端完全忽略该消息类型，用户只看到"角色不说话"（见调研 P1-6）。
   */
  onError: ((message: string) => void) | null = null;
  /** 历史消息回灌（`history-data`：连接时自动恢复上文，或用户切换会话后） */
  onHistoryData:
    | ((payload: { history_uid: string; messages: HistoryMessage[] }) => void)
    | null = null;
  /** 历史会话列表（`history-list`） */
  onHistoryList: ((histories: HistoryEntry[]) => void) | null = null;
  /** 新会话已创建（`new-history-created`） */
  onHistoryCreated: ((historyUid: string) => void) | null = null;
  /** 历史已删除（`history-deleted`） */
  onHistoryDeleted:
    | ((payload: { history_uid: string; success: boolean }) => void)
    | null = null;
  /** token 级增量文本（P1-2 流式上屏；`text-delta`） */
  onTextDelta: ((text: string) => void) | null = null;
  /** 思维链增量（DeepSeek thinking；`reasoning-delta`）——渲染与否由前端开关决定 */
  onReasoningDelta: ((text: string) => void) | null = null;
  /**
   * 控制消息（`{"type":"control","text":...}`）。
   * 后端一直在发（`start-mic` / `interrupt`（VAD 打断）/ `mic-audio-end` /
   * `conversation-chain-end`），但前端此前**完全忽略** control —— 这就是
   * 「语音链路与 VAD 打断对用户不可见」的根因（调研 P0-2）。
   */
  onControl: ((text: string) => void) | null = null;

  /** 最近上屏过的字幕文本（环形小窗口）；用于跳过 audio.display_text 的重复上屏 */
  private _recentSubtitleTexts: string[] = [];
  /** 连接状态回调（onopen/onclose，聊天面板状态指示用） */
  onConnectionChange: ((connected: boolean) => void) | null = null;
  /** TTS 播放状态回调（播放中/空闲） */
  onTtsStatus: ((status: string) => void) | null = null;
  /** emotion 帧转发（PLAN-V3 P1：soullink 表演引擎意图注入；legacy 模式无接收者） */
  onEmotionFrame: ((emotion: string, strength: number) => void) | null = null;
  /** 括注 cue 到点转发（动作名 → 表演重音意图） */
  onCue: ((motion: string) => void) | null = null;
  /** 音频片转发（volumes+sliceLength → 引擎口型拉模型） */
  onAudioSlices: ((volumes: number[], sliceLengthMs?: number) => void) | null = null;
  /**
   * 句首音频块开播回调（字幕-声音同步）：每句第一个音频块真正开始播放时触发。
   * 聊天面板用它把已积累的干净句子文本收尾上屏——对话与声音同步出现；
   * 静默帧（无音频可播）也会立即触发一次（对话提早出）。
   */
  onSentenceSpoken: (() => void) | null = null;
  /** 直播/弹幕事件回调（后端 POST /api/live/event 广播后触发） */
  onLiveEvent: ((payload: LiveEventPayload) => void) | null = null;

  private url: string;
  private ws: WebSocket | null = null;
  private emotionConsumer: EmotionConsumer | null = null;
  private bodyMotion: BodyActionPlayer | null = null;
  private lipSync: LipSync | null = null;
  /**
   * 括注动作帧调度器（my-neuro 式）：actions.motionTimeline 存在时，把动作挂到
   * 正在播放的音频元素上，播放到对应进度才 bodyMotion.play —— 替代旧的
   * "audio 帧到达即整句一次性触发"。
   */
  private motionScheduler = new MotionTimelineScheduler((motion) => {
    this.bodyMotion?.play(motion);
    this.onCue?.(motion);
  });
  private reconnectAttempts = 0;
  private reconnectTimer: number | null = null;
  private manuallyClosed = false;

  constructor(url: string) {
    this.url = url;
  }

  /** 建立 WebSocket 连接并挂接消息分发 */
  connect(): void {
    // 手动关闭标记复位；清掉可能残留的重连定时器
    this.manuallyClosed = false;
    this._clearReconnectTimer();

    let ws: WebSocket;
    try {
      ws = new WebSocket(this.url);
    } catch (err) {
      console.error('[ws-bridge] WebSocket 创建失败:', err);
      this._scheduleReconnect();
      return;
    }

    this.ws = ws;
    ws.onmessage = (event: MessageEvent) => this._handleRaw(event);
    ws.onclose = () => {
      this.onConnectionChange?.(false);
      if (this.manuallyClosed) return;
      this.ws = null;
      this._scheduleReconnect();
    };
    ws.onerror = () => {
      // 连接错误后通常伴随 onclose → 重连由 onclose 统一调度
      console.warn('[ws-bridge] WebSocket 连接错误');
    };
    ws.onopen = () => {
      this.reconnectAttempts = 0; // 连接成功：重置退避计数
      this.onConnectionChange?.(true);
    };
  }

  /** 主动断开：不触发重连 */
  disconnect(): void {
    this.manuallyClosed = true;
    this._clearReconnectTimer();
    if (this.ws) {
      try {
        this.ws.close();
      } catch (err) {
        console.warn('[ws-bridge] 关闭连接异常（忽略）:', err);
      }
      this.ws = null;
    }
  }

  /**
   * 发送聊天文本（spoq-fix：PC 端对话框）。
   * 后端 WS 协议：{"type":"text-input","text":...}（proxy_handler / ws_utils 处理）。
   * 未连接时静默丢弃（聊天面板应提示未连接）。
   * @returns 是否已发送（false = 未连接）
   */
  sendText(text: string): boolean {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return false;
    }
    this.ws.send(JSON.stringify({ type: 'text-input', text }));
    return true;
  }

  /** 是否已建立连接（readyState === WebSocket.OPEN） */
  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === 1; // 1 = WebSocket.OPEN
  }

  /** 发送一条控制消息（未连接时静默丢弃，返回是否已发送） */
  private send(payload: Record<string, unknown>): boolean {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return false;
    try {
      this.ws.send(JSON.stringify(payload));
      return true;
    } catch (err) {
      console.warn('[ws-bridge] 发送失败（忽略）:', err);
      return false;
    }
  }

  // ── 聊天历史（P0-3；复用后端既有 WS 协议，未新增消息类型） ──

  /** 请求历史会话列表 → `history-list` */
  fetchHistoryList(): boolean {
    return this.send({ type: 'fetch-history-list' });
  }

  /** 切换到指定历史（同时切换 LLM 记忆） → `history-data` */
  fetchAndSetHistory(historyUid: string): boolean {
    if (!historyUid) return false;
    return this.send({ type: 'fetch-and-set-history', history_uid: historyUid });
  }

  /** 开始新会话 → `new-history-created` */
  createNewHistory(): boolean {
    return this.send({ type: 'create-new-history' });
  }

  /** 删除指定历史 → `history-deleted` */
  deleteHistory(historyUid: string): boolean {
    if (!historyUid) return false;
    return this.send({ type: 'delete-history', history_uid: historyUid });
  }

  // ── 语音输入（P0-2；把音频送回后端 ASR，复用既有协议） ──

  /**
   * 上行一片麦克风音频（16kHz 单声道 Float32 → 普通数组）。
   * 后端 `_handle_audio_data` 累积到会话缓冲，`mic-audio-end` 时交给 ASR。
   */
  sendMicAudio(samples: Float32Array | number[]): boolean {
    const audio = Array.from(samples);
    if (audio.length === 0) return false;
    return this.send({ type: 'mic-audio-data', audio });
  }

  /** 录音结束 → 后端跑 ASR 并开始对话 */
  sendMicAudioEnd(): boolean {
    return this.send({ type: 'mic-audio-end' });
  }

  /** 主动打断当前发言（用户插话；后端取消本轮任务） */
  sendInterrupt(heardResponse = ''): boolean {
    this.stopAudio();
    return this.send({ type: 'interrupt-signal', text: heardResponse });
  }

  /** 设置表情消费者（RM4） */
  setEmotionConsumer(consumer: EmotionConsumer): void {
    this.emotionConsumer = consumer;
  }

  /** 设置半身动作播放器（对话 actions.motion → bodyMotion.play） */
  setBodyMotion(player: BodyActionPlayer): void {
    this.bodyMotion = player;
  }

  /** 设置口型驱动（RM5） */
  setLipSync(lipSync: LipSync): void {
    this.lipSync = lipSync;
  }

  // ── 音频播放（spoq-fix：电脑没声音根因——前端只做口型/表情/字幕，从不播放 audio 帧） ──
  /** WAV 播放队列 + 顺序播放（入队即创建 Audio 并 preload，减少句间 gap） */
  private audioQueue: QueuedAudio[] = [];
  private audioPlaying = false;
  private pendingPlaybackComplete = false;
  /** 正在播放的元素（打断时需要真的把它按停，光清队列不够） */
  private currentAudio: HTMLAudioElement | null = null;

  private playAudioQueue(): void {
    if (this.audioPlaying || this.audioQueue.length === 0) {
      if (!this.audioPlaying && this.audioQueue.length === 0) {
        this.onTtsStatus?.('TTS 空闲');
        this.trySendPlaybackComplete();
      }
      return;
    }
    const item = this.audioQueue.shift()!;
    const el = item.el;
    this.audioPlaying = true;
    this.currentAudio = el;
    if (item.startsSentence) this.onSentenceSpoken?.();
    this.onTtsStatus?.('🔊 播放中');
    console.debug(`[LATENCY] play_start_ms=${performance.now().toFixed(0)}`);
    try {
      // 提前预加载下一段，减少句间解码等待
      if (this.audioQueue.length > 0) {
        this.audioQueue[0].el.preload = 'auto';
      }
      el.onended = () => {
        this.motionScheduler.cancel();
        this.audioPlaying = false;
        this.playAudioQueue();
      };
      el.onerror = () => {
        console.warn('[ws-bridge] 音频播放失败（跳过）');
        this.motionScheduler.cancel();
        this.audioPlaying = false;
        this.playAudioQueue();
      };
      void el
        .play()
        .then(() => {
          // 括注动作帧：挂到真实播放进度上，播放到对应位置才触发半身动作
          if (item.timeline.length > 0) {
            this.motionScheduler.start(el, item.timeline);
          }
        })
        .catch((err) => {
          // 浏览器自动播放策略拦截：首次交互后才会放行（用户点过聊天框后 OK）
          console.warn('[ws-bridge] 音频自动播放被拦截（浏览器策略）:', err?.message ?? err);
          this.audioPlaying = false;
          this.playAudioQueue();
        });
    } catch (err) {
      console.warn('[ws-bridge] 音频构造失败（跳过）:', err);
      this.audioPlaying = false;
      this.playAudioQueue();
    }
  }

  /** 入队并尝试播放（base64 WAV；timeline=括注动作帧；startsSentence=句首块标记） */
  private enqueueAudio(
    base64Wav: string,
    timeline: MotionCue[] = [],
    startsSentence = false,
  ): void {
    const dataUri = `data:audio/wav;base64,${base64Wav}`;
    const el = new Audio(dataUri);
    el.preload = 'auto';
    this.audioQueue.push({ el, timeline, startsSentence });
    this.playAudioQueue();
  }

  /**
   * 立刻停止播放并清空队列（VAD 打断 / 用户插话）。
   * 打断语义要求"声音马上停"，否则用户会听到自己被角色压过去。
   */
  stopAudio(): void {
    this.motionScheduler.cancel(); // 打断时同步取消未触发的括注动作帧
    this.audioQueue.length = 0;
    if (this.currentAudio) {
      try {
        this.currentAudio.pause();
        this.currentAudio.currentTime = 0;
        // 清掉回调，避免 onended/onerror 再把队列泵起来
        this.currentAudio.onended = null;
        this.currentAudio.onerror = null;
      } catch (err) {
        console.warn('[ws-bridge] 停止音频失败（忽略）:', err);
      }
      this.currentAudio = null;
    }
    this.audioPlaying = false;
    this.pendingPlaybackComplete = false;
    this.onTtsStatus?.('TTS 空闲');
  }

  /** 后端已通知合成完毕且本地队列已空 → 通知后端可结束本轮 */
  private trySendPlaybackComplete(): void {
    if (!this.pendingPlaybackComplete || this.audioPlaying || this.audioQueue.length > 0) {
      return;
    }
    this.pendingPlaybackComplete = false;
    console.debug(`[LATENCY] playback_complete_ms=${performance.now().toFixed(0)}`);
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    try {
      this.ws.send(JSON.stringify({ type: 'frontend-playback-complete' }));
    } catch (err) {
      console.warn('[ws-bridge] 发送 playback-complete 失败（忽略）:', err);
    }
  }

  /** 记录最近上屏字幕，窗口上限 20 条（防并发 TTS 乱序导致重复） */
  private _rememberSubtitleText(text: string): void {
    this._recentSubtitleTexts.push(text);
    if (this._recentSubtitleTexts.length > 20) {
      this._recentSubtitleTexts.shift();
    }
  }

  /** 原始消息入口：JSON 解析（try/catch 兜底，解析失败只记日志不崩溃） */
  private _handleRaw(event: MessageEvent): void {
    let data: unknown;
    try {
      data = typeof event.data === 'string' ? JSON.parse(event.data) : event.data;
    } catch (err) {
      console.error('[ws-bridge] 消息 JSON 解析失败（忽略）:', err);
      return;
    }
    this._dispatch(data);
  }

  /** 消息分发（type 判断；未知 type / 结构异常一律 try/catch 兜底，RM7-04） */
  private _dispatch(data: unknown): void {
    try {
      const msg = data as { type?: string } | null;
      switch (msg?.type) {
        case 'set-model-and-conf': {
          const info = (data as { model_info?: ModelInfo } | null)?.model_info;
          if (info) this.onModelChange?.(info);
          break;
        }
        case 'subtitle': {
          const subtitle = data as { text?: string; name?: string } | null;
          if (subtitle && typeof subtitle.text === 'string') {
            this._rememberSubtitleText(subtitle.text);
            this.onSubtitle?.({ text: subtitle.text, name: subtitle.name ?? '' });
          }
          break;
        }
        case 'audio': {
          const audio = data as {
            audio?: string | null;
            volumes?: number[];
            visemes?: VisemeFrame[];
            slice_length?: number;
            actions?: {
              parameterOverrides?: Record<string, number>;
              motion?: string | null;
              motionTimeline?: MotionCue[] | null;
            } | null;
            display_text?: DisplayText | null;
            sentence_start?: boolean;
          } | null;
          // 口型：volumes 存在时按切片播放（RM7-02）；同时转发给表演引擎拉模型
          if (audio && Array.isArray(audio.volumes)) {
            this.lipSync?.play(audio.volumes, audio.visemes ?? [], audio.slice_length ?? 20);
            this.onAudioSlices?.(audio.volumes, audio.slice_length);
          }
          // 表情：actions.parameterOverrides（RM7-03）
          if (audio?.actions?.parameterOverrides) {
            this.emotionConsumer?.consume(audio.actions.parameterOverrides);
          }
          // 半身动作：括注动作帧时间轴优先（随播放进度触发）；无时间轴时退回
          // 旧的一次性触发（导演/主脑产出 motionLabel → bodyMotion.play）。
          // 有时间轴时刻意忽略一次性 motion，避免同一动作双重触发。
          const motionTimeline = Array.isArray(audio?.actions?.motionTimeline)
            ? (audio.actions.motionTimeline as MotionCue[]).filter(Boolean)
            : [];
          if (motionTimeline.length === 0 && audio?.actions?.motion) {
            this.bodyMotion?.play(audio.actions.motion);
          }
          // 字幕（subtitle 消息已先上屏同一句时跳过，避免聊天面板重复显示）
          if (audio?.display_text) {
            const text = audio.display_text.text;
            if (!this._recentSubtitleTexts.includes(text)) {
              this._rememberSubtitleText(text);
              this.onSubtitle?.(audio.display_text);
            }
          }
          // 音频播放（spoq-fix：audio 帧 base64 WAV → 播放；None 为静默帧跳过）
          if (typeof audio?.audio === 'string' && audio.audio.length > 0) {
            console.debug(`[LATENCY] audio_received_ms=${performance.now().toFixed(0)}`);
            this.enqueueAudio(audio.audio, motionTimeline, audio.sentence_start === true);
          } else if (audio?.sentence_start === true) {
            // 静默帧没有可"开播"的音频：立即触发句首回调（对话提早出，不等声音）
            this.onSentenceSpoken?.();
          }
          break;
        }
        case 'emotion': {
          const frame = data as EmotionMessage | null;
          if (!frame) break;
          const hasOverrides =
            frame.parameterOverrides && Object.keys(frame.parameterOverrides).length > 0;
          if (frame.emotion || hasOverrides) {
            this.emotionConsumer?.consumeFrame(frame);
            this.onEmotionFrame?.(frame.emotion ?? 'neutral', frame.strength ?? 2);
          }
          break;
        }
        case 'backend-synth-complete': {
          this.pendingPlaybackComplete = true;
          this.trySendPlaybackComplete();
          break;
        }
        case 'external-input': {
          const injected = data as { text?: string; source?: string } | null;
          if (injected && typeof injected.text === 'string' && injected.text) {
            this.onExternalInput?.({
              text: injected.text,
              source: injected.source || 'external',
            });
          }
          break;
        }
        case 'user-input-transcription': {
          // 后端 ASR 结果：把用户说的话上屏（此前这条消息被完全忽略，
          // 说出去的话在聊天里「消失」，只有 AI 干接话）
          const t = (data as { text?: string } | null)?.text;
          this.onTranscription?.(typeof t === 'string' ? t : '');
          break;
        }
        case 'live-event': {
          const payload = (data as { event?: LiveEventPayload } | null)?.event;
          if (payload && typeof payload === 'object') {
            this.onLiveEvent?.(payload);
          }
          break;
        }
        case 'error': {
          const err = data as { message?: string } | null;
          const message = (err?.message || '').trim();
          if (message) this.onError?.(message);
          break;
        }
        case 'history-data': {
          const payload = data as
            | { history_uid?: string; messages?: HistoryMessage[] }
            | null;
          this.onHistoryData?.({
            history_uid: payload?.history_uid || '',
            messages: Array.isArray(payload?.messages) ? payload!.messages! : [],
          });
          break;
        }
        case 'history-list': {
          const payload = data as { histories?: HistoryEntry[] } | null;
          this.onHistoryList?.(
            Array.isArray(payload?.histories) ? payload!.histories! : [],
          );
          break;
        }
        case 'new-history-created': {
          const payload = data as { history_uid?: string } | null;
          if (payload?.history_uid) this.onHistoryCreated?.(payload.history_uid);
          break;
        }
        case 'history-deleted': {
          const payload = data as { history_uid?: string; success?: boolean } | null;
          this.onHistoryDeleted?.({
            history_uid: payload?.history_uid || '',
            success: !!payload?.success,
          });
          break;
        }
        case 'reasoning-delta': {
          const rp = data as { text?: string } | null;
          if (rp?.text) this.onReasoningDelta?.(rp.text);
          break;
        }
        case 'text-delta': {
          const payload = data as { text?: string } | null;
          if (payload?.text) this.onTextDelta?.(payload.text);
          break;
        }
        case 'control': {
          const payload = data as { text?: string } | null;
          const command = (payload?.text || '').trim();
          if (!command) break;
          // VAD 打断：立刻停掉正在播放/排队的音频，否则"她还在说、你已经插话"
          if (command === 'interrupt') this.stopAudio();
          this.onControl?.(command);
          break;
        }
        default:
          // 未知 type：忽略（RM7-04）
          break;
      }
    } catch (err) {
      console.error('[ws-bridge] 消息分发异常（忽略）:', err);
    }
  }

  /** 指数退避重连调度（1s → 2s → 4s → ... → 封顶 30s） */
  private _scheduleReconnect(): void {
    if (this.reconnectTimer !== null || this.manuallyClosed) return;
    const delay = Math.min(
      RECONNECT_BASE_MS * Math.pow(2, this.reconnectAttempts),
      RECONNECT_MAX_MS,
    );
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  private _clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }
}
