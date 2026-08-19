/**
 * RM2 模型加载与相机 — RED 测试（test-designer 产出）
 *
 * 契约来源：
 *   - contract-pc-render.md §RM2（RM2-01 ~ RM2-06）
 *   - contract-C-M2.md（M2-A1/A3/A4/A5 —— CAMERA_MODE 默认 / FIT-05/06/07 增量契约）
 * 补充约束：
 *   - schedule-pc.md 配对 B —— 相机默认 {scale:1.8, yOffset:100}
 *     可被 model_info.kScale / model_info.initialYshift 覆盖
 *   - contract-C-M2.md §0/§2.1 —— 默认 CAMERA_MODE='fit'（D2 裁定，禁止默认半身）
 * 架构来源：architecture-pc-fit.md §4.1（M2 参数化，选型 1 方案 A + 选型 2 公式 1）
 *
 * 设计说明：
 *  - WebGL/pixi 依赖通过 vi.mock 完全 mock（方案 A，见 testplan-pc.md §1.2）。
 *  - camera 引入 `resolveCameraConfig`：把 model_info 的可选字段（kScale/initialYshift）
 *    合并到默认 {scale:1.8, xOffset:0, yOffset:100}，再交给 applyCamera —— 落实
 *    "默认可被 model_info 覆盖" 的排班约束。
 *  - C-M2 增量（contract-C-M2.md）：camera.ts 新增 CameraMode / CAMERA_MODE /
 *    FIT_PADDING / HALF_BODY_BOOST / HALF_BODY_Y_OFFSET / computeFitScale /
 *    applyCameraMode 分发。FIT-05/06/07 为新增契约断言；FIT-01~04 原样保留。
 *  - FIT-07 半身数值走常量引用（HALF_BODY_BOOST / HALF_BODY_Y_OFFSET），不硬编码数值——
 *    半身 boost/yOffset 最终数值由用户目视 A/B 裁定（M2-A8 human_signoff），
 *    故 CT 断言只锁"结构引用常量"，用户改数值不破坏测试。
 *  - 本文件为 RED：实现由配对 coder 完成 GREEN。C-M2 新增导出在 src/camera.ts
 *    尚不存在 → 本文件 import 阶段即编译失败（RED）。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

// ─────────────────────────────────────────────────────────────────────────────
// Mock pixi-live2d-display / pixi.js —— 不真实加载 WebGL
// ─────────────────────────────────────────────────────────────────────────────

/** 可注入的 Live2DModel 假对象（模拟 PixiJS DisplayObject + destroyed） */
function makeFakeModel() {
  return {
    scale: { set: vi.fn(), x: 1, y: 1 },
    anchor: { set: vi.fn() },
    rotation: 0,
    x: 0,
    y: 0,
    width: 622, // 模型基础宽度（Layout 归一化后，bai 参考）
    height: 933, // 模型基础高度
    destroyed: false,
    parent: null,
    destroy: vi.fn(function (this: any) {
      this.destroyed = true;
    }),
    internalModel: { coreModel: 'fake-core-model' },
  };
}

/**
 * 注意：camera.ts / model-loader.ts 通过 import 从 pixi 包取 Live2DModel 类型/值。
 * coder 实现应使用类型导入（不会被类型擦除触发运行时），值导入处仅用到
 * Live2DModel.from 静态方法（在 mock 中提供）。
 */
vi.mock('pixi-live2d-display-lipsyncpatch/cubism4', () => ({
  Live2DModel: {
    from: vi.fn(),
  },
}));

vi.mock('pixi-live2d-display-lipsyncpatch', () => ({
  Live2DModel: {
    from: vi.fn(),
  },
}));

// mock pixi.js Application —— model-loader 接收的 app 只用到 stage
vi.mock('pixi.js', () => ({
  Application: vi.fn(),
}));

import { loadModel, unloadModel } from '../src/model-loader';
import {
  applyCamera,
  resolveCameraConfig,
  applyFitCamera,
  // ── C-M2 增量导出（契约 contract-C-M2.md §2.1）──
  // 以下在 src/camera.ts 尚不存在 → RED（import 编译失败）。全部契约签名以
  // contract-C-M2.md 为准。
  CAMERA_MODE,
  FIT_PADDING,
  HALF_BODY_BOOST,
  HALF_BODY_Y_OFFSET,
  computeFitScale,
  applyCameraMode,
} from '../src/camera';
import { Live2DModel } from 'pixi-live2d-display-lipsyncpatch/cubism4';

