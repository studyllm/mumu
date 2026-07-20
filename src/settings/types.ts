/**
 * 设置窗口类型（与 Rust Settings 完全对齐）
 */

export interface Settings {
  version: number
  reminders: ReminderSettings
  care: CareSettings
  general: GeneralSettings
  advanced: AdvancedSettings
}

export interface ReminderSettings {
  work_start: string
  work_end: string
  interval_minutes: number
  rest_seconds: number
  show_popup: boolean
  play_sound: boolean
}

export interface CareSettings {
  eye_drop_enabled: boolean
  eye_drop_interval_minutes: number
  warm_compress_enabled: boolean
  warm_compress_time: string
}

export interface GeneralSettings {
  quick_pause: "30min" | "1h" | "till_tomorrow"
}

export interface AdvancedSettings {
  auto_start: boolean
  debug_mode: boolean
}

/** 默认值兜底 */
export const DEFAULT_SETTINGS: Settings = {
  version: 1,
  reminders: {
    work_start: "09:00",
    work_end: "18:00",
    interval_minutes: 20,
    rest_seconds: 20,
    show_popup: true,
    play_sound: true,
  },
  care: {
    eye_drop_enabled: true,
    eye_drop_interval_minutes: 120,
    warm_compress_enabled: true,
    warm_compress_time: "13:00",
  },
  general: {
    quick_pause: "30min",
  },
  advanced: {
    auto_start: true,
    debug_mode: false,
  },
}
