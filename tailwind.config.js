/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./reminder.html",
    "./softprompt.html",
    "./settings.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  // T31：沐目是浅色护眼工具，不跟随系统深色主题。
  // 用 'class' 模式但永远不激活 .dark / [data-theme="dark"]，
  // 任何 dark: 变体都不会被应用。
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // 来自 design.md 的配色方案
        bg: {
          light: '#FAF8F5',      // 米白主背景
          card: '#FFFFFF',        // 卡片背景
          dark: '#1F1B16',        // 深棕主背景
          'dark-card': '#2A2520', // 深棕卡片背景
        },
        text: {
          primary: {
            light: '#2C2825',    // 深棕主文字
            dark: '#F5F0E8',     // 米白主文字
          },
          secondary: {
            light: '#8B8378',    // 灰棕副文字
            dark: '#B5AC9F',     // 浅灰棕副文字
          },
          hint: {
            light: '#C7BFB4',    // 浅灰提示
            dark: '#6B6259',     // 中灰提示
          },
        },
        accent: {
          DEFAULT: '#87A878',    // 薄荷绿主强调
          warm: '#C8956D',       // 暖橙警示
        },
      },
      fontFamily: {
        sans: [
          'Microsoft YaHei',
          'PingFang SC',
          'Noto Sans CJK SC',
          'Segoe UI',
          'sans-serif',
        ],
      },
      fontSize: {
        // 来自 design.md 的字号规范
        'mega': ['64px', { lineHeight: '1.1', fontWeight: '700' }],
        'title': ['24px', { lineHeight: '1.3', fontWeight: '500' }],
        'body': ['14px', { lineHeight: '1.5', fontWeight: '400' }],
        'caption': ['12px', { lineHeight: '1.4', fontWeight: '400' }],
        'tiny': ['11px', { lineHeight: '1.3', fontWeight: '400' }],
      },
      borderRadius: {
        'sm': '4px',
        DEFAULT: '8px',
        'lg': '12px',
      },
      spacing: {
        // 基础单位 4px，常用 8/12/16/24/32/48
        'base': '4px',
      },
      backdropBlur: {
        'xs': '20px',
      },
    },
  },
  plugins: [],
}
