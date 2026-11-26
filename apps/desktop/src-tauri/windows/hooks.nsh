; Tessera installer hooks (included by Tauri's NSIS installer).
;
; Adds "Scan with Tessera" to File Explorer's right-click menu for folders,
; for the empty space inside an open folder, and for drives. On Windows 11
; it appears under "Show more options". Per-user keys only (no admin), and
; all of them are removed again by the uninstaller.
;
; The folder arrives as the first argument; Tessera scans it (or, if Tessera
; is already open, the open window scans it). See src/launch.rs.

!macro TESSERA_ADD_VERB KEY
  WriteRegStr HKCU "Software\Classes\${KEY}\shell\Tessera" "" "Scan with Tessera"
  WriteRegStr HKCU "Software\Classes\${KEY}\shell\Tessera" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe",0'
  WriteRegStr HKCU "Software\Classes\${KEY}\shell\Tessera\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%V"'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro TESSERA_ADD_VERB "Directory"
  !insertmacro TESSERA_ADD_VERB "Directory\Background"
  !insertmacro TESSERA_ADD_VERB "Drive"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\Directory\shell\Tessera"
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\Tessera"
  DeleteRegKey HKCU "Software\Classes\Drive\shell\Tessera"
  ; Settings > Background > "Start at sign-in" (tauri-plugin-autostart) adds a
  ; Run entry named Tessera. Remove it on a real uninstall, not during an update.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Tessera"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Tessera"
  ${EndIf}
!macroend
