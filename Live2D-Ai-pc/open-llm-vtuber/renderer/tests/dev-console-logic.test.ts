/**
 * dev-console-logic.test.ts — 开发者控制台纯逻辑（node 环境可跑）
 *
 * 覆盖：
 *  - catalogEntries：目录 ids → 动作条目（去重/排序/编排 vs 兼容分类）；
 *  - coerceStrength：任意输入 → 强度 1|2|3（非法回落 2）；
 *  - action-registry 集成：loadCatalog 只注册六动作 + idle（release）；
 *    happy/excited/sway/bow/lean_in/wave/bounce 等已删除动作被 UI 拒绝。
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { catalogEntries, coerceStrength } from '../src/dev-console/dev-console';
import { devActionRegistry, DevActionRegistry } from '../src/dev-console/action-registry';

describe('catalogEntries — 纯分类', () => {
  it('六动作标 choreo，idle 标 gesture，并按 id 排序', () => {
    const entries = catalogEntries(['nod', 'shake_no', 'tilt', 'look_around', 'listen', 'surprise', 'idle']);
    expect(entries).toEqual([
      { id: 'idle', kind: 'gesture' },
      { id: 'listen', kind: 'choreo' },
      { id: 'look_around', kind: 'choreo' },
      { id: 'nod', kind: 'choreo' },
      { id: 'shake_no', kind: 'choreo' },
      { id: 'surprise', kind: 'choreo' },
      { id: 'tilt', kind: 'choreo' },
    ]);
  });

  it('空白/重复 id 被过滤；大小写归一', () => {
    const entries = catalogEntries(['Nod', 'nod', '', '   ', 'TILT']);
    expect(entries).toEqual([
      { id: 'nod', kind: 'choreo' },
      { id: 'tilt', kind: 'choreo' },
    ]);
  });

  it('空列表返回空', () => {
    expect(catalogEntries([])).toEqual([]);
  });
});

describe('coerceStrength — 强度归一', () => {
  it('合法值 1/2/3 原样返回', () => {
    expect(coerceStrength(1)).toBe(1);
    expect(coerceStrength('2')).toBe(2);
    expect(coerceStrength(3)).toBe(3);
  });

  it('非法/缺失回落 2', () => {
    expect(coerceStrength(0)).toBe(2);
    expect(coerceStrength(4)).toBe(2);
    expect(coerceStrength('abc')).toBe(2);
    expect(coerceStrength(null)).toBe(2);
    expect(coerceStrength(undefined)).toBe(2);
    expect(coerceStrength(NaN)).toBe(2);
  });
});

describe('DevActionRegistry — 集成（UI 只出现六动作 + release）', () => {
  beforeEach(() => {
    devActionRegistry.loadCatalog([]);
  });

  it('loadCatalog 往返：六动作注册为 choreo；bow/happy/sway 等已删除动作被拒绝', () => {
    const reg = new DevActionRegistry();
    reg.loadCatalog(['nod', 'tilt', 'look_around', 'listen', 'surprise', 'shake_no']);
    for (const id of ['nod', 'tilt', 'look_around', 'listen', 'surprise', 'shake_no']) {
      expect(reg.isAllowed(id)).toBe(true);
      expect(reg.list().find((e) => e.id === id)?.kind).toBe('choreo');
    }
  });

  it('已删除的身体类动作不再进入 UI 分类（即使后端目录仍下发）', () => {
    const reg = new DevActionRegistry();
    reg.loadCatalog(['nod', 'bow', 'lean_in', 'happy', 'excited', 'sway', 'wave_big', 'bounce_happy', 'sad', 'anger']);
    expect(reg.isAllowed('nod')).toBe(true);
    for (const dead of ['bow', 'lean_in', 'happy', 'excited', 'sway', 'wave_big', 'bounce_happy', 'sad', 'anger']) {
      expect(reg.isAllowed(dead), `${dead} 应被拒绝`).toBe(false);
    }
    expect(reg.list().map((e) => e.id)).toEqual(['nod']);
    expect(reg.isAllowed('wave')).toBe(false);
    expect(reg.isAllowed('unknown_xyz')).toBe(false);
  });

  it('优先消费后端 action_meta 的中文 label/description（取代硬编码表）', () => {
    const reg = new DevActionRegistry();
    reg.loadCatalog(['nod', 'listen'], {
      nod: { label: '点头', description: '头部上下轻点两下示意' },
      listen: { label: '倾听', description: '微微侧头专注倾听' },
    });
    const nod = reg.list().find((e) => e.id === 'nod')!;
    const listen = reg.list().find((e) => e.id === 'listen')!;
    expect(nod.name).toBe('点头');
    expect(nod.description).toContain('轻点');
    expect(listen.name).toBe('倾听');
  });
});
