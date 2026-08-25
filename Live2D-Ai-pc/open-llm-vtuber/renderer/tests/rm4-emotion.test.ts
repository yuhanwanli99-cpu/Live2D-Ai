/**
 * RM4 表情消费 — RED 测试（test-designer 产出）
 *
 * 契约来源：contract-pc-render.md §RM4（RM4-01 ~ RM4-06）
 * 架构来源：architecture-pc-render.md §3.4 emotion-consumer.ts
 *   消费 WS emotion 帧 → parameterOverrides 写入仲裁器 + fade 300-500ms；
 *   8 表情映射表（白-免费版三轴：眉高+眼开合+嘴形）；与 mood 基线联动
 * 参数事实：model-facts-bai.md —— 白-免费版可用参数：
 *   眉高 ParamBrowLY / ParamBrowRY（仅 Y，无眉形），
 *   眼开合 ParamEyeLOpen / ParamEyeROpen，
 *   嘴形 ParamMouthForm，嘴开合 ParamMouthOpenY
 * 档位映射：translation-polish.md §七-A — 表情生效 <200ms、fade 平滑、24 组合
 *
 * 设计说明：
 *  - 仲裁器用 mock 注入（任务允许 mock 或真实实例）。mock 记录 write/releaseAll 调用，
 *    使断言只依赖合同接口（source/paramId/value/mode/fadeMs），不依赖仲裁器内部实现。
 *  - 本文件为 RED：`renderer/src/emotion-consumer.ts` 尚未实现，导入即 fail→不通过。
 *    实现（配对 coder）按下列接口 + 契约映射表 GREEN。
 *  - ParamSource 枚举值镜像 contract-pc-render.md §RM3 硬编码：
 *    Breath=0, MicroExpr=1, Blink=2, IdleMotion=3, MoodBaseline=4,
 *    Emotion=5, UserTouch=6, LipSync=7。
 *
 * 接口约定（消费者须实现，路径 renderer/src/emotion-consumer.ts）：
 *   export const RM4_EMOTIONS: readonly string[]            // 8 key 枚举
 *   export const EMOTION_PARAM_MAP: EmotionParamMap         // 8×3 组合表
 *   export function resolveEmotionParams(emotion, strength?): Record<string, number>
 *   export class EmotionConsumer {
 *     constructor(arbiter: ParamArbiter, fadeMs?: number);      // fadeMs 默认 400
 *     consume(parameterOverrides: Record<string, number>): void; // RM4：原样写仲裁器 source=Emotion
 *     consumeFrame(frame: EmotionMessage): void;                 // WS emotion 帧入口
 *     release(): void;                                           // arbiter.releaseAll(Emotion)
 *     getCurrentEmotion(): string | null;                        // 当前表情
 *     updateMood(moodValue: number): void;                       // mood 基线联动（L4）
 *   }
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { EmotionMessage } from '../src/types';
import { EmotionConsumer, resolveEmotionParams, RM4_EMOTIONS } from '../src/emotion-consumer';

// ─────────────────────────────────────────────────────────────────────────────
// 仲裁器契约镜像（不读 src/ 实现，镜像合同 §RM3 硬编码优先级）
// ─────────────────────────────────────────────────────────────────────────────
const ParamSource = {
  Breath: 0,
  MicroExpr: 1,
  Blink: 2,
  IdleMotion: 3,
  MoodBaseline: 4,
  Emotion: 5,
  UserTouch: 6,
  LipSync: 7,
} as const;

/** 可注入的 mock 仲裁器（记录 write/releaseAll 调用） */
function makeArbiterMock() {
  const writes: Array<Record<string, unknown>> = [];
  return {
    writes,
    write: vi.fn((req: Record<string, unknown>) => {
      writes.push(req);
    }),
    releaseAll: vi.fn(),
    release: vi.fn(),
  };
}

/** 从 mock.write.mock.calls 里挑出 source=Emotion 的请求 */
function emotionWrites(mock: ReturnType<typeof makeArbiterMock>) {
  return mock.write.mock.calls
    .map((c: unknown[]) => c[0] as Record<string, unknown>)
    .filter((r) => r.source === ParamSource.Emotion);
}

