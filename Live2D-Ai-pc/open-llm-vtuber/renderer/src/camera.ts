/**
 * RM2 相机控制 —— 对齐 contract-pc-render.md §RM2 + architecture-pc-render.md §3.2.2
 *
 *   半身构图：推近（scale）+ 镜头下移（yOffset）
 *   支持 WS model_info 的 kScale / initialYshift / kXOffset 覆盖默认值（applyCameraFromModelInfo）
 *
 * 验收断言：
 *   RM2-04 applyCamera(model, {scale:1.8, xOffset:0, yOffset:100}, 800, 600) → model.scale.x === 1.8
 *   RM2-05 应用后 model.x / model.y 为有限值（非 NaN/Infinity）
 */
import { Live2DModel } from 'pixi-live2d-display-lipsyncpatch/cubism4';
import type { ModelInfo } from './types';

/** 相机配置（契约 RM2 接口签名） */
export interface CameraConfig {
  /** 缩放倍率（推荐默认 1.8） */
  scale: number;
  /** 水平偏移（px） */
  xOffset: number;
  /** 垂直偏移（px），正 = 下移，推荐 100 */
  yOffset: number;
}

/** 默认相机参数（白-免费版半身构图参考值，需实机微调） */
export const DEFAULT_CAMERA: CameraConfig = {
  scale: 1.8,
  xOffset: 0,
  yOffset: 100,
};

/** 相机模式（contract-C-M2 §2.1；C-M6 override 之外的默认路径） */
export type CameraMode = 'fit' | 'half-body';

/** 唯一默认开关（D2 裁定：默认 fit，禁止默认半身） */
export const CAMERA_MODE: CameraMode = 'fit';

/** fit 安全边距（现为 applyFitCamera 形参默认值，抽为导出常量供 FIT-05 显式断言） */
export const FIT_PADDING = 0.05;

/** 半身构图缩放倍数（fit 相对式参考初值；最终数值由用户目视 A/B 裁定后回填，机器不定值） */
export const HALF_BODY_BOOST = 1.8;

/** 半身构图垂直偏移（px，>0 下移；与旧 DEFAULT_CAMERA 同量级参考初值，同上待用户裁定回填） */
export const HALF_BODY_Y_OFFSET = 100;

/**
 * 合并相机配置：model_info 的可选字段（kScale/initialYshift/kXOffset）
 * 或部分 CameraConfig 覆盖默认 {scale:1.8, xOffset:0, yOffset:100}。
 * 缺失项回退默认（半身构图参考值）。
 */
export function resolveCameraConfig(
  partial?: Partial<CameraConfig> | Pick<ModelInfo, 'kScale' | 'initialYshift' | 'kXOffset'>,
): CameraConfig {
  const p = (partial ?? {}) as Record<string, number | undefined>;
  return {
    scale: p.scale ?? p.kScale ?? DEFAULT_CAMERA.scale,
    xOffset: p.xOffset ?? p.kXOffset ?? DEFAULT_CAMERA.xOffset,
    yOffset: p.yOffset ?? p.initialYshift ?? DEFAULT_CAMERA.yOffset,
  };
}

/**
 * 应用相机参数（PixiJS 变换）
 *
 * 半身构图实现：scale 放大 + 模型中心定位到画布中心，再叠加偏移。
 * anchor 居中（(0.5, 0.5)）使 scale/position 围绕模型中心生效。
 *
 * @param model        Live2D 模型
 * @param config       相机配置（scale/xOffset/yOffset）
 * @param canvasWidth  画布宽（px）
 * @param canvasHeight 画布高（px）
 */
export function applyCamera(
  model: Live2DModel,
  config: CameraConfig,
  canvasWidth: number,
  canvasHeight: number,
): void {
  const { scale, xOffset, yOffset } = config;

  // scale.set 同时设置 scale.x / scale.y（PixiJS 标准 API；mock 下显式赋值兜底）
  model.scale.set?.(scale);
  model.scale.x = scale;
  model.scale.y = scale;
  // 以模型中心为锚点
  model.anchor.set(0.5, 0.5);
  // 定位到画布中心 + 偏移
  model.x = canvasWidth / 2 + xOffset;
  model.y = canvasHeight / 2 + yOffset;
}

/**
 * 从 WS "set-model-and-conf" 的 model_info 应用相机参数。
 * kScale / initialYshift / kXOffset 存在时覆盖默认值，缺失时回退默认（半身构图参考值）。
 *
 * @param model        Live2D 模型
 * @param modelInfo    model_info 字段（kScale=scale / initialYshift=yOffset / kXOffset=xOffset）
 * @param canvasWidth  画布宽（px）
 * @param canvasHeight 画布高（px）
 */
