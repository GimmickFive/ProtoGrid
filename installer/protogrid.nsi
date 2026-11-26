Unicode true
!include "MUI2.nsh"
!include "x64.nsh"

!define APP_NAME "ProtoGrid"
!define APP_VERSION "0.2.0"
!define APP_EXE "ProtoGrid.exe"
!define CORE_EXE "protogrid-core.exe"
!define APP_PUBLISHER "ProtoGrid"
!define APP_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ProtoGrid"
!define INSTALL_DIR "$LOCALAPPDATA\Programs\ProtoGrid"

Name "${APP_NAME} ${APP_VERSION}"
OutFile "..\dist\ProtoGrid-${APP_VERSION}-windows-x64-setup.exe"
InstallDir "${INSTALL_DIR}"
InstallDirRegKey HKCU "Software\ProtoGrid" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

VIProductVersion "0.2.0.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "FileDescription" "${APP_NAME} setup"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "LegalCopyright" "MIT"

!define MUI_ICON "..\apps\protogrid-desktop\assets\icon.ico"
!define MUI_UNICON "..\apps\protogrid-desktop\assets\icon.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "..\installer\welcome.bmp"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "..\installer\header.bmp"
!define MUI_HEADERIMAGE_RIGHT
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Запустить ${APP_NAME}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Russian"

Section "ProtoGrid" SEC_APP
    SectionIn RO
    SetOutPath "$INSTDIR"
    File /oname=${APP_EXE} "..\target\x86_64-pc-windows-gnu\release\protogrid.exe"
    ; The core and wintun.dll are installed next to the launcher instead of
    ; being embedded into it, so the launcher stays small.
    File /oname=${CORE_EXE} "..\dist\fork\sing-box-protogrid-windows-amd64.exe"
    File /oname=wintun.dll "..\dist\fork\wintun.dll"
    File /oname=LICENSE.txt "..\LICENSE"
    File /oname=THIRD_PARTY_NOTICES.txt "..\THIRD_PARTY_NOTICES.md"
    SetOutPath "$INSTDIR\licenses"
    File "..\installer\licenses\GPL-3.0.txt"
    File "..\installer\licenses\wintun-GPL-2.0.txt"
    SetOutPath "$INSTDIR"
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Create the profile with the default config before the first launch.
    ExecWait '"$INSTDIR\${APP_EXE}" --init' $0
    DetailPrint "--init exit code: $0"
    IntCmp $0 0 init_ok
        MessageBox MB_ICONEXCLAMATION "Не удалось подготовить профиль (код $0). Запустите ProtoGrid один раз вручную."
    init_ok:

    WriteRegStr HKCU "Software\ProtoGrid" "InstallDir" "$INSTDIR"
    WriteRegStr HKCU "${APP_KEY}" "DisplayName" "${APP_NAME}"
    WriteRegStr HKCU "${APP_KEY}" "DisplayVersion" "${APP_VERSION}"
    WriteRegStr HKCU "${APP_KEY}" "Publisher" "${APP_PUBLISHER}"
    WriteRegStr HKCU "${APP_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
    WriteRegStr HKCU "${APP_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegDWORD HKCU "${APP_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${APP_KEY}" "NoRepair" 1

    CreateDirectory "$SMPROGRAMS\${APP_NAME}"
    CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
    CreateShortcut "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
SectionEnd

Section /o "Ярлык на рабочем столе" SEC_DESKTOP
    CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\${APP_EXE}"
    Delete "$INSTDIR\${CORE_EXE}"
    Delete "$INSTDIR\wintun.dll"
    Delete "$INSTDIR\LICENSE.txt"
    Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
    Delete "$INSTDIR\licenses\GPL-3.0.txt"
    Delete "$INSTDIR\licenses\wintun-GPL-2.0.txt"
    RMDir "$INSTDIR\licenses"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"

    Delete "$DESKTOP\${APP_NAME}.lnk"
    Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
    Delete "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk"
    RMDir "$SMPROGRAMS\${APP_NAME}"

    DeleteRegKey HKCU "${APP_KEY}"
    DeleteRegKey HKCU "Software\ProtoGrid"
    ; User data in %APPDATA%\app.protogrid.client is intentionally kept.
SectionEnd
