import { useState } from 'react';
import type { SkullState } from '../../domain';
import { SkullAnimator } from './SkullAnimator';
import { POSE_NAMES } from './frames';
import './preview.css';

/** Development-only, isolated from sessions, scans, captures, and success events. */
export function SkullPreview() {
  const [state, setState] = useState<SkullState>('success');
  const [motion, setMotion] = useState(true);
  const [replay, setReplay] = useState(0);
  return <main className="skull-dev" data-theme="phosphor">
    <header><div><p>DEAD//AIR · CHARACTER CELL LAB · DEVELOPMENT ONLY</p><h1>SKULL / ANATOMY & MOTION</h1></div><a href="/">RETURN TO APPLICATION ↗</a></header>
    <div className="skull-dev-controls"><label>Application state <select aria-label="Skull state" value={state} onChange={e => { setState(e.target.value as SkullState); setReplay(r => r + 1); }}>{(['startup', 'idle', 'scanning', 'network', 'grin', 'success', 'lab', 'disappointed', 'error', 'alert', 'critical', 'shutdown', 'ambient-blink', 'ambient-glance', 'ambient-twitch', 'ambient-stare', 'ambient-grin'] as const).map(value => <option key={value}>{value}</option>)}</select></label><label><input type="checkbox" checked={motion} onChange={e => setMotion(e.target.checked)} />Motion</label><button onClick={() => setReplay(r => r + 1)}>REPLAY STATE</button><span>72 × 48 CELLS / 4 JAW POSES / NO RANDOM FILL</span></div>
    <section className="skull-live-stage"><SkullAnimator state={state} motion={motion} replayKey={replay} /><div><h2>{state.toUpperCase()}</h2><p>Upper skull locked.<br />Rigid mandible moves 0 → 1 → 3 → 6 cells.<br />Two laugh cycles, then closed.<br />Eyes are a separate crimson layer.</p><p>Fixed text grid. No image assets.<br />Background tabs pause the animation clock.<br />Reduced motion follows system preference.</p></div></section>
    <section className="skull-contact-sheet">{POSE_NAMES.map((name, i) => <figure key={name}><figcaption>0{i + 1} / {name}</figcaption><SkullAnimator state="success" motion={false} poseOverride={i} /></figure>)}</section>
  </main>;
}
