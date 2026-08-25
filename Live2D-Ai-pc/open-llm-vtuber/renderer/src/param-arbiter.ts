/**
 * param-arbiter.ts — RM3 参数仲裁器（tier: premium）
 *
 * 多源同参数冲突仲裁（借鉴 VTube Studio 优先级叠加机制）：
 *   - 同参数：高优先级源覆盖低优先级源；低优先级写入被阻塞（不生效）但记录在案，
 *     高优先级结束后参数回退到低优先级的最后有效值。
 *   - 同优先级内：后写入覆盖先写入。
 *   - fade：线性插值（lerp），from → to，时长 fadeMs（默认 400ms，300-500ms 平滑区间；
 *     fadeMs=0 立即覆盖）。
 *   - Blink 眼部特权（用户确认 Q2，仅 ParamEyeLOpen / ParamEyeROpen 两参数）：
 *       · Blink 写眼部参数可穿透 ≤ Emotion 优先级的源（含 Emotion）直接夺权；
 *       · Blink 持有眼部参数期间，≤ Emotion 的源不能夺权（保证眨眼闭眼不被表情打断）；
 *       · UserTouch(L6) / LipSync(L7) 仍按普通规则可压过 Blink。
 *   - 三态叠加模式：set（覆盖）/ add（叠加）/ mult（乘算），在 JS 层归一化后
 *     统一经 setParameterValueById 覆盖写入 coreModel（架构 §3.3.4，避免双重语义混乱）。
 *
 * 优先级（硬编码，低 → 高）：
 *   Breath(0) < MicroExpr(1) < Blink(2) < IdleMotion(3) < MoodBaseline(4)
 *   < Emotion(5) < UserTouch(6) < LipSync(7)
 */


import { buildHalfBodyProfile, normalizedToModelValue, type HalfBodyProfile } from './halfbody-profile';

/** 参数来源优先级（低 → 高，硬编码不可配置） */
export enum ParamSource {
  Breath = 0,
  MicroExpr = 1,
  Blink = 2,
  IdleMotion = 3,
  MoodBaseline = 4,
  Emotion = 5,
  UserTouch = 6,
  LipSync = 7,
  /** 直播/弹幕事件触发的身体动作（追加项，不改变既有枚举数值） */
  LiveEvent = 8,
}

/** 参数写入请求（不立即生效，由 write() 判定仲裁后进入 fade） */
export interface ParamWrite {
  /** 来源（决定优先级） */
  source: ParamSource;
  /** Cubism 参数 ID，如 "ParamMouthForm" */
  paramId: string;
  /** 目标值 0..1（归一化） */
  value: number;
  /** 叠加模式：set=覆盖 / add=叠加 / mult=乘算 */
  mode: 'set' | 'add' | 'mult';
  /** 过渡时间 ms（0=立即覆盖，默认 400） */
  fadeMs: number;
  /** 写入时刻 */
  timestamp: number;
  /**
   * 数值空间：'model' = 直接模型值（如 EMOTION_PARAMS），'normalized' = 0..1 归一化值
   * 默认 'model'，保持既有写入行为不变。
   */
  valueSpace?: 'model' | 'normalized';
}

type ParamMode = 'set' | 'add' | 'mult';

/** 单源在单参数上的最后有效写入 */
interface SourceRecord {
  value: number;
  mode: ParamMode;
  fadeMs: number;
  timestamp: number;
  valueSpace?: 'model' | 'normalized';
}

/** 单参数仲裁状态 */
interface ParamState {
  /** 参数 ID（release 回默认时查 profile 用） */
  paramId: string;
  /** 各源最后有效写入（含被阻塞的低优先级记录，用于回退） */
  sources: Map<ParamSource, SourceRecord>;
  /** 当前驱动该参数的源（-1 = 无源驱动，回退到 profile 默认） */
  activeSource: ParamSource | -1;
  /** 当前呈现值所处数值空间（fromValue/currentValue/toValue 均在此空间） */
  valueSpace: ValueSpace;
  /** 该参数是否曾被任何源激活过（首次激活时以 profile 默认值为 fade 基线） */
  activated: boolean;
  /** 当前输出值（lerp 后） */
  currentValue: number;
  /** fade 起始值 */
  fromValue: number;
  /** fade 目标值 */
  toValue: number;
  /** fade 总时长 ms */
  fadeMs: number;
  /** fade 已推进 ms */
  elapsed: number;
  /** 当前帧是否需要写入 coreModel（值/源/fade 发生变化时置位） */
  dirty: boolean;
}

