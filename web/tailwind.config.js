/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{vue,ts}'],
  theme: {
    extend: {
      colors: {
        // Deep projector-friendly background and high-contrast accents.
        board: {
          bg: '#0a0e14',
          panel: '#131a26',
          line: '#243045',
        },
        accent: {
          DEFAULT: '#22d3ee',
          win: '#34d399',
          bust: '#f87171',
          turn: '#fbbf24',
        },
      },
      fontFamily: {
        // Tabular numerals keep big scores from jittering as they change.
        score: ['"DSEG14"', '"Roboto Mono"', 'ui-monospace', 'monospace'],
      },
      fontSize: {
        // Sized for across-the-room readability on a projector.
        mega: ['11rem', { lineHeight: '1' }],
        huge: ['7rem', { lineHeight: '1' }],
      },
      keyframes: {
        'pop-in': {
          '0%': { transform: 'scale(0.6)', opacity: '0' },
          '60%': { transform: 'scale(1.08)', opacity: '1' },
          '100%': { transform: 'scale(1)', opacity: '1' },
        },
        'flash': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.35' },
        },
      },
      animation: {
        'pop-in': 'pop-in 0.25s ease-out',
        'flash': 'flash 1s ease-in-out infinite',
      },
    },
  },
  plugins: [],
}