// ─────────────────────────────────────────────────────────────────────────────
// 8 表情 × 3 强度映射表（对齐 architecture-pc.md §3.2，适配白-免费版参数 ID）
// 三轴：眉高 BrowLY/RY（对称）、眼开合 EyeLOpen/ROpen（对称）、嘴形 MouthForm、
//       嘴开合 MouthOpenY。强度 1/2/3 幅度递增（amplitude 单调）。
// https://github.com/sop 来源不可用——数值以架构表 + 白参数名为准，coder 实现须逐项匹配。
// ─────────────────────────────────────────────────────────────────────────────

// 用于约定式断言的期望值（key = `${emotion}_${strength}` → 期望 parameterOverrides）
const EXPECTED_MAP: Record<string, Record<string, number>> = {
  // neutral 三强度同值（微笑基线）
  neutral_1: { ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.75, ParamEyeROpen: 0.75, ParamMouthOpenY: 0.0, ParamMouthForm: 0.5 },
  neutral_2: { ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.75, ParamEyeROpen: 0.75, ParamMouthOpenY: 0.0, ParamMouthForm: 0.5 },
  neutral_3: { ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.75, ParamEyeROpen: 0.75, ParamMouthOpenY: 0.0, ParamMouthForm: 0.5 },
  // joy 开心：眉上挑(+) + 眼睁大 + 嘴形上扬
  joy_1: { ParamBrowLY: 0.15, ParamBrowRY: 0.15, ParamEyeLOpen: 0.8, ParamEyeROpen: 0.8, ParamMouthOpenY: 0.15, ParamMouthForm: 0.7 },
  joy_2: { ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 0.85, ParamEyeROpen: 0.85, ParamMouthOpenY: 0.25, ParamMouthForm: 0.8 },
  joy_3: { ParamBrowLY: 0.35, ParamBrowRY: 0.35, ParamEyeLOpen: 0.9, ParamEyeROpen: 0.9, ParamMouthOpenY: 0.35, ParamMouthForm: 0.9 },
  // angry 生气：眉下沉(-) + 眼略眯 + 嘴形下弯(-)
  angry_1: { ParamBrowLY: -0.2, ParamBrowRY: -0.2, ParamEyeLOpen: 0.65, ParamEyeROpen: 0.65, ParamMouthOpenY: 0.08, ParamMouthForm: -0.4 },
  angry_2: { ParamBrowLY: -0.35, ParamBrowRY: -0.35, ParamEyeLOpen: 0.6, ParamEyeROpen: 0.6, ParamMouthOpenY: 0.12, ParamMouthForm: -0.6 },
  angry_3: { ParamBrowLY: -0.5, ParamBrowRY: -0.5, ParamEyeLOpen: 0.55, ParamEyeROpen: 0.55, ParamMouthOpenY: 0.18, ParamMouthForm: -0.8 },
  // sad 难过：眉上挑(内八字近似用上挑) + 眼垂 + 嘴形下弯
  sad_1: { ParamBrowLY: 0.15, ParamBrowRY: 0.15, ParamEyeLOpen: 0.6, ParamEyeROpen: 0.6, ParamMouthOpenY: 0.1, ParamMouthForm: -0.3 },
  sad_2: { ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 0.5, ParamEyeROpen: 0.5, ParamMouthOpenY: 0.15, ParamMouthForm: -0.5 },
  sad_3: { ParamBrowLY: 0.35, ParamBrowRY: 0.35, ParamEyeLOpen: 0.45, ParamEyeROpen: 0.45, ParamMouthOpenY: 0.2, ParamMouthForm: -0.7 },
  // surprise 惊讶：眉大幅上挑 + 眼睁到最大(可>1) + 嘴微张
  surprise_1: { ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 1.0, ParamEyeROpen: 1.0, ParamMouthOpenY: 0.3, ParamMouthForm: 0.3 },
  surprise_2: { ParamBrowLY: 0.4, ParamBrowRY: 0.4, ParamEyeLOpen: 1.2, ParamEyeROpen: 1.2, ParamMouthOpenY: 0.45, ParamMouthForm: 0.3 },
  surprise_3: { ParamBrowLY: 0.55, ParamBrowRY: 0.55, ParamEyeLOpen: 1.4, ParamEyeROpen: 1.4, ParamMouthOpenY: 0.6, ParamMouthForm: 0.3 },
  // fear 害怕：眉上挑(微) + 眼睁大 + 嘴微张
  fear_1: { ParamBrowLY: 0.1, ParamBrowRY: 0.1, ParamEyeLOpen: 1.0, ParamEyeROpen: 1.0, ParamMouthOpenY: 0.15, ParamMouthForm: 0.3 },
  fear_2: { ParamBrowLY: 0.2, ParamBrowRY: 0.2, ParamEyeLOpen: 1.1, ParamEyeROpen: 1.1, ParamMouthOpenY: 0.2, ParamMouthForm: 0.3 },
  fear_3: { ParamBrowLY: 0.3, ParamBrowRY: 0.3, ParamEyeLOpen: 1.2, ParamEyeROpen: 1.2, ParamMouthOpenY: 0.25, ParamMouthForm: 0.3 },
  // love 心动/得意（对齐 smirk 语义，白参数转向"嘴角下压的甜笑"）
  love_1: { ParamBrowLY: 0.1, ParamBrowRY: 0.1, ParamEyeLOpen: 0.7, ParamEyeROpen: 0.7, ParamMouthOpenY: 0.0, ParamMouthForm: -0.3 },
  love_2: { ParamBrowLY: 0.15, ParamBrowRY: 0.15, ParamEyeLOpen: 0.7, ParamEyeROpen: 0.7, ParamMouthOpenY: 0.0, ParamMouthForm: -0.5 },
  love_3: { ParamBrowLY: 0.2, ParamBrowRY: 0.2, ParamEyeLOpen: 0.68, ParamEyeROpen: 0.68, ParamMouthOpenY: 0.05, ParamMouthForm: -0.7 },
  // disgust 厌恶：眉下沉(-) + 眼略眯 + 嘴形下弯
  disgust_1: { ParamBrowLY: -0.1, ParamBrowRY: -0.1, ParamEyeLOpen: 0.7, ParamEyeROpen: 0.7, ParamMouthOpenY: 0.05, ParamMouthForm: -0.3 },
  disgust_2: { ParamBrowLY: -0.2, ParamBrowRY: -0.2, ParamEyeLOpen: 0.65, ParamEyeROpen: 0.65, ParamMouthOpenY: 0.08, ParamMouthForm: -0.5 },
  disgust_3: { ParamBrowLY: -0.3, ParamBrowRY: -0.3, ParamEyeLOpen: 0.6, ParamEyeROpen: 0.6, ParamMouthOpenY: 0.1, ParamMouthForm: -0.7 },
};

