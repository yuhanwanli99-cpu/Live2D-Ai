/**
 * action-scheduler.test.ts — 动作核心验收（六动作定版）
 *
 * 覆盖：
 * 1. 参数白名单：编排库只允许头/脸通道；ParamBodyAngle*（含 BodyY / 肩 / 胯 / 腿）
 *    从未出现在任何模板，真实 arbiter 播放全部动作后 coreModel 也从未收到身体写入。
 * 2. 单一 active 动作调度器：同一时刻只播一个动作；同动作重复点击忽略；
 *    更高优先级可打断（短过渡），相同/更低优先级进唯一 pending 槽（不叠加不抖动）。
 * 3. 优先级：dev/live > timeline cue > 情绪 fallback > idle。
 * 4. 冷却与自动回默认：同动作结束后冷却期内忽略；播完/release 回 profile 默认。
 *
 * 使用真实 ParamArbiter + FakeCoreModel：ticker 逐帧推进，断言模型参数真实变化与回默认。
 */
import { describe, it, expect } from 'vitest';
import {
  BodyActionPlayer,
  CHOREOGRAPHY_LIBRARY,
  ActionPriority,
} from '../src/choreography';
import {
  HEAD_FACE_PARAM_WHITELIST,
  FORBIDDEN_BODY_PARAMS,
} from '../src/action-policy';
import { ParamArbiter } from '../src/param-arbiter';
import { FakeCoreModel, BAI_LIKE_RANGES, makeTicker } from './helpers/fake-core-model';

interface FakeInternalModel {
  coreModel: FakeCoreModel;
}

function makeEnv(opts: { cooldownMs?: number } = {}) {
  const core = new FakeCoreModel();
  const internalModel: FakeInternalModel = { coreModel: core };
  const arb = new ParamArbiter(internalModel);
  const ticker = makeTicker();
  const bap = new BodyActionPlayer(arb, { now: ticker.now, cooldownMs: opts.cooldownMs ?? 250 });
  return { core, arb, ticker, bap };
}

/* ---------- 1. 参数白名单（用户定版） ---------- */

describe('头/脸参数白名单（用户定版）', () => {
  it('编排库全部参数都在白名单内', () => {
    for (const [name, frames] of Object.entries(CHOREOGRAPHY_LIBRARY)) {
      for (const frame of frames) {
        for (const paramId of Object.keys(frame.delta)) {
          expect(
            HEAD_FACE_PARAM_WHITELIST.has(paramId),
            `${name} 使用了非白名单参数 ${paramId}`,
          ).toBe(true);
        }
      }
    }
  });

  it('禁止通道（ParamBodyAngle* 全家 / 胯 / 腿等）从未出现在任何动作模板', () => {
    const used = new Set<string>();
    for (const frames of Object.values(CHOREOGRAPHY_LIBRARY)) {
      for (const frame of frames) for (const k of Object.keys(frame.delta)) used.add(k);
    }
    for (const forbidden of FORBIDDEN_BODY_PARAMS) {
      expect(used.has(forbidden), `禁止通道 ${forbidden} 出现在动作模板`).toBe(false);
    }
  });

  it('真实 arbiter 播放全部动作后，coreModel 从未收到禁止通道写入（模型存在这些通道）', () => {
    const { core, arb, ticker, bap } = makeEnv({ cooldownMs: 0 });
    for (const name of Object.keys(CHOREOGRAPHY_LIBRARY)) {
      bap.play(name, 3, { priority: ActionPriority.Live, source: 'dev' });
      for (let i = 0; i < 150; i++) ticker.tick(arb, bap, 16);
    }
    const written = core.writtenParams();
    // FakeCoreModel 故意包含 ParamBodyAngleX/Y/Z——证明其永不被写入
    for (const forbidden of FORBIDDEN_BODY_PARAMS) {
      expect(written.has(forbidden), `coreModel 收到禁止通道 ${forbidden}`).toBe(false);
    }
    for (const id of written) {
      expect(HEAD_FACE_PARAM_WHITELIST.has(id), `非白名单写入 ${id}`).toBe(true);
    }
  });
});

/* ---------- 2. 单 active 调度 / 同动作忽略 / 冷却 ---------- */

