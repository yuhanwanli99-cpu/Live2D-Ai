/**
 * emotion-consumer.ts — RM4 表情消费（tier: premium）
 *
 * 契约：contract-pc-render.md §RM4 + tests/rm4-emotion.test.ts（**接口以测试为准**）
 *
 * 职责（打磨集成阶段重写，对齐 tester 出题接口）：
 *   - RM4_EMOTIONS：8 key 表情枚举（joy/sad/angry/surprise/fear/disgust/neutral/love）
 *   - resolveEmotionParams(emotion, strength?)：纯函数查表，返回 parameterOverrides
 *     （8 表情 × 3 强度 = 24 组合，值逐项对齐测试 EXPECTED_MAP；strength 缺省=2、
 *      非法 strength 回退 2；未知 emotion 回退 neutral 组）
 *   - EmotionConsumer：
 *       · consume(overrides, fadeMs?)：原样写仲裁器（source=Emotion）
 *       · consumeFrame(frame)：WS emotion 帧入口（emotion/strength/parameterOverrides/fadeMs
 *         字段覆盖帧级过渡）
 *       · applyEmotion(emotion, strength?, fadeMs?)：写 6 个三轴参数（眉高对称+眼开合对称+嘴形/嘴开合）
 *       · updateMood(mood)：mood 基线联动（0 不写；正 mood MouthForm+/EyeLOpen+ 偏移；
 *         负 mood BrowY-/MouthForm- 偏移；source=MoodBaseline；越界 clamp ±100）
 *       · release()：释放 EMOTION 源控制
 *       · getCurrentEmotion()：最近表情（初始 null，未知回退 neutral）
 *
 * fade 规范（架构 A3）：默认 400ms，钳制在 [300, 500]（1200→500、150→300），
 *   构造器 fadeMs 与帧级 fadeMs 均受钳制。
 *
 * 参数值来源：tests/rm4-emotion.test.ts EXPECTED_MAP（测试为准，精确期望值表）。
 * 白-免费版参数 ID（ParamBrowLY/RY 眉高、ParamEyeLOpen/ROpen 眼开合、
 * ParamMouthForm/MouthOpenY 嘴形）已由 model-facts-bai.md 实读确认全部存在。
 *
 * 写入语义：
 *   - Emotion 源：value clamp 到 [0,1]（契约 ParamWrite.value 归一化约束；架构表原始值
 *     含负值/超 1，如 surprise._3 ParamEyeLOpen: 1.4，由消费端 clamp 修正）。
 *   - MoodBaseline 源：不 clamp（mood 偏移需表达负值——负 mood 嘴角下弯/眉下沉，
 *     测试断言写入负值；仲裁器内部 clamp01 由仲裁器自行处理）。
 */

import { ParamArbiter, ParamSource } from './param-arbiter';
import type { ParamWrite } from './param-arbiter';
import type { EmotionMessage, ParameterOverrides } from './types';

/** 表情强度档位（1=弱 / 2=中 / 3=强；缺省/非法一律回退 2） */
export type EmotionStrengthLevel = 1 | 2 | 3;

/** 8 表情 key 枚举（RM4-01 验收：覆盖 joy/sad/angry/surprise/fear/disgust/neutral/love） */
export const RM4_EMOTIONS = [
  'joy',
  'sad',
  'angry',
  'surprise',
  'fear',
  'disgust',
  'neutral',
  'love',
] as const;
export type EmotionKey = (typeof RM4_EMOTIONS)[number];

/** 表情 → 强度 → 参数组（目标绝对值，非乘数） */
export type EmotionParamTable = Record<string, Record<EmotionStrengthLevel, ParameterOverrides>>;

/**
 * EMOTION_PARAMS 映射表：8 表情 × 3 强度 × 6 参数（三轴：眉高/眼开合/嘴形）。
 * 数值逐项对齐 tests/rm4-emotion.test.ts EXPECTED_MAP（测试为准，不可擅改）。
 */
