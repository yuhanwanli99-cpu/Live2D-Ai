/**
 * RM7 WebSocket 桥接 — RED 测试（test-designer 产出）
 *
 * 契约来源：contract-pc-render.md §RM7（RM7-01 ~ RM7-05）
 * 架构来源：architecture-pc-render.md §3.7 ws-bridge.ts（连接/重连、
 *   消息分发到 emotion-consumer / lip-sync / model-loader，audio 帧处理）
 * 类型来源：renderer/src/types.ts（WsMessage：AudioMessage / EmotionMessage /
 *   SetModelAndConfMessage）
 *
 * 设计说明：
 *  - WebSocket 用手写可控 mock（vi.stubGlobal），可模拟 open / message / close 事件。
 *  - onmessage 收到的原始 payload 按真实 WS 语义传字符串，WsBridge 自行 JSON.parse。
 *  - 重连用 vi.useFakeTimers() 推进指数退避时间片。
 *  - 本文件为 RED：实现由配对 coder 完成 GREEN。
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { WsBridge } from '../src/ws-bridge';

// ─────────────────────────────────────────────────────────────────────────────
// 手写 WebSocket mock：可控 open / message / close
// ─────────────────────────────────────────────────────────────────────────────
class MockWebSocket {
  static instances: MockWebSocket[] = [];
  url: string;
  readyState: number = 0; // CONNECTING
  onopen: ((ev: any) => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onclose: ((ev: any) => void) | null = null;
  onerror: ((ev: any) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  // —— WsBridge 主动调用的 API ——
  close(): void {}
  send(): void {}

  // —— 测试侧触发事件 ——
  simulateOpen(): void {
    this.readyState = 1;
    if (this.onopen) this.onopen({});
  }
  /** @param payload 字符串（真实 WS 交付格式）或对象（自动 JSON.stringify） */
  simulateMessage(payload: string | object): void {
    if (this.onmessage) {
      this.onmessage({ data: typeof payload === 'string' ? payload : JSON.stringify(payload) });
    }
  }
  simulateClose(): void {
    this.readyState = 3;
    if (this.onclose) this.onclose({ code: 1000, reason: '' });
  }
}

// stub 全局 WebSocket —— WsBridge 内部 new WebSocket(url) 会命中 mock。
// ⚠️ 必须在每个测试前重新 stub：各 describe 的 afterEach 调 vi.unstubAllGlobals()
//    会在第 1 个测试结束后拆除全局 stub，此后 new WebSocket 落到 Node 原生 WebSocket
//    （不 push 进 MockWebSocket.instances）→ instances[0] 为 undefined，测试全挂。
vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
beforeEach(() => {
  vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
});

// ─────────────────────────────────────────────────────────────────────────────
// 工具：可注入的消费回调 mock
// ─────────────────────────────────────────────────────────────────────────────
let lipSync: { play: ReturnType<typeof vi.fn> };
let emotionConsumer: { consume: ReturnType<typeof vi.fn> };
let onModelChange: ReturnType<typeof vi.fn>;
let onSubtitle: ReturnType<typeof vi.fn>;

function resetState(): void {
  MockWebSocket.instances = [];
}
function makeConsumers(): {
  lipSync: typeof lipSync;
  emotionConsumer: typeof emotionConsumer;
  onModelChange: typeof onModelChange;
  onSubtitle: typeof onSubtitle;
} {
  lipSync = { play: vi.fn() };
  emotionConsumer = { consume: vi.fn() };
  onModelChange = vi.fn();
  onSubtitle = vi.fn();
  return { lipSync, emotionConsumer, onModelChange, onSubtitle };
}
/** 创建 WsBridge 并 wire 消费者，connect + 触发 open */
function openBridge(url = 'ws://localhost:12393/ws'): {
  bridge: WsBridge;
  ws: MockWebSocket;
  lipSync: typeof lipSync;
  emotionConsumer: typeof emotionConsumer;
  onModelChange: typeof onModelChange;
  onSubtitle: typeof onSubtitle;
} {
  const consumers = makeConsumers();
  const bridge = new WsBridge(url);
  bridge.setLipSync(consumers.lipSync as any);
  bridge.setEmotionConsumer(consumers.emotionConsumer as any);
  bridge.onModelChange = consumers.onModelChange;
  bridge.onSubtitle = consumers.onSubtitle;
  bridge.connect();
  const ws = MockWebSocket.instances[0];
  ws.simulateOpen();
  return { bridge, ws, ...consumers };
}

