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
; ⚠️ TLI 67 (20.10.2026) — DVE OPRAVY, OBE Z RICHARDOVHO PC:
; po aktualizácii na `0.24.0` mal nainštalované OBE verzie naraz (stará
; perMachine 0.21.1 v Program Files + nová perUser 0.24.0 v AppData\Local).
; Hook teda bežal, ale starú MSI neodstránil. Príčiny:
;
; (1) `ExecWait 'msiexec /X{GUID} /qn'` Z NEELEVOVANÉHO PROCESU NEVYVOLÁ UAC
;     a odinštalovanie perMachine produktu zlyhá (typicky 1730/1925
;     „insufficient privileges"). Pôvodný komentár tu tvrdil opak — „Installer
;     Detection" elevuje dvojklik na .msi SÚBOR, nie `msiexec /X` s `/qn`,
;     ktoré UI (a teda aj UAC dialóg) výslovne potláča. Opravené na
;     `ExecShellWait "runas"`, čo o eleváciu požiada výslovne: jedno UAC okno,
;     presne ako to Richard schválil v zadaní.
;
; (2) MARKER SA PÍSAL AJ PO NEÚSPEŠNOM POKUSE, takže sa to už nikdy
;     nezopakovalo — stará verzia by tam ostala navždy. Odteraz sa marker
;     zapíše LEN KEĎ stará MSI naozaj zmizla z registra (overuje sa znova
;     prečítaním kľúča, nie návratovým kódom — `ExecShellWait` ho ani
;     nevracia). Keď sa to nepodarí, skúsi sa to pri NASLEDUJÚCEJ aktualizácii
;     znova: najviac jedno UAC okno na jednu aktualizáciu, nie pri každej
;     30-minútovej kontrole.
;
; (3) MARKER MÁ NOVÉ MENO (`...-v2.flag`). Na stroji, kde starý hook už marker
;     zapísal a MSI nechal na mieste (= Richardov PC), by inak nová verzia
;     hooku nikdy nedostala šancu. Starý súbor sa zámerne nemaže.
;
; MARKER (`$APPDATA\sk.tvojtatko.tli-overlay\migracia-msi-pokusena-v2.flag`)
; zaručuje "UAC najviac raz, a len kým je čo odinštalovať": po úspechu sa
; nepýta už nikdy, pri neúspechu najviac raz za aktualizáciu.
!macro NSIS_HOOK_PREINSTALL
  Push $0
  Push $1
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4

  StrCpy $R2 "$APPDATA\sk.tvojtatko.tli-overlay"
  IfFileExists "$R2\migracia-msi-pokusena-v2.flag" tli52c_koniec
  ; Strop pokusov (TLI 67): používateľ bez práv správcu by inak dostal UAC
  ; okno pri KAŽDEJ tichej aktualizácii donekonečna — a tá beží z 30-minútovej
  ; kontroly, teda typicky uprostred hry. Tri pokusy stačia na „hráč okno
  ; omylom zavrel"; po treťom sa to vzdá a marker sa zapíše natrvalo.
  IfFileExists "$R2\migracia-msi-pokus-3.flag" tli52c_vzdavam

  ; Hľadaj starú perMachine MSI v HKLM Uninstall podľa DisplayName+Publisher
  ; (rovnaký pár hodnôt, aký kontroluje aj CI `verify-install.yml`) — najprv
  ; 64-bit register view, potom WOW6432Node (appka doteraz stavala len x64
  ; MSI, ale 32-bit view sa kontroluje pre istotu, keby sa to niekedy zmenilo).
  ; ⚠️ Prepnutie pohľadu na register JE POTREBNÉ. NSIS je 32-bitový proces,
  ; takže bez neho sa `HKLM\SOFTWARE\...` ticho presmeruje do `WOW6432Node`
  ; a 64-bitová vetva registra — tá, do ktorej sa zapisuje x64 MSI — sa
  ; neprehľadá vôbec. Šablóna `installer.nsi` to nikde nenastavuje, takže to
  ; musíme my; na konci makra sa vracia späť na predvolené.
  SetRegView 64
  StrCpy $R4 "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"
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
  SetRegView 32
  StrCpy $R4 "SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
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

    ; Mal starý (perMachine) stav zástupcu v Po spustení? Zistí sa PRED
    ; odinštalovaním, nech sa autostart dá obnoviť už na novú cestu (TLI 67,
    ; bod 4 zadania: "autostart a zástupcovia ukazujú len na novú").
    StrCpy $R3 "nie"
    SetShellVarContext all
    IfFileExists "$SMSTARTUP\${PRODUCTNAME}.lnk" 0 +2
      StrCpy $R3 "ano"
    SetShellVarContext current
    IfFileExists "$SMSTARTUP\${PRODUCTNAME}.lnk" 0 +2
      StrCpy $R3 "ano"

    ; `runas` = výslovná žiadosť o eleváciu (jedno UAC okno). Bez nej
    ; `msiexec /X ... /qn` z neelevovaného inštalátora perMachine produkt
    ; neodinštaluje — pozri hlavičku, bod (1).
    ExecShellWait "runas" "msiexec.exe" "/X$1 /qn /norestart"

    ; Návratový kód `ExecShellWait` nedáva, tak sa úspech overí tam, kde je
    ; jednoznačný: či kľúč v registri ešte existuje.
    ReadRegStr $R0 HKLM "$R4\$1" "DisplayName"
    StrCmp $R0 "" tli52c_odinstalovane

    DetailPrint "TLI 52c: stará MSI tam ostala (UAC odmietnuté alebo zlyhanie) — skúsi sa pri ďalšej aktualizácii"
    ; Poznač neúspešný pokus (1 → 2 → 3). Po treťom sa už nabudúce nespýta.
    CreateDirectory "$R2"
    StrCpy $R1 "$R2\migracia-msi-pokus-1.flag"
    IfFileExists "$R2\migracia-msi-pokus-1.flag" 0 +2
      StrCpy $R1 "$R2\migracia-msi-pokus-2.flag"
    IfFileExists "$R2\migracia-msi-pokus-2.flag" 0 +2
      StrCpy $R1 "$R2\migracia-msi-pokus-3.flag"
    FileOpen $R0 "$R1" w
    FileWrite $R0 "produkt=$1$\r$\nvysledok=neodinstalovane$\r$\n"
    FileClose $R0
    Goto tli52c_zastupcovia

  tli52c_odinstalovane:
    DetailPrint "TLI 52c: stará MSI odinštalovaná"
    CreateDirectory "$R2"
    FileOpen $R0 "$R2\migracia-msi-pokusena-v2.flag" w
    FileWrite $R0 "produkt=$1$\r$\nvysledok=odinstalovane$\r$\n"
    FileClose $R0

  tli52c_zastupcovia:
    ; $WixMode je existujúca globálna premenná šablóny (Var WixMode) — jej
    ; jediný iný zápis je v NEspustenej `PageReinstall` (náš `/S` tam nikdy
    ; nedôjde, pozri komentár hore), takže toto nastavenie tu nič nekoliduje.
    ; DÔVOD, PREČO JU TREBA NASTAVIŤ: `CreateOrUpdateStartMenuShortcut`
    ; (neskôr v TEJTO istej Section) zástupcu v `/UPDATE` režime PRESKOČÍ,
    ; pokiaľ `$WixMode <> 1` — predpokladá, že pri obyčajnom update appky už
    ; zástupca z predošlej inštalácie existuje. Pri migrácii z MSI to ale
    ; neplatí: je to PRVÁ perUser inštalácia a bez tohto nastavenia by hráč
    ; po tichej migrácii nemal v Štart menu (ani na pracovnej ploche) VÔBEC
    ; žiadny zástupca, lebo starý (Program Files) odišiel so starou MSI.
    StrCpy $WixMode 1

    ; Zvyšky zástupcov pre VŠETKÝCH používateľov po starej MSI (TLI 67).
    ; Odinštalovanie MSI si svoje zástupce berie samo, ale keď UAC zlyhalo
    ; alebo si ich hráč niekam skopíroval, ukazovali by na Program Files,
    ; teda na verziu, ktorú chceme mať preč. Per-user zástupcov sa
    ; NEDOTÝKAME — tie si o chvíľu prepíše sama šablóna na novú cestu.
    SetShellVarContext all
    Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    Delete "$DESKTOP\${PRODUCTNAME}.lnk"
    Delete "$SMSTARTUP\${PRODUCTNAME}.lnk"
    ; ⚠️ Kontext sa MUSÍ vrátiť na `current` — zvyšok inštalačnej sekcie
    ; (zástupcovia, register, odinštalátor) je perUser a s `all` by písal
    ; do systémových vetiev, kam neelevovaný inštalátor nemá prístup.
    SetShellVarContext current

    ; Autostart: keď ho hráč mal, nech ho má ďalej — ale na novú appku.
    StrCmp $R3 "ano" 0 tli52c_koniec
    Delete "$SMSTARTUP\${PRODUCTNAME}.lnk"
    CreateShortcut "$SMSTARTUP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    DetailPrint "TLI 52c: autostart prepísaný na novú verziu"
    Goto tli52c_koniec

  tli52c_vzdavam:
    ; Tri pokusy a stále tam je — ďalšie UAC okná by už boli len otravovanie.
    ; Zapíše sa trvalý marker; stará verzia ostane nainštalovaná vedľa novej
    ; (spúšťa sa aj tak len tá nová) a dá sa odstrániť ručne cez „Aplikácie
    ; a súčasti".
    CreateDirectory "$R2"
    FileOpen $R0 "$R2\migracia-msi-pokusena-v2.flag" w
    FileWrite $R0 "vysledok=vzdane-po-3-pokusoch$\r$\n"
    FileClose $R0
    DetailPrint "TLI 52c: po troch pokusoch sa stará MSI odstrániť nedala — ďalej sa to neskúša"

  tli52c_koniec:
  SetRegView default
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
  Pop $1
  Pop $0
!macroend