export const EMOTION_PARAMS: EmotionParamTable = {
  neutral: {
    1: { ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.75, ParamEyeROpen: 0.75, ParamMouthForm: 0.5, ParamMouthOpenY: 0.0 },
    2: { ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.75, ParamEyeROpen: 0.75, ParamMouthForm: 0.5, ParamMouthOpenY: 0.0 },
    3: { ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.75, ParamEyeROpen: 0.75, ParamMouthForm: 0.5, ParamMouthOpenY: 0.0 },
  },
  joy: {
    1: { ParamBrowLY: 0.15, ParamBrowRY: 0.15, ParamEyeLOpen: 0.8, ParamEyeROpen: 0.8, ParamMouthForm: 0.7, ParamMouthOpenY: 0.15 },
    2: { ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 0.85, ParamEyeROpen: 0.85, ParamMouthForm: 0.8, ParamMouthOpenY: 0.25 },
    3: { ParamBrowLY: 0.35, ParamBrowRY: 0.35, ParamEyeLOpen: 0.9, ParamEyeROpen: 0.9, ParamMouthForm: 0.9, ParamMouthOpenY: 0.35 },
  },
  angry: {
    1: { ParamBrowLY: -0.2, ParamBrowRY: -0.2, ParamEyeLOpen: 0.65, ParamEyeROpen: 0.65, ParamMouthForm: -0.4, ParamMouthOpenY: 0.08 },
    2: { ParamBrowLY: -0.35, ParamBrowRY: -0.35, ParamEyeLOpen: 0.6, ParamEyeROpen: 0.6, ParamMouthForm: -0.6, ParamMouthOpenY: 0.12 },
    3: { ParamBrowLY: -0.5, ParamBrowRY: -0.5, ParamEyeLOpen: 0.55, ParamEyeROpen: 0.55, ParamMouthForm: -0.8, ParamMouthOpenY: 0.18 },
  },
  sad: {
    1: { ParamBrowLY: 0.15, ParamBrowRY: 0.15, ParamEyeLOpen: 0.6, ParamEyeROpen: 0.6, ParamMouthForm: -0.3, ParamMouthOpenY: 0.1 },
    2: { ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 0.5, ParamEyeROpen: 0.5, ParamMouthForm: -0.5, ParamMouthOpenY: 0.15 },
    3: { ParamBrowLY: 0.35, ParamBrowRY: 0.35, ParamEyeLOpen: 0.45, ParamEyeROpen: 0.45, ParamMouthForm: -0.7, ParamMouthOpenY: 0.2 },
  },
  surprise: {
    1: { ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 1.0, ParamEyeROpen: 1.0, ParamMouthForm: 0.3, ParamMouthOpenY: 0.3 },
    2: { ParamBrowLY: 0.4, ParamBrowRY: 0.4, ParamEyeLOpen: 1.2, ParamEyeROpen: 1.2, ParamMouthForm: 0.3, ParamMouthOpenY: 0.45 },
    3: { ParamBrowLY: 0.55, ParamBrowRY: 0.55, ParamEyeLOpen: 1.4, ParamEyeROpen: 1.4, ParamMouthForm: 0.3, ParamMouthOpenY: 0.6 },
  },
  fear: {
    1: { ParamBrowLY: 0.1, ParamBrowRY: 0.1, ParamEyeLOpen: 1.0, ParamEyeROpen: 1.0, ParamMouthForm: 0.3, ParamMouthOpenY: 0.15 },
    2: { ParamBrowLY: 0.2, ParamBrowRY: 0.2, ParamEyeLOpen: 1.1, ParamEyeROpen: 1.1, ParamMouthForm: 0.3, ParamMouthOpenY: 0.2 },
    3: { ParamBrowLY: 0.3, ParamBrowRY: 0.3, ParamEyeLOpen: 1.2, ParamEyeROpen: 1.2, ParamMouthForm: 0.3, ParamMouthOpenY: 0.25 },
  },
  love: {
    1: { ParamBrowLY: 0.1, ParamBrowRY: 0.1, ParamEyeLOpen: 0.7, ParamEyeROpen: 0.7, ParamMouthForm: -0.3, ParamMouthOpenY: 0.0 },
    2: { ParamBrowLY: 0.15, ParamBrowRY: 0.15, ParamEyeLOpen: 0.7, ParamEyeROpen: 0.7, ParamMouthForm: -0.5, ParamMouthOpenY: 0.0 },
    3: { ParamBrowLY: 0.2, ParamBrowRY: 0.2, ParamEyeLOpen: 0.68, ParamEyeROpen: 0.68, ParamMouthForm: -0.7, ParamMouthOpenY: 0.05 },
  },
  disgust: {
    1: { ParamBrowLY: -0.1, ParamBrowRY: -0.1, ParamEyeLOpen: 0.7, ParamEyeROpen: 0.7, ParamMouthForm: -0.3, ParamMouthOpenY: 0.05 },
    2: { ParamBrowLY: -0.2, ParamBrowRY: -0.2, ParamEyeLOpen: 0.65, ParamEyeROpen: 0.65, ParamMouthForm: -0.5, ParamMouthOpenY: 0.08 },
    3: { ParamBrowLY: -0.3, ParamBrowRY: -0.3, ParamEyeLOpen: 0.6, ParamEyeROpen: 0.6, ParamMouthForm: -0.7, ParamMouthOpenY: 0.1 },
  },
};

