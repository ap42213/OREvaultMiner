/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './pages/**/*.{js,ts,jsx,tsx,mdx}',
    './components/**/*.{js,ts,jsx,tsx,mdx}',
    './app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        // Dark theme colors
        background: '#0a0a0a',
        surface: '#141414',
        'surface-light': '#1f1f1f',
        border: '#2a2a2a',
        primary: '#10b981', // Emerald for positive
        danger: '#ef4444',  // Red for negative
        warning: '#f59e0b', // Amber for warnings
        muted: '#6b7280',   // Gray for secondary text
      },
      fontFamily: {
        mono: ['JetBrains Mono', 'SF Mono', 'Monaco', 'Consolas', 'monospace'],
      },
    },
  },
  plugins: [],
};