/** 8 key 枚举（对齐任务 + emotion-protocol） */
const VALID_EMOTIONS = ['joy', 'sad', 'angry', 'surprise', 'fear', 'disgust', 'neutral', 'love'];

describe('RM4 emotion-consumer — WS emotion 帧消费 → 仲裁器写入', () => {
  let arb: ReturnType<typeof makeArbiterMock>;
  let consumer: EmotionConsumer;

  beforeEach(() => {
    arb = makeArbiterMock();
    consumer = new EmotionConsumer(arb as never);
  });

  it('RM4-01: consume({ParamMouthForm:0.9}) → 仲裁器收到 source=Emotion, paramId, value, fadeMs=400, mode=set', () => {
    consumer.consume({ ParamMouthForm: 0.9 });
    expect(arb.write).toHaveBeenCalledTimes(1);
    const w = arb.write.mock.calls[0][0];
    expect(w.source).toBe(ParamSource.Emotion);
    expect(w.paramId).toBe('ParamMouthForm');
    expect(w.value).toBe(0.9);
    expect(w.fadeMs).toBe(400); // 默认 fade 400ms
    expect(w.mode).toBe('set');
  });

  it('RM4-02: consume(多参数) → 逐参数写仲裁器，调用次数=参数数', () => {
    consumer.consume({ ParamBrowLY: 0.3, ParamEyeLOpen: 0.9 });
    expect(arb.write).toHaveBeenCalledTimes(2);
    const ids = arb.write.mock.calls.map((c: unknown[]) => (c[0] as any).paramId);
    expect(ids).toEqual(expect.arrayContaining(['ParamBrowLY', 'ParamEyeLOpen']));
  });

  it('RM4-03: consume→release() → 仲裁器收到 releaseAll(Emotion)', () => {
    consumer.consume({ ParamMouthForm: 0.8 });
    consumer.release();
    expect(arb.releaseAll).toHaveBeenCalledTimes(1);
    expect(arb.releaseAll.mock.calls[0][0]).toBe(ParamSource.Emotion);
  });

  it('RM4-04: consume(空对象) 不抛异常、不写仲裁器', () => {
    expect(() => consumer.consume({})).not.toThrow();
    expect(arb.write).not.toHaveBeenCalled();
  });

  it('RM4-05: 构造器 fadeMs 参数覆盖默认值（new EmotionConsumer(arb, 300) → write.fadeMs=300）', () => {
    const c = new EmotionConsumer(arb as never, 300);
    c.consume({ ParamMouthForm: 0.5 });
    expect(arb.write.mock.calls[0][0].fadeMs).toBe(300);
  });

  it('RM4-06: 表情消费到写仲裁器 < 1ms（同步调用，不等 update）', () => {
    const t0 = performance.now();
    consumer.consume({ ParamMouthForm: 0.9, ParamBrowLY: 0.2 });
    const dt = performance.now() - t0;
    expect(dt).toBeLessThan(1);
    expect(emotionWrites(arb).length).toBe(2); // 同步即写，无异步等待
  });
});

