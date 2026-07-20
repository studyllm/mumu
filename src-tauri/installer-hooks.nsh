; 沐目 - NSIS 安装/卸载钩子
;
; 由 tauri.conf.json 的 bundle.windows.nsis.installerHooks 引用
; 文档：https://nsis.sourceforge.io/Docs/Chapter4.html

; 卸载前提示：告知用户将清除数据（T14 spec 要求）
;
; 用 MessageBox MB_YESNO|MB_ICONEXCLAMATION：
; - Yes → 继续卸载
; - No  → Abort（NSIS 标准 abort 流程，退出码告诉卸载器取消）
;
; $APPDATA 在 NSIS 中指向 %APPDATA% (HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders\AppData)
; currentUser 模式下标准 AppData 路径就是它
!macro NSIS_HOOK_PREUNINSTALL
  MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_TOPMOST "卸载沐目将清除所有设置与统计数据（%APPDATA%\沐目），是否继续？" IDYES +2 IDNO 0
  Abort
!macroend

; 卸载后清理
; 范围：仅卸载 productName 匹配的应用数据，不影响其他程序
!macro NSIS_HOOK_POSTUNINSTALL
  ; 删除整个 沐目 目录（settings.json + stats.db 等用户数据）
  ; $APPDATA 在 NSIS 里会被展开成用户实际路径
  RMDir /r $APPDATA\沐目

  ; 删除开机自启注册表项（T13 衔接）
  ; key 名由 tauri-plugin-autostart 默认使用 exe 文件名，即 mumu
  ; 只删精确条目，不影响同机器上其他使用 HKCU\...\Run 的软件
  ; HKCU 在 NSIS 是 HKCU 标准简写，等价 HKEY_CURRENT_USER
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "mumu"
!macroend
