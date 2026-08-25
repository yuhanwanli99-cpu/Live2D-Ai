/**
 * dev-console-logic.test.ts — 开发者控制台纯逻辑（node 环境可跑）
 *
 * 覆盖：
 *  - catalogEntries：目录 ids → 动作条目（去重/排序/手势 vs 编排分类）；
 *  - coerceStrength：任意输入 → 强度 1|2|3（非法回落 2）；
 *  - action-registry 集成：loadCatalog 与 list 往返。
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { catalogEntries, coerceStrength } from '../src/dev-console/dev-console';
import { devActionRegistry, DevActionRegistry } from '../src/dev-console/action-registry';

describe('catalogEntries — 纯分类', () => {
  it('编排动作标 choreo，其余标 gesture，并按 id 排序', () => {
    const entries = catalogEntries(['nod', 'wave_big', 'bow', 'idle', 'look_around']);
    expect(entries).toEqual([
      { id: 'bow', kind: 'choreo' },
      { id: 'idle', kind: 'gesture' },
      { id: 'look_around', kind: 'choreo' },
      { id: 'nod', kind: 'gesture' },
      { id: 'wave_big', kind: 'choreo' },
    ]);
  });

  it('空白/重复 id 被过滤；大小写归一', () => {
    const entries = catalogEntries(['Nod', 'nod', '', '   ', 'BOW']);
    expect(entries).toEqual([
      { id: 'bow', kind: 'choreo' },
      { id: 'nod', kind: 'gesture' },
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

describe('DevActionRegistry — 集成', () => {
  beforeEach(() => {
    devActionRegistry.loadCatalog([]);
  });

  it('loadCatalog 往返：未知 id 拒绝，已知可触发', () => {
    const reg = new DevActionRegistry();
    reg.loadCatalog(['nod', 'bow', 'wave_big']);
    expect(reg.isAllowed('nod')).toBe(true);
    expect(reg.isAllowed('bow')).toBe(true);
    expect(reg.isAllowed('unknown_xyz')).toBe(false);
    const list = reg.list();
    expect(list.map((e) => e.id)).toEqual(['bow', 'nod', 'wave_big']);
    expect(list.find((e) => e.id === 'bow')?.kind).toBe('choreo');
    expect(list.find((e) => e.id === 'nod')?.kind).toBe('gesture');
  });
});