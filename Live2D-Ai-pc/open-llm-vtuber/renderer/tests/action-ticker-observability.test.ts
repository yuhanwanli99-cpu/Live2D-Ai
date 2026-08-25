/**
 * action-ticker-observability.test.ts（六动作定版）
 *
 * 端到端动作可观测性：play() → 编排泵写 arbiter（LiveEvent）→ arbiter.update()
 * lerp fade → normalizedToModelValue → setParameterValueById 写 FakeCoreModel。
 * 断言「模型参数真的动了」且动在正确的轴上：
 *   nod=AngleY / shake_no=AngleX / tilt=AngleZ / look_around=EyeBallX 先行+AngleX 轻随
 *   listen=轻 AngleY/Z + 凝视/眉 / surprise=EyeOpen+BrowY+MouthOpenY+轻 AngleY；
 * 并断言 fade 时长与模板一致、播完/释放回 profile 运行时默认（含 EyeLOpen 默认 1）。
 */
import { describe, it, expect } from 'vitest';
import { BodyActionPlayer, CHOREOGRAPHY_LIBRARY } from '../src/choreography';
import { ParamArbiter, ParamSource } from '../src/param-arbiter';
import type { ParamWrite } from '../src/param-arbiter';
import { FakeCoreModel, BAI_LIKE_RANGES, makeTicker } from './helpers/fake-core-model';

const DEFAULTS = BAI_LIKE_RANGES;

/** 带写入记录的真实仲裁器（捕获 fadeMs，用于断言 fade 时长与模板一致） */
function makeRecordingArbiter(core: FakeCoreModel) {
  const arb = new ParamArbiter({ coreModel: core });
  const writes: Array<{ paramId: string; value: number; fadeMs: number }> = [];
  const original = arb.write.bind(arb);
  const recorder = (req: ParamWrite): void => {
    writes.push({ paramId: req.paramId, value: req.value, fadeMs: req.fadeMs });
    original(req);
  };
  (arb as unknown as { write: typeof recorder }).write = recorder;
  return { arb, writes };
}

function makeEnv() {
  const core = new FakeCoreModel();
  const { arb, writes } = makeRecordingArbiter(core);
  const ticker = makeTicker();
  const bap = new BodyActionPlayer(arb, { now: ticker.now });
  return { core, arb, writes, ticker, bap };
}

/** 播放一个动作直到自然结束并让 fade 完全收敛 */
function playToCompletion(env: ReturnType<typeof makeEnv>, name: string, strength = 2) {
  const { arb, ticker, bap } = env;
  bap.play(name, strength);
  for (let i = 0; i < 300 && (bap.currentGesture() !== null || i < 10); i++) {
    ticker.tick(arb, bap, 16);
  }
  // release fade 收敛（最长 ~500ms）
  for (let i = 0; i < 60; i++) ticker.tick(arb, bap, 16);
}