export function applyCameraFromModelInfo(
  model: Live2DModel,
  modelInfo: Pick<ModelInfo, 'kScale' | 'initialYshift' | 'kXOffset'> | undefined,
  canvasWidth: number,
  canvasHeight: number,
): void {
  applyCamera(model, resolveCameraConfig(modelInfo), canvasWidth, canvasHeight);
}

/**
 * fit scale 纯函数（applyFitCamera 与 half-body 共用，防公式漂移）：
 *  尺寸无效（0/NaN/非有限值）→ NaN；否则 min(canvasW/model.width, canvasH/model.height) * (1-padding)
 */
export function computeFitScale(
  canvasWidth: number,
  canvasHeight: number,
  model: Live2DModel,
  padding: number = FIT_PADDING,
): number {
  const baseW = model.width;
  const baseH = model.height;
  // 尺寸无效（未就绪/零尺寸）→ NaN（调用方据此判断不动模型）
  if (!baseW || !baseH || !Number.isFinite(baseW) || !Number.isFinite(baseH)) {
    return NaN;
  }
  return (
    Math.min(canvasWidth / baseW, canvasHeight / baseH) * (1 - padding)
  );
}

/**
 * 自适应相机（fit 模式，spoq-fix 返工）：模型完整可见且居中。
 *
 * 背景：bai.moc3 画布 4000×6000 而角色 bbox 仅占 8.3%×16.8% 偏下；固定 scale=1.8 的
 * 半身构图对这类“大画布小角色”模型只显示局部（用户实锤“大腿”）。
 * 本函数按模型基础尺寸（Layout 归一化后的 model.width/height）对画布做 contain-fit，
 * 保证任意模型首次加载即完整可见。
 *
 * @param model        Live2D 模型
 * @param canvasWidth  画布宽（px）
 * @param canvasHeight 画布高（px）
 * @param padding      安全边距比例（0~1，默认 5% 防贴边裁切）
 */
export function applyFitCamera(
  model: Live2DModel,
  canvasWidth: number,
  canvasHeight: number,
  padding = FIT_PADDING,
): void {
  const fitScale = computeFitScale(canvasWidth, canvasHeight, model, padding);
  // 尺寸无效（未就绪/零尺寸）→ 不动，保持调用方既有状态
  if (!Number.isFinite(fitScale)) {
    return;
  }
  model.scale.set?.(fitScale);
  model.scale.x = fitScale;
  model.scale.y = fitScale;
  model.anchor.set(0.5, 0.5);
  model.x = canvasWidth / 2;
  model.y = canvasHeight / 2;
}

/**
 * 模式分发（contract-C-M2 §2.1 唯一入口）：
 *  mode='fit'       → 现有 applyFitCamera 语义（contain-fit + padding + 居中）
 *  mode='half-body' → scale = computeFitScale(...) × HALF_BODY_BOOST；
 *                      x = canvasWidth/2 + (config?.xOffset ?? 0)；
 *                      y = canvasHeight/2 + (config?.yOffset ?? HALF_BODY_Y_OFFSET)；
 *                      anchor(0.5, 0.5)（对齐 applyCamera 语义）
 */
export function applyCameraMode(
  model: Live2DModel,
  mode: CameraMode,
  canvasWidth: number,
  canvasHeight: number,
  config?: { xOffset?: number; yOffset?: number },
): void {
  if (mode === 'fit') {
    applyFitCamera(model, canvasWidth, canvasHeight, FIT_PADDING);
    return;
  }
  // half-body：fit 相对式（公式 1）——以 fit scale 为基准 × boost，中心 + 偏移
  const fitScale = computeFitScale(canvasWidth, canvasHeight, model, FIT_PADDING);
  // 尺寸无效 → 不动，保持调用方既有状态（与 applyFitCamera 一致）
  if (!Number.isFinite(fitScale)) {
    return;
  }
  const scale = fitScale * HALF_BODY_BOOST;
  const x = canvasWidth / 2 + (config?.xOffset ?? 0);
  const y = canvasHeight / 2 + (config?.yOffset ?? HALF_BODY_Y_OFFSET);
  model.scale.set?.(scale);
  model.scale.x = scale;
  model.scale.y = scale;
  model.anchor.set(0.5, 0.5);
  model.x = x;
  model.y = y;
}
