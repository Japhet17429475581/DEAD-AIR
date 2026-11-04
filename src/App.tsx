import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { native, isDesktop, discoverLive } from './bridge';
import { bandOf, cleanSsid, demoNetworks, filterNetworks, maskBssid, reportFor, review, type Capture, type Diagnostics, type LabStatus, type LogEntry, type Network, type Page, type Policy, type Session, type Source } from './domain';
import { ChannelChart, Modal, NetworkTable, Panel, Signal } from './ui';
import { SkullAnimator } from './skull';
import { DependencySetup } from './components/DependencySetup';
import { CaptureHealth } from './components/CaptureHealth';
import { useSkullReactions } from './components/skull/useSkullReactions';
import { ArchiveFragment, TerminalAudio } from './components/Atmosphere';
import { MacControls, PrivilegedControls, usePanicControls } from './components/PrivacyControls';
import {Academy,LearningShelf,useAcademy} from './academy/Academy';

const navigation: { id: Page; title: string; ru: string; icon: string }[] = [
  { id: 'dashboard', title: 'OVERVIEW', ru: 'ОБЗОР', icon: '⌂' }, { id: 'discover', title: 'DISCOVER', ru: 'СЕТИ', icon: '⌁' },
  { id: 'inspector', title: 'INSPECTOR', ru: 'ИНСПЕКТОР', icon: '⊞' }, { id: 'spectrum', title: 'SIGNAL', ru: 'СИГНАЛ', icon: '≋' },
  { id: 'capture', title: 'PASSIVE LAB', ru: 'НАБЛЮДЕНИЕ', icon: '◎' }, { id: 'adapter', title: 'ADAPTERS', ru: 'АДАПТЕРЫ', icon: '⊡' },
  { id: 'sessions', title: 'SESSIONS', ru: 'СЕССИИ', icon: '▤' }, { id: 'lab', title: 'HASHCAT LAB', ru: 'HASHCAT LAB', icon: '⌘' },
  { id: 'settings', title: 'SETTINGS', ru: 'НАСТРОЙКИ', icon: '⚙' },
  { id: 'academy', title: 'ACADEMY', ru: 'ОБУЧЕНИЕ', icon: '◇' },
];
const emptyCapture: Capture = { running: false, status: 'IDLE', packets: [], count: 0, dropped: 0, error: null, exchanges: [], pmkids:[] };
const emptyLab: LabStatus = { running: false, status: 'IDLE', lines: [], exitCode: null };
const stamp = () => new Date().toLocaleTimeString('en-GB', { hour12: false });
const errorMessage = (error: unknown) => typeof error === 'string' ? error : error instanceof Error ? error.message : JSON.stringify(error);

