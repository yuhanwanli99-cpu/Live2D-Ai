/**
 * bootstrap/ 启动期工具提取 — 行为保持测试
 *
 * 验证 createDefaultStorage 与 loadLive2dModules 从 main.ts 抽取后：
 * 1. node/vitest 环境下 storage 降级为 no-op（不抛错）。
 * 2. node/vitest 环境下 live2d 动态导入返回 null（与原 main.ts 行为一致）。
 * 3. main.ts 不再保留已抽取的底层实现（静态回归断言）。
 */
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createDefaultStorage } from '../src/bootstrap/storage';
import { loadLive2dModules } from '../src/bootstrap/live2d-modules';

const RENDERER_ROOT = resolve(__dirname, '..');
const MAIN_TS = join(RENDERER_ROOT, 'src', 'main.ts');

describe('bootstrap/storage', () => {
  it('node 环境返回 no-op storage，读写不抛错', () => {
    const storage = createDefaultStorage();
    expect(storage.getItem("any")).toBeNull();
    expect(() => storage.setItem("any", "value")).not.toThrow();
    expect(() => storage.removeItem("any")).not.toThrow();
    expect(storage.getItem("any")).toBeNull();
  });
});

describe('bootstrap/live2d-modules', () => {
  it('node 环境动态导入失败并返回 null（与原 main.ts 降级一致）', async () => {
    const modules = await loadLive2dModules();
    expect(modules).toBeNull();
  });
});

describe('bootstrap 提取回归：main.ts 不再包含已迁移实现', () => {
  const source = readFileSync(MAIN_TS, 'utf-8');

  it('main.ts 不再定义 createDefaultStorage', () => {
    expect(source.includes("function createDefaultStorage(")).toBe(false);
    expect(source.includes("const createDefaultStorage =")).toBe(false);
  });

  it('main.ts 不再定义 loadLive2dModules', () => {
    expect(source.includes("function loadLive2dModules(")).toBe(false);
    expect(source.includes("const loadLive2dModules =")).toBe(false);
  });

  it('main.ts 通过 bootstrap/ 导入这两个工具', () => {
    expect(source.includes("from './bootstrap/storage'") || source.includes('from "./bootstrap/storage"')).toBe(true);
    expect(source.includes("from './bootstrap/live2d-modules'") || source.includes('from "./bootstrap/live2d-modules"')).toBe(true);
  });
});
