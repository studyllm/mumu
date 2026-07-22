/**
 * 弱提示弹窗（T10.5 / T33）
 *
 * 规格：openspec/changes/add-mumu-eye-care/specs/ui/spec.md § Soft prompt
 *
 * 视觉与动画：
 * - 280×80 半透明 + 20px backdrop blur + 12px 圆角
 * - 单行消息 + 倒计时（T33 加）
 * - 入场 300ms fade-in ease-out；退场 500ms fade-out ease-in
 *
 * 行为：
 * - 收到 show-soft-prompt → 入场 + 启动 10s 倒计时（每秒 -1），归零自动 dismiss
 * - 点击任意位置 → 立即 dismiss（推回后端 + 退场）
 * - 收到 hide-soft-prompt → 立即退场
 * - 不抢焦点（依赖窗口 focus:false）
 */

import { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import {
  HIDE_SOFT_PROMPT_EVENT,
  SHOW_SOFT_PROMPT_EVENT,
  type ShowSoftPromptPayload,
} from "./types"
import { playWoodenFish, preloadWoodenFish } from "../reminder/audio"

type Phase = "hidden" | "entering" | "shown" | "exiting"

const AUTO_DISMISS_MS = 10_000

export function SoftPrompt() {
  const [phase, setPhase] = useState<Phase>("hidden")
  const [message, setMessage] = useState<string>("")
  const [kind, setKind] = useState<"eye_drop" | "warm_compress" | null>(null)
  // T33：倒计时显示给用户（之前只有 10s 自动消失，没有可视化倒计时）
  const [secondsLeft, setSecondsLeft] = useState<number>(10)
  const autoTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const tickIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const dismissCommittedRef = useRef(false)

  const clearTimers = () => {
    if (autoTimerRef.current) {
      clearTimeout(autoTimerRef.current)
      autoTimerRef.current = null
    }
    if (tickIntervalRef.current) {
      clearInterval(tickIntervalRef.current)
      tickIntervalRef.current = null
    }
  }

  const exit = () => {
    setPhase("exiting")
    setTimeout(() => {
      setPhase("hidden")
      setMessage("")
      setKind(null)
      setSecondsLeft(10)
      dismissCommittedRef.current = false
    }, 500)
  }

  const commitDismiss = () => {
    if (dismissCommittedRef.current) return
    dismissCommittedRef.current = true
    clearTimers()
    if (kind) {
      invoke("softprompt_dismiss", { kind })
        .catch((e) => console.error("softprompt_dismiss failed", e))
    }
    exit()
  }

  const enter = (msg: string, k: "eye_drop" | "warm_compress") => {
    setMessage(msg)
    setKind(k)
    setSecondsLeft(10)
    setPhase("entering")
    requestAnimationFrame(() => {
      requestAnimationFrame(() => setPhase("shown"))
    })
    clearTimers()
    // T33：每秒 -1 显示给用户
    tickIntervalRef.current = setInterval(() => {
      setSecondsLeft((prev) => (prev > 1 ? prev - 1 : prev))
    }, 1000)
    autoTimerRef.current = setTimeout(() => {
      // T33：倒计时归零也播木鱼声（与强提醒归零一致体验）
      playWoodenFish().catch((e) => console.error("audio play failed", e))
      commitDismiss()
    }, AUTO_DISMISS_MS)
  }

  // 后端事件订阅
  useEffect(() => {
    const unlistens: UnlistenFn[] = []
    let cancelled = false

    // T33：与 reminder 共用同一个音频；提前 preload + 解锁 AudioContext
    preloadWoodenFish()

    ;(async () => {
      const u1 = await listen<ShowSoftPromptPayload>(
        SHOW_SOFT_PROMPT_EVENT,
        (e) => {
          enter(e.payload.message, e.payload.kind)
        }
      )
      if (cancelled) {
        u1()
        return
      }
      unlistens.push(u1)

      const u2 = await listen(HIDE_SOFT_PROMPT_EVENT, () => {
        // 后端主动隐藏（锁屏 / 暂停）—— 同样推回 dismiss 让状态机更新队列
        commitDismiss()
      })
      if (cancelled) {
        u2()
        return
      }
      unlistens.push(u2)
    })()

    return () => {
      cancelled = true
      clearTimers()
      unlistens.forEach((u) => u())
    }
  }, [])

  if (phase === "hidden") return null

  return (
    <div className="w-screen h-screen flex items-center justify-center pointer-events-none select-none">
      <button
        onClick={commitDismiss}
        aria-label="关闭提示"
        className="softprompt-card pointer-events-auto block"
        style={{
          opacity: phase === "exiting" ? 0 : 1,
          transition:
            phase === "exiting"
              ? "opacity 500ms ease-in"
              : "opacity 300ms ease-out",
        }}
      >
        <div className="softprompt-inner flex items-center gap-3 px-4 h-full">
          <span className="softprompt-icon" aria-hidden>
            {kind === "warm_compress" ? "♨" : "💧"}
          </span>
          <span className="softprompt-msg">{message}</span>
          <span className="softprompt-countdown" aria-label={`${secondsLeft} 秒后自动关闭`}>
            {secondsLeft}
          </span>
        </div>
      </button>

      <style>{`
        .softprompt-card {
          width: 280px;
          height: 80px;
          border-radius: 12px;
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
          backdrop-filter: blur(20px);
          -webkit-backdrop-filter: blur(20px);
          background: rgba(255, 255, 255, 0.85);
          border: none;
          padding: 0;
          cursor: pointer;
          text-align: left;
          font-family: inherit;
          color: inherit;
        }
        .softprompt-inner { width: 100%; }
        .softprompt-icon {
          font-size: 22px;
          line-height: 1;
          flex-shrink: 0;
        }
        .softprompt-msg {
          font-size: 14px;
          color: #2C2825;
          font-family: "Microsoft YaHei", "PingFang SC", "Noto Sans CJK SC", sans-serif;
          letter-spacing: 0.02em;
          flex: 1;
        }
        /* T33：倒计时数字——右下小号，与消息保持视觉层级 */
        .softprompt-countdown {
          font-size: 18px;
          font-weight: 600;
          color: #87A878;
          font-variant-numeric: tabular-nums;
          flex-shrink: 0;
          min-width: 20px;
          text-align: right;
        }
        @media (prefers-color-scheme: dark) {
          .softprompt-card { background: rgba(31, 27, 22, 0.85); }
          .softprompt-msg { color: #F5F0E8; }
          .softprompt-countdown { color: #A8C29B; }
        }
      `}</style>
    </div>
  )
}
