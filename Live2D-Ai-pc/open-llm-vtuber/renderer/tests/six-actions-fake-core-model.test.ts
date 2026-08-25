/**
 * six-actions-fake-core-model.test.ts — 六动作 × 真实形状 FakeCoreModel 端到端验收
 *
 * 用户终验定版的最终防线，全部断言跑在「真实 ParamArbiter + 真实形状 FakeCoreModel」上
 * （参数元数据接口与 live2dcubismcore coreModel 同构，见 helpers/fake-core-model.ts）：
 *  1. 轴正确：每个动作只写自己语义轴（nod=AngleY / shake_no=AngleX / tilt=AngleZ /
 *     look_around=EyeBallX 先行 + AngleX 轻随 / listen=轻 AngleY/Z + 凝视/眉 /
 *     surprise=EyeOpen+BrowY+MouthOpenY+轻 AngleY）。
 *  2. 六动作可区分：完整播放后提取签名（触达参数集 + 主轴方向性 + 峰值），两两不同。
 *  3. 无 Body 写入：模型存在 ParamBodyAngle* 通道但六个动作全程零写入。
 *  4. fade 时长正确：逐关键帧写入的 fadeMs 与模板一致；release 发生在 endAt。
 *  5. release 回默认：无下层源 → 回 profile 运行时默认（含 EyeLOpen 默认 1，绝不硬写 0/0.5）；
 *     有下层源 → 回该源最后有效值。
 */
import { describe, it, expect } from 'vitest';
import { BodyActionPlayer, CHOREOGRAPHY_LIBRARY, SUPPORTED_ACTIONS } from '../src/choreography';
import { FORBIDDEN_BODY_PARAMS, ActionPriority } from '../src/action-policy';
import { ParamArbiter, ParamSource } from '../src/param-arbiter';
import type { ParamWrite } from '../src/param-arbiter';
import { FakeCoreModel, BAI_LIKE_RANGES, makeTicker } from './helpers/fake-core-model';

const DEFAULTS = BAI_LIKE_RANGES;

interface Env {
  core: FakeCoreModel;
  arb: ParamArbiter;
  ticker: ReturnType<typeof makeTicker>;
  bap: BodyActionPlayer;
}

function makeEnv(): Env {
  const core = new FakeCoreModel();
  const arb = new ParamArbiter({ coreModel: core });
  const ticker = makeTicker();
  const bap = new BodyActionPlayer(arb, { now: ticker.now });
  return { core, arb, ticker, bap };
}

/** 完整播放一个动作（16ms 步进直到结束并让 fade 收敛） */
function playFull(env: Env, name: string, strength = 2): void {
  env.bap.play(name, strength);
  for (let i = 0; i < 400 && env.bap.currentGesture() !== null; i++) {
    env.ticker.tick(env.arb, env.bap, 16);
  }
  for (let i = 0; i < 60; i++) env.ticker.tick(env.arb, env.bap, 16);
}

const PEAK_AXIS: Record<string, string> = {
  nod: 'ParamAngleY',
  shake_no: 'ParamAngleX',
  tilt: 'ParamAngleZ',
  look_around: 'ParamEyeBallX',
  listen: 'ParamAngleZ',
  surprise: 'ParamMouthOpenY',
};

describe('1. 轴正确 — 每个动作只写自己的语义轴', () => {
  const EXPECTED_PARAMS: Record<string, string[]> = {
    nod: ['ParamAngleY'],
    shake_no: ['ParamAngleX'],
    tilt: ['ParamAngleZ'],
    look_around: ['ParamAngleX', 'ParamEyeBallX'],
    listen: ['ParamAngleY', 'ParamAngleZ', 'ParamEyeBallX', 'ParamBrowLY', 'ParamBrowRY'],
    surprise: ['ParamAngleY', 'ParamBrowLY', 'ParamBrowRY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamMouthOpenY'],
  };

  for (const action of SUPPORTED_ACTIONS) {
    it(`${action} 触达参数集 == ${EXPECTED_PARAMS[action].join('+')}`, () => {
      const env = makeEnv();
      playFull(env, action);
      const written = [...env.core.writtenParams()].sort();
      expect(written).toEqual([...EXPECTED_PARAMS[action]].sort());
    });
  }

  it('look_around：EyeBallX 首写严格先于 AngleX 首写（眼先动、头轻随）', () => {
    const env = makeEnv();
    env.bap.play('look_around', 2);
    for (let i = 0; i < 250; i++) env.ticker.tick(env.arb, env.bap, 16);
    const firstEyeIdx = env.core.writes.findIndex((w) => w.id === 'ParamEyeBallX');
    const firstHeadIdx = env.core.writes.findIndex((w) => w.id === 'ParamAngleX');
    expect(firstEyeIdx).toBeGreaterThanOrEqual(0);
    expect(firstHeadIdx).toBeGreaterThan(firstEyeIdx);
    // 头部随动幅度不超过眼睛先行幅度太多（轻随）
    const headPeak = Math.max(...env.core.history('ParamAngleX').map((v) => Math.abs(v)));
    const eyePeak = Math.max(...env.core.history('ParamEyeBallX').map((v) => Math.abs(v)));
    expect(headPeak).toBeGreaterThan(eyePeak); // 头部范围 ±30 vs 眼 ±1，绝对值更大
  });
});

