/**
 * RM8 纹理适配 —— 对齐 contract-pc-render.md §RM8 + architecture-pc-render.md §3.8
 *
 *   checkMaxTextureSize(gl)  → 查询 GL_MAX_TEXTURE_SIZE（WebGL context 不存在时抛明确错误）
 *   needsDownsample(size, max) → size > max 需要降采样
 *   checkTextureFit(...)      → 运行时纹理检查：给出结论 + 降级目标尺寸
 *
 * 背景：白-免费版原纹理 16384×16384，触及桌面 GPU MAX_TEXTURE_SIZE 上限
 * （部分集显仅 8192/4096）。模型资产已预降采样到 8192（model-facts-bai.md），
 * 运行时仍保留 GPU 上限检测兜底，超限时报明确错误而非白屏/崩溃。
 *
 * 验收断言：
 *   RM8-01 checkMaxTextureSize(gl) 返回 GPU 实际 MAX_TEXTURE_SIZE（≥ 4096）
 *   RM8-02 needsDownsample(16384, 8192) === true
 *   RM8-03 needsDownsample(8192, 8192) === false
 */

/** WebGL 1 / 2 context 均可检测 */
export type AnyWebGLContext = WebGLRenderingContext | WebGL2RenderingContext;

/** GL_MAX_TEXTURE_SIZE 常量值（0x0D33），context 未暴露常量时的兜底 */
const GL_MAX_TEXTURE_SIZE: GLenum = 0x0d33;

/**
 * 查询 GPU 实际 MAX_TEXTURE_SIZE
 * @param gl WebGL context（WebGL1 或 WebGL2）
 * @returns GL_MAX_TEXTURE_SIZE（px，2 的幂，如 4096/8192/16384）
 * @throws 明确错误——context 不存在 / 无法查询 / 查询值非法时
 */
export function checkMaxTextureSize(gl: AnyWebGLContext): number {
  if (!gl || typeof gl.getParameter !== 'function') {
    throw new Error(
      '[texture-adapter] checkMaxTextureSize: WebGL context is not available; ' +
        'cannot query GL_MAX_TEXTURE_SIZE',
    );
  }

  const pname: GLenum = gl.MAX_TEXTURE_SIZE ?? GL_MAX_TEXTURE_SIZE;
  const raw = gl.getParameter(pname);
  const size = typeof raw === 'number' ? raw : Number(raw);

  if (!Number.isFinite(size) || size <= 0) {
    throw new Error(
      `[texture-adapter] checkMaxTextureSize: invalid GL_MAX_TEXTURE_SIZE value "${String(raw)}"`,
    );
  }

  return size;
}

/**
 * 判断纹理是否需要降采样（size 超过 GPU 上限）
 * @param textureSize 纹理尺寸（px，如 16384）
 * @param maxSize     GPU 上限（GL_MAX_TEXTURE_SIZE，如 8192）
 */
export function needsDownsample(textureSize: number, maxSize: number): boolean {
  return textureSize > maxSize;
}

/** 兼容别名（语义与 needsDownsample 完全相同） */
export const shouldDownsample: typeof needsDownsample = needsDownsample;

/** 运行时纹理检查结果 */
export interface TextureFitResult {
  /** 纹理实际尺寸（px） */
  textureSize: number;
  /** GPU 上限（GL_MAX_TEXTURE_SIZE） */
  maxSize: number;
  /** true = 纹理超出 GPU 上限，需要降级 */
  needsDownsample: boolean;
  /** 建议降级到的尺寸（2 的幂且 ≤ maxSize；无需降级时 = textureSize） */
  targetSize: number;
}

/**
 * 运行时纹理检查：对照 GPU MAX_TEXTURE_SIZE，给出明确结论与降级建议。
 * 超限时由调用方决定：降采样重载 / 提示用户，而非静默白屏。
 *
 * @param textureSize 纹理尺寸（px）
 * @param maxSize     GPU 上限（GL_MAX_TEXTURE_SIZE）
 */
export function checkTextureFit(
  textureSize: number,
  maxSize: number,
): TextureFitResult {
  const over = needsDownsample(textureSize, maxSize);
  return {
    textureSize,
    maxSize,
    needsDownsample: over,
    targetSize: over ? largestPow2Leq(maxSize) : textureSize,
  };
}

/** 计算 ≤ n 的最大 2 的幂（降级目标尺寸用） */
function largestPow2Leq(n: number): number {
  if (!Number.isFinite(n) || n <= 0) {
    return 1;
  }
  let p = 1;
  while (p * 2 <= n) {
    p *= 2;
  }
  return p;
}