const mockFrom = Live2DModel.from as unknown as ReturnType<typeof vi.fn>;

describe('RM2 model-loader — 模型加载契约', () => {
  let app: any;

  beforeEach(() => {
    app = {
      stage: { addChild: vi.fn(), children: [] },
    };
    mockFrom.mockReset();
  });

  it('RM2-01: loadModel(app, 合法url) 返回含 model 与 internalModel 的结果', async () => {
    const fake = makeFakeModel();
    mockFrom.mockResolvedValue(fake);

    const result = await loadModel(app, '/live2d-models/白-免费版/runtime/白-免费版.model3.json');

    expect(result).not.toBeNull();
    expect(result.model).toBe(fake);
    expect(result.internalModel).toBe(fake.internalModel);
    // model 必须是 Live2DModel-like（契约：result.model instanceof Live2DModel == true）
    expect(result.model).toBeDefined();
  });

  it('RM2-01b: loadModel 以 autoInteract:false / autoUpdate:true 调用 Live2DModel.from', async () => {
    const fake = makeFakeModel();
    mockFrom.mockResolvedValue(fake);

    const url = '/live2d-models/白-免费版/runtime/白-免费版.model3.json';
    await loadModel(app, url);

    expect(mockFrom).toHaveBeenCalledTimes(1);
    const [calledUrl, options] = mockFrom.mock.calls[0];
    expect(calledUrl).toBe(url);
    expect(options.autoInteract).toBe(false); // 不自动鼠标跟踪
    expect(options.autoUpdate).toBe(true);    // PIXI tick 自动驱动 Cubism update
  });

  it('RM2-02: loadModel(app, 非法url) 抛出明确错误（非静默失败）', async () => {
    mockFrom.mockRejectedValue(new Error('Failed to load model: 404'));

    await expect(loadModel(app, '/liver2d/不存在.model3.json')).rejects.toThrowError(
      /model|load/i
    );
  });

  it('RM2-03: 加载成功后 model 已添加到 app.stage', async () => {
    const fake = makeFakeModel();
    mockFrom.mockResolvedValue(fake);

    // 模拟 addChild 实际把 model.parent 指向 stage
    app.stage.addChild.mockImplementation((child: any) => {
      child.parent = app.stage;
      app.stage.children.push(child);
    });

    await loadModel(app, '/live2d-models/白-免费版/runtime/白-免费版.model3.json');

    expect(app.stage.addChild).toHaveBeenCalledWith(fake);
    expect(app.stage.children).toContain(fake);
    expect(fake.parent).toBe(app.stage);
  });

  it('RM2-06: unloadModel(model) 后 model.destroyed=true 且从 stage 移除', () => {
    const fake = makeFakeModel();
    fake.destroy = vi.fn(function (this: any) {
      this.destroyed = true;
      this.parent = null;
    });
    fake.parent = app.stage;

    unloadModel(fake as any);

    expect(fake.destroy).toHaveBeenCalled();
    expect(fake.destroyed).toBe(true);
    expect(app.stage.children).not.toContain(fake);
  });
});

describe('RM2 camera — resolveCameraConfig 默认值 + 覆盖', () => {
  it('无 any 输入时返回默认 {scale:1.8, xOffset:0, yOffset:100}', () => {
    const cfg = resolveCameraConfig();
    expect(cfg).toEqual({ scale: 1.8, xOffset: 0, yOffset: 100 });
  });

  it('model_info.kScale 覆盖默认 scale', () => {
    // 语义：旧前端 model_info.kScale → pixi scale（schedule：default 可被覆盖）
    const cfg = resolveCameraConfig({ scale: 2.4 });
    expect(cfg.scale).toBe(2.4);
    expect(cfg.yOffset).toBe(100); // 其他字段保留默认
  });

  it('model_info.initialYshift 覆盖默认 yOffset', () => {
    const cfg = resolveCameraConfig({ yOffset: 150 });
    expect(cfg.yOffset).toBe(150);
    expect(cfg.scale).toBe(1.8); // 其他字段保留默认
  });

  it('model_info.kXOffset 覆盖默认 xOffset', () => {
    const cfg = resolveCameraConfig({ xOffset: -20 });
    expect(cfg.xOffset).toBe(-20);
  });

  it('部分输入只覆盖提供项（kScale 单独覆盖不改 yOffset）', () => {
    const cfg = resolveCameraConfig({ scale: 1.5 });
    expect(cfg).toEqual({ scale: 1.5, xOffset: 0, yOffset: 100 });
  });
});

