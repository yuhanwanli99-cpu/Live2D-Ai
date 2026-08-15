/**
 * RM8 纹理适配 — RED 测试（test-designer 产出）
 *
 * 契约来源：contract-pc-render.md §RM8（RM8-01 ~ RM8-03）
 * 架构来源：architecture-pc-render.md §3.8 texture-adapter.ts
 *   - checkMaxTextureSize 必须在 WebGL context 存在时调用（否则抛出明确错误，不静默失败）
 *   - needsDownsample：16384 纹理在 maxSize<16384（如 8192）时触发降采样路径
 * 模型事实：model-facts-bai.md —— 白-免费版.16384/texture_00.png 为 16384×16384
 *
 * 设计说明：
 *  - WebGL context 用最小 mock fake（只暴露 getParameter + MAX_TEXTURE_SIZE 常量），
 *    符合方案 A（mock WebGL context），Node 环境可机械执行。
 *  - 本文件为 RED：实现由配对 coder 完成 GREEN。
 */
import { describe, it, expect, vi } from 'vitest';

import { checkMaxTextureSize, needsDownsample } from '../src/texture-adapter';

/** 构造一个最小 WebGLRenderingContext 假对象，返回指定 MAX_TEXTURE_SIZE */
function makeFakeGl(maxTextureSize: number): WebGLRenderingContext {
  return {
    // WebGL 常量：MAX_TEXTURE_SIZE 的真实值是 0x8871 (33761)
    MAX_TEXTURE_SIZE: 0x8871,
    getParameter: vi.fn((pname: number) => {
      if (pname === 0x8871) return maxTextureSize;
      throw new Error(`unexpected getParameter pname ${pname}`);
    }),
  } as unknown as WebGLRenderingContext;
}

describe('RM8 checkMaxTextureSize — WebGL 边界与上限', () => {
  it('RM8-01: checkMaxTextureSize(gl) 返回 GPU 实际 MAX_TEXTURE_SIZE', () => {
    const gl = makeFakeGl(8192);
    expect(checkMaxTextureSize(gl)).toBe(8192);
  });

  it('RM8-01b: 返回 ≥ 4096 的上限（常见桌面 GPU 至少 4096）', () => {
    const gl = makeFakeGl(16384);
    expect(checkMaxTextureSize(gl)).toBeGreaterThanOrEqual(4096);
  });

  it('checkMaxTextureSize 通过 gl.getParameter(gl.MAX_TEXTURE_SIZE) 读取', () => {
    const gl = makeFakeGl(8192);
    const before = (gl.getParameter as any).mock.calls.length;
    checkMaxTextureSize(gl);
    const after = (gl.getParameter as any).mock.calls.length;
    expect(after - before).toBeGreaterThanOrEqual(1);
  });

  it('传入 null/undefined gr → 抛出明确错误（不静默失败）', () => {
    // @ts-expect-error —— 契约要求 null WebGL context 报错而非静默返回
    expect(() => checkMaxTextureSize(null)).toThrowError(/gl|webgl|context|texture/i);
  });

  it('传入对象但 getParameter 缺失 → 抛出明确错误', () => {
    const bad: any = {};
    expect(() => checkMaxTextureSize(bad)).toThrowError();
  });
});

describe('RM8 needsDownsample — 16384 纹理降采样判断', () => {
  it('RM8-02: needsDownsample(16384, 8192) = true（白-免费版触发降级路径）', () => {
    expect(needsDownsample(16384, 8192)).toBe(true);
  });

  it('RM8-03: needsDownsample(8192, 8192) = false（恰好等于上限不需降采样）', () => {
    expect(needsDownsample(8192, 8192)).toBe(false);
  });

  it('needsDownsample(16384, 16384) = false（GPU 恰好支持 16384 无需降级）', () => {
    expect(needsDownsample(16384, 16384)).toBe(false);
  });

  it('needsDownsample(8192, 4096) = true（纹理超过上限需降采样）', () => {
    expect(needsDownsample(8192, 4096)).toBe(true);
  });

  it('needsDownsample 在 textureSize <= maxSize 时为 false（边界相等不需降采样）', () => {
    expect(needsDownsample(4096, 4096)).toBe(false);
    expect(needsDownsample(2048, 4096)).toBe(false);
  });

  it('needsDownsample 是纯布尔判断，无数值副作用', () => {
    const result = needsDownsample(16384, 8192);
    expect(typeof result).toBe('boolean');
  });
});
