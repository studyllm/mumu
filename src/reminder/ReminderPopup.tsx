/**
 * 强提醒弹窗（T10）+ 木鱼音效（T12）
 *
 * 规格：
 * - UI: openspec/changes/add-mumu-eye-care/specs/ui/spec.md § Popup
 * - Sound: openspec/changes/add-mumu-eye-care/specs/reminders/spec.md § Wooden fish
 *
 * 视觉与动画：
 * - 320×200 半透明 + 20px backdrop blur + 12px 圆角 + 软阴影
 * - 浅色 rgba(255,255,255,0.85)，深色 rgba(31,27,22,0.85)
 * - 300ms opacity fade-in ease-out；500ms fade-out ease-in
 *
 * 行为：
 * - 倒计时：客户端 setInterval 每秒递减
 * - 跳过：右下角 11px 灰字 30% 透明 → invoke('reminder_skip')，**不播放声音**
 * - 归零：invoke('reminder_complete') + 500ms 退场 + 若 play_sound 则播木鱼声
 * - 锁屏事件 HideAllPopups：立即停计时 + 退场（保留剩余时间给续弹用）
 */

import { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import {
  HIDE_ALL_POPUPS_EVENT,
  SHOW_STRONG_REMINDER_EVENT,
  type ShowStrongReminderPayload,
} from "./types"
import { playWoodenFish, preloadWoodenFish } from "./audio"

type Phase = "hidden" | "entering" | "shown" | "exiting"

export function ReminderPopup() {
  const [phase, setPhase] = useState<Phase>("hidden")
  const [seconds, setSeconds] = useState<number>(20)
  const [playSound, setPlaySound] = useState<boolean>(false)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const enteredAtRef = useRef<number>(0)
  // T27 修复：playSound 用 ref 镜像，避免 useEffect mount 一次后
  // setInterval 闭包永远捕获 mount 时的 false → 倒计时归零不响木鱼声
  const playSoundRef = useRef<boolean>(false)

  const stopInterval = () => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current)
      intervalRef.current = null
    }
  }

  // 进入退场动画（CSS class 切换）
  const enter = () => {
    setPhase("entering")
    // 下一帧切到 shown 触发 CSS transition
    requestAnimationFrame(() => {
      requestAnimationFrame(() => setPhase("shown"))
    })
  }

  const exit = () => {
    setPhase("exiting")
    setTimeout(() => setPhase("hidden"), 500)
  }

  const startCountdown = (total: number) => {
    stopInterval()
    enteredAtRef.current = Date.now()
    setSeconds(total)
    intervalRef.current = setInterval(() => {
      setSeconds((prev) => {
        if (prev <= 1) {
          stopInterval()
          // 归零：先播声音（若启用），再通知后端 + 退场
          // T27 修复：从 ref 读最新 playSound，而非 setInterval 创建时的 closure
          if (playSoundRef.current) {
            playWoodenFish().catch((e) =>
              console.error("audio play failed", e)
            )
          }
          invoke("reminder_complete").catch((e) =>
            console.error("reminder_complete failed", e)
          )
          exit()
          return 0
        }
        return prev - 1
      })
    }, 1000)
  }

  const handleSkip = () => {
    stopInterval()
    invoke("reminder_skip").catch((e) => console.error("reminder_skip failed", e))
    exit()
  }

  // 订阅后端事件
  useEffect(() => {
    const unlistens: UnlistenFn[] = []
    let cancelled = false

    // T12：popup 第一次 mount 时预加载音频 + 尝试 resume AudioContext
    // （reminder 触发时倒计时归零不算 user gesture，需提前解锁）
    preloadWoodenFish()

    ;(async () => {
      const u1 = await listen<ShowStrongReminderPayload>(
        SHOW_STRONG_REMINDER_EVENT,
        (e) => {
          const { duration_seconds, play_sound } = e.payload
          setPlaySound(play_sound)
          // T27：ref 同步，避免 setInterval 闭包捕获 stale false
          playSoundRef.current = play_sound
          enter()
          startCountdown(duration_seconds)
        }
      )
      if (cancelled) {
        u1()
        return
      }
      unlistens.push(u1)

      const u2 = await listen(HIDE_ALL_POPUPS_EVENT, () => {
        stopInterval()
        exit()
      })
      if (cancelled) {
        u2()
        return
      }
      unlistens.push(u2)
    })()

    return () => {
      cancelled = true
      stopInterval()
      unlistens.forEach((u) => u())
    }
  }, [])

  // phase 决定容器可见性与 opacity
  const visible = phase !== "hidden"

  return (
    <div
      className="w-screen h-screen flex items-center justify-center pointer-events-none select-none"
      style={{ background: "transparent" }}
    >
      <div
        className="reminder-card pointer-events-auto"
        style={{
          opacity: phase === "entering" || phase === "shown" ? 1 : 0,
          transition: "opacity 300ms ease-out",
        }}
        // 退场用更慢的 ease-in
        // 通过 phase 切换内联 style 实现
      >
        {visible && (
          <div
            className="reminder-card-inner"
            style={{
              transition: "opacity 500ms ease-in",
              opacity: phase === "exiting" ? 0 : 1,
            }}
          >
            <div className="flex flex-col items-center justify-center gap-3 px-6 py-5">
              <p className="text-caption text-text-hint-light">休息一下</p>
              <div className="text-mega tracking-tight text-text-primary-light">
                {seconds}
              </div>
              <p className="text-caption text-text-hint-light">
                看向 6 米外的物体 {seconds > 0 ? `${seconds} 秒` : ""}
              </p>
              {!playSound && (
                <p className="text-tiny text-text-hint-light">（本次不播放声音）</p>
              )}
            </div>
            <button
              onClick={handleSkip}
              className="absolute bottom-2 right-3 text-[11px] text-text-hint-light hover:opacity-100"
              style={{ opacity: 0.3, transition: "opacity 200ms" }}
            >
              跳过
            </button>
          </div>
        )}
      </div>

      {/* 内联样式：弹窗视觉（不污染全局 CSS） */}
      <style>{`
        .reminder-card {
          width: 320px;
          height: 200px;
          border-radius: 12px;
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
          backdrop-filter: blur(20px);
          -webkit-backdrop-filter: blur(20px);
          background: rgba(255, 255, 255, 0.85);
          overflow: hidden;
          position: relative;
        }
        .reminder-card-inner {
          width: 100%;
          height: 100%;
          position: relative;
        }
        @media (prefers-color-scheme: dark) {
          .reminder-card {
            background: rgba(31, 27, 22, 0.85);
          }
        }
      `}</style>
    </div>
  )
}
