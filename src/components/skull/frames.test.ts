import { describe, expect, it } from 'vitest';
import { cellFrames, eyeLayer, frameAt, GRID_HEIGHT, GRID_WIDTH, JAW_OFFSETS, LAUGH_CYCLE_MS, LAUGH_SEQUENCE, poseAt, skullFrames, upperLayer } from './frames';

describe('character-cell anatomy', () => {
  it('keeps exactly 72 × 48 cells in every pose without cropping the jaw', () => {
    expect(new Set(skullFrames).size).toBe(4);
    for (const frame of skullFrames) {
      const rows = frame.split('\n');
      expect(rows).toHaveLength(GRID_HEIGHT);
      rows.forEach(row => expect(row).toHaveLength(GRID_WIDTH));
      expect(rows.at(-1)?.trim()).toBe('');
    }
  });
  it('locks upper skull and orbital geometry while the mandible moves', () => {
    upperLayer.forEach((row, y) => [...row].forEach((glyph, x) => {
      if (glyph !== ' ' && eyeLayer[y][x] === ' ') cellFrames.forEach(frame => expect(frame[y][x].glyph).toBe(glyph));
    }));
    const lowestJawRow = (pose: number) => cellFrames[pose].reduce((last, row, y) => row.some(cell => cell.tone === 'jaw' && cell.glyph !== ' ') ? y : last, -1);
    JAW_OFFSETS.forEach((offset, i) => expect(lowestJawRow(i) - lowestJawRow(0)).toBe(offset));
  });
  it('places the red eyes only within the two hollow sockets', () => {
    const xPositions: number[] = [];
    eyeLayer.forEach((row, y) => [...row].forEach((glyph, x) => {
      if (glyph !== ' ') {
        xPositions.push(x);
        expect(upperLayer[y][x]).toBe(' ');
        expect(y).toBeGreaterThanOrEqual(19);
        expect(y).toBeLessThanOrEqual(23);
      }
    }));
    expect(xPositions.some(x => x < 32)).toBe(true);
    expect(xPositions.some(x => x > 40)).toBe(true);
    expect(xPositions.some(x => x >= 32 && x <= 40)).toBe(false);
  });
});
describe('state-driven choreography', () => {
  it('plays closed → slightly open → wide → maximum → wide → slight → closed', () => {
    let elapsed = 0;
    for (const frame of LAUGH_SEQUENCE) { expect(poseAt(elapsed, 'success')).toBe(frame.pose); elapsed += frame.duration; }
    expect(poseAt(LAUGH_CYCLE_MS * 2, 'success')).toBe(0);
    expect(poseAt(1e9, 'success')).toBe(0);
  });
  it('never laughs because time passed in idle, scanning, or error states', () => {
    for (const state of ['idle', 'scanning', 'error', 'shutdown'] as const) for (const time of [0, 160, 480, 1840, 10000]) expect(poseAt(time, state)).toBe(0);
  });
  it('provides a genuinely static reduced-motion fallback', () => {
    expect(frameAt('success', 600, false)).toEqual(cellFrames[0]);
    expect(frameAt('error', 4200, false)).toEqual(cellFrames[0]);
  });
  it('ends shutdown with an empty grid of identical dimensions', () => {
    const frame = frameAt('shutdown', 2400, true);
    expect(frame).toHaveLength(GRID_HEIGHT);
    expect(frame.every(row => row.length === GRID_WIDTH && row.every(cell => cell.glyph === ' '))).toBe(true);
  });
  it('limits the occasional glitch to two cranial rows', () => {
    const normal = frameAt('scanning', 0, true);
    const glitch = frameAt('scanning', 8560, true);
    const changed = glitch.flatMap((row, y) => JSON.stringify(row) !== JSON.stringify(normal[y]) ? [y] : []);
    expect(changed).toEqual([10, 12]);
    expect(frameAt('scanning', 8640, true)).toEqual(normal);
  });
});
