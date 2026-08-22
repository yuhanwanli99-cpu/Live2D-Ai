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
import { initSettingsPanel, type SettingsPanel } from './settings-ui';
import { applyBackgroundLayer, fetchBackgroundState } from './background';
import { initHistoryDrawer, mapHistoryMessages } from './history';
import { initOnboarding } from './onboarding';
import { createVoiceCapture } from './voice-input';
import type { ChatPanel } from './chat-ui';
import { clearToast, toast } from './toast';
import { icon } from './icons';
import {
  createInteractionStore,
  wheelScale,
  shouldApplyOverride,
} from './interaction';
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
import { BodyActionPlayer } from './choreography';
import {
  createSoullinkPerformanceAdapter,
  resolvePerformanceEngine,
  loadSoullinkProfile,
  type SoullinkPerformanceAdapter,
} from './soullink-adapter';
import { LiveEventController, type LiveEventPayload } from './live-event';
import { createCameraModeStore } from './camera-state';

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
/** 表演引擎开关（PLAN-V3 P1 双跑）：localStorage performanceEngine，默认 legacy */
const PERFORMANCE_ENGINE = resolvePerformanceEngine();

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
  bodyMotion: BodyActionPlayer;
  liveEvent: LiveEventController;
  /** PLAN-V3 P1：soullink 表演引擎适配器（仅 performanceEngine=soullink 时非空） */
  soullink: SoullinkPerformanceAdapter | null;
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
    soullink: SoullinkPerformanceAdapter | null = null,
  ): RenderPipeline => {
    const arbiter = new ParamArbiter(internalModel);
    const emotionConsumer = new EmotionConsumer(arbiter);
    const bodyMotion = new BodyActionPlayer(arbiter, { now: () => performance.now() });
    const liveEvent = new LiveEventController(emotionConsumer, bodyMotion);
    const pipeline: RenderPipeline = {
      arbiter,
      emotionConsumer,
      lipSync: new LipSync(arbiter),
      blink: new BlinkStateMachine(arbiter),
      breath: new BreathController(arbiter),
      microExpr: new MicroExpressionController(arbiter),
      idleMotion:
        model && live2dModules
          ? new live2dModules.idleMotion.IdleMotionController(model)
          : null,
      bodyMotion,
      liveEvent,
      soullink,
    };
    wsBridge.setEmotionConsumer(pipeline.emotionConsumer);
    wsBridge.setBodyMotion(pipeline.bodyMotion);
    wsBridge.setLipSync(pipeline.lipSync);

    // 核心缺陷修复（PLAN-V3 §1.5/§8）：模型每帧会重置参数，此前 arbiter 写入发生在
    // 模型自身更新之前而被整帧覆盖——表现就是"只有嘴动"。挂在 afterMotionUpdate
    // （动作已应用、形变未发生）重写全部参数，眨眼/呼吸/表情/动作才能真正可见。
    const imEvents = internalModel as unknown as {
      on?: (event: string, cb: () => void) => void;
    };
    imEvents.on?.('afterMotionUpdate', () => {
      pipeline.arbiter.update(lastFrameDeltaMs);
      if (PERFORMANCE_ENGINE === 'soullink') pipeline.soullink?.frame(lastFrameDeltaMs / 1000);
    });

    return pipeline;
  };

  // 渲染管线：先以空转管线起步（arbiter 无 coreModel，update 不写），保证返回结构完整
  let pipeline: RenderPipeline = createPipeline(null, null);

  // 当前模型状态（模型热切换用）
  let currentModel: Live2DModel | null = null;
  let currentModelInfo: ModelInfo | null = null;

  // ---- C-M6 用户交互持久化（localStorage；浏览器缺省，node 环境降级 no-op） ----
  const interactionStore = createInteractionStore(createDefaultStorage());
  // PC 半身镜头本地偏好：默认 half-body，用户可在设置页切换；非法值回落默认
  const cameraModeStore = createCameraModeStore(createDefaultStorage());

  /** 模型 key（C-M6 §2.2：model_info.url 的 encodeURIComponent，url 不同 → override 独立） */
  const modelKeyFor = (modelInfo: ModelInfo): string =>
    encodeURIComponent(modelInfo.url);

  /** 当前模型 key（无模型 → null） */
  const currentModelKey = (): string | null =>
    currentModelInfo ? modelKeyFor(currentModelInfo) : null;

  /**
   * 统一位姿入口（contract-C-M6 §2.2 优先级：用户 override > 相机模式）。
   * load / window resize / WS 同模型更新三处共用：
   *   有 override → 应用绝对值（scale/anchor/x/y），resize 不重新 fit（用户锁定优先）；
   *   无 override → applyCameraMode（镜头模式来自 camera-state，默认 half-body）。
   */
  const applyModelPose = (model: Live2DModel, modelKey: string): void => {
    const override = interactionStore.load(modelKey);
    if (shouldApplyOverride(override) && override) {
      model.scale.set?.(override.scale);
      model.scale.x = override.scale;
      model.scale.y = override.scale;
      model.anchor.set(0.5, 0.5);
      model.x = override.x;
      model.y = override.y;
      return;
    }
    // 无 override → 相机模式路径（默认 half-body；本地设置可切回 fit）
    live2dModules?.camera.applyCameraMode(
      model,
      cameraModeStore.load(),
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
    applyModelPose(loaded.model, modelKeyFor(modelInfo));
    applyTextureGuard(loaded.model);
    let soullink: SoullinkPerformanceAdapter | null = null;
    if (PERFORMANCE_ENGINE === 'soullink') {
      // profile 随前端包发布：/soullink-profiles/<皮套目录名>.profile.json
      const skin = /\/live2d-models\/([^/]+)\//.exec(modelInfo.url || '')?.[1] ?? 'bai';
      const profile = await loadSoullinkProfile('/soullink-profiles/' + skin + '.profile.json');
      if (!profile) {
        console.error('[perf-engine] profile 缺失，本次会话回退 legacy 行为');
      } else {
        try {
          soullink = await createSoullinkPerformanceAdapter({
            profile,
            coreModel: () => loaded.internalModel?.coreModel ?? null,
            now: () => performance.now() / 1000,
          });
        } catch (err) {
          console.error('[perf-engine] 初始化失败，回退 legacy（永不静默崩溃）:', err);
        }
      }
    }
    pipeline = createPipeline(loaded.model, loaded.internalModel, soullink);

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
        // 同模型：仅重应用位姿（override 存在 → 保持用户覆盖；无 → 相机模式重算）
        applyModelPose(currentModel, modelKeyFor(modelInfo));
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
  let lastFrameDeltaMs = FRAME_MS;
  app.ticker?.add((dt) => {
    const deltaMs = dt * FRAME_MS;
    lastFrameDeltaMs = deltaMs;
    pipeline.blink.update(deltaMs);
    pipeline.breath.update(deltaMs);
    pipeline.microExpr.update(deltaMs);
    pipeline.idleMotion?.update(deltaMs);
    pipeline.bodyMotion.update(deltaMs);
    pipeline.arbiter.update(deltaMs);
    if (PERFORMANCE_ENGINE === 'soullink' && pipeline.soullink) {
      pipeline.soullink.frame(deltaMs / 1000); // 保底：事件缺失时也推进引擎
    }
  });

  // ---- RM7 WS 桥接：分发 set-model-and-conf（模型切换）/ audio（口型+表情+字幕）/ emotion / live-event ----
  wsBridge.onModelChange = (modelInfo) => {
    void handleModelChange(modelInfo);
  };
  wsBridge.onLiveEvent = (payload: LiveEventPayload) => {
    pipeline.liveEvent.handle(payload);
    if (payload.text) {
      toast(
        payload.user ? `[直播] ${payload.user}：${payload.text}` : `[直播] ${payload.text}`,
        'info',
        { key: 'live-event' },
      );
    }
  };

  // ---- C-M6 画布交互：拖拽 + 滚轮缩放（→ localStorage 持久化） ----
  // 面板为独立 fixed 层，画布事件不会干扰聊天输入框/按钮（click 不消费）
  const view = app.view as HTMLCanvasElement | undefined;
  if (view?.addEventListener) {
    const DRAG_THRESHOLD_PX = 10; // N.E.K.O. R08 CLICK_THRESHOLD_DISTANCE
    const WHEEL_SAVE_DEBOUNCE_MS = 300; // N.E.K.O. _debouncedSnapCheck 语义
    let pointerDown = false;
    let dragActive = false;
    let downX = 0;
    let downY = 0;
    let modelStartX = 0;
    let modelStartY = 0;
    let wheelSaveTimer: ReturnType<typeof setTimeout> | null = null;

    /** 保存当前位姿为 override（拖拽结束 / 滚轮防抖后） */
    const saveOverride = (): void => {
      const key = currentModelKey();
      if (!key || !currentModel) return;
      interactionStore.save(key, {
        scale: currentModel.scale?.x ?? 1,
        x: currentModel.x,
        y: currentModel.y,
        version: 1,
      });
    };

    view.addEventListener('pointerdown', (e) => {
      if (!currentModel) return;
      pointerDown = true;
      dragActive = false;
      downX = e.clientX;
      downY = e.clientY;
      modelStartX = currentModel.x;
      modelStartY = currentModel.y;
    });
    view.addEventListener('pointermove', (e) => {
      if (!pointerDown || !currentModel) return;
      const dx = e.clientX - downX;
      const dy = e.clientY - downY;
      // 未超阈值 = click → 不消费（不位移、不持久化）
      if (!dragActive && Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) return;
      dragActive = true;
      // 位置跟随指针（PixiJS 屏幕坐标，y 同向，无符号翻转）；模型中心锚定不变
      currentModel.x = modelStartX + dx;
      currentModel.y = modelStartY + dy;
    });
    const endDrag = (): void => {
      if (!pointerDown) return;
      pointerDown = false;
      if (dragActive) saveOverride();
      dragActive = false;
    };
    view.addEventListener('pointerup', endDrag);
    view.addEventListener('pointercancel', endDrag);
    view.addEventListener(
      'wheel',
      (e) => {
        if (!currentModel) return;
        e.preventDefault(); // { passive:false }：防止页面滚动
        const newScale = wheelScale(currentModel.scale?.x ?? 1, e.deltaY);
        currentModel.scale.set?.(newScale);
        currentModel.scale.x = newScale;
        currentModel.scale.y = newScale;
        // 缩放围绕模型中心（anchor 0.5,0.5，位置不变）→ 防抖保存
        if (wheelSaveTimer) clearTimeout(wheelSaveTimer);
        wheelSaveTimer = setTimeout(() => {
          wheelSaveTimer = null;
          saveOverride();
        }, WHEEL_SAVE_DEBOUNCE_MS);
      },
      { passive: false },
    );
  }

  // ---- 聊天面板 + 背景层 + 全局提示（UI 重设计：见 docs/research/neko-ui-alignment-and-gaps.md） ----
  const backgroundEl =
    typeof document !== 'undefined'
      ? document.getElementById('app-background')
      : null;
  /** 启动时拉一次背景状态并应用（用户上次选的背景在刷新后仍然在） */
  const restoreBackground = async (): Promise<void> => {
    if (!backgroundEl) return;
    try {
      const state = await fetchBackgroundState();
      applyBackgroundLayer(backgroundEl, state.current_url);
    } catch (err) {
      console.warn('[renderer] 背景状态加载失败（保持透明背景）:', err);
    }
  };
  void restoreBackground();

  // 语音采集回调需要在面板创建前就能引用面板（回调只在用户点麦后触发）
  let chatPanelRef: ChatPanel | null = null;

  const chatHost = document.getElementById('chat');
  if (chatHost) {
    let settingsPanel: SettingsPanel | null = null;
    // P0-2：麦克风采集器 —— 把音频送回后端 ASR（不支持时 chat-ui 自动回落 Web Speech）
    const voiceCapture = createVoiceCapture({
      onFrame: (samples) => {
        wsBridge.sendMicAudio(samples);
      },
      onState: (state, detail) => {
        chatPanelRef?.setVoiceState(state, detail);
        if (state === 'transcribing') {
          // 录音结束 → 让后端跑 ASR 并开始对话
          if (!wsBridge.sendMicAudioEnd()) {
            chatPanelRef?.setVoiceState('error', '未连接服务器，语音未发出');
          } else {
            chatPanelRef?.setThinking(true);
          }
        }
        if (state === 'error' && detail) {
          toast(detail, 'error', { key: 'voice' });
        }
      },
    });

    const chatPanel = initChatPanel(chatHost, {
      onSend: (text) => {
      pipeline.soullink?.conversationEvent('listening'); // 用户开口 → 注视/倾听姿态
      return wsBridge.sendText(text);
    },
      onOpenSettings: () => settingsPanel?.open(),
      voice: voiceCapture,
    });
    chatPanelRef = chatPanel;
    settingsPanel = initSettingsPanel(document.body, {
      // 背景图选中变化 → 立刻换背景（不刷新页面）
      onBackgroundChanged: (state) =>
        applyBackgroundLayer(backgroundEl, state.current_url),
      // 外部接入开关变化 → 聊天头部胶囊同步 + 一条轻提示
      onExternalGateChange: (status) => {
        chatPanel.setExternalGate(status.enabled);
        toast(
          status.enabled
            ? status.allow_remote
              ? '外部接入已开启（本机 + 局域网，需令牌）'
              : '外部接入已开启（仅本机）'
            : '外部接入已断开',
          status.enabled ? 'ok' : 'info',
          { key: 'external-gate' },
        );
      },
    });

    // ---- P0-3 聊天历史：连接即恢复上文 + 抽屉回看/切换/新建/删除 ----
    let currentHistoryUid: string | null = null;
    const historyDrawer = initHistoryDrawer(chatPanel.root(), {
      onRefresh: () => wsBridge.fetchHistoryList(),
      onSelect: (uid) => {
        if (!wsBridge.fetchAndSetHistory(uid)) {
          toast('未连接服务器，无法切换会话', 'error', { key: 'history' });
        }
      },
      onCreate: () => {
        if (wsBridge.createNewHistory()) {
          chatPanel.clear();
          toast('已开始新会话（上一段已保存）', 'ok', { key: 'history' });
        } else {
          toast('未连接服务器，无法新建会话', 'error', { key: 'history' });
        }
      },
      onDelete: (uid) => {
        if (!wsBridge.deleteHistory(uid)) {
          toast('未连接服务器，无法删除会话', 'error', { key: 'history' });
        }
      },
    });

    const headerActions = chatPanel.headerActions();
    if (headerActions) {
      const historyBtn = document.createElement('button');
      historyBtn.type = 'button';
      historyBtn.className = 'chat-icon-btn chat-history-btn ui-lift';
      historyBtn.title = '聊天历史';
      historyBtn.setAttribute('aria-label', '查看聊天历史');
      historyBtn.innerHTML = icon('history');
      historyBtn.addEventListener('click', () => historyDrawer.toggle());
      headerActions.insertBefore(historyBtn, headerActions.firstChild);
    }

    wsBridge.onHistoryData = ({ history_uid, messages }) => {
      currentHistoryUid = history_uid || null;
      historyDrawer.setCurrent(currentHistoryUid);
      const restored = mapHistoryMessages(messages);
      if (restored.length > 0) {
        chatPanel.restoreMessages(restored);
        toast(`已恢复上次对话（${restored.length} 条）`, 'info', { key: 'history' });
      }
    };
    wsBridge.onHistoryList = (histories) => {
      historyDrawer.setHistories(histories);
      historyDrawer.setCurrent(currentHistoryUid);
    };
    wsBridge.onHistoryCreated = (uid) => {
      currentHistoryUid = uid;
      historyDrawer.setCurrent(uid);
    };
    wsBridge.onHistoryDeleted = ({ history_uid, success }) => {
      if (!success) {
        toast('删除会话失败', 'error', { key: 'history' });
        return;
      }
      if (history_uid === currentHistoryUid) {
        currentHistoryUid = null;
        chatPanel.clear();
      }
      wsBridge.fetchHistoryList(); // 删完立刻刷新列表
      toast('会话已删除', 'ok', { key: 'history' });
    };

    // ---- C-M6 重置入口：聊天面板头部「重置模型位置」按钮（main.ts 接线，不重构面板模块） ----
    const chatHeaderActions = chatHost.querySelector('.chat-header-actions');
    if (chatHeaderActions) {
      const resetBtn = document.createElement('button');
      resetBtn.type = 'button';
      resetBtn.className = 'chat-icon-btn chat-reset ui-lift';
      resetBtn.title = '重置模型位置与缩放';
      resetBtn.setAttribute('aria-label', '重置模型位置与缩放');
      resetBtn.innerHTML = icon('recenter');
      resetBtn.addEventListener('click', () => {
        const key = currentModelKey();
        if (!key || !currentModel) return;
        interactionStore.clear(key); // 清 override
        applyModelPose(currentModel, key); // 回相机模式 fit
        toast('已重置角色位置与缩放', 'ok', { key: 'model-reset' });
      });
      chatHeaderActions.insertBefore(resetBtn, chatHeaderActions.firstChild);
    }

    // P1-2 流式上屏：token 增量先到 → 句子级字幕作为权威文本收尾，不重复上屏
    let finalParts: string[] = [];
    wsBridge.onTextDelta = (delta) => {
      chatPanel.streamDelta(delta);
    };
    wsBridge.onSubtitle = (text) => {
      if (!text?.text) return;
      if (chatPanel.isStreaming()) {
        finalParts.push(text.text); // 流式气泡已在显示，这里只记权威文本
        return;
      }
      chatPanel.addMessage({ role: 'assistant', content: text.text });
    };
    // 字幕-声音同步：每句语音真正开播（或静默帧）时，把已积累的干净句子
    // 收尾上屏——对话与声音一起出；无声音路径由句首静默帧立即触发（提早出）。
    // 此前这些干净文本要等整条对话链结束才收尾，观感上"文字比声音晚很多"。
    wsBridge.onSentenceSpoken = () => {
      if (chatPanel.isStreaming() && finalParts.length > 0) {
        chatPanel.finalizeStream(finalParts.join(''));
        finalParts = [];
      }
    };
    // 后端一直在发的 control 消息（此前前端完全忽略 → VAD 打断对用户不可见）
    wsBridge.onControl = (command) => {
      if (command === 'conversation-chain-end') {
        chatPanel.finalizeStream(finalParts.join(''));
        finalParts = [];
        chatPanel.setThinking(false);
        chatPanel.setVoiceState('idle');
      } else if (command === 'interrupt') {
        // Silero VAD 判定用户插话 → 音频已停（ws-bridge），这里给可见反馈
        chatPanel.finalizeStream(finalParts.join(''));
        finalParts = [];
        chatPanel.setVoiceState('interrupted');
        toast('检测到你说话，已打断', 'info', { key: 'voice' });
      } else if (command === 'mic-audio-end') {
        chatPanel.setVoiceState('transcribing');
      }
    };
    // 外部程序注入的文字：以「外部接入」角标的用户气泡上屏 + 顶部轻提示
    wsBridge.onExternalInput = ({ text, source }) => {
      chatPanel.addMessage({ role: 'user', content: text, sourceTag: '外部接入' });
      chatPanel.setThinking(true);
      toast(`收到外部消息（${source}）`, 'info', { key: 'external-input' });
    };
    wsBridge.onConnectionChange = (connected) => {
      chatPanel.setConnected(connected);
      if (connected) {
        clearToast('ws-offline');
        toast('已连接服务器，可以开始聊天', 'ok', { key: 'ws-online' });
      } else {
        chatPanel.setThinking(false);
        toast('与服务器连接断开，正在自动重连…', 'error', {
          key: 'ws-offline',
          sticky: true,
        });
      }
    };
    wsBridge.onTtsStatus = (status) => {
      chatPanel.setTtsStatus(status);
    };
    // PLAN-V3 P1：表演引擎三态/意图转发（legacy 模式下 soullink 为 null，全部 no-op）
    wsBridge.onEmotionFrame = (emotion, strength) => {
      pipeline.soullink?.setEmotion(emotion, strength);
    };
    wsBridge.onCue = (motion) => {
      pipeline.soullink?.cue(motion);
    };
    wsBridge.onAudioSlices = (volumes, sliceMs) => {
      pipeline.soullink?.feedMouth(volumes, sliceMs);
    };
    // 后端错误（未配置 Key / LLM / TTS 失败）→ 错误气泡 + Toast，不再静默
    wsBridge.onError = (message) => {
      chatPanel.finalizeStream();
      chatPanel.setThinking(false);
      chatPanel.addMessage({ role: 'error', content: message });
      toast(message, 'error', { key: 'backend-error', durationMs: 6000 });
    };

    // ---- P0-5 首次启动引导：缺 Provider / 缺 Key 时给一张"缺什么"清单 + 一键跳转 ----
    const onboarding = initOnboarding(document.body, {
      openSettings: (tab) => settingsPanel?.open(tab),
    });
    void onboarding.maybeShow();
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
      if (currentModel && currentModelInfo) {
        // override 存在 → 保持绝对值（不重新 fit）；无 → 相机模式随视口重算
        applyModelPose(currentModel, modelKeyFor(currentModelInfo));
      }
    });
    window.addEventListener?.('beforeunload', () => {
      wsBridge.disconnect();
    });
  }

  return { app, modelLoader, wsBridge, arbiter: pipeline.arbiter };
}

/**
 * 默认 storage（C-M6）：浏览器 localStorage；node/隐私模式等不可用场景降级 no-op
 * （持久化静默禁用，渲染/交互不受影响）。
 */
function createDefaultStorage(): Pick<
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
