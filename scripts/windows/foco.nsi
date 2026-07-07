Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetOverwrite on

!ifndef APP_EXE
  !error "APP_EXE is required"
!endif
!ifndef APP_RESOURCES
  !error "APP_RESOURCES is required"
!endif
!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef PRODUCT_VERSION
  !define PRODUCT_VERSION "${VERSION}.0"
!endif
!ifndef OUT_FILE
  !define OUT_FILE "dist\windows\Foco-setup.exe"
!endif

!define APP_NAME "Foco"
!define COMPANY_NAME "Foco"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Foco"

Name "${APP_NAME}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Foco"
BrandingText "${APP_NAME} ${VERSION}"

VIProductVersion "${PRODUCT_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "${APP_NAME}"
VIAddVersionKey /LANG=1033 "CompanyName" "${COMPANY_NAME}"
VIAddVersionKey /LANG=1033 "FileDescription" "${APP_NAME} installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"

Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /oname=foco.exe "${APP_EXE}"

  SetOutPath "$INSTDIR\resources"
  File /r "${APP_RESOURCES}\*"

  CreateDirectory "$SMPROGRAMS\Foco"
  CreateShortcut "$SMPROGRAMS\Foco\Foco.lnk" "$INSTDIR\foco.exe"

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "${COMPANY_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\foco.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '$"$INSTDIR\Uninstall.exe$"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '$"$INSTDIR\Uninstall.exe$" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\Foco\Foco.lnk"
  RMDir "$SMPROGRAMS\Foco"
  Delete "$INSTDIR\foco.exe"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir /r "$INSTDIR\resources"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
