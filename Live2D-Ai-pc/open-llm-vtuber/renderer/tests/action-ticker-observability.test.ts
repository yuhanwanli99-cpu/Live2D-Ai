/**
 * action-ticker-observability.test.ts
 *
 * 端到端动作可观测性测试：
 * - 用户实测「dev-action / audio 动作都到后端，但 Live2D 肉眼无动作」。
 * - 此前 choreography.test.ts 用 FakeArbiter 只验证「函数被调用、写到 arbiter 记录」，
 *   从不驱动真实 ParamArbiter.update() 到 FakeCoreModel，因此无法证明「模型参数真的变了」。
 * - 本测试改用真实 ParamArbiter + FakeCoreModel，模拟 ticker 逐帧推进：
 *      play() → choreo.pump()（写 arbiter, LiveEvent 最高优先）
 *      → arbiter.update(delta)（lerp fade → normalizedToModelValue → setParameterValueById 写 coreModel）
 *   断言 coreModel 中 ParamAngleX/ParamBodyAngleX 等「确实从中性偏离」且「末帧回中性」。
 *
 * 数值空间说明：arbiter.update 对 valueSpace:'normalized' 的源，把归一化 0..1 映射到模型
 * 真实范围（Fake 范围 -30..30，默认 0）。因此归一化 0.5 == 模型 0（中性）。本测试以模型
 * 默认值 0 为中性基准，断言写入的模型值偏离该基准。
 */
import { describe, it, expect } from 'vitest';
import { BodyActionPlayer, CHOREOGRAPHY_LIBRARY } from '../src/choreography';
import { ParamArbiter, ParamSource } from '../src/param-arbiter';

/** 记录 setParameterValueById 写入的 FakeCoreModel（构造出 arbiter 需要的参数范围） */
interface FakeParamInfo {
  minimum: number;
  maximum: number;
  defaultValue: number;
}

const RANGES: Record<string, FakeParamInfo> = {
  ParamAngleX: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamAngleY: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamAngleZ: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamBodyAngleX: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamBodyAngleY: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamBodyAngleZ: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamEyeBallX: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamEyeBallY: { minimum: -1, maximum: 1, defaultValue: 0 },
};

class FakeCoreModel {
  ids: string[] = Object.keys(RANGES);
  writes: Array<{ id: string; value: number }> = [];
  getParameterCount(): number {
    return this.ids.length;
  }
  getParameterId(i: number): string {
    return this.ids[i];
  }
  getParameterMinimumValue(i: number): number {
    return RANGES[this.ids[i]].minimum;
  }
  getParameterMaximumValue(i: number): number {
    return RANGES[this.ids[i]].maximum;
  }
  getParameterDefaultValue(i: number): number {
    return RANGES[this.ids[i]].defaultValue;
  }
  getModel(): unknown {
    return undefined;
  }
  setParameterValueById(id: string, value: number): void {
    this.writes.push({ id, value });
  }
  last(id: string): number | undefined {
    for (let i = this.writes.length - 1; i >= 0; i--) {
      if (this.writes[i].id === id) return this.writes[i].value;
    }
    return undefined;
  }
}

interface FakeInternalModel {
  coreModel: FakeCoreModel;
}

/** 关键动作参数模型默认值 = 中性（本 Fake 均为 0） */
const NEUTRAL = 0;

/** 把模型值偏离（绝对值）折算成归一化偏离（范围 -30..30 → 除 30），便于表达“幅度阈值” */
function normalizedDeviation(modelValue: number): number {
  return Math.abs(modelValue - NEUTRAL) / 30;
}

function makeTicker() {
  let t = 0;
  return {
    now: () => t,
    /** 推进时钟 + 推进一次 choreo/arbiter，模拟一个渲染 tick */
    tick: (arb: ParamArbiter, bap: BodyActionPlayer, deltaMs: number) => {
      t += deltaMs;
      bap.update(deltaMs); // choreo pump（用 now() 判定到点关键帧）
      arb.update(deltaMs); // lerp fade → 写 coreModel
    },
  };
}

