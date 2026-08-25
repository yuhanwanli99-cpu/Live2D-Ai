/**
 * action-scheduler.test.ts — 动作核心重构验收（用户终验定版）
 *
 * 覆盖：
 * 1. 半身参数白名单：编排库与手势表只允许已验证通道（头/眼/眉/嘴/呼吸/BodyAngleX/Z）；
 *    禁止全身位移 / BodyY 纵向跳跃通道（ParamBodyAngleY 等）写入模型。
 * 2. 单一 active 动作调度器：同一时刻只播一个动作；同动作重复点击忽略；
 *    更高优先级可打断（短过渡），相同/更低优先级进唯一 pending 槽（不叠加不抖动）。
 * 3. 优先级：dev/live > timeline cue > 情绪 fallback > idle。
 * 4. 每动作明确 duration/cooldown，结束自动回中；stop/release 立即清空 pending。
 *
 * 使用真实 ParamArbiter + FakeCoreModel：ticker 逐帧推进，断言模型参数真实变化与回中。
 */
import { describe, it, expect } from 'vitest';
import {
  BodyActionPlayer,
  CHOREOGRAPHY_LIBRARY,
  ActionPriority,
} from '../src/choreography';
import { BODY_GESTURE_PARAMS } from '../src/body-motion';
import {
  HALF_BODY_PARAM_WHITELIST,
  FORBIDDEN_FULL_BODY_PARAMS,
} from '../src/action-policy';
import { ParamArbiter, ParamSource } from '../src/param-arbiter';

/* ---------- 真实 arbiter + FakeCoreModel ---------- */

interface FakeParamInfo {
  minimum: number;
  maximum: number;
  defaultValue: number;
}

/** 故意包含 ParamBodyAngleY（模型存在该通道）以证明其永不被写入 */
const RANGES: Record<string, FakeParamInfo> = {
  ParamAngleX: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamAngleY: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamAngleZ: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamBodyAngleX: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamBodyAngleY: { minimum: -30, maximum: 30, defaultValue: 0 }, // 禁止写入
  ParamBodyAngleZ: { minimum: -30, maximum: 30, defaultValue: 0 },
  ParamEyeBallX: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamEyeBallY: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamEyeLOpen: { minimum: 0, maximum: 1, defaultValue: 0.5 },
  ParamEyeROpen: { minimum: 0, maximum: 1, defaultValue: 0.5 },
  ParamBrowLY: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamBrowRY: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamMouthOpenY: { minimum: 0, maximum: 1, defaultValue: 0 },
  ParamMouthForm: { minimum: -1, maximum: 1, defaultValue: 0 },
  ParamBreath: { minimum: 0, maximum: 1, defaultValue: 0.5 },
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
  setParameterValueById(id: string, value: number): void {
    this.writes.push({ id, value });
  }
  writtenParams(): Set<string> {
    return new Set(this.writes.map((w) => w.id));
  }
}

interface FakeInternalModel {
  coreModel: FakeCoreModel;
}

function makeTicker() {
  let t = 0;
  return {
    now: () => t,
    tick: (arb: ParamArbiter, bap: BodyActionPlayer, deltaMs: number) => {
      t += deltaMs;
      bap.update(deltaMs);
      arb.update(deltaMs);
    },
  };
}

/* ---------- 1. 半身参数白名单 ---------- */

