; Koalino's portable Windows .exe (NSIS 3, #177): one file that runs Koalino without installing it.
; build-installer.sh compiles it from the same release folder (koalino-windows-ARCH-VERSION/) as
; the installer, so both carry exactly the files of the release.
;
; On first start it unpacks Koalino into "Koalino VERSION ARCH" beside itself (so it runs from a
; USB stick too), showing its progress, then starts Koalino. When that folder cannot be written
; (a read-only drive or a protected folder) it unpacks into %LOCALAPPDATA%\Koalino Portable
; instead. Later starts find the unpacked copy and start Koalino at once. A file marks a complete
; unpacking, so an interrupted one is redone.
;
; It installs nothing: no Start menu entry, no uninstaller, no registry entry; deleting the .exe and
; its folder removes it. Meetings are kept where the installed app keeps them
; (%APPDATA%\capannone-dev\Koalino), so both see the same meetings.
;
; Defines expected from the command line: VERSION (e.g. 0.2.0), ARCH (x86_64 or arm64), SOURCE_DIR
; (the release folder), OUTPUT (the .exe's path), ICON (app_icon.ico).

Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

!define APP_NAME "Koalino"
!define APP_EXE "note_taker_ui.exe"
!define PUBLISHER "capannone-dev"
!include "architecture-check.nsh"
; Written last: an unpacked copy without it is incomplete and is unpacked again.
!define COMPLETE_MARKER ".koalino-portable-complete"
!define FOLDER_NAME "Koalino ${VERSION} ${ARCH}"

Name "${APP_NAME} ${VERSION}"
Caption "${APP_NAME} ${VERSION}"
OutFile "${OUTPUT}"
BrandingText "${APP_NAME} ${VERSION} (portable)"
ShowInstDetails nevershow
AutoCloseWindow true

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${APP_NAME} (portable)"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "the Koalino authors. GPL-3.0-or-later"

!define MUI_ICON "${ICON}"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "header.bmp"
!define MUI_INSTFILESPAGE_FINISHHEADER_TEXT "Starting ${APP_NAME}"
!define MUI_PAGE_HEADER_TEXT "Preparing ${APP_NAME}"
!define MUI_PAGE_HEADER_SUBTEXT "Unpacking ${APP_NAME} beside this file. This happens only the first time."
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; Whether $0 is a folder this program can write into: creates it, then writes and deletes a file.
; Output: $1 is 1 when it can, 0 otherwise.
Function CanWrite
  StrCpy $1 0
  ClearErrors
  CreateDirectory "$0"
  IfErrors done
  FileOpen $2 "$0\.koalino-write-test" w
  IfErrors done
  FileClose $2
  Delete "$0\.koalino-write-test"
  StrCpy $1 1
  done:
FunctionEnd

; Starts the unpacked Koalino from $INSTDIR and quits.
Function StartKoalino
  SetOutPath "$INSTDIR"
  Exec '"$INSTDIR\${APP_EXE}"'
  Quit
FunctionEnd

Function .onInit
  !insertmacro CHECK_ARCHITECTURE "portable .exe"
  ; Beside this file, or in the user's local app data when that is not writable.
  StrCpy $0 "$EXEDIR\${FOLDER_NAME}"
  ${If} ${FileExists} "$0\${COMPLETE_MARKER}"
    StrCpy $INSTDIR "$0"
    Call StartKoalino
  ${EndIf}
  StrCpy $0 "$LOCALAPPDATA\Koalino Portable\${VERSION} ${ARCH}"
  ${If} ${FileExists} "$0\${COMPLETE_MARKER}"
    StrCpy $INSTDIR "$0"
    Call StartKoalino
  ${EndIf}
  ; Not unpacked yet: choose where.
  StrCpy $0 "$EXEDIR\${FOLDER_NAME}"
  Call CanWrite
  ${If} $1 == 0
    StrCpy $0 "$LOCALAPPDATA\Koalino Portable\${VERSION} ${ARCH}"
    Call CanWrite
    ${If} $1 == 0
      MessageBox MB_OK|MB_ICONSTOP "${APP_NAME} cannot unpack itself: neither $EXEDIR nor $LOCALAPPDATA can be written." /SD IDOK
      Quit
    ${EndIf}
  ${EndIf}
  StrCpy $INSTDIR "$0"
FunctionEnd

Section "Unpack"
  SetOutPath "$INSTDIR"
  File /r "${SOURCE_DIR}\*.*"
  FileOpen $2 "$INSTDIR\${COMPLETE_MARKER}" w
  FileWrite $2 "${APP_NAME} ${VERSION} ${ARCH}"
  FileClose $2
SectionEnd

Function .onInstSuccess
  Call StartKoalino
FunctionEnd
