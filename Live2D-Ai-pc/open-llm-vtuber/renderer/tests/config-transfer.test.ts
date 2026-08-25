/**
 * config-transfer.test.ts — 配置导入/导出纯逻辑单测
 *
 * 被测：renderer/src/config-transfer.ts（exportConfig / importConfigFile）。
 * 用 vi.stubGlobal('fetch', ...) mock fetch，覆盖成功与非 2xx 错误路径。
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { exportConfig, importConfigFile } from '../src/config-transfer';

const okJson = (payload: unknown) =>
  ({ ok: true, status: 200, json: async () => payload }) as unknown as Response;

const errJson = (status: number, payload: unknown) =>
  ({ ok: false, status, json: async () => payload }) as unknown as Response;

describe('exportConfig', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('GET /api/config/export 并解析 conf_yaml/persona', async () => {
    const spy = vi.fn(async () =>
      okJson({
        ok: true,
        format: 'live2dai-config',
        version: 1,
        conf_yaml: 'character_name: 白',
        persona: { name: '白', role: '助手' },
      }),
    );
    vi.stubGlobal('fetch', spy);

    const result = await exportConfig();
    expect(result.conf_yaml).toBe('character_name: 白');
    expect(result.persona).toEqual({ name: '白', role: '助手' });
    expect(result.format).toBe('live2dai-config');
    expect(spy.mock.calls[0][0]).toBe('/api/config/export');
  });

  it('非 2xx → 抛后端文案', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => errJson(500, { error: '导出失败' })));
    await expect(exportConfig()).rejects.toThrow('导出失败');
  });
});

describe('importConfigFile', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('读文本 → JSON.parse → POST conf_yaml + persona', async () => {
    const spy = vi.fn(async () =>
      okJson({ ok: true, persisted: true, message: '已导入并持久化' }),
    );
    vi.stubGlobal('fetch', spy);

    const file = new File(
      [JSON.stringify({ conf_yaml: 'x: 1', persona: { name: 'A' } })],
      'live2dai-config.json',
    );
    const result = await importConfigFile(file);
    expect(result.persisted).toBe(true);
    expect(result.message).toBe('已导入并持久化');

    const [url, init] = spy.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('/api/config/import');
    expect(init.method).toBe('POST');
    expect(JSON.parse(String(init.body))).toEqual({ conf_yaml: 'x: 1', persona: { name: 'A' } });
  });

  it('缺 conf_yaml 时补空串，缺 persona 补空对象', async () => {
    const spy = vi.fn(async () => okJson({ ok: true, persisted: true, message: 'ok' }));
    vi.stubGlobal('fetch', spy);

    await importConfigFile(new File(['{}'], 'c.json'));
    const init = spy.mock.calls[0][1] as RequestInit;
    expect(JSON.parse(String(init.body))).toEqual({ conf_yaml: '', persona: {} });
  });

  it('非法 JSON 直接抛解析错误且不发请求', async () => {
    const spy = vi.fn();
    vi.stubGlobal('fetch', spy);
    await expect(importConfigFile(new File(['not json'], 'c.json'))).rejects.toThrow('解析失败');
    expect(spy).not.toHaveBeenCalled();
  });

  it('非 2xx → 抛后端文案', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => errJson(400, { error: '非法配置' })));
    await expect(importConfigFile(new File(['{}'], 'c.json'))).rejects.toThrow('非法配置');
  });
});
