; Custom NSIS installer hooks (Tauri v2, bundle.windows.nsis.installerHooks).
;
; Adds a branded "Open in Recast" right-click verb for .recast files. Tauri's
; fileAssociations already registers the default "Open" handler (double-click);
; this is the explicitly-labelled context-menu entry.
;
; Written under SystemFileAssociations\.recast so it is ADDITIVE — it shows
; regardless of which app owns the default association, and does not disturb it.
; SHCTX follows the installer's mode: HKLM for per-machine, HKCU for per-user.
; The launched exe receives the file path as %1; the app's argv/file-association
; path (parse_open_arg + single-instance) opens it in a fresh editor window.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\SystemFileAssociations\.recast\shell\OpenInRecast" "" "Open in Recast"
  WriteRegStr SHCTX "Software\Classes\SystemFileAssociations\.recast\shell\OpenInRecast" "Icon" "$INSTDIR\recast.exe,0"
  WriteRegStr SHCTX "Software\Classes\SystemFileAssociations\.recast\shell\OpenInRecast\command" "" '"$INSTDIR\recast.exe" "%1"'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey SHCTX "Software\Classes\SystemFileAssociations\.recast\shell\OpenInRecast"
!macroend
