import { useEffect, useState } from 'react';
import type { SkullState } from '../domain';

const fragments = ['THE AIR REMEMBERS', 'SOMETHING IS LISTENING', 'YOU WERE NEVER ALONE', 'ARCHIVE 03 // NO KNOWN AUTHOR'];
export function ArchiveFragment({ enabled }: { enabled: boolean }) {
  const [phrase, setPhrase] = useState('');
  useEffect(() => {
    if (!enabled || window.matchMedia('(prefers-reduced-motion: reduce)').matches) { setPhrase(''); return; }
    let show: ReturnType<typeof setTimeout>; let hide: ReturnType<typeof setTimeout>;
    const schedule = () => {
      const values = crypto.getRandomValues(new Uint32Array(2));
      show = setTimeout(() => {
        if (!document.hidden && !document.querySelector('dialog[open]') && !['INPUT','TEXTAREA','SELECT'].includes(document.activeElement?.tagName || '')) {
          setPhrase(fragments[values[1] % fragments.length]); hide = setTimeout(() => setPhrase(''), 7000);
        }
        schedule();
      }, 90_000 + values[0] % 120_000);
    }; schedule(); return () => { clearTimeout(show); clearTimeout(hide); };
  }, [enabled]);
  return <div className="archive-fragment" aria-hidden="true"><small>ARCHIVE FRAGMENT / DECORATIVE</small><span>{enabled ? phrase : ''}</span></div>;
}

export function TerminalAudio({ enabled, reaction, reactionId }: { enabled: boolean; reaction: SkullState; reactionId: number }) {
  const [context, setContext] = useState<AudioContext | null>(null);
  useEffect(() => {
    if (!enabled) return;
    const ctx = new AudioContext(); setContext(ctx);
    const gain = ctx.createGain(); gain.gain.value = .006; gain.connect(ctx.destination);
    const hum = ctx.createOscillator(); hum.type = 'sine'; hum.frequency.value = 49.5; hum.connect(gain); hum.start();
    const buffer = ctx.createBuffer(1, ctx.sampleRate * 3, ctx.sampleRate); const data = buffer.getChannelData(0);
    for (let i = 0; i < data.length; i++) data[i] = (Math.random() * 2 - 1) * .1;
    const noise = ctx.createBufferSource(); noise.buffer = buffer; noise.loop = true;
    const filter = ctx.createBiquadFilter(); filter.type = 'lowpass'; filter.frequency.value = 220; noise.connect(filter); filter.connect(gain); noise.start();
    const visibility = () => { if (document.hidden) void ctx.suspend(); else void ctx.resume().catch(() => {}); };
    document.addEventListener('visibilitychange', visibility); visibility();
    return () => { document.removeEventListener('visibilitychange', visibility); hum.stop(); noise.stop(); void ctx.close(); setContext(null); };
  }, [enabled]);
  useEffect(() => {
    if (!context || context.state !== 'running' || document.hidden || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    if (!['success','error'].includes(reaction)) return;
    const gain = context.createGain(); gain.connect(context.destination); const time = context.currentTime;
    gain.gain.setValueAtTime(.001, time); gain.gain.linearRampToValueAtTime(.018, time + .02); gain.gain.exponentialRampToValueAtTime(.001, time + .5);
    const tone = context.createOscillator(); tone.type = 'triangle'; tone.frequency.setValueAtTime(reaction === 'error' ? 80 : 150, time); tone.frequency.exponentialRampToValueAtTime(40, time + .45); tone.connect(gain); tone.start(time); tone.stop(time + .5); tone.onended = () => { tone.disconnect(); gain.disconnect(); };
  }, [context, reaction, reactionId]);
  return null;
}
