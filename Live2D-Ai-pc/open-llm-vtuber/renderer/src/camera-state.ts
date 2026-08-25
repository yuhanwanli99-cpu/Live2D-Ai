/**
 * camera-state.ts — PC 镜头模式本地状态。
 *
 * 为什么放 localStorage 而不是 conf.yaml：
 *   - 镜头模式是前端呈现偏好，改完应即时生效、刷新保留；
 *   - 不需要后端热重载，也不属于 LLM/TTS 语义配置。
 *
 * 默认值按 2026-08 需求改为 'half-body'（半身镜头，Live2D 动作更明显）。
 * 用户后续可在设置页「形象 / 背景」切换；本地损坏/非法值一律回落默认。
 */

export type CameraMode = 'fit' | 'half-body';

export const CAMERA_MODE_STORAGE_KEY = 'live2dai.camera_mode';
export const DEFAULT_CAMERA_MODE: CameraMode = 'half-body';

/** 解析本地存储的镜头模式；非法值回退 DEFAULT_CAMERA_MODE。 */
export function parseCameraMode(raw: string | null): CameraMode {
  if (raw === 'fit' || raw === 'half-body') return raw;
  return DEFAULT_CAMERA_MODE;
}

/** 注入式存储（浏览器传 localStorage，测试传内存 fake）。 */
export function createCameraModeStore(
  storage: Pick<Storage, 'getItem' | 'setItem'>,
): {
  load(): CameraMode;
  save(mode: CameraMode): void;
} {
  return {
    load(): CameraMode {
      try {
        return parseCameraMode(storage.getItem(CAMERA_MODE_STORAGE_KEY));
      } catch {
        return DEFAULT_CAMERA_MODE;
      }
    },
    save(mode: CameraMode): void {
      if (mode !== 'fit' && mode !== 'half-body') return;
      storage.setItem(CAMERA_MODE_STORAGE_KEY, mode);
    },
  };
}
