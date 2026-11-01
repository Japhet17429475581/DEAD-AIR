import { useEffect, useState } from 'react';
import type { SkullState } from '../../domain';
import { frameAt, GRID_HEIGHT, GRID_WIDTH, LAUGH_CYCLE_MS, poseAt, rowRuns } from './frames';
import './skull.css';

export function SkullAnimator({ state, motion, poseOverride, replayKey = 0 }: { state: SkullState; motion: boolean; poseOverride?: number; replayKey?: number }) {
  const [elapsed, setElapsed] = useState(0);
  const [reduceMotion, setReduceMotion] = useState(() => window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  const animated = motion && !reduceMotion && poseOverride === undefined;
  useEffect(() => {
    const query = window.matchMedia('(prefers-reduced-motion: reduce)');
    const change = () => setReduceMotion(query.matches);
    query.addEventListener('change', change); return () => query.removeEventListener('change', change);
  }, []);
  useEffect(() => {
    setElapsed(0);
    if (!animated) return;
    let clock = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = () => {
      clock += 80; setElapsed(clock);
      if ((state === 'shutdown' || state === 'cleanup') && clock >= 2400) return;
      if (state === 'critical' && clock >= 3600) return;
      if (state === 'success' && clock >= LAUGH_CYCLE_MS * 2) return;
      timer = setTimeout(tick, 80);
    };
    const visible = () => { clearTimeout(timer); if (!document.hidden) timer = setTimeout(tick, 80); };
    document.addEventListener('visibilitychange', visible); visible();
    return () => { clearTimeout(timer); document.removeEventListener('visibilitychange', visible); };
  }, [state, animated, replayKey]);
  const pose = poseOverride ?? (animated ? poseAt(elapsed, state) : 0);
  const frame = frameAt(state, elapsed, animated, poseOverride);
  return <div className={`skull skull-${state} ${animated ? 'skull-animated' : 'skull-static'}`}
    role="img" aria-label={`ASCII skull: ${state}`} data-state={state} data-pose={pose}
    data-grid={`${GRID_WIDTH}x${GRID_HEIGHT}`}>
    <pre className="skull-grid" aria-hidden="true">{frame.map((row, y) => <span className="skull-row" key={y}>{rowRuns(row).map((run, i) => <span className={`skull-cell-${run.tone}`} key={i}>{run.text}</span>)}</span>)}</pre>
  </div>;
}
