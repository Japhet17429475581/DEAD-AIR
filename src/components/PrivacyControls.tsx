import { useEffect, useRef, useState, type ReactNode } from 'react';
import { isDesktop, native } from '../bridge';
import type { Adapter } from '../domain';
import { Panel } from '../ui';
import { bindingFrom, bindingLabel, DEFAULT_PANIC_BINDING, matchesBinding } from './privacyKeys';
function EmergencyDialog({children}:{children:ReactNode}) {
  const ref=useRef<HTMLDialogElement>(null);
  useEffect(()=>{const dialog=ref.current;dialog?.showModal();return()=>dialog?.close();},[]);
  return <dialog ref={ref} className="panic-overlay" aria-label="PANIC cleanup" onCancel={event=>event.preventDefault()}>{children}</dialog>;
}

export function usePanicControls(demo: boolean, onStart: () => void) {
  const [binding, setBinding] = useState(DEFAULT_PANIC_BINDING);
  const [recording, setRecording] = useState(false);
  const [allSessions, setAllSessions] = useState(true);
  const [shred, setShred] = useState(true);
  const [phase, setPhase] = useState<'idle' | 'running' | 'failed' | 'preview'>('idle');
  const [error, setError] = useState('');
  const pending = useRef(false);
  const current = useRef({demo, onStart}); current.current = {demo, onStart};
  async function run() {
    if (pending.current) return;
    pending.current = true; setRecording(false); setError(''); setPhase('running'); current.current.onStart();
    try {
      if (current.current.demo || !isDesktop()) {
        setError('PREVIEW ONLY: the interface was cleared. No native files were deleted and no daemon was stopped.'); setPhase('preview');
      } else {
        await native<number>('panic_cleanup', {allSessions, shred});
        // The backend exits the application only after verified cleanup. Never fake completion here.
      }
    } catch (e) { setError(String(e)); setPhase('failed'); }
    finally { pending.current = false; }
  }
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (recording || phase === 'running') return;
      if (matchesBinding(event, binding)) { event.preventDefault(); event.stopImmediatePropagation(); void run(); }
    };
    window.addEventListener('keydown', key, true);
    return () => window.removeEventListener('keydown', key, true);
  });
  const button = <button className="panic-button" title={`Stop jobs and erase ${allSessions ? 'ALL app sessions, including persistent' : 'the current session'}; ${shred ? 'shred then delete' : 'delete'}; exit. ${bindingLabel(binding)}`} disabled={phase === 'running'} onClick={() => void run()}>PANIC · {phase === 'running' ? 'CLEANING…' : 'ERASE & EXIT'}</button>;
  const settings = <Panel title="PANIC // STOP · ERASE · EXIT">
    <p className="body-copy">Stops capture and Hashcat, restores the MAC lease, removes the selected app-owned sessions, stops the privileged daemon and exits. An error keeps the window open.</p>
    <label className="checkbox"><input type="checkbox" checked={allSessions} disabled={phase === 'running'} onChange={e => setAllSessions(e.target.checked)} />Erase ALL DEAD//AIR sessions, including persistent sessions and saved captures</label>
    <label className="checkbox"><input type="checkbox" checked={shred} disabled={phase === 'running'} onChange={e => setShred(e.target.checked)} />Overwrite regular session files with GNU shred before deletion</label>
    <label className="field-label">PANIC shortcut<input aria-label="PANIC shortcut" readOnly value={recording ? 'Press a key or combination…' : bindingLabel(binding)} onFocus={() => setRecording(true)} onBlur={() => setRecording(false)} onKeyDown={event => {event.preventDefault(); event.stopPropagation(); const next = bindingFrom(event.nativeEvent); if (next) {setBinding(next); setRecording(false); event.currentTarget.blur();}}} /></label>
    <p className="muted small">Click the shortcut field to bind a key. Works while DEAD//AIR has focus, including text fields. Reserved OS shortcuts may be unavailable. Settings stay in memory; restart restores Ctrl + Shift + F12.</p>
    <p className="muted small">This is not Tails or guaranteed secure erasure. Shred cannot guarantee removal from SSD remapped blocks, snapshots, copy-on-write storage, swap, system logs or external copies.</p>
  </Panel>;
  const overlay = phase !== 'idle' && <EmergencyDialog><section><h2>PANIC // {phase === 'running' ? 'CLEANUP IN PROGRESS' : phase === 'failed' ? 'CLEANUP INCOMPLETE' : 'DEMO / BROWSER PREVIEW'}</h2><p>{phase === 'running' ? 'Stopping operations and cleaning the selected session data. Keep the application open until this finishes.' : error}</p>{phase === 'failed' && <div className="button-row"><button className="danger" onClick={() => void run()}>RETRY PANIC</button><button onClick={() => setPhase('idle')}>SHOW RECOVERY CONTROLS</button></div>}{phase === 'preview' && <button onClick={() => setPhase('idle')}>RETURN TO INTERFACE</button>}</section></EmergencyDialog>;
  return {button, settings, overlay, running: phase === 'running', recording};
}

