/**
 * renderer 入口（RM1 集成接线——PC polish Batch 4）——对齐 contract-pc-render.md §RM1 + architecture-pc-render.md §3
 *
 *   export function initApp(containerId: string): Promise<PixiApp>
 *   // 返回：{ app: PIXI.Application, modelLoader: ModelLoader, wsBridge: WsBridge, arbiter: ParamArbiter }
 *
 * 集成接线（替换骨架占位——页面加载模型 + 连 WS）：
 *   1. PIXI.Application 挂到容器（透明背景 + 自动 resize + 高 DPI 封顶 2x）
 *   2. model-loader 加载默认模型（/live2d-models/bai/runtime/bai.model3.json，后端 /live2d-models 静态托管）
 *   3. ParamArbiter 仲裁器（RM3）+ EmotionConsumer（RM4）+ LipSync（RM5）
 *   4. 待机四件套（RM6）：眨眼 / 呼吸 / 微表情 / 小动作，挂 app.ticker
 *   5. WsBridge（RM7）连 WS "/client-ws"（后端 routes.py 实际路由），
 *      分发 set-model-and-conf（模型热切换）/ audio（口型+表情+字幕）/ emotion
 *   6. 相机半身构图（RM2 camera.ts）+ 纹理适配检查（RM8 texture-adapter.ts）
 *   7. 窗口 resize → 重应用相机；beforeunload → 主动断开 WS
 *
 * 容错设计：
 *   - live2d 依赖模块（model-loader/camera/idle-motion）动态导入 + 失败降级：
 *     node/vitest 环境（无 window）导入失败 → 空转管线继续，initApp 仍正常返回；
 *     Web 环境加载成功后全部能力就位。WS 桥独立于模型——断线重连由 WsBridge 内置
 *     （指数退避，封顶 30s）。
 *   - 默认模型加载失败 → 记日志继续（仲裁器空转），待后端 set-model-and-conf
 *     下发后自动重试加载。
 */
import { Application } from 'pixi.js';
import { WsBridge } from './ws-bridge';
import { initChatPanel } from './chat-ui';
import { ParamArbiter } from './param-arbiter';
import { EmotionConsumer } from './emotion-consumer';
import { LipSync } from './lip-sync';
import { BlinkStateMachine } from './idle/idle-blink';
import { BreathController } from './idle/idle-breath';
import { MicroExpressionController } from './idle/idle-micro-expr';
import {
  checkMaxTextureSize,
  checkTextureFit,
  type AnyWebGLContext,
} from './texture-adapter';

// ---- 类型导入（编译期擦除——不在运行时触碰 live2d 依赖链，node/vitest 可安全 import 本模块） ----
import type { Live2DModel, Cubism4InternalModel } from 'pixi-live2d-display-lipsyncpatch/cubism4';
import type { ModelLoadResult } from './model-loader';
import type { IdleMotionController } from './idle/idle-motion';
import type { ModelInfo } from './types';

/** 默认模型 URL（后端 /live2d-models 静态托管；set-model-and-conf 未到/无 model_dict 时的兜底） */
const DEFAULT_MODEL_URL = '/live2d-models/bai/runtime/bai.model3.json';

/** WS 端点路径：后端 routes.py 实际注册 /client-ws（契约草案中的 /ws 已废弃） */
const WS_PATH = '/client-ws';

/** 帧间隔换算：PIXI Ticker delta 以 60fps 帧为单位 → ms（架构 §3.6.5） */
const FRAME_MS = 1000 / 60;

/** 画布尺寸兜底（app.screen / 容器尺寸不可用时的构图回退值） */
const FALLBACK_WIDTH = 800;
const FALLBACK_HEIGHT = 600;

/** 解析 WS 地址：与页面同源（生产：后端托管页面 + WS 同一 origin） */
function resolveWsUrl(): string {
  const proto =
    typeof location !== 'undefined' && location.protocol === 'https:'
      ? 'wss'
      : 'ws';
  const host = typeof location !== 'undefined' ? location.host : 'localhost:12393';
  return `${proto}://${host}${WS_PATH}`;
}

/** RM2 模型加载器接口（对齐契约返回类型；model-loader.ts 导出函数的聚合） */
export interface ModelLoader {
  loadModel: (app: Application, modelUrl: string) => Promise<ModelLoadResult>;
  unloadModel: (model: Live2DModel) => void;
}

/** initApp 返回结构（契约 RM1：槽位全部就位，无 null 占位） */
export interface PixiApp {
  app: Application;
  /** RM2: 模型加载器 */
  modelLoader: ModelLoader;
  /** RM7: WS 桥接（已 connect，内置断线重连） */
  wsBridge: WsBridge;
  /** RM3: 参数仲裁器（模型未就绪时为空转实例，update 不写 coreModel） */
  arbiter: ParamArbiter;
}

