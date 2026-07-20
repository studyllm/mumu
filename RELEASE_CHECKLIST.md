# T22 发布流程清单

> 状态：当前环境**无 git 仓库** + **无 gh CLI**（未认证），所以本任务**无法全自动**完成。
> 本文件是一份**手动** checklist，让你在 30 分钟内完成 GitHub Release 发布。

## 前置条件

- [ ] GitHub 账号 + 已在本地配置好 `git config user.name` / `user.email`
- [ ] 已在 GitHub 创建一个空仓库 `mumu`（public）
- [ ] 本机已装 `gh` CLI（可选，文档里给了两种方法）

## 步骤 1：初始化 git 仓库

```powershell
cd "D:\wxw-workspace\project\study\EyeProtectionTool"
git init
git add .
git status  # 检查是否包含敏感文件（应该没有：.gitignore 已覆盖 target/node_modules/.claude/）
```

**必看**：检查 `git status` 输出，确认**没有**以下文件：
- `mumu/src-tauri/target/` ← 应该被 `.gitignore` 忽略
- `mumu/node_modules/` ← 应该被忽略
- `mumu/dist/` ← 应该被忽略
- `.claude/` ← 应该被忽略

如果出现这些目录，`git rm --cached -r <path>` 移除追踪。

## 步骤 2：首次提交

```powershell
git commit -m "沐目 v0.1.0 首发

- T01-T19: 完整功能实现（强提醒 / 弱提示 / 统计 / 托盘 / 设置 / 自启 / 卸载清理 / 性能 / 视觉规范）
- T20: 用户文档
- T21: 隐私政策 (MIT) + LICENSE
- T22: 发布 v0.1.0"
```

## 步骤 3：推送到 GitHub

```powershell
git remote add origin https://github.com/yourname/mumu.git
git branch -M main
git push -u origin main
```

把 `yourname/mumu` 替换成你的实际仓库地址。

## 步骤 4：上传安装包作为 Release Asset

### 方法 A：用 gh CLI（推荐）

```powershell
gh release create v0.1.0 `
  --title "沐目 v0.1.0" `
  --notes-file mumu/RELEASE_NOTES.md `
  "mumu/src-tauri/target/release/bundle/nsis/沐目_0.1.0_x64-setup.exe"
```

如果 `gh` 未认证，先跑 `gh auth login`。

### 方法 B：手动上传（无 gh CLI）

1. 浏览器打开 `https://github.com/yourname/mumu/releases/new`
2. 填写：
   - **Tag version**: `v0.1.0`
   - **Release title**: `沐目 v0.1.0`
   - **Description**: 复制粘贴 `mumu/RELEASE_NOTES.md` 全部内容
3. 拖拽 `mumu/src-tauri/target/release/bundle/nsis/沐目_0.1.0_x64-setup.exe` 到 "Attach binaries"
4. 勾选 "Set as the latest release"
5. 点 "Publish release"

## 步骤 5：验证

- [ ] 访问 `https://github.com/yourname/mumu/releases/tag/v0.1.0`
- [ ] 看到安装包可下载
- [ ] 在另一台干净 Windows 11 机器上下载安装，确认能正常运行
- [ ] （可选）在 Release 评论里贴上"完整 changelog 链接"

## 步骤 6：归档 OpenSpec change

发布成功后，把开发规范归档：

```powershell
cd "D:\wxw-workspace\project\study\EyeProtectionTool"
openspec archive add-mumu-eye-care --yes
```

## 步骤 7（如有官网）

如果有项目官网：
- [ ] 在首页横幅贴 "🎉 v0.1.0 发布" 链接到 GitHub Release
- [ ] 在下载页指向 GitHub Releases

## 故障排除

| 问题 | 解决 |
|------|------|
| `git push` 失败：仓库不存在 | 先在 GitHub 网页创建空仓库（不要勾选 README） |
| `gh release create` 失败：未认证 | 跑 `gh auth login`，按提示粘贴 token |
| 安装包哈希不一致 | 重新 `npm run tauri build` 一次，更新 RELEASE_NOTES.md 的 SHA256 |
| 推送超过 100 MB 限制 | 不应发生 — `.gitignore` 已忽略 target/。检查是否漏了 .gitignore |
| 用户反馈"装不上" | 检查是否装了 WebView2 Runtime（Win10 需要手动装） |

## 发布后下一步

- [ ] 在 GitHub 仓库设置里添加 description + website + topics（tauri, rust, eye-care, dry-eye）
- [ ] 在 README 顶部贴上 `[![Latest Release](https://img.shields.io/github/v/release/yourname/mumu)](https://github.com/yourname/mumu/releases/latest)` badge
- [ ] 监控 GitHub Issues，回复用户反馈
- [ ] 24h 后看 Release 下载数

---

**预计总耗时**：30 分钟（含 push 大文件等待）
**总产物大小**：2.9 MB（单安装包）