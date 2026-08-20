/**
 * icons.ts — 统一内联 SVG 图标集（替换 emoji）
 *
 * 背景：此前 UI 直接用 emoji 当图标（⚙ 设置、🎤 麦克风、🔊 播放中），
 * 不同平台字形、字重、基线都不一致，是"廉价感"的直接来源之一
 * （见 docs/research/neko-ui-alignment-and-gaps.md §2.3-8，P1-3）。
 *
 * 约定：
 *   - 统一 24×24 viewBox、`currentColor` 描边、线宽 1.8、圆头圆角；
 *   - 只导出字符串（不引入任何图标库/字体，零依赖、零网络请求）；
 *   - 尺寸由外部用 CSS（width/height）控制，图标本身不写死 px。
 */

export type IconName =
  | 'settings'
  | 'mic'
  | 'send'
  | 'close'
  | 'volume'
  | 'sparkle'
  | 'image'
  | 'upload'
  | 'trash'
  | 'plug'
  | 'recenter'
  | 'history'
  | 'check'
  | 'alert'
  | 'info';

/** 统一图标外壳：stroke 版（线性图标，视觉重量一致） */
const stroke = (body: string, extra = ''): string =>
  `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" ` +
  `stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" ${extra}>${body}</svg>`;

const PATHS: Record<IconName, string> = {
  settings:
    '<circle cx="12" cy="12" r="3.2"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.11A1.7 1.7 0 0 0 8.9 19.3a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.11A1.7 1.7 0 0 0 4.7 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 1 1 4 0v.11A1.7 1.7 0 0 0 15 4.7a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9v.03a1.7 1.7 0 0 0 1.56 1.03H21a2 2 0 1 1 0 4h-.11a1.7 1.7 0 0 0-1.49 1z"/>',
  mic: '<path d="M12 3.5a3 3 0 0 0-3 3v5a3 3 0 0 0 6 0v-5a3 3 0 0 0-3-3z"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0"/><path d="M12 17.5V21"/><path d="M8.5 21h7"/>',
  send: '<path d="M4.5 12 20 4.5 15.5 20 12 13.5z"/><path d="M12 13.5 20 4.5"/>',
  close: '<path d="M6 6l12 12"/><path d="M18 6 6 18"/>',
  volume:
    '<path d="M4 9.5h3L11.5 6v12L7 14.5H4z"/><path d="M15.5 9a4 4 0 0 1 0 6"/><path d="M18 6.5a7.5 7.5 0 0 1 0 11"/>',
  sparkle:
    '<path d="M12 3.5l1.6 4.4 4.4 1.6-4.4 1.6L12 15.5l-1.6-4.4L6 9.5l4.4-1.6z"/><path d="M18.5 15.5l.8 2 2 .8-2 .8-.8 2-.8-2-2-.8 2-.8z"/>',
  image:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><circle cx="9" cy="10" r="1.6"/><path d="M4 17l4.5-4.5a2 2 0 0 1 2.8 0L20 20"/>',
  upload: '<path d="M12 16.5V4.5"/><path d="M7.5 9 12 4.5 16.5 9"/><path d="M4.5 15.5v3a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-3"/>',
  trash:
    '<path d="M4.5 7h15"/><path d="M9.5 7V5.2a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1V7"/><path d="M6.5 7l.8 12a2 2 0 0 0 2 1.9h5.4a2 2 0 0 0 2-1.9l.8-12"/><path d="M10.5 11v6"/><path d="M13.5 11v6"/>',
  plug:
    '<path d="M9 3.5v5"/><path d="M15 3.5v5"/><path d="M6.5 8.5h11v3a5.5 5.5 0 0 1-11 0z"/><path d="M12 17v3.5"/>',
  recenter:
    '<circle cx="12" cy="12" r="3"/><path d="M12 3v3"/><path d="M12 18v3"/><path d="M3 12h3"/><path d="M18 12h3"/>',
  history:
    '<path d="M3.6 12a8.4 8.4 0 1 0 2.6-6.05"/><path d="M3.5 4.2v4.3h4.3"/><path d="M12 7.8v4.6l3.1 1.8"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  alert: '<path d="M12 4.5 21 19.5H3z"/><path d="M12 10v4"/><path d="M12 16.6h.01"/>',
  info: '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5"/><path d="M12 7.8h.01"/>',
};

/**
 * 取得图标 SVG 字符串。
 * @param name 图标名
 * @param extraAttrs 额外 svg 属性（如 `class="icon-lg"`）
 */
export function icon(name: IconName, extraAttrs = ''): string {
  return stroke(PATHS[name], extraAttrs);
}
