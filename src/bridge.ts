import type { Snapshot } from './domain';
export const isDesktop = () => '__TAURI_INTERNALS__' in window;
export async function native<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop()) throw new Error('Linux desktop backend unavailable. Open the Tauri application on Linux, or explicitly enable DEMO MODE to explore the interface.');
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command, args);
}
export const discoverLive = (adapter: string) => native<Snapshot>('discover', { adapter });