describe('RM7 ws-bridge — 连接（url 配置）', () => {
  beforeEach(() => {
    vi.useRealTimers();
    resetState();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('RM7 连接：new WsBridge(url) 用给定 url 建立 WebSocket（connect() 后）', () => {
    const url = 'ws://localhost:12393/interact';
    const bridge = new WsBridge(url);
    bridge.connect();

    expect(MockWebSocket.instances).toHaveLength(1);
    expect(MockWebSocket.instances[0].url).toBe(url);
    bridge.disconnect();
  });

  it('RM7 连接：connect() 前不创建 WebSocket，connect 后才建立', () => {
    const bridge = new WsBridge('ws://localhost:12393/ws');
    expect(MockWebSocket.instances).toHaveLength(0);
    bridge.connect();
    expect(MockWebSocket.instances).toHaveLength(1);
    bridge.disconnect();
  });

  it('RM7 连接：open 后 isConnected() === true', () => {
    const { bridge, ws } = openBridge();
    expect(bridge.isConnected()).toBe(true);
    expect(ws.readyState).toBe(1);
    bridge.disconnect();
  });
});

describe('RM7 ws-bridge — 消息分发', () => {
  beforeEach(() => {
    vi.useRealTimers();
    resetState();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('RM7-01: 收到 "set-model-and-conf" → onModelChange 回调被调用，参数含 model_info.url', () => {
    const { bridge, ws, onModelChange } = openBridge();

    ws.simulateMessage({
      type: 'set-model-and-conf',
      model_info: { url: '/live2d-models/白-免费版/runtime/白-免费版.model3.json', kScale: 1.8 },
    });

    expect(onModelChange).toHaveBeenCalledTimes(1);
    const [modelInfo] = (onModelChange as any).mock.calls[0];
    expect(modelInfo.url).toBe('/live2d-models/白-免费版/runtime/白-免费版.model3.json');
    bridge.disconnect();
  });

  it('RM7-02: 收到 "audio" 且 volumes 非空 → LipSync.play 被调用', () => {
    const { bridge, ws, lipSync } = openBridge();

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5, 0.8, 0.3],
      slice_length: 20,
      visemes: [],
    });

    expect(lipSync.play).toHaveBeenCalledTimes(1);
    expect(lipSync.play).toHaveBeenCalledWith([0.5, 0.8, 0.3], [], 20);
    bridge.disconnect();
  });

  it('RM7-02b: audio.visemes 缺省时传空数组、slice_length 缺省时用默认 20', () => {
    const { bridge, ws, lipSync } = openBridge();

    // 后端可能不携带 visemes / slice_length
    ws.simulateMessage({ type: 'audio', volumes: [0.4] });

    expect(lipSync.play).toHaveBeenCalledTimes(1);
    expect(lipSync.play).toHaveBeenCalledWith([0.4], [], 20);
    bridge.disconnect();
  });

  it('RM7-03: 收到 "audio" 且 actions.parameterOverrides 存在 → EmotionConsumer.consume 被调用', () => {
    const { bridge, ws, emotionConsumer } = openBridge();

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.2],
      slice_length: 20,
      visemes: [],
      actions: { parameterOverrides: { ParamMouthForm: 0.9, ParamBrowLY: 0.3 } },
    });

    expect(emotionConsumer.consume).toHaveBeenCalledTimes(1);
    expect(emotionConsumer.consume).toHaveBeenCalledWith({ ParamMouthForm: 0.9, ParamBrowLY: 0.3 });
    bridge.disconnect();
  });

  it('RM7-03b: audio 无 parameterOverrides → 不调用 emotionConsumer.consume', () => {
    const { bridge, ws, emotionConsumer } = openBridge();

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.2],
      slice_length: 20,
      visemes: [],
      actions: { expressions: ['smile'] }, // 无 parameterOverrides
    });

    expect(emotionConsumer.consume).not.toHaveBeenCalled();
    bridge.disconnect();
  });

  it('RM7-06: 收到 "subtitle" → onSubtitle 被调用（LLM 句子先于 audio 上屏）', () => {
    const { bridge, ws, onSubtitle } = openBridge();

    ws.simulateMessage({
      type: 'subtitle',
      text: '欢迎回来今天也要一起加油哦!',
      name: '白',
    });

    expect(onSubtitle).toHaveBeenCalledTimes(1);
    expect(onSubtitle).toHaveBeenCalledWith({
      text: '欢迎回来今天也要一起加油哦!',
      name: '白',
    });
    bridge.disconnect();
  });

  it('RM7-06b: subtitle 已上屏同一句，紧随的 audio.display_text 不再重复上屏', () => {
    const { bridge, ws, onSubtitle } = openBridge();

    ws.simulateMessage({
      type: 'subtitle',
      text: '欢迎回来今天也要一起加油哦!',
      name: '白',
    });
    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      display_text: { text: '欢迎回来今天也要一起加油哦!', name: '白' },
    });

    expect(onSubtitle).toHaveBeenCalledTimes(1);
    bridge.disconnect();
  });

  it('RM7-06c: 没有 subtitle 时 audio.display_text 仍会上屏（无重复抑制）', () => {
    const { bridge, ws, onSubtitle } = openBridge();

    ws.simulateMessage({
      type: 'audio',
      volumes: [0.5],
      slice_length: 20,
      visemes: [],
      display_text: { text: '直接音频字幕', name: '白' },
    });

    expect(onSubtitle).toHaveBeenCalledTimes(1);
    expect(onSubtitle).toHaveBeenCalledWith({ text: '直接音频字幕', name: '白' });
    bridge.disconnect();
  });
});

