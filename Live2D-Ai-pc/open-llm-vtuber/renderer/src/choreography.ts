/**
 * choreography.ts — 头部 / 面部动作引擎（用户终验定版）
 *
 * 定版口径（2026-08-25 用户终验）：
 *  - 只保留六个通用「头 / 脸」动作 + release：
 *        nod        点头（AngleY 上下 2 拍）
 *        shake_no   摇头（AngleX 左右 2 拍）
 *        tilt       歪头（AngleZ 单侧保持后回中）
 *        look_around 左右张望（先眼动，再头随动）
 *        listen     倾听（轻微 AngleZ/AngleY + 稳定视线 + 微眉，不写身体）
 *        surprise   惊讶（睁眼 + 挑眉 + 张嘴 + 头部轻微后仰）
 *  - 彻底禁止任何 ParamBodyAngle* / 肩 / 胯 / 腿通道；动作系统不再声称
 *    「半身 / body」动作，只有头部与面部参数。
 *  - 正确轴约定：AngleX=左右转头(yaw)、AngleY=上下点头(pitch)、AngleZ=侧倾(roll)。
 *  - 所有中性 / 默认由 profile（min/max/defaultValue）映射，不假设眼嘴 0.5；
 *    release 回 profile 默认值（由 ParamArbiter 的 release 语义完成）。
 *  - 单一 active 动作调度器：同一时刻只播一个动作；同动作重复 / 冷却期忽略；
 *    更高优先级安全打断；相同 / 更低优先级进唯一 pending 槽；播放结束 / release 回中。
 *
 * 优先级：dev 显式 / 用户命令(Live) > timeline cue(Cue) > 情绪 fallback(Fallback) > idle(Idle)。
 */

import { ParamArbiter, ParamSource } from './param-arbiter';
import { ActionPriority, sanitizeOverrides } from './action-policy';
import { semanticToNormalized, type ParameterRange } from './halfbody-profile';

export { ActionPriority } from './action-policy';

/** 受支持的六个头部 / 面部动作 + release（released 用 'idle' / '' 表达）。 */
export const SUPPORTED_ACTIONS: readonly string[] = [
  'nod',
  'shake_no',
  'tilt',
  'look_around',
  'listen',
  'surprise',
];

export function isSupportedAction(id: string | null | undefined): boolean {
  return SUPPORTED_ACTIONS.includes(String(id ?? '').trim().toLowerCase());
}

/** 语义关键帧：at=相对开始 ms；delta=各参数相对 profile 默认的语义意图，-1..1（0=默认）。 */
export interface Keyframe {
  at: number;
  fadeMs?: number;
  /**
   * 语义幅度：-1..1，0 = 该参数的 profile 默认值（不是 0.5！）。
   * 空对象表示「本帧不再写参数」，随后在 endAt 触发 release 回默认。
   */
  delta: Record<string, number>;
}

/**
 * 六动作模板库（全部只含白名单头 / 脸参数；末帧为空 delta 表示回默认）。
 */
