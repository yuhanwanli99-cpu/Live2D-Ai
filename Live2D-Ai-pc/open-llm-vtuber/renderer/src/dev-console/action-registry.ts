/** Developer-mode action extension registry.
 *
 * The backend owns the canonical allowlist; this frontend registry mirrors it
 * and provides an extension point for future easter-egg actions without
 * hardcoding UI changes. Unknown IDs are rejected by both sides.
 */

export interface DevActionMeta {
  /** 技术动作 ID（与后端白名单对齐，如 nod / shake_no）。 */
  label: string;
  /** 中文显示名（如 点头）。 */
  name: string;
  /** 简短中文说明（如 轻轻点头示意）。 */
  description: string;
  /** 'gesture' = single-pose body motion; 'choreo' = timeline choreography. */
  kind: 'gesture' | 'choreo';
}

/** 每个动作的中文名称 + 简短说明（用于开发者面板可视化，避免只显示英文 ID）。 */
const ACTION_ZH: Record<string, { name: string; description: string }> = {
  nod: { name: '点头', description: '轻轻点头示意' },
  shake_no: { name: '摇头', description: '摇头否认或拒绝' },
  tilt: { name: '歪头', description: '歪头好奇/思考' },
  look_around: { name: '左右张望', description: '左右张望观察四周' },
  bow: { name: '鞠躬', description: '鞠躬行礼' },
  lean_in: { name: '靠近', description: '凑近/前倾靠近' },
  wave_big: { name: '挥手', description: '挥手打招呼' },
  bounce_happy: { name: '蹦跳', description: '开心蹦跳' },
  happy: { name: '开心', description: '开心雀跃' },
  excited: { name: '兴奋', description: '兴奋激动' },
  sad: { name: '难过', description: '低沉难过' },
  anger: { name: '生气', description: '不悦生气' },
  surprise: { name: '惊讶', description: '惊讶' },
  fear: { name: '害怕', description: '害怕退缩' },
  disgust: { name: '嫌弃', description: '嫌弃' },
  neutral: { name: '平静', description: '平静自然' },
  breathe: { name: '呼吸', description: '轻微呼吸起伏' },
  idle: { name: '待机', description: '待机小动作' },
  sway: { name: '摇摆', description: '轻轻摇摆' },
  listen: { name: '倾听', description: '专注倾听' },
};

function zhMeta(id: string): { name: string; description: string } {
  return ACTION_ZH[id] ?? { name: id, description: '开发者动作' };
}

export class DevActionRegistry {
  private readonly actions = new Map<string, DevActionMeta>();

  constructor(initial?: Record<string, DevActionMeta>) {
    if (initial) {
      for (const [id, meta] of Object.entries(initial)) {
        this.register(id, meta);
      }
    }
  }

  register(id: string, meta: DevActionMeta): void {
    const key = id.trim().toLowerCase();
    if (!key) return;
    const zh = zhMeta(key);
    this.actions.set(key, { ...meta, name: meta.name ?? zh.name, description: meta.description ?? zh.description });
  }

  unregister(id: string): void {
    this.actions.delete(id.trim().toLowerCase());
  }

  isAllowed(id: string): boolean {
    return this.actions.has(id.trim().toLowerCase());
  }

  list(): Array<{ id: string } & DevActionMeta> {
    return Array.from(this.actions.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([id, meta]) => ({ id, ...meta }));
  }

  /** Replace the entire catalog (e.g. after fetching from /api/dev/actions/catalog). */
  loadCatalog(ids: string[]): void {
    this.actions.clear();
    for (const id of ids) {
      const key = id.trim().toLowerCase();
      if (!key) continue;
      const kind =
        key === 'wave_big' ||
        key === 'bow' ||
        key === 'bounce_happy' ||
        key === 'shake_no' ||
        key === 'lean_in' ||
        key === 'look_around'
          ? 'choreo'
          : 'gesture';
      const zh = zhMeta(key);
      this.register(key, { label: key, name: zh.name, description: zh.description, kind });
    }
  }
}

/** Shared registry instance used by the dev console. */
export const devActionRegistry = new DevActionRegistry();
