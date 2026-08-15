/**
 * RM5 口型驱动 — RED 测试（test-designer 产出）
 *
 * 契约来源：contract-pc-render.md §RM5（RM5-01 ~ RM5-05）
 * 架构来源：architecture-pc-render.md §3.5 lip-sync.ts（volumes[]→MouthOpenY、
 *   visemes[]→MouthForm，多参数联动、无 fade 瞬间跟随音频）
 * 类型来源：renderer/src/types.ts（VisemeFrame 字段 mouth_open_y / mouth_form）
 *
 * 设计说明：
 *  - LipSync 依赖 ParamArbiter，全程用一个可注入的 mock arbiter（不读实现）。
 *  - 定时器用 vi.useFakeTimers()，把 slice 对齐到确定的时间片推进。
 *  - 本文件为 RED：实现由配对 coder 完成 GREEN。
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { LipSync } from '../src/lip-sync';
import { ParamSource } from '../src/param-arbiter';
import type { VisemeFrame } from '../src/types';

/** 记录仲裁器每次 write / release 的可注入 mock */
function makeArbiterMock() {
  const writes: any[] = [];
  const releases: Array<[string, number]> = [];
  return {
    writes,
    releases,
    write: vi.fn((w: any) => {
      writes.push(w);
    }),
    release: vi.fn((paramId: string, source: number) => {
      releases.push([paramId, source]);
    }),
  };
}

/** 从 write 记录中筛出指定 paramId 的 value 序列 */
function valuesFor(paramId: string, writes: any[]): number[] {
  return writes
    .filter((w) => w.paramId === paramId)
    .map((w) => w.value);
}

describe('RM5 lip-sync — 能量 volumes[]→MouthOpenY 映射', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('RM5-01: play([0.5,0.8,0.3],[],100) → 第1片写 MouthOpenY=0.5，第2片写 0.8', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0.5, 0.8, 0.3], [], 100);

    vi.advanceTimersByTime(100); // 第1个时间片
    expect(arb.writes).toHaveLength(1);
    expect(arb.writes[0].paramId).toBe('ParamMouthOpenY');
    expect(arb.writes[0].value).toBe(0.5);

    vi.advanceTimersByTime(100); // 第2个时间片
    const openY = valuesFor('ParamMouthOpenY', arb.writes);
    expect(openY).toEqual([0.5, 0.8]);
  });

  it('RM5-05: play() 到第一次仲裁器写入 < 1 个 sliceLength（即时开始）', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0.9], [], 100);

    // 推进一个时间片即应产生首次写入（不得超过一个 sliceLength 才开始）
    vi.advanceTimersByTime(100);
    expect(valuesFor('ParamMouthOpenY', arb.writes).length).toBeGreaterThan(0);
    expect(arb.writes[0].value).toBe(0.9);
  });

  it('RM5-05b: play() 后未推进时间片（或推进不足一帧）不产生写入', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0.9], [], 100);
    vi.advanceTimersByTime(0);
    expect(arb.writes).toHaveLength(0);
  });

  it('音量阈值（无声不张嘴）：volumes=0 → MouthOpenY 写入值 = 0', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0], [], 50);
    vi.advanceTimersByTime(50);
    expect(valuesFor('ParamMouthOpenY', arb.writes)).toEqual([0]);
  });

  it('低音量（低于阈值）不触发张嘴：volumes=[0.01] → MouthOpenY≈0', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0.01], [], 50);
    vi.advanceTimersByTime(50);
    const v = valuesFor('ParamMouthOpenY', arb.writes);
    expect(v[0]).toBeLessThanOrEqual(0.02);
  });

  it('时间片索引：volumes[i] 按 i 依序写入（volumes=[0.1,0.2,0.3]）', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0.1, 0.2, 0.3], [], 50);
    vi.advanceTimersByTime(50);
    vi.advanceTimersByTime(50);
    vi.advanceTimersByTime(50);
    expect(valuesFor('ParamMouthOpenY', arb.writes)).toEqual([0.1, 0.2, 0.3]);
  });
});

