/**
 * ws-bridge × motionTimeline 集成测试 —— 括注动作帧按播放进度触发
 *
 * 契约：
 *  - audio 帧 actions.motionTimeline 存在 → 不再立即播一次性 motion；
 *    音频真正开始播放后挂调度器，进度到点才 bodyMotion.play；
 *  - 无时间轴 → 保持旧行为（帧到达即播 actions.motion）；
 *  - control "interrupt" → stopAudio 取消未触发的动作帧；
 *  - 静默帧（audio=null）带时间轴 → 不创建音频、不触发。
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { WsBridge } from '../src/ws-bridge';
import { ActionPriority } from '../src/choreography';
import type { TimelineTimeSource } from '../src/motion-timeline';

// 手写 WebSocket mock（与 rm7-wsbridge 同款可控桩）
class MockWebSocket {
  static instances: MockWebSocket[] = [];
  url: string;
  readyState = 0;
  onopen: ((ev: unknown) => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onclose: ((ev: unknown) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }
  close(): void {}
  send(): void {}
  simulateOpen(): void {
    this.readyState = 1;
    this.onopen?.({});
  }
  simulateMessage(payload: string | object): void {
    this.onmessage?.({
      data: typeof payload === 'string' ? payload : JSON.stringify(payload),
    });
  }
}

// 手写 HTMLAudioElement mock：记录实例、可控行为
class MockAudio implements TimelineTimeSource {
  static instances: MockAudio[] = [];
  src: string;
  preload = '';
  onended: (() => void) | null = null;
  onerror: (() => void) | null = null;
  currentTime = 0;
  duration = NaN;
  ended = false;
  private listeners = new Map<string, Set<() => void>>();

  constructor(src: string) {
    this.src = src;
    MockAudio.instances.push(this);
  }
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
  async play(): Promise<void> {}
  pause(): void {}
}

vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
vi.stubGlobal('Audio', MockAudio as unknown as typeof Audio);

function openBridge(bodyPlay: ReturnType<typeof vi.fn>): {
  bridge: WsBridge;
  ws: MockWebSocket;
} {
  const bridge = new WsBridge('ws://localhost:12393/ws');
  bridge.setBodyMotion({
    play: bodyPlay,
    release: vi.fn(),
  } as never);
  bridge.connect();
  const ws = MockWebSocket.instances.at(-1)!;
  ws.simulateOpen();
  return { bridge, ws };
}

/** 等待 el.play() 的 promise 链 resolve（调度器在 .then 里启动） */
async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe('ws-bridge × motionTimeline — 播放位置触发的半身动作', () => {
  beforeEach(() => {
    // unstubAllGlobals 会拆除模块级 stub，必须每个用例前重新挂
    // （与 rm7-wsbridge.test.ts 同款约定）
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    vi.stubGlobal('Audio', MockAudio as unknown as typeof Audio);
    MockWebSocket.instances = [];
    MockAudio.instances = [];
    vi.stubGlobal('requestAnimationFrame', undefined); // 只由手动 emit/pump 驱动
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('带 motionTimeline 的 audio 帧：不立即播动作，播放到对应进度才触发', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
      actions: { motion: 'nod', motionTimeline: [{ at: 0.5, motion: 'listen' }] },
    });
    // 时间轴接管：一次性 motion 不立即触发
    expect(bodyPlay).not.toHaveBeenCalled();

    await flushMicrotasks();
    const el = MockAudio.instances[0];
    expect(el).toBeDefined();

    el.duration = 10;
    el.currentTime = 3;
    el.emit('timeupdate');
    expect(bodyPlay).not.toHaveBeenCalled();

    el.currentTime = 5;
    el.emit('timeupdate');
    expect(bodyPlay).toHaveBeenCalledTimes(1);
    expect(bodyPlay).toHaveBeenCalledWith(
      'listen',
      2,
      expect.objectContaining({ source: 'cue' }),
    );
    bridge.disconnect();
  });

  it('无 motionTimeline 时保留旧行为：audio 帧到达即播 actions.motion', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
      actions: { motion: 'look_around' },
    });

    expect(bodyPlay).toHaveBeenCalledTimes(1);
    expect(bodyPlay).toHaveBeenCalledWith(
      'look_around',
      2,
      expect.objectContaining({ source: 'audio' }),
    );
    await flushMicrotasks();
    bridge.disconnect();
  });

  it('interrupt（stopAudio）取消未触发的动作帧', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
      actions: { motionTimeline: [{ at: 0.9, motion: 'surprise' }] },
    });
    await flushMicrotasks();

    ws.simulateMessage({ type: 'control', text: 'interrupt' });
    const el = MockAudio.instances[0];
    el.duration = 10;
    el.currentTime = 9.5;
    el.emit('timeupdate');
    expect(bodyPlay).not.toHaveBeenCalled();
    bridge.disconnect();
  });

  it('静默帧（audio=null）带时间轴：不创建音频元素、不触发', () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);

    ws.simulateMessage({
      type: 'audio',
      volumes: [],
      slice_length: 20,
      visemes: [],
      audio: null,
      actions: { motionTimeline: [{ at: 0.1, motion: 'nod' }] },
    });

    expect(MockAudio.instances).toHaveLength(0);
    expect(bodyPlay).not.toHaveBeenCalled();
    bridge.disconnect();
  });
});

