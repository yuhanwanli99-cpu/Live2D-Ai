/**
 * model-upload.ts — Live2D 模型 zip 上传纯逻辑模块（不依赖 DOM，可直接 vitest import）
 *
 * 后端契约：POST /api/models/upload，multipart FormData。
 *   字段：file（zip）、可选 model_id / display_name / overwrite。
 *   成功：{"ok":true,"model_id":"...","model3_json_path":"...","applied":bool}
 *   失败：非 2xx，body 含 {"error":"..."}。
 */

export interface ModelUploadResult {
  ok: boolean;
  model_id: string;
  model3_json_path: string;
  applied: boolean;
}

export interface ModelUploadOptions {
  modelId?: string;
  displayName?: string;
  overwrite?: boolean;
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

/** 上传 Live2D 模型 zip（上传后由后端解包/校验）。 */
export async function uploadModelZip(
  file: File,
  opts: ModelUploadOptions = {},
): Promise<ModelUploadResult> {
  const form = new FormData();
  form.append('file', file);
  if (opts.modelId) form.append('model_id', opts.modelId);
  if (opts.displayName) form.append('display_name', opts.displayName);
  if (opts.overwrite) form.append('overwrite', 'true');

  const res = await fetch('/api/models/upload', { method: 'POST', body: form });
  if (!res.ok) throw new Error(await readError(res));

  const data = (await res.json()) as Partial<ModelUploadResult> & { error?: string };
  if (data.error) throw new Error(String(data.error));
  return {
    ok: data.ok ?? true,
    model_id: String(data.model_id ?? ''),
    model3_json_path: String(data.model3_json_path ?? ''),
    applied: Boolean(data.applied),
  };
}
