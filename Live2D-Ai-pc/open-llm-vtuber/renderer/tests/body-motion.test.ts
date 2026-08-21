import { describe, it, expect } from 'vitest';
import {
  BODY_GESTURE_PARAMS,
  BodyMotionPlayer,
  resolveBodyGesture,
} from '../src/body-motion';
import { ParamSource } from '../src/param-arbiter';

function makeArbiter() {
  const writes: any[] = [];
  return {
    writes,
    write(req: any) {
      writes.push(req);
    },
    releaseAll(source: any) {
      writes.push({ __releaseAll: source });
    },
  };
}

describe('body-motion resolveBodyGesture', () => {
  it('已知动作原样返回', () => {
    expect(resolveBodyGesture('happy')).toBe('happy');
    expect(resolveBodyGesture('NOD')).toBe('nod');
  });

  it('未知/空动作回退 idle', () => {
    expect(resolveBodyGesture('dance')).toBe('idle');
    expect(resolveBodyGesture(undefined)).toBe('idle');
    expect(resolveBodyGesture('')).toBe('idle');
  });
});

describe('BodyMotionPlayer', () => {
  it('播放 happy 会以 LiveEvent 源写参数，并在 hold 后 release', () => {
    let now = 1000;
    const arbiter = makeArbiter();
    const player = new BodyMotionPlayer(arbiter as any, { now: () => now, holdMs: 500 });

    player.play('happy');
    expect(player.currentGesture()).toBe('happy');
    expect(arbiter.writes.filter((w) => w.__releaseAll).length).toBe(0);
    expect(arbiter.writes.some((w) => w.source === ParamSource.LiveEvent)).toBe(true);

    now = 1499;
    player.update(16);
    expect(player.currentGesture()).toBe('happy');

    now = 1500;
    player.update(16);
    expect(player.currentGesture()).toBeNull();
    expect(arbiter.writes.some((w) => w.__releaseAll === ParamSource.LiveEvent)).toBe(true);
  });

  it('播放 idle 直接 release，不写参数', () => {
    const arbiter = makeArbiter();
    const player = new BodyMotionPlayer(arbiter as any);
    player.play('idle');
    expect(player.currentGesture()).toBeNull();
    expect(arbiter.writes.some((w) => w.__releaseAll === ParamSource.LiveEvent)).toBe(true);
    expect(arbiter.writes.filter((w) => !w.__releaseAll).length).toBe(0);
  });

  it('参数表只包含 bai 实读存在的半身参数', () => {
    for (const gesture of Object.keys(BODY_GESTURE_PARAMS)) {
      for (const paramId of Object.keys((BODY_GESTURE_PARAMS as any)[gesture])) {
        expect(paramId).toMatch(/^Param(Angle|BodyAngle)/);
      }
    }
  });
});
