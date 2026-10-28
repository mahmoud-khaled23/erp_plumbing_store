; No download, bootstrapper or automatic runtime update is permitted here.
!include WinVer.nsh
!include LogicLib.nsh
!include WordFunc.nsh

!macro ERP_REQUIRE_CLOSED
  ; Use the executable's file lock, not its changing Arabic window title.
  Push $0
  Push $1
  Push $2
  Push $3
  Push $R8
  Push $R9
  StrCpy $R9 0
  ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro RestartManager_StartSession $R8
    ${If} $R8 != ""
      !insertmacro RestartManager_RegisterFile $R8 "$INSTDIR\${MAINBINARYNAME}.exe"
      ${If} $0 = 0
        ; Query only: zero array capacity, no shutdown or restart is ever requested.
        System::Call 'RSTRTMGR::RmGetList(i R8, *i0.r1, *i0.r2, p0, *i0.r3) i.r0'
      ${EndIf}
      StrCpy $R9 $0
      !insertmacro RestartManager_EndSession $R8
    ${Else}
      StrCpy $R9 1
    ${EndIf}
  ${EndIf}
  ${If} $R9 != 0
    MessageBox MB_OK|MB_ICONEXCLAMATION "$(erpCloseFirst)" /SD IDOK
    SetErrorLevel 1618
    Quit
  ${EndIf}
  Pop $R9
  Pop $R8
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_PREINSTALL
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_OK|MB_ICONSTOP "$(erpWindowsRequired)" /SD IDOK
    SetErrorLevel 1633
    Quit
  ${EndIf}
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "$(erpWindowsRequired)" /SD IDOK
    SetErrorLevel 1633
    Quit
  ${EndIf}
  !insertmacro ERP_REQUIRE_CLOSED
  Push $0
  Push $1
  Push $2
  StrCpy $2 0
  ; Microsoft documents HKLM in the 32-bit view and HKCU in the native view.
  SetRegView 32
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${VersionCompare} "$0" "109.0.0.0" $1
  ${If} $0 != ""
  ${AndIf} $1 != 2
    StrCpy $2 1
  ${EndIf}
  SetRegView 64
  ReadRegStr $0 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${VersionCompare} "$0" "109.0.0.0" $1
  ${If} $0 != ""
  ${AndIf} $1 != 2
    StrCpy $2 1
  ${EndIf}
  SetRegView 32
  ReadRegStr $0 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${VersionCompare} "$0" "109.0.0.0" $1
  ${If} $0 != ""
  ${AndIf} $1 != 2
    StrCpy $2 1
  ${EndIf}
  SetRegView 64
  ${If} $2 != 1
    MessageBox MB_OK|MB_ICONSTOP "$(erpRuntimeMissing)" /SD IDOK
    SetErrorLevel 1603
    Quit
  ${EndIf}
  Pop $2
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro ERP_REQUIRE_CLOSED
!macroend
