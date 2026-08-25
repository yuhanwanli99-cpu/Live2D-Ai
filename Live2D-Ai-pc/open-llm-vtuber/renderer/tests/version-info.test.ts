// 版本可观测性：renderer 版本常量与 api-client 端点
import { describe, expect, it, vi } from 'vitest';
import { PRODUCT_VERSION, FRONTEND_VERSION } from '../src/version-info';
import { getVersionInfo } from '../src/settings/api-client';

describe('version-info (renderer)', () => {
  it('exposes a single product version v0.2.0-preview.1', () => {
    expect(PRODUCT_VERSION).toBe('v0.2.0-preview.1');
  });

  it('exposes a frontend version string', () => {
    expect(typeof FRONTEND_VERSION).toBe('string');
    expect(FRONTEND_VERSION.length).toBeGreaterThan(0);
  });
});

describe('version api-client', () => {
  it('getVersionInfo fetches the unified version endpoint', () => {
    const fetcher = vi.fn(() => Promise.resolve(new Response('{}')));
    vi.stubGlobal('fetch', fetcher);
    void getVersionInfo();
    expect(fetcher).toHaveBeenCalledWith('/api/version');
    vi.unstubAllGlobals();
  });
});