interface CoreModelLike {
  setParameterValueById?: (id: string, value: number) => void;
  getParameterCount?: () => number;
  getParameterId?: (index: number) => unknown;
  getParameterMinimumValue?: (index: number) => number;
  getParameterMaximumValue?: (index: number) => number;
  getParameterDefaultValue?: (index: number) => number;
  getModel?: () => unknown;
}

interface InternalModelLike {
  coreModel?: CoreModelLike;
}

/** Blink 眼部特权穿透覆盖的参数（仅这两参数，用户确认 Q2） */
const EYE_PARAMS = new Set(['ParamEyeLOpen', 'ParamEyeROpen']);

/** fade 默认时长 ms（契约 A3：300-500ms 平滑区间，默认 400） */
const DEFAULT_FADE_MS = 400;

function clamp01(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return Math.min(1, Math.max(0, v));
}

function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

/** 数值空间：'model' = 模型原始值；'normalized' = 0..1 归一化值 */
type ValueSpace = 'model' | 'normalized';

/** 模型值 → 归一化（0..1）；range 保证 maximum > minimum */
function normalizedFromModelValue(range: { minimum: number; maximum: number }, modelValue: number): number {
  return clamp01((modelValue - range.minimum) / (range.maximum - range.minimum));
}

/**
 * 参数仲裁器——多轨融合核心。
 *
 * 用法：
 *   const arbiter = new ParamArbiter(internalModel);
 *   arbiter.write({ source: ParamSource.Emotion, paramId: 'ParamMouthForm',
 *                   value: 0.9, mode: 'set', fadeMs: 400, timestamp: Date.now() });
 *   // 每帧：
 *   arbiter.update(deltaMs);   // 推进 lerp fade + 合并多源 → 写入 coreModel
 */
export class ParamArbiter {
  private readonly model: InternalModelLike | null;
  private readonly profile: HalfBodyProfile;
  private readonly paramStates = new Map<string, ParamState>();

  constructor(internalModel: unknown) {
    this.model = (internalModel ?? null) as InternalModelLike | null;
    this.profile = buildHalfBodyProfile(this.model?.coreModel);
  }

  /** 当前模型真实参数范围与半身能力（只读快照）。 */
  getHalfBodyProfile(): HalfBodyProfile {
    return { ranges: { ...this.profile.ranges }, capabilities: { ...this.profile.capabilities } };
  }

  /**
   * 写入一个参数请求（含仲裁判定；不立即生效，进入 fade，由 update() 推进并写 coreModel）
   */
  write(req: ParamWrite): void {
    if (!req || !req.paramId) return;

    const mode: ParamMode = req.mode ?? 'set';
    const fadeMs = Number.isFinite(req.fadeMs) ? Math.max(0, req.fadeMs) : DEFAULT_FADE_MS;
    const timestamp = Number.isFinite(req.timestamp) ? req.timestamp : Date.now();

    let state = this.paramStates.get(req.paramId);
    if (!state) {
      state = {
        paramId: req.paramId,
        sources: new Map(),
        activeSource: -1,
        valueSpace: 'model',
        activated: false,
        currentValue: 0,
        fromValue: 0,
        toValue: 0,
        fadeMs: 0,
        elapsed: 0,
        dirty: true,
      };
      this.paramStates.set(req.paramId, state);
    }

    // 记录该源的最后一次写入（无论是否被更高优先级阻塞——回退时用到）
    state.sources.set(req.source, { value: req.value, mode, fadeMs, timestamp, valueSpace: req.valueSpace });

    // 仲裁判定：该写入是否有权成为当前驱动源
    if (!this.canOverride(state.activeSource, req.source, req.paramId)) {
      return; // 被更高优先级阻塞
    }

    this.activate(state, req.source);
  }

