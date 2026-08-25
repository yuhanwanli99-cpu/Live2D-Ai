/**
 * background.ts — 角色背景图：画布背景层 + 设置页管理区
 *
 * 链路：`POST /api/background/upload`（本机限定）→ 文件落在后端 `backgrounds/`
 *      → 静态目录 `/bg/<name>` → 前端把它铺在 `#app-background`（PixiJS 画布**之下**，
 *      画布 `backgroundAlpha: 0` 透明，所以换背景不影响模型渲染，也不用重启）。
 *
 * 纯函数（formatBytes / validateLocalFile / backgroundUrlFor / cssBackgroundValue）
 * 与 DOM 装配分离，便于 vitest 直接单测。
 */

/* ------------------------------------------------------------------ *
 * 类型（与后端 backgrounds.py 的返回同构）
 * ------------------------------------------------------------------ */

export interface BackgroundFile {
  name: string;
  url: string;
  size: number;
  mtime: number;
}

export interface BackgroundState {
  files: BackgroundFile[];
  current: string | null;
  current_url: string | null;
}

/** 与后端 backgrounds.py 保持一致的校验口径（前端先挡一次，省一次往返） */
export const ALLOWED_BACKGROUND_EXTENSIONS = ['.png', '.jpg', '.jpeg', '.webp', '.gif'] as const;
export const MAX_BACKGROUND_BYTES = 12 * 1024 * 1024;

export const EMPTY_BACKGROUND_STATE: BackgroundState = {
  files: [],
  current: null,
  current_url: null,
};

/* ------------------------------------------------------------------ *
 * 纯函数
 * ------------------------------------------------------------------ */

/** 人类可读体积（KB/MB，一位小数）。 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '—';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** 背景图静态地址（后端 /bg 挂载 backgrounds/ 目录）。 */
export function backgroundUrlFor(name: string | null | undefined): string | null {
  if (!name) return null;
  return `/bg/${encodeURIComponent(name)}`;
}

/** `background-image` 的 CSS 值；无背景 → 'none'（回到透明桌宠形态）。 */
export function cssBackgroundValue(url: string | null | undefined): string {
  return url ? `url("${url}")` : 'none';
}

/**
 * HTML 转义（缩略图卡片要把文件名塞进 innerHTML）。
 *
 * 为什么必须做：`backgrounds/` 目录里的文件**不一定都经过上传接口清洗** ——
 * 用户可以直接把文件拷进去，文件名里带 `<img onerror=...>` 也会被列表接口如实返回。
 */