describe('A4 bai 相机对齐 — 无相机字段回退 DEFAULT_CAMERA（contract-M-PC §3 A4 / §2.2）', () => {
  it('A4-1: 仅含需求字段(name/url/idleMotionGroupName/emotionMap/description)的 bai model_info → 回退 DEFAULT_CAMERA {scale:1.8,xOffset:0,yOffset:100}', () => {
    // 契约 §2.2：删除 kScale/initialXshift/initialYshift/kXOffset 四个键后，
    // resolveCameraConfig(baiModelInfo) 全部字段缺失 → 回退 DEFAULT_CAMERA
    const baiNoCamera = {
      name: 'bai',
      url: '/live2d-models/bai/runtime/bai.model3.json',
      idleMotionGroupName: '',
      emotionMap: { neutral: 0 },
      description: '白-免费版',
    };
    const cfg = resolveCameraConfig(baiNoCamera as any);
    expect(cfg).toEqual({ scale: 1.8, xOffset: 0, yOffset: 100 });
  });

  it('A4-2: registry 最小 model_info（仅 name/url/emotionMap，架构 E11）→ 同样回退 DEFAULT_CAMERA', () => {
    // 架构 architecture-pc-fix.md E11：_registry_model_info 只含 name/url/emotionMap
    const registryMinimal = { name: 'bai', url: '/live2d-models/bai/runtime/bai.model3.json', emotionMap: { neutral: 0 } };
    const cfg = resolveCameraConfig(registryMinimal as any);
    expect(cfg).toEqual({ scale: 1.8, xOffset: 0, yOffset: 100 });
  });

  it('A4-3(回归守门): 若 bai model_info 显式带 kScale=0.5/kXOffset=1150（S2 前冲突配置）仍被覆盖——既有语义不回归', () => {
    // 契约 A4：resolveCameraConfig({kScale:0.5,...}) 仍被覆盖（既有语义不回归）
    const conflictConfig = { kScale: 0.5, initialXshift: 0, initialYshift: 0, kXOffset: 1150 };
    const cfg = resolveCameraConfig(conflictConfig as any);
    expect(cfg.scale).toBe(0.5);
    expect(cfg.xOffset).toBe(1150);
    expect(cfg.yOffset).toBe(0);
  });

  it('A4-4: resolveCameraConfig 是纯函数——输入不含相机字段时不产生任何相机副作用字段', () => {
    const cfg = resolveCameraConfig({ name: 'bai' } as any);
    // 返回对象应恰好等于默认（不新增 kScale/kXOffset 等冗余键）
    expect(Object.keys(cfg).sort()).toEqual(['scale', 'xOffset', 'yOffset']);
  });
});

describe('RM2 camera — applyCamera 半身构图逻辑', () => {
  let model: any;

  beforeEach(() => {
    model = makeFakeModel();
  });

  it('RM2-04: applyCamera(scale=1.8) 后 model.scale.x === 1.8', () => {
    applyCamera(model, { scale: 1.8, xOffset: 0, yOffset: 100 }, 800, 600);
    expect(model.scale.set).toHaveBeenCalledWith(1.8);
    expect(model.scale.x).toBe(1.8);
  });

  it('RM2-04b: 以 kScale 覆盖后 applyCamera 生效 scale 值', () => {
    const cfg = resolveCameraConfig({ scale: 2.4 });
    applyCamera(model, cfg, 800, 600);
    expect(model.scale.set).toHaveBeenCalledWith(2.4);
  });

  it('RM2-05: applyCamera 后 model.x/model.y 为有限数值（非 NaN/Infinity）', () => {
    applyCamera(model, { scale: 1.8, xOffset: 0, yOffset: 100 }, 800, 600);
    expect(Number.isFinite(model.x)).toBe(true);
    expect(Number.isFinite(model.y)).toBe(true);
  });

  it('以画布中心 + 偏移放置（半身构图：中心 + yOffset 下移）', () => {
    applyCamera(model, { scale: 1.8, xOffset: 0, yOffset: 100 }, 800, 600);
    // 架构：model.x = canvasWidth/2 + xOffset；model.y = canvasHeight/2 + yOffset
    expect(model.x).toBeCloseTo(800 / 2 + 0, 5);      // 400
    expect(model.y).toBeCloseTo(600 / 2 + 100, 5);    // 400
  });

  it('anchor 设为 (0.5, 0.5)（以模型中心为锚点的半身构图）', () => {
    applyCamera(model, { scale: 1.8, xOffset: 0, yOffset: 100 }, 800, 600);
    expect(model.anchor.set).toHaveBeenCalledWith(0.5, 0.5);
  });

  it('xOffset 参与水平定位（kXOffset 生效）', () => {
    const cfg = resolveCameraConfig({ xOffset: -30 });
    applyCamera(model, cfg, 800, 600);
    expect(model.x).toBeCloseTo(800 / 2 - 30, 5);
  });
});

