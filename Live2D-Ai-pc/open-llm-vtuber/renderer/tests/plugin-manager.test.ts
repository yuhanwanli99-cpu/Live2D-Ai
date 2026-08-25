/**
 * plugin-manager.test.ts — 插件管理纯逻辑模块单测
 */

import { describe, it, expect, vi, afterEach } from 'vitest';
import { listPlugins, enablePlugin, disablePlugin } from '../src/plugin-manager';
import type { PluginInfo } from '../src/plugin-manager';

const okJson = (payload: unknown) =>
  ({ ok: true, status: 200, json: async () => payload }) as unknown as Response;

const errJson = (status: number, payload: unknown) =>
  ({ ok: false, status, json: async () => payload }) as unknown as Response;

const samplePlugin = (overrides: Partial<PluginInfo> = {}): PluginInfo => ({
  id: 'p1',
  name: '示例插件',
  version: '1.0.0',
  description: 'desc',
  origin: 'native',
  enabled: true,
  status: 'ok',
  hook_points: ['on_chat'],
  capabilities: ['voice'],
  config_schema: {},
  ...overrides,
});

describe('plugin-manager', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('listPlugins 解析 plugins 并标记 native/external', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () =>
        okJson({
          ok: true,
          plugins: [
            samplePlugin(),
            samplePlugin({ id: 'p2', origin: 'external', enabled: false, status: 'disabled' }),
          ],
        }),
      ),
    );
    const plugins = await listPlugins();
    expect(plugins).toHaveLength(2);
    expect(plugins[0].origin).toBe('native');
    expect(plugins[0].enabled).toBe(true);
    expect(plugins[1].origin).toBe('external');
    expect(plugins[1].enabled).toBe(false);
  });

  it('enablePlugin 发送 POST 并返回 plugin', async () => {
    const spy = vi.fn(async () => okJson({ ok: true, plugin: samplePlugin() }));
    vi.stubGlobal('fetch', spy);
    const plugin = await enablePlugin('p1');
    expect(plugin.id).toBe('p1');
    const [url, init] = spy.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('/api/plugins/p1/enable');
    expect(init.method).toBe('POST');
  });

  it('disablePlugin 发送 POST 并返回 plugin', async () => {
    const spy = vi.fn(async () =>
      okJson({ ok: true, plugin: samplePlugin({ enabled: false, status: 'disabled' }) }),
    );
    vi.stubGlobal('fetch', spy);
    const plugin = await disablePlugin('p1');
    expect(plugin.enabled).toBe(false);
    const [url, init] = spy.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('/api/plugins/p1/disable');
    expect(init.method).toBe('POST');
  });

  it('非 2xx 抛后端 error（可读消息）', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => errJson(400, { ok: false, error: '插件损坏' })));
    await expect(enablePlugin('p1')).rejects.toThrow('插件损坏');
  });

  it('2xx 但 data.error 抛 error', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => okJson({ ok: true, error: '后端失败' })));
    await expect(listPlugins()).rejects.toThrow('后端失败');
  });
});