export const CHOREOGRAPHY_LIBRARY: Record<string, Keyframe[]> = {
  // 点头：AngleY（pitch，上下点头）往返两拍
  nod: [
    { at: 0, fadeMs: 120, delta: { ParamAngleY: -0.55 } },
    { at: 240, fadeMs: 120, delta: { ParamAngleY: 0.45 } },
    { at: 500, fadeMs: 120, delta: { ParamAngleY: -0.5 } },
    { at: 760, fadeMs: 120, delta: { ParamAngleY: 0.4 } },
    { at: 1000, fadeMs: 320, delta: {} },
  ],
  // 摇头：AngleX（yaw，左右转头）往返两拍
  shake_no: [
    { at: 0, fadeMs: 90, delta: { ParamAngleX: -0.55 } },
    { at: 180, fadeMs: 90, delta: { ParamAngleX: 0.55 } },
    { at: 360, fadeMs: 90, delta: { ParamAngleX: -0.5 } },
    { at: 540, fadeMs: 90, delta: { ParamAngleX: 0.5 } },
    { at: 700, fadeMs: 300, delta: {} },
  ],
  // 歪头：AngleZ（roll，侧倾）单侧保持后回中
  tilt: [
    { at: 0, fadeMs: 220, delta: { ParamAngleZ: -0.5 } },
    { at: 650, fadeMs: 220, delta: { ParamAngleZ: -0.4 } },
    { at: 1100, fadeMs: 400, delta: {} },
  ],
  // 左右张望：先眼睛（EyeBallX）看向一侧，头部（AngleX）随动，再看另一侧，最后回中
  look_around: [
    { at: 0, fadeMs: 140, delta: { ParamEyeBallX: -0.55 } },
    { at: 200, fadeMs: 260, delta: { ParamEyeBallX: -0.35, ParamAngleX: -0.5 } },
    { at: 750, fadeMs: 180, delta: { ParamEyeBallX: 0.6 } },
    { at: 950, fadeMs: 260, delta: { ParamEyeBallX: 0.4, ParamAngleX: 0.5 } },
    { at: 1500, fadeMs: 420, delta: {} },
  ],
  // 倾听：轻微 AngleY/Z + 凝视（EyeBall 随头微偏）+ 微眉；不写身体
  listen: [
    { at: 0, fadeMs: 300, delta: { ParamAngleZ: -0.16, ParamAngleY: 0.12, ParamEyeBallX: -0.22, ParamBrowLY: 0.12, ParamBrowRY: 0.12 } },
    { at: 900, fadeMs: 300, delta: { ParamAngleZ: -0.14, ParamAngleY: 0.1, ParamEyeBallX: -0.18, ParamBrowLY: 0.1, ParamBrowRY: 0.1 } },
    { at: 1700, fadeMs: 480, delta: {} },
  ],
  // 惊讶：睁眼（EyeOpen 正向）+ 挑眉 + 张嘴 + 头部轻微后仰（AngleY 小幅度）
  surprise: [
    { at: 0, fadeMs: 120, delta: { ParamEyeLOpen: 0.85, ParamEyeROpen: 0.85, ParamBrowLY: 0.8, ParamBrowRY: 0.8, ParamMouthOpenY: 0.9, ParamAngleY: 0.2 } },
    { at: 500, fadeMs: 120, delta: { ParamEyeLOpen: 0.6, ParamEyeROpen: 0.6, ParamBrowLY: 0.6, ParamBrowRY: 0.6, ParamMouthOpenY: 0.7, ParamAngleY: 0.15 } },
    { at: 900, fadeMs: 400, delta: {} },
  ],
};

/** 解析动作名：命中六动作库返回 id，否则 null（未知 / 已删除动作 → 不播放）。 */
export function resolveChoreography(name: string | undefined | null): string | null {
  if (!name) return null;
  const key = name.startsWith('choreo:') ? name.slice(7) : name;
  const id = key.trim().toLowerCase();
  return CHOREOGRAPHY_LIBRARY[id] ? id : null;
}

/**
 * 时间轴播放器：按真实时钟推进关键帧，逐帧写入 Arbiter（LiveEvent 源）。
 * 关键帧是「语义幅度」，经 profile（min/max/default）映射为归一化值写入；
 * 播完 / cancel 后 release 回 profile 默认（仲裁器回退语义）。
 */
export class ChoreographyPlayer {
  private readonly arbiter: ParamArbiter;
  private readonly now: () => number;
  private readonly ranges: Record<string, ParameterRange>;
  private startedAt = 0;
  private pending: Keyframe[] = [];
  private intensity = 1;
  private activeId: string | null = null;
  private endAt = Infinity;

  constructor(arbiter: ParamArbiter, opts: { now?: () => number } = {}) {
    this.arbiter = arbiter;
    this.now = opts.now ?? Date.now;
    this.ranges = arbiter.getHalfBodyProfile().ranges;
  }

  /** 播放一段动作；未知 / 未受支持返回 false。intensity 缩放语义幅度。 */
  play(name: string | undefined | null, intensity = 1): boolean {
    const id = resolveChoreography(name);
    if (!id) return false;
    this.cancel();
    const frames = CHOREOGRAPHY_LIBRARY[id];
    if (!frames || frames.length === 0) return false;
    this.activeId = id;
    this.intensity = Math.min(1.35, Math.max(0.5, Number.isFinite(intensity) ? intensity : 1));
    this.pending = frames.map((f) => ({ at: f.at, fadeMs: f.fadeMs ?? 160, delta: { ...f.delta } }));
    this.startedAt = this.now();
    const last = this.pending[this.pending.length - 1];
    this.endAt = last.at + (last.fadeMs ?? 160) + 80;
    this.pump();
    return true;
  }

