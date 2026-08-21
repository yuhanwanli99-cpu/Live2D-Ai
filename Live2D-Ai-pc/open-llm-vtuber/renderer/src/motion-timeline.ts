/**
 * motion-timeline.ts — 按 TTS 播放进度触发的半身动作调度器
 *
 * 背景（my-neuro 同款思路）：括注动作原本随 audio 帧到达即整句一次性触发，
 * 与"这句话说到哪了"无关。后端现在把括注解析成带文本位置的动作帧
 * （actions.motionTimeline，at ∈ [0,1) 为所在音频块的归一化进度），本模块把它
 * 挂到真正播放的那个音频元素上：监听 timeupdate / rAF 轮询 currentTime，
 * 进度到达 at 时触发一次 bodyMotion.play(motion)。
 *
 * 设计约束：
 *  - 不依赖 DOM 全局：时间源抽象成 TimelineTimeSource 接口（HTMLAudioElement
 *    结构兼容），node 环境测试可直接喂 fake 对象并手动调 pump()。
 *  - 打断安全：cancel() 幂等；同一 cue 只触发一次；元素 ended 后自动清理。
 *  - duration 未就绪（NaN/0）时不触发也不丢帧，等元数据加载后继续。
 */

import type { MotionCue } from './types';

/** 时间源最小接口（HTMLAudioElement 结构兼容） */
export interface TimelineTimeSource {
  currentTime: number;
  duration: number;
  ended: boolean;
  addEventListener(type: string, cb: () => void): void;
  removeEventListener(type: string, cb: () => void): void;
}

/** 触发提前量秒数：略微提前，让动作与语音听感对齐 */
const FIRE_EPSILON_S = 0.03;

export class MotionTimelineScheduler {
  private pending: MotionCue[] = [];
  private source: TimelineTimeSource | null = null;
  private rafId: number | null = null;
  private detachListeners: (() => void) | null = null;

  constructor(private readonly fire: (motion: string) => void) {}

  /**
   * 绑定音频元素并开始调度（重复调用先自动取消旧绑定）。
   * 无有效帧时是 no-op——不挂任何监听。
   */
  start(el: TimelineTimeSource, cues: MotionCue[]): void {
    this.cancel();
    const valid = (Array.isArray(cues) ? cues : []).filter(
      (c) =>
        c != null &&
        typeof c.at === 'number' &&
        Number.isFinite(c.at) &&
        c.at >= 0 &&
        typeof c.motion === 'string' &&
        c.motion.length > 0,
    );
    if (valid.length === 0) return;

    this.source = el;
    this.pending = valid.slice().sort((a, b) => a.at - b.at);

    const onTimeUpdate = () => this.pump();
    const onEnded = () => this.cancel();
    el.addEventListener('timeupdate', onTimeUpdate);
    el.addEventListener('ended', onEnded);
    this.detachListeners = () => {
      el.removeEventListener('timeupdate', onTimeUpdate);
      el.removeEventListener('ended', onEnded);
    };

    // rAF 提升触发精度（timeupdate 在部分浏览器仅 ~4Hz）；node/测试环境没有则跳过
    if (typeof requestAnimationFrame === 'function') {
      const tick = () => {
        if (!this.source) return; // 已 cancel
        this.pump();
        if (this.source && this.pending.length > 0) {
          this.rafId = requestAnimationFrame(tick);
        }
      };
      this.rafId = requestAnimationFrame(tick);
    }
  }

  /** 检查播放进度并触发所有已到点的动作帧（幂等，可安全多次调用）。 */
  pump(): void {
    if (!this.source || this.pending.length === 0) return;
    const { currentTime, duration } = this.source;
    if (!(Number.isFinite(duration) && duration > 0)) return;
    while (
      this.pending.length > 0 &&
      currentTime >= this.pending[0].at * duration - FIRE_EPSILON_S
    ) {
      const cue = this.pending.shift()!;
      try {
        this.fire(cue.motion);
      } catch (err) {
        console.warn('[motion-timeline] 动作触发失败（忽略）:', err);
      }
    }
    if (this.pending.length === 0 || this.source.ended) {
      this.cancel();
    }
  }

  /** 取消调度并解绑事件（打断 / 换句 / 播完）。幂等。 */
  cancel(): void {
    if (this.rafId !== null) {
      try {
        cancelAnimationFrame(this.rafId);
      } catch {
        /* 忽略 */
      }
      this.rafId = null;
    }
    if (this.detachListeners) {
      try {
        this.detachListeners();
      } catch {
        /* 忽略 */
      }
      this.detachListeners = null;
    }
    this.source = null;
    this.pending = [];
  }

  /** 是否仍有未触发的动作帧 */
  hasPending(): boolean {
    return this.pending.length > 0;
  }
}
