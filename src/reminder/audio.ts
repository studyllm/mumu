/**
 * 音频播放（T12 木鱼声）
 *
 * 用 Web Audio API 而非 <audio> 元素：
 * - 更精确的延迟控制（短促一击不能拖泥带水）
 * - 不需要 DOM 节点，干净
 * - 遵循系统音量（AudioContext 默认 output device）
 *
 * AudioContext 必须在用户交互后才允许播放（浏览器策略）；
 * 第一次播放前用户必须有点击/键盘动作。第一次解锁由"打开主界面"或"设置页"
 * 等任意 UI 交互触发；倒计时归零不算 UI 交互——但 spec 要求倒计时归零播放，
 * 所以让 AudioContext 在弹窗出现的瞬间 resume（用户点过触发弹窗的某个动作，
 * 比如打开设置后点"测试提醒"，或自然 reminder 触发——此时用户已 active）。
 */

let _ctx: AudioContext | null = null
let _buffer: AudioBuffer | null = null
let _loadingPromise: Promise<AudioBuffer | null> | null = null

function getCtx(): AudioContext | null {
  if (!_ctx) {
    try {
      const Ctx =
        (window as unknown as { webkitAudioContext?: typeof AudioContext })
          .webkitAudioContext ?? window.AudioContext
      _ctx = new Ctx()
    } catch (e) {
      console.error("AudioContext init failed", e)
      return null
    }
  }
  return _ctx
}

/** 提前加载并解码音频（用户首次点击后调用） */
export async function preloadWoodenFish(): Promise<void> {
  if (_buffer || _loadingPromise) return
  _loadingPromise = (async () => {
    const ctx = getCtx()
    if (!ctx) return null
    try {
      const res = await fetch("/sounds/wooden_fish.wav")
      if (!res.ok) {
        console.error("wooden_fish.wav fetch failed", res.status)
        return null
      }
      const arr = await res.arrayBuffer()
      _buffer = await ctx.decodeAudioData(arr)
      return _buffer
    } catch (e) {
      console.error("wooden_fish.wav decode failed", e)
      return null
    }
  })()
  await _loadingPromise
}

/** 播放木鱼声（一次） */
export async function playWoodenFish(): Promise<void> {
  const ctx = getCtx()
  if (!ctx) return
  if (ctx.state === "suspended") {
    await ctx.resume()
  }
  if (!_buffer) {
    await preloadWoodenFish()
  }
  if (!_buffer) return
  const src = ctx.createBufferSource()
  src.buffer = _buffer
  src.connect(ctx.destination)
  src.start(0)
}
