/**
 * head-face-actions-fake-core-model.test.ts — 头/脸动作扩展验收（FakeCoreModel 实证）
 *
 * 任务五要素，全部断言跑在「真实 ParamArbiter + 真实形状 FakeCoreModel」上：
 *  1. 区分动作：基础六动作 + 能力门控扩展九动作完整播放后的签名两两不同；
 *     新旧动作互不混淆（glance≠look_around、smile≠frown）；
 *     wink_left / wink_right 已删除（真机核验不可用）→ 请求一律 no-op。
 *  2. strength 2 阈值校准：STRENGTH_INTENSITY = {1:0.6, 2:1.35, 3:1.8} ——
 *     1=轻微但可见、2=正常清晰（原「强度 3」的视觉水平）、3=强烈但被安全 clamp
 *     （模型空间峰值封顶在 min/max 内，绝不越界）。
 *  3. 能力过滤：缺门槛参数（Param4/Param6/EyeBallY…）的动作不可用——
 *     availableActions() 不含、play 为 no-op、目录与播放同边界。
 *  4. release 回 default：能力门控动作播完 / 手动 release 后参数回 profile 默认，
 *     绝不停留在动作姿态。
 *  5. 无 Body：全部动作 × 全部强度后 ParamBodyAngle* 零写入。
 */
import { describe, it, expect } from 'vitest';
import {
  BodyActionPlayer,
  CHOREOGRAPHY_LIBRARY,
  STRENGTH_INTENSITY,
  SUPPORTED_ACTIONS,
} from '../src/choreography';
import { FORBIDDEN_BODY_PARAMS, ActionPriority } from '../src/action-policy';
import { ParamArbiter } from '../src/param-arbiter';
import { FakeCoreModel, BAI_LIKE_RANGES, makeTicker, type FakeParamInfo } from './helpers/fake-core-model';

const DEFAULTS = BAI_LIKE_RANGES;

interface Env {
  core: FakeCoreModel;
  arb: ParamArbiter;
  ticker: ReturnType<typeof makeTicker>;
  bap: BodyActionPlayer;
}

function makeEnv(ranges: Record<string, FakeParamInfo> = BAI_LIKE_RANGES): Env {
  const core = new FakeCoreModel(ranges);
  const arb = new ParamArbiter({ coreModel: core });
  const ticker = makeTicker();
  const bap = new BodyActionPlayer(arb, { now: ticker.now });
  return { core, arb, ticker, bap };
}

/** 完整播放一个动作并让 fade 收敛 */
function playFull(env: Env, name: string, strength = 2): void {
  env.bap.play(name, strength);
  for (let i = 0; i < 400 && env.bap.currentGesture() !== null; i++) {
    env.ticker.tick(env.arb, env.bap, 16);
  }
  for (let i = 0; i < 60; i++) env.ticker.tick(env.arb, env.bap, 16);
}

/** 播放到指定相对时刻（推进关键帧泵，不等待结束） */
function playUntil(env: Env, name: string, strength: number, ms: number): void {
  env.bap.play(name, strength);
  let elapsed = 0;
  while (elapsed < ms && env.bap.currentGesture() !== null) {
    env.ticker.tick(env.arb, env.bap, 16);
    elapsed += 16;
  }
}

/** 模型空间峰值偏差（|v − defaultValue| 的最大值） */
function peakDeviation(core: FakeCoreModel, paramId: string): number {
  const def = DEFAULTS[paramId].defaultValue;
  return Math.max(0, ...core.history(paramId).map((v) => Math.abs(v - def)));
}