describe('RM2 camera — applyFitCamera 自适应 fit（spoq-fix 返工：大画布小角色完整可见）', () => {
  let model: any;

  beforeEach(() => {
    model = makeFakeModel();
  });

  it('FIT-01: 模型 fit 到画布内完整可见（scale = min(画布/模型宽, 画布/模型高)×0.95）', () => {
    // 模型 622×933，画布 800×600 → 限制维度是高：600/933 = 0.643 × 0.95 ≈ 0.611
    applyFitCamera(model, 800, 600);
    const expectScale = Math.min(800 / 622, 600 / 933) * (1 - 0.05);
    expect(model.scale.x).toBeCloseTo(expectScale, 4);
    expect(model.scale.y).toBeCloseTo(expectScale, 4);
    expect(model.scale.set).toHaveBeenCalled();
  });

  it('FIT-02: fit 后模型居中（anchor 0.5 + 画布中心）', () => {
    applyFitCamera(model, 800, 600);
    expect(model.anchor.set).toHaveBeenCalledWith(0.5, 0.5);
    expect(model.x).toBeCloseTo(800 / 2, 5);
    expect(model.y).toBeCloseTo(600 / 2, 5);
  });

  it('FIT-03: 无效尺寸（0/NaN）不改变模型状态（防未就绪时崩）', () => {
    const m0 = makeFakeModel();
    m0.width = 0;
    m0.height = 0;
    applyFitCamera(m0, 800, 600);
    expect(m0.scale.x).toBe(1); // 未变
    const mN = makeFakeModel();
    mN.width = NaN;
    applyFitCamera(mN, 800, 600);
    expect(mN.scale.x).toBe(1);
  });

  it('FIT-04: 大画布小角色不再被固定 1.8 推近（回归：曾只显示局部"大腿"）', () => {
    // bai: 基础 622×933，画布 800×600；1.8 倍时 1120×1680 超出画布 → 修复后 fit scale < 1
    applyFitCamera(model, 800, 600);
    expect(model.scale.x).toBeLessThan(1);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// C-M2：PC fit 参数化 —— 半身构图公式 1 契约（contract-C-M2.md）
//   M2-A1  默认 CAMERA_MODE='fit'（D2 裁定：禁止默认半身）
//   M2-A3  FIT-05：padding 显式生效 + FIT_PADDING === 0.05
//   M2-A4  FIT-06：applyCameraMode 模式分发（fit 默认 / half-body 模式切换）
//   M2-A5  FIT-07：half-body fit 相对式结构（scale=fitScale×boost、坐标=中心+偏移），
//           值走常量引用（不硬编码数值）；窗口 resize 构图自适应
//
//   [UNSURE 边界] 本测试仅断言 camera.ts 导出（applyCameraMode / computeFitScale /
//   FIT_PADDING / CAMERA_MODE / HALF_BODY_BOOST / HALF_BODY_Y_OFFSET）的契约语义；
//   main.ts applyModelCamera 为 DOM 接线不进单测（既有约定，vitest env=node），
//   该接线正确性由 S4 截图脚本 + 用户目视双覆盖（M2-A8）。
// ─────────────────────────────────────────────────────────────────────────────
describe('C-M2 camera — 模式机制（M2-A1/A3/A4/A5，contract-C-M2.md）', () => {
  let model: any;

  beforeEach(() => {
    model = makeFakeModel();
  });

  it('M2-A1: 默认 CAMERA_MODE === "fit"（D2 裁定：已提交状态为默认，禁止默认半身）', () => {
    // 契约 §2.1：CAMERA_MODE 是唯一默认开关，值必须为 'fit'（默认行为保持 fit 不变）
    expect(CAMERA_MODE).toBe('fit');
  });

  it('M2-A3/FIT-05a: FIT_PADDING 常量 === 0.05（现为 applyFitCamera 形参默认值抽出）', () => {
    // 契约 §2.1 / M2-A3：FIT_PADDING === 0.05
    expect(FIT_PADDING).toBe(0.05);
  });

  it('M2-A3/FIT-05b: applyFitCamera(m,800,600,0) → scale == min(800/622,600/933)（padding=0 无 0.95 折扣）', () => {
    // 契约 M2-A3：padding 显式 0 → 无 (1-padding) 折扣
    applyFitCamera(model, 800, 600, 0);
    const expectScale = Math.min(800 / 622, 600 / 933);
    expect(model.scale.x).toBeCloseTo(expectScale, 4);
    expect(model.scale.y).toBeCloseTo(expectScale, 4);
  });

  it('M2-A3/FIT-05c: applyFitCamera(m,800,600,0.1) → scale == min(...)×0.9（显式 padding 生效）', () => {
    // 契约 M2-A3：padding=0.1 → ×0.9
    applyFitCamera(model, 800, 600, 0.1);
    const expectScale = Math.min(800 / 622, 600 / 933) * (1 - 0.1);
    expect(model.scale.x).toBeCloseTo(expectScale, 4);
    expect(model.scale.y).toBeCloseTo(expectScale, 4);
  });

  it('M2-A4/FIT-06a: computeFitScale(800,600,model) == min(...)×(1-FIT_PADDING)（纯函数，fit 共用基准）', () => {
    // 契约 §2.1：computeFitScale 为 applyFitCamera 与半身共用的 fit scale 纯函数；
    // 默认 padding 采用 FIT_PADDING。
    const s = computeFitScale(800, 600, model);
    const expectScale = Math.min(800 / 622, 600 / 933) * (1 - FIT_PADDING);
    expect(s).toBeCloseTo(expectScale, 4);
  });

  it('M2-A4/FIT-06b: applyCameraMode(m,"fit",800,600) → scale==computeFitScale、居中(x=400,y=300,anchor 0.5)', () => {
    // 契约 M2-A4：mode='fit' → 现有 applyFitCamera 语义（contain-fit + padding + 居中）
    applyCameraMode(model, 'fit', 800, 600);
    const expectScale = computeFitScale(800, 600, model);
    expect(model.scale.x).toBeCloseTo(expectScale, 4);
    expect(model.scale.y).toBeCloseTo(expectScale, 4);
    expect(model.x).toBeCloseTo(800 / 2, 5);   // 400
    expect(model.y).toBeCloseTo(600 / 2, 5);   // 300
    expect(model.anchor.set).toHaveBeenCalledWith(0.5, 0.5);
  });

  it('M2-A4/FIT-06c: applyCameraMode(m,"half-body",800,600) → scale==fitScale×HALF_BODY_BOOST（常量引用）、y==300+HALF_BODY_Y_OFFSET、anchor(0.5,0.5)', () => {
    // 契约 §2.1 / M2-A4：
    //   scale = computeFitScale(...) × HALF_BODY_BOOST
    //   x     = canvasWidth/2 + (xOffset ?? 0) = 400
    //   y     = canvasHeight/2 + (yOffset ?? HALF_BODY_Y_OFFSET) = 300 + HALF_BODY_Y_OFFSET
    // 值全部走常量引用（HALF_BODY_BOOST / HALF_BODY_Y_OFFSET），不锁具体数值 ——
    // 用户目视 A/B 改数值不破坏测试（M2-A8 human_signoff）。
    applyCameraMode(model, 'half-body', 800, 600);
    const expectScale = computeFitScale(800, 600, model) * HALF_BODY_BOOST;
    expect(model.scale.x).toBeCloseTo(expectScale, 4);
    expect(model.scale.y).toBeCloseTo(expectScale, 4);
    expect(model.x).toBeCloseTo(800 / 2, 5);                                  // 400（中心 + xOffset=0）
    expect(model.y).toBeCloseTo(600 / 2 + HALF_BODY_Y_OFFSET, 5);            // 300 + 常量
    expect(model.anchor.set).toHaveBeenCalledWith(0.5, 0.5);                 // 对齐 applyCamera 语义
    // 结构契约守卫：boost 必须 >1（半身观感）——但只断言"结构上 >0 且非固定 fit"，不锁具体值
    expect(HALF_BODY_BOOST).toBeGreaterThan(0);
  });

  it('M2-A4/FIT-06d: applyCameraMode half-body 支持 config.xOffset / config.yOffset 显式覆盖', () => {
    // 契约 §2.1：x = canvasWidth/2 + (config?.xOffset ?? 0)；y = canvasHeight/2 + (config?.yOffset ?? HALF_BODY_Y_OFFSET)
    applyCameraMode(model, 'half-body', 800, 600, { xOffset: -40, yOffset: 200 });
    expect(model.x).toBeCloseTo(800 / 2 - 40, 5);       // 400 - 40
    expect(model.y).toBeCloseTo(600 / 2 + 200, 5);      // 300 + 200
  });

  it('M2-A4/FIT-06e: applyCameraMode(m,"fit",...) 与 applyFitCamera 同参数语义一致（默认路径不回归）', () => {
    // 契约 §2.1：mode='fit' → 现有 applyFitCamera 语义；二者结果应一致
    const viaMode = makeFakeModel();
    const viaDirect = makeFakeModel();
    applyCameraMode(viaMode, 'fit', 800, 600);
    applyFitCamera(viaDirect, 800, 600);
    expect(viaMode.scale.x).toBeCloseTo(viaDirect.scale.x, 4);
    expect(viaMode.scale.y).toBeCloseTo(viaDirect.scale.y, 4);
    expect(viaMode.x).toBeCloseTo(viaDirect.x, 5);
    expect(viaMode.y).toBeCloseTo(viaDirect.y, 5);
  });

  it('M2-A5/FIT-07a: 半身 scale == computeFitScale×HALF_BODY_BOOST（结构断言，值走常量；模型622×933/画布800×600）', () => {
    // 契约 M2-A5：模型 622×933 / 画布 800×600 → half-body scale == 0.611×HALF_BODY_BOOST
    // 但不得硬编码 0.611 —— 断言用 computeFitScale(...)×HALF_BODY_BOOST 结构式，改 padding/常量不破坏。
    applyCameraMode(model, 'half-body', 800, 600);
    const fitScale = computeFitScale(800, 600, model);
    const expectScale = fitScale * HALF_BODY_BOOST;
    expect(model.scale.x).toBeCloseTo(expectScale, 4);
    // 结构守卫：fit 是 contain-fit（≤1），半身放大必然 > fit 且 >1（半身观感由用户裁定具体值）
    expect(model.scale.x).toBeGreaterThan(fitScale);
  });

  it('M2-A5/FIT-07b: 半身 x==400、y==600/2+HALF_BODY_Y_OFFSET（中心+偏移，值走常量）', () => {
    // 契约 M2-A5：x=400（800/2）、y=400（=300+100 初值语义，但断言引用常量不锁死 100）
    applyCameraMode(model, 'half-body', 800, 600);
    expect(model.x).toBeCloseTo(800 / 2, 5);                       // 400
    expect(model.y).toBeCloseTo(600 / 2 + HALF_BODY_Y_OFFSET, 5);  // 300 + 常量
    expect(model.anchor.set).toHaveBeenCalledWith(0.5, 0.5);
  });

  it('M2-A5/FIT-07c: 画布 1600×1200 → 半身 scale 同比自适应放大（非固定绝对值；窗口 resize 构图稳定）', () => {
    // 契约 M2-A5：画布 1600×1200 时 fitScale = min(1600/622,1200/933)×(1-padding)，
    // 同比放大（公式 1 fit 相对式），非固定 1.8 绝对值。
    const fit800: number = computeFitScale(800, 600, model);
    const fit1600: number = computeFitScale(1600, 1200, model);
    // 同长宽比画布 → fitScale 严格同比翻倍；半身 scale 同样同比（结构：均 ×HALF_BODY_BOOST）
    const halfBody800 = fit800 * HALF_BODY_BOOST;
    const halfBody1600 = fit1600 * HALF_BODY_BOOST;
    applyCameraMode(model, 'half-body', 1600, 1200);
    expect(model.scale.x).toBeCloseTo(halfBody1600, 4);
    expect(model.scale.x).toBeCloseTo(2 * halfBody800, 4); // 同比 ×2（fit 相对式自适应）
    // 对比守卫：非固定 1.8 绝对值——大画布务必放大（自适应构图稳定）
    expect(halfBody1600).toBeGreaterThan(halfBody800);
  });

  it('M2-A7 前置守卫: HALF_BODY_BOOST / HALF_BODY_Y_OFFSET 必须是有限正数与有限数值（结构合法性）', () => {
    // 半身公式数值当前为 human_signoff 待裁定项（A8）；测试只校验常量结构合法，
    // 不锁具体值——用户回填后测试依旧稳定。
    expect(Number.isFinite(HALF_BODY_BOOST)).toBe(true);
    expect(Number.isFinite(HALF_BODY_Y_OFFSET)).toBe(true);
  });
});
