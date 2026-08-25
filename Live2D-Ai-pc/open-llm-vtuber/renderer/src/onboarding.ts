/**
 * onboarding.ts — 首次启动引导（P0-5）
 *
 * 目标（对齐项目哲学「动动手指即可配置」）：新用户第一次打开页面时，不应该面对一个
 * "角色不说话但也不告诉你为什么"的界面。这里给出一张**缺什么就说什么**的清单，
 * 每项一键跳到设置中心对应分区。
 *
 * 设计原则：
 *   - **不重复造表单**：引导只做"诊断 + 跳转"，真正的配置仍在设置中心完成
 *     （避免出现第二套 Key/模型输入逻辑，与 audit 的"别再堆重复实现"一致）；
 *   - **不挡路**：一切就绪时完全不出现；用户可"稍后再说"或"不再提示"（localStorage）；
 *   - 诊断结论只依赖 `GET /api/config` 的既有字段，不新增后端接口。
 */
import { icon } from './icons';

/* ------------------------------------------------------------------ *
 * 类型（GET /api/config 的子集）
 * ------------------------------------------------------------------ */

export interface OnboardingConfigView {
  llm_provider?: string;
  llm_configs?: Array<{ id: string; has_key?: boolean; model?: string; key_env?: string | null }>;
  tts_model?: string;
  tts_configs?: Array<{ id: string; has_key?: boolean; voice?: string }>;
  persona?: { system_prompt?: string } | null;
}

export type OnboardingTab = 'llm' | 'keys' | 'tts' | 'persona' | 'live2d';

export interface OnboardingStep {
  id: string;
  title: string;
  hint: string;
  /** 点击后跳转到设置中心的哪个分区 */
  tab: OnboardingTab;
  /** 是否已满足 */
  done: boolean;
  /** 未满足时是否阻塞"能聊天"这件事 */
  blocking: boolean;
}

export const ONBOARDING_STORAGE_KEY = 'live2dai.onboarding.dismissed';

/* ------------------------------------------------------------------ *
 * 纯逻辑：诊断
 * ------------------------------------------------------------------ */

/**
 * 根据配置视图算出引导清单。
 *
 * 判定口径（全部来自 `/api/config`）：
 *   1. 选了 LLM Provider 吗？（`llm_provider` 非空）——不选无法对话（阻塞）
 *   2. 该 Provider 有 Key 吗？（`llm_configs[].has_key`）——没有则第一句话就失败（阻塞）
 *   3. 选了 TTS 吗？（`tts_model` 非空）——没有则只有字幕没声音（不阻塞）
 *   4. 选中的 TTS 需要 Key 且没配？（不阻塞，Edge TTS 等本地引擎 has_key 恒真）
 *   5. 有人设吗？（`persona.system_prompt`）——没有也能聊（不阻塞）
 */
export function computeOnboardingSteps(config: OnboardingConfigView | null): OnboardingStep[] {
  const cfg = config || {};
  const llmId = (cfg.llm_provider || '').trim();
  const llm = (cfg.llm_configs || []).find((p) => p.id === llmId);
  const ttsId = (cfg.tts_model || '').trim();
  const tts = (cfg.tts_configs || []).find((p) => p.id === ttsId);
  const prompt = (cfg.persona?.system_prompt || '').trim();

  return [
    {
      id: 'llm-provider',
      title: '选择对话模型（LLM）',
      hint: llmId ? `当前：${llmId}` : '还没选 —— 角色需要一个大模型来思考',
      tab: 'llm',
      done: !!llmId,
      blocking: true,
    },
    {
      id: 'llm-key',
      title: '填写模型 API Key',
      hint: llm?.has_key
        ? `已配置${llm.key_env ? `（${llm.key_env}）` : ''}`
        : `还没填${llm?.key_env ? `（需要 ${llm.key_env}）` : ''} —— 本项目不含任何共享/免费 Key`,
      tab: 'keys',
      done: !!llm?.has_key,
      blocking: true,
    },
    {
      id: 'tts',
      title: '选择语音（TTS）',
      hint: ttsId
        ? tts?.has_key === false
          ? `${ttsId} 还缺 Key —— 会只有字幕没有声音`
          : `当前：${ttsId}${tts?.voice ? ` · ${tts.voice}` : ''}`
        : '还没选 —— 不选只会有字幕，没有声音',
      tab: 'tts',
      done: !!ttsId && tts?.has_key !== false,
      blocking: false,
    },
    {
      id: 'persona',
      title: '设定人设（可选）',
      hint: prompt ? `已设定（${prompt.length} 字）` : '还没写 —— 决定她的性格与说话方式',
      tab: 'persona',
      done: !!prompt,
      blocking: false,
    },
  ];
}

/** 是否需要弹引导：有任何**阻塞项**未完成才弹（否则别打扰用户）。 */
export function shouldShowOnboarding(steps: OnboardingStep[]): boolean {
  return steps.some((s) => s.blocking && !s.done);
}