describe('半身参数白名单（用户定版）', () => {
  it('编排库全部参数都在白名单内', () => {
    for (const [name, frames] of Object.entries(CHOREOGRAPHY_LIBRARY)) {
      for (const frame of frames) {
        for (const paramId of Object.keys(frame.params)) {
          expect(
            HALF_BODY_PARAM_WHITELIST.has(paramId),
            `${name} 使用了非白名单参数 ${paramId}`,
          ).toBe(true);
        }
      }
    }
  });

  it('手势表全部参数都在白名单内', () => {
    for (const [gesture, params] of Object.entries(BODY_GESTURE_PARAMS)) {
      for (const paramId of Object.keys(params)) {
        expect(
          HALF_BODY_PARAM_WHITELIST.has(paramId),
          `gesture ${gesture} 使用了非白名单参数 ${paramId}`,
        ).toBe(true);
      }
    }
  });

  it('禁止通道（BodyY / 胯 / 腿等）从未出现在任何动作模板', () => {
    const used = new Set<string>();
    for (const frames of Object.values(CHOREOGRAPHY_LIBRARY)) {
      for (const frame of frames) for (const k of Object.keys(frame.params)) used.add(k);
    }
    for (const gesture of Object.values(BODY_GESTURE_PARAMS)) {
      for (const k of Object.keys(gesture)) used.add(k);
    }
    for (const forbidden of FORBIDDEN_FULL_BODY_PARAMS) {
      expect(used.has(forbidden), `禁止通道 ${forbidden} 出现在动作模板`).toBe(false);
    }
  });

  it('真实 arbiter 播放全部动作后，coreModel 从未收到禁止通道写入（含 ParamBodyAngleY）', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now });

    for (const name of Object.keys(CHOREOGRAPHY_LIBRARY)) {
      bap.play(name, 3);
      for (let i = 0; i < 150; i++) ticker.tick(arb, bap, 16);
    }
    const written = internalModel.coreModel.writtenParams();
    for (const forbidden of FORBIDDEN_FULL_BODY_PARAMS) {
      expect(written.has(forbidden), `coreModel 收到禁止通道 ${forbidden}`).toBe(false);
    }
    // 全部写入都在白名单内
    for (const id of written) {
      expect(HALF_BODY_PARAM_WHITELIST.has(id)).toBe(true);
    }
  });
});

/* ---------- 2. 单 active 调度 / 同动作忽略 / 优先级 ---------- */

