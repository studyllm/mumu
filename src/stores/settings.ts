import { create } from "zustand"

/**
 * 设置状态
 *
 * 等 T03 完成后从 settings.json 加载真实数据
 */

export interface ReminderSettings {
  workStart: string         // "09:00"
  workEnd: string           // "18:00"
  intervalMinutes: number   // 20
  restSeconds: number       // 20
  showPopup: boolean        // true
  playSound: boolean        // true
}

export interface CareSettings {
  eyeDropEnabled: boolean              // true
  eyeDropIntervalMinutes: number       // 120
  warmCompressEnabled: boolean         // true
  warmCompressTime: string             // "13:00"
}

export interface AdvancedSettings {
  autoStart: boolean    // true
  debugMode: boolean    // false
}

export interface Settings {
  reminders: ReminderSettings
  care: CareSettings
  advanced: AdvancedSettings
}

const defaultSettings: Settings = {
  reminders: {
    workStart: '09:00',
    workEnd: '18:00',
    intervalMinutes: 20,
    restSeconds: 20,
    showPopup: true,
    playSound: true,
  },
  care: {
    eyeDropEnabled: true,
    eyeDropIntervalMinutes: 120,
    warmCompressEnabled: true,
    warmCompressTime: '13:00',
  },
  advanced: {
    autoStart: true,
    debugMode: false,
  },
}

interface SettingsState {
  settings: Settings
  setSettings: (settings: Settings) => void
  updateReminders: (partial: Partial<ReminderSettings>) => void
  updateCare: (partial: Partial<CareSettings>) => void
  updateAdvanced: (partial: Partial<AdvancedSettings>) => void
}

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: defaultSettings,

  setSettings: (settings) => set({ settings }),

  updateReminders: (partial) =>
    set((state) => ({
      settings: {
        ...state.settings,
        reminders: { ...state.settings.reminders, ...partial },
      },
    })),

  updateCare: (partial) =>
    set((state) => ({
      settings: {
        ...state.settings,
        care: { ...state.settings.care, ...partial },
      },
    })),

  updateAdvanced: (partial) =>
    set((state) => ({
      settings: {
        ...state.settings,
        advanced: { ...state.settings.advanced, ...partial },
      },
    })),
}))
