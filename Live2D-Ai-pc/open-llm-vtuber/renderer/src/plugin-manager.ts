/**
 * plugin-manager.ts — 插件管理纯逻辑模块（无 DOM，可直接 vitest import）
 *
 * 后端契约：
 *   GET    /api/plugins                  → { ok: true, plugins: PluginInfo[] }
 *   POST   /api/plugins/{id}/enable      → { ok: true, plugin: PluginInfo }
 *   POST   /api/plugins/{id}/disable     → { ok: true, plugin: PluginInfo }
 *
 * 说明：只负责请求与错误归一，不做 UI；渲染在 settings-ui.ts。
 */

export type PluginOrigin = 'native' | 'external';
export type PluginStatus = 'ok' | 'disabled' | 'error' | 'gated' | 'core' | 'stub';
export type PluginImplementation = 'runtime' | 'gated' | 'core' | 'stub';

export interface PluginInfo {
  id: string;
  name: string;
  name_zh?: string;
  version: string;
  description: string;
  description_zh?: string;
  origin: PluginOrigin;
  enabled: boolean;
  status: PluginStatus;
  hook_points: string[];
  capabilities: string[];
  config_schema: Record<string, unknown>;
  implementation: PluginImplementation;
}

async function readError(res: Response): Promise<string> {
  try {
    const data = (await res.json()) as { error?: unknown };
    if (data.error) return String(data.error);
  } catch {
    // ignore non-json body
  }
  return `HTTP ${res.status}`;
}

export async function listPlugins(): Promise<PluginInfo[]> {
  const res = await fetch('/api/plugins');
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { plugins?: PluginInfo[]; error?: string };
  if (data.error) throw new Error(String(data.error));
  return Array.isArray(data.plugins) ? data.plugins : [];
}

export async function enablePlugin(id: string): Promise<PluginInfo> {
  return setPluginEnabled(id, true);
}

export async function disablePlugin(id: string): Promise<PluginInfo> {
  return setPluginEnabled(id, false);
}

async function setPluginEnabled(id: string, enabled: boolean): Promise<PluginInfo> {
  const action = enabled ? 'enable' : 'disable';
  const res = await fetch(`/api/plugins/${encodeURIComponent(id)}/${action}`, {
    method: 'POST',
  });
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { plugin?: PluginInfo; error?: string };
  if (data.error) throw new Error(String(data.error));
  if (!data.plugin) throw new Error('后端未返回 plugin');
  return data.plugin;
}