  /**
   * 每帧调用：推进所有参数的 lerp fade，并把合并结果写入 coreModel。
   * @param deltaMs 帧间隔 ms
   */
  update(deltaMs: number): void {
    const dt = Number.isFinite(deltaMs) && deltaMs > 0 ? deltaMs : 0;
    const coreModel = this.model?.coreModel;

    for (const [paramId, state] of this.paramStates) {
      const needsUpdate = state.dirty || state.fadeMs > 0;
      if (!needsUpdate) continue;

      if (state.fadeMs > 0) {
        if (state.elapsed < state.fadeMs) {
          state.elapsed += dt;
          const t = Math.min(state.elapsed / state.fadeMs, 1);
          state.currentValue = lerp(state.fromValue, state.toValue, t);
        } else {
          state.currentValue = state.toValue; // fade 完成，稳定在目标值
        }
      }
      // fadeMs === 0 → activate() 时已立即生效，此处直接沿用 currentValue

      const range = this.profile.ranges[paramId];
      // 呈现值空间由 state.valueSpace 跟踪（release 后源记录已删除，不能反查 sources）
      const modelValue = (range && state.valueSpace === 'normalized')
        ? normalizedToModelValue(range, state.currentValue)
        : state.currentValue;
      coreModel?.setParameterValueById?.(paramId, modelValue);
      state.dirty = false;
    }
  }

  /**
   * 释放某个源对某参数的控制（如口型结束）。
   * 若该源正驱动此参数 → 回退到剩余最高优先级源的最后有效值；
   * 无剩余源 → 回模型/profile 运行时默认值（绝不硬写 0 / 假 0.5）。
   */
  release(paramId: string, source: ParamSource): void {
    const state = this.paramStates.get(paramId);
    if (!state) return;

    const releasedFadeMs = state.sources.get(source)?.fadeMs ?? 0;
    state.sources.delete(source);

    if (state.activeSource !== source) return;
    this.fallBackAfterRelease(state, releasedFadeMs);
  }

  /**
   * 释放某源对所有参数的控制（如表情结束 → 回退到 idle/mood 基线；
   * 无低优先级源时逐参数回模型/profile 运行时默认值）。
   */
  releaseAll(source: ParamSource): void {
    for (const state of this.paramStates.values()) {
      const wasActive = state.activeSource === source;
      if (!wasActive) {
        state.sources.delete(source); // 非驱动源：仅清记录
        continue;
      }

      const releasedFadeMs = state.sources.get(source)?.fadeMs ?? 0;
      state.sources.delete(source);
      this.fallBackAfterRelease(state, releasedFadeMs);
    }
  }

  /**
   * 当前驱动某参数的源（-1 = 无源驱动，即回退到 profile 默认）。
   */
  getCurrentSource(paramId: string): ParamSource | -1 {
    const state = this.paramStates.get(paramId);
    return state ? state.activeSource : -1;
  }

  /**
   * 清空全部仲裁状态，并把已知参数写回 profile 默认（模型复位）。
   */
  reset(): void {
    const coreModel = this.model?.coreModel;
    for (const paramId of this.paramStates.keys()) {
      const range = this.profile.ranges[paramId];
      coreModel?.setParameterValueById?.(paramId, range?.defaultValue ?? 0);
    }
    this.paramStates.clear();
  }

  // ---- 内部实现 ----

  /**
   * release/releaseAll 之后的回退：
   *  - 有低优先级源 → 回退到该源的最后有效值（跨空间时先换算到该源空间）；
   *  - 无任何剩余源 → 回模型/profile 运行时默认值（归一化空间则换算为归一化默认），
   *    绝不把眼/嘴/头硬写 0 或假 0.5。
   */
  private fallBackAfterRelease(state: ParamState, releasedFadeMs: number): void {
    const fallback = this.pickFallback(state);
    const range = this.profile.ranges[state.paramId];

    if (fallback) {
      const recSpace: ValueSpace = fallback.rec.valueSpace ?? 'model';
      if (recSpace !== state.valueSpace && range) {
        const modelValue = state.valueSpace === 'normalized'
          ? normalizedToModelValue(range, state.currentValue)
          : state.currentValue;
        state.currentValue = recSpace === 'normalized'
          ? normalizedFromModelValue(range, modelValue)
          : modelValue;
        state.fromValue = state.currentValue;
      }
      state.valueSpace = recSpace;
      state.activeSource = fallback.source;
      const target = this.computeTarget(fallback.rec.mode, fallback.rec.value, state.currentValue);
      this.fadeTo(state, target, fallback.rec.fadeMs);
      return;
    }

    state.activeSource = -1;
    let target = 0; // profile 无该参数范围（未知通道）：保持旧兜底 0
    if (range) {
      target = range.defaultValue;
      if (state.valueSpace === 'normalized') {
        target = normalizedFromModelValue(range, range.defaultValue);
      }
    }
    // 目标本身就是合法默认值 → 不再 clamp01（模型空间角度默认可能超出 [0,1]）
    this.fadeToRaw(state, target, releasedFadeMs);
  }