/** 渲染管线（仲裁器 + 消费者 + 待机控制器）；模型切换时整体重建 */
interface RenderPipeline {
  arbiter: ParamArbiter;
  emotionConsumer: EmotionConsumer;
  lipSync: LipSync;
  blink: BlinkStateMachine;
  breath: BreathController;
  microExpr: MicroExpressionController;
  idleMotion: IdleMotionController | null;
}

/**
 * 初始化 PIXI 应用并接线全部渲染模块（模型加载 / 仲裁器 / 待机 / WS 桥 / 相机 / 纹理）。
 * @param containerId 容器元素 id（index.html 中为 "canvas"）
 */
export async function initApp(containerId: string): Promise<PixiApp> {
  const container = document.getElementById(containerId);
  if (!container) {
    throw new Error(
      `[renderer] initApp: container "#${containerId}" not found in DOM`,
    );
  }

  // 透明背景：配合桌面壳（Electron/Tauri 透明窗口）实现悬浮桌宠；
  // Web 模式下由 index.html 的 body 背景色控制观感。
  const app = new Application({
    backgroundAlpha: 0,
    resizeTo: container,
    antialias: true,
    autoDensity: true,
    // 高 DPI 屏保清晰，封顶 2 倍防止极端设备性能损耗
    resolution: Math.min(window.devicePixelRatio || 1, 2),
  });

  // ICanvas → HTMLCanvasElement：Web 环境实际渲染目标就是 canvas 元素
  container.appendChild(app.view as HTMLCanvasElement);

  // ---- live2d 依赖模块：动态导入（node/vitest 无 window → 失败降级，不阻塞启动） ----
  const live2dModules = await loadLive2dModules();

  // ---- 模块接线 ----
  const modelLoader: ModelLoader = live2dModules
    ? {
        loadModel: live2dModules.modelLoader.loadModel,
        unloadModel: live2dModules.modelLoader.unloadModel,
      }
    : {
        loadModel: async () => {
          throw new Error(
            '[renderer] model-loader 不可用（live2d 依赖未加载）',
          );
        },
        unloadModel: () => {
          /* 无可卸载模型 */
        },
      };
  const wsBridge = new WsBridge(resolveWsUrl());

  /** 画布尺寸（app.screen 优先；mock/未就绪时回退容器/常量） */
  const canvasWidth = (): number =>
    app.screen?.width ?? container.clientWidth ?? FALLBACK_WIDTH;
  const canvasHeight = (): number =>
    app.screen?.height ?? container.clientHeight ?? FALLBACK_HEIGHT;

  /** 构建渲染管线：仲裁器 → 消费者 → 待机控制器 → WS 分发接线 */
  const createPipeline = (
    model: Live2DModel | null,
    internalModel: Cubism4InternalModel | null,
  ): RenderPipeline => {
    const arbiter = new ParamArbiter(internalModel);
    const pipeline: RenderPipeline = {
      arbiter,
      emotionConsumer: new EmotionConsumer(arbiter),
      lipSync: new LipSync(arbiter),
      blink: new BlinkStateMachine(arbiter),
      breath: new BreathController(arbiter),
      microExpr: new MicroExpressionController(arbiter),
      idleMotion:
        model && live2dModules
          ? new live2dModules.idleMotion.IdleMotionController(model)
          : null,
    };
    wsBridge.setEmotionConsumer(pipeline.emotionConsumer);
    wsBridge.setLipSync(pipeline.lipSync);
    return pipeline;
  };

  // 渲染管线：先以空转管线起步（arbiter 无 coreModel，update 不写），保证返回结构完整
  let pipeline: RenderPipeline = createPipeline(null, null);

  // 当前模型状态（模型热切换用）
  let currentModel: Live2DModel | null = null;
  let currentModelInfo: ModelInfo | null = null;

  /** 应用相机（contract-C-M2：改走 applyCameraMode 分发；默认 CAMERA_MODE='fit' 保持现状；_modelInfo 保留签名（方案 B 预留位）） */
  const applyModelCamera = (
    model: Live2DModel,
    _modelInfo: ModelInfo | undefined,
  ): void => {
    live2dModules?.camera.applyCameraMode(
      model,
      live2dModules.camera.CAMERA_MODE,
      canvasWidth(),
      canvasHeight(),
    );
  };

  /** 纹理适配检查（RM8）：模型主纹理超出 GPU MAX_TEXTURE_SIZE 时明确告警 */
  const applyTextureGuard = (model: Live2DModel): void => {
    try {
      const gl = (app.renderer as unknown as { gl?: AnyWebGLContext | null }).gl;
      if (!gl) return; // 无 GL 上下文（mock/软件渲染）→ 跳过
      const maxSize = checkMaxTextureSize(gl);
      const firstTexture = model.textures?.[0];
      const textureSize = firstTexture
        ? Math.max(firstTexture.width, firstTexture.height)
        : 0;
      if (textureSize <= 0) return; // 纹理尺寸未知 → 跳过
      const fit = checkTextureFit(textureSize, maxSize);
      if (fit.needsDownsample) {
        console.warn(
          `[renderer] 模型纹理 ${fit.textureSize}px 超出 GPU MAX_TEXTURE_SIZE=${fit.maxSize}px，` +
            `建议降采样到 ${fit.targetSize}px（当前 GPU 可能无法渲染该纹理）`,
        );
      }
    } catch (err) {
      console.warn('[renderer] 纹理适配检查跳过:', err);
    }
  };

  /** 加载并挂载模型：新模型就绪后释放旧模型 → 重建管线 → 相机 + 纹理检查 */
  const loadAndAttach = async (modelInfo: ModelInfo): Promise<void> => {
    if (!live2dModules) {
      throw new Error('[renderer] live2d 依赖未加载，无法加载模型');
    }
    const loaded: ModelLoadResult = await live2dModules.modelLoader.loadModel(
      app,
      modelInfo.url,
    );
    // 新模型就绪后，释放旧模型资源（RM2-06）
    if (currentModel) {
      live2dModules.modelLoader.unloadModel(currentModel);
      currentModel = null;
    }
    currentModel = loaded.model;
    currentModelInfo = modelInfo;
    applyModelCamera(loaded.model, modelInfo);
    applyTextureGuard(loaded.model);
    pipeline = createPipeline(loaded.model, loaded.internalModel);

    // ---- S3 渲染日志探针（contract-M-PC §2.3；只读：不改渲染逻辑/相机应用顺序，仅 console 输出） ----
    // 1) 模型原始画布尺寸（bai: 4000×6000）
    console.log(
      `[probe] originalWidth=${loaded.internalModel.originalWidth} originalHeight=${loaded.internalModel.originalHeight}`,
    );
    // 2) 角色实际包围盒：遍历全部 drawable 的 getDrawableVertexPositions 求 min/max
    //    顶点为模型单位（bai 画布 1.0×1.5）；乘 pixelsPerUnit 归一为画布像素单位（对齐
    //    architecture E5：x≈[-165,167]、y≈[-994,16]）。若原始值已是画布像素量级（>2）则不再缩放。
    const probeCoreModel = loaded.internalModel.coreModel;
    const probeCount = probeCoreModel.getDrawableCount();
    let probeMinX = Infinity;
    let probeMaxX = -Infinity;
    let probeMinY = Infinity;
    let probeMaxY = -Infinity;
    for (let i = 0; i < probeCount; i++) {
      const verts = probeCoreModel.getDrawableVertexPositions(i);
      if (!verts) continue;
      for (let j = 0; j < verts.length; j += 2) {
        const vx = verts[j];
        const vy = verts[j + 1];
        if (vx < probeMinX) probeMinX = vx;
        if (vx > probeMaxX) probeMaxX = vx;
        if (vy < probeMinY) probeMinY = vy;
        if (vy > probeMaxY) probeMaxY = vy;
      }
    }
    const probeUnit =
      Math.max(probeMaxX - probeMinX, probeMaxY - probeMinY) < 2
        ? loaded.internalModel.pixelsPerUnit
        : 1;
    console.log(
      `[probe] bbox x=[${probeMinX * probeUnit},${probeMaxX * probeUnit}] y=[${probeMinY * probeUnit},${probeMaxY * probeUnit}]`,
    );
    // 3) 应用相机后的模型变换/尺寸（scale=1.8，width/height=画布×Layout 缩放）
    console.log(
      `[probe] model scale=${loaded.model.scale?.x ?? 'n/a'} width=${loaded.internalModel.width} height=${loaded.internalModel.height}`,
    );
  };

  /** WS "set-model-and-conf" 回调：同模型仅更新相机；不同模型热切换（失败保留旧模型不崩溃） */
  const handleModelChange = async (modelInfo: ModelInfo): Promise<void> => {
    try {
      if (
        currentModel &&
        currentModelInfo &&
        modelInfo.url === currentModelInfo.url
      ) {
        // 同模型：仅重应用相机参数（后端可能更新 kScale / initialYshift / kXOffset）
        applyModelCamera(currentModel, modelInfo);
        return;
      }
      await loadAndAttach(modelInfo);
    } catch (err) {
      // 切换失败：保留旧模型/空舞台，不崩溃（待下次 set-model-and-conf 重试）
      console.error(`[renderer] 模型 "${modelInfo.url}" 加载失败:`, err);
    }
  };

  // ---- 默认模型兜底加载（后端连接后还会下发 set-model-and-conf → handleModelChange 幂等） ----
  try {
    await loadAndAttach({ url: DEFAULT_MODEL_URL });
  } catch (err) {
    console.error(
      `[renderer] 默认模型加载失败（空舞台继续，等待 set-model-and-conf 重试）: ${DEFAULT_MODEL_URL}`,
      err,
    );
  }

  // ---- RM6 待机循环 + RM3 仲裁器写入（每帧：眨眼/呼吸/微表情/小动作 → 仲裁器合并写 coreModel） ----
  app.ticker?.add((dt) => {
    const deltaMs = dt * FRAME_MS;
    pipeline.blink.update(deltaMs);
    pipeline.breath.update(deltaMs);
    pipeline.microExpr.update(deltaMs);
    pipeline.idleMotion?.update(deltaMs);
    pipeline.arbiter.update(deltaMs);
  });

  // ---- RM7 WS 桥接：分发 set-model-and-conf（模型切换）/ audio（口型+表情+字幕）/ emotion ----
  wsBridge.onModelChange = (modelInfo) => {
    void handleModelChange(modelInfo);
  };

  // ---- 聊天面板（spoq-fix：双端对话框；直播/微信风格右侧面板） ----
  const chatHost = document.getElementById('chat');
  if (chatHost) {
    const chatPanel = initChatPanel(chatHost, {
      onSend: (text) => wsBridge.sendText(text),
    });
    chatPanel.addMessage({ role: 'system', content: '连接服务器中…（WS 握手）' });
    wsBridge.onSubtitle = (text) => {
      if (text?.text) chatPanel.addMessage({ role: 'assistant', content: text.text });
    };
    wsBridge.onConnectionChange = (connected) => {
      chatPanel.setConnected(connected);
      if (connected) {
        chatPanel.addMessage({ role: 'system', content: '已连接，可以开始聊天' });
      } else {
        chatPanel.addMessage({ role: 'system', content: '连接断开，重连中…' });
      }
    };
  } else {
    // 无聊天容器（测试环境）→ 字幕仍走 console
    wsBridge.onSubtitle = (text) => {
      console.debug(
        '[renderer] subtitle:',
        text.name ? `[${text.name}] ` : '',
        text.text,
      );
    };
  }
  wsBridge.connect();

  // ---- 窗口处理：resize 重应用相机（构图跟随）；beforeunload 主动断开 WS ----
  if (typeof window !== 'undefined') {
    window.addEventListener?.('resize', () => {
      if (currentModel) {
        applyModelCamera(currentModel, currentModelInfo ?? undefined);
      }
    });
    window.addEventListener?.('beforeunload', () => {
      wsBridge.disconnect();
    });
  }

  return { app, modelLoader, wsBridge, arbiter: pipeline.arbiter };
}

