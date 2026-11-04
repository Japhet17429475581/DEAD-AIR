export type Page = 'dashboard' | 'discover' | 'inspector' | 'spectrum' | 'capture' | 'adapter' | 'sessions' | 'lab' | 'settings' | 'academy';
export type SkullState = 'startup' | 'idle' | 'scanning' | 'network' | 'grin' | 'success' | 'error' | 'alert' | 'critical' | 'lab' | 'disappointed' | 'cleanup' | 'shutdown' | 'ambient-blink' | 'ambient-glance' | 'ambient-twitch' | 'ambient-stare' | 'ambient-grin';
export type Source = 'demo' | 'live';
export type SessionState = 'NEW' | 'ACTIVE' | 'CLOSED';
export type Policy = 'MEMORY_ONLY' | 'RAM_ONLY' | 'TEMPORARY' | 'PERSISTENT';
export interface Session { id: string; policy: Policy; state: string; path: string | null; files: string[] }
export interface Adapter { name: string; driver: string; phy: string; mode: string; frequency: number | null; monitorSupported: boolean; connected: boolean; bands: string[] }
export interface ToolStatus { name: string; available: boolean; version: string; detail: string }
export interface Diagnostics { platform: string; tools: ToolStatus[]; adapters: Adapter[]; errors: string[] }
export interface Packet { time: number; source: string; destination: string; station: string; kind: string; message: number | null; replay: string | null; signal: number | null; bytes: number }
export interface CaptureProgress { state: string; elapsedSeconds: number; remainingSeconds: number; limitSeconds: number; recordsRead: number | null; pcapBytes: number | null; beacons: number; eapolFrames: number; actualFrequency: number | null; heartbeatAgeSeconds: number | null; helperVersion: string | null; note: string }
export interface Capture { running: boolean; status: string; packets: Packet[]; count: number; dropped: number; error: string | null; exchanges: { station: string; messages: number[]; complete: boolean }[]; pmkids: {station:string;bssid:string;value:string;source:string;time:number}[]; progress?: CaptureProgress | null; stopReason?: string | null }
export interface LabStatus { running: boolean; status: string; lines: string[]; exitCode: number | null }
export interface Network {
  ssid: string; bssid: string; channel: number; frequency: number;
  quality: number; security: string; cipher: string; pmf: string;
}
export interface Snapshot { networks: Network[]; adapter: string; driver: string; source: string }
export interface LogEntry { id: number; time: string; level: 'OK' | 'INFO' | 'WARN'; message: string }
export const demoNetworks: Network[] = [
  { ssid: 'DEADAIR_LAB', bssid: '02:DA:00:00:01:01', channel: 6, frequency: 2437, quality: 94, security: 'WPA2-Personal', cipher: 'CCMP', pmf: 'Unknown' },
  { ssid: 'NEON_APARTMENT', bssid: '02:DA:00:00:02:02', channel: 36, frequency: 5180, quality: 81, security: 'WPA3-Personal', cipher: 'CCMP', pmf: 'Unknown' },
  { ssid: 'VOID_5G', bssid: '02:DA:00:00:03:03', channel: 44, frequency: 5220, quality: 72, security: 'WPA2/WPA3', cipher: 'CCMP', pmf: 'Unknown' },
  { ssid: 'WORKSHOP', bssid: '02:DA:00:00:04:04', channel: 11, frequency: 2462, quality: 66, security: 'WPA2-Personal', cipher: 'CCMP', pmf: 'Unknown' },
  { ssid: 'GUEST_ACCESS', bssid: '02:DA:00:00:05:05', channel: 1, frequency: 2412, quality: 48, security: 'Open', cipher: 'None', pmf: 'Unknown' },
  { ssid: '', bssid: '02:DA:00:00:06:06', channel: 6, frequency: 2437, quality: 35, security: 'WPA2-Personal', cipher: 'CCMP', pmf: 'Unknown' },
  { ssid: 'OLD_ROUTER', bssid: '02:DA:00:00:07:07', channel: 3, frequency: 2422, quality: 26, security: 'WPA legacy', cipher: 'TKIP', pmf: 'Unknown' },
  { ssid: 'STUDIO_NET', bssid: '02:DA:00:00:08:08', channel: 149, frequency: 5745, quality: 59, security: 'WPA2-Enterprise', cipher: 'CCMP', pmf: 'Unknown' },
];
export const cleanSsid = (name: string) => name.replace(/[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/g, '').slice(0, 64) || '<hidden network>';
export const maskBssid = (bssid: string) => bssid.split(':').map((part, index) => index > 2 ? '**' : part).join(':');
export const bandOf = (frequency: number) => frequency >= 5925 ? '6 GHz' : frequency >= 4900 ? '5 GHz' : '2.4 GHz';
export function filterNetworks(networks: Network[], query: string, band: string, sort: string) {
  return networks.filter(n => (cleanSsid(n.ssid).toLowerCase().includes(query.toLowerCase()) || n.bssid.toLowerCase().includes(query.toLowerCase())) && (band === 'all' || bandOf(n.frequency) === band))
    .sort((a, b) => sort === 'name' ? cleanSsid(a.ssid).localeCompare(cleanSsid(b.ssid)) : sort === 'channel' ? a.channel - b.channel : b.quality - a.quality);
}
export function review(network: Network) {
  const weak = network.security === 'Open' || network.security === 'WPA legacy' || network.cipher.includes('TKIP');
  return [
    { status: weak ? 'WARN' : 'OK', title: 'Advertised protection', detail: weak ? 'Open or legacy protection advertised. Configure WPA2-AES or WPA3 on your access point.' : `${network.security} advertised. This does not verify the configuration or negotiated connection.` },
    { status: network.pmf === 'Unknown' ? 'UNKNOWN' : 'INFO', title: 'Protected management frames', detail: `PMF: ${network.pmf}. Discovery does not establish enforcement on a client connection.` },
    { status: 'MANUAL', title: 'Unique network password', detail: 'Password strength cannot be inferred from radio advertisements. Review it on your router.' },
    { status: 'MANUAL', title: 'Router firmware', detail: 'Check your manufacturer’s supported firmware and disable unused remote administration.' },
  ];
}
export function reportFor(networks: Network[], source: Source) {
  return { application: 'DEAD//AIR', version: '0.4.0', source, generatedAt: new Date().toISOString(), identifiers: 'BSSIDs masked', networks: networks.map(n => ({ ...n, ssid: cleanSsid(n.ssid), bssid: maskBssid(n.bssid) })) };
}
