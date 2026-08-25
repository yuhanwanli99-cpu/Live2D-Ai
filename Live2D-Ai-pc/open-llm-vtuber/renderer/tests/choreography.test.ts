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
  writes: Array<{ paramId: string; value: number; fadeMs?: number; valueSpace?: string }> = [];
  released = 0;
  write(w: { source: unknown; paramId: string; value: number; fadeMs?: number; valueSpace?: string }) {
    this.writes.push({ paramId: w.paramId, value: w.value, fadeMs: w.fadeMs, valueSpace: w.valueSpace });
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
    // 发布定版：nod/tilt（明确用户命令）从单姿态手势升级为大幅度编排，保证可见
    expect(resolveChoreography('nod')).toBe('nod');
    expect(resolveChoreography('tilt')).toBe('tilt');
    expect(resolveChoreography(undefined)).toBeNull();
  });

  it('nod/tilt 编排幅度明确偏离中性（避免"仅嘴动"的不可见手势）', () => {
    const nodFrames = CHOREOGRAPHY_LIBRARY['nod'];
    const nodDivergence = nodFrames.some((f) =>
      Object.values(f.params).some((v) => typeof v === 'number' && Math.abs(v - 0.5) >= 0.2),
    );
    expect(nodDivergence).toBe(true);
    const tiltFrames = CHOREOGRAPHY_LIBRARY['tilt'];
    const tiltDivergence = tiltFrames.some((f) =>
      Object.values(f.params).some((v) => typeof v === 'number' && Math.abs(v - 0.5) >= 0.2),
    );
    expect(tiltDivergence).toBe(true);
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
    p.play('lean_in', 0.5); // 帧值 0.78 → 0.5+(0.28*0.5)=0.64
    const y = arb.writes.find((w) => w.paramId === 'ParamAngleY');
    expect(y?.value).toBeCloseTo(0.64, 5);
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

    p.play('sway'); // 单姿态手势（未纳入编排库）走 gestures 回落
    expect(p.currentGesture()).toBe('sway');

    p.play('wave_big'); // 编排接管 + 清残留手势
    expect(p.currentGesture()).toBe('choreo:wave_big');
    expect(arb.released).toBe(1); // gestures.release 被调

    clock.advance(3000);
    p.update(16);
    p.release();
    expect(p.currentGesture()).toBeNull();
  });
});

describe('Preview deterministic director motion policy', () => {
  it('look_around drives bounded gaze then returns both eyes to center', () => {    const frames = CHOREOGRAPHY_LIBRARY.look_around;
    expect(frames.some((frame) => frame.params.ParamEyeBallX !== undefined)).toBe(true);
    for (const frame of frames.slice(0, -1)) {
      const x = frame.params.ParamEyeBallX;
      const y = frame.params.ParamEyeBallY;
      if (x !== undefined) expect(x).toBeGreaterThanOrEqual(0.25);
      if (x !== undefined) expect(x).toBeLessThanOrEqual(0.75);
      if (y !== undefined) expect(y).toBeGreaterThanOrEqual(0.45);
      if (y !== undefined) expect(y).toBeLessThanOrEqual(0.60);
    }
    const last = frames[frames.length - 1].params;
    expect(last.ParamEyeBallX).toBe(0.5);
    expect(last.ParamEyeBallY).toBe(0.5);
  });

  it('strength 1/2/3 maps to increasing normalized motion amplitude', () => {
    const run = (strength: number) => {
      const arb = new FakeArbiter();
      const clock = makeClock();
      const player = new BodyActionPlayer(arb as never, { now: clock.now });
      player.play('lean_in', strength);
      return arb.writes.find((w) => w.paramId === 'ParamAngleY')?.value as number;
    };
    const low = run(1);
    const medium = run(2);
    const high = run(3);
    expect(low).toBeLessThan(medium);
    expect(medium).toBeLessThan(high);
  });

  it('all deterministic body writes declare normalized parameter space', () => {
    const arb = new FakeArbiter();
    const clock = makeClock();
    const player = new BodyActionPlayer(arb as never, { now: clock.now });
    player.play('look_around', 2);
    expect(arb.writes.length).toBeGreaterThan(0);
    expect(arb.writes.every((write) => write.valueSpace === 'normalized')).toBe(true);
  });
});

describe('Preview unified amplitude policy — 参数峰值验收', () => {
  // 每次播放后抓取该源本次写入的快照（BodyActionPlayer 组合门面调完即生效）。
  function snapshotAfterPlay(name: string, strength: number) {
    const arb = new FakeArbiter();
    const clock = makeClock();
    const player = new BodyActionPlayer(arb as never, { now: clock.now });
    player.play(name, strength);
    const peak: Record<string, number> = {};
    for (const w of arb.writes) {
      if (w.valueSpace !== 'normalized' || w.paramId === undefined) continue;
      const d = Math.abs((w.value as number) - 0.5);
      if (!(w.paramId in peak) || d > peak[w.paramId]) peak[w.paramId] = d;
    }
    return { arb, peak };
  }

  it('strength 3 的头部/身体关键参数达到明确可见阈值（偏离中性>=0.25）', () => {
    const cases: Array<[string, string[]]> = [
      ['nod', ['ParamAngleX', 'ParamBodyAngleX']],
      ['tilt', ['ParamAngleZ', 'ParamBodyAngleZ']],
      ['bow', ['ParamAngleX', 'ParamBodyAngleX']],
      ['shake_no', ['ParamAngleX']],
      ['lean_in', ['ParamAngleY', 'ParamBodyAngleX']],
    ];
    for (const [name, params] of cases) {
      const { peak } = snapshotAfterPlay(name, 3);
      for (const p of params) {
        expect(peak[p]).toBeGreaterThanOrEqual(0.25);
      }
    }
  });

  it('六显式命令强度 1/2/3 单调递增（克制 < 明显 < 强烈）', () => {
    const commands = ['nod', 'tilt', 'shake_no', 'look_around', 'bow', 'lean_in'];
    for (const name of commands) {
      const peakLow = Math.max(...Object.values(snapshotAfterPlay(name, 1).peak));
      const peakMid = Math.max(...Object.values(snapshotAfterPlay(name, 2).peak));
      const peakHigh = Math.max(...Object.values(snapshotAfterPlay(name, 3).peak));
      expect(peakLow).toBeLessThan(peakMid);
      expect(peakMid).toBeLessThan(peakHigh);
      expect(peakHigh).toBeGreaterThan(0.25);
    }
  });

  it('强度 1 克制但仍可见（至少一个关键参数偏离中性>=0.12）', () => {
    const commands = ['nod', 'tilt', 'shake_no', 'look_around', 'bow', 'lean_in'];
    for (const name of commands) {
      const peakMax = Math.max(...Object.values(snapshotAfterPlay(name, 1).peak));
      expect(peakMax).toBeGreaterThanOrEqual(0.12);
    }
  });

  it('所有写入在 0..1 内且编排末帧回中性（不越界、不残留）', () => {
    for (const name of Object.keys(CHOREOGRAPHY_LIBRARY)) {
      const { arb } = snapshotAfterPlay(name, 3);
      for (const w of arb.writes) {
        expect((w.value as number)).toBeGreaterThanOrEqual(0);
        expect((w.value as number)).toBeLessThanOrEqual(1);
      }
      const last = CHOREOGRAPHY_LIBRARY[name][CHOREOGRAPHY_LIBRARY[name].length - 1];
      for (const v of Object.values(last.params)) {
        expect(v).toBeCloseTo(0.5, 4);
      }
    }
  });
});
