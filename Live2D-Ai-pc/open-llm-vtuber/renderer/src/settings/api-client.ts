// API client for the settings panel.
// All direct fetch() calls live here so settings-ui.ts can focus on UI logic.

export async function getConfig(): Promise<Response> {
  return fetch('/api/config');
}

export async function getConfigTree(): Promise<Response> {
  return fetch('/api/config/tree');
}

export async function saveConfig(payload: Record<string, unknown>): Promise<Response> {
  return fetch('/api/config', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
}

export async function saveConfigTree(yaml: string): Promise<Response> {
  return fetch('/api/config/tree', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ yaml }),
  });
}

export async function discoverLlmModels(body: Record<string, unknown>): Promise<Response> {
  return fetch('/api/llm/discover-models', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}

export async function testLlm(body: Record<string, unknown>): Promise<Response> {
  return fetch('/api/test-llm', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}

export async function testTts(body: Record<string, unknown>): Promise<Response> {
  return fetch('/api/test-tts', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}

export async function checkProviders(body: Record<string, unknown>): Promise<Response> {
  return fetch('/api/providers/check', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}

export async function getModels(): Promise<Response> {
  return fetch('/api/models');
}

export async function scanModels(): Promise<Response> {
  return fetch('/api/models/scan', { method: 'POST' });
}

export async function selectModel(modelId: string): Promise<Response> {
  return fetch('/api/models/select', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model_id: modelId }),
  });
}

export async function getFrontendVersion(): Promise<Response> {
  return fetch('/api/frontend-version');
}

export async function getVersionInfo(): Promise<Response> {
  return fetch('/api/version');
}

// Developer-mode endpoints (localhost-only, disabled by default on the server).

export async function getDevCatalog(): Promise<Response> {
  return fetch('/api/dev/actions/catalog');
}

export async function triggerDevAction(
  action: string,
  opts: { strength?: 1 | 2 | 3; release?: boolean } = {},
): Promise<Response> {
  return fetch('/api/dev/actions/trigger', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      action,
      strength: opts.strength ?? 2,
      release: opts.release ?? false,
    }),
  });
}

export async function analyzeDevDirector(text: string): Promise<Response> {
  return fetch('/api/dev/director/analyze', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ text }),
  });
}
