/**
 * choreography 编排导演层测试：关键帧时序、幅度缩放、打断、回落路由。
 */
import { describe, it, expect, vi } from 'vitest';
import {
  CHOREOGRAPHY_LIBRARY,
  ChoreographyPlayer,
  BodyActionPlayer,
  resolveChoreography,
} from '../src/choreography';
import { ParamSource } from '../src/param-arbiter';

class FakeArbiter {
  writes: Array<{ paramId: string; value: number; fadeMs?: number }> = [];
  released = 0;
  write(w: { source: unknown; paramId: string; value: number; fadeMs?: number }) {
    this.writes.push({ paramId: w.paramId, value: w.value, fadeMs: w.fadeMs });
  }
  releaseAll(_src: unknown) {
    this.released += 1;
  }
}

function makeClock() {
  let t = 1000;
  return {
    now: () => t,
    advance: (ms: number) => {
      t += ms;
    },
  };
}

describe('resolveChoreography — 路由', () => {
  it('库内动作与 choreo: 前缀都能命中；未知返回 null', () => {
    expect(resolveChoreography('wave_big')).toBe('wave_big');
    expect(resolveChoreography('choreo:bow')).toBe('bow');
    expect(resolveChoreography('nod')).toBeNull(); // 旧手势走回落
    expect(resolveChoreography(undefined)).toBeNull();
  });
});

describe('ChoreographyPlayer — 时间轴执行', () => {
  it('首帧立即写入，后续帧按 at 推进，播完释放回基线', () => {
    const arb = new FakeArbiter();
    const clock = makeClock();
    const p = new ChoreographyPlayer(arb as never, { now: clock.now });
    expect(p.play('shake_no')).toBe(true);
    expect(p.isPlaying()).toBe(true);
    // 首帧已写（AngleX=0.16）
    expect(arb.writes.some((w) => w.paramId === 'ParamAngleX' && w.value < 0.3)).toBe(true);

    clock.advance(400); // 到第二帧(190ms)+第三帧(380ms)
    p.update(0);
    expect(arb.writes.some((w) => w.paramId === 'ParamAngleX' && w.value > 0.8)).toBe(true);

    clock.advance(2000); // 远超末帧 → 自动释放
    p.update(0);
    expect(p.isPlaying()).toBe(false);
    expect(arb.released).toBe(1);
  });

  it('intensity 以 0.5 为不动点缩放幅度', () => {
    const arb = new FakeArbiter();
    const clock = makeClock();
    const p = new ChoreographyPlayer(arb as never, { now: clock.now });
    p.play('lean_in', 0.5); // 帧值 0.72 → 0.5+(0.22*0.5)=0.61
    const y = arb.writes.find((w) => w.paramId === 'ParamAngleY');
    expect(y?.value).toBeCloseTo(0.61, 5);
  });

  it('cancel 后不再推进；新 play 重置时间轴', () => {
    const arb = new FakeArbiter();
    const clock = makeClock();
    const p = new ChoreographyPlayer(arb as never, { now: clock.now });
    p.play('bow');
    const countAtCancel = arb.writes.length;
    p.cancel();
    clock.advance(5000);
    p.update(0);
    expect(arb.writes.length).toBe(countAtCancel);

    expect(p.play('choreo:bounce_happy')).toBe(true); // 前缀形式也可
    clock.advance(50);
    p.update(0);
    expect(arb.writes.length).toBeGreaterThan(countAtCancel);
  });

  it('所有模板最后一帧都回中性（0.5）且时间戳递增', () => {
    for (const [name, frames] of Object.entries(CHOREOGRAPHY_LIBRARY)) {
      for (let i = 1; i < frames.length; i++) {
        expect(frames[i].at).toBeGreaterThanOrEqual(frames[i - 1].at);
      }
      const lastParams = frames[frames.length - 1].params;
      const values = Object.values(lastParams);
      expect(values.length).toBeGreaterThan(0);
      for (const v of values) expect(v).toBeCloseTo(0.5, 4);
      void name;
    }
  });
});

describe('BodyActionPlayer — 组合门面', () => {
  it('编排名走 choreo，手势名走 gestures；update 双驱动', () => {
    const arb = new FakeArbiter();
    const clock = makeClock();
    const p = new BodyActionPlayer(arb as never, { now: clock.now });

    p.play('nod');
    expect(p.currentGesture()).toBe('nod');

    p.play('wave_big'); // 编排接管 + 清残留手势
    expect(p.currentGesture()).toBe('choreo:wave_big');
    expect(arb.released).toBe(1); // gestures.release 被调

    clock.advance(3000);
    p.update(16);
    p.release();
    expect(p.currentGesture()).toBeNull();
  });
});