interface MacStatus {active: boolean; adapter: string; profile: string; original: string; assigned: string; current: string; error: string | null}
const emptyMac: MacStatus = {active:false,adapter:'',profile:'',original:'',assigned:'',current:'',error:null};
export function MacControls({adapters, demo, captureRunning, onChanged}: {adapters:Adapter[]; demo:boolean; captureRunning:boolean; onChanged:()=>void}) {
  const [adapter, setAdapter] = useState('');
  const [profile, setProfile] = useState('private');
  const [allowDisconnect,setAllowDisconnect]=useState(false);
  const [state, setState] = useState<MacStatus>(emptyMac);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(()=>{setAllowDisconnect(false);},[adapter]);
  useEffect(() => {if (!demo && isDesktop()) {let alive=true; const refresh=()=>native<MacStatus>('mac_status').then(s=>{if(alive)setState(s);}).catch(e=>{if(alive)setError(String(e));});void refresh();const timer=setInterval(()=>void refresh(),2000);return ()=>{alive=false;clearInterval(timer);};}},[demo]);
  async function operation(command:string) {
    setBusy(true);setError('');
    try {const result=await native<MacStatus|string>(command, command==='mac_start'?{adapter,profile,...(allowDisconnect?{allowDisconnect:true}:{})}:undefined);if(typeof result==='string')setError(result);else setState(result);onChanged();}catch(e){setError(String(e));}finally{setBusy(false);}
  }
  return <Panel title="MAC SPOOFING // DEVICE PREFIX" meta={state.active ? 'LEASE ACTIVE' : 'IDLE'}>
    <div className="two-column"><label className="field-label">Wi-Fi adapter · built-in or USB<select aria-label="MAC adapter" value={adapter} disabled={busy||state.active||captureRunning} onChange={e=>setAdapter(e.target.value)}><option value="">Select adapter</option>{adapters.map(a=><option key={a.name} value={a.name} disabled={a.mode!=='managed'}>{a.name} / {a.driver} / {a.connected?'CONNECTED':a.mode}</option>)}</select></label>
    <label className="field-label">MAC profile<select aria-label="MAC profile" value={profile} disabled={busy||state.active} onChange={e=>setProfile(e.target.value)}><option value="private">Random private address</option><option value="apple">iPhone / Apple vendor prefix</option><option value="samsung">Android / Samsung vendor prefix</option><option value="intel">Laptop / Intel vendor prefix</option><option value="raspberry">Raspberry Pi vendor prefix</option></select></label></div>
    <label className="checkbox"><input type="checkbox" checked={allowDisconnect} disabled={busy||state.active||captureRunning} onChange={e=>setAllowDisconnect(e.target.checked)} />I allow Wi-Fi disconnection for this MAC lease. The selected card stays disconnected until RESTORE ORIGINAL MAC; its saved profile is then reactivated.</label>
    <p className="muted small">Built-in and USB cards are allowed in managed mode. Finish capture and restore managed mode before applying a profile. NetworkManager is released during the lease to prevent an immediate address overwrite. Driver support is required. Vendor prefixes change only the MAC, not the phone model, OS or radio capabilities.</p>
    <dl className="key-values"><dt>Lease adapter</dt><dd>{state.adapter||'—'}</dd><dt>Original MAC</dt><dd>{state.original||'—'}</dd><dt>Assigned MAC</dt><dd>{state.assigned||'—'}</dd><dt>Current readback</dt><dd>{state.current||'—'}{state.active&&state.current!==state.assigned?' / CHANGED EXTERNALLY':''}</dd></dl>
    <div className="button-row"><button className="primary" disabled={demo||!isDesktop()||!adapter||busy||state.active||captureRunning||!!adapters.find(a=>a.name===adapter)?.connected&&!allowDisconnect} onClick={()=>void operation('mac_start')}>APPLY MAC PROFILE</button><button disabled={demo||busy||!state.active||captureRunning} onClick={()=>void operation('mac_restore')}>RESTORE ORIGINAL MAC</button><button disabled={demo||busy||state.active||captureRunning||!isDesktop()} onClick={()=>void operation('mac_recover')}>RESTORE STALE MAC</button></div>
    {(error||state.error)&&<p className="error-inline" role="alert">{error||state.error}</p>}
  </Panel>;
}

export function PrivilegedControls() {
  const [active,setActive]=useState(false);const [busy,setBusy]=useState(false);const [error,setError]=useState('');
  useEffect(()=>{if(!isDesktop())return;let alive=true;const refresh=()=>native<boolean>('privileged_status').then(v=>{if(alive)setActive(v);}).catch(()=>{if(alive)setActive(false);});void refresh();const timer=setInterval(()=>void refresh(),2000);return()=>{alive=false;clearInterval(timer);};},[]);
  return <Panel title="PRIVILEGED DAEMON" meta={active?'AUTHORIZED':'NOT AUTHORIZED'}><p className="body-copy">One system authorization per working session. The scoped daemon handles adapter operations and dependency installation. It never stores your password or accepts arbitrary shell commands.</p><div className="button-row"><button disabled={busy||active||!isDesktop()} onClick={()=>{setBusy(true);setError('');void native<boolean>('privileged_authorize').then(setActive).catch(e=>setError(String(e))).finally(()=>setBusy(false));}}>{busy?'WAITING FOR POLKIT…':active?'DAEMON ACTIVE':'AUTHORIZE DAEMON'}</button></div><p className="muted small">Started on demand. Ends after END SESSION, PANIC, or closing the application. A new working session needs authorization again.</p>{error&&<p role="alert" className="error-inline">{error}</p>}</Panel>;
}