// ─────────────────────────────────────────────
// 1. 区分动作 — 签名两两不同
// ─────────────────────────────────────────────
describe('1. 区分动作 — 全部动作签名两两不同', () => {
  function signatureOf(action: string): string {
    const env = makeEnv();
    playFull(env, action);
    const touched = [...env.core.writtenParams()].sort().join(',');
    // 主轴方向性 + 峰值（语义幅度空间）
    const frames = CHOREOGRAPHY_LIBRARY[action];
    const deltas = frames.flatMap((f) => Object.values(f.delta));
    const peakSem = Math.max(0, ...deltas.map((v) => Math.abs(v as number)));
    const hasPos = deltas.some((v) => (v as number) > 0);
    const hasNeg = deltas.some((v) => (v as number) < 0);
    const dir = hasPos && hasNeg ? 'both' : hasPos ? '+' : hasNeg ? '-' : 'none';
    return JSON.stringify({ touched, dir, peak: Math.round(peakSem * 100) });
  }

  it('15 个动作签名互不相同（含能力门控动作之间）', () => {
    const sigs = SUPPORTED_ACTIONS.map((a) => [a, signatureOf(a)] as const);
    expect(sigs.length).toBe(15);
    expect(new Set(sigs.map(([, s]) => s)).size).toBe(sigs.length);
    expect(SUPPORTED_ACTIONS).not.toContain('wink_left');
    expect(SUPPORTED_ACTIONS).not.toContain('wink_right');
  });

  it('新旧近邻动作互不混淆', () => {
    // glance 与 look_around 触达参数集不同（头不动 vs 头随动）
    expect(signatureOf('glance_left')).not.toBe(signatureOf('look_around'));
    // smile 与 frown 方向相反
    expect(signatureOf('smile')).not.toBe(signatureOf('frown'));
    // look_up 与 nod 同用 AngleY 但触达集不同（EyeBallY 有无）
    expect(signatureOf('nod')).not.toBe(signatureOf('look_up'));
  });
});

// ─────────────────────────────────────────────
// 2. strength 2 阈值 — 校准表 + 模型空间阈值
// ─────────────────────────────────────────────
describe('2. strength 阈值校准 — 1 轻微可见 / 2 正常清晰(原3水平) / 3 强烈安全clamp', () => {
  it('STRENGTH_INTENSITY 校准表固定为 {1:0.6, 2:1.35, 3:1.8}', () => {
    expect(STRENGTH_INTENSITY[1]).toBeCloseTo(0.6, 5);
    expect(STRENGTH_INTENSITY[2]).toBeCloseTo(1.35, 5); // 原强度 3 的视觉水平
    expect(STRENGTH_INTENSITY[3]).toBeGreaterThan(STRENGTH_INTENSITY[2]);
  });

  it('nod 主轴峰值：s1 可见(>8°) < s2≈22.3°(原s3水平) < s3 但绝不越过 ±30° 边界', () => {
    const peaks: Record<1 | 2 | 3, number> = { 1: 0, 2: 0, 3: 0 };
    for (const s of [1, 2, 3] as const) {
      const env = makeEnv();
      playFull(env, 'nod', s);
      peaks[s] = peakDeviation(env.core, 'ParamAngleY');
    }
    // 强度 1：轻微但可见（> 8° 且明显小于正常档）
    expect(peaks[1]).toBeGreaterThan(8);
    // 强度 2：正常清晰 == 原「强度 3」的视觉水平（0.55 × 1.35 × 30° ≈ 22.28°）
    expect(peaks[2]).toBeGreaterThan(20);
    expect(peaks[2]).toBeLessThan(24);
    // 强度 3：更强但被安全 clamp 在模型范围内
    expect(peaks[3]).toBeGreaterThan(peaks[2]);
    expect(peaks[3]).toBeLessThanOrEqual(30 + 1e-6);
  });

  it('clamp 上限实证：frown s3 压眉精确到模型最小值；surprise s3 睁眼精确到最大值', () => {
    // frown 眉 -0.75 × s3(1.8) = -1.35 → 安全 clamp 到语义 -1 = 模型最小值
    const fr = makeEnv();
    playUntil(fr, 'frown', 3, 400); // 保持相位（第二关键帧 800ms 未到）
    expect(fr.core.last('ParamBrowLY')).toBeCloseTo(DEFAULTS.ParamBrowLY.minimum, 5);
    expect(fr.core.last('ParamBrowRY')).toBeCloseTo(DEFAULTS.ParamBrowRY.minimum, 5);

    const su = makeEnv();
    playUntil(su, 'surprise', 3, 200);
    expect(su.core.last('ParamEyeLOpen')).toBeCloseTo(DEFAULTS.ParamEyeLOpen.maximum, 5);
    expect(su.core.last('ParamMouthOpenY')).toBeLessThanOrEqual(DEFAULTS.ParamMouthOpenY.maximum + 1e-9);
  });

  it('强度 1 下所有动作都有非零可见写入（轻微但可见，不是无感）', () => {
    for (const action of SUPPORTED_ACTIONS) {
      const env = makeEnv();
      env.bap.play(action, 1);
      for (let i = 0; i < 30 && env.bap.currentGesture() !== null; i++) {
        env.ticker.tick(env.arb, env.bap, 16);
      }
      expect(env.core.writes.length, `${action} @s1 应有写入`).toBeGreaterThan(0);
      const moved = [...env.core.writtenParams()].some((p) => {
        const def = DEFAULTS[p]?.defaultValue ?? 0;
        return Math.max(...env.core.history(p).map((v) => Math.abs(v - def))) > 1e-6;
      });
      expect(moved, `${action} @s1 应产生可见偏移`).toBe(true);
    }
  });

  it('非法强度回落 2；BodyActionPlayer 拒绝未知 id 时保持当前状态（旧 provider 忽略未知）', () => {
    const env = makeEnv();
    playUntil(env, "tilt", 99, 100); // 非法强度 → 按 2 执行
    expect(Math.max(...env.core.history('ParamAngleZ').map((v) => Math.abs(v)))).toBeGreaterThan(10);

    playFull(env, 'listen');
    env.bap.play('choreo:nod', 2, { priority: ActionPriority.Live, source: 'dev' }); // 正常排队路径不受影响
    env.bap.play('wave_big' as string, 3); // 旧 provider 下发的已删 id → 忽略
    env.bap.play('totally_unknown' as string, 3); // 完全未知 id → 忽略
    expect(['choreo:nod', 'choreo:listen'].includes(env.bap.currentGesture() ?? '')).toBe(true);
  });
});

