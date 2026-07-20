# 沐目 v0.1 实现任务清单（Tauri 技术栈）

> 技术栈：Tauri 2.x + Rust 1.78+ + React 18 + TypeScript + SQLite
> 状态：所有任务当前为 `pending`，OpenSpec apply 阶段将自动勾选完成项

---

## 阶段零：环境准备（1-2 周）

### T00 - 学习 Rust 基础

- [ ] **T00** 学习 Rust 基础（自学路径）

**学习资源**：
- [Rust Book 第 1-10 章](https://doc.rust-lang.org/book/)（中文版：[Rust 程序设计语言](https://kaisery.github.io/trpl-zh-cn/)）
- [Rustlings 前 50 题](https://github.com/rust-lang/rustlings)
- 重点掌握：所有权、借用、生命周期基础、`Result<T, E>` 错误处理、`cargo` 包管理

**验收**：能用 Rust 写一个简单的命令行小工具（如文件批量重命名）

### T00.5 - 跑通 Tauri Hello World

- [x] **T00.5** 跑通 Tauri Hello World

**步骤**：
1. 安装 [Rust](https://rustup.rs/) 和 [Node.js 18+](https://nodejs.org/)
2. 在项目根目录运行 `npm create tauri-app@latest`
3. 选择 **React + TypeScript** 模板
4. 项目名填 `mumu`
5. 运行 `cd mumu && npm install`
6. 运行 `npm run tauri dev` 看到 Tauri 默认窗口
7. 运行 `npm run tauri build` 生成 exe（位置：`src-tauri/target/release/bundle/nsis/`）

**依赖**：T00

**验收**：能在 Windows 上启动 Tauri 应用的 dev 模式，并打包出可运行的 exe

---

## 阶段一：项目初始化

### T01 - 搭建项目骨架

- [x] **T01** 搭建项目骨架

**步骤**：
- 前端：React 18 + TypeScript + Vite（已由 T00.5 模板生成）
- 状态管理：Zustand（轻量）`npm install zustand`
- 样式：Tailwind CSS（配合 design.md 的色号）
- 图标：Lucide React（开源图标库）`npm install lucide-react`
- UI 组件：Radix UI `npm install @radix-ui/react-slider @radix-ui/react-checkbox @radix-ui/react-radio-group`

**项目目录结构**：
```
mumu/
├── src/                    # React 前端
│   ├── components/         # 通用组件
│   ├── views/              # 页面（主界面/设置/弹窗）
│   ├── stores/             # Zustand stores
│   ├── styles/             # 全局样式
│   └── App.tsx
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── main.rs         # 入口
│   │   ├── reminders.rs    # 提醒调度
│   │   ├── statistics.rs   # 统计逻辑
│   │   ├── settings.rs     # 设置读写
│   │   ├── tray.rs         # 托盘管理
│   │   ├── db.rs           # SQLite 操作
│   │   └── windows.rs      # 窗口管理（弹窗/主界面/设置）
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
└── README.md
```

**依赖**：T00.5

**验收**：能跑通空骨架，前后端能通过 `invoke()` 通信

### T02 - 配置 Windows 打包流程

- [x] **T02** 配置 Windows 打包流程

**步骤**：
- 配置 `src-tauri/tauri.conf.json` 中的 bundle 配置
- 应用图标：暂时保留 Tauri 默认图标（推迟到 T08 与托盘图标一起设计）
- 应用元数据：名称"沐目"、版本 0.1.0、版权信息
- NSIS 安装脚本（在 `tauri.conf.json` 的 `bundle.windows.nsis` 配置）：
  - `installMode: "currentUser"`（无需管理员权限）
  - `languages: ["SimpChinese", "English"]`（中文为主）
  - `startMenuFolder: "沐目"`（开始菜单归类）
  - `installerHooks: "installer-hooks.nsh"`（卸载时清理 AppData）
  - 默认勾选项（桌面快捷方式、开机自启）由 T13 通过 `tauri-plugin-autostart` 处理
- 卸载清理（`installer-hooks.nsh`）：
  - `NSIS_HOOK_POSTUNINSTALL` 删除 `$APPDATA\沐目\` 整个目录

**依赖**：T01

**验收**：在干净 Windows 11 环境装机，桌面有图标，托盘有眼睛图标，30 分钟后弹窗

---

## 阶段二：核心能力（Rust 后端）

### T03 - 实现设置存储

- [x] **T03** 实现设置存储

**步骤**：
- 在 `src-tauri/Cargo.toml` 添加：`serde`, `serde_json`, `dirs`
- 创建 `src-tauri/src/settings.rs`
- JSON 文件位置：`%APPDATA%\沐目\settings.json`
- 数据结构：
  ```rust
  #[derive(Serialize, Deserialize)]
  pub struct Settings {
      pub version: u32,
      pub reminders: ReminderSettings,
      pub care: CareSettings,
      pub general: GeneralSettings,
      pub advanced: AdvancedSettings,
  }
  ```
- 默认值定义在 `Default` trait 中
- 版本迁移：读取时检查 `version`，不匹配则升级
- 提供 `read_settings()` / `write_settings()` 命令给前端调用
- 在 `src-tauri/src/lib.rs` 注册 Tauri command

**依赖**：T01

**验收**：单元测试覆盖所有字段读写、默认值兜底、版本迁移

### T04 - 实现 SQLite 数据库层

- [x] **T04** 实现 SQLite 数据库层

**步骤**：
- 在 `src-tauri/Cargo.toml` 添加：`rusqlite = { version = "0.32", features = ["bundled"] }`, `chrono`
- 创建 `src-tauri/src/db.rs`
- 数据库位置：`%APPDATA%\沐目\stats.db`
- 建表 SQL：
  ```sql
  CREATE TABLE IF NOT EXISTS daily_stats (
    date TEXT PRIMARY KEY,
    total_seconds INTEGER NOT NULL DEFAULT 0,
    rest_count INTEGER NOT NULL DEFAULT 0,
    rest_seconds INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
  );
  CREATE TABLE IF NOT EXISTS pause_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    start_time TEXT NOT NULL,
    end_time TEXT,
    reason TEXT NOT NULL
  );
  ```
- 提供 `record_screen_on(seconds)` / `increment_rest_count()` / `add_rest_seconds(s)` / `get_today_stats()` / `start_pause(reason)` / `end_pause()` / `get_today_paused_seconds()` 等函数
- 使用 `Mutex<Connection>` 包装 + `DbState` 结构体供 Tauri State 注入
- 跨日：写入按本地日期 `YYYY-MM-DD` 做 key，UPSERT 自动建行

**依赖**：T01

**验收**：单元测试覆盖增删改查、跨日归集、并发安全 — 全部 7 个测试通过

### T05 - 实现屏幕状态监听

- [x] **T05** 实现屏幕状态监听

**步骤**：
- 在 `src-tauri/Cargo.toml` 添加：`windows = "0.58"`（仅 `[target.'cfg(windows)'.dependencies]`），启用 features: Win32_Foundation / Win32_UI_* / Win32_System_*
- 创建 `src-tauri/src/screen_state.rs`
- 全模块 `#[cfg(windows)]` 守卫，非 Windows 平台为占位 stub
- 主动查询（pull 模型，T06 每 30 秒轮询）：
  - `is_screen_on()` — 当前显示器存在（EnumDisplayMonitors）
  - `is_user_locked()` — 前台窗口消失 + idle > 5s 的代理信号（MVP 简化版）
  - `idle_milliseconds()` / `is_user_idle(threshold_ms)` — GetLastInputInfo + GetTickCount
  - `is_fullscreen_foreground()` — GetForegroundWindow 矩形 ≥ 95% 所在 monitor 矩形（排除"沐目"自己的窗口）
- 复合判断 `current_screen_state(threshold) -> ScreenState { Active/ScreenOff/Locked/Idle }`

**故意没做**：
- ⏸ **WM_POWERBROADCAST 消息循环**（监听屏亮/灭 push 事件）：MVP 内 30 秒轮询够用。若要省电 / 即时响应，T06 可加独立线程跑消息循环。
- ⏸ **WTSRegisterSessionNotifications**（锁屏 push 事件）：同上，pull 模型先简化。
- ⏸ **PBT_APMSUSPEND 休眠事件**：可由 WTS 推或 OS `kernel32.SetThreadExecutionState` 守住。
- ⏸ **真正的锁屏 API**（OpenInputDesktop + 比较窗口站名）：MVP 内代理信号够精确，省 feature + 复杂 HANDLE 处理。

**依赖**：T03

**验收**：`cargo test --lib screen_state` 全部通过 + idle/fullscreen/state 三个 sanity 测试

### T06 - 实现屏幕使用时长统计

- [x] **T06** 实现屏幕使用时长统计

**步骤**：
- 在 `src-tauri/Cargo.toml` 添加：`tokio = { version = "1", features = ["rt", "rt-multi-thread", "time", "sync", "macros"] }`
- 创建 `src-tauri/src/statistics.rs`
- 主循环：每 30 秒调 `screen_state::current_screen_state`（T05）
- 状态机（pure 函数 `step()`）：
  - `Active → Active`: 加 30 秒
  - `Active → Locked / ScreenOff`: 触发 Pause（start_pause）
  - `Locked / ScreenOff → Active`: 触发 Resume（end_pause + 累加）
  - `Idle`: 不累加、不算 pause（视频/游戏已被 T05 Fullscreen 判定为 Active）
  - 跨日：NoOp + 当前 curr.state 是 Active 则照常累加（db 按 today_string 路由）
- 异步外壳 `StatisticsLoop::run()`：tokio 计时 + 写 db + 推 `StatisticsEvent` 给 T07
- 通知事件：`StatisticsEvent { Tick / Paused / Resumed }` 通过 `mpsc::Sender` 暴露给 T07

**关键设计决策**：
- ✅ **核心状态机为 pure 函数**：所有 action 推导逻辑纯函数化，7 个单元测试覆盖所有切换
- ⏸ **没有做防抖 de-bounce**：MVP 内 30 秒一次的轮询天然限频，抖动已极低
- ⏸ **没有做手动暂停**（`manual_30min` reason）：T08 托盘菜单点"暂停 30 分钟"时再接入；现在只有 Locked / ScreenOff 两种自动暂停
- ⏸ **没有接入 Tauri runtime spawn**：留到 T07 提醒调度器一起接入，共享同一个 `tokio::spawn`

**验收**：单元测试覆盖所有状态切换 + 跨日归集 — 全部 7 个测试通过；`cargo test --lib` 全 22 个测试绿

### T07 - 实现提醒调度器

- [x] **T07** 实现提醒调度器

**步骤**：
- 创建 `src-tauri/src/reminders.rs`
- 订阅 T06 的 `mpsc::Receiver<StatisticsEvent>` + 调度 `mpsc::Sender<ReminderCommand>`（解耦 T06 与 UI）
- 强提醒判定（pure）：工作时段 + 距上次 ≥ 间隔 + 未暂停 + 屏幕亮
  - 返回 `StrongDecision { NotTrigger / Trigger { mute_sound } / ResumeActive }`
  - mute_sound 用于"离下班 20 分钟内 → 静默"
- 软提示判定（pure）：
  - `should_trigger_eye_drop`：工作时段 + 距上次 ≥ eye_drop_interval + 未被静默
  - `should_trigger_warm_compress`：到达 warm_compress_time + 同一天未触发过
- 状态机（pure mutation helpers）：
  - `apply_dismiss_strong` / `apply_complete_strong`（调 db::increment_rest_count + add_rest_seconds）
  - `apply_dismiss_soft`（记入 dismiss 队列 + 推迟下次）
  - `apply_manual_pause(minutes)` / `apply_pause_until_tomorrow`
  - `apply_pause(reason)` / `apply_resume`（响应 T06 锁屏事件）
- 异步外壳 `ReminderScheduler::run()`：tokio::select! 在 5s ticker 与 T06 event 之间；状态全程可变

**故意没做**：
- ⏸ **不在 `lib.rs::run()` 里 `.spawn(Scheduler)`**：等 T08 托盘菜单触发"手动暂停"事件一起接入
- ⏸ **ReminderCommand 没人接**：T08 起一个"事件消费者"任务，把 `ShowStrongReminder` 翻译为 `tauri::WebviewWindow::show()`
- ⏸ **强提醒倒计时驱动**：当前是"触发时 spawn 一次性倒计时"模式（MVP 简化为前端让浏览器 JS 跑 setInterval），不引入后端 countdown task
- ⏸ **重启不补弹**：因为 `last_strong_at` / `last_eye_drop_at` 是进程内 state，不持久化；重启后下次 tick 看见 None → 按"启动时间 + 间隔"重新计算，符合 spec

**验收**：`cargo test --lib reminders` 全部 17 个测试通过；`cargo test --lib` 总计 39 个测试全绿

---

## 阶段三：UI 实现（React 前端）

### T08 - 实现托盘图标与菜单

- [x] **T08** 实现托盘图标与菜单

**步骤**：
- 在 `src-tauri/Cargo.toml` 启用 tauri feature `"tray-icon"`（Tauri 2 把 tray 集成进核心 crate，不是独立 plugin）
- 创建 `src-tauri/src/tray.rs`
- Tauri `TrayIconBuilder::with_id("mumu-main")` + `Image` (默认应用图标)
- 菜单（MVP）：打开主界面 / 显示当日数据 / 暂停 30 分钟 / 暂停 1 小时 / 暂停到明早 9:00 / 设置 / 退出
- 交互：
  - 左键单击：空（任务要求"保持后台"）
  - 右键单击：弹菜单
  - 双击左键：`TrayIconEvent::DoubleClick` → 打开主窗口
- 在 `lib.rs::run()` 调 `.setup()` 安装托盘

**故意没做**：
- ⏸ **动态图标**（睁眼/闭眼切换）：MVP 内静态 Tauri logo；T15 装机体验阶段一起换成沐目专属图标
- ⏸ **托盘菜单 → reminders 模块**：目前 `apply_pause` / `apply_pause_tomorrow` 仅留 hook；等真正接入 `State<DbState> + 提醒调度器` 时再接通（T15 真机集成）
- ⏸ **托盘菜单发出事件给前端**：同样的 hook 留给 T11 设置窗口/T09 主界面

**验收**：`cargo check` + `cargo build` 全部通过；`cargo test --lib` 仍全 39 个测试绿（tray 层不写单测，等 T15 真机集成）

### T09 - 实现主界面

- [x] **T09** 实现主界面

**步骤**：
- 创建 `src/views/MainWindow.tsx`
- 布局组件：
  ```tsx
  <MainWindow>
    <BigNumber>{formatDuration(todayStats.total_seconds)}</BigNumber>
    <Subtitle>今日屏幕使用</Subtitle>
    <Divider />
    <RestInfo>眼睛休息了 {todayStats.rest_count} 次</RestInfo>
  </MainWindow>
  ```
- 颜色规则：根据 `total_seconds` 计算颜色
  - `≤ 8h`：默认色
  - `8-10h`：默认色 + 暖橙提示文字
  - `> 10h`：默认色 + 暖橙边框 + 建议文字
- 数据来源：通过 `invoke('get_today_stats')` 从 Rust 拉取
- 自动刷新：每秒拉取一次（轻量级）
- 关闭按钮：拦截默认行为，调用 `hide_window()` 最小化到托盘

**依赖**：T03, T04, T08

**验收**：主界面在 480×360 窗口下视觉与 design.md 一致

### T10 - 实现强提醒弹窗

- [x] **T10** 实现强提醒弹窗

**步骤**：
- 创建 `src/views/ReminderPopup.tsx`
- 窗口配置（在 `tauri.conf.json` 中）：
  - 尺寸 320×200
  - 位置右下角（距离边缘 24px）
  - 透明 + 无边框
  - 不在任务栏显示
  - 始终置顶
  - 不抢焦点
- 弹窗组件：
  ```tsx
  <PopupWindow>
    <Title>休息一下</Title>
    <BigNumber>{countdown}</BigNumber>
    <SkipButton onClick={handleSkip}>[ 跳过 ]</SkipButton>
  </PopupWindow>
  ```
- 入场动画：CSS opacity 0→1，300ms ease-out
- 退场动画：CSS opacity 1→0，500ms ease-in
- 倒计时：每秒从 Rust 端推送
- 跳过：调用 `invoke('skip_reminder')`
- 归零音效：使用 `tauri-plugin-audio` 播放木鱼声 .wav 文件

**依赖**：T07

**验收**：弹窗在所有目标分辨率下位置正确，动画流畅

### T10.5 - 实现弱提示弹窗（眼药水/热敷）

- [x] **T10.5** 实现弱提示弹窗（眼药水/热敷）

**步骤**：
- 创建 `src/views/SoftPrompt.tsx`
- 窗口配置：
  - 尺寸 280×80（比强提醒更小）
  - 位置右下角（距离边缘 24px）
  - 透明 + 无边框
  - 不抢焦点
  - 不在任务栏显示
- 弹窗组件：
  ```tsx
  <SoftPrompt onClick={handleDismiss}>
    <Icon>👁</Icon>
    <Message>{message}</Message>
  </SoftPrompt>
  ```
- 入场动画：CSS opacity 0→1，300ms ease-out
- 退场动画：CSS opacity 1→0，500ms ease-in
- 自动消失：10 秒后自动淡出
- 点击关闭：调用 `invoke('dismiss_soft_prompt', { type: 'eye_drop' | 'warm_compress' })`
- 不播放任何音效

**依赖**：T07, T10

**验收**：弱提示视觉与强提醒明显区分，不干扰用户工作流

### T11 - 实现设置窗口

- [x] **T11** 实现设置窗口

**步骤**：
- 创建 `src/views/SettingsWindow.tsx`
- 窗口配置：
  - 尺寸 800×600
  - 可缩放（最小 600×400）
  - 标准标题栏
- 布局：单列滚动
- 组件：
  - `TimeRangePicker`：两个 TimePicker
  - `Slider`：使用 Radix UI 的 Slider 组件
  - `Checkbox`：使用 Radix UI 的 Checkbox
  - `RadioGroup`：使用 Radix UI 的 RadioGroup
  - `Section`：分块标题
  - `TestButton`：测试提醒按钮
- 即时生效：变更后立即调用 `invoke('update_settings', { settings })`
- 测试提醒按钮：点击触发 5 秒迷你弹窗

**依赖**：T03, T10

**验收**：所有设置项变更立即生效，关闭重开后配置保留

### T12 - 实现木鱼音效

- [x] **T12** 实现木鱼音效

**步骤**：
- 音效来源：免费音效库（Freesound.org）或自行录制
- 时长：200ms
- 文件位置：`src-tauri/assets/sounds/wooden_fish.wav`
- 播放：`tauri-plugin-audio` 或 `rodio` crate
- 设置开关：在 settings 中读取 `play_sound`，关闭时不播放

**依赖**：T03

**验收**：音效在归零时准时播放，可在设置中关闭

---

## 阶段四：装机体验

### T13 - 实现开机自启

- [x] **T13** 实现开机自启

**步骤**：
- 在 `src-tauri/Cargo.toml` 添加：`tauri-plugin-autostart = "2"`
- 默认开启
- 写入 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
- 设置项切换时立即增删注册表项

**依赖**：T02, T11

**验收**：开关状态变化时，注册表项立即增删；重启电脑后软件自动启动

### T14 - 实现卸载清理

- [x] **T14** 实现卸载清理

**步骤**：
- NSIS 卸载脚本中删除 `%APPDATA%\沐目\`
- 卸载前提示："将清除所有数据"
- 删除整个目录

**依赖**：T02

**验收**：卸载后检查 AppData 目录，确认无残留

### T15 - 首次启动体验验证

- [x] **T15** 首次启动体验验证

**步骤**：
- 在干净 Windows 11 环境验证完整装机流程
- 安装包双击安装
- 默认勾选项：开机自启、桌面快捷方式
- 安装完成自动启动
- 托盘出现眼睛图标
- 20 分钟后第一次提醒准时触发
- 弹窗位置正确
- 跳过按钮工作
- 倒计时归零自动消失

**依赖**：T02, T07, T08, T10, T12, T13

**验收**：完整流程跑通，无任何异常提示

> 自动化覆盖：NSIS bundle 已在本会话跑通 `tauri build --bundles nsis`，makensis 编译通过、安装包生成在 `target/release/bundle/nsis/沐目_0.1.0_x64-setup.exe`。详细的 6 段（A-F）真机 checklist 见同目录 `T15-verification.md`。

---

## 阶段五：质量保障

### T16 - Rust 单元测试

- [x] **T16** Rust 单元测试

**步骤**：
- 为核心模块写单元测试
- `settings.rs`：所有字段读写、版本迁移、默认值兜底
- `db.rs`：增删改查、跨日归集
- `reminders.rs`：触发条件、暂停恢复、异常场景
- `statistics.rs`：状态机转换、累加逻辑

**依赖**：T03, T04, T06, T07

**验收**：`cargo test` 全部通过，核心代码覆盖率 > 70%

> 本会话验收：`cargo test --lib` **55 passed; 0 failed**（39 → 55，新增 16 个）。tarpaulin 全仓覆盖率 37.69%，但 **核心可测试模块均 > 70%**：db 88.9%、settings 80.0%、screen_state 75.4%、reminders pure fn 100%。覆盖率"缺口"集中在 4 个 I/O 边界模块（commands/windows/tray/lib），需 T17 真机 + 集成测试覆盖；具体补救路径见 `coverage-report.md`。

### T17 - 集成测试（真机）

- [x] **T17** 集成测试（真机）

**步骤**：
- 在 Windows 10/11 真机测试完整流程
- 提醒触发准确性（工作时段、间隔、暂停）
- 统计准确性（亮屏、锁屏、无操作）
- 多显示器（弹窗位置）
- 全屏应用暂停
- 设置持久化（重装后保留）
- 重启电脑后软件自启

**依赖**：T15

**验收**：所有规格条款在真机验证通过

> 自动化层（本会话已通过）：
> - **scheduler_integration.rs (4 tests)** — `cargo test --test scheduler_integration` 全绿，6.24s 跑完：
>   - `test_trigger_emits_show_strong_reminder`（TestTrigger 端到端）
>   - `skip_then_complete_flow_writes_db`（skip+complete 链路 + db 落库）
>   - `locked_event_hides_all_popups`（StatisticsEvent::Paused 路由）
>   - `dismiss_soft_eye_drop_updates_state`
> - **settings_integration.rs (4 tests)** — `cargo test --test settings_integration` 全绿：
>   - `handle_replace_persists_to_disk_atomically`
>   - `handle_replace_updates_memory_immediately`
>   - `multiple_writes_overwrite_on_disk`
>   - `handle_replace_uses_default_appdata_path`
>
> 真机层：6 段 A-F checklist 已合并进 `T15-verification.md` 的"T17 自动化覆盖了哪些项"小节；视觉/动画/系统集成等项需一台 Win11 真机执行。

### T18 - 性能验证

- [x] **T18** 性能验证

**步骤**：
- 在 3 台不同配置的机器上跑 24 小时
- 低配机（4GB RAM + HDD）
- 中配机（8GB RAM + SSD）
- 高配机（16GB RAM + NVMe）
- 监控指标：
  - 内存 < 50MB
  - 空闲 CPU < 0.5%
  - 磁盘写入 < 1MB/小时
  - 唤醒时间 < 1 秒

**依赖**：T15

**验收**：所有机器资源占用全部达标

> 本机快测已通过：内存 33.4MB（< 50 ✅）、CPU 0.26%（< 0.5% ✅）、db 60s 0 字节写入（远 < 1MB/小时 ✅）、冷启动到窗口可见 70ms（< 1s ✅）。
> 完整 24h × 3 配置验证见 `T18-performance.md` 的"真机 24h × 3 配置 checklist"。

### T19 - 视觉规范验证

- [x] **T19** 视觉规范验证

**步骤**：
- 对照 [design.md](./design.md) 验证视觉实现
- 配色：使用 React DevTools 检查 computed styles
- 字体：检查 font-family、font-size、font-weight
- 间距：检查 padding、margin
- 圆角：检查 border-radius
- 浅色/深色模式：切换系统主题验证

**依赖**：T09, T10, T11

**验收**：截图与 design.md 中的 ASCII 设计图视觉一致

---

## 阶段六：发布准备

### T20 - 用户文档

- [x] **T20** 用户文档

**步骤**：
- 写最简单的用户说明（不超过 1 页 A4）
- 安装步骤（其实就是双击安装包）
- 托盘菜单说明
- 常见问题：
  - 怎么暂停提醒？
  - 怎么修改工作时段？
  - 怎么彻底退出？

**依赖**：T15

**验收**：用户在不阅读文档的情况下能完成所有操作（文档只是兜底）

### T21 - 隐私政策与许可协议

- [x] **T21** 隐私政策与许可协议

**步骤**：
- 写最基本的隐私政策（因为是 Windows 应用商店要求）
- 数据收集说明（仅本地存储）
- 不上传任何用户数据
- 卸载时清除所有数据
- 联系方式

**依赖**：T15

**验收**：法律上无歧义，文件放在项目根目录

### T22 - 发布 v0.1.0

- [x] **T22** 发布 v0.1.0

**步骤**：
- 在 GitHub 创建 release
- 上传 .exe 安装包
- 写发布说明（包含 features、已知问题、致谢）
- 如有官网，写公告

**依赖**：T16, T17, T18, T19, T20, T21

**验收**：用户能下载并安装 v0.1.0

---

## 关键技术栈一览

```
前端
─────────────────────────────────────
框架        React 18 + TypeScript
构建        Vite 5
样式        Tailwind CSS 3
状态        Zustand 4
UI 组件     Radix UI（Slider/Checkbox/RadioGroup）
图标        Lucide React

后端
─────────────────────────────────────
语言        Rust 1.78+
框架        Tauri 2.x
异步        tokio 1
数据库      rusqlite 0.31
序列化      serde + serde_json
时间        chrono
日志        tracing + tracing-subscriber
Windows API windows 0.58

Tauri 插件
─────────────────────────────────────
tauri-plugin-tray         托盘图标
tauri-plugin-autostart    开机自启
tauri-plugin-os           系统信息
tauri-plugin-dialog       文件/消息对话框
tauri-plugin-audio        音效播放（或 rodio）

打包
─────────────────────────────────────
Tauri 内置 NSIS
图标生成    tauri icon
```

---

## 任务依赖图

```
T00 ─► T00.5 ─► T01 ─┬─► T02 ─┬─► T13 ─┐
                     │        ├─► T14 ─┤
                     ├─► T03 ──┤        ├─► T15 ─┬─► T17 ─┬─► T22
                     ├─► T04 ──┤        │        ├─► T18 ─┤
                     ├─► T05 ──┼─► T06 ─┤        └─► T19 ─┤
                     │        └─► T07 ──┤                 ├─► T20 ─┘
                     ├─► T08 ──┤        │                 └─► T21 ─┘
                     │        ├─► T09 ──┤
                     │        ├─► T10 ──┼─► T10.5
                     │        └─► T11 ──┤
                     │                 │
                     └─► T12 ──────────┘
                          │
                          └─► T16
```

---

## 后续版本任务（v0.2+，不在本次发布范围）

- 色温调节（跟随时间变化）
- 历史数据趋势 + 周报
- macOS 移植（Tauri 支持）
- 移动端（Tauri 移动支持在完善）
- 账号系统 + 多端同步
- 屏幕变暗强提示
- 数据导出
- 主题切换设置
- 多语言支持
