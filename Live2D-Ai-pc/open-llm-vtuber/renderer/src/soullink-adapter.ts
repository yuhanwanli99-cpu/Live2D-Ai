/**
 * soullink-adapter.ts — @soullink-emotion/engine 适配层（PLAN-V3 P1 双跑）
 *
 * 职责：把既有链路信号翻译为引擎输入——
 *   emotion 帧(8key+strength) ──► EmotionIntent(naturalVAD 精确驱动，不赌词表)
 *   括注 cue(动作名)          ──► 重音 intent 脉冲
 *   audio.volumes(slice)      ──► AudioLevelAnalyzer 拉模型（口型单一来源归引擎）
 * 每帧 frame(dt)：runtime.update → snapshot.live2dParams 覆盖写入 coreModel
 * （在 afterMotionUpdate 相位执行，见 main.ts 接线——规避模型每帧参数重置）。
 *
 * 失败语义：永不静默崩溃——运行期异常吞掉但 console.error 首次可见；构造失败由调用方回退 legacy。
 */
import { SoullinkRuntime } from '@soullink-emotion/engine';

export type PerformanceEngine = 'legacy' | 'soullink';

/** 读取表演引擎开关（localStorage；缺省 legacy）。独立函数便于测试与设置页复用。 */
export function resolvePerformanceEngine(store?: Pick<Storage, 'getItem'>): PerformanceEngine {
  try {
    const s = store ?? (typeof localStorage !== 'undefined' ? localStorage : undefined);
    return s?.getItem('performanceEngine') === 'soullink' ? 'soullink' : 'legacy';
  } catch {
    return 'legacy';
  }
}

/** 8 key 情绪 → 连续 VAD（Valence/Arousal/Dominance ∈ [-1,1]）：力度与方向由这里定义 */
export const EMOTION_VAD: Record<string, { valence: number; arousal: number; dominance: number }> = {
  neutral:  { valence: 0,    arousal: 0,    dominance: 0 },
  joy:      { valence: 0.8,  arousal: 0.5,  dominance: 0.2 },
  anger:    { valence: -0.6, arousal: 0.7,  dominance: 0.5 },
  sadness:  { valence: -0.7, arousal: -0.4, dominance: -0.3 },
  surprise: { valence: 0.3,  arousal: 0.8,  dominance: -0.1 },
  fear:     { valence: -0.6, arousal: 0.6,  dominance: -0.6 },
  smirk:    { valence: 0.2,  arousal: 0.2,  dominance: 0.6 },
  disgust:  { valence: -0.5, arousal: -0.1, dominance: 0.4 },
};

/** 括注/动作名 → 重音情绪提示（cue 到点时的 intent 来源） */
export const MOTION_EMOTION_HINT: Record<string, string> = {
  nod: 'joy', tilt: 'smirk', sway: 'joy', happy: 'joy', surprise: 'surprise',
  listen: 'neutral', excited: 'joy', wave_big: 'joy', bow: 'neutral',
  shake_no: 'anger', lean_in: 'smirk', look_around: 'fear', bounce_happy: 'joy',
};

export interface EmotionIntentLike {
  emotion: string;
  naturalVAD: { valence: number; arousal: number; dominance: number };
  intensity: number;
  contextTags: string[];
}

/** 8key+strength → 引擎 intent（naturalVAD 精确驱动；intensity = strength/3 下限 0.2） */
export function mapEmotionToIntent(emotion: string, strength: number): EmotionIntentLike {
  const key = EMOTION_VAD[emotion] ? emotion : 'neutral';
  const vad = EMOTION_VAD[key];
  const intensity = Math.min(1, Math.max(0.2, strength / 3));
  return { emotion: key, naturalVAD: { ...vad }, intensity, contextTags: ['dialogue'] };
}

/** 括注动作名 → 重音 intent；未知动作回落 neutral 微脉冲 */
export function mapCueToIntent(motionName: string): EmotionIntentLike {
  const emotion = MOTION_EMOTION_HINT[motionName] ?? 'neutral';
  return mapEmotionToIntent(emotion, motionName === 'neutral' ? 1 : 3);
}

/** 音频拉桥：把 WS 推来的 volumes 片段转成引擎要的 getLevel() 拉模型 */
export class PushPullAudioLevels {
  private slices: number[] = [];
  private sliceMs = 20;
  private startMs = 0;
  constructor(private readonly nowMs: () => number) {}
  push(volumes: number[], sliceMs: number): void {
    if (!Array.isArray(volumes) || volumes.length === 0) return;
    this.slices = volumes.slice();
    this.sliceMs = sliceMs > 0 ? sliceMs : 20;
    this.startMs = this.nowMs();
  }
  getLevel(): number {
    if (this.slices.length === 0) return 0;
    const elapsed = this.nowMs() - this.startMs;
    const idx = Math.floor(elapsed / this.sliceMs);
    if (idx < 0 || idx >= this.slices.length) return 0;
    return this.slices[idx];
  }
  reset(): void {
    this.slices = [];
  }
}