describe('ws-bridge × 帧级 strength/source/priority — 交给单 active 调度器', () => {
  beforeEach(() => {
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    vi.stubGlobal('Audio', MockAudio as unknown as typeof Audio);
    MockWebSocket.instances = [];
    MockAudio.instances = [];
    vi.stubGlobal('requestAnimationFrame', undefined);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('时间轴帧携带 strength/source/priority：按帧级值播放（llm_tool → Cue 档）', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);
    const played: Array<[string, number]> = [];
    bridge.onActionPlayed = (action, strength) => played.push([action, strength]);

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
      actions: {
        motionTimeline: [
          { at: 0.1, motion: 'shake_no', strength: 3, source: 'llm_tool', priority: 90, reason: '否认' },
          { at: 0.8, motion: 'nod', strength: 1, source: 'llm_tool', priority: 90 },
        ],
      },
    });
    await flushMicrotasks();
    const el = MockAudio.instances[0];
    el.duration = 10;

    el.currentTime = 2;
    el.emit('timeupdate');
    expect(bodyPlay).toHaveBeenCalledTimes(1);
    expect(bodyPlay).toHaveBeenNthCalledWith(
      1,
      'shake_no',
      3,
      expect.objectContaining({ priority: ActionPriority.Cue, source: 'llm_tool' }),
    );
    expect(played[0]).toEqual(['shake_no', 3]);

    el.currentTime = 9;
    el.emit('timeupdate');
    expect(bodyPlay).toHaveBeenCalledTimes(2);
    // 帧序即触发顺序：第二帧用自己的强度/来源，不串帧
    expect(bodyPlay).toHaveBeenNthCalledWith(
      2,
      'nod',
      1,
      expect.objectContaining({ priority: ActionPriority.Cue, source: 'llm_tool' }),
    );
    expect(played[1]).toEqual(['nod', 1]);
    bridge.disconnect();
  });

  it('用户命令帧（priority≥100）映射 Live 档位，可打断低优先级动作', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
      actions: {
        motionTimeline: [
          { at: 0.4, motion: 'look_around', source: 'user_command', priority: 100 },
        ],
      },
    });
    await flushMicrotasks();
    const el = MockAudio.instances[0];
    el.duration = 8;
    el.currentTime = 4;
    el.emit('timeupdate');
    expect(bodyPlay).toHaveBeenCalledWith(
      'look_around',
      2,
      expect.objectContaining({ priority: ActionPriority.Live, source: 'user_command' }),
    );
    bridge.disconnect();
  });

  it('缺省元数据的旧帧回退旧行为（强度 2 / Cue / cue 来源）', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
      actions: { motionTimeline: [{ at: 0.5, motion: 'tilt' }] },
    });
    await flushMicrotasks();
    const el = MockAudio.instances[0];
    el.duration = 10;
    el.currentTime = 6;
    el.emit('timeupdate');
    expect(bodyPlay).toHaveBeenCalledWith(
      'tilt',
      2,
      expect.objectContaining({ priority: ActionPriority.Cue, source: 'cue' }),
    );
    bridge.disconnect();
  });

  it('新 epoch（startEpoch）清空未触发的工具动作帧', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
      actions: {
        motionTimeline: [{ at: 0.9, motion: 'surprise', strength: 3, source: 'llm_tool' }],
      },
    });
    await flushMicrotasks();

    // 用户发新消息 / 打断 → 新 epoch：未触发的时间轴帧必须全部作废
    bridge.startEpoch();
    const el = MockAudio.instances[0];
    el.duration = 10;
    el.currentTime = 9.5;
    el.emit('timeupdate');
    expect(bodyPlay).not.toHaveBeenCalled();
    bridge.disconnect();
  });
});

describe('ws-bridge × sentence_start — 字幕-声音同步回调', () => {
  beforeEach(() => {
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
    vi.stubGlobal('Audio', MockAudio as unknown as typeof Audio);
    MockWebSocket.instances = [];
    MockAudio.instances = [];
    vi.stubGlobal('requestAnimationFrame', undefined);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('句首块开播时触发 onSentenceSpoken；后续块不重复触发', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);
    const spoken = vi.fn();
    bridge.onSentenceSpoken = spoken;

    // 句首块：真实 play() 成功后才触发 onSentenceSpoken（与 rm7-wsbridge 契约一致）
    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
      sentence_start: true,
    });
    expect(spoken).toHaveBeenCalledTimes(0);
    await flushMicrotasks();
    expect(spoken).toHaveBeenCalledTimes(1);

    // 同句后续块：无标记 → 不再触发
    ws.simulateMessage({
      type: 'audio',
      volumes: [0.4],
      slice_length: 20,
      visemes: [],
      audio: 'QkJC',
    });
    await flushMicrotasks();
    expect(spoken).toHaveBeenCalledTimes(1);
    bridge.disconnect();
  });

  it('静默帧带 sentence_start：立即触发回调（对话提早出）', () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);
    const spoken = vi.fn();
    bridge.onSentenceSpoken = spoken;

    ws.simulateMessage({
      type: 'audio',
      volumes: [],
      slice_length: 20,
      visemes: [],
      audio: null,
      sentence_start: true,
    });

    expect(spoken).toHaveBeenCalledTimes(1);
    expect(MockAudio.instances).toHaveLength(0); // 无音频可播
    bridge.disconnect();
  });

  it('无 sentence_start 的旧 payload 不触发回调（向后兼容）', async () => {
    const bodyPlay = vi.fn();
    const { bridge, ws } = openBridge(bodyPlay);
    const spoken = vi.fn();
    bridge.onSentenceSpoken = spoken;

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      audio: 'QUFB',
    });
    await flushMicrotasks();
    expect(spoken).not.toHaveBeenCalled();
    bridge.disconnect();
  });
});
