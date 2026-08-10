/**
 * WebSocket 桥接（RM7）——对齐 contract-pc-render.md §RM7 + architecture-pc-render.md §3.7
 *
 * 消费的消息类型（对齐 types.ts WsMessage）：
 *   "set-model-and-conf" → onModelChange(model_info)           （RM7-01）
 *   "audio"              → lipSync.play(volumes, visemes, slice_length)（RM7-02）
 *                          + emotionConsumer.consume(actions.parameterOverrides)（RM7-03）
 *                          + onSubtitle(display_text)
 *   "emotion"            → emotionConsumer.consume(parameterOverrides)
 *   未知 type            → 忽略（try/catch 兜底，不崩溃，RM7-04）
 *
 * 断线自动重连：指数退避（1s 起 ×2，封顶 30s，RM7-05）；手动 disconnect() 不重连。
 */
import { LipSync, VisemeFrame } from './lip-sync';
import { EmotionConsumer } from './emotion-consumer';
import type { DisplayText, ModelInfo } from './types';

/** 重连参数（RM7-05：断开后 5s 内触发首次重连；指数退避，最大间隔 30s） */
const RECONNECT_BASE_MS = 1000;
const RECONNECT_MAX_MS = 30_000;

export class WsBridge {
  /** 模型切换回调（"set-model-and-conf" → model_info） */
  onModelChange: ((modelInfo: ModelInfo) => void) | null = null;
  /** 字幕回调（"audio" → display_text） */
  onSubtitle: ((text: DisplayText) => void) | null = null;

  private url: string;
  private ws: WebSocket | null = null;
  private emotionConsumer: EmotionConsumer | null = null;
  private lipSync: LipSync | null = null;
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

  /** 是否已建立连接（readyState === WebSocket.OPEN） */
  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === 1; // 1 = WebSocket.OPEN
  }

  /** 设置表情消费者（RM4） */
  setEmotionConsumer(consumer: EmotionConsumer): void {
    this.emotionConsumer = consumer;
  }

  /** 设置口型驱动（RM5） */
  setLipSync(lipSync: LipSync): void {
    this.lipSync = lipSync;
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
        case 'audio': {
          const audio = data as {
            volumes?: number[];
            visemes?: VisemeFrame[];
            slice_length?: number;
            actions?: { parameterOverrides?: Record<string, number> } | null;
            display_text?: DisplayText | null;
          } | null;
          // 口型：volumes 存在时按切片播放（RM7-02）
          if (audio && Array.isArray(audio.volumes)) {
            this.lipSync?.play(audio.volumes, audio.visemes ?? [], audio.slice_length ?? 20);
          }
          // 表情：actions.parameterOverrides（RM7-03）
          if (audio?.actions?.parameterOverrides) {
            this.emotionConsumer?.consume(audio.actions.parameterOverrides);
          }
          // 字幕
          if (audio?.display_text) {
            this.onSubtitle?.(audio.display_text);
          }
          break;
        }
        case 'emotion': {
          const emotion = data as { parameterOverrides?: Record<string, number> } | null;
          if (emotion?.parameterOverrides) {
            this.emotionConsumer?.consume(emotion.parameterOverrides);
          }
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
