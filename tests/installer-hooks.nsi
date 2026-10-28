; Compiler-only fixture. This is not the application's installer and is never shipped.
Unicode true
!include MUI2.nsh
!include x64.nsh
!include "Win\RestartManager.nsh"
!define MAINBINARYNAME "plumbing-erp"
!define PRODUCTNAME "PlumbingStore"
!define VERSION "0.1.0"
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Arabic"
!include "../src-tauri/windows/Arabic.nsh"
!include "../src-tauri/windows/hooks.nsh"
Name "ERP installer hook compiler test"
OutFile "/tmp/erp-hook-check.exe"
RequestExecutionLevel user
Section
  !insertmacro NSIS_HOOK_PREINSTALL
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd
Section Uninstall
  !insertmacro NSIS_HOOK_PREUNINSTALL
SectionEnd