describe('动作端到端可观测性：真实 arbiter → FakeCoreModel', () => {
  it('nod：AngleY 上下两拍明显偏离，AngleX/AngleZ 保持默认', () => {
    const env = makeEnv();
    env.bap.play('nod', 2);
    expect(env.arb.getCurrentSource('ParamAngleY')).toBe(ParamSource.LiveEvent);

    let peakY = 0;
    let peakX = 0;
    let peakZ = 0;
    for (let i = 0; i < 200; i++) {
      env.ticker.tick(env.arb, env.bap, 16);
      peakY = Math.max(peakY, Math.abs((env.core.last('ParamAngleY') ?? DEFAULTS.ParamAngleY.defaultValue) - DEFAULTS.ParamAngleY.defaultValue));
      peakX = Math.max(peakX, Math.abs(env.core.last('ParamAngleX') ?? 0));
      peakZ = Math.max(peakZ, Math.abs(env.core.last('ParamAngleZ') ?? 0));
    }
    expect(peakY).toBeGreaterThan(8); // ±30 范围内 0.55 幅度 ≈ ±16.5
    expect(peakX).toBe(0); // 轴正确：点头不转头
    expect(peakZ).toBe(0);

    // 播完自动释放 → 回默认 0
    expect(env.arb.getCurrentSource('ParamAngleY')).toBe(-1);
    expect(env.core.last('ParamAngleY')).toBeCloseTo(DEFAULTS.ParamAngleY.defaultValue, 2);
  });

  it('shake_no：AngleX 左右两拍（符号交替），其余头轴不动', () => {
    const env = makeEnv();
    env.bap.play('shake_no', 2);
    for (let i = 0; i < 200; i++) env.ticker.tick(env.arb, env.bap, 16);

    const xs = env.core.history('ParamAngleX').filter((v) => Math.abs(v) > 5);
    expect(xs.length).toBeGreaterThanOrEqual(4); // 两拍往返的多次采样
    let alt = 0;
    for (let i = 1; i < xs.length; i++) if (Math.sign(xs[i]) !== Math.sign(xs[i - 1])) alt++;
    expect(alt).toBeGreaterThanOrEqual(2);
    expect(Math.max(...env.core.history('ParamAngleY').map((v) => Math.abs(v)))).toBeLessThan(1e-6);
    expect(Math.max(...env.core.history('ParamAngleZ').map((v) => Math.abs(v)))).toBeLessThan(1e-6);
    expect(env.core.writtenParams().has('ParamBodyAngleX')).toBe(false); // 无身体写入
  });

  it('tilt：AngleZ 单侧保持（全部非零偏移同号），回中归默认', () => {
    const env = makeEnv();
    env.bap.play('tilt', 2);
    for (let i = 0; i < 200; i++) env.ticker.tick(env.arb, env.bap, 16);

    const zs = env.core.history('ParamAngleZ');
    const signs = new Set(zs.filter((v) => Math.abs(v) > 2).map((v) => Math.sign(v)));
    expect(signs.size).toBe(1); // 单侧
    expect(zs.some((v) => Math.abs(v) > 8)).toBe(true); // 保持段明显
    expect(env.core.last('ParamAngleZ')).toBeCloseTo(DEFAULTS.ParamAngleZ.defaultValue, 2); // 回中
  });

  it('look_around：EyeBallX 先行于 AngleX（写入顺序），两者都回默认', () => {
    const env = makeEnv();
    env.bap.play('look_around', 2);
    for (let i = 0; i < 250; i++) env.ticker.tick(env.arb, env.bap, 16);

    const firstEye = env.core.writes.findIndex((w) => w.id === 'ParamEyeBallX');
    const firstHead = env.core.writes.findIndex((w) => w.id === 'ParamAngleX');
    expect(firstEye).toBeGreaterThanOrEqual(0);
    expect(firstHead).toBeGreaterThan(firstEye); // 眼先行，头随动
    expect(Math.abs(env.core.history('ParamEyeBallX').at(-1)! - DEFAULTS.ParamEyeBallX.defaultValue)).toBeLessThan(0.05);
    expect(Math.abs(env.core.last('ParamAngleX')!)).toBeLessThan(1.5);
  });

  it('listen：轻幅度（头部 |dev| < surprise/nod 峰值的一半）+ 眉/凝视参与；surprise：眼眉嘴正向大偏离', () => {
    const listenEnv = makeEnv();
    listenEnv.bap.play('listen', 2);
    for (let i = 0; i < 260; i++) listenEnv.ticker.tick(listenEnv.arb, listenEnv.bap, 16);

    const headDev = (core: FakeCoreModel, id: string) =>
      Math.max(...core.history(id).map((v) => Math.abs(v - DEFAULTS[id].defaultValue)));
    const listenPeak = Math.max(headDev(listenEnv.core, 'ParamAngleY'), headDev(listenEnv.core, 'ParamAngleZ'));
    expect(listenPeak).toBeGreaterThan(0.5); // 有可感知幅度
    // 强度 2 校准为原「3」的视觉水平（0.16×1.35×30° ≈ 6.5°），
    // 仍远小于 nod 峰值（≈22.3°）的一半 —— 「轻」的设计意图不变。
    expect(listenPeak).toBeLessThan(11);
    expect(headDev(listenEnv.core, 'ParamBrowLY')).toBeGreaterThan(0); // 微眉
    expect(headDev(listenEnv.core, 'ParamEyeBallX')).toBeGreaterThan(0); // 凝视

    const surEnv = makeEnv();
    surEnv.bap.play('surprise', 2);
    for (let i = 0; i < 220; i++) surEnv.ticker.tick(surEnv.arb, surEnv.bap, 16);
    expect(headDev(surEnv.core, 'ParamMouthOpenY')).toBeGreaterThan(0.3); // 张嘴（默认 0）
    expect(headDev(surEnv.core, 'ParamBrowLY')).toBeGreaterThan(0.2); // 挑眉
    expect(headDev(surEnv.core, 'ParamEyeLOpen')).toBeGreaterThan(0.01); // 睁眼方向有偏移
    expect(headDev(surEnv.core, 'ParamAngleY')).toBeLessThan(10); // 头部只是轻微（0.2×1.35×30° ≈ 8.1°）

    // surprise 结束后眼部回默认 1（不是硬写 0！）
    expect(surEnv.core.last('ParamEyeLOpen')).toBeCloseTo(DEFAULTS.ParamEyeLOpen.defaultValue, 2);
    expect(surEnv.arb.getCurrentSource('ParamMouthOpenY')).toBe(-1);
  });

  it('强度 3 峰值输出模型值有限且在合法范围内', () => {
    const env = makeEnv();
    playToCompletion(env, 'look_around', 3);
    expect(env.core.writes.length).toBeGreaterThan(0);
    for (const w of env.core.writes) {
      expect(Number.isFinite(w.value)).toBe(true);
      expect(w.value).toBeGreaterThanOrEqual(DEFAULTS[w.id].minimum - 1e-9);
      expect(w.value).toBeLessThanOrEqual(DEFAULTS[w.id].maximum + 1e-9);
    }
  });
});