describe('RM5 lip-sync — visemes[] 元音→MouthForm 映射（多参数联动）', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('RM5-02: play([0.5],[{mouth_form:0.6}],100) → 同时写 MouthOpenY=0.5 和 MouthForm=0.6', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);
    const visemes: VisemeFrame[] = [{ mouth_form: 0.6 }];

    lipSync.play([0.5], visemes, 100);
    vi.advanceTimersByTime(100);

    const openY = valuesFor('ParamMouthOpenY', arb.writes);
    const form = valuesFor('ParamMouthForm', arb.writes);
    expect(openY).toEqual([0.5]);
    expect(form).toEqual([0.6]);
  });

  it('visemes 缺失时纯能量驱动（兜底）：本片不写 MouthForm', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    // visemes 数组为空 → 不产生任何 MouthForm 写入
    lipSync.play([0.5], [], 100);
    vi.advanceTimersByTime(100);
    expect(valuesFor('ParamMouthForm', arb.writes)).toEqual([]);
  });

  it('visemes 提供 mouth_open_y 精确值 → 覆盖 MouthOpenY（能量+元音双驱，元音优先）', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);
    // 能量 0.5，但元音 mouth_open_y=0.9（更精确的开合值）应最终生效
    const visemes: VisemeFrame[] = [{ mouth_open_y: 0.9, mouth_form: 0.3 }];

    lipSync.play([0.5], visemes, 100);
    vi.advanceTimersByTime(100);

    // 该时间片最后写入的 MouthOpenY 为元音精确值 0.9（覆盖能量值）
    const openY = valuesFor('ParamMouthOpenY', arb.writes);
    expect(openY[openY.length - 1]).toBe(0.9);
    // 且 MouthForm 由 mouth_form 驱动
    expect(valuesFor('ParamMouthForm', arb.writes)).toEqual([0.3]);
  });
});

describe('RM5 lip-sync — 仲裁器写入（LipSync 最高优先级）+ 停止/释放', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('所有口型写入都带 source=LipSync（优先级最高，L7）', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);
    const visemes: VisemeFrame[] = [{ mouth_form: 0.6 }];

    lipSync.play([0.5, 0.8], visemes, 100);
    vi.advanceTimersByTime(200);

    expect(arb.writes.length).toBeGreaterThan(0);
    for (const w of arb.writes) {
      expect(w.source).toBe(ParamSource.LipSync);
      expect(w.mode).toBe('set');
      expect(w.fadeMs).toBe(0); // 口型无 fade：瞬间跟随音频（同步 <100ms）
    }
  });

  it('RM5-03: 播放完所有 volumes 后自动 stop（不再写仲裁器）', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0.5, 0.8], [], 100);
    vi.advanceTimersByTime(200); // 两个片都消费完

    const writesAfter = arb.writes.length;
    // 再推进超过一个时间片，不应产生新写入
    vi.advanceTimersByTime(500);
    expect(arb.writes.length).toBe(writesAfter);
    expect(lipSync.isPlaying()).toBe(false);
  });

  it('RM5-04: stop() 后仲裁器收到 release(ParamMouthOpenY/LipSync) 和 release(ParamMouthForm/LipSync)', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0.5], [], 100);
    vi.advanceTimersByTime(50); // 尚未完成，手动 stop
    lipSync.stop();

    expect(arb.releases.map((r) => r[0])).toEqual(
      expect.arrayContaining(['ParamMouthOpenY', 'ParamMouthForm'])
    );
    for (const r of arb.releases) {
      expect(r[1]).toBe(ParamSource.LipSync);
    }
  });

  it('RM5-03b: 自动结束时（consumed 完）也释放口型控制', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.play([0.5], [], 100);
    vi.advanceTimersByTime(300); // 播完 → 自动 stop → release

    expect(arb.releases.map((r) => r[0])).toEqual(
      expect.arrayContaining(['ParamMouthOpenY', 'ParamMouthForm'])
    );
  });

  it('stop() 幂等：重复调用不产生重复 release', () => {
    const arb = makeArbiterMock();
    const lipSync = new LipSync(arb as any);

    lipSync.stop();
    lipSync.stop();
    expect(arb.releases.filter((r) => r[0] === 'ParamMouthOpenY')).toHaveLength(0);
  });
});
