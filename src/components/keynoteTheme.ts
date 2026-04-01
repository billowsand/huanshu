export type Tone = 'amber' | 'blue' | 'green' | 'red' | 'teal' | 'indigo'

export function toneVars(tone?: Tone) {
  switch (tone) {
    case 'blue':
    case 'indigo':
      return {
        bg: 'var(--info-bg)',
        border: 'var(--info-border)',
        text: 'var(--info-light)',
        contrast: 'var(--info-contrast)',
        solid: 'var(--info)',
      }
    case 'green':
    case 'teal':
      return {
        bg: 'var(--success-bg)',
        border: 'var(--success-border)',
        text: 'var(--success-light)',
        contrast: 'var(--success-contrast)',
        solid: 'var(--success)',
      }
    case 'red':
      return {
        bg: 'var(--danger-bg)',
        border: 'var(--danger-border)',
        text: 'var(--danger-light)',
        contrast: 'var(--danger-contrast)',
        solid: 'var(--danger)',
      }
    case 'amber':
    default:
      return {
        bg: 'var(--warning-bg)',
        border: 'var(--warning-border)',
        text: 'var(--warning-light)',
        contrast: 'var(--warning-contrast)',
        solid: 'var(--warning)',
      }
  }
}
