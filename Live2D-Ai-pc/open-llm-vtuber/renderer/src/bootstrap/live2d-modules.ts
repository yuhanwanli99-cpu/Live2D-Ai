/**
 * 启动期 live2d 依赖模块加载器：动态导入 model-loader / camera / idle-motion。
 *
 * 从 main.ts 抽取，使 initApp 专注于渲染管线接线，加载器/缓存独立可测。
 */

/** live2d 依赖模块（动态导入缓存） */
export interface Live2dModules {
  modelLoader: typeof import('../model-loader');
  camera: typeof import('../camera');
  idleMotion: typeof import('../idle/idle-motion');
}

let live2dModulesCache: Live2dModules | null = null;

/**
 * 动态加载 live2d 依赖模块（model-loader / camera / idle-motion）。
 * 这三个模块值导入 pixi-live2d-display（其模块级访问 window）——node/vitest 环境
 * 导入必然失败；这里 try/catch 兜底，Web 环境加载成功后缓存复用。
 */
export async function loadLive2dModules(): Promise<Live2dModules | null> {
  if (live2dModulesCache) return live2dModulesCache;
  try {
    const [modelLoader, camera, idleMotion] = await Promise.allSettled([
      import('../model-loader'),
      import('../camera'),
      import('../idle/idle-motion'),
    ]);
    const settled = (
      r: PromiseSettledResult<unknown>,
      name: string,
    ): boolean => {
      if (r.status === 'fulfilled') return true;
      console.warn(`[renderer] live2d 依赖模块 "${name}" 加载失败:`, r.reason);
      return false;
    };
    if (
      !settled(modelLoader, 'model-loader') ||
      !settled(camera, 'camera') ||
      !settled(idleMotion, 'idle-motion')
    ) {
      return null;
    }
    live2dModulesCache = {
      modelLoader: (modelLoader as PromiseFulfilledResult<
        typeof import('../model-loader')
      >).value,
      camera: (camera as PromiseFulfilledResult<
        typeof import('../camera')
      >).value,
      idleMotion: (idleMotion as PromiseFulfilledResult<
        typeof import('../idle/idle-motion')
      >).value,
    };
    return live2dModulesCache;
  } catch (err) {
    console.error('[renderer] live2d 依赖模块加载失败（渲染不可用）:', err);
    return null;
  }
}
