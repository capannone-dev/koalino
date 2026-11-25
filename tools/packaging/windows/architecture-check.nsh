; Checks that this computer can run the Koalino build a package carries (#176, #177), for the
; installer (koalino.nsi) and the portable .exe (koalino-portable.nsi). Both packages are 32-bit
; x86 programs, which every Windows runs, so the check is theirs to make:
; - an ARM64 build needs Windows on ARM;
; - the x86_64 build needs 64-bit Windows. Windows on ARM can run it under emulation, but slowly,
;   so the person is asked first and pointed to the ARM64 download. Silently (/S) it goes ahead.
;
; Needs: ARCH (x86_64 or arm64) and APP_NAME defined, and x64.nsh included.
; Usage: !insertmacro CHECK_ARCHITECTURE "<what to download, e.g. installer>", in .onInit.
; Side effects: shows a message and quits the package when the computer does not fit.

!macro CHECK_ARCHITECTURE DOWNLOAD
  !if "${ARCH}" == "arm64"
    ${IfNot} ${IsNativeARM64}
      MessageBox MB_OK|MB_ICONSTOP "This is ${APP_NAME} for Windows on ARM. Download the x86_64 ${DOWNLOAD} for this computer." /SD IDOK
      Quit
    ${EndIf}
  !else
    ${IfNot} ${IsNativeAMD64}
      ${IfNot} ${IsNativeARM64}
        MessageBox MB_OK|MB_ICONSTOP "${APP_NAME} needs 64-bit Windows." /SD IDOK
        Quit
      ${EndIf}
      MessageBox MB_YESNO|MB_ICONEXCLAMATION "This is ${APP_NAME} for x64 computers. This computer runs Windows on ARM, where the ARM64 ${DOWNLOAD} transcribes much faster.$\r$\n$\r$\nUse the x64 version anyway?" /SD IDYES IDYES +2
      Quit
    ${EndIf}
  !endif
!macroend
