/**
 * voice-library.ts — 声音克隆库纯逻辑模块（无 DOM，可直接 vitest import）
 *
 * 后端契约：
 *   GET    /api/voice/list
 *   POST   /api/voice/upload   multipart(file, voice_name?)
 *   POST   /api/voice/select   {voice_id}
 *   POST   /api/voice/apply    {voice_id}
 *   POST   /api/voice/delete   {voice_id}
 */

export interface VoiceProfile {
  id: string;
  name: string;
  file: string;
  ext: string;
  bytes: number;
  created_at: number;
  ref_audio_path: string;
  active: boolean;
}

export interface VoiceApplyResult {
  ok: boolean;
  applied: boolean;
  requires_restart: boolean;
  message: string;
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

export async function listVoices(): Promise<VoiceProfile[]> {
  const res = await fetch('/api/voice/list');
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { voices?: VoiceProfile[]; error?: string };
  if (data.error) throw new Error(String(data.error));
  return Array.isArray(data.voices) ? data.voices : [];
}

export async function uploadVoice(file: File, voiceName?: string): Promise<VoiceProfile> {
  const form = new FormData();
  form.append('file', file);
  if (voiceName) form.append('voice_name', voiceName);
  const res = await fetch('/api/voice/upload', { method: 'POST', body: form });
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { voice?: VoiceProfile; error?: string };
  if (data.error) throw new Error(String(data.error));
  if (!data.voice) throw new Error('后端未返回 voice');
  return data.voice;
}

export async function selectVoice(voiceId: string): Promise<VoiceProfile> {
  const res = await fetch('/api/voice/select', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ voice_id: voiceId }),
  });
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { voice?: VoiceProfile; error?: string };
  if (data.error) throw new Error(String(data.error));
  if (!data.voice) throw new Error('后端未返回 voice');
  return data.voice;
}

export async function applyVoice(voiceId: string): Promise<VoiceApplyResult> {
  const res = await fetch('/api/voice/apply', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ voice_id: voiceId }),
  });
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as Partial<VoiceApplyResult> & { error?: string };
  if (data.error) throw new Error(String(data.error));
  return {
    ok: data.ok ?? true,
    applied: Boolean(data.applied),
    requires_restart: Boolean(data.requires_restart),
    message: String(data.message ?? '已应用'),
  };
}

export async function deleteVoice(voiceId: string): Promise<void> {
  const res = await fetch('/api/voice/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ voice_id: voiceId }),
  });
  if (!res.ok) throw new Error(await readError(res));
  const data = (await res.json()) as { error?: string };
  if (data.error) throw new Error(String(data.error));
}