describe('RM7 ws-bridge — 消息解析 / 错误处理', () => {
  beforeEach(() => {
    vi.useRealTimers();
    resetState();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('RM7-04: 收到未知 type 消息不抛异常（try-catch 兜底，不崩溃）', () => {
    const { bridge, ws } = openBridge();

    expect(() =>
      ws.simulateMessage({ type: 'unknown-message-type', foo: 1 })
    ).not.toThrow();
    bridge.disconnect();
  });

  it('消息解析错误（无效 JSON）不崩溃', () => {
    const { bridge, ws } = openBridge();

    // 非法 JSON 字符串：JSON.parse 抛错，但 handleMessage 应吞掉
    expect(() => ws.simulateMessage('{{not valid json')).not.toThrow();
    bridge.disconnect();
  });

  it('\u6d88\u606f\u89e3\u6790\u5f02\u5e38\u540e\u5176\u4ed6\u6b63\u5e38\u6d88\u606f\u4ecd\u80fd\u5904\u7406', () => {
    const { bridge, ws, onModelChange } = openBridge();

    ws.simulateMessage('not json at all'); // 先喂脏数据
    ws.simulateMessage({
      type: 'set-model-and-conf',
      model_info: { url: '/m.model3.json' },
    });

    expect(onModelChange).toHaveBeenCalledTimes(1);
    bridge.disconnect();
  });
});

describe('RM7 ws-bridge — 断线重连（指数退避）', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetState();
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('RM7-05: 断开后自动重连——close 后 5s 内触发第一次重连（新建 WebSocket）', () => {
    const { bridge, ws } = openBridge();

    const instanceCountBeforeClose = MockWebSocket.instances.length;
    expect(bridge.isConnected()).toBe(true);

    ws.simulateClose(); // 触发断线
    expect(bridge.isConnected()).toBe(false);

    // 指数退避第一次重连应发生在断开后 5s 内
    vi.advanceTimersByTime(5000);

    expect(MockWebSocket.instances.length).toBe(instanceCountBeforeClose + 1);
    bridge.disconnect();
  });

  it('重连建立的连接 open 后 isConnected() 恢复 true', () => {
    const { bridge, ws } = openBridge();

    ws.simulateClose();
    vi.advanceTimersByTime(5000);

    const reconnected = MockWebSocket.instances[1];
    expect(reconnected).toBeDefined();
    reconnected.simulateOpen();
    expect(bridge.isConnected()).toBe(true);
    bridge.disconnect();
  });

  it('重连退避有上限（最大 30s）——第 n 次重连间隔不超 30s', () => {
    const { bridge, ws } = openBridge();

    ws.simulateClose();
    // 连续推进 120s，重连间隔不应超过 30s，因此应有大量重连尝试（> 4 次）
    for (let i = 0; i < 4; i++) {
      vi.advanceTimersByTime(30001);
      const last = MockWebSocket.instances[MockWebSocket.instances.length - 1];
      last.simulateClose();
    }

    // 初始1个 + close 后已重连至少 4 次 → 至少 5 个实例
    expect(MockWebSocket.instances.length).toBeGreaterThanOrEqual(5);
    bridge.disconnect();
  });

  it('disconnect() 后不再自动重连（主动关闭则退避停止）', () => {
    const { bridge, ws } = openBridge();

    bridge.disconnect();
    const count = MockWebSocket.instances.length;
    ws.simulateClose();

    vi.advanceTimersByTime(30_000);
    expect(MockWebSocket.instances.length).toBe(count);
  });
});
