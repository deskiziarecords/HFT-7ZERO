/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        hft: {
          bg: '#0a0a0c',
          card: '#141417',
          accent: '#3b82f6',
          danger: '#ef4444',
          success: '#10b981',
          warning: '#f59e0b'
        }
      }
    },
  },
  plugins: [],
}