export function escapeHtml(text: string): string {
  return String(text ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/** CSS `url()` 里用的转义（防止文件名里的引号/括号截断样式）。 */
export function escapeCssUrl(url: string): string {
  return String(url ?? '').replace(/["'()\\\s]/g, (ch) => `\\${ch}`);
}

/** 上传前的本地预检：返回错误文案，通过则 null。 */
export function validateLocalFile(file: { name: string; size: number }): string | null {
  const lower = (file.name || '').toLowerCase();
  const ok = ALLOWED_BACKGROUND_EXTENSIONS.some((ext) => lower.endsWith(ext));
  if (!ok) {
    return `只支持 ${ALLOWED_BACKGROUND_EXTENSIONS.map((e) => e.slice(1)).join(' / ')} 格式`;
  }
  if (!file.size) return '文件内容为空';
  if (file.size > MAX_BACKGROUND_BYTES) {
    return `文件过大（${formatBytes(file.size)}，上限 ${formatBytes(MAX_BACKGROUND_BYTES)}）`;
  }
  return null;
}

/* ------------------------------------------------------------------ *
 * 背景层（主画面）
 * ------------------------------------------------------------------ */

/**
 * 把背景图应用到背景层元素。
 * @param el `#app-background`
 * @param url `/bg/xxx.png` 或 null（透明无背景）
 */
export function applyBackgroundLayer(el: HTMLElement | null, url: string | null): void {
  if (!el) return;
  el.style.backgroundImage = cssBackgroundValue(url);
  el.classList.toggle('has-image', !!url);
}

/* ------------------------------------------------------------------ *
 * API 客户端
 * ------------------------------------------------------------------ */

async function readJson(res: Response): Promise<Record<string, unknown>> {
  try {
    return (await res.json()) as Record<string, unknown>;
  } catch {
    return {};
  }
}

function asState(data: Record<string, unknown>): BackgroundState {
  return {
    files: Array.isArray(data.files) ? (data.files as BackgroundFile[]) : [],
    current: typeof data.current === 'string' ? data.current : null,
    current_url: typeof data.current_url === 'string' ? data.current_url : null,
  };
}

/** 读取背景列表 + 当前选中。 */
export async function fetchBackgroundState(): Promise<BackgroundState> {
  const res = await fetch('/api/background/list');
  const data = await readJson(res);
  if (!res.ok || data.error) throw new Error(String(data.error || `HTTP ${res.status}`));
  return asState(data);
}

/** 上传背景图（上传即选中）。 */
export async function uploadBackground(file: File): Promise<BackgroundState> {
  const form = new FormData();
  form.append('file', file);
  const res = await fetch('/api/background/upload', { method: 'POST', body: form });
  const data = await readJson(res);
  if (!res.ok || data.error) throw new Error(String(data.error || `HTTP ${res.status}`));
  return asState(data);
}

/** 选择背景图；`null` = 透明无背景。 */
export async function selectBackground(name: string | null): Promise<BackgroundState> {
  const res = await fetch('/api/background/select', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file: name }),
  });
  const data = await readJson(res);
  if (!res.ok || data.error) throw new Error(String(data.error || `HTTP ${res.status}`));
  return asState(data);
}

/** 删除背景图（若正被选中则自动回到透明）。 */
export async function deleteBackground(name: string): Promise<BackgroundState> {
  const res = await fetch('/api/background/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file: name }),
  });
  const data = await readJson(res);
  if (!res.ok || data.error) throw new Error(String(data.error || `HTTP ${res.status}`));
  return asState(data);
}

/* ------------------------------------------------------------------ *
 * 设置页「角色背景」区
 * ------------------------------------------------------------------ */

export interface BackgroundSection {
  /** 拉取最新状态并重渲染 */
  refresh(): Promise<void>;
}

/**
 * 在设置面板内装配背景管理区（上传 / 选择 / 删除 / 恢复透明）。
 *
 * @param root 承载区域（settings-ui 提供的 `[data-bgsection]`）
 * @param opts.onChanged 选中项变化回调 → 主画面立即换背景（无需刷新页面）
 * @param opts.onStatus  状态文案回调（复用设置面板底部状态条）
 */
