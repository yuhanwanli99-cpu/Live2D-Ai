/**
 * RM2 模型加载器 —— 对齐 contract-pc-render.md §RM2 + architecture-pc-render.md §3.2.1
 *
 *   loadModel(app, modelUrl) → Live2DModel.from + app.stage.addChild → {model, internalModel}
 *   unloadModel(model)       → 从舞台摘除 + destroy（资源释放）
 *
 * 验收断言：
 *   RM2-01 loadModel(app, validUrl) 返回非 null ModelLoadResult（model instanceof Live2DModel）
 *   RM2-02 loadModel(app, invalidUrl) 抛明确错误（非静默失败，message 含 "model" / "load"）
 *   RM2-03 加载后 model.parent === app.stage
 *   RM2-06 unloadModel 后 model.destroyed === true 且 app.stage.children 不含 model
 *
 * 模型路径不硬编码——一律从参数传入（WS "set-model-and-conf" 的 model_info.url）。
 */
import { Application } from 'pixi.js';
import {
  Live2DModel,
  Cubism4InternalModel,
} from 'pixi-live2d-display-lipsyncpatch/cubism4';

/** 模型加载结果（契约 RM2 接口签名） */
export interface ModelLoadResult {
  /** pixi-live2d-display 模型实例（PIXI Container 子类，可挂舞台/做变换） */
  model: Live2DModel;
  /** Cubism4 内部模型（coreModel 参数控制入口：setParameterValueById 等） */
  internalModel: Cubism4InternalModel;
}

/** 模型加载选项（契约 RM2 数据 schema，必填值） */
const MODEL_LOAD_OPTIONS = {
  autoInteract: false, // 必须 false：不自动鼠标跟踪（相机/交互自行控制）
  autoUpdate: true,    // 必须 true：PIXI tick 自动驱动 Cubism update
} as const;

/**
 * 从 modelUrl 加载 Live2D 模型并添加到 app 舞台
 * @param app      PIXI Application（模型挂到 app.stage）
 * @param modelUrl model3.json 路径，如 "/live2d-models/白-免费版/runtime/白-免费版.model3.json"
 * @returns { model, internalModel }
 * @throws 加载失败时抛明确错误（message 含 "model"/"load"），不静默失败
 */
export async function loadModel(
  app: Application,
  modelUrl: string,
): Promise<ModelLoadResult> {
  if (typeof modelUrl !== 'string' || modelUrl.length === 0) {
    throw new Error(
      `[model-loader] loadModel: invalid model url "${String(modelUrl)}"`,
    );
  }

  let model: Live2DModel;
  try {
    // ticker: 传 app.ticker（pixi-live2d-display 默认用 PIXI.Ticker.shared——未启动则模型不自动更新，
    // 表现为“模型无法移动”。用 app.ticker 保证每帧驱动 Cubism update）
    model = await Live2DModel.from(modelUrl, {
      ...MODEL_LOAD_OPTIONS,
      ticker: app.ticker,
    });
  } catch (err) {
    const detail = err instanceof Error ? err.message : String(err);
    throw new Error(
      `[model-loader] failed to load Live2D model from "${modelUrl}": ${detail}`,
    );
  }

  // 挂到舞台（RM2-03：model.parent === app.stage）
  app.stage.addChild(model);

  // 内部模型引用（加载完成后 internalModel 已就绪）
  const internalModel = model.internalModel as Cubism4InternalModel;

  return { model, internalModel };
}

/**
 * 释放模型资源：先从父容器摘除，再 destroy。
 * PIXI Container.destroy() 不保证自动从 parent 摘除——显式 removeChild 兜底，
 * 保证卸载后 app.stage.children 不含该模型（RM2-06）。
 */
export function unloadModel(model: Live2DModel): void {
  if (!model || model.destroyed) {
    return;
  }
  if (model.parent && typeof (model.parent as { removeChild?: unknown }).removeChild === 'function') {
    model.parent.removeChild(model);
  }
  model.destroy();
}
