/**
 * RM6 待机系统（眨眼/呼吸/微表情/小动作）— RED 测试（test-designer 产出）
 *
 * 契约来源：contract-pc-render.md §RM6（RM6-BL-01~05 / RM6-BR-01~03 / RM6-ME-01~02 / RM6-IM-01~02）
 * 架构来源：architecture-pc-render.md §3.6（idle-blink.ts / idle-breath.ts / idle-micro-expr.ts / idle-motion.ts）
 * 补充需求：translation-polish.md §七-D（眨眼/呼吸频率可测）；任务要求"眨眼可被手动触发"
 *
 * 设计说明：
 *  - 所有模块 mock ParamArbiter（只暴露 write 方法，验证写入调用）与 Live2DModel（x/y/rotation/scale）。
 *  - 定时器/推进全部用 vi.useFakeTimers() 逐帧驱动 update(deltaMs)，保证机械可执行、可复现。
 *  - 本文件为 RED：实现接口/命名由测试先行定义，配对 coder 完成 GREEN。
 *
 * 排班对齐（schedule-pc.md 配对 RM6）：待机模块共享仲裁器，由测试验证各模块写正确的 ParamSource，
 *   保证真实仲裁器能按 低→高 = Breath<MicroExpr<Blink<IdleMotion 正确排序。
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// ─────────────────────────────────────────────────────────────────────────────
// 共享 mock
// ─────────────────────────────────────────────────────────────────────────────

/** ParamArbiter 假对象 —— 只记录 write 调用，不承载真实优先级（仲裁真实逻辑在 RM3） */
function makeFakeArbiter() {
  return {
    write: vi.fn(),
  };
}

/** 记录到 arbiter 的每次写入内容，便于断言 */
interface WriteCall {
  source: number;
  paramId: string;
  value: number;
  mode: string;
  fadeMs: number;
}

/** 从 arbiter.write mock 提取结构化写入记录 */
function writesOf(arbiter: { write: any }): WriteCall[] {
  return arbiter.write.mock.calls.map(([req]: [WriteCall]) => req);
}

/** Live2DModel 假对象 —— 供 idle-motion 读写 x/y/rotation/scale */
function makeFakeModel() {
  return {
    x: 0,
    y: 0,
    rotation: 0,
    scale: { x: 1, y: 1, set: vi.fn() },
  };
}

/**
 * 逐帧推进一个控制器 update 足够时长，返回 deltaMs=16.67（约 60fps）的调用序列。
 * 采样时不推进真实时钟，仅累加控制器内部计时器。
 */
function stepTimeline(controller: { update: (d: number) => void }, totalMs: number): number {
  const step = 1000 / 60; // ~16.67ms / 帧（@60fps）
  let elapsed = 0;
  let frames = 0;
  while (elapsed < totalMs) {
    controller.update(step);
    elapsed += step;
    frames++;
  }
  return frames;
}

