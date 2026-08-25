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

/** 每个动作的中文名称 + 简短说明（用于开发者面板可视化，避免只显示英文 ID）。
 *  注意：后端 motion_catalog 是 label/description 的权威来源，前端优先消费
 *  `action_meta`；此处仅作离线/后端未返回元数据时的回退，且不含 wave_big/
 *  bounce_happy 等 bai 无对应能力的过时动作（真实语义由后端 action_meta 覆盖）。 */
const ACTION_ZH: Record<string, { name: string; description: string }> = {
  nod: { name: '点头', description: '轻轻点头示意' },
  shake_no: { name: '摇头', description: '头部左右摆动否认' },
  tilt: { name: '歪头', description: '歪头好奇/思考' },
  look_around: { name: '左右张望', description: '视线+头部环顾四周' },
  bow: { name: '鞠躬', description: '上身前倾低头行礼' },
  lean_in: { name: '靠近', description: '上身前倾靠近' },
  happy: { name: '开心', description: '上身开心地左右摆动' },
  excited: { name: '兴奋', description: '上身雀跃兴奋摆动' },
  sway: { name: '摇摆', description: '上身左右轻摆' },
  listen: { name: '倾听', description: '专注上身前倾倾听' },
  sad: { name: '难过', description: '低沉难过' },
  anger: { name: '生气', description: '不悦生气' },
  surprise: { name: '惊讶', description: '睁大眼、身体微微后仰' },
  fear: { name: '害怕', description: '害怕退缩' },
  disgust: { name: '嫌弃', description: '嫌弃' },
  neutral: { name: '平静', description: '平静自然' },
  breathe: { name: '呼吸', description: '轻微呼吸起伏' },
  idle: { name: '待机', description: '回到中性待机（仅释放）' },
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

  /** Replace the entire catalog (e.g. after fetching from /api/dev/actions/catalog).
   *  `ids` 是纯 id 列表；若后端同时返回富 `action_meta`（label/description），
   *  则优先消费它作为中文名/说明，不再依赖本文件过时的硬编码表。 */
  loadCatalog(ids: string[], meta?: Record<string, { label?: string; description?: string }>): void {
    this.actions.clear();
    for (const id of ids) {
      const key = id.trim().toLowerCase();
      if (!key) continue;
      const kind =
        key === 'bow' ||
        key === 'shake_no' ||
        key === 'lean_in' ||
        key === 'look_around' ||
        key === 'nod' ||
        key === 'tilt'
          ? 'choreo'
          : 'gesture';
      const zh = zhMeta(key);
      const m = meta?.[key];
      this.register(key, {
        label: key,
        name: (m?.label && m.label.trim() ? m.label : zh.name).trim(),
        description: (m?.description && m.description.trim() ? m.description : zh.description).trim(),
        kind,
      });
    }
  }
}

/** Shared registry instance used by the dev console. */
export const devActionRegistry = new DevActionRegistry();
