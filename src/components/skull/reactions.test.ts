import { describe, expect, it } from 'vitest';
import { ReactionGate } from './reactions';
describe('meaningful reactions', () => {
  it('applies a 30 second cooldown between major animations', () => { const gate = new ReactionGate(); expect(gate.accept('success', 0)).toBe('success'); expect(gate.accept('success', 29999)).toBe('idle'); expect(gate.accept('error', 10000)).toBe('alert'); expect(gate.accept('success', 30000)).toBe('success'); });
  it('does not convert network events into a laugh', () => { const gate = new ReactionGate(); for (const state of ['network','scanning','grin','lab','shutdown','critical'] as const) expect(gate.accept(state, 0)).toBe(state); });
});
