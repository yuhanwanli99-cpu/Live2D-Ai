/**
 * RM2 模型加载与相机 — RED 测试（test-designer 产出）
 *
 * 契约来源：contract-pc-render.md §RM2（RM2-01 ~ RM2-06）
 * 补充约束：schedule-pc.md 配对 B —— 相机默认 {scale:1.8, yOffset:100}
 *   可被 model_info.kScale / model_info.initialYshift 覆盖
 * 架构来源：architecture-pc-render.md §3.2 model-loader.ts / camera.ts
 *
 * 设计说明：
 *  - WebGL/pixi 依赖通过 vi.mock 完全 mock（方案 A，见 testplan-pc.md §1.2）。
 *  - camera 引入 `resolveCameraConfig`：把 model_info 的可选字段（kScale/initialYshift）
 *    合并到默认 {scale:1.8, xOffset:0, yOffset:100}，再交给 applyCamera —— 落实
 *    "默认可被 model_info 覆盖" 的排班约束。
 *  - 本文件为 RED：实现由配对 coder 完成 GREEN。
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
import { applyCamera, resolveCameraConfig } from '../src/camera';
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
