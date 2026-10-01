export const appearances = ['light', 'dark', 'heyday'] as const
export type Appearance = typeof appearances[number]

export function readAppearance(): Appearance {
  try {
    const saved = localStorage.getItem('theme')
    if (appearances.includes(saved as Appearance)) return saved as Appearance
  } catch { /* Appearance remains usable when storage is unavailable. */ }
  return 'heyday'
}
