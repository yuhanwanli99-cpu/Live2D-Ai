/**
 * helpers/fake-core-model.ts — 真实形状的 Cubism4 coreModel 测试替身。
 *
 * 与真实 live2dcubismcore coreModel 的渲染器侧接口同构：
 *   - getParameterCount / getParameterId(i) / getParameterMinimumValue(i) /
 *     getParameterMaximumValue(i) / getParameterDefaultValue(i)
 *   - setParameterValueById(id, value)（记录写入，供断言）
 *   - getModel().parameters.{ids,minimumValues,maximumValues,defaultValues}
 *     （buildHalfBodyProfile 的第二取数路径；对齐真实 core 的 TypedArray 形状）
 *
 * 参数范围取 bai 模型的真实形状：头部 ±30°（默认 0）、眼球 ±1（默认 0）、
 * 眼开合 -0.1..1.2（默认 1 = 睁眼！）、眉 ±1、嘴 [0,1]/[-1,1]、呼吸 [0,1]、
 * 嘟嘴 Param4 [0,1]（默认 0）、歪嘴 Param6 [-1,1]（默认 0）——与 bai.cdi3.json
 * 的「嘟嘴/歪嘴」通道同构。
 * 故意包含 ParamBodyAngleX/Y/Z（模型存在这些通道）——用于断言动作系统绝不写身体。
 */

export interface FakeParamInfo {
  minimum: number;
  maximum: number;
  defaultValue: number;
}

/** bai 类模型典型参数范围（含被禁止的身体通道，证明其永不被写入） */
export const BAI_LIKE_RANGES: Record<string, FakeParamInfo> = {
  ParamAngleX: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamAngleY: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamAngleZ: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamEyeLOpen: { minimum: -0.1, maximum: 1.2, defaultValue: 1 }, // 默认即睁眼，不是 0.5
  ParamEyeROpen: { minimum: -0.1, maximum: 1.2, defaultValue: 1 },
  ParamEyeBallX: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamEyeBallY: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamBrowLY: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamBrowRY: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamMouthOpenY: { minimum: 0, maximum: 1, defaultValue: 0 },
  ParamMouthForm: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamBreath: { minimum: 0, maximum: 1, defaultValue: 0.5 },
  // bai 非标面部通道：嘟嘴(Param4)/歪嘴(Param6)——能力门控动作 pout/wry_mouth 依赖
  Param4: { minimum: 0, maximum: 1, defaultValue: 0 }, // 嘟嘴
  Param5: { minimum: 0, maximum: 1, defaultValue: 0 }, // Mouth Pucker Widen（模型存在，动作不用）
  Param6: { minimum: -1, maximum: 1, defaultValue: 0 }, // 歪嘴
  // 身体通道：模型存在，但动作定版后禁止任何写入
  ParamBodyAngleX: { minimum: -10, maximum: 10, defaultValue: 0 },
  ParamBodyAngleY: { minimum: 0, maximum: 1, defaultValue: 0 },
  ParamBodyAngleZ: { minimum: -10, maximum: 10, defaultValue: 0 },
};

export class FakeCoreModel {
  readonly ids: string[];
  readonly writes: Array<{ id: string; value: number }> = [];

  constructor(ranges: Record<string, FakeParamInfo> = BAI_LIKE_RANGES) {
    this.ids = Object.keys(ranges);
    this.ranges = ranges;
  }

  readonly ranges: Record<string, FakeParamInfo>;

  getParameterCount(): number {
    return this.ids.length;
  }

  getParameterId(index: number): string {
    return this.ids[index];
  }

  getParameterMinimumValue(index: number): number {
    return this.ranges[this.ids[index]].minimum;
  }

  getParameterMaximumValue(index: number): number {
    return this.ranges[this.ids[index]].maximum;
  }

  getParameterDefaultValue(index: number): number {
    return this.ranges[this.ids[index]].defaultValue;
  }

  /** 对齐真实 core 的批量元数据接口（buildHalfBodyProfile 兜底路径） */
  getModel(): {
    parameters: {
      ids: string[];
      minimumValues: number[];
      maximumValues: number[];
      defaultValues: number[];
    };
  } {
    return {
      parameters: {
        ids: this.ids,
        minimumValues: this.ids.map((id) => this.ranges[id].minimum),
        maximumValues: this.ids.map((id) => this.ranges[id].maximum),
        defaultValues: this.ids.map((id) => this.ranges[id].defaultValue),
      },
    };
  }

  setParameterValueById(id: string, value: number): void {
    this.writes.push({ id, value });
  }

  /** 某参数最后一次写入的模型值（从未写过 → undefined） */
  last(id: string): number | undefined {
    for (let i = this.writes.length - 1; i >= 0; i--) {
      if (this.writes[i].id === id) return this.writes[i].value;
    }
    return undefined;
  }

  /** 某参数全部写入序列 */
  history(id: string): number[] {
    return this.writes.filter((w) => w.id === id).map((w) => w.value);
  }

  /** 曾被写入过的参数集合 */
  writtenParams(): Set<string> {
    return new Set(this.writes.map((w) => w.id));
  }
}

/** 可控时钟 + 渲染 tick（先推进 BodyActionPlayer/编排泵，再推进仲裁器 fade） */
export function makeTicker() {
  let t = 1000;
  return {
    now: () => t,
    advanceMs: (ms: number) => {
      t += ms;
    },
    tick: (arb: { update: (ms: number) => void }, bap: { update: (ms: number) => void }, deltaMs = 16) => {
      t += deltaMs;
      bap.update(deltaMs);
      arb.update(deltaMs);
    },
  };
}