/**
 * 表情 key 别名归一化（兼容架构 §3.2.2 的命名：anger/sadness/smirk ↔ 本表 8 key）。
 * 后端若下发架构命名，统一归一化到本表 key。
 */
export const EMOTION_ALIASES: Record<string, EmotionKey> = {
  anger: 'angry',
  sadness: 'sad',
  smirk: 'love',
};

/** 表情切换默认 fade（契约 RM4-05：构造器可覆盖；架构 fade 规范 300-500ms） */
const DEFAULT_FADE_MS = 400;
/** mood 基线过渡 fade（架构 fade 规范：更慢更自然） */
const MOOD_FADE_MS = 500;
/** fade 钳制区间（架构 A3：300-500ms 平滑区间） */
const MIN_FADE_MS = 300;
const MAX_FADE_MS = 500;

/** clamp fadeMs 到 [300, 500]（1200→500、150→300；非有限值回退默认 400） */
function clampFade(fadeMs: number): number {
  const f = Number.isFinite(fadeMs) ? fadeMs : DEFAULT_FADE_MS;
  return Math.min(MAX_FADE_MS, Math.max(MIN_FADE_MS, f));
}

/** clamp 到 [0,1]（契约 ParamWrite.value 归一化约束；仅 Emotion 源使用） */
function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(Math.max(value, 0), 1);
}

/** 解析强度档位：1/2/3 原样返回；缺省或非法（非 1/2/3）一律回退 2 */
function resolveStrength(strength?: number): EmotionStrengthLevel {
  const s = Number.isFinite(strength) ? (strength as number) : 2;
  if (s === 1 || s === 2 || s === 3) return s as EmotionStrengthLevel;
  return 2;
}

/** 归一化 emotion key（小写 + 别名映射），未命中返回 'neutral' */
function resolveEmotionKey(emotion: string): EmotionKey {
  const key = String(emotion ?? '').trim().toLowerCase();
  const aliased = EMOTION_ALIASES[key];
  if (aliased) return aliased;
  return (EMOTION_PARAMS[key] ? key : 'neutral') as EmotionKey;
}

/**
 * 纯函数：按表情 + 强度查表，返回 parameterOverrides（浅拷贝，防外部改表）。
 * 8 表情 × 3 强度 = 24 组合；strength 缺省 = 2、非法 strength 回退 2；
 * 未知 emotion 回退 neutral 组（strength 语义不变）。
 */
export function resolveEmotionParams(emotion: string, strength?: number): ParameterOverrides {
  const key = resolveEmotionKey(emotion);
  const level = resolveStrength(strength);
  return { ...EMOTION_PARAMS[key][level] };
}

/**
 * RM4 表情消费者。
 *
 * 用法（WS "emotion" 帧 → consumeFrame；后端直接下发参数组 → consume；
 * 本地表情意图 → applyEmotion；心情底色 → updateMood）：
 *   const consumer = new EmotionConsumer(arbiter);          // fadeMs 默认 400
 *   consumer.consumeFrame({ type:'emotion', emotion:'joy', strength:2,
 *                           parameterOverrides:{...}, fadeMs:450 });
 *   consumer.applyEmotion('joy', 2);
 *   consumer.updateMood(-60);
 *   consumer.release();                                     // 对话结束 → 回退基线
 */
export class EmotionConsumer {
  private readonly arbiter: ParamArbiter;
  private readonly fadeMs: number;
  private currentEmotion: EmotionKey | null = null;

  constructor(arbiter: ParamArbiter, fadeMs?: number) {
    this.arbiter = arbiter;
    this.fadeMs = clampFade(fadeMs ?? DEFAULT_FADE_MS);
  }

