# 沐目 v0.1.0

> 为干眼患者打造的轻量护眼工具 · 首次公开发布

**发布日期**：2026-07-20
**许可协议**：MIT
**下载**：下方 Assets 中的 `沐目_0.1.0_x64-setup.exe`

---

## 简介

沐目是一款 Windows 桌面护眼工具，遵循 **20-20-20 法则**（每 20 分钟看 20 英尺外的物体 20 秒），定时提醒你休息、滴眼药水、热敷。

核心特点：
- 🪶 **极轻量**：Rust 主进程 11.7 MB 安装包 2.9 MB，常驻内存 33 MB
- 🔒 **纯本地**：所有设置和统计数据只存在你电脑里，不上传任何服务器
- 🎨 **克制视觉**：薄荷绿 + 米白，不打扰你的工作流
- 🌙 **后台常驻**：屏幕亮时睁眼图标，锁屏时闭眼图标

---

## 主要功能

- ✅ **强提醒弹窗**（20-20-20）：默认每 20 分钟右下角弹 5 秒倒计时，可跳过可关闭
- ✅ **弱提示**（眼药水 / 热敷）：可自定义间隔，默认 60 分钟滴眼药水提醒
- ✅ **每日统计**：在主窗口看到今日屏幕使用时长 + 休息次数
- ✅ **工作时段**：只在设定的工作时段提醒（如 09:00-18:00）
- ✅ **锁屏自动暂停**：离开电脑后自动暂停统计，回来无缝续接
- ✅ **托盘菜单**：暂停 30 分钟 / 1 小时 / 到明早 9 点
- ✅ **开机自启**：默认开启，可在设置里关闭
- ✅ **卸载清理**：卸载时可选清除所有数据
- ✅ **木鱼声**：归零时一声木鱼（可关闭）

---

## 安装

1. 下载下方 `沐目_0.1.0_x64-setup.exe`（2.85 MB）
2. 双击运行安装包
3. 选择安装目录（默认 `%LOCALAPPDATA%\沐目`）
4. 安装完成后**自动启动**，系统托盘出现薄荷绿眼睛图标
5. 双击托盘图标打开主窗口

卸载：控制面板 → 程序与功能 → 沐目 → 卸载。卸载器会询问"是否清除所有数据"。

---

## 已知问题

- ⚠️ **托盘图标无渐变动画**：状态切换是瞬间的。Tauri tray API 不直接支持过渡动画；32×32 系统托盘上视觉影响极小
- ⚠️ **倒计时数字无平滑过渡**：每秒切换有轻微"跳动感"
- ⚠️ **锁屏检测使用简化信号**：当前用"前台窗口消失 + idle > 5s"代理锁屏事件，**真正的锁屏 API（OpenInputDesktop）已列入 v0.2**
- ⚠️ **未在 3 台真机跑 24h**：性能数据来自 1 台本机快测，达标但完整 long-run 验证待 v0.2

完整已知问题见 `openspec/changes/add-mumu-eye-care/T19-visual.md` 的"后续行动"段。

---

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10 1909+ / Windows 11 |
| 架构 | x64 |
| WebView2 Runtime | Windows 11 预装；Windows 10 需手动装一次（[官方下载](https://developer.microsoft.com/microsoft-edge/webview2/)） |
| 磁盘 | 至少 50 MB 可用空间（应用 + 统计数据） |
| 内存 | 至少 50 MB 可用（应用 + WebView2 子进程） |

---

## 校验

下载后请校验安装包完整性：

```powershell
# Windows PowerShell
Get-FileHash "沐目_0.1.0_x64-setup.exe" -Algorithm SHA256
```

预期 SHA256：
```
a588c0e3a3493baae9ab67a925c0f521322d9b21686ffc6df959241fd1e6eeb0
```

如果哈希不一致，说明下载损坏，请重新下载或检查网络。

---

## 隐私

沐目是**纯本地工具**，不联网、不上传数据、不调用任何第三方分析服务。
完整隐私政策见 [PRIVACY.md](https://github.com/yourname/mumu/blob/main/PRIVACY.md)。

---

## 反馈

- 🐛 **Bug 报告**：[GitHub Issues](https://github.com/yourname/mumu/issues/new?template=bug.md)
- 💡 **功能建议**：[GitHub Issues](https://github.com/yourname/mumu/issues/new?template=feature.md)
- 💬 **讨论**：[GitHub Discussions](https://github.com/yourname/mumu/discussions)

---

## 致谢

本软件基于以下开源项目：

- [Tauri](https://tauri.app/) — 桌面应用框架
- [React](https://react.dev/) — UI 库
- [Rust](https://www.rust-lang.org/) — 系统编程语言
- [SQLite](https://www.sqlite.org/) — 嵌入式数据库
- [Radix UI](https://www.radix-ui.com/) — 无样式可访问组件
- [Tailwind CSS](https://tailwindcss.com/) — CSS 工具集

特别感谢所有参与 alpha 测试的朋友。

---

**完整 changelog 与开发过程**：[openspec/changes/add-mumu-eye-care/](https://github.com/yourname/mumu/tree/main/openspec/changes/add-mumu-eye-care/)