export function initBackgroundSection(
  root: HTMLElement,
  opts: {
    onChanged?: (state: BackgroundState) => void;
    onStatus?: (msg: string, kind?: 'info' | 'ok' | 'error') => void;
  } = {},
): BackgroundSection {
  root.innerHTML = `
    <div class="settings-editor-actions">
      <button class="btn primary bg-upload-btn" type="button">上传背景图</button>
      <button class="btn ghost bg-none-btn" type="button">恢复透明（无背景）</button>
      <input class="bg-file-input" type="file" accept="image/png,image/jpeg,image/webp,image/gif" hidden />
      <span class="settings-tip">支持 png / jpg / webp / gif，单张 ≤ 12MB；上传后立即生效。</span>
    </div>
    <div class="settings-status-inline" data-bgstatus></div>
    <div class="settings-bg-grid" data-bggrid></div>
  `;

  const uploadBtn = root.querySelector<HTMLButtonElement>('.bg-upload-btn')!;
  const noneBtn = root.querySelector<HTMLButtonElement>('.bg-none-btn')!;
  const fileInput = root.querySelector<HTMLInputElement>('.bg-file-input')!;
  const statusEl = root.querySelector<HTMLElement>('[data-bgstatus]')!;
  const gridEl = root.querySelector<HTMLElement>('[data-bggrid]')!;

  let state: BackgroundState = EMPTY_BACKGROUND_STATE;

  const setStatus = (msg: string, kind: 'info' | 'ok' | 'error' = 'info'): void => {
    statusEl.textContent = msg;
    opts.onStatus?.(msg, kind);
  };

  const apply = (next: BackgroundState): void => {
    state = next;
    render();
    opts.onChanged?.(next);
  };

  const render = (): void => {
    gridEl.innerHTML = '';

    const noneCard = document.createElement('button');
    noneCard.type = 'button';
    noneCard.className = `settings-bg-card ui-lift${state.current ? '' : ' selected'}`;
    noneCard.innerHTML = `
      <span class="settings-bg-thumb settings-bg-thumb-none">透明</span>
      <span class="settings-bg-name">无背景</span>
      <span class="settings-bg-meta">桌宠透明模式</span>
    `;
    noneCard.addEventListener('click', () => void choose(null));
    gridEl.appendChild(noneCard);

    for (const f of state.files) {
      const card = document.createElement('div');
      card.className = `settings-bg-card ui-lift${state.current === f.name ? ' selected' : ''}`;
      const safeName = escapeHtml(f.name);
      card.innerHTML = `
        <span class="settings-bg-thumb" style="background-image:url('${escapeCssUrl(f.url)}')"></span>
        <span class="settings-bg-name" title="${safeName}">${safeName}</span>
        <span class="settings-bg-meta">${escapeHtml(formatBytes(f.size))}</span>
        <button class="btn danger btn-xs settings-bg-del" type="button" title="删除这张背景">删除</button>
      `;
      card.addEventListener('click', (e) => {
        if ((e.target as HTMLElement).closest('.settings-bg-del')) return;
        void choose(f.name);
      });
      card
        .querySelector<HTMLButtonElement>('.settings-bg-del')!
        .addEventListener('click', (e) => {
          e.stopPropagation();
          void remove(f.name);
        });
      gridEl.appendChild(card);
    }

    if (state.files.length === 0) {
      const tip = document.createElement('div');
      tip.className = 'settings-tip';
      tip.textContent = '还没有背景图，点「上传背景图」选一张本地图片。';
      gridEl.appendChild(tip);
    }
  };

  const choose = async (name: string | null): Promise<void> => {
    try {
      setStatus(name ? `切换背景「${name}」…` : '恢复透明背景…');
      apply(await selectBackground(name));
      setStatus(name ? `已应用背景「${name}」` : '已恢复透明背景（无背景图）', 'ok');
    } catch (err) {
      setStatus(`切换失败: ${(err as Error).message}`, 'error');
    }
  };

  const remove = async (name: string): Promise<void> => {
    try {
      setStatus(`删除「${name}」…`);
      apply(await deleteBackground(name));
      setStatus(`已删除「${name}」`, 'ok');
    } catch (err) {
      setStatus(`删除失败: ${(err as Error).message}`, 'error');
    }
  };

  const upload = async (file: File): Promise<void> => {
    const localError = validateLocalFile(file);
    if (localError) {
      setStatus(localError, 'error');
      return;
    }
    try {
      setStatus(`上传「${file.name}」…`);
      uploadBtn.disabled = true;
      apply(await uploadBackground(file));
      setStatus(`已上传并应用「${file.name}」`, 'ok');
    } catch (err) {
      setStatus(`上传失败: ${(err as Error).message}`, 'error');
    } finally {
      uploadBtn.disabled = false;
      fileInput.value = '';
    }
  };

  uploadBtn.addEventListener('click', () => fileInput.click());
  noneBtn.addEventListener('click', () => void choose(null));
  fileInput.addEventListener('change', () => {
    const file = fileInput.files?.[0];
    if (file) void upload(file);
  });

  const refresh = async (): Promise<void> => {
    try {
      apply(await fetchBackgroundState());
      setStatus(
        state.current ? `当前背景：${state.current}` : '当前无背景（透明模式）',
        'info',
      );
    } catch (err) {
      setStatus(`背景列表加载失败: ${(err as Error).message}`, 'error');
    }
  };

  render();
  return { refresh };
}
