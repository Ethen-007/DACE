!include "MUI2.nsh"

; Basic settings
Name "Dace Editor"
OutFile "DaceSetup.exe"
InstallDir "$PROGRAMFILES64\Dace"
RequestExecutionLevel admin

; Modern UI settings
!define MUI_ICON "crates\zed\resources\windows\app-icon.ico"
!define MUI_UNICON "crates\zed\resources\windows\app-icon.ico"
!define MUI_WELCOMEPAGE_TITLE "Welcome to Dace Editor Setup"
!define MUI_WELCOMEPAGE_TEXT "This will install Dace Editor on your computer."

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Language
!insertmacro MUI_LANGUAGE "English"

Section "Install"
    SetOutPath "$INSTDIR"
    
    ; Copy main executable and dependencies
    File "dist\bin\zed.exe"
    Rename "$INSTDIR\zed.exe" "$INSTDIR\dace.exe"
    
    ; Create shortcuts
    CreateDirectory "$SMPROGRAMS\Dace"
    CreateShortCut "$SMPROGRAMS\Dace\Dace.lnk" "$INSTDIR\dace.exe"
    CreateShortCut "$DESKTOP\Dace.lnk" "$INSTDIR\dace.exe"
    
    ; Write uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"
    
    ; Write registry keys for uninstaller
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dace" "DisplayName" "Dace Editor"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dace" "UninstallString" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
    ; Remove files
    Delete "$INSTDIR\dace.exe"
    Delete "$INSTDIR\uninstall.exe"
    
    ; Remove shortcuts
    Delete "$SMPROGRAMS\Dace\Dace.lnk"
    Delete "$DESKTOP\Dace.lnk"
    RMDir "$SMPROGRAMS\Dace"
    
    ; Remove installation directory
    RMDir "$INSTDIR"
    
    ; Remove registry keys
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dace"
SectionEnd