describe('fade 时长正确（写入 fadeMs 与模板一致）', () => {
  it('每个动作逐关键帧写入的 fadeMs 序列 == 该参数在模板中的 fadeMs 序列', () => {
    for (const [name, frames] of Object.entries(CHOREOGRAPHY_LIBRARY)) {
      const env = makeEnv();
      env.bap.play(name, 2);
      // 只推进泵（不推进 fade 也无妨——recorder 在 write 时即捕获 fadeMs）
      env.bap.update(0);
      for (const f of frames) {
        env.ticker.advanceMs(f.at + (f.at === 0 ? 0 : 1));
        env.bap.update(0);
      }
      for (const paramId of new Set(frames.flatMap((f) => Object.keys(f.delta)))) {
        const expected = frames
          .filter((f) => paramId in f.delta)
          .map((f) => f.fadeMs ?? 160);
        const actual = env.writes
          .filter((w) => w.paramId === paramId)
          .map((w) => w.fadeMs);
        expect(actual, `${name}:${paramId} fadeMs 序列`).toEqual(expected);
      }
    }
  });
});

describe('BodyActionPlayer×ParamArbiter 契约', () => {
  it('play 后 getCurrentSource 为 LiveEvent，release 后回到 -1 且模型回默认', () => {
    const env = makeEnv();
    env.bap.play('tilt', 2);
    env.ticker.tick(env.arb, env.bap);
    expect(env.arb.getCurrentSource('ParamAngleZ')).toBe(ParamSource.LiveEvent);
    env.bap.release();
    expect(env.arb.getCurrentSource('ParamAngleZ')).toBe(-1);
    for (let i = 0; i < 40; i++) env.ticker.tick(env.arb, env.bap);
    expect(env.core.last('ParamAngleZ')).toBeCloseTo(DEFAULTS.ParamAngleZ.defaultValue, 2);
  });
});
