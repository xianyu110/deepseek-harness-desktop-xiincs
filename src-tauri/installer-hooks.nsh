; Windows Explorer "打开方式" (open-with) shell integration for DeepSeek
; Harness — right-click a folder, or the empty space inside one, and launch
; the app with that folder as its first argument. Consumed by
; requested_workspace() in src-tauri/src/lib.rs, which sets DSH_DESKTOP_CWD
; from it before the server starts.
;
; Written to HKCU\Software\Classes rather than HKLM-backed HKCR: this
; installer runs in Tauri's default per-user NSIS mode (installMode is
; unset in tauri.conf.json, so no admin elevation is requested), and HKLM
; keys aren't reliably writable without it. HKCU\Software\Classes is the
; correct no-elevation per-user equivalent — Windows merges it into the
; effective HKCR view Explorer reads context menus from.

!macro NSIS_HOOK_POSTINSTALL
  ; Right-click *inside* a folder (empty space) — opens that folder.
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\DeepSeekHarness" "" "用 DeepSeek Harness 打开"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\DeepSeekHarness" "Icon" "$INSTDIR\dsh-desktop.exe"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\DeepSeekHarness\command" "" '"$INSTDIR\dsh-desktop.exe" "%V"'

  ; Right-click *on* a folder itself (in its parent's listing).
  WriteRegStr HKCU "Software\Classes\Directory\shell\DeepSeekHarness" "" "用 DeepSeek Harness 打开"
  WriteRegStr HKCU "Software\Classes\Directory\shell\DeepSeekHarness" "Icon" "$INSTDIR\dsh-desktop.exe"
  WriteRegStr HKCU "Software\Classes\Directory\shell\DeepSeekHarness\command" "" '"$INSTDIR\dsh-desktop.exe" "%1"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\DeepSeekHarness"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\DeepSeekHarness"
!macroend
