# 沐目 (MuMu)

> 为干眼患者打造的护眼工具 — 装上就忘、想起来才用、用完就想推荐

![Status](https://img.shields.io/badge/status-v0.1.0%20released-brightgreen)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)
![License](https://img.shields.io/badge/license-MIT-green)
[![Downloads](https://github.com/studyllm/mumu.git)](https://github.com/studyllm/mumu/releases/latest)
## 这是什么

我是一名干眼症患者，每天对着电脑 10 小时以上。

干眼最折磨人的不是眼睛干涩本身，而是**眼睛保养这件事根本不在我的工作流里**。我知道该滴眼药水、该热敷眼罩、该每 20 分钟看远处，但一忙起来就忘。一忘就是几小时，等反应过来眼睛已经又干又疼了。

所以我做了**沐目**——一款把"眼睛保养"嵌进你工作流的小工具：
- 滴眼药水的间隔到了，右下角安静地提醒你一下
- 热敷眼罩的时间到了，不抢你焦点地提示你
- 20-20-20 法则该休息了，倒计时归零一声木鱼

不替你做决定，只在你忘掉的时候轻轻拍一下你的肩膀。

## 下载

👉 [下载 v0.1.0 安装包](https://github.com/studyllm/mumu/releases/latest) — `沐目_0.1.0_x64-setup.exe` (2.85 MB)

安装步骤：双击安装包 → 选择安装目录 → 完成。卸载：控制面板 → 程序与功能 → 沐目。

## 沐目做什么

1. **记得你忘的事** — 滴眼药水、热敷眼罩、20-20-20 休息
2. **不抢你的工作流** — 弹窗轻、可跳过、可一键暂停 30 分钟
3. **让你看到自己** — 今日屏幕使用时长 + 休息次数，不评判只记录

## 为什么是 Tauri

干眼患者用的工具，需要**常驻后台 + 存在感低**。Tauri 相比 Electron：

| 维度 | Electron | Tauri |
|------|---------|-------|
| 安装包 | ~80 MB | ~5 MB |
| 常驻内存 | 150-250 MB | 10-30 MB |
| 后台 CPU | 1-3% | < 0.1% |

沐目当前：安装包 2.85 MB / 内存 33 MB / CPU 0.26% / 启动 70 ms。

## 项目状态

✅ **v0.1.0 已发布**。核心功能完整：

- [x] 强提醒弹窗（20-20-20 法则）
- [x] 弱提示（眼药水 / 热敷）
- [x] 每日屏幕使用统计
- [x] 工作时段设置
- [x] 锁屏自动暂停
- [x] 托盘菜单（暂停 30 分钟 / 1 小时 / 到明早 9 点）
- [x] 开机自启
- [x] 卸载清理
- [x] 性能达标（内存 33MB / CPU 0.26% / 启动 70ms）
- [x] 视觉规范 2/3 完全对齐（颜色 + 状态切换）

完整任务清单与开发过程见 [`openspec/changes/archive/2026-07-20-add-mumu-eye-care/`](openspec/changes/archive/2026-07-20-add-mumu-eye-care/)。

## v0.2 backlog（不在本次发布范围）

色温调节、历史趋势周报、macOS 移植、真机锁屏 API、托盘图标渐变动画、倒计时数字平滑过渡。

## 技术栈

- **前端**：React 19 + TypeScript + Vite + Tailwind CSS + Zustand + Radix UI
- **后端**：Rust 1.78+ + Tauri 2.x + tokio + rusqlite + serde
- **打包**：Tauri 内置 NSIS（生成 .exe 安装包）

## 开发

```bash
# 安装依赖
npm install

# 开发模式（自动打开 Tauri 窗口）
npm run tauri dev

# 打包发布（生成 .exe 安装包）
npm run tauri build
```

## 反馈

- **GitHub Issues**：[issues](../../issues)
- **微信群**：待建立

## 许可

MIT License — 你可以自由使用、修改、分发。全文见 [LICENSE](LICENSE)。

## 隐私

沐目是**纯本地工具**，不联网、不上传数据。详见 [PRIVACY.md](PRIVACY.md)。

## 用户文档

面向最终用户的上手说明见 [USER_GUIDE.md](USER_GUIDE.md)。

## 发布说明

- [RELEASE_NOTES.md](RELEASE_NOTES.md) — v0.1.0 发布说明
- [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) — 手动发布流程（git 仓库 + GitHub Release）

## 致谢

沐目是我为自己做的一个工具。开源是因为我相信很多和我一样的干眼患者需要这款工具。

如果你也是干眼患者，欢迎试用、反馈、贡献代码。
