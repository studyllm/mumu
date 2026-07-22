/**
 * 弱提示弹窗（T10.5 / T33 / T34）
 *
 * 规格：openspec/changes/add-mumu-eye-care/specs/ui/spec.md § Soft prompt
 *
 * 视觉与动画：
 * - 与 reminder 强提醒视觉一致：320×200 半透明 + 20px backdrop blur + 12px 圆角
 * - "休息一下"标题 + 大号倒计时 + 单行消息
 * - 300ms opacity fade-in ease-out；500ms fade-out ease-in
 *
 * 行为：
 * - 收到 show-soft-prompt → 入场 + 启动 10s 倒计时（每秒 -1），归零自动 dismiss + 木鱼声
 * - 点击跳过 → 立即 dismiss（推回后端 + 退场，不播木鱼声）
 * - 收到 hide-soft-prompt → 立即退场
 * - 不抢焦点（依赖窗口 focus:false）
 *
 * T35 修复：phase=hidden 时把 html/body 背景设为透明，否则
 * index.css 的 body { background-color:#FAF8F5 } 会让窗口残留米白色
 * （即使 React 已 return null，body 默认背景仍由全局 CSS 渲染）。
 * 同时把 commitSkip 的窗口 hide 放到 setTimeout(0) 让它在 React 提交之
 * 后再触发，避免和 React 渲染竞争。
 */

import { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import {
  HIDE_SOFT_PROMPT_EVENT,
  SHOW_SOFT_PROMPT_EVENT,
  type ShowSoftPromptPayload,
} from "./types"
import { playWoodenFish, preloadWoodenFish } from "../reminder/audio"

type Phase = "hidden" | "entering" | "shown" | "exiting"

const TOTAL_SECONDS = 10

export function SoftPrompt() {
  const [phase, setPhase] = useState<Phase>("hidden")
  const [message, setMessage] = useState<string>("")
  const [kind, setKind] = useState<"eye_drop" | "warm_compress" | null>(null)
  const [secondsLeft, setSecondsLeft] = useState<number>(TOTAL_SECONDS)
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
    // T35：把窗口 hide 推迟到下一帧——React setPhase 先完成、退出动画
    // 已经开始后，再发 hide RPC，避免和 React 渲染竞争导致 hide 被丢弃
    setTimeout(() => {
      getCurrentWindow()
        .hide()
        .catch((e) => console.error("softprompt window hide failed", e))
    }, 0)
    setTimeout(() => {
      setPhase("hidden")
      setMessage("")
      setKind(null)
      setSecondsLeft(TOTAL_SECONDS)
      dismissCommittedRef.current = false
    }, 500)
  }

  const commitSkip = () => {
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
    setSecondsLeft(TOTAL_SECONDS)
    setPhase("entering")
    requestAnimationFrame(() => {
      requestAnimationFrame(() => setPhase("shown"))
    })
    clearTimers()
    tickIntervalRef.current = setInterval(() => {
      setSecondsLeft((prev) => (prev > 1 ? prev - 1 : prev))
    }, 1000)
    autoTimerRef.current = setTimeout(() => {
      setSecondsLeft(0)
      playWoodenFish().catch((e) => console.error("audio play failed", e))
      commitSkip()
    }, TOTAL_SECONDS * 1000)
  }

  // 后端事件订阅
  useEffect(() => {
    const unlistens: UnlistenFn[] = []
    let cancelled = false

    // 与 reminder 共用同一个音频；提前 preload + 解锁 AudioContext
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
        commitSkip()
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

  // phase 决定容器可见性与 opacity
  const visible = phase !== "hidden"

  // T35：phase=hidden 时给 body 设透明背景，避免 WebView2 在 transparent 窗口
  // 里仍然渲染 index.css 的 #FAF8F5 米白底 → 白框残留
  useEffect(() => {
    if (typeof document === "undefined") return
    if (phase === "hidden") {
      document.documentElement.style.background = "transparent"
      document.body.style.background = "transparent"
    } else {
      document.documentElement.style.background = ""
      document.body.style.background = ""
    }
  }, [phase])

  return (
    <div
      className="w-screen h-screen flex items-center justify-center pointer-events-none select-none"
      style={{ background: "transparent" }}
    >
      {visible && (
        <div
          className="softprompt-card pointer-events-auto"
          style={{
            opacity: phase === "exiting" ? 0 : 1,
            transition:
              phase === "exiting"
                ? "opacity 500ms ease-in"
                : "opacity 300ms ease-out",
          }}
        >
          <div className="softprompt-card-inner">
            <div className="flex flex-col items-center justify-center gap-3 px-6 py-5">
              <p className="text-caption text-text-hint-light">
                {kind === "warm_compress" ? "该热敷眼罩了" : "该滴眼药水了"}
              </p>
              <div className="text-mega tracking-tight text-text-primary-light">
                {secondsLeft}
              </div>
              <p className="text-caption text-text-hint-light">
                {kind === "warm_compress"
                  ? "热敷 10 分钟缓解眼干"
                  : "休息一下眼睛"}
              </p>
              {message && message !== (kind === "warm_compress" ? "该热敷眼罩了" : "该滴眼药水了") && (
                <p className="text-tiny text-text-hint-light">{message}</p>
              )}
            </div>
            <button
              onClick={commitSkip}
              className="absolute bottom-2 right-3 text-[11px] text-text-hint-light hover:opacity-100"
              style={{ opacity: 0.3, transition: "opacity 200ms" }}
            >
              跳过
            </button>
          </div>
        </div>
      )}

      <style>{`
        /* 与 reminder 强提醒视觉一致：320×200 + backdrop blur + 圆角 + 阴影 */
        .softprompt-card {
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
        .softprompt-card-inner {
          width: 100%;
          height: 100%;
          position: relative;
        }
        @media (prefers-color-scheme: dark) {
          .softprompt-card { background: rgba(31, 27, 22, 0.85); }
        }
      `}</style>
    </div>
  )
}