  /** 发起一次 fade 过渡（fadeMs<=0 立即到位） */
  private fadeTo(state: ParamState, target: number, fadeMs: number): void {
    state.fromValue = state.currentValue;
    state.toValue = clamp01(target);
    state.fadeMs = fadeMs > 0 ? fadeMs : 0;
    state.elapsed = 0;
    state.dirty = true;
    if (fadeMs <= 0) {
      state.currentValue = state.toValue;
    }
  }

  /** 同 fadeTo，但目标不 clamp01（用于回 profile 默认：目标已合法） */
  private fadeToRaw(state: ParamState, target: number, fadeMs: number): void {
    const v = Number.isFinite(target) ? target : 0;
    state.fromValue = state.currentValue;
    state.toValue = v;
    state.fadeMs = fadeMs > 0 ? fadeMs : 0;
    state.elapsed = 0;
    state.dirty = true;
    if (fadeMs <= 0) {
      state.currentValue = v;
    }
  }

  /** 让某源成为某参数的驱动源，启动 fade 过渡 */
  private activate(state: ParamState, source: ParamSource): void {
    const rec = state.sources.get(source);
    if (!rec) return;

    // 数值空间切换：把当前呈现值换算到本次写入的空间，保证 fade 全程同空间插值
    const recSpace: ValueSpace = rec.valueSpace ?? 'model';
    const range = this.profile.ranges[state.paramId];
    if (!state.activated) {
      // 首次激活：以 profile 运行时默认值为基线（而不是任意 0），动作从默认姿态平滑出发
      const base = range ? range.defaultValue : 0;
      state.currentValue = (recSpace === 'normalized' && range)
        ? normalizedFromModelValue(range, base)
        : base;
      state.activated = true;
    } else if (recSpace !== state.valueSpace && range) {
      const modelValue = state.valueSpace === 'normalized'
        ? normalizedToModelValue(range, state.currentValue)
        : state.currentValue;
      state.currentValue = recSpace === 'normalized'
        ? normalizedFromModelValue(range, modelValue)
        : modelValue;
    }
    state.valueSpace = recSpace;

    const target = this.computeTarget(rec.mode, rec.value, state.currentValue);
    state.activeSource = source;
    state.fromValue = state.currentValue;
    state.toValue = clamp01(target);
    state.fadeMs = rec.fadeMs;
    state.elapsed = 0;

    state.dirty = true;
    if (rec.fadeMs <= 0) {
      state.currentValue = state.toValue; // 无 fade：立即覆盖
    }
  }

  /**
   * 仲裁判定：incoming 源能否覆盖当前 active 源对 paramId 的控制。
   */
  private canOverride(
    active: ParamSource | -1,
    incoming: ParamSource,
    paramId: string
  ): boolean {
    if (active === -1) return true;
    if (incoming === active) return true; // 同优先级内：后写覆盖先写

    const isEye = EYE_PARAMS.has(paramId);

    if (incoming > active) {
      // 普通规则：高优先级覆盖。但 Blink 正持有眼部参数时，≤ Emotion 的源不能夺权
      // （保证眨眼闭眼过程不被表情/心情基线打断）
      if (active === ParamSource.Blink && isEye && incoming <= ParamSource.Emotion) {
        return false;
      }
      return true;
    }

    // incoming < active：Blink 眼部特权穿透（仅眼部两参数，仅穿透 ≤ Emotion 的源）
    if (incoming === ParamSource.Blink && isEye && active <= ParamSource.Emotion) {
      return true;
    }
    return false;
  }

  /** 计算某模式下的目标值（set/add/mult），并 clamp 到 0..1 */
  private computeTarget(mode: ParamMode, value: number, base: number): number {
    switch (mode) {
      case 'add':
        return clamp01(base + value);
      case 'mult':
        return clamp01(base * value);
      case 'set':
      default:
        return clamp01(value);
    }
  }

  /** 找出剩余源中优先级最高的（-1 = 无） */
  private pickFallback(
    state: ParamState
  ): { source: ParamSource; rec: SourceRecord } | null {
    let best: { source: ParamSource; rec: SourceRecord } | null = null;
    for (const [s, rec] of state.sources) {
      if (!best || s > best.source) best = { source: s, rec };
    }
    return best;
  }
}