interface CoreModelLike {
  setParameterValueById?: (id: string, value: number) => void;
}

export interface SoullinkAdapterOptions {
  profile: unknown;
  coreModel: () => CoreModelLike | null;
  /** 毫秒时钟（默认 Date.now）；测试注入 */
  now?: () => number;
}

/**
 * 表演适配器：持有 SoullinkRuntime，负责三件事——
 *   setEmotion/cue/conversationEvent 注入意图；feedMouth 推音频片；frame 推进并写参数。
 */
export class SoullinkPerformanceAdapter {
  private readonly runtime: SoullinkRuntime;
  private readonly coreModel: () => CoreModelLike | null;
  private readonly levels: PushPullAudioLevels;
  private timeSec = 0;
  private warnedOnce = false;

  constructor(opts: SoullinkAdapterOptions) {
    this.coreModel = opts.coreModel;
    this.levels = new PushPullAudioLevels(opts.now ?? Date.now);
    this.runtime = new SoullinkRuntime({
      profile: opts.profile as never,
      audioLevelAnalyzer: this.levels,
    });
  }

  /** emotion 帧（8key + strength 1..3）→ 引擎意图 */
  setEmotion(emotion: string, strength: number): void {
    const intent = mapEmotionToIntent(emotion, strength);
    this.safe(() => this.runtime.triggerIntent(intent, this.timeSec));
  }

  /** 括注 cue 到点 → 重音脉冲 */
  cue(motionName: string): void {
    const intent = mapCueToIntent(motionName);
    this.safe(() => this.runtime.triggerIntent(intent, this.timeSec));
  }

  /** 对话三态事件（listening/thinking/speaking-start/speaking-end）→ 轻量 VAD 触碰 */
  conversationEvent(evt: 'listening' | 'thinking' | 'speaking-start' | 'speaking-end'): void {
    this.safe(() => {
      switch (evt) {
        case 'speaking-start':
          (this.runtime as unknown as { setVoicePlaybackActive(v: boolean): void }).setVoicePlaybackActive(true);
          break;
        case 'speaking-end':
          (this.runtime as unknown as { setVoicePlaybackActive(v: boolean): void }).setVoicePlaybackActive(false);
          break;
        default:
          break; // listening/thinking 由 emotion intent 与 VAD 自然表达，P1 不额外注入
      }
    });
  }

  /** WS audio 帧到达：推音频片给引擎口型拉模型 */
  feedMouth(volumes: number[], sliceLengthMs?: number): void {
    this.safe(() => this.levels.push(volumes, sliceLengthMs ?? 20));
    this.conversationEvent('speaking-start');
  }

  /** 口型队列排空/打断：结束发声态 */
  endMouth(): void {
    this.levels.reset();
    this.conversationEvent('speaking-end');
  }

  /** 每帧推进：推进引擎时钟 → 写 snapshot 参数到 coreModel（覆盖 legacy 残留写入）。 */
  frame(deltaMs: number): void {
    this.timeSec += Math.max(0, deltaMs) / 1000;
    this.safe(() => {
      const snap = this.runtime.update(this.timeSec, Math.max(0, deltaMs) / 1000);
      const core = this.coreModel();
      if (!core?.setParameterValueById) return;
      const params = snap?.live2dParams ?? {};
      for (const id of Object.keys(params)) {
        core.setParameterValueById(id, params[id]);
      }
    });
  }

  private safe(fn: () => void): void {
    try {
      fn();
    } catch (err) {
      if (!this.warnedOnce) {
        this.warnedOnce = true;
        console.error('[soullink-adapter] 表演引擎异常（首次可见，后续同类静默）:', err);
      }
    }
  }
}

/** 模型 URL → 同目录 soullink.profile.json（带缓存的幂等加载；失败可见并回落 null） */
const profileCache = new Map<string, Promise<unknown | null>>();

export function loadSoullinkProfile(modelUrl: string): Promise<unknown | null> {
  const base = modelUrl.slice(0, modelUrl.lastIndexOf('/') + 1);
  const url = base + 'soullink.profile.json';
  const hit = profileCache.get(url);
  if (hit) return hit;
  const p = (async () => {
    try {
      const res = await fetch(url);
      if (!res.ok) {
        console.warn('[soullink-adapter] profile 未找到（' + res.status + '）:' + url + ' → 回退 legacy 行为');
        return null;
      }
      return (await res.json()) as unknown;
    } catch (err) {
      console.error('[soullink-adapter] profile 加载失败（回退 legacy 行为）:', err);
      return null;
    }
  })();
  profileCache.set(url, p);
  return p;
}