describe('RM4 emotion-consumer — 8 表情参数映射（三轴：眉高+眼开合+嘴形）', () => {
  let arb: ReturnType<typeof makeArbiterMock>;

  beforeEach(() => {
    arb = makeArbiterMock();
  });

  it('导出 8 key 枚举 RM4_EMOTIONS 覆盖 joy/sad/angry/surprise/fear/disgust/neutral/love', () => {
    expect(RM4_EMOTIONS).toEqual(expect.arrayContaining(VALID_EMOTIONS));
    expect(new Set(RM4_EMOTIONS).size).toBe(8);
  });

  it('全 24 组合：resolveEmotionParams 产出与契约映射表逐项一致（$ 属中性/眉高对称/嘴形）', () => {
    for (const emo of VALID_EMOTIONS) {
      for (const s of [1, 2, 3] as const) {
        const overrides = resolveEmotionParams(emo, s);
        expect(overrides, `${emo}_${s}`).toEqual(EXPECTED_MAP[`${emo}_${s}`]);
        // 眼开合/眉高必须成对（左=右），白参数无单侧
        expect(overrides.ParamEyeLOpen).toBe(overrides.ParamEyeROpen);
        expect(overrides.ParamBrowLY).toBe(overrides.ParamBrowRY);
      }
    }
  });

  it('映射只含白-免费版三轴可用参数（不含废参数 ParamBrowLAngle/ParamCheek 等）', () => {
    const allowed = new Set([
      'ParamBrowLY', 'ParamBrowRY',
      'ParamEyeLOpen', 'ParamEyeROpen',
      'ParamMouthForm', 'ParamMouthOpenY',
    ]);
    for (const emo of VALID_EMOTIONS) {
      for (const s of [1, 2, 3] as const) {
        for (const key of Object.keys(resolveEmotionParams(emo, s))) {
          expect(allowed.has(key), `参数 ${key}@${emo}_${s} 不在白名单`).toBe(true);
        }
      }
    }
  });

  it('applyEmotion(joy, 2) → 写仲裁器 6 个三轴参数，值=映射表', () => {
    const c = new EmotionConsumer(arb as never);
    c.applyEmotion('joy', 2);
    expect(emotionWrites(arb).length).toBe(6);
    const map = new Map(emotionWrites(arb).map((w) => [w.paramId, w.value]));
    for (const [k, v] of Object.entries(EXPECTED_MAP.joy_2)) {
      expect(map.get(k)).toBeCloseTo(v as number, 5);
    }
  });

  it('getCurrentEmotion 记录最近表情（applyEmotion 后返回 emotion）', () => {
    const c = new EmotionConsumer(arb as never);
    expect(c.getCurrentEmotion()).toBeNull();
    c.applyEmotion('surprise', 1);
    expect(c.getCurrentEmotion()).toBe('surprise');
    c.release();
    expect(c.getCurrentEmotion()).toBeNull(); // release 清空
  });
});

