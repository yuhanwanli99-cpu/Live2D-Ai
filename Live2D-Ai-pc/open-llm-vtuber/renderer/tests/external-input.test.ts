/**
 * external-input.test.ts — 「外部接入」前端纯逻辑单测
 *
 * 被测：renderer/src/external-input.ts 的纯函数与 API 客户端（fetch 用 stub）。
 * 纪律：不碰真实网络、不依赖 DOM 渲染细节；断言贴合后端 external_input.py 的契约。
 */
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

import {
  EMPTY_EXTERNAL_STATUS,
  buildCurlExample,
  describeGateScope,
  fetchExternalStatus,
  formatRecentLine,
  updateExternalSettings,
  validateGateChange,
  type ExternalStatus,
} from '../src/external-input';

const status = (over: Partial<ExternalStatus> = {}): ExternalStatus => ({
  ...EMPTY_EXTERNAL_STATUS,
  ...over,
});

describe('describeGateScope —— 状态必须对用户如实交代范围', () => {
  it('关闭时明确说"外部程序无法让角色说话"', () => {
    expect(describeGateScope(status({ enabled: false }))).toContain('已断开');
  });

  it('仅本机时点名 127.0.0.1', () => {
    const text = describeGateScope(status({ enabled: true }));
    expect(text).toContain('仅本机');
    expect(text).toContain('127.0.0.1');
  });

  it('局域网 + 令牌时说明需要令牌', () => {
    const text = describeGateScope(
      status({ enabled: true, allow_remote: true, token_set: true }),
    );
    expect(text).toContain('局域网');
    expect(text).toContain('令牌');
  });

  it('allow_remote 但没令牌时不谎称已放开局域网（回落到仅本机文案）', () => {
    const text = describeGateScope(
      status({ enabled: true, allow_remote: true, token_set: false }),
    );
    expect(text).toContain('仅本机');
  });
});

describe('validateGateChange —— 与后端同一条规则（前置挡住"开着门没锁"）', () => {
  it('局域网 + 无令牌 → 拒绝并提示生成令牌', () => {
    const error = validateGateChange({ enabled: true, allowRemote: true, tokenSet: false });
    expect(error).toBeTruthy();
    expect(error).toContain('令牌');
  });

  it('局域网 + 有令牌 → 通过', () => {
    expect(validateGateChange({ enabled: true, allowRemote: true, tokenSet: true })).toBeNull();
  });

  it('仅本机（不开局域网）→ 通过，无论有无令牌', () => {
    expect(validateGateChange({ enabled: true, allowRemote: false, tokenSet: false })).toBeNull();
    expect(validateGateChange({ enabled: false, allowRemote: false, tokenSet: true })).toBeNull();
  });
});

describe('buildCurlExample —— 用户复制即可用', () => {
  it('包含 origin、端点、Content-Type 与 text 字段', () => {
    const cmd = buildCurlExample('http://127.0.0.1:12393', '');
    expect(cmd).toContain('http://127.0.0.1:12393/api/external/chat');
    expect(cmd).toContain("-H 'Content-Type: application/json'");
    expect(cmd).toContain('"text"');
  });

  it('有令牌时带上 X-Live2DAI-Token 头', () => {
    const cmd = buildCurlExample('http://192.168.1.7:12393', 'tok-123');
    expect(cmd).toContain("X-Live2DAI-Token: tok-123");
  });

  it('无令牌时不生成空的令牌头', () => {
    expect(buildCurlExample('http://127.0.0.1:12393', '')).not.toContain('X-Live2DAI-Token');
  });

  it('origin 末尾斜杠不会产生双斜杠；origin 为空时兜底到本机', () => {
    expect(buildCurlExample('http://127.0.0.1:12393/', '')).toContain('12393/api/external/chat');
    expect(buildCurlExample('', '')).toContain('http://127.0.0.1:12393');
  });

  it('自定义端点被采用', () => {
    expect(buildCurlExample('http://a.b', '', '/api/x')).toContain('http://a.b/api/x');
  });
});

describe('formatRecentLine —— 最近记录一行可读', () => {
  it('接收成功：含来源与文本', () => {
    const line = formatRecentLine({ at: 1_700_000_000, source: '127.0.0.1', text: '你好', ok: true });
    expect(line).toContain('127.0.0.1');
    expect(line).toContain('已接收');
    expect(line).toContain('你好');
  });

  it('被拒绝：带上拒绝原因', () => {
    const line = formatRecentLine({
      at: 1_700_000_000,
      source: '192.168.1.9',
      text: 'hi',
      ok: false,
      detail: '访问令牌不正确',
    });
    expect(line).toContain('已拒绝');
    expect(line).toContain('访问令牌不正确');
  });

  it('空文本用占位符而不是空白', () => {
    expect(formatRecentLine({ at: 0, source: 'x', text: '', ok: true })).toContain('(空)');
  });
});

describe('API 客户端 —— 错误必须冒泡，不静默成"成功"', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    vi.restoreAllMocks();
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('fetchExternalStatus 正常解析并补齐缺省字段', async () => {
    globalThis.fetch = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({ ok: true, enabled: true, clients: ['c1'], accepted: 2 }),
    })) as unknown as typeof fetch;

    const s = await fetchExternalStatus();
    expect(s.enabled).toBe(true);
    expect(s.clients).toEqual(['c1']);
    expect(s.accepted).toBe(2);
    expect(s.rejected).toBe(0);
    expect(s.endpoint).toBe('/api/external/chat');
  });

  it('后端 403（仅本机可改）→ 抛出后端错误文案', async () => {
    globalThis.fetch = vi.fn(async () => ({
      ok: false,
      status: 403,
      json: async () => ({ ok: false, error: '外部接入开关只能从本机修改' }),
    })) as unknown as typeof fetch;

    await expect(updateExternalSettings({ enabled: true })).rejects.toThrow(
      '外部接入开关只能从本机修改',
    );
  });

  it('响应不是 JSON 时退化为 HTTP 状态错误（不抛裸解析异常）', async () => {
    globalThis.fetch = vi.fn(async () => ({
      ok: false,
      status: 500,
      json: async () => {
        throw new Error('not json');
      },
    })) as unknown as typeof fetch;

    await expect(fetchExternalStatus()).rejects.toThrow('HTTP 500');
  });
});
