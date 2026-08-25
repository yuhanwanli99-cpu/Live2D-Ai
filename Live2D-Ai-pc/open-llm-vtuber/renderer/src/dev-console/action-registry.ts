/** Developer-mode action extension registry.
 *
 * The backend owns the canonical allowlist; this frontend registry mirrors it
 * and provides an extension point for future actions without hardcoding UI
 * changes. 六动作定版（nod/shake_no/tilt/look_around/listen/surprise + release）：
 * 已删除动作（happy/excited/sway/bow/lean_in/wave/bounce 等）即使后端目录仍下发，
 * 前端也一律拒绝注册——UI 不再出现身体类动作分类。未知 ID are rejected by both sides.
 */

import { SUPPORTED_ACTIONS } from '../choreography';

export interface DevActionMeta {
  /** 技术动作 ID（与后端白名单对齐，如 nod / shake_no）。 */
  label: string;
  /** 中文显示名（如 点头）。 */
  name: string;
  /** 简短中文说明（如 轻轻点头示意）。 */
  description: string;
  /** 六动作全部为 timeline choreography；'gesture' 仅保留给后端历史目录兼容。 */
  kind: 'gesture' | 'choreo';
}

/** 每个动作的中文名称 + 简短说明（用于开发者面板可视化，避免只显示英文 ID）。
 *  注意：后端 motion_catalog 是 label/description 的权威来源，前端优先消费
 *  `action_meta`；此处仅作离线/后端未返回元数据时的回退。只含六动作 + release，
 *  不含 bow/happy/sway/wave_big/bounce_happy 等已删除的身体类动作。 */
const ACTION_ZH: Record<string, { name: string; description: string }> = {
  nod: { name: '点头', description: '头部上下轻点两下' },
  shake_no: { name: '摇头', description: '头部左右摆动否认' },
  tilt: { name: '歪头', description: '歪头好奇/思考' },
  look_around: { name: '左右张望', description: '视线先行+头部随动的环顾' },
  listen: { name: '倾听', description: '微微侧头专注倾听' },
  surprise: { name: '惊讶', description: '睁大眼睛、挑眉、微张嘴' },
  idle: { name: '待机', description: '回到中性待机（仅释放）' },
};

/** 允许进入 UI 的动作 id：六动作 + release 语义的 idle */
function isUiAllowedAction(id: string): boolean {
  return SUPPORTED_ACTIONS.includes(id) || id === 'idle';
}

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
   *  则优先消费它作为中文名/说明。已删除动作（身体类 / 情绪 key）不注册——UI 只出现
   *  六动作 + release(idle)。 */
  loadCatalog(ids: string[], meta?: Record<string, { label?: string; description?: string }>): void {
    this.actions.clear();
    for (const id of ids) {
      const key = id.trim().toLowerCase();
      if (!key || !isUiAllowedAction(key)) continue;
      const kind = SUPPORTED_ACTIONS.includes(key) ? 'choreo' : 'gesture';
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