/** fade 收敛需要的 tick 数（nod fade 上限 300ms；给足余量） */
const CONFIDENCE_TICKS = 40;

describe('动作端到端可观测性：真实 arbiter → FakeCoreModel', () => {
  it('dev-action 一次性动作 nod 让 coreModel 参数实际偏离并回中', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now });

    // dev-action 入径：ws-bridge dev-action → bodyMotion.play('nod', 2)
    bap.play('nod', 2);
    expect(arb.getCurrentSource('ParamAngleX')).toBe(ParamSource.LiveEvent);

    let peakAngleX = 0;
    for (let i = 0; i < CONFIDENCE_TICKS * 5; i++) {
      ticker.tick(arb, bap, 16);
      const v = internalModel.coreModel.last('ParamAngleX') ?? NEUTRAL;
      const dev = normalizedDeviation(v);
      if (dev > peakAngleX) peakAngleX = dev;
    }
    // nod 首帧 0.06、次帧 0.94（归一化）→ 模型值明显偏离中性
    expect(peakAngleX).toBeGreaterThan(0.4);
    // 播完自动释放 → LiveEvent 源移除 → 无更低源 → 回中性（模型 0）
    expect(arb.getCurrentSource('ParamAngleX')).toBe(-1);
    expect(internalModel.coreModel.last('ParamAngleX')).toBeCloseTo(NEUTRAL, 2);
  });

  it('audio 一次性 motion<wave_big> 播放后释放回中性不留残影', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now });

    bap.play('wave_big', 3); // audio.actions.motion 入径
    let peakBodyZ = 0;
    for (let i = 0; i < CONFIDENCE_TICKS * 6; i++) {
      ticker.tick(arb, bap, 16);
      const v = internalModel.coreModel.last('ParamBodyAngleZ') ?? NEUTRAL;
      peakBodyZ = Math.max(peakBodyZ, normalizedDeviation(v));
    }
    expect(peakBodyZ).toBeGreaterThan(0.4); // 曾大幅左右摆动
    expect(arb.getCurrentSource('ParamBodyAngleZ')).toBe(-1);
    expect(internalModel.coreModel.last('ParamBodyAngleZ')).toBeCloseTo(NEUTRAL, 2);
  });

  it('强度 3 峰值入参不越界、输出模型值有限且在合法范围', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now });

    bap.play('bow', 3);
    let sawWrite = false;
    for (let i = 0; i < CONFIDENCE_TICKS * 5; i++) {
      ticker.tick(arb, bap, 16);
      if (internalModel.coreModel.writes.length > 0) sawWrite = true;
    }
    expect(sawWrite).toBe(true);
    for (const w of internalModel.coreModel.writes) {
      expect(Number.isFinite(w.value)).toBe(true);
      expect(w.value).toBeGreaterThanOrEqual(RANGES[w.id].minimum);
      expect(w.value).toBeLessThanOrEqual(RANGES[w.id].maximum);
    }
  });

  it('编排模板末帧全部为归一化 0.5（中性）', () => {
    for (const [name, frames] of Object.entries(CHOREOGRAPHY_LIBRARY)) {
      const last = frames[frames.length - 1].params;
      for (const v of Object.values(last)) {
        expect(v).toBeCloseTo(0.5, 4);
      }
      void name;
    }
  });
});

describe('BodyActionPlayer×ParamArbiter 契约', () => {
  it('play 后 getCurrentSource 为 LiveEvent，release 后回到 -1', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const bap = new BodyActionPlayer(arb, { now: makeTicker().now });
    bap.play('tilt', 2);
    expect(arb.getCurrentSource('ParamAngleZ')).toBe(ParamSource.LiveEvent);
    bap.release();
    expect(arb.getCurrentSource('ParamAngleZ')).toBe(-1);
  });
});
