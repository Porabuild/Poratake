Unicode True
SetCompressor /SOLID lzma

!define PRODUCT_NAME "Poratake"
!define PRODUCT_PUBLISHER "Porabuild"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\com.porabuild.poratake"

Name "${PRODUCT_NAME} ${VERSION}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\Poratake"
InstallDirRegKey HKCU "${PRODUCT_UNINST_KEY}" "InstallLocation"
RequestExecutionLevel user
ShowInstDetails show

!ifdef ICON_PATH
Icon "${ICON_PATH}"
UninstallIcon "${ICON_PATH}"
!endif

Page directory
Page instfiles

UninstPage confirm
UninstPage instfiles

Section "Install"
  SetOutPath "$INSTDIR"

  nsExec::ExecToLog 'taskkill /F /IM poratake-gpui.exe /T'
  nsExec::ExecToLog 'taskkill /F /IM poratake-daemon.exe /T'
  nsExec::ExecToLog 'taskkill /F /IM Poratake.exe /T'
  Sleep 400

  IfFileExists "$INSTDIR\Poratake.exe" 0 +6
    Delete "$INSTDIR\Poratake.exe"
    Delete "$INSTDIR\*.dll"
    Delete "$INSTDIR\*.pak"
    RMDir /r "$INSTDIR\locales"
    RMDir /r "$INSTDIR\resources"

  File /r "${STAGE_DIR}\*.*"

  WriteUninstaller "$INSTDIR\Uninstall Poratake.exe"

  CreateDirectory "$SMPROGRAMS\Poratake"
  CreateShortcut "$SMPROGRAMS\Poratake\Poratake.lnk" "$INSTDIR\poratake-gpui.exe" "" "$INSTDIR\poratake-gpui.exe" 0

  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\poratake-gpui.exe"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall Poratake.exe"'
  WriteRegDWORD HKCU "${PRODUCT_UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${PRODUCT_UNINST_KEY}" "NoRepair" 1

  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "electron.app.Poratake"
  StrCmp $0 "" +2 0
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "electron.app.Poratake" '"$INSTDIR\poratake-gpui.exe"'

  WriteRegStr HKCU "Software\Classes\.poratake" "" "Poratake.Recording"
  WriteRegStr HKCU "Software\Classes\Poratake.Recording" "" "Poratake Recording"
  WriteRegStr HKCU "Software\Classes\Poratake.Recording\DefaultIcon" "" "$INSTDIR\poratake-gpui.exe,0"
  WriteRegStr HKCU "Software\Classes\Poratake.Recording\shell\open\command" "" '"$INSTDIR\poratake-gpui.exe" "%1"'

  IfSilent 0 +2
    Exec '"$INSTDIR\poratake-gpui.exe"'
SectionEnd

Section "Uninstall"
  nsExec::ExecToLog 'taskkill /F /IM poratake-gpui.exe /T'
  nsExec::ExecToLog 'taskkill /F /IM poratake-daemon.exe /T'
  Sleep 400

  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "electron.app.Poratake"
  DeleteRegKey HKCU "${PRODUCT_UNINST_KEY}"
  DeleteRegKey HKCU "Software\Classes\.poratake"
  DeleteRegKey HKCU "Software\Classes\Poratake.Recording"
  Delete "$SMPROGRAMS\Poratake\Poratake.lnk"
  RMDir "$SMPROGRAMS\Poratake"

  RMDir /r "$INSTDIR\daemon"
  RMDir /r "$INSTDIR\binaries"
  RMDir /r "$INSTDIR\licenses"
  RMDir /r "$INSTDIR\sounds"
  Delete "$INSTDIR\poratake-gpui.exe"
  Delete "$INSTDIR\Uninstall Poratake.exe"
  RMDir "$INSTDIR"
SectionEnd
