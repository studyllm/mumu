/**
 * 弱提示弹窗相关类型
 *
 * Rust ReminderCommand::ShowSoftPrompt 反序列化结构
 */

export type SoftPromptKind = "eye_drop" | "warm_compress"

export interface ShowSoftPromptPayload {
  kind: SoftPromptKind
  message: string
}

export const SHOW_SOFT_PROMPT_EVENT = "show-soft-prompt"
export const HIDE_SOFT_PROMPT_EVENT = "hide-soft-prompt"
