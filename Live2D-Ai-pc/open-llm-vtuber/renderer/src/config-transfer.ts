/**
 * config-transfer.ts — 配置导入/导出纯逻辑模块（不依赖 DOM，可直接 vitest import）
 *
 * 后端契约：
 *   GET  /api/config/export → {"ok":true,"format":"live2dai-config","version":1,
 *                               "conf_yaml":"...","persona":{...}}
 *   POST /api/config/import  body {"conf_yaml":"...","persona":{...}}
 *                           → 成功 {"ok":true,"persisted":true,"message":"..."}；失败 4xx 带 {"error":"..."}。
 */

export interface ConfigExportResult {
  ok: boolean;
  format: string;
  version: number;
  conf_yaml: string;
  persona: Record<string, unknown>;
}

export interface ConfigImportResult {
  ok: boolean;
  persisted: boolean;
  message: string;
}

/** 解析后端错误文案；失败时回退到 HTTP 状态码。 */
async function readError(res: Response): Promise<string> {
  try {
    const data = (await res.json()) as { error?: unknown };
    if (data.error) return String(data.error);
  } catch {
    // body 不是 JSON 时忽略
  }
  return `HTTP ${res.status}`;
}

/** 导出当前配置（conf_yaml + persona）。 */
export async function exportConfig(): Promise<ConfigExportResult> {
  const res = await fetch('/api/config/export');
  if (!res.ok) throw new Error(await readError(res));

  const data = (await res.json()) as Partial<ConfigExportResult> & { error?: string };
  if (data.error) throw new Error(String(data.error));
  return {
    ok: data.ok ?? true,
    format: String(data.format ?? 'live2dai-config'),
    version: Number(data.version ?? 1),
    conf_yaml: String(data.conf_yaml ?? ''),
    persona: (data.persona ?? {}) as Record<string, unknown>,
  };
}

/** 读取配置 JSON 文件并导入（读文本 → JSON.parse → POST）。 */
export async function importConfigFile(file: File): Promise<ConfigImportResult> {
  const text = await file.text();
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error('配置 JSON 解析失败');
  }

  const body = parsed as { conf_yaml?: unknown; persona?: unknown };
  const res = await fetch('/api/config/import', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      conf_yaml: typeof body.conf_yaml === 'string' ? body.conf_yaml : '',
      persona: body.persona ?? {},
    }),
  });
  if (!res.ok) throw new Error(await readError(res));

  const data = (await res.json()) as Partial<ConfigImportResult> & { error?: string };
  if (data.error) throw new Error(String(data.error));
  return {
    ok: data.ok ?? true,
    persisted: Boolean(data.persisted),
    message: String(data.message ?? '导入完成'),
  };
}
