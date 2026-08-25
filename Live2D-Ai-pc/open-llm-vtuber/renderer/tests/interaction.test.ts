/**
 * C-M6 用户交互 — RED 测试（test-designer 产出）
 *
 * 契约来源：
 *   - contract-C-M6.md §2.1/§3（M6-A1 ~ A6）
 *   - architecture-pc-fit.md（C-M2 相机模式 = 本模块默认路径基座；override 优先于相机模式）
 *   - search-report-neko-live2d.md R07（滚轮步进 min(|Δy|/1000,0.08)）/ R08（拖拽阈值 10px）
 *     + search-report-industry-live2d.md R03（钳制 MIN=0.1 / MAX=5.0）
 *
 * 测试边界（既有约定，vitest env=node，无 DOM/localStorage/事件）：
 *   - 纯函数部分（wheelZoomStep / wheelScale / clampScale / serializeOverride /
 *     parseOverride / storageKeyForModel / shouldApplyOverride）进单测；
 *   - createInteractionStore 用**注入内存 fake storage**（契约 §2.1 明确 storage 注入式，
 *     env=node 无真实 localStorage）；
 *   - main.ts 事件接线（pointer/wheel/重置按钮 DOM 接线）不进单测（既有约定）——
 *     由 M6-A9 浏览器 E2E（主代理）覆盖。main.ts 接线在 main.ts（禁读）内，不在交互文件。
 *
 * [UNSURE / 契约内部矛盾 —— 必须主代理裁决，禁止测试方擅自定值]
 *   contract-C-M6.md §2.1 声明：
 *     WHEEL_STEP_CAP = 0.08（N.E.K.O. zoomStep = min(|Δy|/1000, 0.08)）
 *     WHEEL_DELTA_DIVISOR = 1000
 *     wheelZoomStep(deltaY) = min(|deltaY|/1000, 0.08)
 *   但 §3 M6-A1 断言 "Δy=±300 → 0.3"、M6-A2 断言 "old=1.0、Δy=-300 → 1.0×1.3=1.3"。
 *
 *   以该公式计算：
 *     |300|/1000 = 0.3 → min(0.3, 0.08) = **0.08**（≠ 契约期望的 0.3）
 *     factor(0.08) = 1.08 → 1.0×1.08 = **1.08**（≠ 契约期望的 1.3）
 *   故 "Δy=±300 → 0.3 / 1.3*" 与 "min(|Δy|/1000, 0.08)" + "WHEEL_STEP_CAP=0.08"
 *   三者**不可同时满足**：不存在单一 min(|δ|/D, C) 使 δ=300→0.3 且 C=0.08。
 *
 *   处置（不猜，如实暴露）：本文件按下述原则写，
 *     - 契约**明确示例值**（Δy=300→0.3、wheelScale Δy=-300→1.3）按契约原文锁定
 *       （契约 M6-A1/A2 验收表是 RED 首要依据）；实现者按字面 min 公式实现必然 RED，
 *       即把该缺陷暴露给主代理裁决。
 *     - 同时以 A3 "常量 0.1/5.0/0.08/1000 与 §2.1 一致" 锁定常量本身（此条不冲突）。
 *     - 步进公式的"封顶行为 + Δy=0/NaN 防御"（与常量一致的部分）独立成块锁定。
 *   主代理需就 Δy=300 期望值（0.3 vs 0.08）做出裁决并同步契约。
 *
 * 本文件为 RED：src/interaction.ts 尚不存在（契约标注"新增"）→ import 阶段即编译失败。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

// ─────────────────────────────────────────────────────────────────────────────
// C-M6 交互纯函数 + 注入式持久化（contract-C-M6.md §2.1）
// 以下从 ../src/interaction 导入 —— 该模块尚不存在 → 编译失败 = RED。
// ─────────────────────────────────────────────────────────────────────────────
import {
  INTERACTION_SCALE_MIN,
  INTERACTION_SCALE_MAX,
  WHEEL_STEP_CAP,
  WHEEL_DELTA_DIVISOR,
  wheelZoomStep,
  wheelScale,
  clampScale,
  serializeOverride,
  parseOverride,
  storageKeyForModel,
  createInteractionStore,
  shouldApplyOverride,
  type OverrideState,
} from '../src/interaction';

// ─────────────────────────────────────────────────────────────────────────────
// 注入内存 fake storage（契约 §2.1：createInteractionStore 接收
// Pick<Storage,'getItem'|'setItem'|'removeItem'>；env=node 无 localStorage）
// ─────────────────────────────────────────────────────────────────────────────
function makeMemoryStorage() {
  const map = new Map<string, string>();
  return {
    getItem: vi.fn((k: string) => (map.has(k) ? map.get(k)! : null)),
    setItem: vi.fn((k: string, v: string) => void map.set(k, v)),
    removeItem: vi.fn((k: string) => void map.delete(k)),
  };
}

function validOverride(over: Partial<OverrideState> = {}): OverrideState {
  return { scale: 2, x: 100, y: 50, version: 1, ...over };
}

// ─────────────────────────────────────────────────────────────────────────────
// M6-A1  wheelZoomStep：步进 = min(|Δy|/1000, 0.08)，NaN 防御
// ─────────────────────────────────────────────────────────────────────────────
describe('C-M6 wheelZoomStep（M6-A1，contract-C-M6.md）', () => {
  it('M6-A1a [契约修正→主代理裁决]: Δy=300 → 0.08（封顶，min(0.3,0.08)）', () => {
    // 主代理 2026-08-12 裁决：契约断言行 0.3 与公式 min(|Δy|/1000,0.08) 矛盾，以公式+行业证据为准（neko R07）→ 0.08
    expect(wheelZoomStep(300)).toBeCloseTo(0.08, 5);
  });

  it('M6-A1b [契约修正]: Δy=-300 → 0.08（对称）', () => {
    expect(wheelZoomStep(-300)).toBeCloseTo(0.08, 5);
  });

  it('M6-A1c: Δy=±2000 → 0.08（封顶，min 生效；与常量一致，无歧义）', () => {
    // |2000|/1000 = 2.0 → min(2.0, 0.08) = 0.08
    expect(wheelZoomStep(2000)).toBe(0.08);
    expect(wheelZoomStep(-2000)).toBe(0.08);
  });

  it('M6-A1d: Δy=0 → 0', () => {
    expect(wheelZoomStep(0)).toBe(0);
  });

  it('M6-A1e: Δy=NaN → 0（有限性防御，契约 §2.1 wheelZoomStep 注释）', () => {
    expect(wheelZoomStep(NaN)).toBe(0);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// M6-A2  wheelScale：factor=1+step；newScale = Δy<0 ? old×factor : old/factor
//        → clampScale
// ─────────────────────────────────────────────────────────────────────────────
describe('C-M6 wheelScale（M6-A2，contract-C-M6.md）', () => {
  it('M6-A2a [契约修正→主代理裁决]: old=1.0、Δy=-300 → 1.0×1.08=1.08（放大，step 封顶 0.08）', () => {
    expect(wheelScale(1.0, -300)).toBeCloseTo(1.08, 4);
  });

  it('M6-A2b [契约修正]: old=1.0、Δy=+300 → 1.0/1.08≈0.926（缩小）', () => {
    expect(wheelScale(1.0, 300)).toBeCloseTo(1 / 1.08, 4);
  });

  it('M6-A2c: 方向语义一致（与步进歧义无关）：Δy<0 放大、Δy>0 缩小', () => {
    // 使用 Δy=±2000（封顶步进 0.08，常量一致）
    const zoomedIn = wheelScale(1.0, -2000);
    const zoomedOut = wheelScale(1.0, 2000);
    expect(zoomedIn).toBeGreaterThan(1.0);   // 放大
    expect(zoomedOut).toBeLessThan(1.0);     // 缩小
    // factor=1.08：old×1.08 vs old/1.08
    expect(zoomedIn).toBeCloseTo(1.0 * (1 + 0.08), 4);
    expect(zoomedOut).toBeCloseTo(1.0 / (1 + 0.08), 4);
  });

  it('M6-A2d: old=0.05、Δy=-2000 → clamp 到 0.1（钳制下限；步进封顶 0.08 与常量一致）', () => {
    // 0.05×1.08=0.054 → clampScale → 0.1
    expect(wheelScale(0.05, -2000)).toBe(0.1);
  });

  it('M6-A2e: old=6.0、Δy=-2000 → clamp 到 5.0（钳制上限）', () => {
    // 6.0×1.08=6.48 → clampScale → 5.0（放大路径）
    expect(wheelScale(6.0, -2000)).toBe(5.0);
  });

  it('M6-A2f: old=6.0、Δy=+2000 → 仍 clamp 到 5.0（缩小路径亦钳制）', () => {
    // 6.0/1.08=5.556 → clampScale → 5.0
    expect(wheelScale(6.0, 2000)).toBe(5.0);
  });

  it('M6-A2g: Δy=0 → old 不变', () => {
    expect(wheelScale(1.0, 0)).toBe(1.0);
    expect(wheelScale(2.5, 0)).toBe(2.5);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// M6-A3  clampScale 钳制 [0.1, 5.0]，NaN→1；常量 0.1/5.0/0.08/1000 与 §2.1 一致
// ─────────────────────────────────────────────────────────────────────────────
describe('C-M6 clampScale + 常量（M6-A3，contract-C-M6.md）', () => {
  it('M6-A3a: clampScale(0.05)→0.1 / (1)→1 / (7)→5 / (NaN)→1', () => {
    expect(clampScale(0.05)).toBe(0.1);
    expect(clampScale(1)).toBe(1);
    expect(clampScale(7)).toBe(5.0);
    expect(clampScale(NaN)).toBe(1);
  });

  it('M6-A3b: 边界值 clampScale(0.1)=0.1、clampScale(5.0)=5.0 恰好落在区间内不变化', () => {
    expect(clampScale(0.1)).toBe(0.1);
    expect(clampScale(5.0)).toBe(5.0);
  });

  it('M6-A3c: 常量 INTERACTION_SCALE_MIN=0.1 / MAX=5.0 / WHEEL_STEP_CAP=0.08 / WHEEL_DELTA_DIVISOR=1000', () => {
    expect(INTERACTION_SCALE_MIN).toBe(0.1);
    expect(INTERACTION_SCALE_MAX).toBe(5.0);
    expect(WHEEL_STEP_CAP).toBe(0.08);
    expect(WHEEL_DELTA_DIVISOR).toBe(1000);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// M6-A4  serialize/parse roundtrip；损坏/缺字段/version≠1/非有限 → null；
//        storageKeyForModel 前缀 + url 区隔
// ─────────────────────────────────────────────────────────────────────────────
describe('C-M6 serialize/parse/storageKeyForModel（M6-A4，contract-C-M6.md）', () => {
  it('M6-A4a: serialize→parse roundtrip 返回等价 OverrideState', () => {
    const o = validOverride({ scale: 2.5, x: -10, y: 300 });
    const parsed = parseOverride(serializeOverride(o));
    expect(parsed).not.toBeNull();
    expect(parsed!.scale).toBeCloseTo(2.5, 5);
    expect(parsed!.x).toBeCloseTo(-10, 5);
    expect(parsed!.y).toBeCloseTo(300, 5);
    expect(parsed!.version).toBe(1);
  });

  it('M6-A4b: serialize 输出合法 JSON（可被 JSON.parse）', () => {
    const s = serializeOverride(validOverride());
    expect(() => JSON.parse(s)).not.toThrow();
    const obj = JSON.parse(s);
    expect(obj).toMatchObject({ scale: 2, x: 100, y: 50, version: 1 });
  });

  it('M6-A4c: parse(null) → null（无持久化记录走相机模式）', () => {
    expect(parseOverride(null)).toBeNull();
  });

  it('M6-A4d: parse(损坏 JSON) → null', () => {
    expect(parseOverride('not-a-json')).toBeNull();
    expect(parseOverride('{broken')).toBeNull();
  });

  it('M6-A4e: parse(缺字段) → null（缺 version 或缺 scale/x/y 任一）', () => {
    expect(parseOverride('{"scale":2,"x":100,"y":50}')).toBeNull();          // 缺 version
    expect(parseOverride('{"scale":2,"x":100,"version":1}')).toBeNull();     // 缺 y
    expect(parseOverride('{"scale":2,"y":50,"version":1}')).toBeNull();      // 缺 x
    expect(parseOverride('{"x":100,"y":50,"version":1}')).toBeNull();        // 缺 scale
  });

  it('M6-A4f: parse(version≠1) → null', () => {
    const j = JSON.stringify({ scale: 2, x: 100, y: 50, version: 2 });
    expect(parseOverride(j)).toBeNull();
  });

  it('M6-A4g: parse(非有限 scale/x/y) → null', () => {
    expect(parseOverride(JSON.stringify(validOverride({ scale: NaN })))).toBeNull();
    expect(parseOverride(JSON.stringify(validOverride({ x: Infinity })))).toBeNull();
    expect(parseOverride(JSON.stringify(validOverride({ y: -Infinity })))).toBeNull();
  });

  it('M6-A4h: storageKeyForModel 前缀 "live2dai.interaction." 且按 url 区分', () => {
    const k = storageKeyForModel('/some/url.model3.json');
    expect(k.startsWith('live2dai.interaction.')).toBe(true);
  });

  it('M6-A4i: storageKeyForModel 不同 url → 不同 key（每模型独立）', () => {
    const k1 = storageKeyForModel('/model/a.model3.json');
    const k2 = storageKeyForModel('/model/b.model3.json');
    expect(k1).not.toBe(k2);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// M6-A5  createInteractionStore（注入内存 fake storage）：
//        无 key→null / save→load roundtrip / clear→null / 每模型独立
// ─────────────────────────────────────────────────────────────────────────────
describe('C-M6 createInteractionStore（M6-A5，contract-C-M6.md，注入内存 storage）', () => {
  let storage: ReturnType<typeof makeMemoryStorage>;
  let store: ReturnType<typeof createInteractionStore>;

  beforeEach(() => {
    storage = makeMemoryStorage();
    store = createInteractionStore(storage);
  });

  it('M6-A5a: 无该 key 记录时 load → null', () => {
    expect(store.load('/model/a.model3.json')).toBeNull();
  });

  it('M6-A5b: save→load roundtrip 返回等价 OverrideState', () => {
    const o = validOverride({ scale: 3, x: -200, y: 120 });
    store.save('/model/a.model3.json', o);
    const loaded = store.load('/model/a.model3.json');
    expect(loaded).not.toBeNull();
    expect(loaded!.scale).toBe(3);
    expect(loaded!.x).toBe(-200);
    expect(loaded!.y).toBe(120);
    expect(loaded!.version).toBe(1);
  });

  it('M6-A5c: clear → load 返回 null（重置入口语义）', () => {
    store.save('/model/a.model3.json', validOverride());
    expect(store.load('/model/a.model3.json')).not.toBeNull();
    store.clear('/model/a.model3.json');
    expect(store.load('/model/a.model3.json')).toBeNull();
  });

  it('M6-A5d: 每模型独立 —— 不同模型 key 互不覆盖', () => {
    store.save('/model/a.model3.json', validOverride({ scale: 2, x: 1, y: 2 }));
    store.save('/model/b.model3.json', validOverride({ scale: 4, x: 10, y: 20 }));
    // a 不受 b 影响
    const a = store.load('/model/a.model3.json');
    expect(a!.scale).toBe(2);
    expect(a!.x).toBe(1);
    // 清空 a 不影响 b
    store.clear('/model/a.model3.json');
    expect(store.load('/model/a.model3.json')).toBeNull();
    expect(store.load('/model/b.model3.json')!.scale).toBe(4);
  });

  it('M6-A5e: save 以 storageKeyForModel 键 setItem、值为 serializeOverride；load 走 getItem', () => {
    const o = validOverride({ scale: 1.5, x: 0, y: 0 });
    store.save('/model/a.model3.json', o);
    const expectedKey = storageKeyForModel('/model/a.model3.json');
    expect(storage.setItem).toHaveBeenCalledWith(expectedKey, serializeOverride(o));

    // load 从 storage.getItem 读
    store.load('/model/a.model3.json');
    expect(storage.getItem).toHaveBeenCalledWith(expectedKey);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// M6-A6  shouldApplyOverride：null→false（走相机模式）；非 null→true
// ─────────────────────────────────────────────────────────────────────────────
describe('C-M6 shouldApplyOverride（M6-A6，contract-C-M6.md）', () => {
  it('M6-A6a: shouldApplyOverride(null) → false（无 override → C-M2 相机模式 fit 生效）', () => {
    expect(shouldApplyOverride(null)).toBe(false);
  });

  it('M6-A6b: shouldApplyOverride(有效 OverrideState) → true', () => {
    expect(shouldApplyOverride(validOverride())).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 契约边界说明（不对应单测，交由主代理/其他验收环节）
//   M6-A7 renderer 全量 vitest 绿 + shared 32/32  → 主代理统一回归跑
//   M6-A8 diff 范围审查（仅 interaction.ts + main.ts + 测试） → git diff 审查
//   M6-A9 浏览器 E2E（wheel/drag/localStorage/refresh/reset/聊天不回归） → 主代理 CDP
//   M6-A10 交互手感目视 → human_signoff（主代理归口 visual_signoff.json）
//   main.ts 事件接线（pointer/wheel/重置按钮）不进单测（env=node，无 DOM/事件），
//     契约 §5 风险段明示，靠 M6-A9 覆盖。
// ─────────────────────────────────────────────────────────────────────────────
