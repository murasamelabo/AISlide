!macro AISLIDE_REQUIRE_CLOSED executableName productName
  Push $R0
  nsis_tauri_utils::FindProcessCurrentUser "${executableName}"
  Pop $R0
  ${If} $R0 = 0
    Pop $R0
    IfSilent +2
    MessageBox MB_OK|MB_ICONEXCLAMATION "Save your work and close ${productName} before continuing."
    SetErrorLevel 2
    Abort
  ${EndIf}
  Pop $R0
!macroend

!macroundef CheckIfAppIsRunning
!macro CheckIfAppIsRunning executableName productName
  !insertmacro AISLIDE_REQUIRE_CLOSED "${executableName}" "${productName}"
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro AISLIDE_REQUIRE_CLOSED "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro AISLIDE_REQUIRE_CLOSED "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
!macroend