// Each renderer window owns its visibility listener for its lifetime.
const initialized = new WeakSet<Document>()
export function initializeEffectVisibility(): void {
  if (typeof document === 'undefined' || !document.addEventListener || initialized.has(document)) return
  initialized.add(document)
  const update = () => document.documentElement.setAttribute('data-effects-hidden', String(document.hidden))
  document.addEventListener('visibilitychange', update)
  update()
}