describe('单一 active 动作调度器', () => {
  it('同一时刻只播放一个动作；同动作重复点击在完成前忽略', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now });

    bap.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.currentGesture()).toBe('choreo:nod');

    // 同动作重复：进行中 → 忽略（pending 不新增、current 不变）
    bap.play('nod', 3, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.currentGesture()).toBe('choreo:nod');
    expect(bap.pendingAction()).toBeNull();

    // 一直 tick 到自然结束
    for (let i = 0; i < 200; i++) ticker.tick(arb, bap, 16);
    expect(bap.currentGesture()).toBeNull();
  });

  it('同动作冷却期内重复请求被忽略；冷却后再请求可播放', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now, cooldownMs: 250 });

    bap.play('tilt', 2);
    // 播到自然结束为止（一到结束立即停下，保证冷却期内）
    for (let i = 0; i < 400 && bap.currentGesture() !== null; i++) {
      ticker.tick(arb, bap, 16);
    }
    expect(bap.currentGesture()).toBeNull();

    const writesBefore = internalModel.coreModel.writes.length;
    bap.play('tilt', 2); // 冷却内 → 忽略
    expect(bap.currentGesture()).toBeNull();
    expect(internalModel.coreModel.writes.length).toBe(writesBefore);

    ticker.tick(arb, bap, 300); // 冷却结束
    bap.play('tilt', 2);
    expect(bap.currentGesture()).toBe('choreo:tilt');
  });

  it('更高优先级打断当前动作；被打断动作不恢复、不进 pending', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now });

    bap.play('bow', 2, { priority: ActionPriority.Fallback, source: 'audio' }); // 情绪兜底
    expect(bap.currentGesture()).toBe('choreo:bow');

    bap.play('nod', 3, { priority: ActionPriority.Live, source: 'dev' }); // dev 显式（最高）
    expect(bap.currentGesture()).toBe('choreo:nod');
    expect(bap.currentPriority()).toBe(ActionPriority.Live);
    expect(bap.pendingAction()).toBeNull();

    // 被打断的 bow 不恢复
    for (let i = 0; i < 200; i++) ticker.tick(arb, bap, 16);
    expect(bap.currentGesture()).toBeNull();
  });

  it('低优先级不打断高优先级：进 pending，当前结束后接续', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now, cooldownMs: 0 });

    bap.play('nod', 3, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.currentGesture()).toBe('choreo:nod');

    // Fallback（情绪兜底）请求 → 不能打断 Live
    bap.play('wave_big', 2, { priority: ActionPriority.Fallback, source: 'audio' });
    expect(bap.currentGesture()).toBe('choreo:nod');
    expect(bap.pendingAction()).toBe('choreo:wave_big');

    // 当前 nod 自然结束后 → pending wave_big 接续播放
    const nod = CHOREOGRAPHY_LIBRARY.nod;
    const nodDuration = nod[nod.length - 1].at + 360;
    for (let i = 0; i < Math.ceil(nodDuration / 16) + 5; i++) {
      ticker.tick(arb, bap, 16);
    }
    expect(bap.currentGesture()).toBe('choreo:wave_big');
  });

  it('timeline cue（Cue）可打断情绪兜底（Fallback），但不能打断 dev（Live）', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now });

    bap.play('bow', 2, { priority: ActionPriority.Fallback, source: 'audio' });
    bap.play('nod', 2, { priority: ActionPriority.Cue, source: 'cue' }); // cue > fallback
    expect(bap.currentGesture()).toBe('choreo:nod');

    bap.play('tilt', 3, { priority: ActionPriority.Live, source: 'dev' });
    bap.play('shake_no', 2, { priority: ActionPriority.Cue, source: 'cue' }); // cue < live
    expect(bap.currentGesture()).toBe('choreo:tilt');
    expect(bap.pendingAction()).toBe('choreo:shake_no');
  });

  it('dev 快速连续点击不同动作：单一 pending 槽替换，不叠加不抖动', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now, cooldownMs: 0 });

    // Live 动作 A 播放中；连续请求 B、C（同为 Live）→ 各自进/替换单一 pending
    bap.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    bap.play('bow', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.pendingAction()).toBe('choreo:bow');
    bap.play('wave_big', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(bap.pendingAction()).toBe('choreo:wave_big'); // 旧 pending 被替换

    // current 结束后只播放最新一个 pending
    const nod = CHOREOGRAPHY_LIBRARY.nod;
    const nodDuration = nod[nod.length - 1].at + 360;
    for (let i = 0; i < Math.ceil(nodDuration / 16) + 5; i++) {
      ticker.tick(arb, bap, 16);
    }
    expect(bap.currentGesture()).toBe('choreo:wave_big');
    expect(bap.pendingAction()).toBeNull();
  });

  it('结束自动回中：播完后 arbiter 无 LiveEvent 驱动、coreModel 回中性', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now });

    bap.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    const frames = CHOREOGRAPHY_LIBRARY.nod;
    const duration = frames[frames.length - 1].at + 360;
    // 播到结束 + 额外 tick 让 arbiter 的 releaseAll fade 收敛回中性
    for (let i = 0; i < Math.ceil(duration / 16) + 45; i++) ticker.tick(arb, bap, 16);

    expect(bap.currentGesture()).toBeNull();
    expect(arb.getCurrentSource('ParamAngleX')).toBe(-1);
    const lastX = internalModel.coreModel.writes
      .filter((w) => w.id === 'ParamAngleX')
      .pop()?.value;
    expect(lastX).toBeCloseTo(0, 2); // 归一化 0.5 == 模型 0（中性）
  });

  it('release/stop 立即清空 pending 并回中', () => {
    const internalModel: FakeInternalModel = { coreModel: new FakeCoreModel() };
    const arb = new ParamArbiter(internalModel);
    const ticker = makeTicker();
    const bap = new BodyActionPlayer(arb, { now: ticker.now, cooldownMs: 0 });

    bap.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    bap.play('wave_big', 2, { priority: ActionPriority.Fallback, source: 'audio' }); // pending
    expect(bap.pendingAction()).toBe('choreo:wave_big');

    bap.release(); // stop：清空 current + pending
    expect(bap.currentGesture()).toBeNull();
    expect(bap.pendingAction()).toBeNull();

    // 继续 tick，pending 不复活
    for (let i = 0; i < 120; i++) ticker.tick(arb, bap, 16);
    expect(bap.currentGesture()).toBeNull();
  });
});