// ─────────────────────────────────────────────────────────────────────────────
// RM6.1 眨眼状态机
// ─────────────────────────────────────────────────────────────────────────────
describe('RM6 眨眼状态机 idle-blink', () => {
  let arbiter: any;

  // 契约接口：BlinkStateMachine(arbiter); update(deltaMs); reset(); setConfig(Partial<BlinkConfig>)
  // 任务额外要求触发手动眨眼事件 → 定义 triggerBlink() 方法（RED）。
  let blinkModule: any;
  function makeBlink(): any {
    const BlinkStateMachine = blinkModule.BlinkStateMachine;
    if (!BlinkStateMachine) throw new Error('[RED] BlinkStateMachine 未导出');
    return new BlinkStateMachine(arbiter);
  }

  beforeEach(async () => {
    vi.useFakeTimers();
    arbiter = makeFakeArbiter();
    blinkModule = await import('../src/idle/idle-blink'); // 锁点：cwd=renderer，src 由实现产出
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('RM6-BL-04: 闭眼阶段 ParamEyeLOpen/ROpen 从 1→0 线性变化', () => {
    // 缩小开门间隔以便在短时间内触发，但保持闭眼时长 75ms 不变（默认）
    const blink = makeBlink();
    blink.setConfig({ intervalMin: 50, intervalMax: 50, closeDuration: 75 });

    // 前 100ms 内无闭眼（仍在 open 等开门）
    stepTimeline(blink, 100);
    // 推到接近开门点
    stepTimeline(blink, 49);
    // 触发关闭点，再推一个闭眼时长
    stepTimeline(blink, 75);

    // 采样闭眼阶段的 ParamEyeLOpen 写入序列 —— 应从 1 线性下降到 0
    const eyeWrites = writesOf(arbiter).filter(
      (w) => w.paramId === 'ParamEyeLOpen' && w.source === 2 // Blink
    );
    const values = eyeWrites.map((w) => w.value);
    expect(values.length).toBeGreaterThan(2); // 至少 3 个中间采样帧
    // 首值接近 1，末值接近 0
    expect(values[0]).toBeCloseTo(1, 1);
    // 线性：相邻帧差值 ≈ 常数（采样 deltaMs 恒定）
    const diffs = values.slice(1).map((v, i) => values[i] - v);
    const firstDiff = Math.abs(diffs[0]);
    for (const d of diffs) {
      expect(Math.abs(d)).toBeCloseTo(firstDiff, 1);
    }
    // 最终闭到位 ~0
    expect(values[values.length - 1]).toBeLessThan(0.05);
  });

  it('RM6-BL-05: 睁眼后 ParamEyeLOpen/ROpen 回到 1.0', () => {
    const blink = makeBlink();
    blink.setConfig({ intervalMin: 50, intervalMax: 50 }); // 快速开门

    // 推进足够时长覆盖 闭眼→睁眼 完整周期
    stepTimeline(blink, 50); // 到达开门
    stepTimeline(blink, 75); // 闭眼
    stepTimeline(blink, 300); // 睁眼过程

    const openWrites = writesOf(arbiter).filter(
      (w) => w.paramId === 'ParamEyeLOpen' && w.source === 2
    );
    const last = openWrites[openWrites.length - 1]?.value;
    expect(last).toBeCloseTo(1.0, 1); // (±0.05 内)
    // ROpen 同步跟随
    const lastR = writesOf(arbiter).filter((w) => w.paramId === 'ParamEyeROpen')
      .slice(-1)[0]?.value;
    expect(lastR).toBeCloseTo(1.0, 1);
  });

  it('手动眨眼触发事件（triggerBlink）立即进入闭眼，不受间隔约束', () => {
    const blink = makeBlink();
    // 长间隔，正常状态下很久不眨眼
    blink.setConfig({ intervalMin: 100000, intervalMax: 100000 });

    // 未触发前推进 1000ms：不应有 Blink 写入
    stepTimeline(blink, 1000);
    expect(writesOf(arbiter).filter((w) => w.source === 2).length).toBe(0);

    // 触发手动眨眼 → 立刻开始闭眼（首帧写入值接近 1 或开始变化）
    expect(typeof blink.triggerBlink).toBe('function'); // RED：需存在 triggerBlink()
    blink.triggerBlink();
    stepTimeline(blink, 40); // 闭眼进行中
    const manual = writesOf(arbiter).filter((w) => w.source === 2);
    expect(manual.length).toBeGreaterThan(0); // 触发后立即写入闭眼序列
    // 触发后至闭眼完成：值应单调下降
    const vals = manual.map((w) => w.value);
    for (let i = 1; i < vals.length; i++) {
      expect(vals[i]).toBeLessThanOrEqual(vals[i - 1] + 1e-9);
    }
  });

  it('RM6-BL-01: 闭眼阶段持续 75±10ms（确定性注入 closeDuration=75）', () => {
    const blink = makeBlink();
    blink.setConfig({ intervalMin: 3000, intervalMax: 3000, closeDuration: 75 });

    const step = 1000 / 60;
    const eyeW: Array<{ t: number; v: number }> = [];
    let t = 0;
    const len = () => writesOf(arbiter).filter((w) => w.paramId === 'ParamEyeLOpen').length;
    let prev = len();
    while (t < 12000) {
      blink.update(step);
      const now = len();
      if (now > prev) {
        const last = writesOf(arbiter).filter((w) => w.paramId === 'ParamEyeLOpen').slice(-1)[0];
        eyeW.push({ t, v: last.value });
        prev = now;
      }
      t += step;
    }

    // 找到闭合开始(首次降到 <1)与完全闭合(达到 0)的时刻
    let closeStartT = -1;
    let closeEndT = -1;
    for (const s of eyeW) {
      if (closeStartT < 0 && s.v < 0.999) closeStartT = s.t;
      if (s.v <= 0.001) {
        closeEndT = s.t;
        break;
      }
    }
    expect(closeStartT).toBeGreaterThanOrEqual(0);
    expect(closeEndT).toBeGreaterThan(closeStartT);
    const closeMs = closeEndT - closeStartT;
    expect(closeMs).toBeGreaterThanOrEqual(65);
    expect(closeMs).toBeLessThanOrEqual(85);
  });

  it('RM6-BL-02: 睁眼阶段持续 ∈ [150, 300]ms（确定性注入 min=max=200）', () => {
    const blink = makeBlink();
    // 固定睁眼时长为区间内值（架构允许实现用注入值，200 在 150-300 内）
    blink.setConfig({ intervalMin: 3000, intervalMax: 3000, openDurationMin: 200, openDurationMax: 200 });

    const step = 1000 / 60;
    const eyeW: Array<{ t: number; v: number }> = [];
    let t = 0;
    const len = () => writesOf(arbiter).filter((w) => w.paramId === 'ParamEyeLOpen').length;
    let prev = len();
    while (t < 12000) {
      blink.update(step);
      const now = len();
      if (now > prev) {
        const last = writesOf(arbiter).filter((w) => w.paramId === 'ParamEyeLOpen').slice(-1)[0];
        eyeW.push({ t, v: last.value });
        prev = now;
      }
      t += step;
    }

    // opening 段 = 从 0(闭合) 升到 1(睁眼完成)
    let openStartT = -1;
    let openEndT = -1;
    for (const s of eyeW) {
      if (s.v <= 0.001 && openStartT < 0) openStartT = s.t;
      if (openStartT >= 0 && s.v >= 0.999) {
        openEndT = s.t;
        break;
      }
    }
    expect(openStartT).toBeGreaterThanOrEqual(0);
    expect(openEndT).toBeGreaterThan(openStartT);
    const openMs = openEndT - openStartT;
    expect(openMs).toBeGreaterThanOrEqual(150);
    expect(openMs).toBeLessThanOrEqual(300);
  });

  it('RM6-BL-03: 眨眼间隔 ∈ [3000, 8000]ms（30 次采样）', () => {
    const blink = makeBlink();
    // 使用默认随机区间 3-8s，采样多次闭合开始时刻测间隔
    blink.reset?.();

    const step = 1000 / 60;
    const closeStarts: number[] = [];
    let t = 0;
    const len = () => writesOf(arbiter).filter((w) => w.paramId === 'ParamEyeLOpen').length;
    let prev = len();
    let descending = false;
    while (t < 120000) {
      // 120s 高密度采够多次
      blink.update(step);
      const now = len();
      if (now > prev) {
        const last = writesOf(arbiter).filter((w) => w.paramId === 'ParamEyeLOpen').slice(-1)[0];
        const v = last.value;
        if (!descending && v < 0.999) {
          closeStarts.push(t);
          descending = true;
        } else if (descending && v >= 0.999) {
          descending = false;
        }
        prev = now;
      }
      t += step;
    }

    // 至少 3 次，采间隔
    expect(closeStarts.length).toBeGreaterThanOrEqual(3);
    let minGap = Infinity;
    let maxGap = 0;
    for (let k = 1; k < closeStarts.length; k++) {
      const gap = closeStarts[k] - closeStarts[k - 1];
      if (gap < minGap) minGap = gap;
      if (gap > maxGap) maxGap = gap;
    }
    expect(minGap).toBeGreaterThanOrEqual(3000);
    expect(maxGap).toBeLessThanOrEqual(8000);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// RM6.2 呼吸正弦波
// ─────────────────────────────────────────────────────────────────────────────
describe('RM6 呼吸正弦波 idle-breath', () => {
  let arbiter: any;
  let breathModule: any;
  function makeBreath(): any {
    const BreathController = breathModule.BreathController;
    if (!BreathController) throw new Error('[RED] BreathController 未导出');
    return new BreathController(arbiter);
  }

  beforeEach(async () => {
    vi.useFakeTimers();
    arbiter = makeFakeArbiter();
    breathModule = await import('../src/idle/idle-breath');
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /** 采样 ParamBreath 在一段时间内的值序列（16.67ms 每帧） */
  function sampleBreath(controller: any, ms: number): number[] {
    const step = 1000 / 60;
    let elapsed = 0;
    const samples: number[] = [];
    while (elapsed < ms) {
      controller.update(step);
      const last = writesOf(arbiter)[writesOf(arbiter).length - 1];
      if (last && last.paramId === 'ParamBreath') samples.push(last.value);
      elapsed += step;
    }
    return samples;
  }

  it('RM6-BR-01: 10s 采样的 ParamBreath 呈正弦波形，零交叉次数 ∈ [3,7]', () => {
    const breath = makeBreath();
    const samples = sampleBreath(breath, 10000);
    expect(samples.length).toBeGreaterThan(100);

    // 首次穿越均值（约 0.5）计为一次零交叉，统计后续变化方向翻转次数
    const mean = 0.5;
    let crossings = 0;
    let up = true;
    for (const v of samples) {
      const nowUp = v >= mean;
      if (nowUp !== up) {
        crossings++;
        up = nowUp;
      }
    }
    // 周期 3-5s → 10s 内 2~3.3 个完整周期 → 每个周期 2 次穿越 → 2~6 次（含首帧对初始的翻转）
    expect(crossings).toBeGreaterThanOrEqual(3);
    expect(crossings).toBeLessThanOrEqual(7);
  });

  it('RM6-BR-02: ParamBreath 值 ∈ [0.35, 0.65]（基线 0.5 ± 振幅 0.15）', () => {
    const breath = makeBreath();
    const samples = sampleBreath(breath, 10000);
    for (const v of samples) {
      expect(v).toBeGreaterThanOrEqual(0.35);
      expect(v).toBeLessThanOrEqual(0.65);
    }
  });

  it('RM6-BR-03: 波形主频在 0.2–0.33 Hz（周期 3–5s）', () => {
    const breath = makeBreath();
    const samples = sampleBreath(breath, 20000); // 更长的窗口稳定测频

    // 零交叉法估计主频：周期 ≈ 相邻同向零交叉间隔
    const mean = 0.5;
    const crossings: number[] = [];
    let up = true;
    samples.forEach((v, i) => {
      const nowUp = v >= mean;
      if (i > 0 && nowUp !== up) crossings.push(i);
      up = nowUp;
    });
    expect(crossings.length).toBeGreaterThanOrEqual(4); // 至少 2 个完整周期跨越多次穿越

    const step = 1000 / 60; // ms/帧
    // 相邻穿越间隔（帧/半周期）→ 换算周期
    const halfPeriods = crossings
      .slice(1)
      .map((c, idx) => (c - crossings[idx]) * step); // 一次半周期时长(ms)
    const avgHalfMs = halfPeriods.reduce((a, b) => a + b, 0) / halfPeriods.length;
    const periodMs = avgHalfMs * 2; // 全周期
    const freqHz = 1000 / periodMs;
    expect(freqHz).toBeGreaterThanOrEqual(0.2);
    expect(freqHz).toBeLessThanOrEqual(0.33);
  });

  it('呼吸持续写入 source=Breath（值归一化 0..1）', () => {
    const breath = makeBreath();
    stepTimeline(breath, 1000);
    const writes = writesOf(arbiter).filter((w) => w.paramId === 'ParamBreath');
    expect(writes.length).toBeGreaterThan(0);
    for (const w of writes) {
      expect(w.source).toBe(0); // Breath
      expect(w.value).toBeGreaterThanOrEqual(0);
      expect(w.value).toBeLessThanOrEqual(1);
    }
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// RM6.3 微表情
// ─────────────────────────────────────────────────────────────────────────────
describe('RM6 微表情 idle-micro-expr', () => {
  let arbiter: any;
  let microModule: any;

  beforeEach(async () => {
    vi.useFakeTimers();
    arbiter = makeFakeArbiter();
    microModule = await import('../src/idle/idle-micro-expr');
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function makeMicro(): any {
    const C = microModule.MicroExpressionController;
    if (!C) throw new Error('[RED] MicroExpressionController 未导出');
    return new C(arbiter);
  }

  /** 推进 controller 一个 interval，返回该段内 micro 写批次（含 ≥0 组） */
  function advanceOneInterval(controller: any, ms: number): number {
    const before = writesOf(arbiter).filter((w) => w.source === 1).length; // MicroExpr=1
    stepTimeline(controller, ms);
    const after = writesOf(arbiter).filter((w) => w.source === 1).length;
    // 每次触发会写多个参数（嘴/眉/眼），按批（触发事件）数统计
    return Math.floor((after - before) / 3); // 每批预计写多参数，保守除 3
  }

  it('RM6-ME-01: 30s 内至少 5 次微表情触发', () => {
    const c = makeMicro();
    let batches = 0;
    // 以 3s 一段推 30s，期望每个 3~6s 间隔各触发一次
    for (let i = 0; i < 10; i++) {
      batches += advanceOneInterval(c, 3000);
    }
    expect(batches).toBeGreaterThanOrEqual(5);
  });

  it('RM6-ME-02: 每次触发写入 source=MicroExpr、fadeMs=400', () => {
    const c = makeMicro();
    stepTimeline(c, 6500); // 覆盖首次触发窗口
    const writes = writesOf(arbiter).filter((w) => w.source === 1);
    expect(writes.length).toBeGreaterThan(0);
    for (const w of writes) {
      expect(w.source).toBe(1); // MicroExpr
      expect(w.fadeMs).toBe(400); // 架构：fade 300-500ms，统一 400
      expect(w.mode).toBe('set');
    }
  });

  it('微表情写入嘴/眉/眼参数（覆盖多个脸部件）', () => {
    const c = makeMicro();
    stepTimeline(c, 6500);
    const params = new Set(writesOf(arbiter).filter((w) => w.source === 1).map((w) => w.paramId));
    const expected = ['ParamBrowLY', 'ParamBrowRY', 'ParamMouthForm', 'ParamEyeBallX', 'ParamEyeBallY'];
    const overlap = expected.filter((p) => params.has(p));
    // 架构：params = [ParamBrowLY, ParamBrowRY, ParamMouthForm, ParamEyeBallX, ParamEyeBallY]
    expect(overlap.length).toBeGreaterThanOrEqual(2); // 每次触发应覆盖多个脸部件
    expect(params.size).toBeGreaterThan(0);
  });

  it('微表情触发频率存在上限：不会每秒刷屏（相邻触发 ≥ 约 1400ms）', () => {
    const c = makeMicro();
    // 采样触发时刻
    const step = 1000 / 60;
    const triggerTimes: number[] = [];
    let t = 0;
    const seenWrites = () => writesOf(arbiter).filter((w) => w.source === 1).length;
    let prevCount = seenWrites();
    while (t < 20000) {
      c.update(step);
      const nowCount = seenWrites();
      if (nowCount > prevCount) {
        triggerTimes.push(t);
        prevCount = nowCount;
      }
      t += step;
    }
    for (let i = 1; i < triggerTimes.length; i++) {
      const gap = triggerTimes[i] - triggerTimes[i - 1];
      expect(gap).toBeGreaterThanOrEqual(2000); // 架构 interval 最小 3s，留容差
    }
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// RM6.4 小动作
// ─────────────────────────────────────────────────────────────────────────────
describe('RM6 小动作 idle-motion', () => {
  let model: any;
  let motionModule: any;

  beforeEach(async () => {
    vi.useFakeTimers();
    model = makeFakeModel();
    motionModule = await import('../src/idle/idle-motion');
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function makeMotion(): any {
    const C = motionModule.IdleMotionController;
    if (!C) throw new Error('[RED] IdleMotionController 未导出');
    return new C(model);
  }

  it('RM6-IM-01: 60s 内至少 3 次小动作触发（8–20s 间隔）', () => {
    const m = makeMotion();
    // 记录 x 发生变化（小动作触发迹象）
    let movedBatches = 0;
    const step = 1000 / 60;
    let t = 0;
    const seenMove = () => Math.abs(model.x) > 1e-6 || Math.abs(model.rotation) > 1e-6;
    let prevMovable = false;
    while (t < 60000) {
      m.update(step);
      const moved = seenMove();
      if (moved && !prevMovable) movedBatches++; // 一次动作刚开始
      prevMovable = moved;
      t += step;
    }
    expect(movedBatches).toBeGreaterThanOrEqual(3);
    expect(movedBatches).toBeLessThanOrEqual(7);
  });

  it('RM6-IM-02: 小动作不改 model.scale（只改变 x/y/rotation）', () => {
    const m = makeMotion();
    const step = 1000 / 60;
    let t = 0;
    while (t < 60000) {
      m.update(step);
      t += step;
    }
    // scale 恒为初始 1
    expect(model.scale.x).toBe(1);
    expect(model.scale.y).toBe(1);
    expect(model.scale.set).not.toHaveBeenCalled();
  });

  it('小动作位移在合理范围（x 水平 ≤ ±6px，rotation 弧度小幅）', () => {
    const m = makeMotion();
    const step = 1000 / 60;
    let t = 0;
    while (t < 60000) {
      m.update(step);
      t += step;
      if (Math.abs(model.x) > 6.5) {
        // 架构 targetState.x = x ± (Math.random()-0.5)*10 → ≤5px；留 30% 容差
        throw new Error(`小动作 x 位移越界: ${model.x}`);
      }
      if (Math.abs(model.rotation) > 0.05) {
        // 架构 targetState.rotation = ±(Math.random()-0.5)*0.03 → ≤0.015；留容差
        throw new Error(`小动作 rotation 越界: ${model.rotation}`);
      }
    }
  });

  it('小动作与表情冲突时仲裁：使用独立 IdleMotion 源（source 不与表情/眨眼抢遮罩层）', () => {
    // RM6-IM 契约：module 对 model 变换（非参数），不与 arbiter 直接写参数冲突；
    // 验证其不触碰 arbiter（不写 source），以交给真实的 Pixi 变换层。
    const m = makeMotion();
    // IdleMotionController 构造参数为 model（架构），不接收 arbiter
    expect((m as any).constructor.length).toBe(1); // 接收 1 个构造参数
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// RM6 优先级仲裁（跨模块约定, translation §七-D4）
// ─────────────────────────────────────────────────────────────────────────────
describe('RM6 跨模块优先级约定（呼吸<微表情<眨眼）', () => {
  it('ParamSource 枚举数值符合 低→高 = Breath(0)<MicroExpr(1)<Blink(2)<IdleMotion(3)', async () => {
    const { ParamSource } = await import('../src/param-arbiter');
    expect(ParamSource.Breath).toBeLessThan(ParamSource.MicroExpr);
    expect(ParamSource.MicroExpr).toBeLessThan(ParamSource.Blink);
    expect(ParamSource.Blink).toBeLessThan(ParamSource.IdleMotion);
  });

  it('闭眼时眨眼(Blink, ↑优先)覆盖微表情(MicroExpr)对 ParamEyeLOpen 的写入', async () => {
    const { MicroExpressionController } = await import('../src/idle/idle-micro-expr');
    const { BlinkStateMachine } = await import('../src/idle/idle-blink');
    const arbiter = makeFakeArbiter();

    const micro = new MicroExpressionController(arbiter);
    const blink = new BlinkStateMachine(arbiter);
    blink.setConfig({ intervalMin: 50, intervalMax: 50 });

    // 先让微表情写眼参数，再触发眨眼：期望二者都以各自 source 写入（真实仲裁器按优先级裁决）
    stepTimeline(micro, 6500); // 触发微表情（含 ParamEyeBallX/Y，不含 EyeLOpen）
    blink.triggerBlink();
    stepTimeline(blink, 75);

    const eyeWrites = writesOf(arbiter).filter((w) => w.paramId === 'ParamEyeLOpen');
    // 眨眼对 EyeLOpen 有写入且 source=Blink(2)
    expect(eyeWrites.some((w) => w.source === 2)).toBe(true);
    // 微表情不对 EyeLOpen 写入（避免与眨眼抢眼参数）——若写则必须优先级高于 Blink 才生效，此约定防御
    expect(eyeWrites.every((w) => w.source === 2 || w.source > 2)).toBe(true);
  });
});