  /** 每帧推进（单个时间推进点，由 BodyActionPlayer.update → 主 ticker 调用）。 */
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
      if (kf.delta && Object.keys(kf.delta).length > 0) this.write(kf);
    }
    if (this.pending.length === 0 && elapsed >= this.endAt) {
      this.cancel();
      this.arbiter.releaseAll(ParamSource.LiveEvent); // 回 profile 默认 / 低优先级基线
    }
  }

  private write(kf: Keyframe): void {
    const ts = this.now();
    const sourceParams = sanitizeOverrides(kf.delta);
    for (const [paramId, rawDelta] of Object.entries(sourceParams)) {
      const range = this.ranges[paramId];
      if (!range) continue; // 模型没有该参数 → 不写（不假设 0.5）
      const d = Math.min(1, Math.max(-1, rawDelta * this.intensity));
      // 语义方向无可动跨度（如 EyeOpen 默认即最大）→ 跳过该参数，
      // 绝不把「假 0.5 / 硬 0」写进模型（release 语义才负责回默认）
      const span = d >= 0 ? range.maximum - range.defaultValue : range.defaultValue - range.minimum;
      if (!(span > 0)) continue;
      const normalized = semanticToNormalized(range, d);
      this.arbiter.write({
        source: ParamSource.LiveEvent,
        paramId,
        value: Math.min(1, Math.max(0, normalized)),
        mode: 'set',
        fadeMs: kf.fadeMs ?? 160,
        timestamp: ts,
        valueSpace: 'normalized',
      });
    }
  }
}

/** 播放请求附带的调度元信息 */
export interface ActionMeta {
  /** 动作优先级（默认 Fallback=情绪兜底，见 ActionPriority） */
  priority?: ActionPriority;
  /** 触发来源（'dev' | 'cue' | 'audio' | 'live' | ...），仅用于日志/面板 */
  source?: string;
}

export interface BodyActionPlayerOptions {
  /** 时钟（测试注入） */
  now?: () => number;
  /** 同动作自然结束后再次触发的冷却 ms（默认 250），防瞬时连点反跳 */
  cooldownMs?: number;
}

interface ActiveAction {
  id: string;
  strength: number;
  priority: ActionPriority;
  source: string;
  requestedAt: number;
}

/**
 * 单一 active 动作调度器（用户终验定版，仅六个头 / 脸动作 + release）：
 *  - 同一时刻只播放一个动作（单 active）；
 *  - 同动作正在播放 / 冷却期内重复请求 → 忽略；
 *  - 更高优先级动作 → 安全打断（arbiter fade 提供短过渡）；
 *  - 相同 / 更低优先级动作 → 进入唯一 pending 槽（替换旧 pending，不叠加不抖动）；
 *  - 未知 / 已删除动作（bow/lean_in/sway/happy/excited/wave_big/bounce_happy 等）→ no-op；
 *  - 动作自然结束自动回 profile 默认；stop / release 立即清空 current 与 pending。
 */
export class BodyActionPlayer {
  private readonly choreo: ChoreographyPlayer;
  private readonly arbiter: ParamArbiter;
  private readonly now: () => number;
  private readonly cooldownMs: number;
  private current: ActiveAction | null = null;
  private pending: ActiveAction | null = null;
  private readonly finishedAt = new Map<string, number>();

  constructor(arbiter: ParamArbiter, options: BodyActionPlayerOptions = {}) {
    this.arbiter = arbiter;
    this.choreo = new ChoreographyPlayer(arbiter, { now: options.now });
    this.now = options.now ?? Date.now;
    this.cooldownMs = options.cooldownMs ?? 250;
  }