/** live2d 依赖模块（动态导入缓存） */
interface Live2dModules {
  modelLoader: typeof import('./model-loader');
  camera: typeof import('./camera');
  idleMotion: typeof import('./idle/idle-motion');
}

let live2dModulesCache: Live2dModules | null = null;

/**
 * 动态加载 live2d 依赖模块（model-loader / camera / idle-motion）。
 * 这三个模块值导入 pixi-live2d-display（其模块级访问 window）——node/vitest 环境
 * 导入必然失败；这里 try/catch 兜底，Web 环境加载成功后缓存复用。
 */
async function loadLive2dModules(): Promise<Live2dModules | null> {
  if (live2dModulesCache) return live2dModulesCache;
  try {
    const [modelLoader, camera, idleMotion] = await Promise.allSettled([
      import('./model-loader'),
      import('./camera'),
      import('./idle/idle-motion'),
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
        typeof import('./model-loader')
      >).value,
      camera: (camera as PromiseFulfilledResult<
        typeof import('./camera')
      >).value,
      idleMotion: (idleMotion as PromiseFulfilledResult<
        typeof import('./idle/idle-motion')
      >).value,
    };
    return live2dModulesCache;
  } catch (err) {
    console.error('[renderer] live2d 依赖模块加载失败（渲染不可用）:', err);
    return null;
  }
}

/**
 * 浏览器环境自动启动（模块入口）。
 * 守卫条件保证 main.ts 在 node/vitest 环境可安全 import（不触碰 DOM）。
 */
if (typeof document !== 'undefined' && typeof window !== 'undefined') {
  const canvasEl = document.getElementById('canvas');
  if (canvasEl) {
    initApp('canvas').catch((err) => {
      console.error('[renderer] initApp failed:', err);
    });
  }
}
