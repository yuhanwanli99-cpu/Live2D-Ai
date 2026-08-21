/**
 * memory-api.ts — 记忆 / 知识库 / 联网查询管理请求模块（无 DOM，可直接 vitest import）
 *
 * 后端契约（plugin_routes.py）：
 *   GET    /api/memory/facts             → { ok: true, facts: string[] }
 *   POST   /api/memory/facts             → { ok: true, facts: string[] }   body { fact }
 *   DELETE /api/memory/facts/{idx}       → { ok: true, facts: string[] }
 *   DELETE /api/memory/facts             → { ok: true, cleared: number }
 *   GET    /api/knowledge/documents      → { ok: true, documents: KnowledgeDoc[] }
 *   POST   /api/knowledge/documents      → { ok: true, name }              body { name, content }
 *   DELETE /api/knowledge/documents/{n}  → { ok: true }
 *   POST   /api/tools/web-search         → { ok, results, provider, error } body { query }
 *
 * 说明：只负责请求与错误归一，不做 UI；渲染在 settings-ui.ts。
 */

/** 知识库文档条目（GET /api/knowledge/documents 的元素） */
export interface KnowledgeDoc {
  name: string;
  size?: number;
  updated?: number;
}

/** 联网查询单条结果 */
export interface WebSearchResult {
  title?: string;
  url?: string;
  snippet?: string;
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

function asFacts(data: { facts?: unknown; error?: string }): string[] {
  if (data.error) throw new Error(String(data.error));
  return Array.isArray(data.facts) ? data.facts.map((f) => String(f)) : [];
}

/** 列出全部长期记忆事实 */
export async function listMemoryFacts(): Promise<string[]> {
  const res = await fetch('/api/memory/facts');
  if (!res.ok) throw new Error(await readError(res));
  return asFacts(await res.json());
}

/** 手动添加一条事实（后端去重；空串由调用方拦截） */
export async function addMemoryFact(fact: string): Promise<string[]> {
  const res = await fetch('/api/memory/facts', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ fact }),
  });
  if (!res.ok) throw new Error(await readError(res));
  return asFacts(await res.json());
}

/** 按下标删除一条事实 */
export async function deleteMemoryFact(index: number): Promise<string[]> {
  const res = await fetch(`/api/memory/facts/${encodeURIComponent(String(index))}`, {
    method: 'DELETE',
  });
  if (!res.ok) throw new Error(await readError(res));
  return asFacts(await res.json());
}

/** 清空全部事实，返回清除条数 */
export async function clearMemoryFacts(): Promise<number> {
  const res = await fetch('/api/memory/facts', { method: 'DELETE' });
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { cleared?: number; error?: string };
  if (data.error) throw new Error(String(data.error));
  return typeof data.cleared === 'number' ? data.cleared : 0;
}

/* ---------------- 知识库 ---------------- */

export async function listKnowledgeDocs(): Promise<KnowledgeDoc[]> {
  const res = await fetch('/api/knowledge/documents');
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { documents?: KnowledgeDoc[]; error?: string };
  if (data.error) throw new Error(String(data.error));
  return Array.isArray(data.documents) ? data.documents : [];
}

export async function addKnowledgeDoc(name: string, content: string): Promise<string> {
  const res = await fetch('/api/knowledge/documents', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, content }),
  });
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { name?: string; error?: string };
  if (data.error) throw new Error(String(data.error));
  return String(data.name || name);
}

export async function deleteKnowledgeDoc(name: string): Promise<void> {
  const res = await fetch(`/api/knowledge/documents/${encodeURIComponent(name)}`, {
    method: 'DELETE',
  });
  if (!res.ok) throw new Error(await readError(res));
}

/* ---------------- 联网查询 ---------------- */

export interface WebSearchResponse {
  results: WebSearchResult[];
  provider?: string;
  error?: string | null;
}

export async function webSearch(query: string): Promise<WebSearchResponse> {
  const res = await fetch('/api/tools/web-search', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ query }),
  });
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as WebSearchResponse & { error?: string };
  return {
    results: Array.isArray(data.results) ? data.results : [],
    provider: data.provider,
    error: data.error ?? null,
  };
}
