/** Bitset B — Tailwind v3 config for the rtok web admin.
 * Colors are CSS variables (RGB channels) defined in src/input.css, mirroring
 * crates/rtok-webui/ui/theme.slint. `.dark` on <html> is the default theme;
 * removing it switches to the light values. Rebuild: see README.md. */
const v = (name) => `rgb(var(--${name}) / <alpha-value>)`;

module.exports = {
  darkMode: 'class',
  content: ['./index.html', './app.js'],
  theme: {
    extend: {
      colors: {
        // Surfaces (theme.slint: canvas / surface-1 / surface-2)
        canvas: v('canvas'),
        navy: v('canvas'),
        surface: { DEFAULT: v('surface'), 2: v('surface-2'), 3: v('surface-3') },
        line: { DEFAULT: v('line'), strong: v('line-strong') },
        // Ink (theme.slint: ink / ink-muted / ink-subtle, subtle raised to AA)
        ink: { DEFAULT: v('ink'), muted: v('ink-muted'), subtle: v('ink-subtle') },
        text: v('ink'),
        // Brand fills: cyan accent + coral Δ. Fixed in both themes.
        accent: { DEFAULT: v('accent'), fg: v('accent-fg'), on: v('on-accent') },
        delta: { DEFAULT: v('delta'), fg: v('delta-fg') },
        danger: { DEFAULT: v('delta'), fg: v('delta-fg') },
        success: { DEFAULT: v('success'), fg: v('success-fg') },
        warn: { DEFAULT: v('warn'), fg: v('warn-fg') },
      },
      fontFamily: {
        mono: ['"IBM Plex Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
      },
      fontSize: {
        // 4/8pt-friendly dense admin scale
        '2xs': ['0.6875rem', { lineHeight: '1rem' }],   // 11/16
        xs: ['0.75rem', { lineHeight: '1rem' }],        // 12/16
        sm: ['0.8125rem', { lineHeight: '1.25rem' }],   // 13/20
        base: ['0.875rem', { lineHeight: '1.5rem' }],   // 14/24
        lg: ['1rem', { lineHeight: '1.5rem' }],         // 16/24
        xl: ['1.25rem', { lineHeight: '1.75rem' }],     // 20/28
        '2xl': ['1.5rem', { lineHeight: '2rem' }],      // 24/32
      },
      borderRadius: {
        // theme.slint radius-sm / radius-md / radius-lg
        sm: '4px', md: '6px', lg: '10px', xl: '14px',
      },
      boxShadow: {
        'e1': '0 1px 0 0 rgb(var(--shine) / 0.06) inset, 0 1px 2px rgb(0 0 0 / 0.20)',
        'e2': '0 1px 0 0 rgb(var(--shine) / 0.07) inset, 0 4px 12px -2px rgb(0 0 0 / 0.28), 0 1px 3px rgb(0 0 0 / 0.20)',
        'e3': '0 1px 0 0 rgb(var(--shine) / 0.08) inset, 0 16px 40px -8px rgb(0 0 0 / 0.45), 0 4px 12px rgb(0 0 0 / 0.25)',
        'ring': '0 0 0 2px rgb(var(--canvas)), 0 0 0 4px rgb(var(--accent))',
      },
      screens: {
        // theme.slint bp-sm/md/lg/xl + RESPONSIVE.md
        sm: '640px', md: '768px', lg: '1024px', xl: '1440px', '2xl': '1920px',
      },
      transitionDuration: { fast: '120ms', base: '180ms' },
    },
  },
  plugins: [],
};