export function App() {
  const [page, setPage] = useState<Page>('dashboard');
  const [source, setSource] = useState<Source>('live');
  const [networks, setNetworks] = useState<Network[]>([]);
  const [selected, setSelected] = useState('');
  const [diagnostics, setDiagnostics] = useState<Diagnostics | null>(null);
  const [adapter, setAdapter] = useState('');
  const [captureAdapter, setCaptureAdapter] = useState('');
  const [adapterReport, setAdapterReport] = useState('');
  const [session, setSession] = useState<Session | null>(null);
  const [policy, setPolicy] = useState<Policy>('MEMORY_ONLY');
  const [scan, setScan] = useState(false);
  const [checking, setChecking] = useState(false);
  const [setupRequest, setSetupRequest] = useState(0);
  const [setupProfile, setSetupProfile] = useState('all');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [query, setQuery] = useState('');
  const [band, setBand] = useState('all');
  const [sort, setSort] = useState('signal');
  const [clock, setClock] = useState(stamp());
  const [scanlines, setScanlines] = useState(true);
  const [glow, setGlow] = useState(65);
  const [motion, setMotion] = useState(!window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  const [sound, setSound] = useState(false);
  const { skull, setSkull, reactionId } = useSkullReactions(motion);
  const [fontScale, setFontScale] = useState(100);
  const [language, setLanguage] = useState<'en' | 'ru'>('en');
  const [masked, setMasked] = useState(true);
  const [capture, setCapture] = useState<Capture>(emptyCapture);
  const [authorized, setAuthorized] = useState(false);
  const [allowDisconnect,setAllowDisconnect]=useState(false);
  const [lab, setLab] = useState<LabStatus>(emptyLab);
  const [labConsent, setLabConsent] = useState(false);
  const [knownPassword, setKnownPassword] = useState('');
  const [wordlist, setWordlist] = useState('');
  const [wordlistName, setWordlistName] = useState('No wordlist selected');
  const [kismetToken, setKismetToken] = useState('');
  const [kismetResult, setKismetResult] = useState('Not checked');
  const [stale, setStale] = useState<Session[]>([]);
  const [modal, setModal] = useState<'demo' | 'end' | 'export' | 'help' | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([{ id: 0, time: stamp(), level: 'INFO', message: 'DEAD//AIR initialized. No radio operations started.' }]);
  const sequence = useRef(0);
  const scanGeneration = useRef(0);
  const demoTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const scanTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const alive = useRef(true);
  const privacyStopping = useRef(false);
  const academy=useAcademy();
  const panic = usePanicControls(source === 'demo', () => {
    privacyStopping.current = true; ++scanGeneration.current;
    if (demoTimer.current) clearInterval(demoTimer.current);
    if (scanTimer.current) clearTimeout(scanTimer.current);
    setScan(false); setSound(false); setModal(null); setKnownPassword(''); setWordlist(''); setKismetToken('');
    setNetworks([]); setSelected(''); setCapture(emptyCapture); setLab(emptyLab); setLogs([]); setSkull('shutdown');
    academy.reset();
  });
  const selectedNetwork = networks.find(n => n.bssid === selected);
  const active = session?.state === 'ACTIVE';
  const visibleNetworks = filterNetworks(networks, query, band, sort);
  const t = (en: string, ru: string) => language === 'ru' ? ru : en;
  const log = (message: string, level: LogEntry['level'] = 'INFO') => setLogs(list => [...list.slice(-79), { id: ++sequence.current, time: stamp(), level, message }]);
  const fail = (e: unknown) => { const message = errorMessage(e); setError(message); setSkull(/restoration|Cleanup required|recovery journal/i.test(message) ? 'critical' : 'error'); log(message, 'WARN'); };
  async function action(task: () => Promise<void>) {
    if (busy) return;
    setBusy(true); setError('');
    try { await task(); } catch (e) { fail(e); } finally { setBusy(false); }
  }
  function celebrate() { setSkull('success'); }
  async function checkSystem() {
    setChecking(true); setError('');
    try {
      const result = await native<Diagnostics>('system_check');
      setDiagnostics(result); setAdapter(a => result.adapters.some(x => x.name === a && x.mode === 'managed') ? a : result.adapters.find(x => x.mode === 'managed')?.name || '');
      setCaptureAdapter(a => result.adapters.some(x => x.name === a) ? a : result.adapters.find(x => !x.connected && (x.mode === 'managed' || x.mode === 'monitor'))?.name || result.adapters.find(x => x.mode === 'managed')?.name || '');
      log(`Dependency check finished: ${result.tools.filter(x => x.available).length}/${result.tools.length} available.`, 'OK');
      if (result.errors.length) log(result.errors.join(' / '), 'WARN');
      else if (result.tools.every(tool => tool.available)) celebrate();
    } catch (e) { fail(e); } finally { setChecking(false); }
  }
  useEffect(() => {
    alive.current = true;
    const timer = window.setInterval(() => setClock(stamp()), 1000);
    if (isDesktop()) void checkSystem();
    return () => { alive.current = false; window.clearInterval(timer); if (demoTimer.current) clearInterval(demoTimer.current); if (scanTimer.current) clearTimeout(scanTimer.current); };
  }, []); // Native system inventory is read once on startup and on explicit refresh.
  useEffect(() => { document.documentElement.lang = language; }, [language]);
  useEffect(() => { if (lab.running) setSkull('lab'); }, [lab.running, setSkull]);
  useEffect(() => {
    if (!isDesktop()) return;
    let disposed = false; let unsubscribe: (() => void) | undefined;
    void import('@tauri-apps/api/event').then(({ listen }) => listen<string>('shutdown-error', event => fail(event.payload))).then(stop => { if (disposed) stop(); else unsubscribe = stop; });
    return () => { disposed = true; unsubscribe?.(); };
  }, []);

  useEffect(() => { setAuthorized(false); }, [selected, captureAdapter, source]);
  useEffect(()=>{setAllowDisconnect(false);},[captureAdapter,source]);
  useEffect(() => { if (page !== 'lab') { setKnownPassword(''); setWordlist(''); setWordlistName('No wordlist selected'); } }, [page]);
  useEffect(() => {
    if (source !== 'live' || (!capture.running && !lab.running)) return;
    let cancelled = false;
    let pending = false;
    const timer = window.setInterval(async () => {
      if (pending) return; pending = true;
      try {
        if (capture.running) { const result = await native<Capture>('capture_status'); if (!cancelled && !privacyStopping.current) { setCapture(result); if (!result.running) { setSkull(result.error ? 'error' : 'idle'); log(result.error || `Observation ended: ${result.count} frames. ${result.stopReason || ''}${result.exchanges.some(e => e.complete) ? '' : ' No complete EAPOL exchange observed.'}`, result.error ? 'WARN' : 'OK'); } } }
        if (lab.running) { const result = await native<LabStatus>('lab_status'); if (!cancelled && !privacyStopping.current) { setLab(result); if (!result.running) { setSkull(result.exitCode === 0 ? 'success' : 'disappointed'); log(`Hashcat: ${result.status}; exit ${result.exitCode ?? 'unknown'}.`); } } }
      } catch (e) { if (!cancelled && !privacyStopping.current) { fail(e); /* Keep polling: an IPC error does not prove that a process stopped. */ } }
      finally { pending = false; }
    }, 900);
    return () => { cancelled = true; clearInterval(timer); };
  }, [source, capture.running, lab.running]);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || panic.recording || panic.running) return;
      if (event.target instanceof HTMLElement && (['INPUT', 'SELECT', 'TEXTAREA'].includes(event.target.tagName) || event.target.isContentEditable)) return;
      if (event.ctrlKey || event.metaKey || event.altKey || modal) return;
      const index = event.key==='0'?9:Number(event.key) - 1;
      if (index >= 0 && index < navigation.length) setPage(navigation[index].id);
      if (event.key.toLowerCase() === 'r' && page !== 'academy' && !scan && active) void startScan();
      if (event.key === '?') setModal('help');
    };
    window.addEventListener('keydown', onKey); return () => window.removeEventListener('keydown', onKey);
  });
  async function startSession() {
    await action(async () => {
      const result = source === 'demo' ? { id: `demo-${Date.now()}`, policy, state: 'ACTIVE', path: null, files: [] } : await native<Session>('session_start', { policy });
      setSession(result); setCapture(emptyCapture); setLab(emptyLab); log(`${source === 'demo' ? 'DEMO ' : ''}Session opened / ${policy}`, 'OK'); setSkull('idle');
    });
  }
  async function startScan() {
    if (!active || scan || capture.running) return;
    const generation = ++scanGeneration.current;
    setScan(true); setSkull('scanning'); setError(''); log(source === 'demo' ? 'DEMO: loading synthetic discovery fixture.' : 'NetworkManager discovery requested.');
    try {
      let found: Network[];
      if (source === 'demo') { await new Promise<void>(resolve => { scanTimer.current = setTimeout(resolve, 900); }); found = demoNetworks; }
      else found = (await discoverLive(adapter)).networks;
      if (!alive.current || generation !== scanGeneration.current) return;
      setNetworks(found); setSelected(value => found.some(n => n.bssid === value) ? value : found[0]?.bssid || '');
      log(`${source === 'demo' ? 'DEMO: ' : ''}${found.length} access points returned.`, 'OK'); setSkull(found.length ? 'network' : 'grin');
    } catch (e) { if (generation === scanGeneration.current) fail(e); }
    finally { if (generation === scanGeneration.current) setScan(false); }
  }
  async function cancelScan() {
    ++scanGeneration.current; setScan(false); setSkull('idle');
    if (source === 'live') { try { await native('cancel_discovery'); } catch (e) { fail(e); } }
    log('Discovery cancelled in DEAD//AIR. An OS scan already requested may finish in NetworkManager.');
  }
  async function endSession() {
    await action(async () => {
      if (scan) await cancelScan();
      if (demoTimer.current) clearInterval(demoTimer.current);
      setSkull('cleanup');
      const ended = source === 'demo' ? { ...session!, state: 'CLOSED', files: [] } : await native<Session>('session_end');
      setSession(ended); setNetworks([]); setSelected(''); setCapture(emptyCapture); setLab(emptyLab); setLogs([]); setModal(null); setSkull('shutdown');
      log(`Session closed. ${policy === 'PERSISTENT' ? 'Saved files retained.' : 'App-managed session data released.'}`, 'OK');
    });
  }
  async function changeSource(next: Source) {
    if (active || scan || capture.running || lab.running) { setError('End the current session before changing data source.'); return; }
    setSource(next); setNetworks([]); setSelected(''); setSession(null); setCapture(emptyCapture); setLab(emptyLab); setError(''); setSkull('idle');
    setLogs([]); log(next === 'demo' ? 'DEMO MODE enabled explicitly. All radio data is synthetic.' : 'LIVE MODE selected. No synthetic observations.'); setModal(null);
  }
  async function startCapture() {
    if (!selectedNetwork || !authorized || !active) return;
    await action(async () => {
      if (source === 'demo') {
        setCapture({ ...emptyCapture, running: true, status: 'DEMO REPLAY' }); setSkull('scanning');
        let step = 0;
        demoTimer.current = setInterval(() => {
          step++;
          setCapture(previous => ({ ...previous, running: step < 4, status: step < 4 ? 'DEMO / EXCHANGE PARTIAL' : 'DEMO / SEQUENCE OBSERVED', count: step,
            packets: [...previous.packets, { time: Date.now() / 1000, source: step % 2 ? selectedNetwork.bssid : '02:DA:CC:00:00:01', destination: step % 2 ? '02:DA:CC:00:00:01' : selectedNetwork.bssid, station: '02:DA:CC:00:00:01', kind: 'EAPOL-Key', message: step, replay: step < 3 ? '1' : '2', signal: null, bytes: 155 }],
            exchanges: [{ station: '02:DA:CC:00:00:01', messages: Array.from({ length: step }, (_, i) => i + 1), complete: step === 4 }] }));
          if (step === 4) { clearInterval(demoTimer.current!); setSkull('grin'); log('DEMO replay finished. No packets captured.', 'OK'); }
        }, 800);
      } else {
        try {
          const result = await native<Capture>('capture_start', { request: { adapter: captureAdapter, bssid: selectedNetwork.bssid, frequency: selectedNetwork.frequency, authorized, allowDisconnect } });
          setCapture(result); setSkull('scanning'); log('Passive capture helper started; waiting for actual frames.');
        } catch (e) {
          setCapture(c => ({ ...c, running: false, status: 'PREFLIGHT FAILED', error: errorMessage(e) }));
          throw e;
        }
      }
    });
  }
  async function stopCapture() {
    await action(async () => {
      if (source === 'demo') { if (demoTimer.current) clearInterval(demoTimer.current); setCapture(c => ({ ...c, running: false, status: 'DEMO STOPPED' })); }
      else { const stopped=await native<Capture>('capture_stop'); setCapture(stopped); await checkSystem(); if(stopped.error) throw stopped.error; }
      setSkull('idle'); log('Capture stop completed.');
    });
  }
  async function exportReport() {
    await action(async () => {
      const report = reportFor(selectedNetwork ? [selectedNetwork] : networks, source);
      if (source === 'demo' || !isDesktop()) {
        const url = URL.createObjectURL(new Blob([JSON.stringify(report, null, 2)], { type: 'application/json' }));
        const a = document.createElement('a'); a.href = url; a.download = 'dead-air-demo-report.json'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
        log('DEMO report download requested.');
      } else { const path = await native<string>('report_export', { report: JSON.stringify(report, null, 2) }); log(`Report written: ${path}`, 'OK'); }
      setModal(null);
    });
  }
  const selectNetwork = (network: Network) => { setSelected(network.bssid); setPage('inspector'); };
  const dependencyReady = (name: string) => diagnostics?.tools.some(tool => tool.name === name && tool.available);
  const sessionControls = <div className="session-start"><label>{t('Retention policy', 'Хранение данных')}<select value={policy} disabled={active || busy} onChange={e => setPolicy(e.target.value as Policy)}><option value="MEMORY_ONLY">MEMORY ONLY</option><option value="RAM_ONLY">RAM SESSION / TMPFS</option><option value="TEMPORARY">TEMPORARY</option><option value="PERSISTENT">PERSISTENT</option></select></label><button className="primary" disabled={busy || active || (source === 'live' && !isDesktop())} onClick={() => void startSession()}>{t('START SESSION', 'НАЧАТЬ СЕССИЮ')} ↗</button></div>;

  return <div className={`app ${scanlines ? 'crt' : ''} ${motion ? 'motion' : 'no-motion'}`} style={{ '--glow': `${glow / 100 * 8}px`, '--font-scale': fontScale / 100 } as CSSProperties}>
    <header className="topbar"><a href="#" className="brand" onClick={e => { e.preventDefault(); setPage('dashboard'); }}>DEAD<span>//</span>AIR<span className="version">v0.3.3</span></a><div className="top-descriptor">WIRELESS SECURITY WORKSTATION <span> / </span> TERMINAL 01</div>{panic.button}<div className="top-state"><i className={source === 'demo' ? 'amber-dot' : ''} />{page === 'academy' ? 'TRAINING MODE' : source === 'demo' ? 'DEMO MODE' : 'LIVE MODE'}<time>{clock}</time></div></header>
    <div className="workspace"><aside className="sidebar"><div className="sidebar-label">MAIN DIRECTORY</div><nav aria-label="Main navigation">{navigation.map((item, i) => <button key={item.id} className={page === item.id ? 'active' : ''} onClick={() => setPage(item.id)} aria-current={page === item.id ? 'page' : undefined}><span className="nav-icon">{item.icon}</span><span>{language === 'ru' ? item.ru : item.title}</span><small>{String(i + 1).padStart(2, '0')}</small></button>)}</nav><div className="sidebar-bottom"><div className="mini-wave" aria-hidden="true">▁▂▁▃▅▂▁▇▃▁▂▅▃▁▂▁</div><span className="tiny">SESSION POLICY</span><strong>{session?.policy.replaceAll('_', ' ') || 'NOT STARTED'}</strong><span className="tiny">{active ? '● SESSION ACTIVE' : '○ STANDBY'}</span><button className="danger" disabled={!active || busy} onClick={() => setModal('end')}>⏻ {t('END SESSION', 'ЗАКРЫТЬ СЕССИЮ')}</button><button className="text-button" onClick={() => setModal('help')}>[ ? ] KEYBOARD HELP</button></div></aside>
    <main><div className="page-heading"><div><div className="eyebrow">DEAD/AIR :~/{page}</div><h1>{navigation.find(n => n.id === page)?.[language === 'ru' ? 'ru' : 'title']}<span className="caret">_</span></h1></div><div className="page-actions"><span className="source-badge">{page === 'academy' ? 'EDUCATIONAL SIMULATIONS' : source === 'demo' ? 'SYNTHETIC DATA' : isDesktop() ? 'NATIVE BACKEND' : 'BROWSER / NO HARDWARE'}</span><button onClick={() => source === 'live' ? setModal('demo') : void changeSource('live')} disabled={active || busy}>{source === 'demo' ? 'SWITCH TO LIVE' : 'ENTER DEMO'}</button></div></div>
    {error && <div className="error-banner" role="alert"><span>[!] {error}</span><button aria-label="Dismiss error" onClick={() => { setError(''); setSkull('idle'); }}>×</button></div>}
    {source === 'demo' && <div className="demo-banner">DEMO MODE // Synthetic networks and replayed events. No hardware operations.</div>}
    <LearningShelf page={page} onOpen={id=>{academy.setSelected(id);setPage('academy');}}/>
    {page==='academy'&&<Academy state={academy} onReturn={setPage}/>}
    {page === 'dashboard' && <>
      <div className="hero-grid"><Panel title="SYSTEM // DEAD AIR" meta={scan ? 'DISCOVERY IN PROGRESS' : 'AWAITING OPERATOR'} className="hero-panel"><div className="hero-art"><div className="coordinates">RX / {source.toUpperCase()}<br />ASCII ENGINE 01</div><SkullAnimator state={skull} motion={motion} replayKey={reactionId} /><div className="hero-index">[ D/A ]<br />NO SIGNAL IS SILENCE.</div></div><div className="hero-wordmark">DEAD<span>//</span>AIR</div><ArchiveFragment enabled={motion && skull === 'idle'} /><div className="hero-caption">WIRELESS SECURITY · PASSIVE DIAGNOSTICS</div><div className="hero-baseline"><span>CHARACTER CELL RENDERER</span><span className={skull === 'error' ? 'danger-text' : ''}>[ {skull.toUpperCase()} ]</span></div></Panel>
      <div className="system-column"><Panel title="01 / SYSTEM STATUS" meta={<span className="status-dot">●</span>}><dl className="key-values"><dt>Runtime</dt><dd>{isDesktop() ? diagnostics?.platform || 'Checking…' : 'Browser preview'}</dd><dt>Interface</dt><dd>{adapter || 'Not selected'}</dd><dt>NetworkManager</dt><dd>{dependencyReady('NetworkManager') ? 'AVAILABLE' : 'UNAVAILABLE'}</dd><dt>Capture helper</dt><dd>{dependencyReady('Capture helper') ? 'FOUND' : 'NOT DETECTED'}</dd><dt>Hashcat</dt><dd>{dependencyReady('Hashcat') ? 'FOUND' : 'NOT DETECTED'}</dd><dt>Session</dt><dd>{session?.state || 'NEW'}</dd></dl><button className="wide" disabled={checking} onClick={() => void checkSystem()}>{checking ? 'CHECKING…' : '[ RECHECK DEPENDENCIES ]'}</button></Panel><Panel title="02 / OPERATOR CONSOLE"><p className="body-copy">{t('Observe the air. Know your network.', 'Наблюдай за эфиром. Изучай свою сеть.')}</p><p className="muted small">{t('Select a session policy before starting. Radio observations are always tied to their data source.', 'Выбери режим хранения перед началом. Источник данных всегда указан.')}</p>{active ? <div className="button-row"><button className="primary" disabled={scan || busy} onClick={() => { setPage('discover'); void startScan(); }}>⌁ {t('DISCOVER NETWORKS', 'НАЙТИ СЕТИ')}</button></div> : sessionControls}<div className="console-note">{policy === 'RAM_ONLY' ? 'Verified tmpfs · files and Hashcat workspace in RAM · no disk fallback' : policy === 'MEMORY_ONLY' ? 'RAM only · no intentional session files' : policy === 'TEMPORARY' ? 'Private session directory · cleanup on close' : 'Explicitly retained session files'}</div></Panel></div></div>
      <div className="metrics"><div><span>ACCESS POINTS</span><strong>{String(networks.length).padStart(2, '0')}<small> OBSERVED</small></strong></div><div><span>RADIO BANDS</span><strong>{String(new Set(networks.map(n => bandOf(n.frequency))).size).padStart(2, '0')}<small> IN RESULTS</small></strong></div><div><span>CAPTURED FRAMES</span><strong>{String(capture.count).padStart(4, '0')}<small>{source === 'demo' ? ' REPLAY' : ' ACTUAL'}</small></strong></div><div><span>RETENTION</span><strong className="metric-text">{session?.policy.replaceAll('_', ' ') || 'NOT SET'}</strong></div></div>
      <div className="lower-grid"><Panel title="03 / DISCOVERY SNAPSHOT" meta={<button className="text-button" onClick={() => setPage('discover')}>OPEN DISCOVER ↗</button>}><NetworkTable compact networks={filterNetworks(networks, '', 'all', 'signal').slice(0, 4)} selected={selected} onSelect={selectNetwork} /></Panel><Panel title="04 / CHANNEL DISTRIBUTION" meta="2.4 GHz"><ChannelChart networks={networks} /><div className="chart-caption">Observed AP count · not airtime utilization</div></Panel></div>
    </>}
    {page === 'discover' && <><div className="toolbar"><label className="search"><span>⌕</span><input aria-label="Filter networks" value={query} onChange={e => setQuery(e.target.value)} placeholder="Filter SSID or BSSID…" /></label><select aria-label="Band filter" value={band} onChange={e => setBand(e.target.value)}><option value="all">ALL BANDS</option><option>2.4 GHz</option><option>5 GHz</option><option>6 GHz</option></select><select aria-label="Sort networks" value={sort} onChange={e => setSort(e.target.value)}><option value="signal">SIGNAL ↓</option><option value="name">SSID A–Z</option><option value="channel">CHANNEL ↑</option></select><button className="primary" disabled={!active || busy || capture.running} onClick={() => void (scan ? cancelScan() : startScan())}>{scan ? '■ CANCEL' : '⌁ SCAN NETWORKS'}</button></div>{!active && <div className="notice">Start a session in Overview or Sessions to discover networks.</div>}<Panel title="NETWORK DISCOVERY" meta={`${visibleNetworks.length} ACCESS POINTS / ${source.toUpperCase()}`}><NetworkTable networks={visibleNetworks} selected={selected} masked={masked} onSelect={selectNetwork} /><div className="panel-footer">{scan ? 'Request in progress…' : 'Select a network to open inspector'}<span>[R] RESCAN · BSSIDs {masked ? 'MASKED' : 'VISIBLE'}</span></div></Panel></>}
    {page === 'inspector' && (selectedNetwork ? <><div className="inspector-top"><Panel title="TARGET // ACCESS POINT" meta={source.toUpperCase()}><h2 className="ssid-title">{cleanSsid(selectedNetwork.ssid)}</h2><dl className="key-values"><dt>BSSID</dt><dd>{masked ? maskBssid(selectedNetwork.bssid) : selectedNetwork.bssid}</dd><dt>Channel / band</dt><dd>{selectedNetwork.channel} / {bandOf(selectedNetwork.frequency)}</dd><dt>Frequency</dt><dd>{selectedNetwork.frequency} MHz</dd><dt>Signal quality</dt><dd><Signal quality={selectedNetwork.quality} /></dd><dt>Advertised security</dt><dd>{selectedNetwork.security}</dd><dt>Cipher / PMF</dt><dd>{selectedNetwork.cipher} / {selectedNetwork.pmf}</dd></dl><div className="button-row"><button className="primary" onClick={() => setPage('capture')}>◎ PASSIVE OBSERVATION</button><button onClick={() => setModal('export')}>EXPORT REPORT</button></div></Panel><Panel title="PROTOCOL NOTES"><div className="large-glyph" aria-hidden="true">(( ⌁ ))</div><p className="body-copy">{selectedNetwork.security.includes('WPA3') ? 'SAE has different security properties from WPA2-PSK. A four-way exchange is not a conventional passive offline password-verification opportunity for SAE.' : selectedNetwork.security.includes('Enterprise') ? 'Enterprise authentication uses 802.1X/EAP. Advertisements do not reveal credentials or validate server certificate policy.' : 'Discovery reports advertised capabilities. It does not establish password strength, router configuration, or whether a network is secure.'}</p><p className="muted small">RSSI is not distance. Signal quality does not identify a physical location.</p></Panel></div><Panel title="DEFENSIVE SECURITY REVIEW" meta="EVIDENCE-BASED CHECKLIST"><div className="review-list">{review(selectedNetwork).map(item => <div key={item.title}><span className={`review-status ${item.status === 'WARN' ? 'danger-text' : ''}`}>[{item.status}]</span><section><h3>{item.title}</h3><p>{item.detail}</p></section></div>)}</div></Panel></> : <Panel title="NETWORK INSPECTOR"><div className="empty"><span>[ SELECT AN ACCESS POINT ]</span><p>No target selected. Open discovery and select an exact BSSID.</p><button className="primary" onClick={() => setPage('discover')}>OPEN DISCOVERY ↗</button></div></Panel>)}
    {page === 'spectrum' && <><div className="two-column"><Panel title="CHANNEL DISTRIBUTION // 2.4 GHz"><ChannelChart networks={networks} /></Panel><Panel title="CHANNEL DISTRIBUTION // 5 GHz"><ChannelChart networks={networks} band="5 GHz" /></Panel></div><Panel title="SIGNAL QUALITY" meta="LATEST DISCOVERY SNAPSHOT"><div className="signal-list">{networks.map(n => <button key={n.bssid} onClick={() => selectNetwork(n)}><span>{cleanSsid(n.ssid)}</span><div><i style={{ width: `${n.quality}%` }} /></div><b>{n.quality}%</b></button>)}{!networks.length && <div className="empty">No measurements available.</div>}</div><div className="panel-footer">AP counts and latest signal quality only. Spectrum analysis / airtime utilization unavailable.</div></Panel></>}
    {page === 'capture' && <><div className="two-column"><Panel title="PASSIVE OBSERVATION // OWNED LAB" meta={capture.running ? '● LISTENING' : '○ IDLE'}><dl className="key-values"><dt>Target</dt><dd>{selectedNetwork ? cleanSsid(selectedNetwork.ssid) : 'Select an AP in discovery'}</dd><dt>Exact BSSID</dt><dd>{selectedNetwork?.bssid || '—'}</dd><dt>Frequency</dt><dd>{selectedNetwork ? `${selectedNetwork.frequency} MHz` : '—'}</dd></dl><label className="field-label">Capture adapter · built-in or USB<select value={captureAdapter} disabled={capture.running} onChange={e => setCaptureAdapter(e.target.value)}><option value="">Select adapter</option>{diagnostics?.adapters.map(a => <option key={a.name} value={a.name}>{a.name} / {a.phy} / {a.connected ? 'CONNECTED' : a.mode}</option>)}{source === 'demo' && <option value="demo0">demo0 / SYNTHETIC</option>}</select></label><p className="muted small">{diagnostics?.adapters.find(a => a.name === captureAdapter)?.mode === 'monitor' ? 'Existing monitor interface: reused directly without creating a second interface. Original channel and link state are restored; the card remains in monitor mode.' : 'The selected interface itself is used for capture. Its original managed mode and saved connection are restored on stop.'}</p><label className="checkbox capture-disconnect"><input type="checkbox" checked={allowDisconnect} disabled={capture.running} onChange={e => setAllowDisconnect(e.target.checked)} /><span>I allow temporary Wi-Fi disconnection and use of this same card in monitor mode. The saved connection will be restored after capture; Internet and remote sessions using this radio will be interrupted.</span></label><label className="checkbox"><input type="checkbox" checked={authorized} disabled={capture.running} onChange={e => setAuthorized(e.target.checked)} /><span>I own this exact AP and have consent from the participating clients.</span></label>{capture.error && <div className="capture-error" role="alert"><strong>OBSERVATION FAILED</strong><pre>{capture.error}</pre><p>Keep these details when reporting a problem. Refresh adapters after an external mode change. If restoration is incomplete, use ADAPTERS → RESTORE STALE CAPTURE INTERFACES.</p></div>}<div className="button-row"><button className="primary" disabled={!active || !authorized || !selectedNetwork || capture.running || scan || busy || (source === 'live' && (!captureAdapter || !!diagnostics?.adapters.find(a => a.name === captureAdapter)?.connected && !allowDisconnect))} onClick={() => void startCapture()}>{source === 'demo' ? '▶ REPLAY DEMO' : '▶ START OBSERVATION'}</button><button className="danger" disabled={!capture.running || busy} onClick={() => void stopCapture()}>■ STOP</button><button disabled={capture.running || busy || checking || source === 'demo'} onClick={() => void checkSystem()}>REFRESH CAPTURE ADAPTERS</button></div><p className="muted small">Wait for a consenting client to connect normally. An exchange may be partial or absent. Built-in cards require monitor-mode driver support. With disconnection consent, the selected card is temporarily released from NetworkManager and its saved profile is reactivated afterward. Reconnection can fail if the AP or credentials are unavailable.</p></Panel><Panel title="EAPOL // FOUR-WAY EXCHANGE" meta={source.toUpperCase()}><div className="exchange-diagram"><span>CLIENT</span><span>ACCESS POINT</span>{[1, 2, 3, 4].map(m => <div key={m} className={capture.exchanges.some(x => x.messages.includes(m)) ? 'observed' : ''}><b>M{m}</b><span>{m % 2 ? '◀──────────────────' : '──────────────────▶'}</span><small>{capture.exchanges.some(x => x.messages.includes(m)) ? 'OBSERVED' : capture.running ? 'NOT OBSERVED YET' : 'NOT OBSERVED'}</small></div>)}</div><div className="capture-state">{capture.status}</div><p className="muted small">Sequence presence does not cryptographically verify key possession. Replay counters and stations are tracked separately.</p>{capture.exchanges.map(x => <div className="station-result" key={x.station}>{masked ? maskBssid(x.station) : x.station}: {x.complete ? 'M1–M4 correlated sequence' : `Partial / ${x.messages.map(m => `M${m}`).join(' ')}`}</div>)}</Panel></div><CaptureHealth capture={capture} demo={source === 'demo'} adapter={diagnostics?.adapters.find(a => a.name === captureAdapter)} /><Panel title="PACKET MONITOR" meta={`${capture.count} FRAMES · ${capture.dropped} NOT RETAINED`}><div className="table-scroll"><table><thead><tr><th>TIME</th><th>SOURCE</th><th>DESTINATION</th><th>TYPE</th><th>INFO</th></tr></thead><tbody>{capture.packets.slice(-100).map((packet, i) => <tr key={`${packet.time}-${i}`}><td>{new Date(packet.time * 1000).toLocaleTimeString('en-GB')}</td><td>{masked ? maskBssid(packet.source) : packet.source}</td><td>{masked ? maskBssid(packet.destination) : packet.destination}</td><td>{packet.kind}</td><td>{packet.message ? `M${packet.message} / replay ${packet.replay}` : `${packet.bytes} bytes`}</td></tr>)}</tbody></table>{!capture.packets.length && <div className="empty">No packet events. No timer-generated traffic in live mode.</div>}</div><div className="panel-footer"><span>Bounded in-memory buffer · selected BSSID only</span><button disabled={source === 'demo' || capture.running || !capture.count || policy === 'MEMORY_ONLY' || busy} onClick={() => void action(async () => { const path = await native<string>('capture_export'); log(`PCAPNG written: ${path}`, 'OK'); setSession(await native<Session>('session_status')); })}>SAVE PCAPNG</button></div><div className="console-note">Export requires a temporary or persistent session and may contain sensitive authentication material. Exported captures are never passed to Hashcat.</div></Panel></>}
    {page === 'capture' && <Panel title="PMKID // PASSIVE OBSERVATIONS" meta={`${capture.pmkids?.length || 0} UNIQUE / NOT VERIFIED`}><p className="muted small">RSN association lists and unencrypted EAPOL key data only. A PMKID is not a password or proof of WPA2-PSK vulnerability. Active PMKID elicitation is available only as a teaching case in Academy.</p>{capture.pmkids?.map(p => <div className="pmkid-observation" key={`${p.station}-${p.value}`}><code>{masked ? maskBssid(p.station) : p.station}</code><span>{p.source}</span><code>{p.value}</code></div>)}{!capture.pmkids?.length && <div className="empty">No PMKID observed in actual frames.</div>}</Panel>}
    {page === 'adapter' && <MacControls adapters={diagnostics?.adapters || []} demo={source === 'demo'} captureRunning={capture.running} onChanged={() => void checkSystem()} />}
    {page === 'adapter' && <><div className="toolbar"><span>Read-only inventory / scoped capture preflight</span><button className="primary" disabled={checking} onClick={() => void checkSystem()}>{checking ? 'CHECKING…' : 'REFRESH ADAPTERS'}</button></div><div className="adapter-grid">{diagnostics?.adapters.map(a => <Panel key={a.name} title={`INTERFACE // ${a.name}`} meta={a.connected ? 'CONNECTED' : 'AVAILABLE'}><dl className="key-values"><dt>Driver</dt><dd>{a.driver}</dd><dt>Radio</dt><dd>{a.phy}</dd><dt>Mode</dt><dd>{a.mode}</dd><dt>Monitor capability</dt><dd>{a.monitorSupported ? 'Advertised' : 'Not reported'}</dd><dt>Bands</dt><dd>{a.bands.join(', ') || 'Unknown'}</dd><dt>Frequency</dt><dd>{a.frequency ? `${a.frequency} MHz` : 'Unknown'}</dd></dl><div className="button-row"><button disabled={a.mode === 'monitor' || scan} onClick={() => { setAdapter(a.name); log(`Discovery interface selected: ${a.name}`); }}>USE FOR DISCOVERY</button><button disabled={capture.running} onClick={() => { setCaptureAdapter(a.name); setPage('capture'); }}>USE FOR CAPTURE</button></div></Panel>)}</div>{!diagnostics?.adapters.length && <Panel title="RADIO INVENTORY"><div className="empty">No verified Linux radio interfaces available.<p>Run the desktop application and check dependencies.</p></div></Panel>}<Panel title="AIRCRACK-NG // ADAPTER DIAGNOSTICS"><div className="button-row"><button disabled={busy || source === 'demo'} onClick={() => void action(async () => setAdapterReport(await native<string>('adapter_report')))}>AIRMON-NG REPORT</button><button disabled={busy || capture.running || source === 'demo'} onClick={() => void action(async () => { const result = await native<string>('recover_capture'); log(result, 'OK'); await checkSystem(); })}>RESTORE STALE CAPTURE INTERFACES</button></div>{adapterReport && <pre className="process-output">{adapterReport}</pre>}</Panel><Panel title="ADAPTER LIFECYCLE"><p className="body-copy">Discovery leaves NetworkManager in control. Capture uses the selected card itself. Connected cards require temporary-disconnection consent; managed mode and the saved profile are restored afterward. Existing monitor interfaces keep their original mode.</p><p className="muted small">The privileged helper journals its changes in /run/deadair. Existing monitor channels and link state are recorded before capture. After a forced helper termination, use RESTORE STALE CAPTURE INTERFACES above. Recovery checks interface identity before changing anything.</p></Panel></>}
    {page === 'sessions' && <><div className="two-column"><Panel title="SESSION CONTROL" meta={session?.state || 'NEW'}>{sessionControls}<dl className="key-values"><dt>Session ID</dt><dd className="break">{session?.id || '—'}</dd><dt>Storage</dt><dd className="break">{session?.path || 'No session directory'}</dd><dt>Owned files</dt><dd>{session?.files.length || 0}</dd></dl><button className="danger" disabled={!active || busy} onClick={() => setModal('end')}>END SESSION / CLEANUP</button></Panel><Panel title="RETENTION POLICY"><div className="retention-notes"><h3>01 / MEMORY ONLY</h3><p>No intentional session files. Capture stays in bounded RAM; Hashcat and exports require a disk-backed policy.</p><h3>02 / TEMPORARY</h3><p>Private directory and explicit file manifest. App-owned files removed on normal close; stale sessions can be reviewed after a crash.</p><h3>03 / PERSISTENT</h3><p>Files remain after close. Export is an explicit action. This policy does not promise trace-free operation or irreversible erasure.</p></div></Panel></div><Panel title="FILE MANIFEST" meta={<button disabled={!active || source === 'demo' || busy} onClick={() => void action(async () => setSession(await native<Session>('session_status')))}>REFRESH</button>}>{session?.files.length ? <ul className="file-list">{session.files.map(file => <li key={file}>{file}</li>)}</ul> : <div className="empty">No app-managed files in this session.</div>}</Panel><Panel title="CRASH RECOVERY" meta={<button disabled={source === 'demo' || busy} onClick={() => void action(async () => setStale(await native<Session[]>('session_recover')))}>FIND STALE TEMP SESSIONS</button>}>{stale.length ? stale.map(s => <div className="recovery-row" key={s.id}><span>{s.id}<small>{s.path} / {s.files.join(', ')}</small></span><button className="danger" disabled={busy} onClick={() => void action(async () => { await native('session_purge', { id: s.id }); setStale(list => list.filter(x => x.id !== s.id)); log(`Removed app-owned stale session ${s.id}`, 'OK'); })}>PURGE LISTED FILES</button></div>) : <div className="empty">No stale sessions listed. Run recovery to inspect app-owned temporary data.</div>}</Panel></>}
    {page === 'lab' && <><div className="two-column"><Panel title="HASHCAT // OFFLINE LAB" meta={source === 'demo' ? 'DISABLED IN DEMO' : lab.status}><p className="body-copy">Native benchmarks and owner-generated SHA-256 known-answer tests.</p><p className="muted small">No wireless capture imports. Hashes are generated locally from the test password you provide. Compute jobs can create high CPU/GPU load.</p><label className="checkbox"><input type="checkbox" checked={labConsent} onChange={e => setLabConsent(e.target.checked)} /><span>I authorize this local lab job and understand the compute load.</span></label><div className="button-row"><button className="primary" disabled={!active || policy === 'MEMORY_ONLY' || !labConsent || source === 'demo' || lab.running || busy} onClick={() => void action(async () => setLab(await native<LabStatus>('lab_start', { request: { kind: 'benchmark', authorized: labConsent, password: null, wordlist: null } })))}>RUN BENCHMARK</button><button className="danger" disabled={!lab.running || busy} onClick={() => void action(async () => setLab(await native<LabStatus>('lab_stop')))}>STOP</button><button disabled={source === 'demo' || busy} onClick={() => void action(async () => { const result = await native<string>('hashcat_devices'); setLab(l => ({ ...l, lines: result.split('\n') })); })}>DEVICES</button><button disabled={source === 'demo' || busy || lab.running} onClick={() => { setSetupProfile('cpu'); setSetupRequest(n => n + 1); }}>INSTALL CPU RUNTIME</button></div><div className="console-note">Requires RAM SESSION, temporary or persistent storage. Hashcat caches are redirected into the session directory.</div></Panel><Panel title="KNOWN-ANSWER TEST VECTOR"><label className="field-label">Test password (never logged)<input autoComplete="off" type="password" value={knownPassword} maxLength={128} onChange={e => setKnownPassword(e.target.value)} /></label><label className="field-label">Local wordlist (max. 4 MiB)<input type="file" accept=".txt" onChange={e => { const file = e.target.files?.[0]; if (!file) return; if (file.size > 4 * 1024 * 1024) { fail('Wordlist exceeds 4 MiB lab limit.'); return; } void file.text().then(text => { setWordlist(text); setWordlistName(file.name); }).catch(fail); }} /></label><p className="muted small">{wordlistName}</p><button disabled={!active || policy === 'MEMORY_ONLY' || !labConsent || !knownPassword || !wordlist || source === 'demo' || lab.running || busy} onClick={() => void action(async () => { const password = knownPassword; setKnownPassword(''); setLab(await native<LabStatus>('lab_start', { request: { kind: 'known-answer', authorized: labConsent, password, wordlist } })); setWordlist(''); setWordlistName('Wordlist released'); })}>START KNOWN-ANSWER JOB ↗</button></Panel></div><Panel title="PROCESS OUTPUT" meta={lab.exitCode === null ? 'NO EXIT CODE' : `EXIT ${lab.exitCode}`}><pre className="process-output">{lab.lines.join('\n') || 'No Hashcat process output. Throughput and device statistics will appear only when reported by the installed tool.'}</pre><div className="panel-footer">{lab.status}<span>PLAINTEXT CANDIDATES AND HASHES ARE NOT DISPLAYED</span></div></Panel></>}
    {page === 'settings' && <>{panic.settings}<PrivilegedControls /></>}
    {page === 'settings' && <><div className="two-column"><Panel title="APPEARANCE // CRT"><label className="checkbox"><input type="checkbox" checked={scanlines} onChange={e => setScanlines(e.target.checked)} />Phosphor scanlines</label><label className="checkbox"><input type="checkbox" checked={motion} onChange={e => setMotion(e.target.checked)} />Animations and skull motion</label><label className="checkbox"><input type="checkbox" checked={sound} onChange={e => setSound(e.target.checked)} />Electrical ambience and event sounds (quiet)</label><label className="field-label">Bloom intensity / {glow}%<input aria-label="Bloom intensity" type="range" min="0" max="100" value={glow} onChange={e => setGlow(Number(e.target.value))} /></label><label className="field-label">Font scale / {fontScale}%<input aria-label="Font scale" type="range" min="90" max="125" value={fontScale} onChange={e => setFontScale(Number(e.target.value))} /></label><div className="button-row"><button onClick={() => { setScanlines(false); setGlow(0); setMotion(false); setFontScale(100); setSound(false); }}>RESET VISUAL EFFECTS</button>{import.meta.env.DEV && <a href="/?skull-preview=1">CHARACTER PREVIEW ?</a>}</div><div className="skull-preview"><SkullAnimator state={skull} motion={motion} replayKey={reactionId} /></div></Panel><div><Panel title="PREFERENCES"><label className="field-label">Interface language<select value={language} onChange={e => setLanguage(e.target.value as 'en' | 'ru')}><option value="en">English</option><option value="ru">Русский (navigation)</option></select></label><label className="checkbox"><input type="checkbox" checked={masked} onChange={e => setMasked(e.target.checked)} />Mask radio identifiers in tables</label><p className="muted small">Visual preferences stay in memory. No browser storage or telemetry is used.</p></Panel><Panel title="KISMET // LOCAL SERVICE"><label className="field-label">API token · memory only<input type="password" autoComplete="off" value={kismetToken} onChange={e => setKismetToken(e.target.value)} /></label><button disabled={!kismetToken || busy || source === 'demo'} onClick={() => void action(async () => { const token = kismetToken; setKismetToken(''); setKismetResult(await native<string>('kismet_check', { token })); })}>CHECK LOCAL API</button><p className="muted small break">{kismetResult}</p><div className="console-note">127.0.0.1:2501 · authenticated status only</div></Panel></div></div><Panel title="DEPENDENCY INVENTORY" meta={<span><button disabled={checking} onClick={() => void checkSystem()}>RECHECK</button> <button onClick={() => { setSetupProfile('all'); setSetupRequest(n => n + 1); }}>INSTALL MISSING TOOLS</button></span>}><div className="dependency-list">{diagnostics?.tools.map(tool => <div key={tool.name}><b className={tool.available ? '' : 'danger-text'}>[{tool.available ? 'OK' : '--'}]</b><span>{tool.name}<small>{tool.detail}</small></span><code>{tool.version}</code></div>) || <div className="empty">Native dependencies have not been checked.</div>}</div></Panel></>}
    <Panel title="SYSTEM LOG" meta={<span>{logs.length} EVENTS / LOCAL SESSION</span>} className="event-panel"><div className="event-log" role="log" aria-live="polite">{logs.slice(-5).map(entry => <div key={entry.id}><time>{entry.time}</time><b className={entry.level === 'WARN' ? 'danger-text' : ''}>[{entry.level}]</b><span>{entry.message}</span></div>)}</div></Panel>
    </main></div><footer className="statusbar"><span><i /> {source === 'demo' ? 'DEMO' : 'LIVE'} / {active ? 'SESSION ACTIVE' : 'STANDBY'}</span><span>{adapter || 'NO INTERFACE'}<b>│</b>{scan ? 'DISCOVERING' : capture.running ? 'OBSERVING' : 'READY'}</span><span>[1–9,0] NAVIGATE <b>│</b> [R] SCAN <b>│</b> [?] HELP</span></footer>
    {panic.overlay}
    <TerminalAudio enabled={sound} reaction={skull} reactionId={reactionId} />
    <DependencySetup request={setupRequest} profile={setupProfile} onComplete={() => void checkSystem()} />
    {modal === 'demo' && <Modal title="ENTER DEMO MODE" onClose={() => setModal(null)}><p>Explore the interface using a fixed set of synthetic access points and a labelled EAPOL replay. Demo mode does not scan, capture, or run Hashcat.</p><div className="button-row"><button className="primary" onClick={() => void changeSource('demo')}>ENABLE DEMO MODE</button><button onClick={() => setModal(null)}>CANCEL</button></div></Modal>}
    {modal === 'end' && <Modal title="END SESSION" onClose={() => setModal(null)}><p>Stop capture and lab jobs, release the monitor interface, and {policy === 'PERSISTENT' ? 'retain your saved files.' : 'remove app-managed temporary session data.'}</p><p className="muted small">{session?.path || 'No app-managed session directory.'} This does not erase system logs, OS swap, snapshots, or files you downloaded separately.</p><div className="button-row"><button className="danger" disabled={busy} onClick={() => void endSession()}>{busy ? 'CLOSING…' : 'STOP AND END SESSION'}</button><button disabled={busy} onClick={() => setModal(null)}>CANCEL</button></div></Modal>}
    {modal === 'export' && <Modal title="EXPORT DIAGNOSTIC REPORT" onClose={() => setModal(null)}><p>Export {selectedNetwork ? cleanSsid(selectedNetwork.ssid) : 'observations'} as JSON. BSSIDs are masked; SSIDs remain in the report. No passwords are included.</p>{policy === 'MEMORY_ONLY' && source === 'live' ? <p className="danger-text">This session is memory-only. Start a RAM SESSION, temporary or persistent session to save reports.</p> : <button className="primary" disabled={busy} onClick={() => void exportReport()}>SAVE REPORT</button>}</Modal>}
    {modal === 'help' && <Modal title="KEYBOARD / OPERATOR HELP" onClose={() => setModal(null)}><dl className="key-values"><dt>1 – 9 / 0</dt><dd>Navigate workspaces</dd><dt>R</dt><dd>Request discovery</dd><dt>Tab / Shift+Tab</dt><dd>Move keyboard focus</dd><dt>Enter / Space</dt><dd>Activate focused control</dd><dt>Escape</dt><dd>Close dialog</dd></dl><p className="muted small">Select a session policy, start the session, then discover networks. The default live mode never loads synthetic data.</p></Modal>}
  </div>;
}
