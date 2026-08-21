/**
 * choreography.ts — 编排导演层：时间轴关键帧驱动的"大幅度"半身动作
 *
 * 背景：BodyMotionPlayer 是单姿态参数手势（一个 pose + hold），幅度小、无过程感。
 * 本模块把动作升级为【多参数 × 时间轴关键帧】序列：每个关键帧在指定时刻写入
 * 一组参数目标值（经 ParamArbiter 的 fade 平滑过渡），形成连续的肢体运动轨迹，
 * 幅度贴近参数极限（0.05~0.95），并保证结束帧回到中性姿态。
 *
 * 触发路径与旧手势完全一致（括注/motionLabel/live-event → play(name)）：
 *   - 名称带 "choreo:" 前缀或命中 CHOREOGRAPHY_LIBRARY → 编排动作；
 *   - 否则回落 BodyMotionPlayer 单姿态手势（完全向后兼容）。
 * BodyActionPlayer 是两者的组合门面：上层（ws-bridge/live-event/main ticker）零改动。
 */

import { ParamArbiter, ParamSource } from './param-arbiter';
import { BodyMotionPlayer } from './body-motion';
import type { ParameterOverrides } from './types';

/** 编排关键帧：at=相对开始毫秒；fadeMs=过渡时长；params=该帧参数目标（0..1，0.5 中性） */
export interface Keyframe {
  at: number;
  fadeMs?: number;
  params: ParameterOverrides;
}

/** 中性姿态（角度类参数 0.5 为中心，bai.cdi3 实读约定） */
const NEUTRAL: ParameterOverrides = {
  ParamAngleX: 0.5,
  ParamAngleY: 0.5,
  ParamAngleZ: 0.5,
  ParamBodyAngleX: 0.5,
  ParamBodyAngleY: 0.5,
  ParamBodyAngleZ: 0.5,
};

/**
 * 编排动作库：key = 动作名；值 = 关键帧序列（按 at 升序，最后一帧回中性）。
 * 幅度刻意贴近参数两端（大幅度）；物理系统会让头发/饰件跟随摆动。
 */
export const CHOREOGRAPHY_LIBRARY: Record<string, Keyframe[]> = {
  // 大幅挥手：身体左右大幅摇摆两轮 + 头部随动（无手臂参数模型的挥手等价表达）
  wave_big: [
    { at: 0,    fadeMs: 160, params: { ParamBodyAngleZ: 0.18, ParamAngleZ: 0.32, ParamAngleY: 0.58 } },
    { at: 340,  fadeMs: 180, params: { ParamBodyAngleZ: 0.82, ParamAngleZ: 0.68 } },
    { at: 700,  fadeMs: 180, params: { ParamBodyAngleZ: 0.18, ParamAngleZ: 0.32 } },
    { at: 1060, fadeMs: 180, params: { ParamBodyAngleZ: 0.82, ParamAngleZ: 0.68 } },
    { at: 1420, fadeMs: 260, params: NEUTRAL },
  ],
  // 鞠躬：深低头 + 上身前倾（AngleX 大幅下压），停顿后缓起
  bow: [
    { at: 0,    fadeMs: 300, params: { ParamAngleX: 0.88, ParamAngleY: 0.36, ParamBodyAngleX: 0.78 } },
    { at: 700,  fadeMs: 240, params: { ParamAngleX: 0.88, ParamAngleY: 0.36, ParamBodyAngleX: 0.78 } },
    { at: 1300, fadeMs: 420, params: NEUTRAL },
  ],
  // 开心蹦跳：身体上下弹三下（BodyAngleY 快速往复）+ 头部雀跃
  bounce_happy: [
    { at: 0,    fadeMs: 110, params: { ParamBodyAngleY: 0.62, ParamAngleY: 0.66 } },
    { at: 200,  fadeMs: 110, params: { ParamBodyAngleY: 0.40, ParamAngleY: 0.44 } },
    { at: 400,  fadeMs: 110, params: { ParamBodyAngleY: 0.64, ParamAngleY: 0.68 } },
    { at: 600,  fadeMs: 110, params: { ParamBodyAngleY: 0.38, ParamAngleY: 0.42 } },
    { at: 800,  fadeMs: 110, params: { ParamBodyAngleY: 0.63, ParamAngleY: 0.67 } },
    { at: 1050, fadeMs: 300, params: NEUTRAL },
  ],
  // 用力摇头：头部左右大幅快速三连
  shake_no: [
    { at: 0,   fadeMs: 100, params: { ParamAngleX: 0.16 } },
    { at: 190, fadeMs: 100, params: { ParamAngleX: 0.84 } },
    { at: 380, fadeMs: 100, params: { ParamAngleX: 0.16 } },
    { at: 570, fadeMs: 100, params: { ParamAngleX: 0.84 } },
    { at: 760, fadeMs: 100, params: { ParamAngleX: 0.30 } },
    { at: 980, fadeMs: 260, params: NEUTRAL },
  ],
  // 凑近倾听：上身前倾贴近镜头感，保持后缓缓退回
  lean_in: [
    { at: 0,    fadeMs: 420, params: { ParamAngleY: 0.72, ParamBodyAngleX: 0.40, ParamAngleX: 0.36 } },
    { at: 1100, fadeMs: 300, params: { ParamAngleY: 0.74, ParamBodyAngleX: 0.38 } },
    { at: 1900, fadeMs: 480, params: NEUTRAL },
  ],
  // 左右张望：头部大幅转左-右各一次
  look_around: [
    { at: 0,   fadeMs: 320, params: { ParamAngleX: 0.14, ParamBodyAngleX: 0.34 } },
    { at: 900, fadeMs: 320, params: { ParamAngleX: 0.86, ParamBodyAngleX: 0.66 } },
    { at: 1750, fadeMs: 400, params: NEUTRAL },
  ],
};