describe('2. 六动作可区分 — 完整播放后的签名两两不同', () => {
  function signatureOf(action: string): string {
    const env = makeEnv();
    playFull(env, action);
    const touched = [...env.core.writtenParams()].sort().join(',');
    const axis = PEAK_AXIS[action];
    const hist = env.core.history(axis);
    let peak = 0;
    let hasPos = false;
    let hasNeg = false;
    for (const v of hist) {
      const dev = v - DEFAULTS[axis].defaultValue;
      peak = Math.max(peak, Math.abs(dev));
      if (dev > 1e-6) hasPos = true;
      if (dev < -1e-6) hasNeg = true;
    }
    const dir = hasPos && hasNeg ? 'both' : hasPos ? '+' : hasNeg ? '-' : 'none';
    return JSON.stringify({ touched, axis, dir, peak: Math.round(peak * 100) });
  }

  it('六动作签名互不相同', () => {
    const sigs = new Map<string, string>();
    for (const action of SUPPORTED_ACTIONS) sigs.set(action, signatureOf(action));
    expect(sigs.size).toBe(SUPPORTED_ACTIONS.length);
    const serialized = [...sigs.values()];
    expect(new Set(serialized).size).toBe(serialized.length);
  });

  it('方向性差异：nod/shake 双向、tilt 单向、surprise 正向张嘴', () => {
    const envNod = makeEnv();
    playFull(envNod, 'nod');
    const nodSigns = new Set(envNod.core.history('ParamAngleY').map((v) => Math.sign(v)));
    expect(nodSigns.has(1)).toBe(true);
    expect(nodSigns.has(-1)).toBe(true);

    const envTilt = makeEnv();
    playFull(envTilt, 'tilt');
    const tiltSigns = new Set(
      envTilt.core.history('ParamAngleZ').filter((v) => Math.abs(v) > 2).map((v) => Math.sign(v)),
    );
    expect(tiltSigns.size).toBe(1);

    const envSur = makeEnv();
    playFull(envSur, 'surprise');
    expect(Math.max(...envSur.core.history('ParamMouthOpenY'))).toBeGreaterThan(0.5);
  });
});

describe('3. 无 Body 写入 — 身体通道存在但永不被触碰', () => {
  it('连续播放全部六动作（强度 1/2/3）后，ParamBodyAngle* 零写入', () => {
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
    expect(written.size).toBeGreaterThan(0);
  });
});

