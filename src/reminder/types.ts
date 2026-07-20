/**
 * 强提醒弹窗相关类型
 *
 * Rust ReminderCommand::ShowStrongReminder 反序列化结构
 */

export interface ShowStrongReminderPayload {
  duration_seconds: number
  /** 跨工作时段（离下班 20 分钟内）静默 */
  mute_sound: boolean
  /** T12：综合用户 play_sound 设置 + mute_sound 后，前端是否应播放木鱼声 */
  play_sound: boolean
}

export const SHOW_STRONG_REMINDER_EVENT = "show-strong-reminder"
export const HIDE_ALL_POPUPS_EVENT = "hide-all-popups"
