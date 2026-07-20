import { useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useStatsStore } from "./stores/stats"

/**
 * 主界面
 *
 * 布局规范（来自 design.md）：
 * - 480 × 360 极简单列
 * - 巨号数字：今日使用时长
 * - 副信息：休息次数
 * - 颜色规则：≤8h 默认色；8-10h 默认色 + "注意休息"；>10h 默认色 + 暖橙边框 + "建议关掉电脑"
 *
 * 数据：T09 接入 Rust `get_today_stats_cmd`，每秒拉取一次
 */
function App() {
  const today = useStatsStore((s) => s.today)
  const refresh = useStatsStore((s) => s.refresh)

  // 每秒刷新一次（轻量 SQLite SELECT，OK 频率）
  useEffect(() => {
    refresh()
    const timer = setInterval(refresh, 1000)
    return () => clearInterval(timer)
  }, [refresh])

  // 关闭按钮：拦截默认行为，隐藏窗口而非退出进程
  useEffect(() => {
    const win = getCurrentWindow()
    let unlisten: (() => void) | undefined
    win.onCloseRequested(async (event) => {
      event.preventDefault()
      try {
        await invoke("hide_main_window")
      } catch (e) {
        console.error("hide_main_window failed", e)
      }
    }).then((u) => {
      unlisten = u
    })
    return () => {
      unlisten?.()
    }
  }, [])

  // 格式化：≤1h 显示 "Xm"；>1h 显示 "Xh Ym"
  const formatDuration = (seconds: number): string => {
    const hours = Math.floor(seconds / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    if (hours > 0) return `${hours}h ${minutes}m`
    return `${minutes}m`
  }

  // 颜色规则（spec: ≤8h / 8-10h / >10h）
  const getUsageColor = (): {
    color: string
    border: string
    warning: string | null
  } => {
    const hours = today.totalSeconds / 3600
    if (hours > 10) {
      return { color: "#2C2825", border: "2px solid #C8956D", warning: "建议关掉电脑" }
    }
    if (hours > 8) {
      return { color: "#2C2825", border: "none", warning: "注意休息" }
    }
    return { color: "#2C2825", border: "none", warning: null }
  }

  const usage = getUsageColor()

  return (
    <main
      className="w-full h-screen flex flex-col items-center justify-center px-6 py-8 select-none relative"
      style={{
        backgroundColor: "#FAF8F5",
        border: usage.border,
        borderRadius: "12px",
      }}
    >
      {/* 右上角手动关闭按钮（窗口无原生标题栏） */}
      <button
        onClick={() => invoke("hide_main_window").catch((e) => console.error("hide failed", e))}
        aria-label="关闭"
        className="absolute top-2 right-3 w-7 h-7 flex items-center justify-center rounded-full text-text-hint-light hover:bg-black/5 hover:text-text-primary-light transition-colors"
      >
        <span className="text-lg leading-none">×</span>
      </button>

      {/* 主数字 */}
      <h1 className="text-mega tracking-tight" style={{ color: usage.color }}>
        {formatDuration(today.totalSeconds)}
      </h1>

      {/* 副标题 */}
      <p className="text-body text-text-secondary-light mt-3">今日屏幕使用</p>

      {/* 警告信息（可选） */}
      {usage.warning && (
        <p className="text-caption mt-4" style={{ color: "#C8956D" }}>
          {usage.warning}
        </p>
      )}

      <div className="flex-1" />

      {/* 副信息：休息次数 */}
      <p className="text-body text-text-primary-light">
        眼睛休息了 <span className="font-medium">{today.restCount}</span> 次
      </p>
      <p className="text-caption text-text-hint-light mt-2">眼睛会更舒服</p>

      {/* 调试：若后端 invoke 出错，显示在底部（开发期可见） */}
      {useStatsStore.getState().lastError && (
        <p className="text-caption text-red-400 mt-2">
          ⚠ {useStatsStore.getState().lastError}
        </p>
      )}
    </main>
  )
}

export default App
