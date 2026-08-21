/**
 * model-upload.test.ts — Live2D 模型 zip 上传纯逻辑单测
 *
 * 被测：renderer/src/model-upload.ts（uploadModelZip）。
 * 用 vi.stubGlobal('fetch', ...) mock fetch，覆盖成功与非 2xx 错误路径。
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { uploadModelZip } from '../src/model-upload';

const okJson = (payload: unknown) =>
  ({ ok: true, status: 200, json: async () => payload }) as unknown as Response;

const errJson = (status: number, payload: unknown) =>
  ({ ok: false, status, json: async () => payload }) as unknown as Response;

describe('uploadModelZip', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('POST /api/models/upload，FormData 携带 file 与可选字段', async () => {
    const spy = vi.fn(async () =>
      okJson({ ok: true, model_id: 'm1', model3_json_path: '/models/m1/m1.model3.json', applied: true }),
    );
    vi.stubGlobal('fetch', spy);

    const file = new File(['zip-bytes'], 'model.zip');
    const result = await uploadModelZip(file, {
      modelId: 'm1',
      displayName: 'My Model',
      overwrite: true,
    });

    expect(result.model_id).toBe('m1');
    expect(result.model3_json_path).toBe('/models/m1/m1.model3.json');
    expect(result.applied).toBe(true);

    const [url, init] = spy.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('/api/models/upload');
    expect(init.method).toBe('POST');
    const form = init.body as FormData;
    expect(form.get('file')).toBe(file);
    expect(form.get('model_id')).toBe('m1');
    expect(form.get('display_name')).toBe('My Model');
    expect(form.get('overwrite')).toBe('true');
  });

  it('不提供可选字段时不追加对应表单项', async () => {
    const spy = vi.fn(async () =>
      okJson({ ok: true, model_id: 'm2', model3_json_path: '/x', applied: false }),
    );
    vi.stubGlobal('fetch', spy);

    await uploadModelZip(new File(['z'], 'm.zip'));
    const form = spy.mock.calls[0][1].body as FormData;
    expect(form.has('file')).toBe(true);
    expect(form.has('model_id')).toBe(false);
    expect(form.has('display_name')).toBe(false);
    expect(form.has('overwrite')).toBe(false);
  });

  it('非 2xx 且 body 含 error → 抛后端文案', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => errJson(400, { error: '不支持的 zip 结构' })));
    await expect(uploadModelZip(new File(['z'], 'm.zip'))).rejects.toThrow('不支持的 zip 结构');
  });

  it('非 2xx 且 body 无 error → 回退 HTTP 状态码', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => errJson(500, {})));
    await expect(uploadModelZip(new File(['z'], 'm.zip'))).rejects.toThrow('HTTP 500');
  });
});
