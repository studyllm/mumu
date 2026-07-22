/**
 * 设置窗口（T11）
 *
 * 规格：openspec/changes/add-mumu-eye-care/specs/ui/spec.md § Settings
 *
 * 4 个 section 单列滚动：
 *  - Reminder: work hours (time picker ×2, 15min 步进), interval slider 15-60 step 5,
 *              rest duration slider 10-60 step 5, show_popup, play_sound
 *  - Care: eye_drop_enabled + interval (60-240 min), warm_compress_enabled + time
 *  - General: quick_pause radio (30min / 1h / 明日 9:00)
 *  - Advanced: auto_start, debug_mode
 *
 * 实时生效：每次变更 → invoke('write_settings_cmd') + 通知 scheduler
 * 工作时段校验：end <= start → 拒绝保存 + UI 红色提示
 * Test 按钮：触发 5 秒迷你强提醒（休息）+ 即时弹弱提示（护眼，眼药水/热敷可切）
 */

import { useCallback, useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import {
  DEFAULT_SETTINGS,
  type Settings,
} from "./types"

const INTERVAL_MIN = 15
const INTERVAL_MAX = 60
const INTERVAL_STEP = 5
const REST_MIN = 10
const REST_MAX = 60
const REST_STEP = 5
const EYE_DROP_MIN = 60
const EYE_DROP_MAX = 240
const EYE_DROP_STEP = 30

// 15-min step 时间选择器
const TIME_OPTIONS = (() => {
  const arr: string[] = []
  for (let h = 0; h < 24; h++) {
    for (let m = 0; m < 60; m += 15) {
      arr.push(`${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`)
    }
  }
  return arr
})()

export function SettingsWindow() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS)
  const [loaded, setLoaded] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [workHourError, setWorkHourError] = useState<string | null>(null)
  const [savedFlash, setSavedFlash] = useState(false)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const saveSeqRef = useRef(0)

  // 初始加载
  useEffect(() => {
    ;(async () => {
      try {
        const s = await invoke<Settings>("read_settings_cmd")
        setSettings(s)
      } catch (e) {
        console.error("read_settings_cmd failed", e)
        setError(String(e))
      } finally {
        setLoaded(true)
      }
    })()
  }, [])

  // 实时保存（去抖 300ms）+ 校验工作时段
  const persist = useCallback(async (next: Settings) => {
    // 工作时段校验
    if (next.reminders.work_end <= next.reminders.work_start) {
      setWorkHourError("下班时间必须晚于上班时间")
      return
    }
    setWorkHourError(null)

    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    const seq = ++saveSeqRef.current
    saveTimerRef.current = setTimeout(async () => {
      try {
        await invoke("write_settings_cmd", { settings: next })
        if (seq === saveSeqRef.current) {
          setSavedFlash(true)
          setTimeout(() => setSavedFlash(false), 1200)
        }
      } catch (e) {
        console.error("write_settings_cmd failed", e)
        setError(String(e))
      }
    }, 300)
  }, [])

  const updateReminder = useCallback(
    <K extends keyof Settings["reminders"]>(key: K, value: Settings["reminders"][K]) => {
      setSettings((prev) => {
        const next = { ...prev, reminders: { ...prev.reminders, [key]: value } }
        persist(next)
        return next
      })
    },
    [persist]
  )

  const updateCare = useCallback(
    <K extends keyof Settings["care"]>(key: K, value: Settings["care"][K]) => {
      setSettings((prev) => {
        const next = { ...prev, care: { ...prev.care, [key]: value } }
        persist(next)
        return next
      })
    },
    [persist]
  )

  const updateGeneral = useCallback(
    <K extends keyof Settings["general"]>(key: K, value: Settings["general"][K]) => {
      setSettings((prev) => {
        const next = { ...prev, general: { ...prev.general, [key]: value } }
        persist(next)
        return next
      })
    },
    [persist]
  )

  const updateAdvanced = useCallback(
    <K extends keyof Settings["advanced"]>(key: K, value: Settings["advanced"][K]) => {
      setSettings((prev) => {
        const next = { ...prev, advanced: { ...prev.advanced, [key]: value } }
        persist(next)
        return next
      })
    },
    [persist]
  )

  const handleReset = useCallback(async () => {
    if (!confirm("确定要恢复默认设置？")) return
    try {
      const defaults = await invoke<Settings>("reset_settings_cmd")
      setSettings(defaults)
      setWorkHourError(null)
    } catch (e) {
      console.error("reset_settings_cmd failed", e)
      setError(String(e))
    }
  }, [])

  const handleTestReminder = useCallback(async () => {
    try {
      await invoke("trigger_test_reminder")
    } catch (e) {
      console.error("trigger_test_reminder failed", e)
    }
  }, [])

  // T32：拆分后的"护眼提醒"按钮——按当前 kind 选择触发对应弱提示
  const [testCareKind, setTestCareKind] =
    useState<"eye_drop" | "warm_compress">("eye_drop")
  const handleTestCareReminder = useCallback(async () => {
    try {
      await invoke("trigger_test_soft_prompt", { kind: testCareKind })
    } catch (e) {
      console.error("trigger_test_soft_prompt failed", e)
    }
  }, [testCareKind])

  if (!loaded) {
    return (
      <div className="w-full h-full flex items-center justify-center text-text-secondary-light">
        加载中…
      </div>
    )
  }

  return (
    <div className="w-full h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-10 py-8 pb-16">
        {/* 顶部标题 + 保存状态 */}
        <div className="flex items-baseline justify-between mb-8">
          <h1 className="text-title font-medium" style={{ color: "#2C2825" }}>
            设置
          </h1>
          <div className="text-caption">
            {savedFlash && <span style={{ color: "#87A878" }}>✓ 已保存</span>}
            {error && <span style={{ color: "#C8956D" }}>⚠ {error}</span>}
          </div>
        </div>

        {/* 提醒设置 */}
        <Section title="提醒设置">
          <Row label="工作时间">
            <div className="flex gap-3 items-center">
              <TimeSelect
                value={settings.reminders.work_start}
                onChange={(v) => updateReminder("work_start", v)}
              />
              <span className="text-text-hint-light">至</span>
              <TimeSelect
                value={settings.reminders.work_end}
                onChange={(v) => updateReminder("work_end", v)}
              />
            </div>
            {workHourError && (
              <p className="text-tiny mt-1" style={{ color: "#C8956D" }}>
                {workHourError}
              </p>
            )}
          </Row>

          <Row label={`提醒间隔：${settings.reminders.interval_minutes} 分钟`}>
            <Slider
              min={INTERVAL_MIN}
              max={INTERVAL_MAX}
              step={INTERVAL_STEP}
              value={settings.reminders.interval_minutes}
              onChange={(v) => updateReminder("interval_minutes", v)}
            />
          </Row>

          <Row label={`休息时长：${settings.reminders.rest_seconds} 秒`}>
            <Slider
              min={REST_MIN}
              max={REST_MAX}
              step={REST_STEP}
              value={settings.reminders.rest_seconds}
              onChange={(v) => updateReminder("rest_seconds", v)}
            />
          </Row>

          <Row label="显示弹窗">
            <Checkbox
              checked={settings.reminders.show_popup}
              onChange={(v) => updateReminder("show_popup", v)}
            />
          </Row>

          <Row label="完成时播放声音">
            <Checkbox
              checked={settings.reminders.play_sound}
              onChange={(v) => updateReminder("play_sound", v)}
            />
          </Row>
        </Section>

        {/* 护眼保养 */}
        <Section title="护眼保养">
          <Row label="眼药水提醒">
            <Checkbox
              checked={settings.care.eye_drop_enabled}
              onChange={(v) => updateCare("eye_drop_enabled", v)}
            />
          </Row>
          <Row
            label={`间隔：${settings.care.eye_drop_interval_minutes} 分钟`}
            disabled={!settings.care.eye_drop_enabled}
          >
            <Slider
              min={EYE_DROP_MIN}
              max={EYE_DROP_MAX}
              step={EYE_DROP_STEP}
              value={settings.care.eye_drop_interval_minutes}
              onChange={(v) => updateCare("eye_drop_interval_minutes", v)}
              disabled={!settings.care.eye_drop_enabled}
            />
          </Row>
          <Row label="热敷提醒">
            <Checkbox
              checked={settings.care.warm_compress_enabled}
              onChange={(v) => updateCare("warm_compress_enabled", v)}
            />
          </Row>
          <Row label="热敷时间" disabled={!settings.care.warm_compress_enabled}>
            <TimeSelect
              value={settings.care.warm_compress_time}
              onChange={(v) => updateCare("warm_compress_time", v)}
              disabled={!settings.care.warm_compress_enabled}
            />
          </Row>
        </Section>

        {/* 通用 */}
        <Section title="通用">
          <Row label="托盘快速暂停">
            <RadioGroup
              value={settings.general.quick_pause}
              onChange={(v) => updateGeneral("quick_pause", v)}
              options={[
                { value: "30min", label: "30 分钟" },
                { value: "1h", label: "1 小时" },
                { value: "till_tomorrow", label: "到明早 9:00" },
              ]}
            />
          </Row>
        </Section>

        {/* 高级 */}
        <Section title="高级">
          <Row label="开机自启">
            <Checkbox
              checked={settings.advanced.auto_start}
              onChange={(v) => updateAdvanced("auto_start", v)}
            />
          </Row>
          <Row label="调试模式">
            <Checkbox
              checked={settings.advanced.debug_mode}
              onChange={(v) => updateAdvanced("debug_mode", v)}
            />
          </Row>
        </Section>

        {/* T32 拆分后的两个测试按钮：休息提醒（强提醒）+ 护眼提醒（弱提示） */}
        <div className="mt-10 flex flex-col items-center gap-3">
          <button
            onClick={handleTestReminder}
            className="test-reminder-btn"
          >
            休息提醒
          </button>

          <div className="flex items-center gap-3">
            <button
              onClick={handleTestCareReminder}
              className="test-reminder-btn"
            >
              护眼提醒
            </button>
            <select
              value={testCareKind}
              onChange={(e) =>
                setTestCareKind(e.target.value as "eye_drop" | "warm_compress")
              }
              className="settings-select"
              aria-label="选择护眼提醒类型"
            >
              <option value="eye_drop">眼药水</option>
              <option value="warm_compress">热敷</option>
            </select>
          </div>
        </div>

        <div className="mt-8 flex justify-center">
          <button
            onClick={handleReset}
            className="text-caption text-text-hint-light hover:text-text-secondary-light"
          >
            恢复默认设置
          </button>
        </div>
      </div>

      <style>{`
        .test-reminder-btn {
          padding: 10px 32px;
          border-radius: 8px;
          background: #87A878;
          color: #FFFFFF;
          font-size: 14px;
          font-family: inherit;
          border: none;
          cursor: pointer;
          transition: background 200ms;
        }
        .test-reminder-btn:hover {
          background: #6F8E64;
        }
      `}</style>
    </div>
  )
}

