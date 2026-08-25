/**
 * onboarding.test.ts — 首次启动引导纯逻辑单测（P0-5）
 *
 * 关键约定：
 *   - 只有**阻塞项**（选 Provider / 填 Key）未完成才打扰用户；
 *   - TTS/人设缺失不阻塞（只有字幕也能用）；
 *   - 判定只依赖 `GET /api/config` 既有字段，不引入新后端接口。
 */
import { describe, expect, it } from 'vitest';

import {
  computeOnboardingSteps,
  onboardingSummary,
  shouldShowOnboarding,
  type OnboardingConfigView,
} from '../src/onboarding';

const ready: OnboardingConfigView = {
  llm_provider: 'deepseek_llm',
  llm_configs: [{ id: 'deepseek_llm', has_key: true, key_env: 'DEEPSEEK_API_KEY' }],
  tts_model: 'edge_tts',
  tts_configs: [{ id: 'edge_tts', has_key: true, voice: 'zh-CN-XiaoxiaoNeural' }],
  persona: { system_prompt: '你是白，一个活泼的虚拟伙伴。' },
};

describe('computeOnboardingSteps', () => {
  it('全部就绪 → 四项都 done', () => {
    const steps = computeOnboardingSteps(ready);
    expect(steps).toHaveLength(4);
    expect(steps.every((s) => s.done)).toBe(true);
  });

  it('没选 Provider → 阻塞项未完成', () => {
    const steps = computeOnboardingSteps({ ...ready, llm_provider: '' });
    const step = steps.find((s) => s.id === 'llm-provider')!;
    expect(step.done).toBe(false);
    expect(step.blocking).toBe(true);
  });

  it('选了 Provider 但没 Key → Key 项未完成，并点名环境变量', () => {
    const steps = computeOnboardingSteps({
      ...ready,
      llm_configs: [{ id: 'deepseek_llm', has_key: false, key_env: 'DEEPSEEK_API_KEY' }],
    });
    const step = steps.find((s) => s.id === 'llm-key')!;
    expect(step.done).toBe(false);
    expect(step.hint).toContain('DEEPSEEK_API_KEY');
    expect(step.hint).toContain('共享');  // 明确"本项目不含共享/免费 Key"
  });

  it('TTS 未选 → 未完成但不阻塞（只有字幕也能用）', () => {
    const steps = computeOnboardingSteps({ ...ready, tts_model: '' });
    const step = steps.find((s) => s.id === 'tts')!;
    expect(step.done).toBe(false);
    expect(step.blocking).toBe(false);
    expect(step.hint).toContain('字幕');
  });

  it('TTS 选了但缺 Key → 视为未完成并说明后果', () => {
    const steps = computeOnboardingSteps({
      ...ready,
      tts_model: 'cosyvoice_cloud',
      tts_configs: [{ id: 'cosyvoice_cloud', has_key: false }],
    });
    const step = steps.find((s) => s.id === 'tts')!;
    expect(step.done).toBe(false);
    expect(step.hint).toContain('缺 Key');
  });

  it('人设缺失 → 未完成但不阻塞', () => {
    const steps = computeOnboardingSteps({ ...ready, persona: { system_prompt: '  ' } });
    const step = steps.find((s) => s.id === 'persona')!;
    expect(step.done).toBe(false);
    expect(step.blocking).toBe(false);
  });

  it('每一项都带跳转分区（否则引导点了没反应）', () => {
    for (const step of computeOnboardingSteps(ready)) {
      expect(['llm', 'keys', 'tts', 'persona', 'live2d']).toContain(step.tab);
    }
  });

  it('null / 空配置不抛错，且判定为需要引导', () => {
    const steps = computeOnboardingSteps(null);
    expect(steps).toHaveLength(4);
    expect(shouldShowOnboarding(steps)).toBe(true);
  });
});

describe('shouldShowOnboarding —— 别打扰已配置好的用户', () => {
  it('全部就绪 → 不弹', () => {
    expect(shouldShowOnboarding(computeOnboardingSteps(ready))).toBe(false);
  });

  it('只缺 TTS / 人设（非阻塞）→ 也不弹', () => {
    expect(
      shouldShowOnboarding(
        computeOnboardingSteps({ ...ready, tts_model: '', persona: { system_prompt: '' } }),
      ),
    ).toBe(false);
  });

  it('缺 Key（阻塞）→ 弹', () => {
    expect(
      shouldShowOnboarding(
        computeOnboardingSteps({
          ...ready,
          llm_configs: [{ id: 'deepseek_llm', has_key: false }],
        }),
      ),
    ).toBe(true);
  });
});

describe('onboardingSummary', () => {
  it('给出 x/y 已就绪', () => {
    expect(onboardingSummary(computeOnboardingSteps(ready))).toBe('4/4 项已就绪');
  });

  it('没选 Provider 时 Key 项也必然未完成（级联），只剩人设达标', () => {
    const steps = computeOnboardingSteps({ ...ready, llm_provider: '', tts_model: '' });
    expect(steps.find((s) => s.id === 'llm-key')!.done).toBe(false);
    expect(onboardingSummary(steps)).toBe('1/4 项已就绪');
  });
});