// ─────────────────────────────────────────────
// 3. 能力过滤 — 缺门槛参数的动作不可用
// ─────────────────────────────────────────────
describe('3. 能力过滤 — 目录按运行时参数能力收缩', () => {
  /** 去掉若干参数的 bai 形状模型 */
  function withoutParams(...ids: string[]): Record<string, FakeParamInfo> {
    const ranges: Record<string, FakeParamInfo> = {};
    for (const [k, v] of Object.entries(BAI_LIKE_RANGES)) {
      if (!ids.includes(k)) ranges[k] = v;
    }
    return ranges;
  }

  it('完整 bai 形状模型：全部 15 动作可用（已删 wink 不复活）', () => {
    const env = makeEnv();
    expect(env.bap.availableActions().length).toBe(15);
    for (const a of SUPPORTED_ACTIONS) expect(env.bap.isActionAvailable(a)).toBe(true);
    expect(env.bap.availableActions()).not.toContain('wink_left');
    expect(env.bap.availableActions()).not.toContain('wink_right');
  });

  it('无 Param4 → pout 不可用且 play no-op；无 Param6 → wry_mouth 同理', () => {
    const env = makeEnv(withoutParams('Param4', 'Param5', 'Param6'));
    expect(env.bap.isActionAvailable('pout')).toBe(false);
    expect(env.bap.isActionAvailable('wry_mouth')).toBe(false);
    expect(env.bap.availableActions()).not.toContain('pout');
    expect(env.bap.availableActions()).not.toContain('wry_mouth');

    env.bap.play('pout', 2, { priority: ActionPriority.Live, source: 'dev' });
    env.bap.play('wry_mouth', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(env.bap.currentGesture()).toBeNull();
    expect(env.core.writes).toHaveLength(0);

    // 基础动作不受影响
    env.bap.play('smile', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(env.bap.currentGesture()).toBe('choreo:smile');
  });

  it('有 Param4 无 Param6：pout 可用、wry_mouth 仍拒绝（逐动作精确门控）', () => {
    const env = makeEnv(withoutParams('Param6'));
    expect(env.bap.isActionAvailable('pout')).toBe(true);
    expect(env.bap.isActionAvailable('wry_mouth')).toBe(false);
    env.bap.play('pout', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(env.bap.currentGesture()).toBe('choreo:pout');
    for (let i = 0; i < 12; i++) env.ticker.tick(env.arb, env.bap, 16);
    expect(env.core.last('Param4')!).toBeGreaterThan(0.1);
  });

  it('look_up/look_down 需要 AngleY+EyeBallY 同时存在：缺 EyeBallY 即不可用', () => {
    const env = makeEnv(withoutParams('ParamEyeBallY'));
    expect(env.bap.isActionAvailable('look_up')).toBe(false);
    expect(env.bap.isActionAvailable('look_down')).toBe(false);
    // 无门槛的 look_around 不受影响（缺通道时逐帧跳过，仍诚实降级）
    expect(env.bap.isActionAvailable('look_around')).toBe(true);
  });

  it('已删除 wink 动作：任何参数集下都不可用、play 一律 no-op', () => {
    // wink_left / wink_right 真机核验后删除：即使模型带全部眼参数也不可用，
    // 旧 provider 残留请求绝不播放、绝不写任何参数（单眼眨动交给自动 EyeBlink）。
    const full = makeEnv();
    expect(full.bap.isActionAvailable('wink_left')).toBe(false);
    expect(full.bap.isActionAvailable('wink_right')).toBe(false);

    full.bap.play('wink_left', 3, { priority: ActionPriority.Live, source: 'dev' });
    full.bap.play('wink_right', 3, { priority: ActionPriority.Live, source: 'dev' });
    for (let i = 0; i < 40; i++) full.ticker.tick(full.arb, full.bap, 16);
    expect(full.bap.currentGesture()).toBeNull();
    expect(full.bap.pendingAction()).toBeNull();
    expect(full.core.writes).toHaveLength(0);
    expect(full.bap.availableActions()).not.toContain('wink_left');
  });
});

// ─────────────────────────────────────────────
// 4. release 回 default — 门控动作同样回 profile 运行时默认
// ─────────────────────────────────────────────
describe('4. release 回 default — 播完/手动释放都回 profile 默认', () => {
  it('pout 播完后 Param4 精确回默认 0，仲裁源清空', () => {
    const env = makeEnv();
    playFull(env, 'pout', 3);
    expect(env.arb.getCurrentSource('Param4')).toBe(-1);
    expect(env.core.last('Param4')).toBeCloseTo(DEFAULTS.Param4.defaultValue, 5);
  });

  it('deep_breath 播完后 ParamBreath 回默认 0.5（不是 0 / 1）', () => {
    const env = makeEnv();
    playFull(env, 'deep_breath', 2);
    expect(env.arb.getCurrentSource('ParamBreath')).toBe(-1);
    expect(env.core.last('ParamBreath')).toBeCloseTo(DEFAULTS.ParamBreath.defaultValue, 2);
  });

  it('手动 release：glance 进行中立即释放，EyeBallX 平滑回 0', () => {
    const env = makeEnv();
    env.bap.play('glance_left', 2, { priority: ActionPriority.Live, source: 'dev' });
    for (let i = 0; i < 12; i++) env.ticker.tick(env.arb, env.bap, 16);
    expect(Math.min(...env.core.history('ParamEyeBallX'))).toBeLessThan(-0.3);
    env.bap.release();
    for (let i = 0; i < 60; i++) env.ticker.tick(env.arb, env.bap, 16);
    expect(env.core.last('ParamEyeBallX')).toBeCloseTo(DEFAULTS.ParamEyeBallX.defaultValue, 2);
    expect(env.arb.getCurrentSource('ParamEyeBallX')).toBe(-1);
  });

  it('smile 播完后眼/眉/嘴全部回默认（EyeLOpen 回 1 而非硬写 0）', () => {
    const env = makeEnv();
    playFull(env, 'smile', 2);
    for (const p of ['ParamMouthForm', 'ParamBrowLY', 'ParamBrowRY'] as const) {
      expect(env.core.last(p)).toBeCloseTo(DEFAULTS[p].defaultValue, 2);
    }
    expect(env.core.last('ParamEyeLOpen')).toBeCloseTo(DEFAULTS.ParamEyeLOpen.defaultValue, 5);
    expect(env.core.last('ParamEyeLOpen')).not.toBeCloseTo(0.5, 2);
  });
});

// ─────────────────────────────────────────────
// 5. 无 Body — 身体通道存在但永不被触碰
// ─────────────────────────────────────────────
describe('5. 无 Body 写入 — 全部动作 × 全部强度零身体写入', () => {
  it('连续播放 15 动作（强度 1/2/3）后 ParamBodyAngle* 零写入', () => {
    const env = makeEnv();
    for (const action of SUPPORTED_ACTIONS) {
      for (const strength of [1, 2, 3] as const) {
        playFull(env, action, strength);
      }
    }
    const written = env.core.writtenParams();
    for (const forbidden of FORBIDDEN_BODY_PARAMS) {
      expect(written.has(forbidden), `身体通道 ${forbidden} 被写入`).toBe(false);
    }
    // 反证：这些通道确实存在于模型中（否则断言无意义）
    for (const p of ['ParamBodyAngleX', 'ParamBodyAngleY', 'ParamBodyAngleZ']) {
      expect(DEFAULTS[p]).toBeDefined();
    }
    expect(written.size).toBeGreaterThanOrEqual(13); // 头3 + 眼4 + 眉2 + 嘴2 + Param4/6 + Breath
  });
});
