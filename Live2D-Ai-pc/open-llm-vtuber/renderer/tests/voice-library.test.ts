/**
 * voice-library.test.ts — 声音库纯逻辑模块单测
 */

import { describe, it, expect, vi, afterEach } from 'vitest';
import { listVoices, uploadVoice } from '../src/voice-library';

const okJson = (payload: unknown) =>
  ({ ok: true, status: 200, json: async () => payload }) as unknown as Response;

const errJson = (status: number, payload: unknown) =>
  ({ ok: false, status, json: async () => payload }) as unknown as Response;

describe('voice-library', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('listVoices 解析 voices 数组', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => okJson({ ok: true, voices: [{ id: 'v1', active: false }] })));
    const voices = await listVoices();
    expect(voices).toEqual([{ id: 'v1', active: false }]);
  });

  it('uploadVoice 使用 FormData 并返回 voice', async () => {
    const spy = vi.fn(async () => okJson({ ok: true, voice: { id: 'v1', name: 'A' } }));
    vi.stubGlobal('fetch', spy);
    const file = new File(['wav'], 'ref.wav');
    const voice = await uploadVoice(file, 'A');
    expect(voice.id).toBe('v1');
    const [url, init] = spy.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('/api/voice/upload');
    expect(init.method).toBe('POST');
    expect((init.body as FormData).get('file')).toBe(file);
  });

  it('非 2xx 抛后端 error', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => errJson(400, { error: 'bad audio' })));
    await expect(uploadVoice(new File(['x'], 'x.wav'))).rejects.toThrow('bad audio');
  });
});