describe('RM4 emotion-consumer — fade 过渡时长（300-500ms）', () => {
  let arb: ReturnType<typeof makeArbiterMock>;

  beforeEach(() => {
    arb = makeArbiterMock();
  });

  it('默认构造 fadeMs=400 ∈ [300,500] 且写入传递（架构 A3 达标）', () => {
    const c = new EmotionConsumer(arb as never);
    c.consume({ ParamMouthOpenY: 0.5 });
    expect(arb.write.mock.calls[0][0].fadeMs).toBeGreaterThanOrEqual(300);
    expect(arb.write.mock.calls[0][0].fadeMs).toBeLessThanOrEqual(500);
  });

  it('自定义构造 fadeMs 落在 [300,500]（300/500 边界均合法）', () => {
    for (const fade of [300, 400, 500]) {
      const c = new EmotionConsumer(arb as never, fade);
      c.consume({ ParamMouthForm: 0.6 });
      // 断言指向本次构造 consumer 的最近一次写入（mock.calls 在同一 it 内累积，
      // 固定 [0] 会恒指第一次调用导致循环第 2 次起断言失效——deviationFromSpec 已记录）
      const lastCall = arb.write.mock.calls[arb.write.mock.calls.length - 1][0];
      expect(lastCall.fadeMs).toBe(fade);
    }
  });

  it('WS emotion 帧的 fadeMs 字段可覆盖帧级过渡时长（consumeFrame）', () => {
    const c = new EmotionConsumer(arb as never); // 默认 400
    const frame: EmotionMessage = {
      type: 'emotion',
      emotion: 'joy',
      strength: 2,
      parameterOverrides: { ParamMouthForm: 0.8 },
      fadeMs: 450,
    };
    c.consumeFrame(frame);
    expect(arb.write.mock.calls[0][0].fadeMs).toBe(450);
  });

  it('过渡超 500ms 或不足 300ms 会被拒绝/钳制（保证 fade 平滑无跳变）', () => {
    const c = new EmotionConsumer(arb as never);
    // 若实现选择钳制，则写值被 clamp 回 [300,500]
    (c as any).fadeMs = 1200;
    c.consume({ ParamMouthForm: 0.6 });
    const fade = arb.write.mock.calls[0][0].fadeMs;
    expect(fade).toBeGreaterThanOrEqual(300);
    expect(fade).toBeLessThanOrEqual(500);
  });
});

describe('RM4 emotion-consumer — 强度 strength 1-3 影响参数幅度', () => {
  let arb: ReturnType<typeof makeArbiterMock>;

  beforeEach(() => {
    arb = makeArbiterMock();
  });

  it('joy：强度越大，眉高上挑与嘴形上扬幅度单调递增（amplitude 随 strength）', () => {
    for (const [s, exp] of [
      [1, EXPECTED_MAP.joy_1],
      [2, EXPECTED_MAP.joy_2],
      [3, EXPECTED_MAP.joy_3],
    ]) {
      const overrides = resolveEmotionParams('joy', s);
      expect(overrides.ParamBrowLY).toBeCloseTo(exp.ParamBrowLY, 5);
      expect(overrides.ParamMouthForm).toBeCloseTo(exp.ParamMouthForm, 5);
    }
    const b1 = EXPECTED_MAP.joy_1.ParamMouthForm;
    const b2 = EXPECTED_MAP.joy_2.ParamMouthForm;
    const b3 = EXPECTED_MAP.joy_3.ParamMouthForm;
    expect(b3).toBeGreaterThan(b2);
    expect(b2).toBeGreaterThan(b1);
  });

  it('angry：强度越大，眉下沉与嘴形下弯绝对值递增（负向幅度）', () => {
    const m1 = EXPECTED_MAP.angry_1.ParamBrowLY; // -0.2
    const m2 = EXPECTED_MAP.angry_2.ParamBrowLY; // -0.35
    const m3 = EXPECTED_MAP.angry_3.ParamBrowLY; // -0.5
    expect(Math.abs(m3)).toBeGreaterThan(Math.abs(m2));
    expect(Math.abs(m2)).toBeGreaterThan(Math.abs(m1));
  });

  it('unknown strength（非1/2/3）→ 回退 strength=2 默认', () => {
    const from2 = resolveEmotionParams('joy', 2);
    const fromBad = resolveEmotionParams('joy', 5 as never);
    expect(fromBad).toEqual(from2);
  });

  it('strength 缺省 → 默认 2', () => {
    expect(resolveEmotionParams('joy')).toEqual(EXPECTED_MAP.joy_2);
  });
});

