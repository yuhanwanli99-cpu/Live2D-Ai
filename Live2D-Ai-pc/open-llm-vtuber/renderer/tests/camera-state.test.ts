import { describe, it, expect } from 'vitest';
import {
  CAMERA_MODE_STORAGE_KEY,
  DEFAULT_CAMERA_MODE,
  createCameraModeStore,
  parseCameraMode,
} from '../src/camera-state';

function makeStorage(initial: Record<string, string> = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k: string) => map.get(k) ?? null,
    setItem: (k: string, v: string) => void map.set(k, v),
  };
}

describe('camera-state', () => {
  it('默认半身镜头', () => {
    expect(DEFAULT_CAMERA_MODE).toBe('half-body');
    expect(parseCameraMode(null)).toBe('half-body');
    expect(parseCameraMode('bad')).toBe('half-body');
  });

  it('合法值原样解析', () => {
    expect(parseCameraMode('fit')).toBe('fit');
    expect(parseCameraMode('half-body')).toBe('half-body');
  });

  it('store 读写', () => {
    const storage = makeStorage();
    const store = createCameraModeStore(storage);
    expect(store.load()).toBe('half-body');
    store.save('fit');
    expect(storage.getItem(CAMERA_MODE_STORAGE_KEY)).toBe('fit');
    expect(store.load()).toBe('fit');
  });
});
