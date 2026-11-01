import type { SkullState } from '../../domain';
export const MAJOR_COOLDOWN_MS = 30_000;
export const reactionDuration: Partial<Record<SkullState, number>> = { startup: 2000, network: 320, grin: 1600, success: 2560, error: 1200, critical: 3600, disappointed: 1800, 'ambient-blink': 320, 'ambient-glance': 1000, 'ambient-twitch': 480, 'ambient-stare': 2200, 'ambient-grin': 1400 };
export class ReactionGate {
  private lastMajor = -Infinity;
  accept(state: SkullState, now: number): SkullState {
    if (state !== 'success' && state !== 'error') return state;
    if (now - this.lastMajor < MAJOR_COOLDOWN_MS) return state === 'error' ? 'alert' : 'idle';
    this.lastMajor = now; return state;
  }
}