/** 进度摘要文案。 */
export function onboardingSummary(steps: OnboardingStep[]): string {
  const done = steps.filter((s) => s.done).length;
  return `${done}/${steps.length} 项已就绪`;
}

/* ------------------------------------------------------------------ *
 * 引导浮层
 * ------------------------------------------------------------------ */

export interface OnboardingOverlay {
  /** 拉配置 → 判定 → 需要时显示（已"不再提示"则直接跳过） */
  maybeShow(): Promise<void>;
  hide(): void;
  destroy(): void;
}

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function safeStorage(): StorageLike {
  try {
    if (typeof window !== 'undefined' && window.localStorage) return window.localStorage;
  } catch {
    /* 隐私模式访问 localStorage 可能抛 SecurityError → 降级 */
  }
  return { getItem: () => null, setItem: () => undefined };
}

function escapeHtml(text: string): string {
  return String(text ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/**
 * 装配首启引导浮层。
 * @param opts.openSettings 打开设置中心（可带分区）——由 main.ts 传入 settingsPanel.open
 * @param opts.fetchConfig  可注入的配置读取（默认 `GET /api/config`），便于测试
 */
export function initOnboarding(
  container: HTMLElement,
  opts: {
    openSettings: (tab?: OnboardingTab) => void;
    fetchConfig?: () => Promise<OnboardingConfigView>;
    storage?: StorageLike;
  },
): OnboardingOverlay {
  const storage = opts.storage || safeStorage();
  const overlay = document.createElement('div');
  overlay.className = 'onboarding-overlay';
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-label', '首次使用引导');
  overlay.innerHTML = `
    <div class="onboarding-card">
      <div class="onboarding-head">
        ${icon('sparkle')}
        <div>
          <div class="onboarding-title">先配三件事，白就能开口了</div>
          <div class="onboarding-sub" data-obsummary></div>
        </div>
      </div>
      <div class="onboarding-steps" data-obsteps></div>
      <div class="onboarding-foot">
        <label class="onboarding-never">
          <input type="checkbox" class="onboarding-never-input" />
          <span>不再提示</span>
        </label>
        <div class="onboarding-foot-actions">
          <button class="btn ghost onboarding-later" type="button">稍后再说</button>
          <button class="btn primary onboarding-start" type="button">打开设置中心</button>
        </div>
      </div>
    </div>
  `;
  container.appendChild(overlay);

  const stepsEl = overlay.querySelector<HTMLElement>('[data-obsteps]')!;
  const summaryEl = overlay.querySelector<HTMLElement>('[data-obsummary]')!;
  const neverInput = overlay.querySelector<HTMLInputElement>('.onboarding-never-input')!;

  const hide = (): void => {
    if (neverInput.checked) storage.setItem(ONBOARDING_STORAGE_KEY, '1');
    overlay.classList.remove('open');
  };

  const render = (steps: OnboardingStep[]): void => {
    summaryEl.textContent = onboardingSummary(steps);
    stepsEl.innerHTML = '';
    for (const step of steps) {
      const row = document.createElement('button');
      row.type = 'button';
      row.className = `onboarding-step${step.done ? ' done' : ''}${
        !step.done && step.blocking ? ' blocking' : ''
      }`;
      row.innerHTML = `
        <span class="onboarding-step-mark">${step.done ? icon('check') : ''}</span>
        <span class="onboarding-step-body">
          <span class="onboarding-step-title">${escapeHtml(step.title)}</span>
          <span class="onboarding-step-hint">${escapeHtml(step.hint)}</span>
        </span>
        ${step.done ? '' : '<span class="onboarding-step-go">去设置 →</span>'}
      `;
      row.addEventListener('click', () => {
        hide();
        opts.openSettings(step.tab);
      });
      stepsEl.appendChild(row);
    }
  };

  overlay.querySelector<HTMLButtonElement>('.onboarding-later')!.addEventListener('click', hide);
  overlay.querySelector<HTMLButtonElement>('.onboarding-start')!.addEventListener('click', () => {
    hide();
    opts.openSettings('keys');
  });
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) hide();
  });

  const loadConfig =
    opts.fetchConfig ||
    (async (): Promise<OnboardingConfigView> => {
      const res = await fetch('/api/config');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return (await res.json()) as OnboardingConfigView;
    });

  return {
    async maybeShow() {
      if (storage.getItem(ONBOARDING_STORAGE_KEY) === '1') return;
      try {
        const steps = computeOnboardingSteps(await loadConfig());
        if (!shouldShowOnboarding(steps)) return;
        render(steps);
        overlay.classList.add('open');
      } catch (err) {
        // 读配置失败不该弹一个空引导（可能只是后端还没起来）
        console.warn('[onboarding] 配置读取失败，跳过引导:', err);
      }
    },
    hide,
    destroy() {
      overlay.remove();
    },
  };
}
