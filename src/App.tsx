import { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useStatsStore } from "./stores/stats"
import type { NextHint } from "./types/reminder"

/**
 * 主界面
 *
 * 布局规范（来自 design.md）：
 * - 480 × 360 极简单列
 * - 巨号数字：今日使用时长
 * - 副信息：休息次数
 * - 颜色规则：≤8h 默认色；8-10h 默认色 + "注意休息"；>10h 默认色 + 暖橙边框 + "建议关掉电脑"
 *
 * T36：在底部副信息下方新增三行提醒时间块（强提醒 / 眼药水 / 热敷），
 * 每行显示"上次 HH:mm" + "下次 HH:mm / 倒计时"。
 * 仅当对应设置项勾选时才显示对应行（强提醒永远显示）。
 * 文案受 NextHint 控制（Paused / OutOfWorkHours / Disabled /
 * WarmCompressAlreadyToday / EyeDropDismissedLimit）。
 *
 * 数据：T09 接入 Rust `get_today_stats_cmd`，T36 接入 `get_reminder_status_cmd`，均 1Hz 轮询
 */

// formatTime：把后端 ISO 时间格式化成"HH:mm" / "昨天 HH:mm" / "MM-DD HH:mm"
function formatTime(iso: string | null): string {
  if (!iso) return "尚无记录"
  const d = new Date(iso)
  const now = new Date()
  const hh = String(d.getHours()).padStart(2, "0")
  const mm = String(d.getMinutes()).padStart(2, "0")
  const sameDay = d.toDateString() === now.toDateString()
  if (sameDay) return `${hh}:${mm}`
  const yesterday = new Date(now)
  yesterday.setDate(now.getDate() - 1)
  if (d.toDateString() === yesterday.toDateString()) return `昨天 ${hh}:${mm}`
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${hh}:${mm}`
}

// formatCountdown：把秒数格式化成无歧义时长
// - >= 1h        → "1h 18m"（中英混合，单位清晰）
// - <  1h & >=60 → "18 分 16 秒"（中文带"分""秒"，避免误读为时间戳）
// - <  60s       → "59 秒"（紧迫态，单一单位）
// - <= 0         → "随时可能"
function formatCountdown(secs: number): string {
  if (secs <= 0) return "随时可能"
  if (secs >= 3600) {
    const h = Math.floor(secs / 3600)
    const m = Math.floor((secs % 3600) / 60)
    return `${h}h ${m}m`
  }
  if (secs >= 60) {
    const m = Math.floor(secs / 60)
    const s = secs % 60
    return `${m} 分 ${s} 秒`
  }
  return `${secs} 秒`
}

// formatPauseRemaining：把"剩余暂停秒数"格式化成 "X 分钟" / "X 小时 Y 分钟" / "明日 HH:MM"
function formatPauseRemaining(secs: number): string {
  // >= 6h → 切换为"明日 HH:MM"显示更直观（通常是暂停到明早 9 点）
  if (secs >= 6 * 3600) {
    const until = new Date(Date.now() + secs * 1000)
    return `到明日 ${String(until.getHours()).padStart(2, "0")}:${String(until.getMinutes()).padStart(2, "0")}`
  }
  // >= 1h → "X 小时 Y 分钟"
  if (secs >= 3600) {
    const h = Math.floor(secs / 3600)
    const m = Math.floor((secs % 3600) / 60)
    return `${h} 小时 ${m} 分钟`
  }
  // < 1h → "X 分钟"
  const m = Math.ceil(secs / 60)
  return `${m} 分钟`
}

// 决定"下次"那一列的文本 + 颜色
// hint 优先级高于 nextSecs；强提醒没有 warmCompress 这种特殊 hint
function resolveNextText(
  hint: NextHint | null,
  nextSecs: number | null,
): { text: string; color: string; fullText: string } {
  const GREY = "#9E958A"
  const GREEN = "#87A878"
  const ORANGE = "#C8956D"

  if (hint) {
    switch (hint.kind) {
      case "paused":
        return { text: "已暂停", color: GREY, fullText: "当前正在暂停（包括锁屏自动暂停）" }
      case "outOfWorkHours":
        return { text: "明日 9:00 起", color: GREY, fullText: "当前不在工作时段内，明日 9:00 工作开始后继续提醒" }
      case "disabled":
        return { text: "未启用", color: GREY, fullText: "该提醒未启用（设置页勾选才生效）" }
      case "warmCompressAlreadyToday":
        return {
          text: `明日 ${hint.value.hhmm}`,
          color: GREY,
          fullText: `今日热敷已触发，下一次 = 明天 ${hint.value.hhmm}`,
        }
      case "eyeDropDismissedLimit":
        return {
          text: `明日 ${hint.value.hhmm}`,
          color: GREY,
          fullText: `今日已跳过 3 次眼药水提醒（当日静默），下一次 = 明天 ${hint.value.hhmm} 工作开始`,
        }
    }
  }

  if (nextSecs === null) return { text: "—", color: GREY, fullText: "下次提醒时间未定" }
  const isUrgent = nextSecs > 0 && nextSecs < 60
  return { text: formatCountdown(nextSecs), color: isUrgent ? ORANGE : GREEN, fullText: `下次提醒：${formatCountdown(nextSecs)} 后` }
}

// ReminderRow：紧凑单行提醒信息（仅 App 内使用）
function ReminderRow({
  icon,
  label,
  lastAt,
  hint,
  nextSecs,
}: {
  icon: string
  label: string
  lastAt: string | null
  hint: NextHint | null
  nextSecs: number | null
}) {
  const { text: nextText, color: nextColor, fullText: nextFullText } = resolveNextText(hint, nextSecs)
  // 把"为什么是明天"的语义放进 title tooltip，单行仍保持紧凑
  const title = nextFullText !== nextText ? nextFullText : undefined
  return (
    <div className="flex items-center justify-between gap-3 px-1 text-caption whitespace-nowrap">
      <span className="text-text-secondary-light truncate" title={`上次提醒时间：${formatTime(lastAt)}`}>
        <span className="mr-1">{icon}</span>
        {label} · 上次 {formatTime(lastAt)}
      </span>
      <span style={{ color: nextColor }} title={title}>下次 {nextText}</span>
    </div>
  )
}

function App() {
  const today = useStatsStore((s) => s.today)
  const reminder = useStatsStore((s) => s.reminder)
  const refresh = useStatsStore((s) => s.refresh)
  const refreshReminder = useStatsStore((s) => s.refreshReminder)

  // T37：客户端递减——后端返回的 nextSecs 只在 last_*_at 变化时才更新，
  // 单纯因为"now 推进"导致的剩余秒数减少由前端每秒 -1 渲染，
  // 这样 1Hz 拉取不会产生"卡住"的视觉（后端 last=None 时的 unwrap_or(now)
  // 退化会让 (now+interval)-now 恒等于 interval）。
  // 关键：用 store 里的 reminder 同步 localReminder 时只在"last_*_at / 暂停 / enabled"
  // 这些真正变化的事件触发，**不能每次 invoke 后都覆盖**——
  // 否则本地递减会被后端"固定值"在每秒 setInterval 里瞬间打回原形。
  const [localReminder, setLocalReminder] = useState(reminder)
  const lastSyncKeyRef = useRef<string>("")
  useEffect(() => {
    const key = [
      reminder.lastStrongAt ?? "",
      reminder.lastEyeDropAt ?? "",
      reminder.lastWarmCompressAt ?? "",
      reminder.isPaused,
      reminder.eyeDropEnabled,
      reminder.warmCompressEnabled,
      reminder.inWorkHours,
      reminder.pauseRemainingSecs,
    ].join("|")
    // 仅当关键事件字段变了才同步（否则保持本地递减值）
    if (key !== lastSyncKeyRef.current) {
      lastSyncKeyRef.current = key
      setLocalReminder(reminder)
    }
  }, [reminder])
  useEffect(() => {
    const t = setInterval(() => {
      setLocalReminder((prev) => {
        const next = { ...prev }
        if (next.nextStrongSecs !== null && next.nextStrongSecs > 0) {
          next.nextStrongSecs = next.nextStrongSecs - 1
        }
        if (next.nextEyeDropSecs !== null && next.nextEyeDropSecs > 0) {
          next.nextEyeDropSecs = next.nextEyeDropSecs - 1
        }
        if (next.nextWarmCompressSecs !== null && next.nextWarmCompressSecs > 0) {
          next.nextWarmCompressSecs = next.nextWarmCompressSecs - 1
        }
        if (next.pauseRemainingSecs !== null && next.pauseRemainingSecs > 0) {
          next.pauseRemainingSecs = next.pauseRemainingSecs - 1
        }
        return next
      })
    }, 1000)
    return () => clearInterval(t)
  }, [])

  // 每秒刷新一次（轻量 SQLite SELECT + 内存状态计算，OK 频率）
  useEffect(() => {
    refresh()
    refreshReminder()
    const timer = setInterval(() => {
      refresh()
      refreshReminder()
    }, 1000)
    return () => clearInterval(timer)
  }, [refresh, refreshReminder])

  // 关闭按钮：拦截默认行为，隐藏窗口而非退出进程
  useEffect(() => {
    let unlisten: (() => void) | undefined
    try {
      const win = getCurrentWindow()
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
    } catch {
      // 非 Tauri 环境（vite dev 浏览器）下 getCurrentWindow 抛错，忽略即可
    }
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

  // 格式化累计休息时长：<60s 显示 "Xs"，否则 "Xm Ys"
  // （一次休息 = 20 秒，N 次累加；用 Xm Xs 让用户感受到"每一秒都在放松"）
  const formatRestSeconds = (seconds: number): string => {
    if (seconds <= 0) return "0 秒"
    if (seconds < 60) return `${seconds} 秒`
    const m = Math.floor(seconds / 60)
    const s = seconds % 60
    if (s === 0) return `${m} 分钟`
    return `${m} 分 ${s} 秒`
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

      {/* 副信息：今日累计休息（次数 + 秒数） */}
      <p className="text-body text-text-primary-light">
        眼睛休息了 <span className="font-medium">{today.restCount}</span> 次
      </p>
      <p className="text-caption text-text-hint-light mt-2">
        累计休息 {formatRestSeconds(today.restSeconds)}
      </p>

      {/* T36+：手动暂停剩余时间 + 继续按钮（来自 reminder.pauseRemainingSecs） */}
      {localReminder.pauseRemainingSecs !== null && localReminder.pauseRemainingSecs > 0 && (
        <div className="flex items-center justify-center gap-3 mt-2">
          <p className="text-caption" style={{ color: "#C8956D" }}>
            已暂停 {formatPauseRemaining(localReminder.pauseRemainingSecs)}
          </p>
          <button
            onClick={() =>
              invoke("resume_reminders_cmd").catch((e) =>
                console.error("resume_reminders_cmd failed", e),
              )
            }
            className="text-caption px-2 py-0.5 rounded-full transition-colors"
            style={{
              color: "#FAF8F5",
              backgroundColor: "#87A878",
            }}
            onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "#6F8F65")}
            onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "#87A878")}
          >
            继续提醒
          </button>
        </div>
      )}

      {/* T36：提醒时间块（三行紧凑堆叠，按 enabled 条件渲染） */}
      <div className="flex flex-col gap-1.5 mt-4 w-full max-w-[340px]">
        <ReminderRow
          icon="☕"
          label="休息"
          lastAt={localReminder.lastStrongAt}
          hint={localReminder.strongHint}
          nextSecs={localReminder.nextStrongSecs}
        />
        {localReminder.eyeDropEnabled && (
          <ReminderRow
            icon="💧"
            label="眼药水"
            lastAt={localReminder.lastEyeDropAt}
            hint={localReminder.eyeDropHint}
            nextSecs={localReminder.nextEyeDropSecs}
          />
        )}
        {localReminder.warmCompressEnabled && (
          <ReminderRow
            icon="♨"
            label="热敷"
            lastAt={localReminder.lastWarmCompressAt}
            hint={localReminder.warmCompressHint}
            nextSecs={localReminder.nextWarmCompressSecs}
          />
        )}
      </div>

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
