Unicode True
SetCompressor /SOLID lzma
!include "LogicLib.nsh"
!include "StrFunc.nsh"
${StrStr}

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

UninstPage uninstConfirm
UninstPage instfiles

Section "Install"
  SetOutPath "$INSTDIR"

  nsExec::ExecToLog 'taskkill /F /IM poratake-gpui.exe /T'
  nsExec::ExecToLog 'taskkill /F /IM poratake-daemon.exe /T'
  nsExec::ExecToLog 'taskkill /F /IM Poratake.exe /T'
  Sleep 400

  File /r "${STAGE_DIR}\*.*"

  ${If} ${FileExists} "$INSTDIR\poratake-gpui.exe"
  ${AndIf} ${FileExists} "$INSTDIR\Poratake.exe"
    Delete "$INSTDIR\Poratake.exe"
    Delete "$INSTDIR\*.dll"
    Delete "$INSTDIR\*.pak"
    RMDir /r "$INSTDIR\locales"
    RMDir /r "$INSTDIR\resources"
    Delete "$SMPROGRAMS\Poratake.lnk"
  ${EndIf}

  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\e53962ac-3318-5756-a837-be6d81ff8e94" "InstallLocation"
  ${StrStr} $1 "$INSTDIR" "$0"
  ${If} $0 != ""
  ${AndIf} $0 != "$INSTDIR"
  ${AndIf} $1 != "$INSTDIR"
  ${AndIf} ${FileExists} "$0\Poratake.exe"
    RMDir /r "$0"
    Delete "$SMPROGRAMS\Poratake.lnk"
  ${EndIf}

  WriteUninstaller "$INSTDIR\Uninstall Poratake.exe"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\e53962ac-3318-5756-a837-be6d81ff8e94"

  CreateDirectory "$SMPROGRAMS\Poratake"
  CreateShortcut "$SMPROGRAMS\Poratake\Poratake.lnk" "$INSTDIR\poratake-gpui.exe" "" "$INSTDIR\poratake.ico" 0

  Delete "$DESKTOP\Poratake.lnk"
  CreateShortcut "$DESKTOP\Poratake.lnk" "$INSTDIR\poratake-gpui.exe" "" "$INSTDIR\poratake.ico" 0

  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr HKCU "Software\Classes\AppUserModelId\electron.app.Poratake" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "Software\Classes\AppUserModelId\electron.app.Poratake" "IconUri" "$INSTDIR\poratake.ico"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\poratake.ico"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall Poratake.exe"'
  WriteRegDWORD HKCU "${PRODUCT_UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${PRODUCT_UNINST_KEY}" "NoRepair" 1

  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "electron.app.Poratake"
  ${StrStr} $1 $0 "Poratake.exe"
  ${If} $1 != ""
    ${StrStr} $2 $0 " --"
    ${If} $2 != ""
      WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "electron.app.Poratake" '"$INSTDIR\poratake-gpui.exe"$2'
    ${Else}
      WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "electron.app.Poratake" '"$INSTDIR\poratake-gpui.exe"'
    ${EndIf}
  ${EndIf}

  WriteRegStr HKCU "Software\Classes\.poratake" "" "Poratake.Recording"
  WriteRegStr HKCU "Software\Classes\Poratake.Recording" "" "Poratake Recording"
  WriteRegStr HKCU "Software\Classes\Poratake.Recording\DefaultIcon" "" "$INSTDIR\poratake-gpui.exe,0"
  WriteRegStr HKCU "Software\Classes\Poratake.Recording\shell\open\command" "" '"$INSTDIR\poratake-gpui.exe" "%1"'

  ${If} ${Silent}
    Exec '"$INSTDIR\poratake-gpui.exe"'
  ${EndIf}
SectionEnd

Section "Uninstall"
  nsExec::ExecToLog 'taskkill /F /IM poratake-gpui.exe /T'
  nsExec::ExecToLog 'taskkill /F /IM poratake-daemon.exe /T'
  Sleep 400

  Delete "$SMPROGRAMS\Poratake\Poratake.lnk"
  RMDir "$SMPROGRAMS\Poratake"
  Delete "$DESKTOP\Poratake.lnk"

  RMDir /r "$INSTDIR\daemon"
  RMDir /r "$INSTDIR\binaries"
  RMDir /r "$INSTDIR\licenses"
  RMDir /r "$INSTDIR\sounds"
  RMDir /r "$INSTDIR\locales"
  RMDir /r "$INSTDIR\resources"
  Delete "$INSTDIR\*.dll"
  Delete "$INSTDIR\*.pak"
  Delete "$INSTDIR\poratake-gpui.exe"
  Delete "$INSTDIR\Poratake.exe"
  Delete "$INSTDIR\Uninstall Poratake.exe"
  Delete "$INSTDIR\poratake.ico"
  RMDir "$INSTDIR"

  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "electron.app.Poratake"
  DeleteRegKey HKCU "${PRODUCT_UNINST_KEY}"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\electron.app.Poratake"
  DeleteRegKey HKCU "Software\Classes\.poratake"
  DeleteRegKey HKCU "Software\Classes\Poratake.Recording"
SectionEnd