describe('单一 active 动作调度器', () => {
  it('同一时刻只播放一个动作；同动作重复点击在完成前忽略', () => {
    const { arb, ticker, bap } = makeEnv();

    bap.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.currentGesture()).toBe('choreo:nod');

    bap.play('nod', 3, { priority: ActionPriority.Live, source: 'dev' }); // 同动作进行中 → 忽略
    expect(bap.currentGesture()).toBe('choreo:nod');
    expect(bap.pendingAction()).toBeNull();

    for (let i = 0; i < 200; i++) ticker.tick(arb, bap, 16);
    expect(bap.currentGesture()).toBeNull();
  });

  it('同动作冷却期内重复请求被忽略；冷却后再请求可播放', () => {
    const { core, arb, ticker, bap } = makeEnv({ cooldownMs: 250 });

    bap.play('tilt', 2);
    for (let i = 0; i < 400 && bap.currentGesture() !== null; i++) {
      ticker.tick(arb, bap, 16);
    }
    expect(bap.currentGesture()).toBeNull(); // 自然结束

    const writesBefore = core.writes.length;
    bap.play('tilt', 2); // 冷却内 → 忽略
    expect(bap.currentGesture()).toBeNull();
    expect(core.writes.length).toBe(writesBefore);

    ticker.tick(arb, bap, 300); // 过冷却
    bap.play('tilt', 2);
    expect(bap.currentGesture()).toBe('choreo:tilt');
  });

  it('更高优先级打断当前动作；被打断动作不恢复、不进 pending', () => {
    const { arb, ticker, bap } = makeEnv();

    bap.play('look_around', 2, { priority: ActionPriority.Fallback, source: 'audio' }); // 情绪兜底
    expect(bap.currentGesture()).toBe('choreo:look_around');

    bap.play('nod', 3, { priority: ActionPriority.Live, source: 'dev' }); // dev 显式（最高）
    expect(bap.currentGesture()).toBe('choreo:nod');
    expect(bap.currentPriority()).toBe(ActionPriority.Live);
    expect(bap.pendingAction()).toBeNull();

    for (let i = 0; i < 200; i++) ticker.tick(arb, bap, 16);
    expect(bap.currentGesture()).toBeNull(); // 被打断的 look_around 不恢复
  });

  it('低优先级不打断高优先级：进 pending，当前结束后接续', () => {
    const { arb, ticker, bap } = makeEnv({ cooldownMs: 0 });

    bap.play('nod', 3, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.currentGesture()).toBe('choreo:nod');

    bap.play('listen', 2, { priority: ActionPriority.Fallback, source: 'audio' }); // Fallback < Live
    expect(bap.currentGesture()).toBe('choreo:nod');
    expect(bap.pendingAction()).toBe('choreo:listen');

    // nod 自然结束（endAt ≈ 1400ms）→ pending listen 接续
    for (let i = 0; i < Math.ceil(1400 / 16) + 5; i++) ticker.tick(arb, bap, 16);
    expect(bap.currentGesture()).toBe('choreo:listen');
  });

  it('timeline cue（Cue）可打断情绪兜底（Fallback），但不能打断 dev（Live）', () => {
    const { arb, ticker, bap } = makeEnv();

    bap.play('surprise', 2, { priority: ActionPriority.Fallback, source: 'audio' });
    bap.play('shake_no', 2, { priority: ActionPriority.Cue, source: 'cue' }); // cue > fallback
    expect(bap.currentGesture()).toBe('choreo:shake_no');

    bap.play('tilt', 3, { priority: ActionPriority.Live, source: 'dev' });
    bap.play('look_around', 2, { priority: ActionPriority.Cue, source: 'cue' }); // cue < live
    expect(bap.currentGesture()).toBe('choreo:tilt');
    expect(bap.pendingAction()).toBe('choreo:look_around');
  });

  it('dev 快速连续点击不同动作：单一 pending 槽替换，不叠加不抖动', () => {
    const { arb, ticker, bap } = makeEnv({ cooldownMs: 0 });

    bap.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    bap.play('tilt', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.pendingAction()).toBe('choreo:tilt');
    bap.play('shake_no', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.pendingAction()).toBe('choreo:shake_no'); // 旧 pending 被替换

    for (let i = 0; i < Math.ceil(1400 / 16) + 5; i++) ticker.tick(arb, bap, 16);
    expect(bap.currentGesture()).toBe('choreo:shake_no');
    expect(bap.pendingAction()).toBeNull();
  });

  it('结束自动回默认：播完后 arbiter 无 LiveEvent 驱动、coreModel 回 profile 默认', () => {
    const { core, arb, ticker, bap } = makeEnv();

    bap.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    // nod endAt = 1000 + 320 + 80 = 1400ms；再给足 fade 收敛时间
    for (let i = 0; i < Math.ceil(1400 / 16) + 60; i++) ticker.tick(arb, bap, 16);

    expect(bap.currentGesture()).toBeNull();
    expect(arb.getCurrentSource('ParamAngleY')).toBe(-1);
    expect(core.last('ParamAngleY')).toBeCloseTo(BAI_LIKE_RANGES.ParamAngleY.defaultValue, 2);
  });

  it('release/stop 立即清空 pending 并回默认', () => {
    const { arb, ticker, bap } = makeEnv({ cooldownMs: 0 });

    bap.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    bap.play('listen', 2, { priority: ActionPriority.Fallback, source: 'audio' }); // pending
    expect(bap.pendingAction()).toBe('choreo:listen');

    bap.release(); // stop：清空 current + pending
    expect(bap.currentGesture()).toBeNull();
    expect(bap.pendingAction()).toBeNull();

    for (let i = 0; i < 120; i++) ticker.tick(arb, bap, 16);
    expect(bap.currentGesture()).toBeNull(); // pending 不复活
  });
});
