import { useCallback, useEffect, useRef, useState } from 'react';
import type { SkullState } from '../../domain';
import { ReactionGate, reactionDuration } from './reactions';

const ambient: SkullState[] = ['ambient-blink', 'ambient-blink', 'ambient-glance', 'ambient-twitch', 'ambient-stare', 'ambient-grin'];
export function useSkullReactions(motion: boolean) {
  const [skull, update] = useState<SkullState>('startup');
  const [reactionId, bump] = useState(0);
  const gate = useRef(new ReactionGate());
  const base = useRef<SkullState>('idle');
  const lastInteraction = useRef(Date.now());
  const setSkull = useCallback((requested: SkullState) => {
    if (['idle', 'scanning', 'lab', 'shutdown', 'cleanup', 'critical'].includes(requested)) base.current = requested;
    if (['success', 'network', 'grin', 'disappointed', 'error', 'alert'].includes(requested)) base.current = 'idle';
    const next = gate.current.accept(requested, Date.now());
    update(next); bump(n => n + 1);
  }, []);
  useEffect(() => {
    const duration = reactionDuration[skull];
    if (!duration || skull === 'critical') return;
    const timer = window.setTimeout(() => update(skull === 'network' ? 'grin' : base.current), duration);
    return () => clearTimeout(timer);
  }, [skull, reactionId]);
  useEffect(() => {
    const interacted = () => { lastInteraction.current = Date.now(); };
    window.addEventListener('pointerdown', interacted); window.addEventListener('keydown', interacted);
    return () => { window.removeEventListener('pointerdown', interacted); window.removeEventListener('keydown', interacted); };
  }, []);
  useEffect(() => {
    if (!motion || skull !== 'idle' || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const random = new Uint32Array(2); crypto.getRandomValues(random);
    const timer = setTimeout(() => {
      const focused = document.activeElement?.tagName;
      if (document.hidden || Date.now() - lastInteraction.current < 12000 || ['INPUT', 'TEXTAREA', 'SELECT'].includes(focused || '') || document.querySelector('dialog[open]')) { bump(n => n + 1); return; }
      update(ambient[random[1] % ambient.length]); bump(n => n + 1);
    }, 45_000 + random[0] % 90_000);
    return () => clearTimeout(timer);
  }, [skull, motion, reactionId]);
  return { skull, setSkull, reactionId };
}
