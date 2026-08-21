/**
 * live-event.ts — PC 通用直播/弹幕事件消费者。
 *
 * 事件来源（后端 POST /api/live/event 广播）可以是任意直播平台桥或本地脚本：
 *   { "type": "danmaku", "text": "...", "user": "...", "motion": "happy", "emotion": "joy" }
 *
 * 本模块只做前端「呈现」：身体动作 + 表情 + 可选 toast/字幕回调。
 * 若要弹幕文字进入 LLM 对话，应走既有的 /api/external/chat 链路，不要在这里重复实现。
 */

import { EmotionConsumer } from './emotion-consumer';
import { BodyActionPlayer } from './choreography';

export type LiveEventKind = 'danmaku' | 'gift' | 'follow' | 'like' | 'command' | string;

export interface LiveEventPayload {
  type?: LiveEventKind;
  event?: LiveEventKind;
  text?: string;
  user?: string;
  /** 半身动作名（见 body-motion.ts BodyGesture） */
  motion?: string;
  /** 表情 key（joy/sad/angry/surprise/fear/disgust/neutral/love） */
  emotion?: string;
  /** 表情强度 1-3 */
  strength?: number;
  /** 直接覆盖 Live2D 参数（可选） */
  parameterOverrides?: Record<string, number>;
}

export interface LiveEventOptions {
  /** 表情保持时长 ms（默认 2600，之后 release） */
  emotionHoldMs?: number;
  /** 时间源（测试可注入） */
  now?: () => number;
}

/** 兼容后端旧字段：event/type 任一都可以表达事件类型 */
export function normalizeLiveEvent(payload: LiveEventPayload): LiveEventPayload {
  if (!payload || typeof payload !== 'object') return {};
  return {
    ...payload,
    type: payload.type || payload.event || 'command',
  };
}

export class LiveEventController {
  private readonly emotion: EmotionConsumer;
  private readonly body: BodyActionPlayer;
  private readonly options: Required<LiveEventOptions>;
  private emotionTimer: ReturnType<typeof setTimeout> | null = null;
  /** 事件文本回调（用于 toast/字幕，不做 LLM 注入） */
  onEventText: ((payload: LiveEventPayload) => void) | null = null;

  constructor(
    emotion: EmotionConsumer,
    body: BodyActionPlayer,
    options: LiveEventOptions = {},
  ) {
    this.emotion = emotion;
    this.body = body;
    this.options = {
      emotionHoldMs: options.emotionHoldMs ?? 2600,
      now: options.now ?? Date.now,
    };
  }

  /** 处理一条直播/弹幕事件。所有字段可选；未知类型不崩溃。 */
  handle(raw: LiveEventPayload): void {
    const payload = normalizeLiveEvent(raw);

    if (payload.motion) {
      this.body.play(payload.motion);
    }

    if (payload.emotion) {
      this.clearEmotionTimer();
      this.emotion.applyEmotion(payload.emotion, payload.strength ?? 2, 260);
      this.emotionTimer = setTimeout(() => {
        this.emotionTimer = null;
        this.emotion.release();
      }, this.options.emotionHoldMs);
    }

    if (payload.parameterOverrides && Object.keys(payload.parameterOverrides).length > 0) {
      this.emotion.consume(payload.parameterOverrides, 260);
    }

    if (payload.text) {
      this.onEventText?.(payload);
    }
  }

  /** 释放所有由事件驱动且仍在保持中的表情/身体动作。 */
  release(): void {
    this.clearEmotionTimer();
    this.emotion.release();
    this.body.release();
  }

  /** 销毁控制器（清定时器）。 */
  dispose(): void {
    this.release();
    this.onEventText = null;
  }

  private clearEmotionTimer(): void {
    if (this.emotionTimer !== null) {
      clearTimeout(this.emotionTimer);
      this.emotionTimer = null;
    }
  }
}
