export interface PanicBinding { code: string; ctrl: boolean; shift: boolean; alt: boolean; meta: boolean }
export const DEFAULT_PANIC_BINDING: PanicBinding = { code: 'F12', ctrl: true, shift: true, alt: false, meta: false };
export function bindingFrom(event: KeyboardEvent): PanicBinding | null {
  if (!event.code || /^(Control|Shift|Alt|Meta)/.test(event.code) || event.isComposing) return null;
  return {code: event.code, ctrl: event.ctrlKey, shift: event.shiftKey, alt: event.altKey, meta: event.metaKey};
}
export function matchesBinding(event: KeyboardEvent, binding: PanicBinding): boolean {
  return !event.repeat && !event.isComposing && event.code === binding.code && event.ctrlKey === binding.ctrl && event.shiftKey === binding.shift && event.altKey === binding.alt && event.metaKey === binding.meta;
}
export function bindingLabel(binding: PanicBinding): string { return [binding.ctrl && 'Ctrl', binding.shift && 'Shift', binding.alt && 'Alt', binding.meta && 'Meta', binding.code.replace(/^Key|^Digit/, '')].filter(Boolean).join(' + '); }
