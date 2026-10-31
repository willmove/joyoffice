Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"
Name "${DISPLAY_NAME}"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Joysheet"
RequestExecutionLevel user
SetCompressor zlib
Icon "${ICON}"
UninstallIcon "${ICON}"
Var TestMode
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PAYLOAD}\LICENSE-MIT"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"
Function .onInit
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/TEST" $R1
  IfErrors +2 0
    StrCpy $TestMode "1"
FunctionEnd
Section "Joysheet desktop and CLI" Main
 SectionIn RO
 SetOutPath "$INSTDIR"
 File /r "${PAYLOAD}\*.*"
 WriteUninstaller "$INSTDIR\Uninstall.exe"
 ${If} $TestMode == "1"
  FileOpen $R0 "$INSTDIR\.joysheet-install-smoke" w
  FileClose $R0
 ${Else}
  CreateDirectory "$SMPROGRAMS\Joysheet"
  CreateShortcut "$SMPROGRAMS\Joysheet\Joysheet.lnk" "$INSTDIR\joysheet.exe"
  CreateShortcut "$SMPROGRAMS\Joysheet\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet" "DisplayName" "${DISPLAY_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet" "DisplayIcon" "$INSTDIR\joysheet.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet" "NoRepair" 1
 ${EndIf}
SectionEnd
Section /o "Add Open with (preserves your default app)" Associations
 ${If} $TestMode != "1"
  WriteRegStr HKCU "Software\Classes\Joysheet.Workbook" "" "Joysheet Workbook"
  WriteRegStr HKCU "Software\Classes\Joysheet.Workbook\DefaultIcon" "" "$INSTDIR\joysheet.exe,0"
  WriteRegStr HKCU "Software\Classes\Joysheet.Workbook\shell\open\command" "" '$\"$INSTDIR\joysheet.exe$\" $\"%1$\"'
  WriteRegStr HKCU "Software\Classes\.xlsx\OpenWithProgids" "Joysheet.Workbook" ""
  WriteRegStr HKCU "Software\Classes\.csv\OpenWithProgids" "Joysheet.Workbook" ""
  WriteRegStr HKCU "Software\Classes\.ods\OpenWithProgids" "Joysheet.Workbook" ""
 ${EndIf}
SectionEnd
Section "Uninstall"
 IfFileExists "$INSTDIR\.joysheet-install-smoke" TestCleanup
 ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet" "InstallLocation"
 ${If} $R0 == "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Joysheet"
  Delete "$SMPROGRAMS\Joysheet\Joysheet.lnk"
  Delete "$SMPROGRAMS\Joysheet\Uninstall.lnk"
  RMDir "$SMPROGRAMS\Joysheet"
 ${EndIf}
 ReadRegStr $R0 HKCU "Software\Classes\Joysheet.Workbook\DefaultIcon" ""
 ${If} $R0 == "$INSTDIR\joysheet.exe,0"
  DeleteRegKey HKCU "Software\Classes\Joysheet.Workbook"
  DeleteRegValue HKCU "Software\Classes\.xlsx\OpenWithProgids" "Joysheet.Workbook"
  DeleteRegValue HKCU "Software\Classes\.csv\OpenWithProgids" "Joysheet.Workbook"
  DeleteRegValue HKCU "Software\Classes\.ods\OpenWithProgids" "Joysheet.Workbook"
 ${EndIf}
 TestCleanup:
 !include "uninstall-files.nsh"
 Delete "$INSTDIR\.joysheet-install-smoke"
 Delete "$INSTDIR\Uninstall.exe"
 RMDir "$INSTDIR"
SectionEnd