  /**
   * 消费后端下发的 parameterOverrides（RM4-01/RM4-02/RM4-04）。
   * 逐参数写入仲裁器（source=Emotion, mode='set', fadeMs=构造器默认/覆盖值）。
   * 空对象不抛异常、不写仲裁器（RM4-04）。同步调用（RM4-06）。
   */
  consume(parameterOverrides: Record<string, number>, fadeMs?: number): void {
    this.writeOverrides(parameterOverrides, clampFade(fadeMs ?? this.fadeMs), ParamSource.Emotion);
  }

  /**
   * WS emotion 帧入口：emotion → applyEmotion，parameterOverrides → consume。
   * 帧级 fadeMs 覆盖帧内过渡（缺省用构造器值），并钳制 [300,500]。
   */
  consumeFrame(frame: EmotionMessage): void {
    const fade = clampFade(frame.fadeMs ?? this.fadeMs);
    if (frame.emotion) {
      this.applyEmotion(frame.emotion, frame.strength, fade);
    }
    if (frame.parameterOverrides && Object.keys(frame.parameterOverrides).length > 0) {
      this.consume(frame.parameterOverrides, fade);
    }
  }

  /**
   * 按 8 表情 key + 强度（1-3）消费表情：查 EMOTION_PARAMS 表生成参数组 →
   * 写入仲裁器 6 个三轴参数（source=Emotion）。未知 emotion 回退 neutral；
   * strength 缺省/非法回退 2。记录 currentEmotion（归一化后 key）。
   */
  applyEmotion(emotion: string, strength?: number, fadeMs?: number): void {
    const key = resolveEmotionKey(emotion);
    const level = resolveStrength(strength);
    this.writeOverrides(
      EMOTION_PARAMS[key][level],
      clampFade(fadeMs ?? this.fadeMs),
      ParamSource.Emotion,
    );
    this.currentEmotion = key;
  }

  /**
   * mood 基线联动（L4，优先级低于表情 L5）。
   * mood ∈ [-100,100]（负=低落 / 正=愉悦；越界 clamp ±100，不抛异常）。
   * 0 → 不写（无 mood 偏移）；正 mood 写 MouthForm+/EyeLOpen+ 偏移；
   * 负 mood 写 BrowY-/MouthForm- 偏移；source=MoodBaseline。
   * 写入值不 clamp01——负偏移需表达负值（嘴角下弯/眉下沉），
   * 归一化交给仲裁器（mock 直接记录请求值，测试断言负值）。
   */
  updateMood(moodValue: number): void {
    const mood = Number.isFinite(moodValue)
      ? Math.min(100, Math.max(-100, moodValue))
      : 0;
    if (mood === 0) return; // 中性 → 不写 mood 基线偏移
    const t = mood / 100;
    const overrides: ParameterOverrides = {
      ParamBrowLY: 0.25 * t,
      ParamBrowRY: 0.25 * t,
      ParamEyeLOpen: 0.25 * t,
      ParamEyeROpen: 0.25 * t,
      ParamMouthForm: 0.4 * t,
      ParamMouthOpenY: 0,
    };
    this.writeOverrides(overrides, MOOD_FADE_MS, ParamSource.MoodBaseline);
  }

  /** 表情结束 → 释放 EMOTION 源对全部参数的控制（RM4-03），回退 mood/idle 基线 */
  release(): void {
    this.currentEmotion = null;
    this.arbiter.releaseAll(ParamSource.Emotion);
  }

  /** 当前表情 key（未消费过或已 release 返回 null；未知 emotion 回退 neutral） */
  getCurrentEmotion(): string | null {
    return this.currentEmotion;
  }

  /** 逐参数写入仲裁器（fadeMs 统一钳制 [300,500]；空对象 no-op） */
  private writeOverrides(
    overrides: Record<string, number>,
    fadeMs: number,
    source: ParamSource = ParamSource.Emotion,
  ): void {
    const timestamp = Date.now();
    const fade = clampFade(fadeMs);
    for (const [paramId, value] of Object.entries(overrides)) {
      const req: ParamWrite = {
        source,
        paramId,
        value: source === ParamSource.Emotion ? clamp01(value) : value,
        mode: 'set',
        fadeMs: fade,
        timestamp,
      };
      this.arbiter.write(req);
    }
  }
}
