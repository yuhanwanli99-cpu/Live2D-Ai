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
