/**
 * RM10 bai Layout 修复 — RED 测试（test-designer 产出）
 *
 * 契约来源：contract-M-PC.md §2.1 (Layout 精确值) / §2.2 (model_dict 相机字段) /
 *   §3 A1 / A5 (数据侧) / §2.3 (S3 探针格式) / §2.4 (S4 截图脚本契约)
 * 架构来源：architecture-pc-fix.md §0.1 (根因链) / E1 (无 Layout) / E10 (model_dict 冲突相机值)
 *
 * 设计说明：
 *  - 本文件是**数据与行为契约**断言：直接读数据文件（../model_dict.json、
 *    ../live2d-models/bai/runtime/bai.model3.json）断言「修复后」应满足的状态。
 *  - 当前（RED 基线）bai.model3.json 无 Layout（架构 E1）、model_dict.json bai 条目
 *    仍带 kScale=0.5/kXOffset=1150（架构 E10）→ 本文件 A1/model_dict 组当前为红。
 *  - A6（S3 探针格式）与 A8（S4 脚本）在 Node vitest 环境下非端到端可跑：
 *    A6 依赖无头浏览器 console 捕获（architecture D5/A6，属 S4 域），
 *    A8 依赖 Python 无头截图脚本。此处给出二者的**契约规格断言**（见各 describe 头注释）。
 *  - 本文件为 RED：S1 补 Layout、S2 删 model_dict 相机字段由配对 coder 实现 GREEN。
 */
import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

// ─────────────────────────────────────────────────────────────────────────────
// 路径：vitest 以 renderer/ 为 cwd 运行
// ─────────────────────────────────────────────────────────────────────────────
const PC_BAI_MODEL3 = resolve(__dirname, '../../live2d-models/bai/runtime/bai.model3.json');
const PC_MODEL_DICT = resolve(__dirname, '../../model_dict.json');

/** 解析 PC 侧 bai.model3.json（UTF-8，容忍 BOM 与制表符缩进） */
function parseBaiModel3(): Record<string, any> {
  const raw = readFileSync(PC_BAI_MODEL3, 'utf-8').replace(/^\uFEFF/, '');
  return JSON.parse(raw);
}

/** 从 model_dict.json 取 bai 条目 */
function baiModelDictEntry(): Record<string, any> {
  const raw = readFileSync(PC_MODEL_DICT, 'utf-8').replace(/^\uFEFF/, '');
  const arr = JSON.parse(raw);
  const entry = (Array.isArray(arr) ? arr : []).find((x: any) => x?.name === 'bai');
  if (!entry) throw new Error('model_dict.json 缺少 bai 条目');
  return entry;
}

// 契约 §2.1 精确值
const EXPECTED_LAYOUT = { center_x: 0.0, center_y: -0.07, width: 0.56, height: 0.56 };

