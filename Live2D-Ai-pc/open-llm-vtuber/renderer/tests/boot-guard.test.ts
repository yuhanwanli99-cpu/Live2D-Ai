/**
 * 启动引导防回归 — motion 相位钩子空引用守卫（2026-08-22 黑屏事故）
 *
 * 事故：createPipeline(null, null)（启动空转管线）时 internalModel 为 null，
 * 钩子注册写成 imEvents.on?.(...) —— a.on?.() 只防 on 缺失，不防 a 为 null，
 * 读 .on 直接 TypeError → initApp 在聊天面板构建前死亡 → 全黑屏零交互。
 *
 * 测试方式：静态断言（对齐 rm1-skeleton 惯例——不 import 实现本体，
 * 避免 DOM/WebGL 依赖），锁定「必须双保险可选链」这一具体形态。
 */
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, join } from 'node:path';

const RENDERER_ROOT = resolve(__dirname, '..');
const MAIN_TS = join(RENDERER_ROOT, 'src', 'main.ts');

const GUARDED = 'imEvents?.on?.(';

describe('启动防回归：afterMotionUpdate 钩子空引用守卫', () => {
  const source = readFileSync(MAIN_TS, 'utf-8');

  it('钩子注册使用 imEvents?.on?. 双保险可选链', () => {
    expect(
      source.includes(GUARDED),
      'imEvents 可能为 null（启动空转管线），必须 imEvents?.on?.(…)；' +
        '单层 .on?.() 在 a 为 null 时依旧抛 TypeError',
    ).toBe(true);
  });

  it('不存在未防护的 imEvents.on?.( 写法', () => {
    // 剔除已防护写法后，不允许再出现裸 imEvents.on 形态
    const stripped = source.split(GUARDED).join('');
    expect(stripped.includes('imEvents.on'), '发现未防护的 imEvents.on 访问').toBe(false);
  });
});