  /**
   * 请求播放一个动作。
   * @param name 动作名（六个头/脸动作之一）；'idle' / '' / null = release
   * @param strength 强度 1/2/3
   * @param meta 优先级与来源（决定打断/排队语义）
   */
  play(name: string | undefined | null, strength = 2, meta: ActionMeta = {}): void {
    const rawId = String(name ?? '').trim().toLowerCase();
    if (rawId === 'idle' || rawId === '' || rawId === 'release') {
      this.release();
      return;
    }
    const id = resolveChoreography(rawId);
    if (!id) return; // 未知 / 已删除动作：no-op，不改变当前状态

    const req: ActiveAction = {
      id,
      strength: strength === 1 || strength === 2 || strength === 3 ? strength : 2,
      priority: meta.priority ?? ActionPriority.Fallback,
      source: meta.source ?? 'unknown',
      requestedAt: this.now(),
    };

    // 冷却保护：同动作自然结束后一段时间内的重复请求一律忽略（防瞬时连点反跳）
    const lastFinished = this.finishedAt.get(req.id);
    if (lastFinished !== undefined && this.now() - lastFinished < this.cooldownMs) return;

    if (!this.current) {
      this.start(req);
      return;
    }

    // 同动作：正在播放 → 忽略
    if (this.current.id === req.id) return;

    if (req.priority > this.current.priority) {
      // 更高优先级可打断当前动作（被打断的动作不恢复，避免出队顺序紊乱）
      this.cancelActive();
      this.start(req);
      return;
    }

    // 相同 / 更低优先级：单一 pending 槽，替换旧 pending → 不叠加、不抖动
    this.pending = req;
  }

  /** 每帧推进（单时间推进点）。 */
  update(deltaMs: number): void {
    this.choreo.update(deltaMs);
    if (!this.current) return;
    if (!this.choreo.isPlaying()) this.finishCurrent();
  }

  /** 立即停止当前动作并清空 pending，参数回 profile 默认 / 低优先级基线。 */
  release(): void {
    this.cancelActive();
    this.pending = null;
  }

  /** 当前正在播放的动作（choreo 带 'choreo:' 前缀；无 = null），供面板/测试用 */
  currentGesture(): string | null {
    if (!this.current) return null;
    return `choreo:${this.current.id}`;
  }

  /** 当前动作优先级（面板/测试用） */
  currentPriority(): ActionPriority | null {
    return this.current?.priority ?? null;
  }

  /** 当前 pending 的动作名（单一槽位；无 = null） */
  pendingAction(): string | null {
    if (!this.pending) return null;
    return `choreo:${this.pending.id}`;
  }

  private start(req: ActiveAction): void {
    if (this.choreo.play(req.id, this.intensityOf(req.strength))) {
      this.current = req;
      this.logAction(req);
      return;
    }
    this.current = null;
  }

  /** 打断/停止当前动作：终止编排并释放 LiveEvent 全部记录，
   *  让参数立即开始回 profile 默认 / 下层低优先级基线（fade 过渡）。
   *  被打断动作直接被丢弃，不进入 pending；紧跟的新动作会重新建立 LiveEvent 记录。 */
  private cancelActive(): void {
    this.choreo.cancel();
    // 释放 LiveEvent 全部记录：参数回 profile 默认 / 低优先级基线
    this.arbiter.releaseAll(ParamSource.LiveEvent);
    this.current = null;
  }

  /** 当前动作自然结束：记录冷却 → 尝试启动 pending */
  private finishCurrent(): void {
    if (this.current) {
      this.finishedAt.set(this.current.id, this.now());
      this.current = null;
    }
    if (this.pending) {
      const next = this.pending;
      this.pending = null;
      this.start(next);
    }
  }

  private logAction(req: ActiveAction): void {
    const peak = this.peakDeviation(req.id, this.intensityOf(req.strength));
    console.debug(
      `[BodyActionPlayer] start=choreo:${req.id} level=${req.strength} priority=${req.priority} source=${req.source} peakDev=${peak.toFixed(2)} pending=${this.pending ? this.pending.id : '-'}`,
    );
  }

  private intensityOf(level: number): number {
    return level === 1 ? 0.7 : level === 3 ? 1.35 : 1;
  }

  /** 语义幅度峰值（用于可观测日志；范围上限为最小轴的最大可能幅度）。 */
  private peakDeviation(id: string | null | undefined, intensity: number): number {
    const frames = CHOREOGRAPHY_LIBRARY[String(id ?? '')];
    if (!frames) return 0;
    let peak = 0;
    for (const f of frames) {
      for (const v of Object.values(f.delta)) {
        if (typeof v !== 'number') continue;
        peak = Math.max(peak, Math.min(1, Math.abs(v * intensity)));
      }
    }
    return peak;
  }
}