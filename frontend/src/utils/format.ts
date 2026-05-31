import type { ModelEntry } from '@/api/types'

/** Human-readable context window, e.g. 128000 → "128K", 1000000 → "1M". */
export function fmtContext(n?: number): string {
  if (!n) return '—'
  if (n >= 1_000_000) {
    const m = n / 1_000_000
    return `${Number.isInteger(m) ? m : m.toFixed(1)}M`
  }
  if (n >= 1000) return `${Math.round(n / 1000)}K`
  return String(n)
}

export function hasModality(m: ModelEntry, kind: string): boolean {
  return Boolean(m.modalities?.input?.includes(kind))
}

/** Input/output cost per million tokens, e.g. "$1.25 / $2.5". */
export function fmtCost(m: ModelEntry): string {
  const c = m.cost
  if (!c || (c.input == null && c.output == null)) return '—'
  const i = c.input != null ? `$${c.input}` : '—'
  const o = c.output != null ? `$${c.output}` : '—'
  return `${i} / ${o}`
}
