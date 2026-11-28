; Glow Reading (微光阅读) Windows Installer Script (NSIS 3+)
; Per-user installation — preserves user books, highlights, and SQLite database on upgrade/uninstall.

Unicode True
!include "MUI2.nsh"

!ifndef APP_VERSION
  !define APP_VERSION "0.2.0"
!endif
!ifndef STAGE_DIR
  !define STAGE_DIR "..\dist\windows-x64"
!endif
!ifndef OUT_FILE
  !define OUT_FILE "..\dist\GlowReading-${APP_VERSION}-windows-x64-setup.exe"
!endif

Name "微光阅读 Glow Reading ${APP_VERSION}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\GlowReading"
InstallDirRegKey HKCU "Software\GlowReading" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ABORTWARNING
!define MUI_ICON "..\assets\glow.ico"
!define MUI_UNICON "..\assets\glow.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\glow-reading.exe"
!define MUI_FINISHPAGE_RUN_TEXT "立即启动微光阅读"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Glow Reading (核心程序)" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"

  ; Check Microsoft Edge WebView2 Runtime presence (non-blocking friendly warning)
  ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  StrCmp $0 "" check_hkcu webview2_ok
check_hkcu:
  ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  StrCmp $0 "" 0 webview2_ok
  MessageBox MB_ICONINFORMATION|MB_OK "未检测到 Microsoft Edge WebView2 Runtime。安装将继续，首次打开书页前请前往 Microsoft 官网安装 WebView2 运行时。"
webview2_ok:

  File /r "${STAGE_DIR}\*.*"

  WriteRegStr HKCU "Software\GlowReading" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\GlowReading" "Version" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlowReading" "DisplayName" "微光阅读 (Glow Reading)"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlowReading" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlowReading" "Publisher" "Glow Reading Studio"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlowReading" "DisplayIcon" "$INSTDIR\assets\glow.ico"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlowReading" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlowReading" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlowReading" "NoRepair" 1

  CreateDirectory "$SMPROGRAMS\Glow Reading"
  CreateShortcut "$SMPROGRAMS\Glow Reading\微光阅读.lnk" "$INSTDIR\glow-reading.exe" "" "$INSTDIR\assets\glow.ico"
  CreateShortcut "$SMPROGRAMS\Glow Reading\卸载微光阅读.lnk" "$INSTDIR\Uninstall.exe"
  CreateShortcut "$DESKTOP\微光阅读.lnk" "$INSTDIR\glow-reading.exe" "" "$INSTDIR\assets\glow.ico"

  WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  ; Remove program files only — NEVER delete user library/database under %LOCALAPPDATA%\GlowReading
  Delete "$DESKTOP\微光阅读.lnk"
  RMDir /r "$SMPROGRAMS\Glow Reading"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlowReading"
  DeleteRegKey HKCU "Software\GlowReading"
SectionEnd
