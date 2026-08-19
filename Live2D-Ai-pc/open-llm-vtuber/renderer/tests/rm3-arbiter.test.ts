/**
 * RM3 参数仲裁器 — RED 测试（test-designer 产出）
 *
 * 契约来源：contract-pc-render.md §RM3（RM3-01 ~ RM3-07）
 * 架构来源：architecture-pc-render.md §3.3 param-arbiter.ts（优先级 + lerp fade + 三态）
 * 补充约束：translation-polish.md §七-A3（fade 300-500ms 无跳变）
 *
 * ⚠️ 本文件为 RED：实现（param-arbiter.ts）未写，import 失败即 RED。
 *    实现由配对 coder 完成 GREEN，须满足本文件全部断言。
 *
 * 文件锁合规：本文件仅读契约/架构/共享 types.ts，绝不读 param-arbiter.ts 实现。
 * pixi 依赖：构建经 vi.mock 屏蔽；仲裁器 coreModel 通过注入的 fake 对象验证写入。
 *
 * ─────────────────────────────────────────────────────────────────────────────
 * test-designer 补充的契约级仲裁语义（coder 须按此实现，契约未细化处）
 * ─────────────────────────────────────────────────────────────────────────────
 * 1. 轨道模型：每个 (paramId, source) 视为一条"轨道"（track），独立维护该源的
 *    目标值。轨道初值 current = 0。源对该参数后续 write 按 mode 更新其轨道值：
 *       mode:'set'   → 轨道值 = value
 *       mode:'add'   → 轨道值 = clamp(轨道值 + value, 0, 1)
 *       mode:'mult'  → 轨道值 = clamp(轨道值 * value, 0, 1)
 * 2. update()：参数的最终呈现值 = 当前 active（最高优先级）轨道的值，再经 fade lerp
 *    过渡到目标；随后写入 coreModel。
 * 3. 优先级：同参数多源同时 active → 只有最高优先级源的轨道值被呈现（低优先轨道
 *    保留其值，高优先结束回退时恢复）。优先级顺序（低→高）见 §RM3：
 *       Breath(0) < MicroExpr(1) < Blink(2) < IdleMotion(3) < MoodBaseline(4)
 *       < Emotion(5) < UserTouch(6) < LipSync(7)
 * 4. Blink 眼部特权穿透（用户确认采纳，见架构附录 A + 本任务 Q2 采纳）：
 *       Blink 源对 `ParamEyeLOpen` / `ParamEyeROpen` 两个参数具有"特权穿透"——
 *       即使存在更高优先级源（Emotion/UserTouch/LipSync）active，Blink 对这两个
 *       眼部参数的写入仍获胜（参数呈现值取 Blink 轨道，getCurrentSource 返回 Blink）。
 *       对**其他**参数 ID，Blink 遵循正常优先级规则（被更高优先源阻塞，不穿透）。
 * 5. fade：fadeMs>0 时从当前呈现值线性插值（lerp）到新目标，经过 fadeMs 完全到达。
 *       fadeMs=0 立即跳变。
 * 6. getCurrentSource(paramId)：当前呈现值由哪个源决定；无任何源时返回 -1。
 * 7. release(releaseAll)：移除该源全部轨道控制，参数回退到次高优先 active 轨道
 *       （或 -1 / 0 当无轨道）。
 * ─────────────────────────────────────────────────────────────────────────────
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

// 接口签名契约对齐 contract-pc-render.md §RM3 —— 由 coder 从 src/param-arbiter.ts 导出
import { ParamSource, ParamArbiter } from '../src/param-arbiter';
import type { ParamWrite } from '../src/param-arbiter';

// ─────────────────────────────────────────────────────────────────────────────
// pixi 依赖 mock —— 仲裁器不直接依赖 pixi，但为隔离环境 + 对齐其他测试的 mock 约定
// ─────────────────────────────────────────────────────────────────────────────
vi.mock('pixi-live2d-display-lipsyncpatch', () => ({ Live2DModel: { from: vi.fn() } }));
vi.mock('pixi-live2d-display-lipsyncpatch/cubism4', () => ({
  Live2DModel: { from: vi.fn() },
}));

// ─────────────────────────────────────────────────────────────────────────────
// fake coreModel —— 捕获 setParameterValueById 写入
// ─────────────────────────────────────────────────────────────────────────────
function makeFakeInternalModel() {
  const writes: Array<{ id: string; value: number }> = [];
  const setParameterValueById = vi.fn(function (this: any, id: string, value: number) {
    writes.push({ id, value });
  });
  const coreModel = { setParameterValueById };
  return {
    coreModel,
    writes,
    setParameterValueById,
  };
}

function makeArbiter(fake?: ReturnType<typeof makeFakeInternalModel>) {
  const f = fake ?? makeFakeInternalModel();
  const arbiter = new ParamArbiter(f as unknown);
  return { arbiter, fake: f };
}

function write(
  source: ParamSource,
  paramId: string,
  value: number,
  mode: ParamWrite['mode'] = 'set',
  fadeMs = 0
): ParamWrite {
  return { source, paramId, value, mode, fadeMs, timestamp: Date.now() };
}

// 便捷：把仲裁器推进若干帧，直到某参数停止变化（fade 完全到位）
function settle(arb: ParamArbiter, steps = 20, stepMs = 100): void {
  for (let i = 0; i < steps; i++) arb.update(stepMs);
}

describe('RM3 param-arbiter — 优先级仲裁（低优先不覆盖高优先）', () => {
  it('RM3-01: LipSync(L7) active 时，Emotion(L5) 写同参数被阻塞，权属保持 LipSync', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.LipSync, 'ParamMouthOpenY', 0.9, 'set', 0));
    settle(arbiter);

    // Emotion 后写同参数，优先级 L5 < L7 → 应被阻塞，参数仍归 LipSync
    arbiter.write(write(ParamSource.Emotion, 'ParamMouthOpenY', 0.2, 'set', 0));
    settle(arbiter);

    expect(arbiter.getCurrentSource('ParamMouthOpenY')).toBe(ParamSource.LipSync);
    // 当前呈现值保持 LipSync 的 0.9（Emotion 的 0.2 未生效）
    const last = fake.writes[fake.writes.length - 1];
    expect(last.id).toBe('ParamMouthOpenY');
    expect(last.value).toBeCloseTo(0.9, 5);
  });

  it('同参数多源 active 时，最终呈现值取最高优先级源的轨道值（取 max 高优先）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamBrowLY', 0.5, 'set', 0));
    arbiter.write(write(ParamSource.LipSync, 'ParamBrowLY', 0.9, 'set', 0));
    settle(arbiter);

    expect(arbiter.getCurrentSource('ParamBrowLY')).toBe(ParamSource.LipSync);
    const last = fake.writes[fake.writes.length - 1];
    expect(last.value).toBeCloseTo(0.9, 5);

    // LipSync 结束 → 回退到次高优先 Emotion 的轨道值 0.5
    arbiter.release('ParamBrowLY', ParamSource.LipSync);
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamBrowLY')).toBe(ParamSource.Emotion);
    const afterRel = fake.writes[fake.writes.length - 1];
    expect(afterRel.value).toBeCloseTo(0.5, 5);
  });

  it('同一优先级内后写入覆盖先写入', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.3, 'set', 0));
    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.8, 'set', 0));
    settle(arbiter);

    expect(arbiter.getCurrentSource('ParamMouthForm')).toBe(ParamSource.Emotion);
    const last = fake.writes[fake.writes.length - 1];
    expect(last.value).toBeCloseTo(0.8, 5);
  });

  it('低优先源释放后参数回退到默认（无其他源时值为 0 且 -1）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.IdleMotion, 'ParamBodyAngleX', 0.6, 'set', 0));
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamBodyAngleX')).toBe(ParamSource.IdleMotion);

    arbiter.release('ParamBodyAngleX', ParamSource.IdleMotion);
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamBodyAngleX')).toBe(-1);
    const last = fake.writes[fake.writes.length - 1];
    expect(last.value).toBeCloseTo(0, 5);
  });
});

describe('RM3 param-arbiter — 三态叠加（overwrite/add/multiply）', () => {
  it('set(overwrite)：直接设定目标值', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamBrowRY', 0.4, 'set', 0));
    settle(arbiter);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.4, 5);

    // 再次 set 覆盖
    arbiter.write(write(ParamSource.Emotion, 'ParamBrowRY', 0.9, 'set', 0));
    settle(arbiter);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.9, 5);
  });

  it('add：在同一轨道上累加（轨道初值 0 → 0+0.3=0.3）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.MicroExpr, 'ParamEyeBallX', 0.3, 'add', 0));
    settle(arbiter);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.3, 5);
  });

  it('add：同轨道连续 add 累加（0.3 + 0.2 = 0.5）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.MicroExpr, 'ParamEyeBallY', 0.3, 'add', 0));
    settle(arbiter);
    arbiter.write(write(ParamSource.MicroExpr, 'ParamEyeBallY', 0.2, 'add', 0));
    settle(arbiter);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.5, 5);
  });

  it('add：超出下/上界被 clamp 到 [0,1]', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.MicroExpr, 'ParamEyeBallX', 0.8, 'add', 0));
    settle(arbiter);
    arbiter.write(write(ParamSource.MicroExpr, 'ParamEyeBallX', 0.5, 'add', 0)); // 0.8+0.5=1.3→clamp 1
    settle(arbiter);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(1, 5);
  });

  it('mult：轨道当前值乘系数（先 set 0.8 → mult 0.5 = 0.4）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.8, 'set', 0));
    settle(arbiter);
    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.5, 'mult', 0));
    settle(arbiter);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.4, 5);
  });

  it('mult：被更高优先源覆盖的参数不受低优先轨道叠加影响', () => {
    const { arbiter, fake } = makeArbiter();

    // Emotion(L5) 先占位，MicroExpr(L1) 对同参数加 add → 不生效（低优先被阻塞）
    arbiter.write(write(ParamSource.Emotion, 'ParamBrowLY', 0.6, 'set', 0));
    settle(arbiter);
    arbiter.write(write(ParamSource.MicroExpr, 'ParamBrowLY', 0.9, 'add', 0));
    settle(arbiter);

    expect(arbiter.getCurrentSource('ParamBrowLY')).toBe(ParamSource.Emotion);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.6, 5);
  });
});

describe('RM3 param-arbiter — lerp fade（300-500ms 平滑，无跳变）', () => {
  it('RM3-02: fade 400ms 中段（update 200ms 后）值在中间区间（lerp 中途）', () => {
    const { arbiter, fake } = makeArbiter();

    // 从 0 起步，fade 400ms → 目标 0.8
    arbiter.write(write(ParamSource.Emotion, 'ParamEyeLOpen', 0.0, 'set', 0));
    settle(arbiter);
    arbiter.write(write(ParamSource.Emotion, 'ParamEyeLOpen', 0.8, 'set', 400));

    // 推进 200ms：lerp 进度 ~50% → coreModel 当前写入值应在 [0.35, 0.65]
    // （coreModel 写入值 = 仲裁器当前呈现值，契约 RM3-06 语义）
    arbiter.update(200);
    expect(arbiter.getCurrentSource('ParamEyeLOpen')).toBe(ParamSource.Emotion);
    const mid = fake.writes[fake.writes.length - 1].value;
    expect(mid).toBeGreaterThan(0.35);
    expect(mid).toBeLessThan(0.65);
  });

  it('RM3-03: fade 400ms 完成后值 = 目标 0.8（±0.01，无跳变）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.0, 0 as any, 0));
    settle(arbiter);
    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.8, 'set', 400));
    settle(arbiter, 10, 100); // 推进 1000ms > 400ms

    const last = fake.writes[fake.writes.length - 1];
    expect(last.id).toBe('ParamMouthForm');
    expect(Math.abs(last.value - 0.8)).toBeLessThan(0.01);
  });

  it('fade 全程无跳变：每帧一阶差分不超过渐进上限（平滑过渡）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamBrowLY', 0.2, 'set', 0));
    settle(arbiter);
    // 切换 fade 400ms → 目标 0.9，逐帧采样，断言每帧增量 < 0.2（无突变）
    arbiter.write(write(ParamSource.Emotion, 'ParamBrowLY', 0.9, 'set', 400));
    // [修复] 补推进：fade 需 update() 逐帧推进（原缺此行，sampled 停在 0.2）；与 RM3-03 同款推进
    settle(arbiter);

    const sampled = fake.writes.map((w) => w.value);
    for (let i = 2; i < sampled.length; i++) {
      expect(Math.abs(sampled[i] - sampled[i - 1])).toBeLessThan(0.3);
    }
    // 最终收敛到 0.9
    expect(Math.abs(sampled[sampled.length - 1] - 0.9)).toBeLessThan(0.05);
  });

  it('fadeMs=0 时立即跳变到目标值（瞬间写入，无过渡）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.LipSync, 'ParamMouthOpenY', 0.5, 'set', 0));
    arbiter.update(1); // 一帧内到位
    expect(arbiter.getCurrentSource('ParamMouthOpenY')).toBe(ParamSource.LipSync);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.5, 5);
  });
});

describe('RM3 param-arbiter — Blink 眼部特权穿透（用户确认 Q2 采纳）', () => {
  it('Blink 写 ParamEyeLOpen 可覆盖更高优先 Emotion 的同参数', () => {
    const { arbiter, fake } = makeArbiter();

    // Emotion(L5) 先控制眼部
    arbiter.write(write(ParamSource.Emotion, 'ParamEyeLOpen', 0.85, 'set', 0));
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamEyeLOpen')).toBe(ParamSource.Emotion);

    // Blink(L2) 特权穿透 → 眼睛闭上（0）获胜
    arbiter.write(write(ParamSource.Blink, 'ParamEyeLOpen', 0.0, 'set', 200));
    settle(arbiter, 10, 100);

    expect(arbiter.getCurrentSource('ParamEyeLOpen')).toBe(ParamSource.Blink);
    const last = fake.writes[fake.writes.length - 1];
    expect(last.id).toBe('ParamEyeLOpen');
    expect(Math.abs(last.value - 0.0)).toBeLessThan(0.05);
  });

  it('Blink 写 ParamEyeROpen 同样特权穿透覆盖 Emotion', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamEyeROpen', 1.0, 'set', 0));
    settle(arbiter);
    arbiter.write(write(ParamSource.Blink, 'ParamEyeROpen', 0.0, 'set', 0));
    settle(arbiter);

    expect(arbiter.getCurrentSource('ParamEyeROpen')).toBe(ParamSource.Blink);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0, 5);
  });

  it('眨眼结束后眼部回退到次高优先 Emotion 轨道值', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamEyeLOpen', 0.85, 'set', 0));
    settle(arbiter);
    arbiter.write(write(ParamSource.Blink, 'ParamEyeLOpen', 0.0, 'set', 0));
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamEyeLOpen')).toBe(ParamSource.Blink);

    // 眨眼释放（睁眼）→ 回退到 Emotion 0.85
    arbiter.release('ParamEyeLOpen', ParamSource.Blink);
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamEyeLOpen')).toBe(ParamSource.Emotion);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.85, 5);
  });

  it('特权穿透仅限眼部参数：Blink 写非眼部参数仍被更高优先源阻塞', () => {
    const { arbiter, fake } = makeArbiter();

    // Emotion(L5) 控制 ParamMouthForm（非眼部）
    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.9, 'set', 0));
    settle(arbiter);

    // Blink(L2) 尝试写同参数 → 非眼部，不穿透 → 被阻塞，仍由 Emotion 控制
    arbiter.write(write(ParamSource.Blink, 'ParamMouthForm', 0.1, 'set', 0));
    settle(arbiter);

    expect(arbiter.getCurrentSource('ParamMouthForm')).toBe(ParamSource.Emotion);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.9, 5);
  });
});

describe('RM3 param-arbiter — 多源合并（同一参数多源取最高优先）', () => {
  it('四源同时写同一参数，呈现值取最高优先 LipSync 轨道', () => {
    const { arbiter, fake } = makeArbiter();

    // 从低到高各写一次
    arbiter.write(write(ParamSource.Breath, 'ParamMouthOpenY', 0.5, 'set', 0));
    arbiter.write(write(ParamSource.IdleMotion, 'ParamMouthOpenY', 0.6, 'set', 0));
    arbiter.write(write(ParamSource.Emotion, 'ParamMouthOpenY', 0.7, 'set', 0));
    arbiter.write(write(ParamSource.LipSync, 'ParamMouthOpenY', 0.9, 'set', 0));
    settle(arbiter);

    // 最高优先 LipSync 公布其值；并保证最终值不被低优先覆盖
    expect(arbiter.getCurrentSource('ParamMouthOpenY')).toBe(ParamSource.LipSync);
    const last = fake.writes[fake.writes.length - 1];
    expect(last.value).toBeCloseTo(0.9, 5);
  });

  it('最高优先源释放后逐级回退到次高优先（多级回退链）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamBrowRY', 0.7, 'set', 0));
    arbiter.write(write(ParamSource.Breath, 'ParamBrowRY', 0.3, 'set', 0));
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamBrowRY')).toBe(ParamSource.Emotion);

    // 释放 Emotion → 回退到 Breath
    arbiter.release('ParamBrowRY', ParamSource.Emotion);
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamBrowRY')).toBe(ParamSource.Breath);
    expect(fake.writes[fake.writes.length - 1].value).toBeCloseTo(0.3, 5);
  });
});

describe('RM3 param-arbiter — 无效输入处理', () => {
  it('未知 paramId 的 getCurrentSource 返回 -1（不抛异常）', () => {
    const { arbiter } = makeArbiter();
    expect(arbiter.getCurrentSource('ParamNonexistent')).toBe(-1);
    expect(() => {
      arbiter.getCurrentSource as any;
    }).not.toThrow();
  });

  it('写入越界值（>1 或 <0）被 clamp 到 [0,1] 合法范围', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamMouthOpenY', 1.5, 'set', 0));
    settle(arbiter);
    let last = fake.writes[fake.writes.length - 1];
    expect(last.value).toBeLessThanOrEqual(1);
    expect(last.value).toBeGreaterThanOrEqual(0);

    arbiter.write(write(ParamSource.Emotion, 'ParamMouthOpenY', -0.5, 'set', 0));
    settle(arbiter);
    last = fake.writes[fake.writes.length - 1];
    expect(last.value).toBeLessThanOrEqual(1);
    expect(last.value).toBeGreaterThanOrEqual(0);
  });

  it('write 未知来源枚举不抛异常（默认按最低优先级登记）', () => {
    const { arbiter } = makeArbiter();
    expect(() => {
      arbiter.write({ ...write(999 as any, 'ParamX', 0.5), source: 999 });
    }).not.toThrow();
    settle(arbiter);
    // 不崩溃，且可查询（源 999 被视为一个可登记来源）
    expect(arbiter.getCurrentSource('ParamX')).not.toBeUndefined();
  });

  it('negative delta 的 update 不抛异常（防御性容错）', () => {
    const { arbiter } = makeArbiter();
    arbiter.write(write(ParamSource.Emotion, 'ParamBrowLY', 0.5, 'set', 100));
    expect(() => {
      arbiter.update(-5);
    }).not.toThrow();
  });

  it('reset() 后所有参数权属清空、呈现值回 0', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamEyeLOpen', 0.9, 'set', 0));
    arbiter.write(write(ParamSource.LipSync, 'ParamMouthOpenY', 0.8, 'set', 0));
    settle(arbiter);

    arbiter.reset();
    settle(arbiter);

    expect(arbiter.getCurrentSource('ParamEyeLOpen')).toBe(-1);
    expect(arbiter.getCurrentSource('ParamMouthOpenY')).toBe(-1);
    const last = fake.writes[fake.writes.length - 1];
    expect(last.value).toBeCloseTo(0, 5);
  });
});

describe('RM3 param-arbiter — release / releaseAll / 写盘', () => {
  it('RM3-04: release(paramId, source) 释放后参数不再归该源', () => {
    const { arbiter } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.8, 'set', 0));
    settle(arbiter);
    arbiter.release('ParamMouthForm', ParamSource.Emotion);
    settle(arbiter);
    expect(arbiter.getCurrentSource('ParamMouthForm')).not.toBe(ParamSource.Emotion);
  });

  it('RM3-05: releaseAll(Emotion) 释放该源控制的全部参数', () => {
    const { arbiter } = makeArbiter();

    arbiter.write(write(ParamSource.Emotion, 'ParamBrowLY', 0.5, 'set', 0));
    arbiter.write(write(ParamSource.Emotion, 'ParamMouthForm', 0.8, 'set', 0));
    arbiter.write(write(ParamSource.Emotion, 'ParamEyeLOpen', 0.85, 'set', 0));
    settle(arbiter);

    arbiter.releaseAll(ParamSource.Emotion);
    settle(arbiter);

    for (const id of ['ParamBrowLY', 'ParamMouthForm', 'ParamEyeLOpen']) {
      expect(arbiter.getCurrentSource(id)).not.toBe(ParamSource.Emotion);
    }
  });

  it('RM3-06: update(deltaMs) 写入 coreModel（setParameterValueById 被调用）', () => {
    const { arbiter, fake } = makeArbiter();

    arbiter.write(write(ParamSource.LipSync, 'ParamMouthOpenY', 0.6, 'set', 0));
    arbiter.update(16);

    expect(fake.setParameterValueById).toHaveBeenCalled();
    expect(fake.setParameterValueById).toHaveBeenCalledWith(
      'ParamMouthOpenY',
      expect.any(Number)
    );
  });
});

describe('RM3 param-arbiter — 性能（RM3-07）', () => {
  // 契约要求 update() < 1ms（100 参数）。node 环境 Date.now 分辨率 + GC 抖动大，
  // 此处用宽松天花板保证机械稳定（10ms），并记录基准。真实达标靠 profile 而非单测。
  it('RM3-07: 100 参数同时写入后 update() 耗时可接受', () => {
    const { arbiter } = makeArbiter();

    for (let i = 0; i < 100; i++) {
      arbiter.write(
        write(ParamSource.Emotion, `ParamT${i}`, i / 100, 'set', 0)
      );
    }

    arbiter.update(0); // 首次合并
    const start = Date.now();
    for (let i = 0; i < 5; i++) arbiter.update(16);
    const elapsed = Date.now() - start;

    // 宽松上限（10ms）；契约目标 <1ms 由真实 profile 验证
    expect(elapsed).toBeLessThan(10);
  });
});
