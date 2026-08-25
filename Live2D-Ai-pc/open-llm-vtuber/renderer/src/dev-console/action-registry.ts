/** Developer-mode action extension registry.
 *
 * The backend owns the canonical allowlist; this frontend registry mirrors it
 * and provides an extension point for future easter-egg actions without
 * hardcoding UI changes. Unknown IDs are rejected by both sides.
 */

export interface DevActionMeta {
  label: string;
  /** 'gesture' = single-pose body motion; 'choreo' = timeline choreography. */
  kind: 'gesture' | 'choreo';
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
    this.actions.set(key, { ...meta });
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

  /** Replace the entire catalog (e.g. after fetching from /api/dev/actions/catalog). */
  loadCatalog(ids: string[]): void {
    this.actions.clear();
    for (const id of ids) {
      const key = id.trim().toLowerCase();
      if (!key) continue;
      const kind =
        key === 'wave_big' ||
        key === 'bow' ||
        key === 'bounce_happy' ||
        key === 'shake_no' ||
        key === 'lean_in' ||
        key === 'look_around'
          ? 'choreo'
          : 'gesture';
      this.register(key, { label: key, kind });
    }
  }
}

/** Shared registry instance used by the dev console. */
export const devActionRegistry = new DevActionRegistry();
