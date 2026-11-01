import { useEffect, useRef, type ReactNode } from 'react';
import { bandOf, cleanSsid, maskBssid, type Network } from './domain';

export function Panel({ title, meta, children, className = '' }: { title: string; meta?: ReactNode; children: ReactNode; className?: string }) {
  return <section className={`panel ${className}`}><header className="panel-heading"><h2>{title}</h2><span>{meta}</span></header>{children}</section>;
}
export function Signal({ quality }: { quality: number }) {
  return <span className="signal" aria-label={`${quality}% signal quality`}>{Array.from({ length: 12 }, (_, i) => <i key={i} className={i < Math.round(quality / 100 * 12) ? 'lit' : ''} />)}<b>{quality}%</b></span>;
}
export function NetworkTable({ networks, selected, onSelect, compact = false, masked = true }: { networks: Network[]; selected: string; onSelect: (n: Network) => void; compact?: boolean; masked?: boolean }) {
  return <div className="table-scroll"><table className="network-table"><thead><tr><th>#</th><th>SSID / NETWORK</th>{!compact && <th>BSSID</th>}<th>CH</th><th>SIGNAL</th><th>SECURITY</th></tr></thead><tbody>{networks.map((n, i) => <tr key={n.bssid} className={selected === n.bssid ? 'selected' : ''} onClick={() => onSelect(n)}><td>{String(i + 1).padStart(2, '0')}</td><td><button className="row-button" onClick={e => { e.stopPropagation(); onSelect(n); }}><span className="selection-arrow">{selected === n.bssid ? '▸' : '·'}</span>{cleanSsid(n.ssid)}</button><small>{bandOf(n.frequency)}</small></td>{!compact && <td className="muted">{masked ? maskBssid(n.bssid) : n.bssid}</td>}<td>{n.channel || '—'}</td><td><Signal quality={n.quality} /></td><td><span className={n.security === 'Open' || n.security.includes('legacy') ? 'danger-text' : 'security'}>{n.security}</span></td></tr>)}</tbody></table>{networks.length === 0 && <div className="empty"><span>[ NO OBSERVATIONS ]</span><p>Start discovery to read available access points.</p></div>}</div>;
}
export function ChannelChart({ networks, band = '2.4 GHz' }: { networks: Network[]; band?: string }) {
  const channels = band === '2.4 GHz' ? Array.from({ length: 13 }, (_, i) => i + 1) : [36, 40, 44, 48, 52, 56, 60, 64, 100, 116, 132, 149, 161];
  const counts = channels.map(ch => networks.filter(n => bandOf(n.frequency) === band && n.channel === ch).length);
  return <div className="channel-chart" role="img" aria-label={`Observed access points per channel in ${band}`}><div className="chart-grid"><span>APs</span><span>3</span><span>2</span><span>1</span><span>0</span></div><div className="bars">{channels.map((ch, i) => <div className="bar-column" key={ch}><div className="bar-track"><div className="bar" style={{ height: `${Math.min(counts[i], 4) / 4 * 100}%` }}><span>{counts[i] || ''}</span></div></div><label>{ch}</label></div>)}</div></div>;
}
export function Modal({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { const d = ref.current; d?.showModal(); return () => d?.close(); }, []);
  return <dialog ref={ref} onCancel={onClose} className="modal"><header className="panel-heading"><h2>{title}</h2><button aria-label="Close dialog" onClick={onClose}>×</button></header><div className="modal-content">{children}</div></dialog>;
}
