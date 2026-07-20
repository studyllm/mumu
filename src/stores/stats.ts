import { create } from "zustand"
import { invoke } from "@tauri-apps/api/core"

/**
 * 今日统计状态
 *
 * T09：从 Rust 端通过 invoke('get_today_stats_cmd') 拉取
 */

export interface TodayStats {
  totalSeconds: number
  restCount: number
  restSeconds: number
}

interface StatsState {
  today: TodayStats
  setToday: (stats: TodayStats) => void
  refresh: () => Promise<void>
  lastError: string | null
}

export const useStatsStore = create<StatsState>((set) => ({
  today: { totalSeconds: 0, restCount: 0, restSeconds: 0 },
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
      // 静默降级：不让 UI 抖；保留 lastError 给调试 UI 看
      const msg = e instanceof Error ? e.message : String(e)
      set({ lastError: msg })
    }
  },
}))
