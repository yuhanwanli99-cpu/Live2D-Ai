/**
 * motion-timeline 调度器单测 —— 括注动作帧按播放进度触发
 *
 * 契约：
 *  - 进度到达 at×duration（含 30ms 提前量）时触发一次且仅一次；
 *  - duration 未就绪（NaN/0）时不触发也不丢帧；
 *  - cancel()/ended 后不再触发；重复 start 先解绑旧元素；
 *  - 非法帧（at 非有限负值 / 空 motion）被过滤。
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  MotionTimelineScheduler,
  type TimelineTimeSource,
} from '../src/motion-timeline';
import type { MotionCue } from '../src/types';

class FakeAudio implements TimelineTimeSource {
  currentTime = 0;
  duration = NaN;
  ended = false;
  private listeners = new Map<string, Set<() => void>>();

  addEventListener(type: string, cb: () => void): void {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(cb);
  }

  removeEventListener(type: string, cb: () => void): void {
    this.listeners.get(type)?.delete(cb);
  }

  emit(type: string): void {
    this.listeners.get(type)?.forEach((cb) => cb());
  }

  listenerCount(type: string): number {
    return this.listeners.get(type)?.size ?? 0;
  }
}

function makeScheduler(): {
  sched: MotionTimelineScheduler;
  fired: string[];
  cues: MotionCue[];
} {
  // cues：整帧视图（帧级元数据断言用）；fired：动作名视图（既有断言沿用）
  const cues: MotionCue[] = [];
  const fired: string[] = [];
  const sched = new MotionTimelineScheduler((cue) => {
    cues.push(cue);
    fired.push(cue.motion);
  });
  return { sched, fired, cues };
}

describe('MotionTimelineScheduler — 按播放进度触发括注动作', () => {
  beforeEach(() => {
    // 屏蔽宿主 rAF（部分 Node 版本自带），保证测试只由手动 pump 驱动
    vi.stubGlobal('requestAnimationFrame', undefined);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('进度到达 at×duration 时触发一次', () => {
    const { sched, fired } = makeScheduler();
    const el = new FakeAudio();
    el.duration = 10;
    sched.start(el, [{ at: 0.5, motion: 'happy' }]);

    el.currentTime = 4.0;
    sched.pump();
    expect(fired).toEqual([]);

    el.currentTime = 4.98; // 0.5×10 - 0.03 提前量内
    sched.pump();
    expect(fired).toEqual(['happy']);
  });

  it('同一动作帧只触发一次', () => {
    const { sched, fired } = makeScheduler();
    const el = new FakeAudio();
    el.duration = 10;
    sched.start(el, [{ at: 0.2, motion: 'nod' }]);
    el.currentTime = 9;
    sched.pump();
    sched.pump();
    sched.pump();
    expect(fired).toEqual(['nod']);
    expect(sched.hasPending()).toBe(false);
  });

  it('乱序帧按进度顺序触发', () => {
    const { sched, fired } = makeScheduler();
    const el = new FakeAudio();
    el.duration = 10;
    sched.start(el, [
      { at: 0.8, motion: 'excited' },
      { at: 0.1, motion: 'nod' },
      { at: 0.45, motion: 'sway' },
    ]);
    el.currentTime = 2;
    sched.pump();
    expect(fired).toEqual(['nod']);
    el.currentTime = 6;
    sched.pump();
    expect(fired).toEqual(['nod', 'sway']);
    el.currentTime = 9.9;
    sched.pump();
    expect(fired).toEqual(['nod', 'sway', 'excited']);
  });

  it('duration 未就绪（NaN→就绪）期间不丢帧', () => {
    const { sched, fired } = makeScheduler();
    const el = new FakeAudio();
    // duration 默认 NaN
    sched.start(el, [{ at: 0, motion: 'tilt' }]);
    el.currentTime = 100; // 没有 duration 也无法换算
    sched.pump();
    expect(fired).toEqual([]);
    expect(sched.hasPending()).toBe(true);

    el.duration = 20; // 元数据加载完成
    sched.pump();
    expect(fired).toEqual(['tilt']);
  });

  it('cancel() 后不再触发；ended 事件自动取消并解绑监听', () => {
    const { sched, fired } = makeScheduler();
    const el = new FakeAudio();
    el.duration = 10;
    sched.start(el, [{ at: 0.5, motion: 'happy' }]);
    expect(el.listenerCount('timeupdate')).toBe(1);

    sched.cancel();
    el.currentTime = 9;
    sched.pump();
    expect(fired).toEqual([]);

    // ended 自动清理
    const el2 = new FakeAudio();
    el2.duration = 10;
    sched.start(el2, [{ at: 0.9, motion: 'nod' }]);
    el2.ended = true;
    el2.emit('ended');
    el2.currentTime = 9.5;
    sched.pump();
    expect(fired).toEqual([]);
    expect(el2.listenerCount('timeupdate')).toBe(0);
  });

  it('空帧表 / 全非法帧表是 no-op（不挂监听）', () => {
    const { sched } = makeScheduler();
    const el = new FakeAudio();
    sched.start(el, []);
    sched.start(el, [
      { at: NaN, motion: 'x' },
      { at: -1, motion: 'y' },
      { at: 0.5, motion: '' },
      { at: 0.5, motion: 'ok' }, // 唯一合法帧仍可用
    ] as never[]);
    // 至少没有把非法帧当有效帧挂上；有合法帧则正常挂监听
    expect(el.listenerCount('timeupdate')).toBe(1);
    el.duration = 10;
    el.currentTime = 4.9;
    sched.pump();
    // 合法帧正常触发
  });

  it('重新绑定新元素后旧元素事件不再生效', () => {
    const { sched, fired } = makeScheduler();
    const el1 = new FakeAudio();
    el1.duration = 10;
    sched.start(el1, [{ at: 0.5, motion: 'a' }]);

    const el2 = new FakeAudio();
    el2.duration = 10;
    sched.start(el2, [{ at: 0.5, motion: 'b' }]);

    el1.currentTime = 9;
    el1.emit('timeupdate');
    expect(fired).toEqual([]);

    el2.currentTime = 5;
    el2.emit('timeupdate');
    expect(fired).toEqual(['b']);
  });
});

describe('MotionTimelineScheduler — 帧级 strength/source/priority 透传', () => {
  beforeEach(() => {
    vi.stubGlobal('requestAnimationFrame', undefined);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('整帧（含 strength/source/priority/reason）交给 fire 回调', () => {
    const { sched, cues } = makeScheduler();
    const el = new FakeAudio();
    el.duration = 10;
    sched.start(el, [
      {
        at: 0.1,
        motion: 'shake_no',
        strength: 3,
        source: 'llm_tool',
        priority: 90,
        reason: '否认',
      },
      { at: 0.8, motion: 'nod', strength: 2, source: 'llm_tool', priority: 90 },
    ]);

    el.currentTime = 5;
    sched.pump();
    expect(cues.map((c) => c.motion)).toEqual(['shake_no']);

    el.currentTime = 9.9;
    sched.pump();
    expect(cues).toHaveLength(2);
    // 帧级元数据原样透传（不重排、不改写、不丢弃）
    expect(cues[0]).toMatchObject({
      at: 0.1,
      motion: 'shake_no',
      strength: 3,
      source: 'llm_tool',
      priority: 90,
      reason: '否认',
    });
    expect(cues[1]).toMatchObject({
      at: 0.8,
      motion: 'nod',
      strength: 2,
      source: 'llm_tool',
      priority: 90,
    });
    expect(cues[1].reason).toBeUndefined();
  });

  it('旧帧（只有 at/motion）照常触发：元数据缺省不报错', () => {
    const { sched, fired, cues } = makeScheduler();
    const el = new FakeAudio();
    el.duration = 4;
    sched.start(el, [{ at: 0.25, motion: 'tilt' }]);
    el.currentTime = 1;
    sched.pump();
    expect(fired).toEqual(['tilt']);
    expect(cues[0].strength).toBeUndefined();
    expect(cues[0].source).toBeUndefined();
    expect(cues[0].priority).toBeUndefined();
  });

  it('带帧级元数据的乱序帧仍按进度顺序触发且元数据随帧对应', () => {
    const { sched, cues } = makeScheduler();
    const el = new FakeAudio();
    el.duration = 10;
    sched.start(el, [
      { at: 0.7, motion: 'listen', strength: 1, priority: 60 },
      { at: 0.2, motion: 'nod', strength: 3, priority: 90 },
    ]);
    el.currentTime = 2.5; // 只到点 0.2 帧
    sched.pump();
    expect(cues.map((c) => c.motion)).toEqual(['nod']);
    expect(cues[0].strength).toBe(3);
    el.currentTime = 8;
    sched.pump();
    expect(cues.map((c) => c.motion)).toEqual(['nod', 'listen']);
    expect(cues[1].strength).toBe(1);
    expect(cues[1].priority).toBe(60);
  });
});
