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
    // nod / tilt 现为半身编排动作（CHOREOGRAPHY_LIBRARY），标 choreo；
    // idle 为 release、sway/happy 等为单姿态手势。wave_big/bounce_happy 不在
    // PC Preview 安全目录（bai 无手臂/全身能力），不应出现在该分类用例中。
    const entries = catalogEntries(['nod', 'bow', 'idle', 'look_around', 'sway', 'happy']);
    expect(entries).toEqual([
      { id: 'bow', kind: 'choreo' },
      { id: 'happy', kind: 'gesture' },
      { id: 'idle', kind: 'gesture' },
      { id: 'look_around', kind: 'choreo' },
      { id: 'nod', kind: 'choreo' },
      { id: 'sway', kind: 'gesture' },
    ]);
  });

  it('空白/重复 id 被过滤；大小写归一', () => {
    const entries = catalogEntries(['Nod', 'nod', '', '   ', 'BOW']);
    expect(entries).toEqual([
      { id: 'bow', kind: 'choreo' },
      { id: 'nod', kind: 'choreo' },
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

  it('loadCatalog 往返：未知 id 拒绝，已知可触发；no bump/bounce in safe set', () => {
    const reg = new DevActionRegistry();
    reg.loadCatalog(['nod', 'bow', 'shake_no']);
    expect(reg.isAllowed('nod')).toBe(true);
    expect(reg.isAllowed('bow')).toBe(true);
    expect(reg.isAllowed('shake_no')).toBe(true);
    expect(reg.isAllowed('wave_big')).toBe(false); // 已从安全目录移除
    expect(reg.isAllowed('bounce_happy')).toBe(false); // 已从安全目录移除
    expect(reg.isAllowed('unknown_xyz')).toBe(false);
    const list = reg.list();
    expect(list.map((e) => e.id)).toEqual(['bow', 'nod', 'shake_no']);
    expect(list.find((e) => e.id === 'bow')?.kind).toBe('choreo');
    expect(list.find((e) => e.id === 'nod')?.kind).toBe('choreo');
    expect(list.find((e) => e.id === 'shake_no')?.kind).toBe('choreo');
  });

  it('优先消费后端 action_meta 的中文 label/description（取代硬编码表）', () => {
    const reg = new DevActionRegistry();
    reg.loadCatalog(['happy', 'excited', 'sway'], {
      happy: { label: '开心', description: '上身开心地左右摆动（模型无手臂）' },
      excited: { label: '兴奋', description: '上身雀跃兴奋摆动（无鼓掌）' },
      sway: { label: '摇摆', description: '上身左右轻摆' },
    });
    const happy = reg.list().find((e) => e.id === 'happy')!;
    const excited = reg.list().find((e) => e.id === 'excited')!;
    expect(happy.name).toBe('开心');
    expect(happy.description).toContain('上身');
    expect(excited.name).toBe('兴奋');
    expect(excited.description).toContain('无鼓掌');
  });
});