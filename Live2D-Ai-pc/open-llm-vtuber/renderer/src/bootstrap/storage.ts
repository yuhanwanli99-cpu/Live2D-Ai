/**
 * 启动期 storage 工具：浏览器 localStorage 降级 no-op（node/隐私模式）。
 *
 * 从 main.ts 抽取，使 initApp 专注于渲染管线接线，storage 行为独立可测。
 */

/**
 * 默认 storage（C-M6）：浏览器 localStorage；node/隐私模式等不可用场景降级 no-op
 * （持久化静默禁用，渲染/交互不受影响）。
 */
export function createDefaultStorage(): Pick<
  Storage,
  'getItem' | 'setItem' | 'removeItem'
> {
  try {
    if (typeof window !== 'undefined' && window.localStorage) {
      return window.localStorage;
    }
  } catch {
    // 隐私模式等场景访问 localStorage 可能抛 SecurityError → 降级 no-op
  }
  return {
    getItem: () => null,
    setItem: () => {
      /* no-op */
    },
    removeItem: () => {
      /* no-op */
    },
  };
}
