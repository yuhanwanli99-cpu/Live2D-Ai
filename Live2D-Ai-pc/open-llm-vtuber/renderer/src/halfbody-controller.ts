/** Preset-free semantic half-body intent consumer. */
import { ParamArbiter, ParamSource } from './param-arbiter';
import { expandHalfBodyIntent, type HalfBodyProfile } from './halfbody-profile';

export class HalfBodyController {
  private lastIntentKey = '';

  constructor(private readonly arbiter: ParamArbiter, private readonly profile: HalfBodyProfile) {}

  consume(intent: Record<string, number>, fadeMs = 400): void {
    const key = JSON.stringify(intent);
    if (key === this.lastIntentKey) return;
    this.lastIntentKey = key;

    const params = expandHalfBodyIntent(intent, this.profile);
    const timestamp = Date.now();
    for (const [paramId, value] of Object.entries(params)) {
      this.arbiter.write({
        source: ParamSource.Emotion,
        paramId,
        value,
        mode: 'set',
        fadeMs,
        timestamp,
        valueSpace: 'normalized',
      });
    }
  }

  capabilities(): Record<string, boolean> {
    return { ...this.profile.capabilities };
  }
}