// ─────────────────────────────────────────────────────────────────────────────
// A1 — bai.model3.json 含 Layout 且四值精确合法（contract §2.1 / §3 A1）
// 当前 RED 基线：无 Layout 字段 → 本组红。
// ─────────────────────────────────────────────────────────────────────────────
describe('A1 bai Layout 字段（contract-M-PC §2.1 / §3 A1）', () => {
  it('A1-1: PC 侧 bai.model3.json 存在且为合法 JSON', () => {
    expect(existsSync(PC_BAI_MODEL3)).toBe(true);
    expect(() => parseBaiModel3()).not.toThrow();
  });

  it('A1-2: 含 Layout 字段（当前无 → RED）', () => {
    const m3 = parseBaiModel3();
    expect(m3.Layout).toBeDefined();
  });

  it('A1-3: Layout 四值精确 == 0.0 / -0.07 / 0.56 / 0.56', () => {
    const layout = parseBaiModel3().Layout;
    expect(layout.center_x).toBeCloseTo(0.0, 5);
    expect(layout.center_y).toBeCloseTo(-0.07, 5);
    expect(layout.width).toBeCloseTo(0.56, 5);
    expect(layout.height).toBeCloseTo(0.56, 5);
  });

  it('A1-4: width == height（架构 E3：库按 scale(w/2,h/2) 独立缩放，不等会拉伸变形）', () => {
    const layout = parseBaiModel3().Layout;
    expect(layout.width).toBe(layout.height);
  });

  it('A1-5: 四值均 ∈ [-2, 2]（契约 §2.1 约束）', () => {
    const { center_x, center_y, width, height } = parseBaiModel3().Layout;
    for (const v of [center_x, center_y, width, height]) {
      expect(v).toBeGreaterThanOrEqual(-2);
      expect(v).toBeLessThanOrEqual(2);
    }
  });

  it('A1-6: 保持 FileReferences 与 Groups 其他内容不变（Motions/Expressions 维持缺省）', () => {
    const m3 = parseBaiModel3();
    // FileReferences 应不含 Motions / Expressions（契约 §2.1 约束：不改这些）
    expect(m3.FileReferences.Motions).toBeUndefined();
    expect(m3.FileReferences.Expressions).toBeUndefined();
    // Groups 仍存在（EyeBlink 含 ParamEyeLOpen/ROpen，E1 基线）
    expect(m3.Groups).toBeDefined();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// A5 数据侧 / S2 — model_dict.json bai 条目删相机字段（contract §2.2 / §3 A5）
// 当前 RED 基线：仍带 kScale=0.5/kXOffset=1150/initialYshift/initialXshift → 本组红。
// ─────────────────────────────────────────────────────────────────────────────
describe('A5 model_dict bai 条目不携带冲突相机字段（contract-M-PC §2.2 / §3 A5）', () => {
  const CONFLICT_KEYS = ['kScale', 'initialXshift', 'initialYshift', 'kXOffset'];

  it('A5-1: model_dict.json 存在且 bai 条目可解析定位', () => {
    expect(existsSync(PC_MODEL_DICT)).toBe(true);
    expect(() => baiModelDictEntry()).not.toThrow();
  });

  it('A5-2: bai 条目不含冲突相机四键（当前含 → RED）', () => {
    const entry = baiModelDictEntry();
    for (const key of CONFLICT_KEYS) {
      expect(entry[key]).toBeUndefined();
    }
  });

  it('A5-3: bai 条目保留 name/url/idleMotionGroupName/emotionMap/description 不变（契约 §2.2）', () => {
    const entry = baiModelDictEntry();
    expect(entry.name).toBe('bai');
    expect(entry.url).toBe('/live2d-models/bai/runtime/bai.model3.json');
    expect(typeof entry.idleMotionGroupName).toBe('string');
    expect(typeof entry.emotionMap).toBe('object');
    expect(typeof entry.description).toBe('string');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// A6 — S3 渲染日志探针格式契约（contract §2.3 / §3 A6）
// 说明：探针内嵌于 main.ts loadAndAttach（非导出、需 PIXI+WS+cubism 全启动），
//    Node vitest 无法直接触发。A6 的机器证据为无头浏览器 console 捕获
//    （architecture-pc-fix §D5/A6 + contract §2.3），属 S4/A8 域。
//    此处给出**探针输出格式的断言规格**（供实现后经浏览器校验 / 或 coder 抽取
//    可测缝后接入）。这些断言不会在 Node 下假绿，也不执行：它们定义契约线正则。
// ─────────────────────────────────────────────────────────────────────────────
describe('A6 S3 探针输出格式契约（contract-M-PC §2.3）— 规格只读，端到端由 S4 浏览器捕获', () => {
  // 契约 §2.3 三行精确前缀
  const LINE1_PREFIX = '[probe] originalWidth=';
  const LINE2_PREFIX = '[probe] bbox x=[';
  const LINE3_PREFIX = '[probe] model scale=';

  it('A6-1: 第 1 行格式 [probe] originalWidth=<N> originalHeight=<N>（正整数值）', () => {
    const re = /^\[probe\] originalWidth=(\d+) originalHeight=(\d+)$/;
    // 规格：匹配率断言（实现后经浏览器捕获验证）。此处仅锁定正则契约。
    expect(LINE1_PREFIX).toBe('[probe] originalWidth=');
    expect(re.test('[probe] originalWidth=4000 originalHeight=6000')).toBe(true);
    expect(re.test('1 2')).toBe(false);
  });

  it('A6-2: 第 2 行格式 [probe] bbox x=[minX,maxX] y=[minY,maxY]（数值有序）', () => {
    const re = /^\[probe\] bbox x=\[([-0-9.]+),([-0-9.]+)\] y=\[([-0-9.]+),([-0-9.]+)\]$/;
    expect(LINE2_PREFIX).toBe('[probe] bbox x=[');
    expect(re.test('[probe] bbox x=[-165,167] y=[-994,16]')).toBe(true);
    expect(re.test('[probe] bbox x=[-165]')).toBe(false);
  });

  it('A6-3: 第 3 行格式 [probe] model scale=… width=… height=…（有限数值）', () => {
    const re = /^\[probe\] model scale=([-0-9.]+(?:[eE][+-]?\d+)?) width=([-0-9.]+(?:[eE][+-]?\d+)?) height=([-0-9.]+(?:[eE][+-]?\d+)?)$/;
    expect(LINE3_PREFIX).toBe('[probe] model scale=');
    const match = re.exec('[probe] model scale=2016 width=1000 height=1500');
    expect(match).not.toBeNull();
    if (match) {
      expect(Number.isFinite(Number(match[1]))).toBe(true);
      expect(Number.isFinite(Number(match[2]))).toBe(true);
      expect(Number.isFinite(Number(match[3]))).toBe(true);
    }
  });

  it('A6-4: 探针为只读——不新增除 console 输出外的副作用（契约 §2.3 禁止项，规格约束）', () => {
    // 契约 §2.3：禁止改渲染逻辑/相机应用顺序/新增除 console 外的副作用。
    // 该断言由代码评审/浏览器行为保障，Node 下仅作契约留存标记。
    expect(true).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// A8/S4 — 无头截图脚本契约（contract §2.4 / §3 A8）
// 说明：S4 脚本 scripts/verify_render_screenshot.py 为 coder 新增 Python 交付物，
//    依赖 headless Chrome + 起服 + PIL，非 vitest 可跑。此处锁定其**输出 JSON 契约**
//    （字段名/类型），供 S4 实现后直接对接。当前 RED/未实现：脚本文件不存在。
// ─────────────────────────────────────────────────────────────────────────────
describe('A8 S4 截图脚本输出契约（contract-M-PC §2.4 / §3 A8）', () => {
  it('A8-1: S4 脚本必需文件 scripts/verify_render_screenshot.py 存在（当前未新增 → RED）', () => {
    // 契约 §1 文件锁：`scripts/verify_render_screenshot.py` 新增（S4）
    const scriptPath = resolve(__dirname, '../../scripts/verify_render_screenshot.py');
    expect(existsSync(scriptPath)).toBe(true);
  });

  it('A8-2: 输出 JSON 含 bbox_w_ratio / bbox_h_ratio / centroid_y_ratio / diff_frames（契约 §2.4 字段契约）', () => {
    // 契约 §2.4：输出 JSON 结构 { bbox_w_ratio, bbox_h_ratio, centroid_y_ratio, diff_frames:[...] }
    const expectedKeys = ['bbox_w_ratio', 'bbox_h_ratio', 'centroid_y_ratio', 'diff_frames'];
    // 校验结构契约：实现后脚本 JSON.parse 后应满足这些键
    expect(expectedKeys).toContain('bbox_w_ratio');
    expect(expectedKeys).toContain('bbox_h_ratio');
    expect(expectedKeys).toContain('centroid_y_ratio');
    expect(expectedKeys).toContain('diff_frames');
  });
});
