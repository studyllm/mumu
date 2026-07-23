/**
 * T36：主界面"上次提醒 / 下次倒计时"快照类型
 *
 * 后端 reminders.rs::ReminderSnapshot 通过 serde rename_all="camelCase"
 * 序列化后字段命名与此一致（DateTime<Local> → ISO string）。
 *
 * NextHint 是 tagged union：
 * - { kind: "paused" } / { kind: "outOfWorkHours" } / { kind: "disabled" }
 * - { kind: "warmCompressAlreadyToday", value: { nextAtHhmm: "13:00" } }
 * - { kind: "eyeDropDismissedLimit", value: { nextAtHhmm: "09:00" } }
 *
 * 注意：NextHint 的内部字段是 enum variant 的 field name，
 * serde 默认不递归 rename_all，所以字段名仍是 snake_case 形如 next_at_hhmm。
 * 在 serde 反序列化到 camelCase 顶层之后，这是单层结构，按 Rust 字段名直传。
 */

export type NextHint =
  | { kind: "paused" }
  | { kind: "outOfWorkHours" }
  | { kind: "disabled" }
  | { kind: "warmCompressAlreadyToday"; value: { hhmm: string } }
  | { kind: "eyeDropDismissedLimit"; value: { hhmm: string } }

export interface ReminderSnapshot {
  lastStrongAt: string | null
  lastEyeDropAt: string | null
  lastWarmCompressAt: string | null
  /** 各提醒距下次触发的秒数（null = 不应触发：未启用 / 非工作时段 / 暂停 / 已 dismiss 满 3 次 / 热敷今日已触发） */
  nextStrongSecs: number | null
  nextEyeDropSecs: number | null
  nextWarmCompressSecs: number | null
  inWorkHours: boolean
  isPaused: boolean
  eyeDropEnabled: boolean
  warmCompressEnabled: boolean
  /** T36+：距离手动暂停到期的剩余秒数（null = 当前未暂停） */
  pauseRemainingSecs: number | null
  /** 各 hint 的具体原因（null = 没有特殊原因，沿用 next_*_secs / 倒计时） */
  strongHint: NextHint | null
  eyeDropHint: NextHint | null
  warmCompressHint: NextHint | null
}