/** 解析动作名：返回编排动作 id 或 null（null = 走旧单姿态手势） */
export function resolveChoreography(name: string | undefined | null): string | null {
  if (!name) return null;
  const key = name.startsWith('choreo:') ? name.slice(7) : name;
  return CHOREOGRAPHY_LIBRARY[key] ? key : null;
}

/**
 * 编排播放器：按真实时钟推进关键帧，逐帧写入 Arbiter（LiveEvent 源，fade 过滤平滑）。
 * 打断安全：cancel()/新 play() 立即清态；播完自动回中性并释放。
 */
export class ChoreographyPlayer {
  private readonly arbiter: ParamArbiter;
  private readonly now: () => number;
  private startedAt = 0;
  private pending: Keyframe[] = [];
  private intensity = 1;
  private activeId: string | null = null;
  private endAt = Infinity;

  constructor(arbiter: ParamArbiter, opts: { now?: () => number } = {}) {
    this.arbiter = arbiter;
    this.now = opts.now ?? Date.now;
  }

  /** 播放一段编排；未知动作返回 false（调用方可回落手势）。intensity 缩放幅度。 */
  play(name: string | undefined | null, intensity = 1): boolean {
    const id = resolveChoreography(name);
    if (!id) return false;
    this.cancel();
    const frames = CHOREOGRAPHY_LIBRARY[id];
    this.activeId = id;
    this.intensity = Math.min(1.2, Math.max(0.4, intensity || 1));
    this.pending = frames.map((f) => ({ ...f, params: { ...f.params } }));
    this.startedAt = this.now();
    const last = this.pending[this.pending.length - 1];
    this.endAt = last.at + (last.fadeMs ?? 150) + 60;
    this.pump();
    return true;
  }

  /** 每帧推进：写出已到点的关键帧；全部写完且过尾帧 → 回中性释放。 */
  update(_deltaMs: number): void {
    this.pump();
  }

  cancel(): void {
    this.pending = [];
    this.activeId = null;
    this.endAt = Infinity;
  }

  isPlaying(): boolean {
    return this.activeId !== null;
  }

  currentId(): string | null {
    return this.activeId;
  }

  private pump(): void {
    if (this.activeId === null) return;
    const elapsed = this.now() - this.startedAt;
    while (this.pending.length > 0 && this.pending[0].at <= elapsed) {
      const kf = this.pending.shift()!;
      this.write(kf);
    }
    if (this.pending.length === 0 && elapsed >= this.endAt) {
      const id = this.activeId;
      this.cancel();
      this.arbiter.releaseAll(ParamSource.LiveEvent); // 回 idle/mood 基线
      void id;
    }
  }

  private write(kf: Keyframe): void {
    const ts = this.now();
    const isNeutralFrame = kf.params === NEUTRAL || Object.keys(kf.params).length === 0;
    for (const [paramId, raw] of Object.entries(isNeutralFrame ? NEUTRAL : kf.params)) {
      const value = isNeutralFrame ? raw : this.scale(raw);
      this.arbiter.write({
        source: ParamSource.LiveEvent,
        paramId,
        value: Math.min(1, Math.max(0, value)),
        mode: 'set',
        fadeMs: kf.fadeMs ?? 140,
        timestamp: ts,
      });
    }
  }

  /** 幅度缩放：以 0.5 为中心放大/收缩，保持中性不动点 */
  private scale(v: number): number {
    return 0.5 + (v - 0.5) * this.intensity;
  }
}

/**
 * 组合门面：编排优先，其余回落单姿态手势。接口与 BodyMotionPlayer 一致，
 * 因此 ws-bridge / live-event / main ticker 无需感知差异。
 */
export interface BodyActionPlayerOptions {
  /** 单姿态手势层选项（透传 BodyMotionPlayer） */
  gestures?: { holdMs?: number; fadeMs?: number; now?: () => number };
  /** 时钟（编排层与手势层共享，测试注入） */
  now?: () => number;
}

export class BodyActionPlayer {
  private readonly gestures: BodyMotionPlayer;
  private readonly choreo: ChoreographyPlayer;

  constructor(arbiter: ParamArbiter, options: BodyActionPlayerOptions = {}) {
    this.gestures = new BodyMotionPlayer(arbiter, options.gestures ?? {});
    this.choreo = new ChoreographyPlayer(arbiter, { now: options.now });
  }

  play(name: string | undefined | null): void {
    if (this.choreo.play(name)) {
      this.gestures.release(); // 编排接管期间清掉残留单姿态
      return;
    }
    this.gestures.play(name);
  }

  update(deltaMs: number): void {
    this.choreo.update(deltaMs);
    this.gestures.update(deltaMs);
  }

  release(): void {
    this.choreo.cancel();
    this.gestures.release();
  }

  currentGesture(): string | null {
    return this.choreo.currentId() ? 'choreo:' + this.choreo.currentId() : this.gestures.currentGesture();
  }
}