// ============================================================================
// 内部小组件
// ============================================================================

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-8">
      <h2
        className="text-body font-medium mb-3 pb-2"
        style={{
          color: "#2C2825",
          borderBottom: "1px solid rgba(0,0,0,0.06)",
        }}
      >
        {title}
      </h2>
      <div className="space-y-4">{children}</div>
    </div>
  )
}

function Row({
  label,
  children,
  disabled,
}: {
  label: string
  children: React.ReactNode
  disabled?: boolean
}) {
  return (
    <div
      className="flex items-center justify-between gap-6"
      style={{ opacity: disabled ? 0.4 : 1 }}
    >
      <label className="text-body flex-shrink-0" style={{ color: "#2C2825", minWidth: 140 }}>
        {label}
      </label>
      <div className="flex-1 flex justify-end">{children}</div>
    </div>
  )
}

function TimeSelect({
  value,
  onChange,
  disabled,
}: {
  value: string
  onChange: (v: string) => void
  disabled?: boolean
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      disabled={disabled}
      className="settings-select"
    >
      {TIME_OPTIONS.map((t) => (
        <option key={t} value={t}>
          {t}
        </option>
      ))}
    </select>
  )
}

function Slider({
  min,
  max,
  step,
  value,
  onChange,
  disabled,
}: {
  min: number
  max: number
  step: number
  value: number
  onChange: (v: number) => void
  disabled?: boolean
}) {
  return (
    <input
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(Number(e.target.value))}
      className="settings-slider"
    />
  )
}

function Checkbox({
  checked,
  onChange,
}: {
  checked: boolean
  onChange: (v: boolean) => void
}) {
  return (
    <button
      role="checkbox"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="settings-checkbox"
      style={{
        background: checked ? "#87A878" : "transparent",
        borderColor: checked ? "#87A878" : "rgba(0,0,0,0.2)",
      }}
    >
      {checked && (
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <path
            d="M2 6L5 9L10 3"
            stroke="white"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )}
    </button>
  )
}

function RadioGroup<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T
  onChange: (v: T) => void
  options: { value: T; label: string }[]
}) {
  return (
    <div className="flex gap-4">
      {options.map((opt) => (
        <label
          key={opt.value}
          className="flex items-center gap-2 cursor-pointer text-caption"
          style={{ color: "#2C2825" }}
          onClick={() => onChange(opt.value)}
        >
          <span
            className="settings-radio"
            style={{
              borderColor: value === opt.value ? "#87A878" : "rgba(0,0,0,0.2)",
            }}
          >
            {value === opt.value && <span className="settings-radio-dot" />}
          </span>
          {opt.label}
        </label>
      ))}
    </div>
  )
}
