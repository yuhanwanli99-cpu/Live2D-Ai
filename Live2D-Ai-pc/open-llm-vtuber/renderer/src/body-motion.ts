/**
 * body-motion.ts — PC 半身身体动作播放器（参数驱动，LiveEvent 优先级）
 *
 * 背景：bai 模型没有 motion3 文件，因此不能用 model.motion() 播放成品动作。
 * 本模块用 Cubism 身体参数做「可立即生效」的半身动作（点头/歪头/摇摆/兴奋/倾听），
 * 全部经 ParamArbiter 的 LiveEvent 源写入，动作结束后 release 回 idle/mood 基线。
 *
 * 为什么参数基线取 0.5：bai.cdi3.json 中的 ParamAngleX/Y/Z、ParamBodyAngleX/Y/Z
 * 是旋转/角度参数，Live2D 常见中性值为 0.5；0 和 1 分别是两个方向的最大偏转。
 */

import { ParamArbiter, ParamSource } from './param-arbiter';
import type { ParameterOverrides } from './types';

export type BodyGesture =
  | 'idle'
  | 'nod'
  | 'tilt'
  | 'sway'
  | 'happy'
  | 'surprise'
  | 'listen'
  | 'excited';

export interface BodyMotionOptions {
  /** 动作保持时长 ms（默认 1800，之后 release 回 idle 基线） */
  holdMs?: number;
  /** 参数过渡时长 ms（默认 220，短促有弹性） */
  fadeMs?: number;
  /** 时间源（测试可注入；默认 Date.now） */
  now?: () => number;
}

interface BodyMotionState {
  gesture: BodyGesture;
  startedAt: number;
  holdMs: number;
}

/** 半身动作参数表：key 与 bai.cdi3.json 实读一致；0.5 为中性
 *  统一幅度策略：base 值本身即明显偏离中性（贴近参数两端），
 *  再经 BodyMotionPlayer.writeParams 的 intensity 缩放（weak 0.7 / mid 1.0 / strong 1.35）。
 */
export const BODY_GESTURE_PARAMS: Record<BodyGesture, ParameterOverrides> = {
  idle: {},
  nod: {
    ParamAngleX: 0.20,
    ParamAngleY: 0.5,
    ParamBodyAngleX: 0.30,
  },
  tilt: {
    ParamAngleZ: 0.18,
    ParamBodyAngleZ: 0.26,
  },
  sway: {
    ParamBodyAngleZ: 0.16,
    ParamBodyAngleY: 0.66,
    ParamAngleZ: 0.30,
  },
  happy: {
    ParamAngleX: 0.24,
    ParamAngleY: 0.68,
    ParamBodyAngleX: 0.28,
    ParamBodyAngleY: 0.72,
    ParamBodyAngleZ: 0.24,
  },
  surprise: {
    ParamAngleY: 0.5,
    ParamAngleX: 0.22,
    ParamBodyAngleX: 0.26,
    ParamBodyAngleY: 0.5,
  },
  listen: {
    ParamAngleY: 0.28,
    ParamBodyAngleY: 0.30,
    ParamAngleZ: 0.20,
  },
  excited: {
    ParamAngleX: 0.20,
    ParamAngleY: 0.72,
    ParamBodyAngleX: 0.22,
    ParamBodyAngleY: 0.78,
    ParamBodyAngleZ: 0.20,
  },
};

/** 归一化动作名（未知/空 → 'idle'） */
export function resolveBodyGesture(gesture: string | undefined | null): BodyGesture {
  if (!gesture) return 'idle';
  const key = String(gesture).trim().toLowerCase();
  return BODY_GESTURE_PARAMS[key as BodyGesture] ? (key as BodyGesture) : 'idle';
}

/**
 * 身体动作播放器。
 *
 * 用法：
 *   const body = new BodyMotionPlayer(arbiter);
 *   app.ticker.add(() => body.update(deltaMs));
 *   body.play('happy');
 */
export class BodyMotionPlayer {
  private readonly arbiter: ParamArbiter;
  private readonly options: Required<BodyMotionOptions>;
  private state: BodyMotionState | null = null;

  constructor(arbiter: ParamArbiter, options: BodyMotionOptions = {}) {
    this.arbiter = arbiter;
    this.options = {
      holdMs: options.holdMs ?? 1800,
      fadeMs: options.fadeMs ?? 220,
      now: options.now ?? Date.now,
    };
  }

  /** 播放一个半身动作。idle/未知动作 = 立即 release；intensity 以 0.5 为中心缩放。 */
  play(gesture: string | undefined | null, intensity = 1): void {
    const key = resolveBodyGesture(gesture);
    if (key === 'idle') {
      this.release();
      return;
    }
    this.writeParams(BODY_GESTURE_PARAMS[key], intensity);
    this.state = {
      gesture: key,
      startedAt: this.options.now(),
      holdMs: this.options.holdMs,
    };
  }

  /** 每帧推进：hold 时间到点则释放参数控制，回退到 idle/mood 基线。 */
  update(deltaMs: number): void {
    if (!this.state) return;
    const dt = Number.isFinite(deltaMs) && deltaMs > 0 ? deltaMs : 0;
    // 用真实时钟推进，避免测试/后台 tab 下 delta 漂移导致动作粘住
    const elapsed = this.options.now() - this.state.startedAt;
    if (elapsed >= this.state.holdMs) {
      this.release();
      return;
    }
    // 保证即使 delta 异常，下次 tick 仍会继续检查
    void dt;
  }

  /** 立即释放 LiveEvent 源对全部参数的控制。 */
  release(): void {
    this.state = null;
    this.arbiter.releaseAll(ParamSource.LiveEvent);
  }

  /** 当前正在播放的动作（无 = null） */
  currentGesture(): BodyGesture | null {
    return this.state?.gesture ?? null;
  }

  private writeParams(params: ParameterOverrides, intensity = 1): void {
    const now = this.options.now();
    // 统一幅度：允许 strong(1.35) 明显放大；weak(0.7) 收缩。轴心 0.5，结果 clamp [0,1]。
    const gain = Math.min(1.5, Math.max(0.4, Number.isFinite(intensity) ? intensity : 1));
    let peak = 0;
    const applied: Record<string, number> = {};
    for (const [paramId, rawValue] of Object.entries(params)) {
      const value = Math.min(1, Math.max(0, 0.5 + (rawValue - 0.5) * gain));
      applied[paramId] = value;
      peak = Math.max(peak, Math.abs(value - 0.5));
      this.arbiter.write({
        source: ParamSource.LiveEvent,
        paramId,
        value,
        mode: 'set',
        fadeMs: this.options.fadeMs,
        timestamp: now,
        valueSpace: 'normalized',
      });
    }
    console.debug(
      `[BodyMotion] gesture=${this.state?.gesture ?? ''} gain=${gain.toFixed(2)} peakDev=${peak.toFixed(2)} params=${JSON.stringify(applied)}`,
    );
  }
}
