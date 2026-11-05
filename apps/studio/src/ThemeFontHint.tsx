import type { Theme, UnsetFonts } from './types'

export type ThemeFontKey = 'major' | 'minor' | 'east_asian' | 'complex_script'
type Group = 'major' | 'minor'
type Script = 'latin' | 'east_asian' | 'complex_script'

// The model keeps one East Asian and one complex-script font for both collections.
const SLOTS: Record<ThemeFontKey, [Group, Script][]> = {
  major: [['major', 'latin']],
  minor: [['minor', 'latin']],
  east_asian: [['major', 'east_asian'], ['minor', 'east_asian']],
  complex_script: [['major', 'complex_script'], ['minor', 'complex_script']],
}

function unsetSlots(theme: Theme, key: ThemeFontKey): [Group, Script][] {
  // `written` marks a slot an edit has already named, which is no longer left to PowerPoint.
  return SLOTS[key].filter(([group, script]) => {
    const state = theme.fonts.unset?.[group]?.[script]
    return state === 'empty' || state === 'absent'
  })
}

/** Marks the slots behind one font as specified, so that applying the theme writes the name shown even when the template left it empty. */
function specifySlots(theme: Theme, key: ThemeFontKey): Theme {
  const unset: { major?: UnsetFonts; minor?: UnsetFonts } = structuredClone(theme.fonts.unset ?? {})
  for (const [group, script] of SLOTS[key]) {
    const entry = unset[group]
    if (entry && entry[script] !== 'written') delete entry[script]
  }
  return { ...theme, fonts: { ...theme.fonts, unset } }
}

export function ThemeFontHint({ theme, fontKey, onChange }: { theme: Theme; fontKey: ThemeFontKey; onChange: (theme: Theme) => void }) {
  const slots = unsetSlots(theme, fontKey)
  if (!slots.length) return null
  const named = slots.map(([group, script]) => `${group} ${script.replace('_', ' ')}`).join(', ')
  return <p className="font-hint" role="note">
    Left empty by the template ({named}): PowerPoint chooses a font by language and installation, while AISlide shows {theme.fonts[fontKey]}.{' '}
    Change the name, or <button type="button" aria-label={`Write ${theme.fonts[fontKey]} as the ${named} theme font`} onClick={() => onChange(specifySlots(theme, fontKey))}>write this value</button>.
  </p>
}
