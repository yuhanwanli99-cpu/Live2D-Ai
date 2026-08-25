/**
 * body-motion 兼容层测试（六动作定版）：
 *  - 旧参数姿态表（sway/happy/excited + ParamBodyAngle*）已删除；
 *  - resolveBodyGesture 只认六个头/脸动作；已删除/未知动作 → null；
 *  - BodyMotionPlayer = BodyActionPlayer 别名（单 active 调度 + release 语义）。
 */
import { describe, it, expect } from 'vitest';
import {
  BODY_GESTURES,
  BodyMotionPlayer,
  resolveBodyGesture,
} from '../src/body-motion';
import { SUPPORTED_ACTIONS } from '../src/choreography';
import { ActionPriority } from '../src/action-policy';
import { ParamArbiter } from '../src/param-arbiter';
import { FakeCoreModel, BAI_LIKE_RANGES, makeTicker } from './helpers/fake-core-model';

describe('resolveBodyGesture — 六动作归一化', () => {
  it('已知动作原样返回（大小写不敏感）', () => {
    expect(resolveBodyGesture('NOD')).toBe('nod');
    expect(resolveBodyGesture('shake_no')).toBe('shake_no');
    expect(resolveBodyGesture('look_around')).toBe('look_around');
    for (const id of SUPPORTED_ACTIONS) {
      expect(resolveBodyGesture(id)).toBe(id);
    }
  });

  it('idle / release / 空 → idle（release 语义）', () => {
    expect(resolveBodyGesture('idle')).toBe('idle');
    expect(resolveBodyGesture('release')).toBe('idle');
    expect(resolveBodyGesture('')).toBe('idle');
    expect(resolveBodyGesture(undefined)).toBe('idle');
    expect(resolveBodyGesture(null)).toBe('idle');
  });

  it('已删除动作（happy/excited/sway/bow/lean_in/wave/bounce 等）与未知动作 → null', () => {
    for (const dead of ['happy', 'excited', 'sway', 'bow', 'lean_in', 'wave', 'wave_big', 'bounce_happy', 'dance']) {
      expect(resolveBodyGesture(dead)).toBeNull();
    }
  });

  it('BODY_GESTURES 与 choreography 六动作一致，不含任何身体动作', () => {
    expect([...BODY_GESTURES].sort()).toEqual([...SUPPORTED_ACTIONS].sort());
    expect(BODY_GESTURES).not.toContain('sway');
    expect(BODY_GESTURES).not.toContain('happy');
    expect(BODY_GESTURES).not.toContain('excited');
  });
});

describe('BodyMotionPlayer（= BodyActionPlayer 别名）', () => {
  it('播放 nod 以 LiveEvent 源写 AngleY，release 后回 profile 默认', () => {
    const core = new FakeCoreModel();
    const arb = new ParamArbiter({ coreModel: core });
    const ticker = makeTicker();
    const player = new BodyMotionPlayer(arb, { now: ticker.now });

    player.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(player.currentGesture()).toBe('choreo:nod');
    for (let i = 0; i < 12; i++) ticker.tick(arb, player); // fade 完成
    // 轴正确：nod 只动 AngleY（默认 0），绝不动身体通道
    expect(Math.abs(core.last('ParamAngleY') ?? NaN)).toBeGreaterThan(8);
    expect(core.writtenParams().has('ParamBodyAngleX')).toBe(false);

    player.release();
    for (let i = 0; i < 60; i++) ticker.tick(arb, player);
    expect(player.currentGesture()).toBeNull();
    expect(core.last('ParamAngleY')).toBeCloseTo(BAI_LIKE_RANGES.ParamAngleY.defaultValue, 2);
  });

  it('播放已删除动作是 no-op；播放 idle 直接释放', () => {
    const core = new FakeCoreModel();
    const arb = new ParamArbiter({ coreModel: core });
    const ticker = makeTicker();
    const player = new BodyMotionPlayer(arb, { now: ticker.now });

    player.play('happy');
    expect(player.currentGesture()).toBeNull();
    expect(core.writes).toHaveLength(0);

    player.play('tilt');
    ticker.tick(arb, player);
    expect(core.writes.length).toBeGreaterThan(0);

    player.play('idle'); // release 别名
    expect(player.currentGesture()).toBeNull();
    expect(arb.getCurrentSource('ParamAngleZ')).toBe(-1);
  });
});