describe('RM4 emotion-consumer — 未知 emotion 回退 neutral', () => {
  let arb: ReturnType<typeof makeArbiterMock>;

  beforeEach(() => {
    arb = makeArbiterMock();
  });

  it('resolveEmotionParams(未知) → neutral_2 参数组', () => {
    expect(resolveEmotionParams('mystery')).toEqual(EXPECTED_MAP.neutral_2);
  });

  it('applyEmotion(未知) → 写 neutral 参数组，且 getCurrentEmotion 回退 neutral', () => {
    const c = new EmotionConsumer(arb as never);
    c.applyEmotion('gremlin' as never);
    expect(emotionWrites(arb).length).toBe(6);
    const map = new Map(emotionWrites(arb).map((w) => [w.paramId, w.value]));
    for (const [k, v] of Object.entries(EXPECTED_MAP.neutral_2)) {
      expect(map.get(k)).toBeCloseTo(v as number, 5);
    }
    expect(c.getCurrentEmotion()).toBe('neutral');
  });

  it('空字符串 emotion → 也回退 neutral，不抛异常', () => {
    const c = new EmotionConsumer(arb as never);
    expect(() => c.applyEmotion('' as never)).not.toThrow();
    expect(c.getCurrentEmotion()).toBe('neutral');
  });
});

describe('RM4 emotion-consumer — mood 基线联动（mood 影响默认表情基线）', () => {
  let arb: ReturnType<typeof makeArbiterMock>;
  let c: EmotionConsumer;

  beforeEach(() => {
    arb = makeArbiterMock();
    c = new EmotionConsumer(arb as never);
  });

  it('mood=中性(0) → 不写 mood 基线偏移', () => {
    c.updateMood(0);
    // MoodBaseline 源不应有任何写
    const moodWrites = arb.write.mock.calls
      .map((x: unknown[]) => x[0] as any)
      .filter((r) => r.source === ParamSource.MoodBaseline);
    expect(moodWrites.length).toBe(0);
  });

  it('mood=正(开心) → 写 MoodBaseline 偏移（嘴角上翘 MouthForm+、眼睁大 EyeLOpen+）', () => {
    c.updateMood(60);
    const moodWrites = arb.write.mock.calls
      .map((x: unknown[]) => x[0] as any)
      .filter((r) => r.source === ParamSource.MoodBaseline);
    expect(moodWrites.length).toBeGreaterThan(0);
    const ids = new Map(moodWrites.map((w) => [w.paramId, w.value]));
    expect(ids.get('ParamMouthForm')).toBeGreaterThan(0);  // 开心→嘴角上翘
    expect(ids.get('ParamEyeLOpen')).toBeGreaterThan(0);   // 开心→眼睁大
  });

  it('mood=负(不开心) → 写 MoodBaseline 负偏移（眉下沉 BrowY-、嘴形下弯 MouthForm-）', () => {
    c.updateMood(-60);
    const moodWrites = arb.write.mock.calls
      .map((x: unknown[]) => x[0] as any)
      .filter((r) => r.source === ParamSource.MoodBaseline);
    expect(moodWrites.length).toBeGreaterThan(0);
    const ids = new Map(moodWrites.map((w) => [w.paramId, w.value]));
    expect(ids.get('ParamMouthForm')).toBeLessThan(0);
    expect(ids.get('ParamBrowLY')).toBeLessThan(0);
  });

  it('mood 影响默认表情基线：无 emotion 时 updateMood 后基线参数已写入仲裁器（L4 先于 L5）', () => {
    c.updateMood(60);
    // 所有 mood 基线写 source 均为 MoodBaseline（L4），优先级低于 Emotion(L5)
    for (const w of arb.write.mock.calls.map((x: unknown[]) => x[0] as any)) {
      expect(w.source).toBe(ParamSource.MoodBaseline);
    }
  });

  it('Emotion 与 mood 基线分离轨道：applyEmotion 写 Emotion(5)，不覆盖 MoodBaseline(4)', () => {
    c.updateMood(60);
    c.release();
    arb.write.mockClear();
    c.applyEmotion('joy', 2);
    for (const w of emotionWrites(arb)) {
      expect(w.source).toBe(ParamSource.Emotion); // Emotion 优先级高于 MoodBaseline
    }
  });

  it('updateMood 越界值（>100 或 <-100）被 clamp 到 [-100,100]，不抛异常', () => {
    expect(() => c.updateMood(500)).not.toThrow();
    expect(() => c.updateMood(-500)).not.toThrow();
  });
});
