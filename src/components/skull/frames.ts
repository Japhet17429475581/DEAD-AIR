import upperSource from './art/upper.txt?raw';
import jawSource from './art/jaw.txt?raw';
import eyesSource from './art/eyes.txt?raw';
import type { SkullState } from '../../domain';

export const GRID_WIDTH = 72;
export const GRID_HEIGHT = 48;
export const JAW_OFFSETS = [0, 1, 3, 6] as const;
export const POSE_NAMES = ['CLOSED', 'SLIGHTLY OPEN', 'WIDE OPEN', 'MAXIMUM LAUGH'] as const;
export type CellTone = 'bone' | 'jaw' | 'eye';
export interface Cell { glyph: string; tone: CellTone }
export interface GlyphRun { text: string; tone: CellTone }

function readLayer(source: string): string[] {
  const rows = source.replace(/\r/g, '').replace(/\n$/, '').split('\n');
  if (rows.length !== GRID_HEIGHT || rows.some(row => row.length !== GRID_WIDTH)) throw new Error('Skull art must be exactly 72 columns × 48 rows.');
  return rows;
}
export const upperLayer = readLayer(upperSource);
export const jawLayer = readLayer(jawSource);
export const eyeLayer = readLayer(eyesSource);

export function composeFrame(pose: number): Cell[][] {
  const offset = JAW_OFFSETS[Math.max(0, Math.min(3, pose))];
  const frame: Cell[][] = upperLayer.map(row => [...row].map(glyph => ({ glyph, tone: 'bone' })));
  // Bone wins at the condyles: the fixed upper skull never changes between poses.
  jawLayer.forEach((row, y) => [...row].forEach((glyph, x) => {
    if (glyph !== ' ' && y + offset < GRID_HEIGHT && frame[y + offset][x].glyph === ' ') frame[y + offset][x] = { glyph, tone: 'jaw' };
  }));
  // Articulated rami keep the moving mandible attached at its two condyles.
  // They extend only into empty cells; the maxilla and cheekbones stay fixed.
  for (let y = 30; y < 32 + offset; y++) for (const x of [19, 20, 51, 52]) {
    if (frame[y][x].glyph === ' ') frame[y][x] = { glyph: x === 19 || x === 52 ? ':' : '#', tone: 'jaw' };
  }
  eyeLayer.forEach((row, y) => [...row].forEach((glyph, x) => {
    if (glyph !== ' ') frame[y][x] = { glyph, tone: 'eye' };
  }));
  return frame;
}
export const cellFrames = JAW_OFFSETS.map((_, i) => composeFrame(i));
export const skullFrames = cellFrames.map(frame => frame.map(row => row.map(cell => cell.glyph).join('')).join('\n'));

// One physical opening/closing cycle. No CSS stretching or moving eye sockets.
export const LAUGH_SEQUENCE = [
  { pose: 0, duration: 160 }, { pose: 1, duration: 160 },
  { pose: 2, duration: 160 }, { pose: 3, duration: 240 },
  { pose: 2, duration: 160 }, { pose: 1, duration: 160 },
  { pose: 0, duration: 240 },
] as const;
export const LAUGH_CYCLE_MS = LAUGH_SEQUENCE.reduce((sum, frame) => sum + frame.duration, 0);
export function poseAt(elapsed: number, state: SkullState): number {
  if (state === 'grin' || state === 'ambient-grin') return elapsed > 400 && elapsed < 1200 ? 1 : 0;
  if (state === 'ambient-twitch') return elapsed > 160 && elapsed < 320 ? 1 : 0;
  if (state !== 'success' || elapsed >= LAUGH_CYCLE_MS * 2) return 0;
  let time = Math.max(0, elapsed) % LAUGH_CYCLE_MS;
  for (const frame of LAUGH_SEQUENCE) { if (time < frame.duration) return frame.pose; time -= frame.duration; }
  return 0;
}
export function frameAt(state: SkullState, elapsed: number, motion: boolean, forcedPose?: number): Cell[][] {
  const pose = forcedPose ?? (motion ? poseAt(elapsed, state) : 0);
  const frame = cellFrames[pose].map(row => row.map(cell => ({ ...cell })));
  if (state === 'startup') {
    const rows = motion ? Math.floor(elapsed / 34) : GRID_HEIGHT;
    frame.forEach((row,y) => row.forEach((cell,x) => { if (y > rows) cell.glyph = ((x * 7 + y * 13) % 97 === 0 && elapsed < 900) ? '.' : ' '; if (cell.tone === 'eye' && elapsed < 1100 && motion) cell.glyph = ' '; }));
    return frame;
  }
  if (state === 'critical') {
    if (elapsed > 1400 || !motion) frame.forEach((row,y) => { if (!motion || y > GRID_HEIGHT - (elapsed-1400)/45) row.forEach(c => { c.glyph = ' '; }); });
    return frame;
  }
  if (state === 'shutdown' || state === 'cleanup') {
    const visibleRows = motion ? Math.max(0, GRID_HEIGHT - Math.floor(elapsed / 40)) : 0;
    frame.forEach((row, y) => row.forEach(cell => { if (cell.tone === 'eye' ? !motion || elapsed >= 2400 : y >= visibleRows) cell.glyph = ' '; }));
    return frame;
  }
  if (motion && (state === 'ambient-blink' && elapsed < 240 || state === 'network' && elapsed < 160)) frame.forEach((row,y) => row.forEach(cell => { if (cell.tone === 'eye') cell.glyph = state === 'ambient-blink' ? y===21 ? '-' : ' ' : '.'; }));
  if (motion && state === 'ambient-glance') {
    frame.forEach(row => { for (let x=0;x<GRID_WIDTH-1;x++) if (row[x].tone==='eye') row[x]={glyph:' ',tone:'bone'}; });
    eyeLayer.forEach((row,y) => [...row].forEach((glyph,x) => { if (glyph!==' ' && x+1<GRID_WIDTH) frame[y][x+1]={glyph,tone:'eye'}; }));
  }
  // Rare 80 ms line displacement; two non-facial rows, no brightness flashes.
  const glitch = motion && (state === 'error' ? elapsed >= 80 && elapsed < 160 : state === 'success' ? elapsed >= 1840 && elapsed < 1920 : state === 'scanning' && elapsed % 8800 >= 8560 && elapsed % 8800 < 8640);
  if (glitch) {
    frame[10] = [{ glyph: ' ', tone: 'bone' }, ...frame[10].slice(0, -1)];
    frame[12] = [...frame[12].slice(1), { glyph: ' ', tone: 'bone' }];
  }
  return frame;
}
export function rowRuns(row: Cell[]): GlyphRun[] {
  const runs: GlyphRun[] = [];
  for (const cell of row) {
    const last = runs.at(-1);
    if (last?.tone === cell.tone) last.text += cell.glyph;
    else runs.push({ text: cell.glyph, tone: cell.tone });
  }
  return runs;
}