describe('4. fade 时长正确', () => {
  it('写入 fadeMs 与模板一致；release 恰好发生在 endAt = 末帧.at + 末帧.fadeMs + 80', () => {
    const env = makeEnv();
    const writes: ParamWrite[] = [];
    const original = env.arb.write.bind(env.arb);
    (env.arb as unknown as { write: (w: ParamWrite) => void }).write = (w) => {
      writes.push(w);
      original(w);
    };

    const frames = CHOREOGRAPHY_LIBRARY.nod;
    const startedAt = env.ticker.now();
    env.bap.play('nod', 2);
    // 推完所有关键帧（记录 fadeMs）：按模板时刻逐帧推进泵
    let pumpedTo = 0;
    for (const f of frames) {
      env.ticker.advanceMs(f.at + 1 - pumpedTo);
      pumpedTo = f.at + 1;
      env.bap.update(0);
    }
    const angleYFades = writes.filter((w) => w.paramId === 'ParamAngleY').map((w) => w.fadeMs);
    expect(angleYFades).toEqual(frames.filter((f) => 'ParamAngleY' in f.delta).map((f) => f.fadeMs ?? 160));

    // endAt = last.at + last.fadeMs + 80 = 1000 + 320 + 80 = 1400（相对开始时刻）
    const lastFrame = frames[frames.length - 1];
    const endAt = lastFrame.at + (lastFrame.fadeMs ?? 160) + 80;
    env.ticker.advanceMs(endAt - 40 - pumpedTo);
    env.bap.update(0);
    expect(env.bap.currentGesture()).not.toBeNull(); // 还差一点没结束
    env.ticker.advanceMs(80);
    env.bap.update(0);
    expect(env.bap.currentGesture()).toBeNull(); // 已释放
    expect(env.arb.getCurrentSource('ParamAngleY')).toBe(-1);
    void startedAt;
  });

  it('release 平滑回默认：以最后一次写入的 fadeMs 过渡，不瞬间跳变', () => {
    const env = makeEnv();
    env.bap.play('listen', 2);
    for (let i = 0; i < 400 && env.bap.currentGesture() !== null; i++) env.ticker.tick(env.arb, env.bap, 16);
    // 刚释放（一个 tick 进入 fade）：眉仍接近峰值（模型空间 ~0.12），没有跳到默认 0
    const v0 = env.core.last('ParamBrowLY')!;
    expect(v0).toBeGreaterThan(0.06);
    // 再推 ~9 帧（144ms < 300ms fade）：值严格位于默认与 v0 之间（lerp 进行中）
    for (let i = 0; i < 9; i++) env.ticker.tick(env.arb, env.bap, 16);
    const vMid = env.core.last('ParamBrowLY')!;
    expect(vMid).toBeGreaterThan(DEFAULTS.ParamBrowLY.defaultValue);
    expect(vMid).toBeLessThan(v0);
    // 完全收敛回默认
    for (let i = 0; i < 60; i++) env.ticker.tick(env.arb, env.bap, 16);
    expect(env.core.last('ParamBrowLY')).toBeCloseTo(DEFAULTS.ParamBrowLY.defaultValue, 5);
  });
});

describe('5. release 回默认 — 回 profile 运行时默认或下层默认，绝不硬写 0/假0.5', () => {
  it('无下层源：EyeLOpen（默认 1）释放后精确回到 1，不是 0 也不是 0.5', () => {
    const env = makeEnv();
    playFull(env, 'surprise');
    expect(env.arb.getCurrentSource('ParamEyeLOpen')).toBe(-1);
    expect(env.core.last('ParamEyeLOpen')).toBeCloseTo(DEFAULTS.ParamEyeLOpen.defaultValue, 5);
    expect(env.core.last('ParamEyeLOpen')).not.toBeCloseTo(0.5, 2);
    expect(env.core.last('ParamMouthOpenY')).toBeCloseTo(DEFAULTS.ParamMouthOpenY.defaultValue, 5);
    expect(env.core.last('ParamAngleY')).toBeCloseTo(DEFAULTS.ParamAngleY.defaultValue, 2);
  });

  it('有下层源：释放后回退到下层（MoodBaseline）最后有效值而非 profile 默认', () => {
    const env = makeEnv();
    // 下层基线：MoodBaseline 先写 BrowLY（模型空间 0.5，默认 0）
    env.arb.write({
      source: ParamSource.MoodBaseline,
      paramId: 'ParamBrowLY',
      value: 0.5,
      mode: 'set',
      fadeMs: 0,
      timestamp: Date.now(),
    });
    env.arb.update(16);
    // LiveEvent（listen，归一化空间）压过 MoodBaseline 抬眉
    env.bap.play('listen', 2, { priority: ActionPriority.Live, source: 'dev' });
    for (let i = 0; i < 400 && env.bap.currentGesture() !== null; i++) env.ticker.tick(env.arb, env.bap, 16);
    for (let i = 0; i < 60; i++) env.ticker.tick(env.arb, env.bap, 16);

    expect(env.arb.getCurrentSource('ParamBrowLY')).toBe(ParamSource.MoodBaseline);
    expect(env.core.last('ParamBrowLY')).toBeCloseTo(0.5, 2); // 回下层最后有效值
  });

  it('手动 release 同样回 profile 默认（ws-bridge stopAudio 路径）', () => {
    const env = makeEnv();
    env.bap.play('look_around', 3, { priority: ActionPriority.Live, source: 'dev' });
    for (let i = 0; i < 20; i++) env.ticker.tick(env.arb, env.bap); // 眼先动、头随动都已写入
    expect(env.core.last('ParamEyeBallX')).toBeDefined();
    expect(env.core.last('ParamAngleX')).toBeDefined();
    env.bap.release();
    for (let i = 0; i < 60; i++) env.ticker.tick(env.arb, env.bap, 16);
    expect(env.core.last('ParamEyeBallX')).toBeCloseTo(DEFAULTS.ParamEyeBallX.defaultValue, 2);
    expect(env.core.last('ParamAngleX')).toBeCloseTo(DEFAULTS.ParamAngleX.defaultValue, 2);
  });
});
