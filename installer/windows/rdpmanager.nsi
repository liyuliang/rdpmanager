Unicode true
ManifestDPIAware true

!ifndef APP_NAME
  !define APP_NAME "RDPManager"
!endif
!ifndef APP_DISPLAY_NAME
  !define APP_DISPLAY_NAME "RDP 管理器"
!endif
!ifndef APP_VERSION
  !define APP_VERSION "0.1.0"
!endif
!ifndef VI_VERSION
  !define VI_VERSION "0.1.0.0"
!endif
!ifndef BIN_PATH
  !define BIN_PATH "..\..\target\release\rdpmanager.exe"
!endif
!ifndef OUT_FILE
  !define OUT_FILE "..\..\dist\rdpmanager-windows-amd64-installer.exe"
!endif
!ifndef ICON_PATH
  !define ICON_PATH "..\..\assets\app.ico"
!endif

!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"

VIProductVersion "${VI_VERSION}"
VIFileVersion    "${VI_VERSION}"
VIAddVersionKey "ProductName"     "${APP_DISPLAY_NAME}"
VIAddVersionKey "FileDescription" "${APP_DISPLAY_NAME} Installer"
VIAddVersionKey "ProductVersion"  "${APP_VERSION}"
VIAddVersionKey "FileVersion"     "${APP_VERSION}"

RequestExecutionLevel admin

!include "MUI2.nsh"
!include "LogicLib.nsh"

!define MUI_ICON "${ICON_PATH}"
!define MUI_UNICON "${ICON_PATH}"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

AutoCloseWindow true

Name "${APP_DISPLAY_NAME}"
OutFile "${OUT_FILE}"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallLocation"
ShowInstDetails show

Function .onInit
  SetRegView 64
  ReadRegStr $0 HKLM "${UNINST_KEY}" "InstallLocation"
  ${If} $0 == ""
    SetRegView 32
    ReadRegStr $0 HKLM "${UNINST_KEY}" "InstallLocation"
    SetRegView 64
  ${EndIf}
  ${If} $0 != ""
    StrCpy $INSTDIR $0
  ${EndIf}
FunctionEnd

Function .onInstSuccess
  Exec '"$INSTDIR\rdpmanager.exe"'
FunctionEnd

Section "MainSection" SEC01
  SetRegView 64
  SetShellVarContext all
  SetOutPath "$INSTDIR"

  File "/oname=rdpmanager.exe" "${BIN_PATH}"

  CreateShortcut "$SMPROGRAMS\${APP_DISPLAY_NAME}.lnk" "$INSTDIR\rdpmanager.exe"
  CreateShortcut "$DESKTOP\${APP_DISPLAY_NAME}.lnk" "$INSTDIR\rdpmanager.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APP_DISPLAY_NAME}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\rdpmanager.exe"
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetRegView 64
  SetShellVarContext all

  Delete "$SMPROGRAMS\${APP_DISPLAY_NAME}.lnk"
  Delete "$DESKTOP\${APP_DISPLAY_NAME}.lnk"
  Delete "$INSTDIR\rdpmanager.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
