import { create } from "zustand"
import { invoke } from "@tauri-apps/api/core"
import type { ReminderSnapshot } from "../types/reminder"

/**
 * 今日统计 + 提醒快照
 *
 * T09：today 从 Rust get_today_stats_cmd 拉取
 * T36：reminder 从 Rust get_reminder_status_cmd 拉取，独立字段
 *
 * 两条 IPC 互不耦合：today 是 SQLite SELECT，reminder 是内存状态计算。
 * 前端用同一 1Hz 间隔并发刷新。
 */

export interface TodayStats {
  totalSeconds: number
  restCount: number
  restSeconds: number
}

interface StatsState {
  today: TodayStats
  reminder: ReminderSnapshot
  setToday: (stats: TodayStats) => void
  refresh: () => Promise<void>
  refreshReminder: () => Promise<void>
  lastError: string | null
}

const DEFAULT_REMINDER: ReminderSnapshot = {
  lastStrongAt: null,
  lastEyeDropAt: null,
  lastWarmCompressAt: null,
  nextStrongSecs: null,
  nextEyeDropSecs: null,
  nextWarmCompressSecs: null,
  inWorkHours: false,
  isPaused: false,
  eyeDropEnabled: false,
  warmCompressEnabled: false,
  pauseRemainingSecs: null,
  strongHint: null,
  eyeDropHint: null,
  warmCompressHint: null,
}

export const useStatsStore = create<StatsState>((set) => ({
  today: { totalSeconds: 0, restCount: 0, restSeconds: 0 },
  reminder: DEFAULT_REMINDER,
  lastError: null,

  setToday: (stats) => set({ today: stats }),

  refresh: async () => {
    try {
      const stats = await invoke<TodayStats>("get_today_stats_cmd")
      set({
        today: {
          totalSeconds: stats.totalSeconds,
          restCount: stats.restCount,
          restSeconds: stats.restSeconds,
        },
        lastError: null,
      })
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      set({ lastError: msg })
    }
  },

  refreshReminder: async () => {
    try {
      const snapshot = await invoke<ReminderSnapshot>("get_reminder_status_cmd")
      set({ reminder: snapshot, lastError: null })
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      set({ lastError: msg })
    }
  },
}))