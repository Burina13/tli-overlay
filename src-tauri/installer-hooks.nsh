; TLI 52c (`2026-10-20-0608-mcp.md`) — vlastný NSIS hook pre jednorazovú
; migráciu zo starej perMachine MSI (TLI 49 a staršie, do `tli-v0.20.4`) na
; nový perUser NSIS inštalátor (TLI 52).
;
; PREČO VLASTNÝ HOOK, NIE VSTAVANÁ MIGRÁCIA ŠABLÓNY (`tauri-bundler` 2.12.3):
; upstream `installer.nsi` rieši migráciu na vlastnej wizard STRÁNKE
; (`Page custom PageReinstall PageLeaveReinstall`, funkcia `PageLeaveReinstall`
; obsahuje `ExecWait` na starú `UninstallString`). Jej `Leave` funkcia sa ale
; spustí len cez skutočný UI prechod tej stránky, alebo keď je nastavený
; `$PassiveMode` (flag `/P`). Appka pri tichej aktualizácii posiela LEN `/S`
; (bez `/P` — `main.rs::pokus_o_tichu_aktualizaciu`, presne argumenty
; oficiálneho `tauri-plugin-updater` v režime Quiet) a v čisto tichom režime
; NSIS wizard stránky vôbec nerenderuje ani cez ne neprejde — `PageLeaveReinstall`
; sa preto v reálnej tichej aktualizácii NIKDY nezavolá. Overené živo
; predchodcom tejto úlohy (`ROZPRACOVANE: TLI 52`, 3. beh CI `migracia`
; vo `verify-install.yml`): nová appka sa nainštalovala správne, ale stará MSI
; ostala nainštalovaná vedľa nej.
;
; Tento hook robí to isté priamo v INŠTALAČNEJ SEKCII (`NSIS_HOOK_PREINSTALL`
; beží v `Section Install`, PRED kopírovaním súborov) — tá sa spúšťa VŽDY, aj
; pri čistom `/S`, bez ohľadu na wizard stránky.
;
; ELEVÁCIA: `msiexec` sám o sebe nepotrebuje spustenie cez vlastný "runas"
; krok — Windows Installer rozpozná, že neelevovaný klient skúša odinštalovať
; perMachine produkt, a UAC súhlas si vyžiada SÁM (rovnaké „Installer
; Detection" správanie ako pri dvojkliku na cudzí .msi súbor). Netreba teda
; vlastný elevovaný migrátor spúšťaný appkou (prvá možnosť zo zadania) —
; obyčajný `ExecWait 'msiexec ...'` z neelevovaného NSIS installera stačí,
; UAC okno si vyžiada systém sám. Druhá možnosť zo zadania (nechať starú MSI
; nainštalovanú natrvalo vedľa, appka len upratuje zástupcov) by znamenala
; TRVALO dve položky v „Aplikácie a súčasti" a riziko, že hráč omylom spustí
; zastaranú MSI verziu zo starého odkazu — menej spoľahlivé z pohľadu čistoty
; systému, preto zvolená TÁTO cesta (skutočné odinštalovanie).
;
; MARKER (`$APPDATA\sk.tvojtatko.tli-overlay\migracia-msi-pokusena.flag`)
; zaručuje "UAC raz, potom nikdy": pokus o odinštalovanie starej MSI sa
; urobí NAJVIAC RAZ za celý život inštalácie, bez ohľadu na to, či hráč prvé
; UAC okno schváli alebo odmietne — appka sa ho nebude pýtať znova pri KAŽDEJ
; ďalšej 30-minútovej kontrole aktualizácie donekonečna.
!macro NSIS_HOOK_PREINSTALL
  Push $0
  Push $1
  Push $R0
  Push $R1
  Push $R2
  Push $R3

  StrCpy $R2 "$APPDATA\sk.tvojtatko.tli-overlay"
  IfFileExists "$R2\migracia-msi-pokusena.flag" tli52c_koniec

  ; Hľadaj starú perMachine MSI v HKLM Uninstall podľa DisplayName+Publisher
  ; (rovnaký pár hodnôt, aký kontroluje aj CI `verify-install.yml`) — najprv
  ; 64-bit register view, potom WOW6432Node (appka doteraz stavala len x64
  ; MSI, ale 32-bit view sa kontroluje pre istotu, keby sa to niekedy zmenilo).
  StrCpy $0 0
  tli52c_loop_64:
    EnumRegKey $1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall" $0
    StrCmp $1 "" tli52c_skusaj_32
    IntOp $0 $0 + 1
    ReadRegStr $R0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "DisplayName"
    StrCmp $R0 "${PRODUCTNAME}" 0 tli52c_loop_64
    ReadRegStr $R0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "Publisher"
    StrCmp $R0 "${MANUFACTURER}" 0 tli52c_loop_64
    StrCpy $R1 "$1"
    Goto tli52c_najdene

  tli52c_skusaj_32:
  StrCpy $0 0
  tli52c_loop_32:
    EnumRegKey $1 HKLM "SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall" $0
    StrCmp $1 "" tli52c_nenajdene
    IntOp $0 $0 + 1
    ReadRegStr $R0 HKLM "SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\$1" "DisplayName"
    StrCmp $R0 "${PRODUCTNAME}" 0 tli52c_loop_32
    ReadRegStr $R0 HKLM "SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\$1" "Publisher"
    StrCmp $R0 "${MANUFACTURER}" 0 tli52c_loop_32
    StrCpy $R1 "$1"
    Goto tli52c_najdene

  tli52c_nenajdene:
    ; Žiadna stará MSI nenájdená — marker sa NEPÍŠE (nič sa ani nepokúsilo),
    ; nech appka vie skúsiť znova nabudúce, keby sa objavila iným spôsobom.
    Goto tli52c_koniec

  tli52c_najdene:
    ; $1 je názov podkľúča pod "...\Uninstall" — pre MSI inštaláciu je to
    ; priamo {ProductCode-GUID}, presne to, čo chce zadanie (bod 1:
    ; "msiexec /X{GUID} /qn").
    DetailPrint "TLI 52c: nájdená stará MSI $1 (${PRODUCTNAME}), odinštalúvam"
    ExecWait 'msiexec.exe /X$1 /qn /norestart' $R3
    DetailPrint "TLI 52c: odinštalovanie starej MSI skončilo s kódom $R3"

    CreateDirectory "$R2"
    FileOpen $R0 "$R2\migracia-msi-pokusena.flag" w
    FileWrite $R0 "produkt=$1$\r$\nvysledok_msiexec=$R3$\r$\n"
    FileClose $R0

  tli52c_koniec:
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
  Pop $1
  Pop $0
!macroend
