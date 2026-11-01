import { useEffect, useState } from 'react';
import { isDesktop, native } from '../bridge';
import { Modal } from '../ui';

interface Plan { distro: string; manager: string; packages: string[]; missing: string[]; helperAvailable: boolean; profile: string }
export function DependencySetup({ request, profile = 'all', onComplete }: { request: number; profile?: string; onComplete: () => void }) {
  const [plan, setPlan] = useState<Plan | null>(null);
  const [open, setOpen] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState('');
  const [done, setDone] = useState(false);
  const [includeLab, setIncludeLab] = useState(true);
  useEffect(() => {
    if (!isDesktop()) return;
    let current = true;
    native<Plan>('dependency_plan', { profile }).then(value => { if (current) { setPlan(value); setOpen(request > 0 || value.missing.length > 0 || !value.helperAvailable); setDone(false); setError(''); } }).catch(e => { if (current) { setError(String(e)); setOpen(true); } });
    return () => { current = false; };
  }, [request, profile]);
  async function changeProfile(include: boolean) {
    setIncludeLab(include); setError('');
    try { setPlan(await native<Plan>('dependency_plan', { profile: include ? 'all' : 'wireless' })); } catch (e) { setError(String(e)); }
  }
  async function install() {
    if (!plan || installing) return;
    setInstalling(true); setError('');
    try {
      const result = await native<Plan>('install_dependencies', { profile: plan.profile, authorized: true });
      setPlan(result); setDone(result.missing.length === 0); onComplete();
      if (result.missing.length) setError(`Installation ended, but these tools are still missing: ${result.missing.join(', ')}`);
    } catch (e) { setError(String(e)); } finally { setInstalling(false); }
  }
  if (!open) return null;
  return <Modal title="FIRST RUN // DEPENDENCY SETUP" onClose={() => { if (!installing) setOpen(false); }}>
    <p>{done ? 'Dependencies installed and checked. You can start a session.' : 'DEAD//AIR checked your system. Install the required wireless tools here; no terminal commands are needed.'}</p>
    {plan && <><dl className="key-values"><dt>Distribution</dt><dd>{plan.distro}</dd><dt>Package manager</dt><dd>{plan.manager}</dd><dt>Missing tools</dt><dd>{plan.missing.join(', ') || 'None'}</dd></dl>
      {profile !== 'cpu' && <label className="checkbox"><input type="checkbox" checked={includeLab} disabled={installing} onChange={e => void changeProfile(e.target.checked)} />Include Hashcat laboratory tools</label>}
      {profile === 'cpu' && <p>This installs an optional CPU OpenCL implementation for Hashcat. It can require several hundred megabytes; GPU drivers are unchanged.</p>}
      <div className="setup-packages">{plan.packages.map(name => <code key={name}>{name}</code>)}</div>
      <p className="small muted">Packages come from your distribution’s configured repositories. Linux will request administrator authorization. GPU drivers and wordlists are not installed.</p>
      {!plan.helperAvailable && <p className="danger-text">The installed setup helper is missing. Install the DEAD//AIR .deb or .rpm package. For source builds, follow docs/LINUX.md to install the helper.</p>}
    </>}
    {error && <p className="error-inline" role="alert">{error}</p>}
    {installing && <div className="installation-progress" role="status"><span />INSTALLING SYSTEM PACKAGES…<small>Waiting for the package manager. Keep this window open.</small></div>}
    <div className="button-row"><button className="primary" disabled={installing || !plan?.helperAvailable || !plan.missing.length} onClick={() => void install()}>{installing ? 'INSTALLING…' : 'INSTALL DEPENDENCIES'}</button><button disabled={installing} onClick={() => setOpen(false)}>{done ? 'CONTINUE' : 'LATER'}</button></div>
  </Modal>;
}
