; NSIS 3.11 per-user installer shared by JoyWord, JoyDeck and JoySheet.
; Reference: https://nsis.sourceforge.io/Docs/Chapter4.html
Unicode true
!include "config.nsh"
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
Name "${DISPLAY_NAME}"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\${TITLE}"
RequestExecutionLevel user
SetCompressor zlib
Icon "${ICON}"
UninstallIcon "${ICON}"
VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "${TITLE}"
VIAddVersionKey /LANG=1033 "FileDescription" "${DISPLAY_NAME} Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "JoyOffice contributors"
Var TestMode
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PAYLOAD}\${LICENSE_FILE}"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"
Function .onInit
  SetShellVarContext current
  !if "${PAYLOAD_ARCH}" == "aarch64"
    ${IfNot} ${IsNativeARM64}
      MessageBox MB_OK|MB_ICONSTOP "This installer requires Windows ARM64." /SD IDOK
      SetErrorLevel 1633
      Quit
    ${EndIf}
  !else
    ${IfNot} ${RunningX64}
      MessageBox MB_OK|MB_ICONSTOP "This installer requires 64-bit Windows." /SD IDOK
      SetErrorLevel 1633
      Quit
    ${EndIf}
  !endif
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/TEST" $R1
  IfErrors +2 0
    StrCpy $TestMode "1"
FunctionEnd
Section "${TITLE} desktop and CLI" Main
  SectionIn RO
  SetOutPath "$INSTDIR"
  ClearErrors
  File /r "${PAYLOAD}\*.*"
  ${If} ${Errors}
    SetErrorLevel 1
    Abort "Unable to write the application files. Close the app and retry."
  ${EndIf}
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  ${If} $TestMode == "1"
    FileOpen $R0 "$INSTDIR\.${PRODUCT}-install-smoke" w
    FileClose $R0
  ${Else}
    CreateDirectory "$SMPROGRAMS\${TITLE}"
    CreateShortcut "$SMPROGRAMS\${TITLE}\${TITLE}.lnk" "$INSTDIR\${PRODUCT}.exe"
    CreateShortcut "$SMPROGRAMS\${TITLE}\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}" "DisplayName" "${DISPLAY_NAME}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}" "DisplayIcon" "$INSTDIR\${PRODUCT}.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}" "NoRepair" 1
  ${EndIf}
SectionEnd
Section /o "Add Open with (preserves your default app)" Associations
  ${If} $TestMode != "1"
    WriteRegStr HKCU "Software\Classes\${PROGID}" "" "${TITLE} ${DESCRIPTION}"
    WriteRegStr HKCU "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\${PRODUCT}.exe,0"
    WriteRegStr HKCU "Software\Classes\${PROGID}\shell\open\command" "" '$\"$INSTDIR\${PRODUCT}.exe$\" $\"%1$\"'
    !include "associations.nsh"
  ${EndIf}
SectionEnd
Section "Uninstall"
  SetShellVarContext current
  IfFileExists "$INSTDIR\.${PRODUCT}-install-smoke" TestCleanup
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}" "InstallLocation"
  ${If} $R0 == "$INSTDIR"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${TITLE}"
    Delete "$SMPROGRAMS\${TITLE}\${TITLE}.lnk"
    Delete "$SMPROGRAMS\${TITLE}\Uninstall.lnk"
    RMDir "$SMPROGRAMS\${TITLE}"
  ${EndIf}
  ReadRegStr $R0 HKCU "Software\Classes\${PROGID}\DefaultIcon" ""
  ${If} $R0 == "$INSTDIR\${PRODUCT}.exe,0"
    DeleteRegKey HKCU "Software\Classes\${PROGID}"
    !include "uninstall-associations.nsh"
  ${EndIf}
  TestCleanup:
  !include "uninstall-files.nsh"
  Delete "$INSTDIR\.${PRODUCT}-install-smoke"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
