# Zoznam zmien

Projekt využíva sémantické verzovanie (Semantic Versioning). Verzie sú označované v tvare `<major>.<minor>.<patch>` a git tagy na releasy v tvare `tli-v<verzia>`.

## 0.28.0 — 20.10.2026 (TLI 70 — zber dát na učenie naozaj zbiera)

**Čo Richard nahlásil 20.10.2026 ~12:4x** (v0.24.0, `tli-overlay.log`): zber na
učenie nefunguje ani jednou cestou. Minimapa fotí („minimapa KuangDuZhiYuan
odfotená (104133 B)"), ale KAŽDÉ odoslanie vráti „Server má poruchu a analýzu
teraz nespraví…" a vo fronte čaká 1488 snímok. Frozen Canvas hlási „automatické
učenie Canvasu beží (čaká 0 snímok)" — a priečinok `canvas-zber` je PRÁZDNY,
hoci Canvas hral a klikal celý deň.

**Minimapa — príčina bola na SERVERI, nie v appke.** `tli-web.service` má
`ProtectSystem=strict` a `/srv/games/tli/minimapa` chýbal v `ReadWritePaths`
(presne tá istá chyba ako pri `tracker` v TLI 25): endpoint padal na
`OSError: [Errno 30] Read-only file system` a vracal 500. Appka sa správala
správne — poruchu rozpoznala a frontu držala. Opravené na serveri; nič sa
nestratilo, fronta sa po oprave sama vyprázdnila.

**Canvas — appka sa odteraz nespolieha len na pixely.** Lokálna detekcia
(`canvas_detekcia`, pomer farieb) ostáva prvou, lacnou cestou, ale keď
nepovie „Canvas", appka sa oprie o to, **čo server naposledy naozaj
PREČÍTAL**: po každej úspešnej ručnej analýze (Ctrl+Q/Ctrl+W/F8/F9) si appka
pamätá názov obrazovky a 5 minút ho berie ako platný
(`core/src/zber_stav.rs`). Zároveň klik, ktorým hráč zhasína radu, už nie je
stratený — appka počká až 900 ms, kým panel zmizne, namiesto toho, aby snímku
ticho zahodila.

**Doska Tartaru sa zbiera tiež** (dovtedy len Canvas) — vlastná fronta
`tartarus-zber`, vlastný denný strop, endpoint `/api/analyza?zber=1`.

**Každé zamietnutie má odteraz dôvod.** Do TLI 70 vracalo všetkých šesť
podmienok `false` ticho, takže z denníka sa nedalo zistiť, prečo sa nefotí —
a presne preto sa táto porucha nedala diagnostikovať na diaľku. Teraz sa
dôvody počítajú (`hra-nie-je-v-popredi`, `panel-appky-svieti`,
`obrazovka-sa-nezbiera`, …), raz za 5 minút sa to, čo PRIBUDLO, zhrnie do
denníka (bežný stav „nehrám" sa nevypisuje) a vidno ich aj v Nastaveniach.

**Nové v Nastaveniach: „Stav zberu na učenie"** (bod 5 zadania) — riadok
„Učenie: Canvas X odoslaných / Y čaká, Tartarus …, Minimapa …, posledná
chyba …", pod ním posledná obrazovka potvrdená serverom a dôvody, prečo sa
po kliku nefotilo. Obnovuje sa sám každých 5 s, kým sú Nastavenia otvorené.

**Odstup medzi odosielanými snímkami zberu je 3 s** (bolo 0,5 s) a server má
preň **vlastný rate-limit kôš** (`zber_za_minutu`). Dovtedy zber na pozadí
jedol ten istý limit ako Richardove ručné F8/Ctrl+W — pri vyprázdňovaní
fronty by mu ručná analýza prestala chodiť.

**Nič sa nemaže** (nezmenené od TLI 60): pri plnej fronte appka len prestane
pridávať; zo spoolu mizne výhradne snímka, ktorú server práve prijal.

## 0.27.0 — 20.10.2026 (TLI 69 — Frozen Canvas radí pre VŠETKY smery, lebo cieľové bunky kreslí sama hra)

**Čo Richard nahlásil 20.10.2026 ~12:2x.** Pri cieli „Fill a Vertical Cell with
a Monster Paint (0/3)" napísal overlay „Zber dát #1 — Frozen Canvas — žiadny
krok. ciel.smer 'vertical' nemá overené pravidlo — solver ho nespočíta". Na tej
istej snímke pritom bolo jasne vidieť, že **hra platné cieľové bunky sama žlto
orámovala**: v stĺpci 3 nad zdrojovým monštrom Lv3 so značkou „!" (riadok 6) to
boli riadky 1, 2, 4 a 5 — riadok 3 je ľad.

**Zovšeobecnenie.** Dosah Color Mixing Effectu sa už NEODVODZUJE z mena smeru.
Platné cieľové bunky = bunky, ktoré hra orámovala; pravidlo je tým jedno pre
`adjacent`, `vertical`, `horizontal`, `diagonal` aj čokoľvek ďalšie, a text
smeru (OCR/vision) slúži už len na kontrolu. Keď si rám a text protirečia,
platí rám.

**Nový modul `core/src/canvas_ramy.rs`** — detekcia rámu lokálne v appke, z
pixelov, nad pevnou mriežkou 6×6 (`canvas_rozlozenie`). Beží nad snímkou
v PLNOM rozlíšení (serveru ide zmenšený JPEG, v ktorom je ~4 px čiara už
nespoľahlivá), bez OCR a bez vision modelu. Zoznam buniek ide na server ako
`?ramy=1-3.2-3.4-3.5-3` a solver z neho berie dosah.

**Dve veci, ktoré rozhodujú medzi „funguje" a „nefunguje"** (obe sú v module
rozpísané aj s číslami):

1. *Rám suseda vyzerá ako vlastný.* Bunky mriežky na seba doliehajú a rám sa
   kreslí tesne vnútri bunky, takže čiara suseda padne do okna hľadania.
   Z kandidátov sa preto berie ten **bližšie k vnútru** bunky, nie ten
   s najvyšším pokrytím.
2. *Zdrojová bunka so značkou „!" orámovaná nie je — ale pri smere `adjacent`
   ju zo všetkých štyroch strán obkolesujú orámovaní susedia*, takže podľa
   pokrytia by vyšla ako orámovaná. Rozlíši ju až **rozmer** nájdeného
   obdĺžnika: vlastný rám je menší než bunka (namerané 91–99 px), zlepenina zo
   štyroch susedov väčšia (105–107 px). Samotné pokrytie na to nestačí —
   rozpätia sa prekrývajú (orámované ≥ 0,161, neorámované ≤ 0,196).

**Overenie na reálnych dátach — 75 unikátnych snímok Frozen Canvasu** z
`/srv/games/tli/analyzy` (Richardovo hranie 19.–20.10.2026):

| | výsledok |
|---|---|
| snímok, kde hra rám kreslila | 30 |
| zhoda tvaru orámovanej množiny s menom smeru z textu hry | **27/27** (adjacent 10×, horizontal 10×, vertical 4×, diagonal 3×; zvyšné 3 nemali rozpoznaný text) |
| snímok bez rámu | 45 — **všetky** z podobrazoviek „Select the Brushstroke direction" (37) a „Draw" (8), kde ho hra nekreslí |
| falošne kladné | 0 |
| zhoda Rust detektora s referenčným Python prototypom | 75/75 snímok, 0 rozdielov |

**Čo sa zmenilo v solveri** (`canvas_solver.rs` aj `canvas_solver.py`, ďalej
riadok po riadku to isté):

- dosah zo žltého rámu pre ľubovoľný smer, so **zálohou podľa mena smeru**
  (`bunky_v_smere`) pre snímky bez rámu — istota 0,9 vs 0,7;
- **rám ostáva aj na obsadenej bunke** (doložené párom `2026-10-20-151222`
  „0/3" → `-151242` „2/3" nad tým istým boardom), takže položiť sa dá len na
  `oramovana && dostupna && paint == null`;
- **cieľ „with a Gameplay Paint" už berie Gameplay Painty**, nie Monster —
  v Richardových snímkach je takých cieľov zhruba tretina a solver na nich
  dovtedy radil nesprávnu kategóriu;
- výber paintov z ruky **maximalizuje výsledné levely** (`min(level+N, strop)`)
  namiesto doterajšieho „najnižší level prv"; pri zhode ide prv nižší vstupný
  level, nech sa z boostu premrhá menej;
- počet levelov sa číta z textu efektu („by N level(s)"), predvolene 1.

**Overlay (bod 4 zadania): rada sa UKÁŽE, aj keď efekt pre daný smer overený
nie je** — len je označená. Nadpis odteraz pomenuje smer („⚠️ neoverené pre
vertical — Frozen Canvas — 3 kroky"), nielen ⚠️, a panel rozlišuje dve veci,
ktoré sa dovtedy zlievali: *cieľové bunky orámovala sama hra (isté)* vs
*cieľové bunky odhadnuté z názvu smeru*. Dvojice zdroj→cieľ sa číslujú ako
doteraz a rada sa skrýva klikom (TLI 67) — to sa nemenilo.

**Čo NIE JE overené a vie sa o tom.** EFEKT (o koľko levelov vyskočí položený
paint) je z párov pred/po potvrdený ďalej len pre `adjacent`. Pre `vertical`
nie je v dátach z 19.–20.10.2026 **ani jeden** pár pred/po — všetkých 16 snímok
má cieľ 0/3 — takže solver berie číslo z textu hry a radu označí. Zapísané do
`canvas.yml` so stupňom istoty („DOSAH OVERENÝ (rám), EFEKT NEOVERENÝ").

**Testy:** core 631 (+13 `canvas_ramy` vrátane 5 fixtures z reálnych snímok
a testu odolnosti voči posunu mriežky o ±3 px, +11 solver), UI 127
(+6 `testy-canvas-ramy.e2e.mjs`), `tli-web` 233 (+3), `testy-canvas-solver.py`
27 (+13).

## 0.26.0 — 20.10.2026 (TLI 68 — dropy po reštarte appky sa už nerátajú ako celý stack z batohu)

**Chyba, ktorú Richard nahlásil 20.10.2026 krátko po 12:00.** Mapa
„Sheng Huo Ling", čas 1 m 36 s, a v zozname „ČO PADLO" stálo
*Voidlands Beacon (Timemark 8) ×49*, *Glacial Abyss Beacon ×16*,
*Secret of the Glacial Abyss ×49* a *Memory Scrap ×155*. V skutočnosti padol
jeden Voidlands Beacon — appka započítala **celý stack, ktorý mal v batohu**.
ZISK 214 FE, FE/h 8 034 aj čísla session boli preto nezmyselne vysoké.

**Príčina.** Appka sa o 11:54 reštartovala. `StavChvosta` si cez reštart drží
**pozíciu v logu**, ale stav batohu nie — appka teda pokračovala v čítaní
uprostred súboru s PRÁZDNYM batohom. Riadok `BagMgr@:Modfy` pritom nenesie
prírastok, ale **absolútne `Num`** (koľko kusov je v slote teraz), takže prvá
zmena každého slotu vyšla ako `Num − 0`, teda celý stack. Reprodukované nad
Richardovým skutočným logom: `6002 ×49`, `400033 ×49`, `400007 ×16`,
`5140 ×155` — presne čísla zo screenshotu.

**Oprava.** Appka si odteraz vedie zoznam stránok batohu, ktorých dump
(`BagMgr@:InitBagData` — hra ho vypíše pri prihlásení a po **Sorte**) v TEJTO
session naozaj videla:

- Slot na stránke so známym dumpom, ktorý v ňom nebol, je naozaj prázdny →
  prvý drop doň sa počíta **celý**.
- Slot na stránke **bez** dumpu má **neznámy základ** → jeho prvá zmena sa do
  dropov **nezapočíta vôbec** (radšej chýbajúci drop než drop ×49). Zapíše sa
  do denníka appky a v hlavnom okne sa ukáže **„Sortni inventár pre presné
  dropy"** aj s počtom vynechaných zmien. Druhá zmena toho istého slotu už
  základ má a je presná — jeden Sort v hre to vyrieši naraz pre celú stránku.
- **Snímka batohu z DB** (TLI 56) ostáva **len** pre Current Inventory a Net
  Worth. Ako základ dropov sa **nepoužíva nikdy**: hráč mohol medzitým predať,
  presortovať alebo hrať bez appky.
- **Reset session** — ručný aj automatický po 30 min — nuluje **len**
  štatistiky session. Stavu batohu ani základu dropov sa nedotkne.
- Keď slot zmení položku za **inú** (iné `ConfigBaseId`), zapíše sa len zisk
  novej; stará sa **nezapíše ako strata** (hra pri Sorte položky len presúva).
  Rovnako to robí referenčný TITrack.

**A hlavne: po reštarte už appka drop nestratí.** Samotné „bez základu
nepočítam" je síce správne, ale drahé — na Richardovom dnešnom logu by reštart
o 11:54 zahodil dropy v 12 mapách, kým by si spomenul na Sort. `StavChvosta`
si ale cez reštart drží POZÍCIU v logu, takže appka tú istú časť súboru vie
prečítať ešte raz do zahoditeľného sledovača a prevziať z neho **len stav
batohu**. Behy, dropy ani ceny z toho nevzniknú (tie už appka spracovala, keď
riadky čítala prvý raz). Výzva na Sort ostáva ako druhá poistka pre prípad,
keď sa nažhavenie nemá o čo oprieť — napríklad keď hra log medzitým rotovala.

**Ešte jeden nález z overovania: prázdna stránka batohu je tiež známa stránka.**
Pri prihlásení hra vypíše `BagMgr@:InitBagData` len pre stránky, v ktorých
niečo je — pre prázdnu nepríde ani jeden riadok. Appka ju preto nepovažovala
za známu a prvý drop do nej zahodila, hoci o nej vedela všetko (Richardov log
zo 17.10., mapa `YJ_LiuJinJieQu200`). Spoľahlivý signál je
`ItemChange@ Reset PageId=N`, ktorý hra pošle **aj pre prázdnu stránku** —
appka ho odteraz číta.

**Overené proti TITracku** (`astockman99/TITrack`) na Richardových skutočných
logoch, per mapu, zoznam dropov 1:1 — dnešok aj všetkých päť archivovaných
rotácií, spolu **748 máp a 2,3 GB logu**:

| log | máp | pred opravou | po oprave |
|---|---|---|---|
| dnešok (206 MB) | 77 | 77/77 | 77/77 |
| 17.10. (149 MB) | 81 | 81/81 | 81/81 |
| 17.–18.10. (66 MB) | 33 | 33/33 | 33/33 |
| 18.–19.10. (846 MB) | 258 | 258/258 | 258/258 |
| 19.10. (30 MB) | 6 | 6/6 | 6/6 |
| 19.–20.10. (977 MB) | 296 | 296/296 | 296/296 |
| **dnešok + reštart appky o 11:54** | 74 | **62/74** | **74/74** |

Nič, čo fungovalo, sa teda nepokazilo, a jediný riadok, ktorý sa hýbe, je
práve ten nahlásený.

**Jednorazový prepočet histórie.** Pri prvom štarte si appka spraví **kópiu
databázy** (`tracker.sqlite3.pred-tli68-<čas>`, overí, že nie je prázdna — bez
nej prepočet vôbec nespustí) a prejde logy, ktoré má hráč na disku, opraveným
výpočtom. Prepíše **len** položky behov, ktoré v databáze naozaj sú; nové mapy
nevytvára a nič nemaže.

## 0.25.2 — 20.10.2026 (TLI 67 — rada sa po kliku skryje vždy; migrácia zo starej MSI)

**Chyba, ktorú Richard nahlásil 20.10.2026 o 11:53: „Prečo mi teraz po kliknutí
nezmizol overlay?"** Panel s radou pre dosku Tartarusu ostal visieť nad hrou,
hoci rovnaké panely pri `Objective` a pri výbere nástroja predtým zmizli
správne. Príčina **nebola viazaná na typ obrazovky** — cesta v kóde je pre
všetky (doska, nástroj, poklad, objective, poradca, Frozen Canvas aj chyba)
jedna a tá istá. Bolo to ČASOVANIE a boli to dve nezávislé chyby naraz:

1. **Sledovanie kliku sa armovalo neskoro.** Spúšťalo sa až po kontrole, či sa
   panel naozaj dostal na obrazovku (TLI 40/44) — a tá obsahuje jedno až dve
   čakania po 400 ms plus dve odfotenia celého okna hry, teda 0,4–1,6 s. Každý
   klik v tom čase sa stratil a rada potom čakala na DRUHÝ klik. Prejavovalo sa
   to hlavne pri doske, kde appka zvýrazní tlačidlo, ktoré hráč aj tak chcel
   stlačiť, takže klikne hneď; pri kartičkách si ich najprv prečíta.
   Teraz sa sledovanie armuje v tej istej chvíli, ako sa rada zobrazí, a
   kontrola prezentácie beží až po ňom (a keď panel medzitým zhasne, preskočí
   sa, nech do denníka nepíše zavádzajúce „frame prezentovaný: NIE").
2. **Krátky klik sa dal minúť.** Stav tlačidiel sa čítal každých 25 ms a len
   cez „tlačidlo je práve dole" — klik kratší než jeden odber teda appka
   nevidela vôbec. Odber je odteraz každých 10 ms a okrem toho sa číta aj
   „stlačené od posledného odberu". Dozvuk kliku spred zobrazenia rady sa
   naďalej nepočíta (zadanie: klik počas „analyzujem/odosielam" sa neráta).

**Pravidlo je teraz pre všetky rady a zvýraznenia rovnaké:** zhasnú pri PRVOM
kliku, pri Esc, alebo po 120 sekundách. Rámiky ciel na doske aj dvojice rámikov
na Frozen Canvase sú súčasťou toho istého okna ako panel, takže miznú spolu
s ním.

**Denník odteraz píše, že sledovanie naozaj beží** („sledujem klik (skryje sa
pri prvom kliku, Esc, alebo o 120 s)"). Presne tento riadok 20.10. chýbal a bez
neho sa z logu nedalo rozlíšiť, či sa sledovanie nespustilo, alebo spustilo
a klik minulo.

**Frozen Canvas ide konečne tou istou cestou ako ostatné výsledky.** Bol
jediný, ktorý emitoval výsledok priamo, teda bez pulzu, ktorým sa budí WebView2
(TLI 44), a bez druhého poslania polohy panela.

**Migrácia zo starej MSI (TLI 52c) opravená.** Na Richardovom PC ostali po
aktualizácii na 0.24.0 nainštalované OBE verzie — stará perMachine 0.21.1
v `Program Files` aj nová perUser v `AppData\Local`. Hook bežal, ale:
`msiexec /X{GUID} /qn` z neelevovaného inštalátora UAC **nevyvolá** a tiché
odinštalovanie perMachine produktu zlyhá; a marker „už sme to skúšali" sa
zapisoval aj po neúspechu, takže sa to nemalo ako zopakovať. Odteraz sa
o eleváciu žiada výslovne (`runas` — jedno UAC okno), úspech sa overuje
prečítaním registra (nie návratovým kódom) a marker sa zapíše len po skutočnom
odinštalovaní; inak sa to skúsi pri nasledujúcej aktualizácii. Marker má nové
meno, takže stroj, kde starý pokus zlyhal, dostane ešte jednu šancu. Zvyšné
zástupce po starej verzii (pre všetkých používateľov) sa zmažú a autostart,
ak ho hráč mal, sa prepíše na novú appku.

K tomu dve poistky z review diffu (druhý názor cez API kredit): hook si teraz
výslovne prepína pohľad na register (`SetRegView 64`/`32`) — NSIS je
32-bitový proces, takže bez toho sa `HKLM\SOFTWARE\…` ticho presmeruje do
`WOW6432Node` a 64-bitová vetva, kam sa x64 MSI zapisuje, by sa nemusela
prehľadať vôbec. A pokusy o odinštalovanie sú zastropované na **tri**: hráč
bez práv správcu by inak dostával UAC okno pri každej tichej aktualizácii
donekonečna, teda typicky uprostred hry.

## 0.25.1 — 20.10.2026 (TLI 65 — `http://` na lokálnu slučku, core-0.17.1)

`Nastavenia::chyba_konfiguracie()` odteraz prepustí `http://` adresu servera,
ale **len na lokálnu slučku** — `127.0.0.1`, `[::1]`, `localhost` a nič iné.

Prečo: testovacia Windows VM nemá osobný kľúč na `tli.tvojtatko.sk`
(`POST /api/analyza` chce `Authorization: Bearer` viazaný na Discord účet
s rolou), takže appka tam nad hrou **nikdy nič nenakreslila** a test to nevedel
odhaliť. Prehrávací server na `127.0.0.1:8099` vracia známu odpoveď a test tak
vie overiť, či overlay sedí na správnej cele — `https` by na ňom znamenalo
vlastný certifikát a tým novú krehkú závislosť.

Bezpečnostne sa nestráca nič: prevádzka na loopback z počítača nikdy neodíde.
Výnimka je úzka a má na to vlastný test — `http://192.168.0.5` aj
`http://localhost.utocnik.sk` cez ňu **neprejdú**.

Obsahuje aj celé `0.25.0` (zrušené záložky Solvery a Analýza).

## 0.25.0 — 20.10.2026 (TLI 65 — appka bez simulátora, analyzera a trénera)

Richard 20.10.2026: „Ja nepotrebujem analyzer v appke!" Appka má mať **Tracker,
Inventory, Economy, Nastavenia** a overlaye (Tartarus, Frozen Canvas, ceny z trhu,
loot mini-panel, zber minimapy) — nič viac.

- **Zrušená záložka „Solvery"** — bol to len prehľad dvoch klávesových skratiek;
  tie sú (a vždy boli) v Nastaveniach. Hlavičkové tlačidlo „Overlay" vedie odteraz
  do Nastavení.
- **Zrušená záložka „Analýza"** vrátane podsekcie „Taktiky" — prehliadač histórie
  behov, exporty CSV/JSON, štatistika setupov a import starých logov.
- Obidve sú **archivované**, nie zmazané: `archiv/tli65-solvery-analyza-2026-10-20/`
  (HTML aj JS fragmenty + postup, ako ich vrátiť).
- Rust backend týchto funkcií ostáva v kóde — zdieľa databázu s Trackerom a Economy.
  Appka ho po tejto zmene nevolá.

**Žiadna kruhová („rotunda") grafika v appke nikdy nebola a nie je** — kruhová doska
zo snímky v hlásení k 0.24.0 pochádzala zo starej snímky NÁŠHO WEBU z TLI 8, ktorú
omylom používal testovací simulátor okna hry. Opravené mimo tohto repozitára
(`scripts/win-test-snimky.sh` v repe `ops`).

Zvyšok appky (overlaye, tracker, mini-panel, zber minimapy) je nezmenený oproti
`0.24.0`.

## 0.24.0 — 20.10.2026 (TLI 62/3c — release zberu minimapy, core-0.17.0)

Prvý release obsahujúci zber snímok minimapy na stavbu knižnice trás k
bossovi / full clear (TLI 62/2, 62/2b, 62/3a+b, úlohy `2026-10-20-0600-agent.md`
a predchádzajúce). **Funkcia je RUČNE zapínaná a PREDVOLENE VYPNUTÁ** —
`Nastavenia.minimapa_zber_zapnute = false` v predvolenom nastavení, hráč si
ju musí sám zapnúť v Nastaveniach a zvoliť režim ROI (`roh`/`stred`). Keď je
vypnutá, appka sa správa úplne rovnako ako `0.23.2`.

Zbiera sa výhradne to, čo minimapa v hre ukazuje (žiadne odkrývanie
neodhalených častí) — vlákno `zber_minimapa.rs` sleduje beh mapy cez stav
trackera (nie druhý log-sledovač) a odosiela výrezy s metadátami zóny na
server, kde sa archivujú pre budúcu statickú knižnicu máp (62/4+).

core 580/580 testov, UI e2e 118/118, `cargo check` zelený v `core` aj
`src-tauri`.

## 0.23.2 — 20.10.2026 (TLI 52c doplnok — po migrácii chýbal Start Menu zástupca)

Živo zistené vlastnou CI poistkou `migracia` (nový krok „Start Menu zástupca
po migrácii ukazuje na NOVÚ verziu") hneď pri prvom behu nad `tli-v0.23.1`:
šablóna `tauri-bundler` preskakuje tvorbu zástupcu vždy, keď `$UpdateMode = 1`
A SÚČASNE `$WixMode <> 1` (`CreateOrUpdateStartMenuShortcut`,
`installer.nsi`) — predpokladá, že pri bežnej aktualizácii zástupca z
PREDOŠLEJ inštalácie už existuje. Pri migrácii z MSI to ale neplatí: ide
o PRVÚ perUser inštaláciu a starý zástupca (v Program Files) zmizol spolu
so starou MSI pri `msiexec /X`. Bez tohto doplnku by hráč po tichej migrácii
nemal VÔBEC ŽIADEN spôsob, ako appku spustiť zo Štart menu ani z pracovnej
plochy.

`installer-hooks.nsh` teraz pri úspešnom nájdení a odinštalovaní starej MSI
nastaví `$WixMode 1` (existujúca globálna premenná šablóny) — tým istým
príznakom, ktorý by nastavila VSTAVANÁ migrácia, keby v tichom behu fungovala
(pozri 0.23.1 vyššie). `CreateOrUpdateStartMenuShortcut` aj analogická
funkcia pre zástupcu na ploche tak zástupcu vytvoria aj v `/UPDATE` režime.

Žiadna iná zmena oproti 0.23.1 — `tli-v0.23.1` ostáva v GitHub releasoch ako
historický (nikdy neponúknutý v `latest-tli.json`), hráči dostanú rovno
`0.23.2`.

## 0.23.1 — 20.10.2026 (TLI 52c — vlastný installer-hooks.nsh, migrácia starej MSI konečne funguje v tichom behu)

Úloha `2026-10-20-0608-mcp.md` (HNED), pokračovanie `ROZPRACOVANE: TLI 52`.
Žiadna zmena v `core` (ostáva 0.17.0) — len NSIS inštalátor appky.

**Príčina, prečo vstavaná migrácia šablóny (0.20.5) v reálnom tichom behu
nefungovala, bola nájdená:** `tauri-bundler` 2.12.3 rieši migráciu zo starej
WiX (MSI) inštalácie na vlastnej wizard STRÁNKE (`Page custom PageReinstall
PageLeaveReinstall`) — jej `Leave` funkcia (kde je `ExecWait` na starú
`UninstallString`) sa spustí len cez skutočný UI prechod tej stránky alebo
keď je nastavený `$PassiveMode` (flag `/P`). Appka pri tichej aktualizácii
posiela len `/S` (bez `/P`) — presne argumenty, aké posiela oficiálny
`tauri-plugin-updater` v režime Quiet — a v čisto tichom režime NSIS wizard
stránky vôbec neprejde, takže `PageLeaveReinstall` sa NIKDY nezavolal. Presne
to zachytila CI poistka `migracia` (`verify-install.yml`) postavená
v `ROZPRACOVANE: TLI 52`: nová perUser appka sa nainštalovala správne, stará
perMachine MSI ostala nainštalovaná vedľa nej.

**Riešenie: vlastný `src-tauri/installer-hooks.nsh`**
(`bundle.windows.nsis.installerHooks` v `tauri.conf.json`), makro
`NSIS_HOOK_PREINSTALL` — beží priamo v `Section Install`, PRED kopírovaním
súborov, VŽDY (aj pri čistom `/S`, nezávisle od wizard stránok). Nájde starú
MSI v `HKLM\...\Uninstall` (64-bit aj `WOW6432Node`) podľa zhody
`DisplayName`+`Publisher`, spustí `msiexec.exe /X{ProductCode} /qn /norestart`
a zapíše marker `migracia-msi-pokusena.flag` do `%APPDATA%\sk.tvojtatko.tli-overlay\`.

**Rozhodnutie o elevácii (zadanie ponúkalo dve cesty, vybraná tretia — čistejšia
než obe):** `msiexec` z neelevovaného NSIS installera nepotrebuje vlastný
"runas" krok — Windows Installer sám rozpozná pokus neelevovaného klienta
odinštalovať perMachine produkt a UAC súhlas si vyžiada SÁM ("Installer
Detection", rovnaké správanie ako dvojklik na cudzí `.msi`). Netreba teda
písať vlastný elevovaný migrátor spúšťaný appkou (prvá možnosť zo zadania) —
tá by pridala kód a riziko navyše za rovnaký výsledok. Druhá možnosť (nechať
starú MSI nainštalovanú natrvalo, appka len maže zástupcov) bola zamietnutá:
znamenala by TRVALO dve položky v „Aplikácie a súčasti" a riziko, že hráč
omylom spustí zastaranú MSI verziu zo starého odkazu. **Marker v `%APPDATA%`
zaručuje „UAC raz, potom nikdy"** bez ohľadu na to, či hráč prvé UAC okno
schváli alebo odmietne — appka sa ho nebude pýtať znova pri každej ďalšej
30-minútovej kontrole aktualizácie.

**Richard pri prvom prechode UVIDÍ presne JEDNO UAC okno** (štandardný
Windows dialóg Windows Installeru na odinštalovanie starej MSI) — appka nič
vlastné nekreslí, nič iné sa nepýta. Všetky ďalšie aktualizácie (NSIS→NSIS)
sú odvtedy úplne ticho, bez akéhokoľvek UAC — marker súbor zariadi, že sa
o migráciu appka nepokúsi druhýkrát ani keby prvý pokus zlyhal/bol odmietnutý.

**CI `migracia` vo `verify-install.yml` rozšírená** — pribudlo overenie
`tracker.sqlite3` (predtým sa kontrolovalo len `nastavenia.json`) a nový krok
„druhý update NSIS→NSIS": zopakuje tichú inštaláciu a overí, že marker súbor
sa NEPREPÍSAL (časová značka nezmenená) — teda že hook sa pri druhej
aktualizácii vôbec nepokúša o `msiexec`, čo je jediné, čo CI runner (bežiaci
ako Administrator, teda bez reálneho UAC dialógu) vie o „žiadny ďalší UAC"
overiť. Skutočné zobrazenie/počet UAC okien na ľudskom desktope overuje
samostatne test na `TLI-TEST-VM` (krok 4b release flow), nie CI.

**⚠️ `autostart ukazuje na novú verziu" zo zadania nie je appkou implementovaný
cieľ** — appka žiadny Windows autostart (spúšťanie pri štarte systému) nemá
a nikdy nemala; zadanie túto formuláciu prevzalo z vlastného návrhu „druhej
možnosti" (ponechať starú MSI, appka maže „zástupcov a autostart starej"),
ktorá napokon nebola zvolená. CI preto namiesto toho overuje Start Menu
zástupcu (`.lnk`), že po migrácii ukazuje do `%LOCALAPPDATA%` (nová perUser
inštalácia), nie do starého `C:\Program Files\...`.

## core 0.17.0 — 20.10.2026 (TLI 62/3a+b: vlákno zberu minimapy, Nastavenia UI; BEZ appka releasu)

Self-queued pokračovanie TLI 62 (`inbox/2026-10-20-0600-agent.md`), nadväzuje
na jadro zberu (62/2). Prepája `core/src/minimapa_fronta.rs` so skutočným
oknom hry, diskom a sieťou — vzor `zber_canvas.rs` (TLI 60).

- `core::minimapa_fronta::kod_zony(cesta)` — posledný segment
  `InMainLevelPath` (kontrakt, parameter `mapa`), nie `LevelId` (62/0: jedna
  zóna má 6–8 `LevelId`). Tracker (`Beh::zona`) nesie celú cestu, appka pred
  odoslaním posiela len tento posledný kus.
- `Nastavenia.minimapa_zber_zapnute` (**predvolene VYPNUTÉ** — ToS, appka
  nesmie fotiť nikoho, kto si to sám nezapol) + `Nastavenia.minimapa_rezim`
  (`RezimMinimapy`, predvolene `stred`) + `adresa_minimapa()`/
  `adresa_minimapa_stav()`.
- `core::api::{MinimapaOdpoved, MinimapaStavOdpoved, MinimapaMapaStav}` —
  tvar odpovedí `POST /api/minimapa` a `GET /api/minimapa/stav` z kontraktu.
- Nový `src-tauri/src/zber_minimapa.rs` — dve vlákna (sledovač + odosielač),
  vlastná fronta na disku (`minimapa-zber/`, nezdieľaná s Canvasom). Zdroj
  „beh mapy prebieha" je TRACKER (`StavTrackera::aktualny_beh`, nová
  verejná metóda), nie druhý sledovač `UE_game.log`. Sidecar `<meno>.jpg.meta`
  vedľa každej snímky nesie `mapa`/`level_uid`/`rezim`/`okno`/`roi` presne
  také, aké platili v čase ODFOTENIA (nerozširuje už otestovaný formát mena
  súboru z kroku 62/2). `klient::posli_minimapa_zber`/`ziskaj_stav_minimapy`.
- Nastavenia UI: prepínač „Zber minimapy" + výber režimu (stred/roh),
  5 nových e2e testov (`testy-nastavenia-minimapa-zber.e2e.mjs`).
- Testy: core 580/580 zelené (9 nových), `cargo check` zelený v `core` aj
  `src-tauri`, UI e2e 118/118 zelené.
- **Appka sa NEVYDÁVA v tomto kroku** — release, oznam do `#torchlight` a
  test na TLI-TEST-VM (VM 210) sú 62/3c, self-queued samostatne.

## core 0.16.0 — 20.10.2026 (TLI 62/2: jadro appky pre zber minimapy — ROI, ticker, denný strop, fronta, `dost_dat`; bez appka releasu)

Self-queued pokračovanie TLI 62 (`inbox/2026-10-20-0440-agent.md`), nadväzuje
na server (62/1) a vzhľad trasy (62/2b). Kontrakt:
`docs/fakty/hry/torchlight-infinite/minimapa-api-kontrakt.md`.

- Nový modul **`core/src/minimapa_fronta.rs`** — rozhodovanie zberu, bez
  obrazovky/siete/Windowsu (vzor `zber_fronta`, TLI 60):
  - `RoiMinimapy`/`RoiSada` — výrez minimapy v `Box01` pre oba režimy
    (`roh`, `stred`), s `normalizuj()` a `ako_parameter()` (`x,y,w,h` pre
    kontrakt). Predvolené hodnoty sú **PRACOVNÁ HYPOTÉZA** (žiadna snímka
    minimapy zatiaľ nebola zozbieraná), prepísateľné cez nový
    `Nastavenia.minimapa_roi` bez nového buildu.
  - `RezimMinimapy` (roh/stred) s parsovaním z textu.
  - `ma_sa_fotit()` — ticker á 2 s, len keď beh mapy prebieha (nie
    v hube/lobby).
  - `DennyStav` — **vlastná inštancia, nezdieľaná s Canvasom** (zdieľaný
    strop by dva zbery jedol navzájom), `smie_pribudnut`/`odstup_po_zlyhani`/
    `nazov_snimky`/`je_subor_fronty` s prefixom `minimapa-`.
  - `DostDatStav` — kombinuje cache `GET /api/minimapa/stav` (platnosť 5 min,
    nepýta sa á 2 s) a pamäť `prijate: false` z `POST /api/minimapa`
    (platí len do konca aktuálneho behu, `level_uid`).
- `Nastavenia.minimapa_roi` (`RoiSada`) + normalizácia (`#[serde(default)]`,
  starý `nastavenia.json` dostane odhadnuté ROI, nie nulové).
- Testy: **25 nových** v `minimapa_fronta` + **3 nové** v `nastavenia`
  (571 spolu v `core`, zelené). `cargo check` zelený v `core` aj
  `src-tauri`. **Appka sa nevydáva** (release je 62/3) — volajúci (vlákno
  nad oknom hry, sledovač logu) zatiaľ neexistuje, modul nikto nevolá.

## core 0.14.0 — 20.10.2026 (TLI 62/2b: vzhľad trasy nad minimapou — opacita, hrúbka, farba, koliesko myši; bez appka releasu)

Doplnok 1 k TLI 62 (`inbox/done/2026-10-20-0158-mcp.md`, bod 4). Richard
20.10.2026: „nech sa tam dá nastaviť opacita."

- Nový modul **`core/src/minimapa_vzhlad.rs`** — `VzhladTrasy`: opacita čiary
  aj celej vrstvy (0–100 %), hrúbka 1–12 px, farba `#RRGGBB`, `koliesko()`
  po 5 % a `farba_rgba()` pre kreslenie. **Nula je platná hodnota** („dočasne
  to nechcem vidieť, ale nevypínam to"), preto nie je v `Rozlozenie`
  (`priehladnost` tam má dolnú medzu 0,3 a platí pre všetky panely naraz).
- `Nastavenia.vzhlad_trasy` + normalizácia (`#[serde(default)]`, takže starý
  `nastavenia.json` dostane viditeľnú predvoľbu, nie nuly).
- **Nastavenia (UI):** sekcia „Trasa nad minimapou" — štyri ovládacie prvky
  s popiskami, ktoré idú s hodnotou.
- **Koliesko myši nad úchytom overlayu** v režime úpravy rozloženia mení
  priehľadnosť vrstvy po 5 %, s bublinou s percentom. Z udalosti sa berie
  iba ZNAMIENKO — „rýchle" myši posielajú násobky a jedno švihnutie by inak
  skočilo cez celý rozsah. Nový príkaz `opacita_trasy_kolieskom`, nová
  udalosť `vzhlad-trasy` → CSS premenné `--trasa-opacita/-hrubka/-farba`.
- **Čiaru samotnú kreslí až krok 62/8** — tieto hodnoty vznikajú skôr
  zámerne, aby sa nemuseli dopĺňať do hotového kresliaceho kódu.
- Testy: **14 nových** v `core` (522 spolu, zelené) + **8 nových** e2e
  v `ui` (113 spolu, zelené). `cargo check` zelený v `core` aj `src-tauri`.
  **Appka sa nevydáva** — release je súčasť 62/8.

## core 0.13.24 — 20.10.2026 (TLI 59b2c: overenie presnosti CELÉHO plátna — NEGATÍVNY výsledok, bez zmeny správania, bez appka releasu)

Úloha `2026-10-20-0141-agent.md` (HNED, self-queued pokračovanie TLI 59b2b).
Zadanie pýtalo zapojenie lokálneho rozpoznania (`canvas_obsah` +
`canvas_sablony` + OCR) do appky, prepínač „lokálne/server" a release — ale
výslovne najprv žiadalo overiť, či to má pri nameranej presnosti zmysel, a
povedať narovinu, ak nie.

**Zmerané na rovnakých 13 reálnych screenshotoch/468 bunkách ako 59b2b**
(nezávislá Python reimplementácia rovnakých Rust vzorcov nad reálnymi
obrázkami, metodika a skript: `docs/fakty/hry/torchlight-infinite/README.md`
+ `scripts/games/tli-canvas-cele-platno-presnost.py` v repe `ops`):
obsah bunky (`Nedostupna`/`Prazdna`/`Namalovana`) správne na **celom** plátne
(všetkých 36 políčok naraz) len **5/13 (38,5 %)**; keď sa pridá aj typ ikony
monštra (`canvas_sablony`), celé plátno súhlasí len **1/13 (7,7 %)**.
**Kľúčový nález:** skóre istoty (histogram intersection) z `canvas_sablony`
NEODLIŠUJE správnu zhodu od nesprávnej na reálnych dátach (prekrývajúce sa
rozsahy skóre) — žiadny prah istoty teda problém nerieši, nie je to otázka
kalibrácie čísla.

**Rozhodnutie: NEROBIŤ zapojenie, prepínač ani release.** Appka by pri
7,7 % presnosti takmer vždy dávala hráčovi nesprávnu radu, a bez funkčného
prahu istoty by appka nevedela, kedy tomu neveriť. `canvas_sablony.rs`
doplnený o tento nález (nahrádza pôvodnú nádej, že prah istoty problém
vyrieši). **Žiadna funkčná zmena** — len doc-comment a verzia; patch bump
kvôli konvencii „každá dotknutá časť core = nová verzia".

Ďalší krok (ak sa k tomu Richard/agent niekedy vráti) je zapísaný
v `BACKLOG.md` (repo `ops`): template matching potrebuje inú črtu než farbu
celej bunky (maskovanie farebného prstenca okolo ikony, napr. len stred
výrezu alebo perceptuálny hash), inak zostáva pri 13 typoch prakticky na
úrovni náhody na reálnych dátach.

## core 0.13.23 — 20.10.2026 (TLI 59b2b: rozpoznanie obsahu bunky + template matching ikon, bez appka releasu)

Úloha `2026-10-20-0032-agent.md` (HNED, self-queued pokračovanie TLI 59b
časti 2a). Zadanie žiadalo body 2–3 z pôvodného zoznamu TLI 59: rozpoznanie
obsahu jednej bunky platna z farby, template matching ikon monštier a pokus
o reálne screenshoty na kalibráciu.

**Nový modul `core/src/canvas_obsah.rs`** — `rozpoznaj_obsah_bunky(rgb,
sirka, vyska, riadok, stlpec) -> Option<ObsahBunky>` (`Nedostupna` / `Prazdna`
/ `Namalovana`), kaskáda jas+sýtosť nad výrezom jednej bunky
(`canvas_rozlozenie` + `obraz::orez_box01`), prahy zmerané na 468 reálnych
bunkách z 13 screenshotov Frozen Canvasu (`/srv/games/tli/analyzy`, CT 114) —
**91,7 % celková presnosť** (96,2 % / 82,6 % / 80,0 % po kategóriách).
Zdokumentovaný dôležitý nález: pole `dostupna` zo servera/Gemini NIE JE
odvoditeľné len z farby jednej bunky (~3,4 % vizuálne identických prázdnych
buniek má `dostupna: false` — je to vlastnosť CELÉHO plátna/susedstva, rieši
`CanvasStav::volne_susedne_bunky`, nie pixely).

**Nový modul `core/src/canvas_sablony.rs`** — `rozpoznaj_ikonu_z_vyrezu`/
`rozpoznaj_ikonu` vracia `(typ, skóre_istoty)` z porovnania kvantizovaného 3D
RGB histogramu (6 binov/kanál, prienik) oproti 13 šablónam vygenerovaným
OFFLINE z reálnych ikon `/srv/games/tli/ikony/canvas/*.webp` skriptom
`scripts/games/tli-ikony-canvas-sablony-generuj.py` (repo `ops`) do
`canvas_sablony/canvas_sablony_data.rs` — appka naďalej nemá žiadnu závislosť
na dekódovaní obrázkov. Zmerané na 40 reálnych bunkách s monster paintom:
**37,5 % top-1 / 50,0 % top-3** presnosť (vyskúšané aj 4/5/7/8 binov, rôzne
orezanie okraja, separovateľný histogram — spoločný 3D histogram celej bunky
pri 6 binoch vyhral). Limit zapísaný v module aj v `BACKLOG.md`: farebný
prstenec okolo ikony sa medzi inštanciami rovnakého typu mení a histogram
celej bunky ho nevie oddeliť od samotnej príšery — vracia sa preto vždy aj
skóre istoty, nie len najlepšia zhoda.

**`canvas_ocr.rs` doplnený o `parsuj_level_badge(text) -> Option<i64>`** —
číta úrovňový odznak „LvN" jednej namaľovanej bunky (OCR výrez
`src-tauri/src/ocr.rs`), rovnaký robustný štýl ako `parsuj_max_level`.

core `508/508` testov zelených (17 nových). Testovacie fixtures (6 nových
`.rgb` výrezov skutočných buniek z reálnych screenshotov, `core/testy-data/`)
— **nedownscalované** na rozdiel od `canvas_detekcia`, lebo tu ide o detail
samotnej bunky (100×100 px), nie celej obrazovky.

Zámerne mimo rozsahu (self-queued ako TLI 59b2c): zapojenie do
`zber_canvas.rs`/`klient.rs`, prepínač „Canvas: lokálne/server", appka
release. Vyskúšané a odložené: rozpoznanie žltého „!" markera kľúčovej bunky
(`oznacena_vykricnikom`) jednoduchou farebnou heuristikou — na 13 pozitívnych
vzorkách sa neoddelilo od negatívov, zostáva `None` ako doteraz.

## core 0.13.22 — 20.10.2026 (TLI 59b, časť 2a: OCR scaffold + parsovanie textu cieľa/max levelu, bez appka releasu)

Úloha `2026-10-20-0005-agent.md` (HNED, self-queued pokračovanie TLI 59b
časti 1). Pôvodné zadanie žiadalo bod 1 „OCR (`Windows.Media.Ocr`) + parsuj
text cieľa a max levelu". Táto čiastková úloha rieši len bod 1 — body 2–5
(rozpoznanie obsahu bunky, template matching, prepínač lokálne/server,
overenie presnosti) sú príliš veľký kus na jeden beh a sú self-queued ako
TLI 59b2b, presne ako zadanie samo predpokladalo.

**Nový modul `core/src/canvas_ocr.rs`** — `parsuj_ciel_text(text) ->
Option<CielMiesania>` a `parsuj_max_level(text) -> Option<i64>`, čistý Rust
bez WinRT, **skutočne otestovaný na CT 111** nad vzormi textu odpísanými
priamo zo zadania TLI 59 („Fill a Adjacent Cell with a Monster Paint (0/3)",
„Current Max Paint Level7" — OCR bez medzery pred číslom je bežný artefakt,
keď je číslo v hre vo vlastnom farebnom rámiku hneď vedľa nápisu). Smer
(`ciel.smer`) sa odvodzuje LEN z overenej vety „Fill a `<X>` Cell …" — iný
tvar necháva smer zámerne prázdny (`""`) namiesto uhádnutia, solver prázdny
smer bezpečne odmietne. `kluc_riadok`/`kluc_stlpec` sa z textu neodvodia
nikdy (vizuálna vlastnosť bunky, rieši TLI 59b2b) a vždy vrátia `None`.
17 nových testov.

**Nový `core::obraz::orez_box01`** — čisté (bez Windows) vyrezanie Box01
obdĺžnika z RGB bufferu, používa rovnaký `Pole` tvar ako
`canvas_rozlozenie.rs`. Pripravené pre OCR len nad malým výrezom panelu, až
raz bude jeho geometria zmeraná (zatiaľ nie je — pozri `ocr.rs` nižšie).
5 nových testov. **`core` 491/491 testov zelených.**

**Nový `src-tauri/src/ocr.rs` (`#[cfg(windows)]`)** — `rozpoznaj_text`/
`rozpoznaj_text_oblasti` nad `Windows::Media::Ocr::OcrEngine`
(`TryCreateFromUserProfileLanguages` → `RecognizeAsync` → blokujúce `.get()`
→ `.Text()`), vstup je `SoftwareBitmap` zostavený z RGB cez `DataWriter`/
`IBuffer` (`Storage_Streams`). Nové featury kraty `windows`: `Media_Ocr`,
`Graphics_Imaging`, `Storage_Streams`. **API overené priamo zo zdroja
vygenerovaných bindingov `windows` 0.58.0** (nie z pamäte — poučenie z
TLI 40), ale **tento súbor sa na Linuxe nedá skompilovať ani spustiť** — je
celý za `#[cfg(windows)]`, overí ho až Windows CI (`Testy` workflow).
`cargo check` na `src-tauri` na CT 111 prebehol čisto (modul je mimo
Linux-cieľa, rovnaké zámerné rozhodnutie ako `canvas.rs`/`canvas_rozlozenie.rs`
predtým). **Modul zatiaľ nemá volajúceho** (rovnako ako `canvas.rs` v
TLI 59a) — `rozpoznaj_text` OCRuje zatiaľ celú snímku (funkčne správne,
`canvas_ocr` si vzor nájde kdekoľvek v texte), presná geometria panelu
cieľa/max levelu (analogická `canvas_rozlozenie.rs` pre mriežku) ešte nie je
zmeraná a je súčasťou TLI 59b2b.

## core 0.13.21 — 20.10.2026 (TLI 59b, časť 1: pevná geometria mriežky Frozen Canvasu, bez appka releasu)

Úloha `2026-10-19-2244-agent.md` (HNED, self-queued pokračovanie TLI 59a).
Pôvodné zadanie TLI 59 žiada pre lokálne rozpoznanie Frozen Canvasu „mriežku
6×6 na pevnom mieste obrazovky — prepočítaj podľa rozlíšenia a DPI (reuse
`suradnice.rs`)". Táto časť rieši presne a LEN geometriu (kam sa pozrieť),
nie samotné rozpoznanie obsahu (OCR, template matching) — to zostáva ako
ROZPRACOVANE, pozri `STAV.md`.

**Nový modul `core/src/canvas_rozlozenie.rs`** — Box01 pozícia všetkých 36
políčok platna a slotov ruky, odvodená VZORCOM (počiatok + pitch), nie 47
magickými číslami. **Hodnoty sú zmerané, nie odhadnuté:** priemer zo 14
reálnych rozpoznaní modulu `canvas` (Gemini vision) z `/srv/games/tli/analyzy`
na CT 114 (Richardova hracia session 19.10.2026) — smerodajná odchýlka
počiatku mriežky len ~0,0006–0,0007 Box01, teda Frozen Canvas UI sedí na
pevnom mieste vzhľadom na okno hry. **Limit zapísaný priamo v kóde:** zmerané
na jednom rozlíšení/okne, platnosť pri iných rozlíšeniach/DPI je zatiaľ len
pracovná hypotéza (Box01 by to malo preniesť, ak hra škáluje UI proporčne) —
treba overiť, keď appka raz zbiera dáta z iného rozlíšenia.
8 nových testov (36 políčok v správnom poradí, zhoda so zmeranými dátami pre
r1c1/r6c6/ruka slot 0, bunky sa pitchom neprekrývajú). `core` **470/470**
testov zelených. `src-tauri` `cargo check` čisto (modul zatiaľ nemá
volajúceho — rovnaké zámerné rozhodnutie ako pri TLI 59a).

**Vedľajší výsledok: 13 chýbajúcich ikon Monster Paint dotiahnutých.**
Zadanie žiadalo overiť, či `/srv/games/tli/ikony` má kompletnú sadu pre
všetkých 13 `MONSTER_PAINT_TYPY` pred nasadením template matchingu — **nemala
ANI JEDNU** (109 položiek v manifeste, nič k téme canvas/paint/monster typom).
Zdroj: `tlidb.com` (`The_Frozen_Canvas_Season`), CDN `cdn.tlidb.com` vyžaduje
`Referer: https://tlidb.com/` (bez neho 403) — zdokumentované v novom
`scripts/games/tli-ikony-canvas-stiahni.py`. Stiahnutých a vizuálne overených
13/13 (ratlin, dwarf, agito, pest, dragonbone, aember, test_subject, gang,
ichi, sand_monster, eumbra, einherjar, ghost), nasadené do
`/srv/games/tli/ikony/canvas/*.webp` + `manifest.json` na CT 114. Template
matching (krok 1, druhá polovica) ich teraz má k dispozícii — predtým by bol
nenasaditeľný úplne.

**Zámerne NEUROBENÉ v tejto časti** (self-queued pokračovanie
`2026-10-20-00xx-agent.md`): `Windows.Media.Ocr` volanie + parsovanie textu
cieľa/levelu (krok 1, OCR polovica), rozpoznanie obsahu bunky z farby/ikony
(prázdna/paint/monštrum s levelom/zamrznutá), prepínač „Canvas: lokálne/
server" (krok 3), presnosť voči `/srv/games/tli/analyzy` (krok 4), appka
release a hlásenie do `#torchlight`. Táto geometria je predpoklad pre
všetky — bez nej by sa OCR/template matching museli hádať, kam sa vôbec
pozrieť.

## 0.23.0 (core 0.13.20) — 19.10.2026 (TLI 61 — FE/min, Reset session na overlayi, automatický reset po nečinnosti)

Úloha `2026-10-19-2241-mcp.md` (HNED, MALÁ, Opus, Gemini nesmel písať kód).
Richard 19.10.2026 ~21:40: „potrebujem na overlayi aj FE/min a reset session,
že ťuknem a začne odznova, lebo to ukazuje kokotiny." Na screenshote overlay
ukazoval SESSION ČAS 1173 m 41 s (~19,5 h) — session sa evidentne počítala
cez viac dní a reštartov appky, nie dnešné hranie.

**Prečo vyšlo 1173 min (zadanie bod 4 — vyšetrenie).** `SESSION ČAS` už aj
predtým počítal presne to, čo TITrack — súčet `trvanie_ms` len nehubových
behov (`Prehlad.celkom.cas_ms` v `core/src/tracker/agregacie.rs`, hubové behy
`ocen_beh` vracia `None`). Chyba teda nebola vo vzorci, ale v tom, že od TLI 56
(spätné načítanie behov z lokálnej DB pri každom štarte appky) sa `celkom`
sčítaval cez VŠETKY uložené mapy od posledného ručného kliknutia na „Reset
Stats" — a to tlačidlo bolo len v hlavnom okne, žiadne pripomenutie ani
automatika ho nespúšťali. Bez automatického resetu číslo rástlo donekonečna
cez reštarty aj naprieč dňami.

**Oprava — automatický reset po 30 min nečinnosti (bod 3).** Appka si
v `tracker_meta.posledna_aktivita_unix` drží čas POSLEDNÉHO živého riadku
v hernom logu (nie dobiehanie starej histórie po štarte appky — to sa
nepočíta). Pri každom tiku čítania logu (`precitaj`, volá ho `tracker_prehlad`
pri každom polli UI) appka porovná „teraz" s touto značkou — keď je medzera
väčšia než 30 minút, zavolá presne tú istú funkciu ako ručné tlačidlo
(`StavTrackera::resetuj_zive_staty`). Pokrýva to „štart appky" AJ „štart hry"
naraz bez toho, aby appka musela osobitne sledovať proces hry: obe situácie sa
prejavia ako dlhá medzera od poslednej živej aktivity. Poistka
`!self.behy.is_empty()` zaručuje, že sa to nespustí opakovane na každý tik
počas dlhšej nečinnosti (po prvom vyprázdnení už nie je čo čistiť).

**Tlačidlo „⟲ Reset session" priamo na overlayi (bod 2).** V pásiku úchytu
mini-panela (`ui/mini.html`/`ui/mini.js`), klikateľné bez Ctrl+F8 — rovnaký
pásik, ktorý sa dá ťahať aj mimo režimu úpravy (TLI 48). Namiesto natívneho
`confirm()` dialógu (ten by appke nad hrou zobral fokus) treba tlačidlo
PODRŽAŤ 1 sekundu — vizuálna výplň ukáže postup, pustenie pred limitom nič
nespraví. Volá ten istý príkaz `tracker_reset_zive_staty`, aký používa „Reset
Stats" v hlavnom okne (bod 5 zadania — obe okná teda zdieľajú jednu
backendovú cestu, nič sa neduplikuje). Po resete sa pod session riadkom ukáže
„session od HH:MM" (nové pole `session_od_unix` v `PrehladProUi`, z
`tracker_meta.stat_reset_unix`) — viditeľné potvrdenie, že appka naozaj počíta
odznova.

**FE/MIN vedľa FE/h (bod 1).** Pridané pre aktuálnu mapu aj pre session,
rovnaký výpočet ako FE/h, len za minútu. Mini-panel je teraz 4-stĺpcový grid
namiesto 3-stĺpcového (zisk/čas/FE-h/FE-min), rovnaké zalomenie pri úzkom
okne ako doteraz.

Testy: `src-tauri` nové jednotkové testy pre automatický reset
(`tracker::testy::automaticky_reset_*`, `cargo check --tests` zelené — `cargo
test` sa v `src-tauri/` na tomto hoste nespúšťa, len na Windows CI,
[[tli-cargo-test-disk]]), `core` 462+4+8 testov bez zmeny (core sa v tomto
tiketu nemenil), `ui` nový `testy-mini-reset-session.e2e.mjs` (6 testov: FE/min,
„session od", podržanie/pustenie tlačidla, stopPropagation proti ťahaniu
okna) + všetkých 105 pôvodných JS testov zelených.

Vedome NEUROBENÉ: vlastné sledovanie OS procesu hry („štart hry" ako
samostatná udalosť) — nahradené jednoduchším a spoľahlivejším sledovaním
nečinnosti v logu, ktoré pokrýva rovnaký prípad bez ďalšieho natívneho API.

## 0.22.0 (core 0.13.20) — 19.10.2026 (TLI 60 — Frozen Canvas: automatické učenie na pozadí)

Úloha `2026-10-19-2135-mcp-2.md` (HNED, Opus, Gemini nesmel písať kód).
Richard 19.10.2026: „kľudne nech si všetko, čo spravím, fotí a na pozadí pošle
agentovi na učenie, a na konci dňa nech sa naučí."

**Čo appka odteraz robí sama.** Keď je v hre otvorený Frozen Canvas, po KAŽDOM
kliku myšou odfotí okno hry (300 ms po kliku), snímku odloží do fronty na disku
a na pozadí ju pošle na `?modul=canvas&zber=1`. Nič neukazuje, na nič nečaká,
nijako nezdrží hráča — jediná stopa je denník appky. Prepínač **„Učenie Canvas
na pozadí"** v Nastaveniach je **predvolene ZAPNUTÝ** (zadanie) a je to **iné
pole než „Zber dát"** (`zber_dat_zapnute`), ktorý ostáva ručný, predvolene
vypnutý a mení, čo appka vykreslí v rade. Dve nezávislé veci, nezlievajú sa.

**Ako appka pozná, že je Canvas otvorený, keď TLI 59 ešte nie je hotová.**
Zadanie pripúšťalo oboje — „podľa mriežky z TLI 59, **prípadne podľa pomeru
farieb**". Lokálne rozpoznanie mriežky (TLI 59b) ešte neexistuje, takže je to
pomer farieb: `core/src/canvas_detekcia.rs` vzorkuje 192×108 bodov a počíta dve
veličiny — podiel „ľadových" bodov v celom okne a podiel modrastých bodov vo
výreze, kde na Canvase leží mriežka 6×6. **Prahy nie sú odhad:** odmerali sa na
309 reálnych snímkach z `/srv/games/tli/analyzy` (17 Canvas, 292 ostatných).
Canvas má `ladovy` ≥ 0,102 a `mriezka_modra` ≥ 0,797; zo zvyšných 292 snímok má
pri modrej mriežke `ladovy` nanajvýš 0,021 a pri ľadovom nádychu `mriezka_modra`
nanajvýš 0,123. Prahy 0,07 a 0,65 ležia hlboko v tej medzere a **obe podmienky
musia platiť naraz** — samotná modrá by chytila obrazovku odmien, samotný ľadový
nádych zasa zasneženú mapu. Na tej sade: **17/17 správne, 0/292 falošne kladných.**
Šesť vzoriek (vrátane oboch najtesnejších protipríkladov) je v `core/testy-data/`
ako surové RGB a testy v CI počítajú tie isté čísla.

**Čo appka zámerne NEFOTÍ** — keď je prepínač vypnutý; keď je vyčerpaný denný
strop 500 snímok; častejšie než raz za 250 ms v Canvase a raz za 2 s mimo neho
(aby sa pri klikaní v boji nefotilo pri každom kliku); **keď okno hry nie je
v popredí** (inak by snímka zachytila prehliadač alebo rozhovor, ktorý má hráč
cez hru); a keď nad hrou práve svieti vlastný panel appky alebo beží ručná
analýza (na snímke by bola rada, nie čistá obrazovka).

**Fronta.** Zmenšený JPEG 1280 px / q75 (menej než 1920 pri ručnej analýze —
tieto snímky idú do nočného učenia, nie do vision modelu na radu). Súbory ležia
v `canvas-zber/` vedľa nastavení, zapisujú sa cez `.tmp` + premenovanie a
odosielajú sa v poradí vzniku. Keď server nie je dostupný, odstup rastie 5 s →
10 s → … → 5 min a snímky čakajú ďalej. **Nič sa nemaže:** pri 1000 čakajúcich
sa len prestane pridávať; jediný súbor, ktorý zo spoolu mizne, je ten, ktorý
server práve prijal a trvalo archivoval.

**Čo pribudlo v kóde:** `core/src/canvas_detekcia.rs` (detekcia + 9 testov nad
reálnymi vzorkami), `core/src/zber_fronta.rs` (denný strop s prechodom cez
polnoc, odstupy, exponenciálny retry, mená súborov — 13 testov),
`src-tauri/src/zber_canvas.rs` (dve vlákna: sledovač kliku a odosielač),
`klient::posli_canvas_zber`, nastavenie `ucenie_canvas_zapnute`,
`Nastavenia::adresa_analyza_canvas_zber()`, prepínač v `ui/nastavenia.html/js`
a `ui/testy-nastavenia-ucenie-canvas.e2e.mjs` (4 jsdom testy — predvolené
zapnutie musí prežiť aj staré `nastavenia.json` bez toho poľa).

**Serverová strana (repo `ops`, nie tu):** `/api/analyza?modul=canvas&zber=1`
bol pripravený už 19.10. večer; `scripts/games/tli-canvas-ucenie.py` odvodzuje
pravidlá aj pre ostatné smery miešania a súhrn chodí do `#torchlight` ráno.

**Rollback:** prepínač „Učenie Canvas na pozadí" v Nastaveniach (vypnutie
zastaví fotenie aj odosielanie okamžite, fronta na disku ostane) alebo
predchádzajúci release `tli-v0.21.3`.

## core 0.13.19 — 19.10.2026 (TLI 59a — solver „adjacent" prenesený do core Rust, ZATIAĽ BEZ RELEASU appky)

Úloha `2026-10-19-2135-mcp.md` (HNED, Opus, Gemini nesmel písať kód, limit 45 min).
Zadanie malo 4 body (lokálne rozpoznanie mriežky+OCR+template matching, port
solvera, prepnutie lokálne/server, overenie presnosti na reálnych snímkach) —
do 45 min sa zmestil len bod 2, zvyšok (bod 1, 3, 4) pokračuje ako **TLI 59b**
(self-queued, pozri `ROZPRACOVANE` v `STAV.md` repa `ops`).

**Čo pribudlo:** `core/src/canvas.rs` (dátový model plátna — `CanvasStav`,
`Bunka`, `SlotRuky`, `CielMiesania`, `Paint`, `volne_susedne_bunky`) a
`core/src/canvas_solver.rs` (`navrhni_kroky`) — riadok-po-riadku port
`canvas_model.py`/`canvas_solver.py` z `stacks/games/tli/engine` (repo `ops`).
Implementovaný je len smer `adjacent` (jediný overený), presne ako v Pythone.
12 nových testov na TOM ISTOM fixture ako `testy-canvas-solver.py` (reálny
screenshot 15.10.2026) + ručne porovnaný JSON výstup oboch solverov na
rovnakom vstupe — kroky, poradie, texty `dovod` aj `istota` vyšli identicky.

**ZÁMERNE BEZ appka release a BEZ appka version bumpu (`0.21.3` ostáva).**
Modul nemá volajúceho v `src-tauri/` — lokálne rozpoznanie mriežky/ruky (bod 1
zadania), ktoré by `CanvasStav` napĺňalo, ešte neexistuje (čaká na TLI 59b).
Vydať appku s mŕtvym, nikým nevolaným kódom by znamenalo CI/MSI/redeploy bez
jedinej zmeny pre hráča — rovnaký dôvod, prečo TLI 5 krok 1 (server endpoint
`/api/sablony`) tiež nešiel von appka-releasom. Release príde s TLI 59b, keď
bude mať appka čo lokálne spočítať. `cargo test` v `core/` (435/435 OK vrátane
2 existujúcich e2e sád), `cargo check` v `src-tauri/` čisto (bez nových
warningov). Žiadna správa do `#torchlight` — tá podľa zadania patrí k reálne
nameranému zrýchleniu rady, ktoré ešte nie je dosiahnuté.

## 0.21.3 — 19.10.2026 (TLI 58 — rada zhasne až po kliku myšou, nie po čase)

Úloha `2026-10-19-2134-mcp.md` (HNED, 30 min). Core `0.13.17` → `0.13.18`
(nové nastavenie `skryt_radu_klikom`).

Richard 19.10.2026: „po stlačení solvera nech mi svieti overlay, kým nespravím
klik myškou, až potom zhasne, namiesto času."

1. **Panel rady a zvýraznenia sa po zobrazení NEskrývajú časovačom.** Zhasnú
   až pri PRVOM kliku ľavým alebo pravým tlačidlom myši kdekoľvek na
   obrazovke. Platí pre oba moduly rovnako (Tartarus aj Frozen Canvas) aj pre
   „Test overlaya" z tray menu — ten má ukázať to, čo hráč naozaj dostane,
   nie vlastný desaťsekundový režim.
2. **Klik ide do hry ďalej a nič sa neblokuje.** Overlay ostáva click-through
   (`set_ignore_cursor_events(true)`, nezmenené) a stav tlačidiel sa len
   ČÍTA — `GetAsyncKeyState` á 25 ms vo vlastnom vlákne. Zadanie pripúšťalo aj
   globálny low-level hook myši; zvolený je polling, lebo hook sa inštaluje
   funkciou, ktorú CI krok „Pravidlá ToS" zakazuje menovite spolu so
   simuláciou vstupu, a navyše ho Windows pri prekročení
   `LowLevelHooksTimeout` ticho odpojí. Polling do cesty kliku nevstupuje
   vôbec. `src-tauri/src/main.rs` (`naplanuj_skrytie_po_kliku`).
   **Pozor na jednu vec:** overlay sa často zobrazí PRÁVE počas kliku (tray
   menu „Odfotiť teraz", držané tlačidlo z hry). Preto sa nečaká na „tlačidlo
   je dole", ale na skutočný nový stlak — automat `krok_cakania_na_klik` najprv
   počká, kým sú obe tlačidlá hore.
3. **Dve poistky.** Panel zhasne aj po **120 s** bez kliku
   (`SKRYTIE_KLIKOM_POISTKA_S`) a vždy pri **Esc** (nezmenené správanie
   skratky). Poistka je zámerne pevná, nie `autoskrytie_s` z nastavení — tam
   smie byť nula („nikdy sám"), a to by pri zlyhanej detekcii kliku nechalo
   panel svietiť navždy.
4. **Nastavenie „Skryť radu: po kliku myšou (predvolené) / po čase".** Pri
   režime kliku je pole „Automatické skrytie (sekundy)" zašednuté a
   zablokované, nech hráč nehľadá, prečo sa číslom nič nemení. Staré
   `nastavenia.json` pole `skryt_radu_klikom` nemajú — `#[serde(default)]`
   im dosadí `true`, takže po aktualizácii sa nová funkcia zapne každému
   (overené testom `stary_subor_bez_pola_dostane_rezim_kliku`).
5. **Denník rozlišuje dôvod zhasnutia:** `[zobraz] skryté (dôvod=klik|esc|
   timeout|hotkey|ui)`. Doteraz sa skrytie do logu nezapisovalo vôbec, takže
   sa pri hlásení „zmizlo mi to samo" nedalo rozlíšiť, čo panel zhaslo.

Testy: `krok_cakania_na_klik` (6 nových v `src-tauri/src/main.rs`, vrátane
scenára „tlačidlo držané v okamihu zobrazenia"), 2 nové v
`core/src/nastavenia.rs`, 4 nové jsdom testy
`ui/testy-nastavenia-skrytie.e2e.mjs`.

## 0.21.2 — 19.10.2026 (TLI 57 — Zber dát neblokuje radu, sort toast menej otravný)

Úloha `2026-10-19-2113-mcp.md` (HNED, 30 min). Core `0.13.16` → `0.13.17`
(`SledovacVarovania` dostal nastaviteľný interval).

Tri malé nahlásené problémy z Richardovho reálneho hrania (v0.21.1, 19.10. ~20:00):

1. **Zber dát (Frozen Canvas) už neblokuje radu.** Keď bolo zapnuté nastavenie
   „Zber dát", appka predtým ukázala LEN „Zber dát — uložené #N" a radu
   solvera vôbec nevykreslila — Richard to vnímal ako „appka vôbec
   nerozpoznáva, čo má robiť". Server stav ukladá vždy, nezávisle od toho,
   či sa rada zobrazí, takže appka teraz robí OBOJE naraz: počíta uložené
   snímky A ZÁROVEŇ vykreslí kroky z tej istej odpovede (`zber_pocet` v novom
   poli odpovede `hotovo_canvas`, fáza `zber_ulozene` zrušená — bola to jediná
   vetva, ktorá ju posielala). `src-tauri/src/main.rs`, `ui/overlay.js`,
   `ui/nastavenia.html` (popis nastavenia).
2. **„Frozen Canvas — nerozpoznané".** Keď solver nemá žiadny krok PRETOŽE
   vision model vôbec nerozpoznal cieľ plátna (`analyza.platno.ciel` chýba —
   typicky zlý screenshot/iné okno), panel to teraz povie rovno: „Nerozpoznal
   som plátno – otvor Frozen Canvas na celú obrazovku." Predtým sa v tomto
   prípade ukázala tá istá všeobecná veta ako pri „cieľ už splnený", čo
   vyzeralo ako ticho. `ui/overlay.js`.
3. **Toast „Sortni inventár v hre" sa už neopakuje donekonečna á 25 s.** Má
   vlastný, oveľa dlhší interval obnovy (`OPAKOVAT_PO_S_SORT` = 600 s,
   `core/src/tracker/varovania.rs`) a navyše zmizne HNEĎ pri prvom
   `InitBagData` po probléme (`skry_pripomienku_ak_text` v `main.rs`), nie až
   po svojom vlastnom časovači. Toast uploadu (`OPAKOVAT_PO_S` = 25 s) sa
   nemení — rieši reálnu stratu dát, nie len neúplný Net Worth.

Meranie latencie `/api/analyza?modul=canvas` (bod 3 zadania, 18 s nahlásených
Richardom): živé meranie v tomto behu nebolo spravené (žiadny snapshot po
ruke, hrozilo by skutočné volanie Vertexu mimo rozpočtu MALEJ úlohy). Z kódu
(`stacks/games/tli-web/app.py` v `ops` repe): `VISION_PROMPT_CANVAS` (627 tok.)
je KRATŠÍ než `VISION_PROMPT` pre Tartarus (1208 tok.), takže vstupný prompt
nie je príčina. Canvas je ale JEDINÝ modul, ktorý si od vision modelu pýta
pixelové súradnice (`box`) pre VŠETKÝCH 36 buniek plátna (6×6) AJ pre každý
slot v ruke naraz (`polia_platno`/`polia_ruky`) — to je rádovo väčší
OUTPUT, ktorý musí model vygenerovať, než pri Tartarus. Odhad: väčšina 18 s
ide na generovanie tohto veľkého štruktúrovaného JSONu vo vision modeli, nie
na sieť/JPEG (zdieľané s Tartarus cez tú istú `zmensenie::MAX_DLZKA`) ani na
solver (`canvas_solver.navrhni_kroky` je čisté počítanie nad už hotovým
stavom, rádovo ms). Rýchla oprava (menší JPEG/iný model) sa v tomto behu
NEROBILA — vyžadovalo by zmenu kontraktu API (`canvas-api-kontrakt.md`) a
testovanie proti reálnym screenshotom, čo presahuje rámec MALEJ úlohy.
**Návrh na ďalší beh:** počítať súradnice 36 buniek/slotov ANALYTICKY z pevnej
mriežky (fixný layout obrazovky) namiesto toho, aby ich vracal vision model —
ten by sa pýtal len na OBSAH buniek (paint/dostupná) a 1–2 kotviace súradnice
plátna, čím by sa output zmenšil rádovo. Druhá, menej istá možnosť: vlastný
(rýchlejší) model len pre `modul=canvas` cez nové pole v `gemini:` configu
(dnes jeden `model` pre všetky moduly) — treba overiť na reálnych dátach, či
nestráca presnosť.

## 0.21.1 — 19.10.2026 (TLI 54b — Supreme potvrdené: 6. stupeň rarity, nie 5)

Úloha `2026-10-19-1731-agent.md` (HNED, self-queued pokračovanie TLI 54). Core
sa nemenil (`0.13.16`) — čisto UI zmena.

TLI 9/9b (15.10.2026) zlúčilo stupne Supreme a Rainbow do jedného najvyššieho
„chromatic" na základe preview videa spred štartu sezóny. Táto úloha overila
hypotézu na ĎALŠOM materiáli (561 uložených analýz z CT 114 za 16.–19.10.2026,
nielen na pôvodnej snímke TLI 54) a potvrdila, že živá hra má 6 stupňov:
Richardove reálne screenshoty ukazujú VÝRAZNE ČERVENÚ (crimson) auru väzňa na
viacerých samostatných snímkach (3 session cez 2 dni), vizuálne jasne
odlišnú od oranžovej (Orange/Legendary) aj od skutočnej dúhovej animácie
Chromatic (panel „Chromatic Prisoner"). Engine navyše vlastným pravidlom
garantuje, že skutočný najvyšší stupeň nikdy nezostáva na doske — čokoľvek
tam sedí hodiny, nemôže byť Chromatic, musí byť stupeň medzi Orange a
Chromatic. Plný dôkaz: `docs/fakty/hry/torchlight-infinite/tartarus-pravidla.md`
(ops repo) § Aktualizácia 19.10.2026 (TLI 54b).

**`ui/ikony.js`:** `TIER_FARBA`/`TIER_SK` rozšírené na 6 položiek (pridaná
položka „Červený"/`#e8303f` medzi Oranžový a Chromatický). Server-side
(engine, `tli-web`) migrácia je v `ops` repe, nie tu — tento release len
zosúlaďuje farby/mená v paneli appky s novým 6-stupňovým modelom.

## 0.21.0 — 19.10.2026 (TLI 55 — loot overlay počas mapy: živý ČAS/FE-h, session súhrn, preklad názvu mapy)

Úloha `2026-10-19-1715-mcp.md` (HNED, zaradené za TLI 53/56). Core sa nemenil
(`0.13.16`).

Richardov screenshot 19.10.2026 z bežiacej mapy: nadpis ukazoval surové
interné meno enginu („SD_DuiLongJuQiang211"), ZISK bol správne 19,15 FE, ale
ČAS aj FE/h ukazovali „—".

**Koreňová príčina (bod 3 zadania preveril, nie je to chýbajúci
`zaciatok_cas`):** `Beh::trvanie_ms()` (`core/src/tracker/stav.rs`) vracia
`None`/`0`, kým mapa beží — potrebuje aj `koniec`, ktorý má len UZAVRETÝ beh.
`ZISK` (`cisty_zisk`) na trvaní nezávisí, preto bol vidieť správne; ČAS a
FE/h z neho priamo vychádzajú, preto „—". Overené priamo na Richardovom
uploadnutom `tracker.db` z CT 114 (372 behov k 18.10.2026): **0 chýbajúcich
`zaciatok_cas`/`trvanie_ms`** — žiadna historická diera na dopočítanie,
appka si tie časy vždy správne zapísala. Oprava preto nemenila `core` ani
históriu, len živé zobrazenie.

**Oprava (`src-tauri/src/tracker.rs`):**
- Nové pole `StavTrackera::zivy_zaciatok_real: Option<(beh_id, unix_cas)>`
  — zachytí SKUTOČNÝ wall-clock čas (`unixovy_cas()`), odkedy appka vidí
  AKTUÁLNE bežiaci beh. Zámerne NIE je to log-ový `beh.zaciatok` porovnaný
  s `unixovy_cas()` — log píše LOKÁLNY čas, `unixovy_cas()` je skutočné UTC,
  rozdiel časových pásiem by živý čas posunul o celé hodiny.
- `prehlad_pre_ui`: keď je `trvanie_ms` z `ocen_beh` `0` (živá mapa), nahradí
  sa `teraz - zivy_zaciatok_real`. `posledna_mapa` (uzavretý beh) sa
  NEDOTÝKA — tam je `trvanie_ms()` z logu presné.
- Known limitation (zdokumentované v kóde): appka spustená UPROSTRED už
  bežiacej mapy (dobiehanie logu) započíta čas od spustenia appky, nie od
  skutočného začiatku mapy.
- 4 nové testy (`mod testy` v `tracker.rs`): živý čas rastie z nuly,
  nezačína odznova pri ďalšom riadku tej istej mapy, uzavretá mapa si drží
  presné trvanie z logu nezávisle od živého odhadu.

**UI (`ui/mini.js`/`.html`/`.css`):**
- `nazovMapy()` (duplikát rovnakej logiky z `ui/hlavne.js` — mini-panel je
  samostatné okno bez zdieľaného JS modulu, rovnaký vzor ako `cas()`/`fe()`)
  nahrádza predošlé `analyzujCestuMapy`, ktoré vracalo surový posledný kus
  cesty. Preloží overené pinyin kódy (`PREKLAD_MAPY`), inak aspoň rozdelí na
  čitateľné slová — nikdy nie surovú internú cestu.
- Po skončení mapy ostávajú ZISK/ČAS/FE-h z POSLEDNEJ mapy (nie „—"), rovnaký
  princíp ako „čo padlo" (TLI 25); úplne nová appka bez jedinej odohranej
  mapy ďalej ukazuje „—".
- Nový riadok „session zisk / session čas / session FE/h" pod hlavnými
  tromi číslami — rovnaké súhrnné čísla ako RUNS/TOTAL TIME/FE-HOUR na
  hlavnom dashboarde (`prehlad.celkom`, teda od posledného „Reset Stats"),
  presne ako to ukazuje TITrack overlay (zadanie bod 2).
- Samostatný 1s ticker (`tiknutie()`) dopočíta ČAS/FE-h medzi dvoma 2s
  pollami backendu z vlastného `Date.now()` — appka teda naozaj tiká každú
  sekundu, nie len raz za 2 s. `obnov()` ostáva na 2s intervale (IPC sa
  nezvyšuje).
- Mini okno zväčšené z 220 na 265 px výšky (`tauri.conf.json`,
  `ui/nahlad-generuj.py`), nech sa nový session riadok zmestí bez toho, aby
  stlačil zoznam dropov na nič.
- 6 nových jsdom testov (`ui/testy-mini-ziva-mapa.e2e.mjs`), zaradené do
  `ui/package.json` → `test`.

**Vedome NEurobené:** TLI 54b (6. stupeň rarity Supreme, `stacks/games/tli`
engine) — iný systém, má vlastnú úlohu v `inbox/`.

## 0.20.9 — 19.10.2026 (TLI 56 — akceptačná kontrola: Current Inventory ako záložka, NET WORTH a Recent Runs prežívajú reštart)

Úloha `2026-10-19-1740-mcp.md` (HNED, Opus). core `0.13.16`.

**Skutočný nájdený regres, nie len kozmetika.** Akceptačná kontrola appky
`0.20.8` priamo na Richardovom PC 19.10.2026 ~17:38 ukázala: Current Inventory
sa v záložke Tracker nedalo nájsť (bolo úplne dole pod Históriou máp), NET
WORTH ukazovalo podozrivo nízkych 191 FE a RUNS/Recent Runs/FE-HOUR boli po
celodennom hraní prázdne, akoby appka nikdy nebežala.

**Koreňová príčina (spoločná pre NET WORTH aj RUNS):** appka od `StavTrackera::
default()` žila výhradne z `chvost.pozicia` v živom logu — tá sa pri reštarte
appky NEVRACIA na začiatok (správne, inak by appka log čítala odznova pri
každom spustení), takže riadky, ktoré hra napísala PRED týmto reštartom (FE
zostatok pri prihlásení, materiály po poslednom Sorte, dokončené mapy), appka
už nikdy neuvidí. `cennik` (ceny) mal presne tento istý problém vyriešený už
od TLI 20 („OFFLINE CACHE“, cenník sa pri štarte načíta z DB) — `behy`
(RUNS) a obsah batohu (Current Inventory/NET WORTH) túto ochranu nikdy
nedostali.

**Oprava — rovnaká filozofia ako TLI 20, rozšírená na zvyšok stavu appky:**
- Nová tabuľka `batoh_snapshot` (per stránka 101/102/103) — appka po KAŽDOM
  kole, keď na stránke živo niečo vidí, uloží jej obsah; keď stránku v tejto
  session ešte nevidela, siahne po poslednej uloženej snímke namiesto
  prázdna/čiastočného súčtu. UI ukáže „stav z HH:MM“, keď použila cache.
- `nacitaj_zive_behy` (core `db.rs`) spätne postaví `Vec<Beh>` z tabuliek
  `behy`+`polozky` a appka ním pri štarte naplní `stav.behy` — RUNS/Recent
  Runs/FE-HOUR/AVG/TOTAL TIME po reštarte UŽ NEZMIZNÚ.
- „Reset Stats“ doteraz mazal len pamäť appky (prežilo by to len do ďalšieho
  reštartu, ktorý by to znova naplnil z DB) — nová tabuľka `tracker_meta`
  (`stat_reset_unix`) drží posledný reset trvalo, `nacitaj_zive_behy` ho
  rešpektuje.
- Current Inventory presunuté z dna záložky Tracker do VLASTNEJ záložky
  „Inventory“ v hlavnom menu (zadanie bod 1, druhá ponúkaná možnosť) — nad
  tabuľkou pribudol súčet hodnoty, triedenie funguje aj kliknutím na
  hlavičku stĺpca (meno/počet/cena/hodnota), nielen cez `<select>`.
- `#[cfg(not(test))]` na oboch nových DB prepojeniach (`StavTrackera::
  default`, `mnozstva_s_cachou`) — `cesta_db_suboru()` nie je v testoch
  izolovaná (jeden zdieľaný súbor pre všetky testy v builde), takže spätné
  načítanie by v builde pre testy spôsobilo nedeterministickú krížovú
  kontamináciu medzi testami. Samotná logika (`nacitaj_zive_behy`,
  `uloz_batoh_snapshot`/`nacitaj_batoh_snapshot`) má vlastné testy nad
  izolovanou `otvor_v_pamati()` DB.

**TLI 53 (ikony všade) overené ako už HOTOVÉ** — Economy/Current Inventory/
Recent Runs/loot mini-panel mali offline ikony už od `0.20.7`/`0.20.8`,
zadanie to len žiadalo potvrdiť.

**Zámerne NEurobené v tomto behu** (vyžadujú samostatnú pozornosť, obe majú
vlastnú úlohu už v `inbox/`): **TLI 55** (živý čas a FE/h počas bežiacej
mapy — koreňová príčina je iná, `Beh::trvanie_ms()` vracia `None`, kým mapa
nemá `koniec`, treba sledovať reálny systémový čas štartu mapy, nie čas z
logu) a **TLI 54b** (6. stupeň rarity Tartarus/Supreme — zásah do
`stacks/games/tli` enginu, nie do tejto appky). Tichá aktualizácia bez UAC
(TLI 52 zvyšok) sa NEzapína naživo — stále čaká na Richardovo osobné
vyskúšanie NSIS inštalátora, presne ako zapísané v `STAV.md`.

## 0.20.8 — 19.10.2026 (TLI 53 časť 2/N doplnok — zmenšené ikony, CI overenie štartu padalo)

Úloha `2026-10-19-1537-agent.md` (to isté zadanie, bezprostredné pokračovanie
0.20.7). core bez zmeny.

**Skutočný nájdený regres, nie kozmetika.** `tli-v0.20.7` (nižšie) pridal
30 MB ikon (256×256 px originály z TITracku) a MSI vyrástlo
6,3 MB → 36,4 MB. `verify-install.yml` nad `tli-v0.20.7` **DVAKRÁT za sebou
zlyhal** na kroku „Appka sa po inštalácii naozaj spustí" — appka sa
nainštalovala aj spustila (log mal `[start] … spustené`), ale do 10 s
nestihla dokončiť registráciu klávesových skratiek (`registrácia OK` sa
v denníku vôbec neobjavilo). Appka nemá konzolu (`windows_subsystem =
"windows"`), takže presnú príčinu (pravdepodobne čas na rozbalenie/
spracovanie oveľa väčšieho vstavaného balíka assetov vo `WebView2` pred
tým, než `setup()` vôbec začne) sa nedalo vidieť priamo — zmenšenie
payloadu je ale lacnejšia a bezpečnejšia oprava než hĺbať v internom
mechanizme Tauri bundlera.

**Oprava:** appka ikony nikdy nekreslí väčšie než 18×18 px (CSS
`.economy-ikona`/`.inventar-ikona`/`.odznak-ikona`/`.dropy .ikona`) — 256 px
zdroj bol teda ~14× väčší, než appka vôbec kedy zobrazí. Všetkých 1 859
súborov v `ui/ikony/polozky/` prevzorkovaných na **48×48 px** (2,67×
rezerva nad 18 px pre HiDPI, nie 1:1) cez Pillow (`Image.LANCZOS` +
WEBP kvalita 85) — vizuálne nerozoznateľné pri skutočnej veľkosti
zobrazenia, súbory **30,0 MB → 3,66 MB** (takmer 8×). MSI by sa touto
zmenou malo vrátiť na rádovo jednotky MB nad pôvodných 6,3 MB, nie
desiatky.

`verify-install.yml` spustený znova nad `tli-v0.20.8` po tejto oprave —
výsledok v `JOURNAL.md` 19.10.2026.

## 0.20.7 — 19.10.2026 (TLI 53 časť 2/N — offline ikony položiek, oprava skutočnej CSP chyby)

Úloha `2026-10-19-1537-agent.md` (HNED, self-queued pokračovanie TLI 53 časti
1/N, nedozorovaný beh, trieda `zasah`). core bez zmeny (0.13.15) — zmena je
čisto v `ui/`, žiadny Rust súbor sa nemenil.

**Skutočný nájdený bug, nie len vizuálne vylepšenie.** Appka od TLI 48
ťahala ikony položiek v Economy aj v loot overlay mini-paneli ako
`<img src="${url_servera}/ikony/polozka/${id}">` — teda z INÉHO pôvodu než
appka sama (herný server `tli.tvojtatko.sk`). Appkina CSP je
`img-src 'self' data:` (rovnaký dôvod, prečo Guide, TLI 11, posiela ikony
ako `data:` URI, JOURNAL.md 15.10.2026) — tie requesty teda CSP ticho
blokovala a `onerror="this.remove()"` zakaždým len schoval prázdny
obrázok. Ikony sa v Economy ani v mini-paneli **nikdy nezobrazili**, odkedy
boli zadané (TLI 48, 19.10.2026).

**Oprava — ikony sú teraz OFFLINE súčasťou appky.** Zdroj: TITrackov
`tlidb_items_seed_en.json` (MIT, `github.com/astockman99/TITrack`), pole
`img`. Stiahnutých **1 859 unikátnych** ikon (33 MB, 27 chýba 404 na CDN —
nevydané/placeholder položky budúcich sezón, rovnaký stav ako chýbajúce
mená) do `ui/ikony/polozky/<subor>.webp`, nový `ui/polozky-ikony.json`
mapuje `id -> relatívna cesta` (2 161 id, viaceré zdieľajú súbor). Súbory
idú do appky cez `frontendDist` (`../ui`), takže `<img src="ikony/polozky/
…webp">` je pre appkinu CSP `'self'` — presne dôvod, prečo sa toto (na
rozdiel od predošlej verzie) naozaj vykreslí.

Nový spoločný helper `ikonaPolozkyHtml(id, trieda)` (`ui/hlavne.js`)
nahrádza mŕtvy `ZAKLAD_IKON`/`nastavenia.url_servera` mechanizmus (ten
zostáva v Nastaveniach ako predtým, používa sa len inde — sync/upload v
Rust vrstve, nie v JS pre ikony). Ikona sa teraz kreslí v:
Economy (`economyIkona`, predtým existovalo, len opravené), CURRENT
INVENTORY (nový stĺpec s ikonou pred menom), Recent Runs rozbalenie „čo
padlo" (ikona pri každom odznaku). Mini-panel (`ui/mini.js`) dostal
rovnakú opravu — `IKONY_POLOZIEK` z `polozky-ikony.json`, bez
`ZAKLAD_IKON`. **`ui/ikony.js` (ručne kreslené SVG siluety väzňov/akcií)
sa nedotýka** — je to zámerne iný, rýchlejší offline systém (komentár na
začiatku súboru), nové reálne ikony položiek sú vedľa neho, nie namiesto.

**Seed zosynchronizovaný na TITrack revision 2** (`ui/polozky-seed-en.json`
1933→2189 položiek, `ui/polozky-kategorie-en.json` 1933→2189 — nových 256
id zaradených podľa existujúcej mapy `type_cn -> kategória` z TLI 49, 6
nových čínskych typov zo SS14 Tartarus — `塔罗斯技能`/`始源塔罗斯技能` →
Skills, `塔罗斯技能历练材料`/`始源塔罗斯技能历练材料` → Skills, `赛季罗盘`
→ Compasses, `追随者` → Followers — priradených ručne proti existujúcim
vzorom, nie necháných padnúť do General).

4 nové UI jsdom testy (Economy/Current Inventory/Recent Runs ikona,
mini-panel ikona) — `ui/testy-dashboard-titrack.e2e.mjs` +3,
`ui/testy-mini-pripomienka.e2e.mjs` +1 (82 zelených spolu, predtým 78).
`src-tauri` len `cargo check` (žiadna zmena v Rust kóde, kontrola len kvôli
istote), `core` `cargo check` nezávisle tiež zelený.

**Bod C (screenshoty pred/po) zo zadania TLI 53 je v tomto behu takisto
spracovaný** — pozri hlásenie úlohy a `JOURNAL.md` 19.10.2026.

## 0.20.6 — 19.10.2026 (TLI 53 časť 1/N — Net Worth počítal len stránku 103, chýbala stránka 102)

Úloha `2026-10-19-1453-mcp.md` (HNED, nedozorovaný beh, trieda `zasah`, repo
`/home/agent/praca/tli-overlay`). core 0.13.14→0.13.15.

**Potvrdený nahlásený bug.** CURRENT INVENTORY/NET WORTH doteraz čítali len
`BagMgr` stránku 103 („ostatné"). Overené priamo na Richardovom uploadnutom
`UE_game.log` (CT 114, cez `pct exec`): FE (100300), embery (200003,
200028–30, 201028–29), memory thready (5142–5144) aj divinity/wedge
materiály (5201, 5210, 5220, 5230, 5250) sú na stránke **102** („meny"), nie
103 — presne ako Richard tušil. Appka stav všetkých stránok (100–102)
interne sledovala správne už od TLI 51 (dvojité počítanie z TLI 49 bodu 3
opravila TLI 51), chýbal len posledný krok: UI stránku 102 vôbec nečítalo.

**Oprava, zhodná s TITrackovým `TRACKED_PAGES`** (`titrack.data.inventory`,
MIT, overené v zdroji): CURRENT INVENTORY aj NET WORTH teraz zahŕňajú
stránky 101 (zručnosti), 102 (meny) a 103 (ostatné) — výstroj (100) ostáva
vynechaná, presne ako TITrackovo `EXCLUDED_PAGES = [GEAR]` (cena závisí od
afixov, appka ju nevie oceniť). Nová `STRANKY_BATOHU_SLEDOVANE` a
`Sledovac::batoh_stranky()` v `core`. FE sa cení vždy 1:1 bez dane (rovnaký
princíp ako `core::tracker::ceny::ocen_v_case`, teraz aj v
`aktualny_batoh()`).

**Zmenená formula Net Worth.** Pôvodne `net_worth_fe = hodnota_batohu (len
stránka 103) + cisty_zisk` — `cisty_zisk` bol náhrada za chýbajúci prístup
k skutočnému FE zostatku. Teraz, keď `hodnota_batohu` FE zostatok obsahuje
priamo, `net_worth_fe = hodnota_batohu` (presne TITrackova formula z
`api/routes/inventory.py`: FE 1:1 + všetko ostatné s daňou). Sčítanie oboch
by FE zarátalo dvakrát. `cisty_zisk` ostáva ako vlastná metrika (karta
„CUMULATIVE VALUE"), len sa už nemieša do Net Worth.

**UI dôsledok:** keďže appka FE/zručnosti vidí hneď pri prihlásení aj bez
Sortu materiálov (na rozdiel od stránky 103), `batoh.length === 0` prestalo
byť spoľahlivým signálom pre upozornenie „Sortni inventár". Appka dostala
nové pole `materialy_sortovane` (`true`, len keď appka už videla dump
stránky 103) a dashboard aj footer naň prešli.

**Testy:** +4 nové (`current_inventory_zahrna_fe_zo_stranky_102_...`,
`..._komoditu_zo_stranky_102...`, `vystroj_na_strance_100_sa_nedostane...`,
`net_worth_sa_uz_nescitava_s_cistym_ziskom...`), core 409→409 (bez zmeny,
zmenil sa len `stav.rs` o pridanú metódu, žiadny nový core-level test),
UI jsdom testy upravené (2× `testy-dashboard-titrack.e2e.mjs`, zmenená
sémantika `materialy_sortovane`), `nahlad-stub.js` dostal `materialy_sortovane: true`.

**Čo zostáva na ďalšiu časť TLI 53** (self-queued pokračovanie, nie
BACKLOG): bod B — grafika ako TITrack vrátane offline ikon ~2000 položiek
(`data:` URI/lokálne súbory, zdroj `cdn.tlidb.com` z `img` poľa v
`tlidb_items_seed_en.json`) naprieč Dashboard/Recent Runs/Current
Inventory/Economy/loot overlay; bod C — screenshoty pred/po loot overlayu.
Audit zvyšku TLI 40–52 (toasty, Tartarus Ctrl+Q, Frozen Canvas F9, ceny F10,
upload logov, self-update) prebehol čítaním kódu/testov/JOURNALu bez nájdenia
ďalšieho rozdielu oproti zadaniu — TLI 51 bežala na Sonnet 5, táto časť na
Sonnet 5 (nie na Opus 5.5 — taký model neexistuje, pozri hlásenie).

## 0.20.5 — 19.10.2026 (TLI 52, časť 2/2 — perUser NSIS inštalátor + kód na tichú aktualizáciu, zatiaľ VYPNUTÉ naživo)

Úloha `2026-10-19-1408-agent.md` (HNED, self-queued pokračovanie časti 1/2,
nedozorovaný beh). core 0.13.13→0.13.14.

**Prieskum namiesto hádania (zadanie žiadalo overiť v dokumentácii nainštalovanej
verzie, nie z pamäte).** Stiahnutý `installer.nsi` zo šablóny `tauri-bundler`
presne v pripnutej verzii (`2.12.3`, z `Cargo.lock`) a zdroj oficiálneho
`tauri-plugin-updater` (vetva `v2`) ukázali niečo podstatné: **Tauriho vlastná
NSIS šablóna UŽ MÁ zabudovanú migráciu z WiX (MSI) inštalácie** — pri inštalácii
hľadá v `HKLM\...\Uninstall` záznam so zhodným `DisplayName`+`Publisher`
(u nás „Tvoj Tatko TLI Overlay"+„Tvoj Tatko" — presne naše hodnoty) a keď ho
nájde, bezpodmienečne spustí jeho `UninstallString` (`msiexec /X{GUID}`) PRED
vlastnou inštaláciou. Žiadny vlastný NSIS hook na migráciu teda netreba písať —
je to presne scenár zo zadania (bod 3), len už hotový v knižnici. A oficiálny
`tauri-plugin-updater` pri režime `Quiet` posiela NSIS inštalátoru presne
`/S /UPDATE /R /ARGS ""` — `/S` = ticho bez okien, `/UPDATE` nemaže nastavenia
appky, `/R /ARGS ""` appku po dokončení reštartuje. perUser (`installMode:
currentUser`) inštalátor pritom žiada `RequestExecutionLevel user`, teda
**žiadny UAC** — jediná výnimka je práve tá vstavaná migrácia zo starej
perMachine MSI, ktorá spúšťa `msiexec` vyžadujúci admin práva, a teda UAC JEDEN
RAZ pri prvom prechode. To je presne to, čo zadanie (bod 3) výslovne prijalo
ako OK — a odvtedy (ďalšie aktualizácie už idú z perUser na perUser) žiadny
ďalší UAC nepríde.

**Čo je hotové:**
- `tauri.conf.json`: `bundle.targets` rozšírené o `"nsis"` vedľa `"msi"`
  (ten sa nemenil), `bundle.windows.nsis.installMode: "currentUser"`.
- `core/src/aktualizacia.rs`: `Manifest` dostal dve nové VOLITEĽNÉ polia
  (`installerUrl`/`installerSha256`, `#[serde(default)]`) — starý aj nový
  manifest bez nich sa parsuje bez chyby (`stary_manifest_bez_installer_poli_
  sa_precita`), `skontroluj_manifest` ich adresu overuje rovnako prísne ako
  `msiUrl` (`manifest_s_cudzim_installerom_je_chyba`, rovnaká poistka ako pri
  incidente A2Tools). Nová `over_sha256()` (crate `sha2`, čistý Rust, beží aj
  v CI na Linuxe) overuje odtlačok stiahnutých bajtov. 4 nové core testy,
  405→409 celkom, všetky zelené.
- `src-tauri/src/klient.rs`: `stiahni_installer()` — stiahne `-setup.exe` ako
  surové bajty (nie text), adresu si overuje rovnako ako `stiahni_manifest`.
- `src-tauri/src/main.rs`: `pokus_o_tichu_aktualizaciu()` — keď manifest nesie
  `installerUrl`+`installerSha256`, appka stiahne inštalátor, overí SHA256,
  zapíše do `%TEMP%`, spustí ho s `/S /UPDATE /R /ARGS ""` a sama sa ukončí
  (`app.exit(0)`). Keď polia chýbajú (dnešný živý manifest), funkcia sa vráti
  hneď na prvom riadku bez akéhokoľvek I/O — appka sa správa presne ako
  doteraz (odkaz na MSI vo footeri/Nastaveniach).
- `.github/workflows/release.yml`: `cargo tauri build` teraz stavia OBA
  targety, workflow premenuje/hashuje/pripína k releasu aj `-setup.exe` vedľa
  `.msi` (premenovaný na „Release MSI + NSIS").

**Čo je ZÁMERNE VYPNUTÉ a prečo.** Live `latest-tli.json` naďalej NEMÁ polia
`installerUrl`/`installerSha256` — `pokus_o_tichu_aktualizaciu` sa preto u
ŽIADNEHO dnes nainštalovaného klienta (vrátane tejto verzie, keby sa nasadila)
nikdy nespustí. Dôvod je rovnaký, aký napísal Richard v zadaní (bod 5): toto je
zmena na jeho DENNE POUŽÍVANOM nástroji a jej prvý skutočný beh (ten jeden UAC
pri migrácii) sa má overiť OSOBNE na jeho reálnom PC, nie naslepo auto-updatom
širokej skupine. `tli-v0.20.5` je preto bezpečný na vydanie — existujúci
klienti uvidia novú verziu, kliknú „Check for Updates" a dostanú presne ten istý
tok ako doteraz (MSI v prehliadači), pretože manifest ešte nič nové neponúka.
NSIS `-setup.exe` je ale od tohto releasu k dispozícii ako asset na GitHube —
Richard si ho môže sám stiahnuť a ručne vyskúšať na svojom stroji (čistá
inštalácia aj migrácia z existujúcej `tli-v0.20.4` MSI).

**Čo NIE je hotové a čaká na ďalší krok:**
- CI build oboch targetov (krok 2 zo zadania) — spustený, výsledok zapísaný
  nižšie/v `JOURNAL.md` podľa toho, čo stihol tento beh.
- Automatizované overenie migrácie perMachine→perUser na Windows runneri
  (vlastný workflow, inštalácia starej MSI → inštalácia novej NSIS → kontrola,
  že stará zmizla a nová je v `HKCU`) — ak ho tento beh nestihol, je to ďalší
  krok, nie dôvod needeploynúť 0.20.5 (ten je, ako vyššie, bez rizika).
- Krok 5 zo zadania: OSOBNÉ overenie na Richardovom PC. Toto sa nedá spraviť
  z CT 111 ani z CI — čaká to na Richarda a na rozhodnutie, kedy (a či vôbec
  automaticky) sa `installerUrl` doplní do živého `latest-tli.json`.
- Bod 7 zo zadania (auto-ceny, overenie proti reálnemu logu) — nezmenilo sa
  oproti 0.20.4: stále čaká, kým Richard pošle obsah `tli-overlay.log`.
- `installer-hooks.nsh` sa NEPÍSAL — po prieskume vyššie by bol zbytočný
  (migrácia je už vstavaná), a písať vlastný hook len preto, že to zadanie
  navrhovalo ako možnosť, by bol kód navyše bez dôvodu.

## 0.20.4 — 19.10.2026 (TLI 52, časť 1/2 — periodická kontrola aktualizácie, loguje a ukazuje posledný výsledok)

Úloha `2026-10-19-1321-mcp.md` (HNED), nedozorovaný beh. core sa nemenilo
(0.13.13), appka/UI áno.

**Čo je hotové — bod 2 zo zadania.** Appka doteraz kontrolu aktualizácie
VÔBEC nespúšťala sama — ani pri štarte, ani periodicky — hráč musel
kliknúť „Check for Updates" ručne (Nastavenia alebo footer), inak appka
o novej verzii nevedela NIKDY a jej log o tom nemal ani riadok. Presne to
sa stalo Richardovi 19.10.2026: bežal tri vydania pozadu (0.20.1, hoci
0.20.2 aj 0.20.3 už boli vonku) a zistil to až náhodou. Oprava:
- Nové vlákno (`spusti_aktualizacia_vlakno`) kontroluje `latest-tli.json`
  hneď pri štarte appky a potom každých 30 minút.
- KAŽDÁ kontrola (periodická aj ručná z tlačidla) ide cez jedno spoločné
  miesto (`vykonaj_kontrolu_aktualizacie`) a zapíše riadok `[update]` do
  `tli-overlay.log` — nájdenú verziu, „beží najnovšia" alebo chybu siete.
- Footer „Collector" (`ui/hlavne.html`) ukazuje nový riadok „posledná
  kontrola HH:MM" vedľa „verzia X" — živo cez udalosť `aktualizacia-kontrola`,
  aj pri otvorení okna PO tom, čo už kontrola prebehla (`posledna_kontrola_
  aktualizacie` príkaz, rovnaký vzor ako `posledna_pripomienka`).
- 2 nové jsdom testy (`testy-dashboard-titrack.e2e.mjs`), `ui` 78/78 zelené.
  `src-tauri`/`core` len `cargo check` (disk rootfs CT 111, pravidlo
  o `cargo test` z predošlých TLI úloh platí ďalej).

**Čo NIE je hotové — bod 1 zo zadania (tichý update bez UAC).** Toto je
jadro Richardovej žiadosti a zámerne sa NEROBILO naslepo v tomto behu.
Zistenie pri prieskume: appka doteraz nemala ŽIADNU automatickú inštaláciu
vôbec (ani tichú, ani s UAC) — „Check for Updates" len otvorí MSI URL
v prehliadači a hráč ho musí sám spustiť a odklikať. Preferované riešenie
zo zadania (perUser inštalácia, Tauri `bundle.windows.nsis.installMode:
currentUser`, s migráciou zo starého perMachine MSI vrátane JEDNÉHO UAC na
odinštalovanie starého) je architektonická zmena inštalátora, ktorú tento
beh (headless na Linuxe, bez Windows stroja) nevie overiť naživo — chybná
migrácia inštalátora na Richardovom DENNE POUŽÍVANOM nástroji je drahá
chyba (dve súbežné inštalácie, rozbitý update uprostred hrania). Zvažovaná
alternatíva zo zadania (naplánovaná úloha s najvyššími právami cez
`schtasks`) sa zámerne NEIMPLEMENTOVALA — ten konkrétny mechanizmus
(„highest privileges" scheduled task spúšťaná netrivializovane z bežného
procesu bez nového UAC) je technicky totožný so známym vzorom UAC-bypassu,
ktorý bezpečnostný softvér (Windows Defender/SmartScreen) občas označuje —
riziko väčšej brzdy než terajší manuálny update. Pokračovanie (TLI 52,
časť 2/2): doriešiť `bundle.windows.nsis` ako DOPLNKOVÝ target vedľa MSI
(appka tak dostane perUser inštalátor bez zrušenia existujúceho release
pipeline), migračný krok (odinštalovanie starého perMachine MSI pri prvej
inštalácii perUser verzie) a samotné tiché stiahnutie+spustenie v Ruste —
toto vyžaduje reálne otestovanie na Richardovom PC (vlastné overenie zo
zadania, bod 3), nie len zelené CI.

**Bod 4 — auto-ceny, zistenie bez zmeny kódu.** Kód (`src-tauri/src/
tracker.rs`, gate `!davka_je_zo_starej_historie && je_otvorenie_trhu`, TLI
48) zostal od poslednej opravy nezmenený a pri čítaní vyzerá správne —
spúšťa sa VÝHRADNE na genuinne NOVOM riadku `AuctionHouseShowItemGoodV2
Run`, nie pri dobiehaní starej histórie. Časová zhoda nahlásenú udalosť
z 0.20.1 (10:19, ts 1792397952) vysvetľuje bez bugu: appka sa reštartovala
na 0.20.1 o 09:54 a CHANGELOG k 0.20.2 (napísaný neskôr tú istú ráno)
zaznamenáva, že dovtedy („log od 09:54") nemal ani jeden výskyt toho
reťazca — 10:19 teda vyzerá ako PRVÉ skutočné otvorenie trhu v hre PO
reštarte, teda presne tá situácia, pre ktorú `trh_otvoreny=true` existuje
(správne správanie, nie regresia). **Toto je prečítanie kódu, nie dôkaz
z reálneho logu** — appka svoj vlastný `tli-overlay.log` nikam neposiela
(len `UE_game.log` sa nahráva), takže si to vie definitívne potvrdiť len
Richard cez tlačidlo „Otvoriť log" (riadok s „Ceny stlačené (zdroj=auto)"
okolo 10:19 a riadky `[update]`/hotkey tesne okolo toho). Bez dôkazu
o opaku sa kód NEMENIL.

## 0.20.3 — 19.10.2026 (TLI 51 — oprava duplicitných položiek na neexponovaných stránkach batohu)

Úloha `2026-10-19-1123-agent.md` (HNED, dokončenie zistenia z TLI 49 bodu 3).
core 0.13.12→0.13.13 (len core, appka/UI sa nemenili).

**Príčina.** Hra pri triedení inventára vydá kompletný nový dump celej
stránky batohu (`BagMgr@:InitBagData`) a tá istá položka v ňom môže dostať
INÝ `SlotId` než mala predtým. Appka doteraz držala `self.batoh` ako trvalú
mapu `(stránka, slot) → položka`, ktorú mazal len explicitný
`RemoveBagItem`/`Modfy Num=0` — pri preradení do iného slotu preto stará
dvojica `(stránka, starý_slot)` ostala navždy a `batoh_stranka()` tú istú
položku započítala dvakrát (starý aj nový slot). Zistené pri overení proti
TITracku nad Richardovým reálnym logom (TLI 49 bod 3): `8125` (2 namiesto
1), `380201` a `382192` (4 namiesto 2), `502100` (420 namiesto 210) — všetko
na stránkach 101/102, ktoré appka hráčovi nikdy neukazuje (len interný stav).

**Oprava** (`core/src/tracker/stav.rs`) replikuje presne TITrackovu logiku
(`titrack.collector.collector:_handle_bag_event`, MIT): appka si pamätá
stránku a čas POSLEDNÉHO spracovaného `InitBagData` riadku; keď ďalší taký
riadok patrí INEJ stránke (alebo tej istej po >2 s medzere — zhodné
s TITrackovým `_init_batch_threshold_seconds`), všetky staré sloty TEJ
stránky sa pred zápisom novej dávky vyčistia. Opakované `InitBagData`
riadky tej istej stránky v tesnom slede (do 2 s, bez inej stránky medzi
nimi — bežný priebeh jedného dumpu) sa nedotknú ničoho.

**Overenie:** nezávislé spustenie skutočného zdrojového kódu TITracku
(`astockman99/TITrack`, klon z GitHubu) nad TÝM ISTÝM 91 MB logom
(780 550 riadkov, rovnaký úsek ako TLI 49) potvrdilo presnú zhodu so
stránkou po oprave — 8125=1, 380201=2, 382192=2, 502100=210, a CELÉ stránky
101 (40/40 položiek) aj 102 (39/39) sa teraz zhodujú s TITrackom **bezo
zvyšku**, nielen pri týchto štyroch položkách. **Stránka 103 (CURRENT
INVENTORY, jediná viditeľná v UI) ostala bezo zmeny** (35/35, 0 rozdielov,
rovnako pred aj po oprave) — porovnané priamo proti pôvodnému JSON výstupu
diagnostického nástroja `core/examples/tli_over_titrack.rs` spred opravy.
Stránka 100 (výstroj) sa s TITrackom nezhoduje zámerne a bez súvisu s touto
opravou — TITrack stránku Gear vedome vynecháva z trackovania celú
(`InventoryPage.GEAR`/`EXCLUDED_PAGES`, „prices too dependent on affixes"),
appka ju interne sleduje ďalej, ale nikde ju nevystavuje.
4 nové unit testy v `core/src/tracker/stav.rs` (`tli51_*`) s doslovnými
riadkami z Richardovho logu — všetky 405 existujúcich testov `core/`
(401 unit + 4+8 e2e) prešli bez zmeny. `src-tauri` len `cargo check`
(bez `cargo test` — disk rootfs CT 111).

## 0.20.2 — 19.10.2026 (TLI 49 — kategórie v Economy, čerstvosť cien všade, overenie proti TITracku)

Úloha `2026-10-19-1011-mcp.md` (HNED, dokončenie zvyšku TLI 48). core
0.13.11→0.13.12.

**Bod 1 — kategórie v Economy HOTOVÉ.** Richardovo tvrdenie bolo správne:
TITrackov zdrojový súbor (`tlidb_items_seed_en.json`) MÁ pole `type_cn` pre
každú položku — appka si pri prevzatí 16.10.2026 zámerne ponechala len
dvojice id→meno (`meta.poznamka`), typy sa nikdy nepreberali. Nové
`ui/polozky-kategorie-en.json` mapuje všetkých 1933 položiek z nášho seedu na
12 anglických kategórií (Legendary Gear, Compasses, Beacons, Skills, Destiny,
Organs, Followers, Prisms, Divinity, Netherrealm, Season Items, General) —
vlastná mapovacia tabuľka nad prevzatým `type_cn`, nie prevzatá kategorizácia
(NOTICE.md bod 4). Economy dostala záložky s ikonami nad tabuľkou (rovnaký
vzor ako titrack.ninja), filtrovanie sa kombinuje so zdrojom aj hľadaním.

**Bod 2 — čerstvosť cien.** Nájdený a opravený skutočný bug: `zapisane`
(lokálny čas zápisu do DB) sa pri cenách od partie/titracku ukladal ako
`c.kedy` — teda čas, kedy NIEKTO V PARTII cenu vyhľadal, nie kedy ju TÁTO
appka stiahla. Presne to je dôvod, prečo Richard na vlastnom PC videl 8 919
riadkov `ceny` so zdrojom `titrack` bez čitateľného „kedy stiahnuté". Opravené
na skutočný čas stiahnutia (`+test`). Appka teraz ukazuje vek ceny v Economy
(bolo už od TLI 48), aj v Current Inventory („Cena/ks" dostala „(pred X
min)"), aj v loot overlay mini-paneli (tooltip „cena pred X min" pre
VŠETKY zdroje, predtým len pre titrack atribúciu). Pätička aj záložka
Economy ukazujú „ceny aktualizované HH:MM" (kedy appka naposledy DOKONČILA
kolo synchronizácie s partiou). Každé kolo synchronizácie zapíše riadok do
denníka appky (počet odoslaných/stiahnutých, trvanie v ms).

**Bod 3 — CURRENT INVENTORY a NET WORTH overené proti TITracku.** TITrackov
vlastný zdrojový kód (MIT, GitHub) sa spustil ako CLI
(`titrack.cli.commands.cmd_parse_file`) nad TOU ISTOU surovou kópiou
Richardovho `UE_game.log`, akú má appka na serveri. Výsledok: appkina
CURRENT INVENTORY (stránka batohu 103 „ostatné") sa s TITrackom ZHODUJE
PRESNE — 42/42 položiek, 210/210 kusov, 0 rozdielov. Appka interne sleduje aj
stránky 100 (výstroj)/101 (zručnosti)/102 (meny), ktoré v UI zatiaľ
NEUKAZUJE — tam sa našli 4 položky s presne 2× vyšším počtom než u TITracku
(`8125`, `380201`, `382192`, `502100`), čo appka ani TITrack v UI dnes
nezobrazujú hráčovi, ale stojí za doriešenie (zadané ako samostatná
nadväzujúca úloha, nie BACKLOG). Formula NET WORTH je štruktúrne zhodná s
TITrackom (mena 1:1 + cena×počet×daň 0,875). Detail: `JOURNAL.md` 19.10.2026,
`NOTICE.md`.

**Bod 4 — auto-ceny.** Regresný test z TLI 48
(`historicke_otvorenie_trhu_pri_prvom_citani_nenastavi_trh_otvoreny`) pokrýval
len JEDNO volanie `precitaj` nad malým súborom. Pridaný
`historicke_otvorenie_trhu_ostava_mrtve_aj_naprieč_viacerymi_davkami_dobiehania`
overuje to isté nad súborom cez viac `DAVKA_B` (2 MB) dávok — presne scenár
reálneho ~90 MB logu. Richardov log od 09:54 (reštart na 0.20.1) nemá ani
jeden výskyt `AuctionHouseShowItemGoodV2 Run` — živé potvrdenie teda zatiaľ
nie je možné, čaká sa, kým Richard znova otvorí trh v hre.

**Bod 5 — loot overlay, ťahanie bez Ctrl+F8.** Návod pre Richarda je v
hlásení úlohy (interaktívne otestovať vie len on sám v hre, rovnako ako TLI
48 bod 2).

## 0.20.1 — 19.10.2026 (TLI 48 — toasty nad hrou, ťahateľný loot overlay, Economy, oprava auto-cien)

Úloha `2026-10-19-0838-mcp.md` (HNED, dokončenie zvyšku TLI 47/doplnku
`0744-mcp`), nedozorovaný beh. core 0.13.10→0.13.11.

**Bod 4 — bug z logu 19.10. opravený.** Appka pri štarte vedela sama spustiť
„Ceny" (zdroj=auto) nad CELOU OBRAZOVKOU a skončiť s „Na trhu sa nenašla žiadna
čitateľná cena". Príčina: `AuctionHouseShowItemGoodV2 Run` sa v logu hľadalo
ako jednoduchá textová zhoda na KAŽDOM riadku, aj pri prvom dočítaní (dobiehaní)
starého, už dávno zavretého výskytu z históriou logu — appka si to vyložila
ako „trh je PRÁVE TERAZ otvorený" a odfotila, čo práve bolo na obrazovke
(typicky plochu/dashboard appky samotnej). Oprava: `StavTrackera` si pri
otvorení cesty (aj po rotácii) zapamätá hranicu „odtiaľto je to už naozaj
nové" (`povodna_velkost_logu`) — riadky z dávky, ktorá ešte len dobieha starý
obsah, sa už NEPOČÍTAJÚ za živé otvorenie trhu. +3 testy
(`src-tauri::tracker`).

**Bod 1 — upozornenia PRIAMO NAD HROU, nie len v pätičke dashboardu.**
Pätička (TLI 47) už vedela rozlíšiť tri stavy, ale na toast nad hrou sa
dostala len polovica („Zapni Enable Log" mala vlastný stavový automat od
TLI 11d odjakživa). Nové: „Log sa neposiela na server" a „Sortni inventár v
hre" teraz appka posiela aj ako toast priamo nad hru, nie len do schovanej
záložky. Nový, zámerne JEDNODUCHŠÍ mechanizmus než `SledovacPripomienky`
(`core::tracker::varovania::SledovacVarovania`) — opakovaný toast namiesto
presného stavového automatu: appka ho obnovuje á 25 s, kým podmienka trvá, a
keď sa vyrieši, prestane ho obnovovať — toast zmizne sám najneskôr do 35 s.
Zdieľa jedno okno s ostatnými toastmi (ceny/Enable Log), „novší vyhráva",
rovnaký vzor ako doteraz. core +6 testov.

**Bod 2 — loot overlay (mini-panel) ťahateľný BEZ Ctrl+F8.** Horný pásik
(`#mini-uchyt-presun`) je odteraz interaktívny aj mimo režimu úpravy — jemne
priehľadný, plne viditeľný pri hoveri/editácii. Keďže Windows/Tauri vie
`ignore_cursor_events` len pre CELÉ okno, appka na pozadí sleduje `GetCursorPos`
(25×/s) a prepína klikateľnosť LEN keď je kurzor nad pásikom — klik kdekoľvek
inde na paneli prechádza ďalej do hry. Poloha sa po potiahnutí ukladá SAMA
(nový príkaz `mini_ulozit_polohu`, rovnaká matematika ako
`rezim_upravy_ulozit_panel`) — per-monitor, rovnako ako doteraz cez Ctrl+F8.
**Nedá sa overiť bez skutočnej hry na Windowse** — CI len skompiluje
`#[cfg(windows)]` vetvu, interaktívne správanie (cítenie ťahania, žiadne
kradnutie focusu hre) musí potvrdiť Richard.

**Bod 3 — Economy: plný prehliadač cien** namiesto odkazu na existujúci
prepínač. Nová záložka v hlavnej navigácii (vedľa Guide), nový príkaz
`tracker_economy` (`core::tracker::economy_prehlad`): meno + ikona (rovnaký
zdroj ako mini-panel), zdroj ceny (vlastný trh/obrazovka/partia/titrack.ninja/
ručne), **zmena za 24 h** (nová DB funkcia `db::ceny_pred` — appka si už aj
tak ukladá históriu cien, len to doteraz nikto nepotreboval vyčítať), vek,
hľadanie podľa mena, filter podľa zdroja, triedenie kliknutím na hlavičku.
**„Kategórie" NIE SÚ implementované** — appka nepozná skutočné herné typy
položiek (`ui/polozky-seed-en.json` § meta.poznamka: seed mien zámerne prevzal
LEN dvojice id→meno, žiadne typy), nie je to medzera v kóde ale chýbajúca
dátová základňa; zapísané do `BACKLOG.md`. core +2 testy, UI +3 jsdom testy.

**Bod 5 — E2E voči Richardovým dátam: ČIASTOČNÉ.** Server má jeho reálne logy
(372 behov zo 17.–18.10., TLI 47) a appka nad nimi počíta Current Inventory/
Net Worth rovnakou cestou, akú si tento beh overil novými testami — ale
porovnanie s TITrack referenciou na Richardovom PC (`G:\Môj disk\claude\tli\
TITrack`) vie spraviť len Richard sám, agent k tomu počítaču prístup nemá.
Zapísané do `BACKLOG.md`.

Core 401+ testov, UI jsdom 69 testov (+3). `cargo check --tests` zelené pre
`core` aj `src-tauri` na Linuxe (CT 111, rootfs 15 GB — `target/` vždy
zmazané hneď po behu). Nič sa nemazalo, baaza ani aion2 sa nedotkli.

## 0.20.0 — 19.10.2026 (TLI 47, 2/2 — CURRENT INVENTORY + upozornenia v pätičke)

Úloha `2026-10-19-0744-mcp.md` (doplnok k `2026-10-19-0731-mcp.md`), nedozorovaný
nočný beh. core 0.13.9→0.13.10.

**CURRENT INVENTORY je HOTOVÉ (zadanie bod 2).** Predošlý predpoklad (BACKLOG
pred týmto behom) bol, že appka potrebuje ÚPLNE NOVÝ, doteraz neanalyzovaný typ
log riadku „po Sorte" — **to bol omyl**: appka od TLI 11b/13 už parsuje
`BagMgr@:InitBagData` (presne ten riadok, ktorý hra zapíše pri Sorte aj pri
prihlásení, `log-format.md` §…) a od začiatku si z neho stavia ÚPLNÝ obraz
batohu (`Sledovac::batoh`) — len ho doteraz používala LEN na počítanie delty
zisku, nikdy ho nevystavila von ako aktuálny stav. Stačilo teda pridať getter
(`core::tracker::stav::batoh_stranka`, nový `pub const STRANKA_BATOHU_OSTATNE =
103`) a agregáciu s cenníkom (`src-tauri::tracker::aktualny_batoh`) — žiadny
nový parser.
- **Záložka Tracker má novú tabuľku „Current inventory"** — položka (meno cez
  existujúci `polozky-seed-en.json`), počet, cena/ks, hodnota, „bez ceny" pre
  neznáme položky (nikdy 0 — nula a „neviem" sú dve rôzne veci). Hľadanie podľa
  mena, triedenie podľa hodnoty/počtu/názvu, tlačidlo **Hide/Unhide** na
  položku (persistentné v `nastavenia.json` → `batoh_skryte_polozky`, nový
  príkaz `tracker_prepni_skrytu_polozku`) — appka nič nemaže ani nepredáva,
  len filtruje VLASTNÉ zobrazenie.
- **Je to výhradne BATOH, nie sklad** — `BagMgr` (PageId 100 výstroj/101
  zručnosti/102 meny/103 ostatné) je úplne iný systém než `WareHouseMgr`
  (sklad), appka do skladu vôbec nenazerá.
- **NET WORTH je teraz skutočný súčet batohu (bez skrytých) + doterajší
  zisk**, presne podľa zadania — kým appka ešte nevidela ani jeden Sort tejto
  session, karta to hovorí nahlas odznakom „⏳ ešte bez batohu" namiesto
  predstierania hodnoty.

**Upozornenia v pätičke dashboardu (zadanie bod 1, časť „červený stav").**
Pätička teraz rozlišuje tri stavy namiesto jedného:
- **„⚠ Log sa neposiela na server"** — keď appka hrá (`zivo` nie je `null`) a
  posledný úspešný upload je starší než 15 minút, alebo posledný pokus
  skončil chybou (nové pole `posledny_upload_pred_s` v `PrehladProUi`).
- **„⚠ Sortni inventár v hre"** — appka hrá, ale `batoh` je ešte prázdny
  (žiadny `InitBagData` v tejto session).
- Mimo hry (žiadna rozohraná mapa) appka mlčí — ticho mimo hry je normálne,
  varovanie len vtedy, keď sa dá naozaj niečo prehliadnuť.
- **Toast nad hrou pre tieto dve nové upozornenia (zadania „toast +" časť)
  NIE JE v tomto behu** — existujúci toast systém (`pripomienka.rs`) je úzko
  postavený len pre „Enable Log" (vlastný stavový automat, testy naviazané na
  presne tento jeden prípad) a rozšíriť ho bezpečne o ďalšie dva typy je
  samostatná práca. Footer je vidieť VŽDY, keď je hlavné okno otvorené —
  čiastočné, nie žiadne pokrytie. Zapísané do `BACKLOG.md` ako pokračovanie.

**Čo v tomto behu NIE JE** (zadania body 1 toast, 3, 4 — zapísané ako
nadväzujúca úloha, nie do `BACKLOG.md`, podľa pokynu zadania „zadaj
nadväzujúcu úlohu sám"): toast nad hrou pre upload/Sort upozornenia; loot
overlay ťahateľný za pásik bez Ctrl+F8 (zmena `ignore_cursor_events` len pre
mini okno); Economy ako plný prehliadač cien (titrack.ninja + vlastné).

Testy: core 395+3 (`batoh_stranka`), ui 66 (+6: NET WORTH z batohu, prázdny
batoh, filter/triedenie/Hide, varovania v pätičke). `cargo check` zelené pre
`core` aj `src-tauri` (dev profil, Linux — `#[cfg(windows)]` vetvy sa
netypujú, BACKLOG položka TLI 37). Rollback: `latest-tli.json` späť na
`0.19.3`.

## 0.19.3 — 19.10.2026 (TLI 47 bod A — viditeľný stav uploadu logu)

Úloha `2026-10-19-0731-mcp.md`, nedozorovaný nočný beh. Richard si myslel, že
upload logu na server nefunguje (server vraj od 14.10. nemá nové dáta) —
server-side overenie (`/srv/games/tli/tracker/<discord_id>/`, nie priečinok
`burina` z predošlej manuálnej analýzy TLI 46) ukázalo, že upload v
skutočnosti **funguje a je priebežne aktuálny** (naposledy 19.10. 00:21,
celý 845 MB log bez medzery). Príčina falošného poplachu bola, že predošlé
analýzy čítali starý, raz ručne nahraný priečinok `burina/`, nie skutočnú
cestu podľa Discord ID, kam appka reálne nahráva. Detail:
`docs/fakty/hry/torchlight-infinite/README.md` § 2026-10-19.

- **Stav uploadu je teraz vidieť aj v pätičke dashboardu**, nielen v
  skrytom paneli Nastavení trackera (`#stav-upload`) — nový
  `vykresliPatickaUploadu()` v `hlavne.js` ukazuje „Posledný upload: pred X"
  / „Posledný upload: chyba (…)" / „Posledný upload: vypnutý" vedľa verzie
  appky, presne ako TITrack „Log was last updated". Dáta sú tie isté, čo už
  appka od TLI 21/24 počíta (`posledna_kontrola_uploadu_pred`,
  `posledna_chyba_uploadu`) — len doteraz neboli nikde trvalo viditeľné.
- Nový E2E test (`ui/testy-dashboard-titrack.e2e.mjs`) — pätička reaguje na
  úspech, chybu aj vypnutie prepínača.

## 0.19.2 — 19.10.2026 (TLI 45 časť 1/3 — dashboard podľa TITracku)

Úloha `2026-10-19-0024-mcp.md`, nedozorovaný nočný beh. Richard pred spaním (cez
Cowork) chce záložku Tracker vizuálne aj funkčne bližšie k TITracku
(astockman99/TITrack, MIT, appka je len pre partiu a nepublikuje sa — smieme
preberať vzhľad/pomenovania s atribúciou). Zadanie malo 6 bodov; tento release
pokrýva **časť vzhľadu a dashboardu bez čítania celého inventára** (to je
samostatná, výrazne väčšia časť — pozri BACKLOG.md a STAV.md `ROZPRACOVANE`).

- **7 kariet presne podľa TITracku**: NET WORTH, CUMULATIVE VALUE, FE/HOUR,
  FE/MAP, RUNS, AVG RUN TIME, TOTAL TIME. Appka ešte NEČÍTA celý batoh, takže
  NET WORTH je zatiaľ len čistý zisk spolu (rovnaké číslo ako CUMULATIVE
  VALUE) — karta to hovorí nahlas odznakom „⏳ bez hodnoty batoha", nepredstiera
  hodnotu, ktorú appka nepozná.
- **Dva grafy** (Cumulative value, FE/hour) — čisté inline SVG z `p.behy`
  (žiadna knižnica, žiadne nové pripojenie).
- **Recent Runs**: riadok histórie máp sa dá rozbaliť kliknutím a ukáže „čo
  reálne padlo" — server teraz počíta `top_dropy` pre KAŽDÝ uložený beh
  (`VysledokBehu.top_dropy`, `core/src/tracker/agregacie.rs`), nie len pre
  živú mapu ako doteraz (mini-panel, TLI 25). Pridané tlačidlo **Report**
  (stiahne `.txt` so zhrnutím tej jednej mapy).
- **Hlavičkové tlačidlá podľa TITracku**: Overlay / Economy / Cloud Sync / ⚙ /
  Instructions / Reset Stats — každé otvára EXISTUJÚcu funkciu appky (skok na
  záložku/prepínač, nový modál, alebo nový príkaz), nič sa neduplikuje.
  **Instructions** je nový statický slovenský modál (Enable Log, Sort batohu
  pred čítaním inventára, hotkeys, mini-panel, Guide).
  **Reset Stats** je nový príkaz `tracker_reset_zive_staty` — vymaže LEN živé
  behy tejto relácie v pamäti (čo kreslí dashboard), nič na disku: pozícia
  čítania logu aj lokálna DB pre záložku Analýza ostávajú nedotknuté. Žiadne
  mazanie súboru, presne podľa zadania („Nič nemaž").
- **Footer „Collector"**: Beží / verzia / Check for Updates (reuse
  `verzia`/`skontroluj_aktualizaciu`, rovnaké ako v Nastaveniach) a
  „Auto-refresh: každé 3 s" (existujúci `setInterval`, teraz pomenovaný).
- Testy: nový `core` test (`ocen_beh_vrati_aj_top_dropy_nie_len_sucty`, 394
  core testov spolu), nový jsdom balík `ui/testy-dashboard-titrack.e2e.mjs`
  (9 testov — karty, grafy, rozbalenie, Report, nav tlačidlá, Instructions,
  Reset Stats, footer), `cargo check` pre `core` aj `src-tauri` zelené.

**Čo v tomto release NIE JE** (pozri BACKLOG.md): CURRENT INVENTORY (čítanie
celého batoha po Sort v hre), loot overlay ťahateľný bez Ctrl+F8, plný
prehliadač cien (Economy zatiaľ len odkazuje na existujúci prepínač a počet
cien), E2E/screenshot CI nad novým dashboardom ako samostatný krok a finálny
release 0.20.0 — ten príde až keď budú hotové aj zvyšné časti zadania.

## 0.19.1 — 19.10.2026 (TLI 44 — Ctrl+Q rada sa pri objective/poradcovi nezobrazí)

Úloha `2026-10-18-2331-mcp.md`. Richard 18.10. večer naživo zistil, že cez
hotkey (appka má Tartarus naviazaný na Ctrl+Q) sa po obrazovkách „Objective"
a univerzálny „poradca" nezobrazí NIČ, hoci log ukáže „analyza OK" aj
„vykreslené panel… viditelny=true" — presne cez tray/hlavné okno („Odfotiť
Tartarus teraz") to isté zafunguje. Doska a výber nástroja cez hotkey fungujú.

**Príčina nadväzuje na TLI 40, ale je jemnejšia:** `aktivuj_a_vrat_fokus`
(pulz `SetForegroundWindow`, ktorý WebView2 „zobudí" z occlusion spánku) sa
volal LEN RAZ, na úplnom začiatku analýzy — pri placeholderi „Snímam…", ešte
pred volaním servera. Doska a výber nástroja zvyknú dobehnúť rýchlo a stihnú
sa vykresliť, kým je WebView2 ešte hore. `poradca` pri neznámej obrazovke
volá model DVA razy (rozpoznanie + poradca, cieľová latencia appky je do 8 s)
a `objective` priraďuje katalóg navyše — kým server odpovedal, WebView2 už
znova zaspalo a panel sa „vykreslil" len v DOM-e/CSS (JS log to aj ukázal),
nie na obrazovke.

**Oprava, štyri časti:**
1. **Jedna funkcia `zobraz_vysledok`** nahrádza samostatné volania
   `posli_polohu_panela` + `app.emit("stav", …)` vo VŠETKÝCH vetvách
   (doska, výber nástroja, výber pokladu, objective, poradca) — pošle
   polohu panela, PULZNE zobudí WebView2 ešte RAZ tesne pred odoslaním
   reálneho obsahu a až potom emituje `stav`. Druhý pulz prichádza tesne
   pred obsahom, nie na začiatku, takže funguje bez ohľadu na to, ako dlho
   server trval. Rovnaký pulz pridaný aj do toastu pripomienky (`pripomienka`
   má `focus: false` ako `overlay` — rovnaké riziko, preventívne).
2. **Automatický fallback v `over_prezentaciu`:** keď kontrola „frame
   prezentovaný" zistí NIE, appka skúsi `aktivuj_a_vrat_fokus` ešte raz a po
   rovnakom oneskorení overí znova (`frame prezentovaný po fallbacku`) —
   namiesto toho, aby len ticho zapísala zlyhanie.
3. **Debounce 500 ms** na globálne klávesové skratky
   (`hotkey_debounovany`) — Richard v logu videl DVE „Tartarus stlačené"
   v tej istej sekunde na jeden stlak (`RegisterHotKey` na Windowse posiela
   `WM_HOTKEY` opakovane, kým je kombinácia držaná, podobne ako key-repeat).
   Druhé doručenie tej istej skratky do 500 ms od predošlej sa teraz zahodí
   potichu, pred akýmkoľvek ďalším rozhodovaním v handleri.
4. **Testy:** `core` bez zmeny (`0.13.7`). `src-tauri` +8 (`hotkey_debounovany`
   4, `vyhodnot_prezentaciu` 4, pridané do existujúcej sady `#[cfg(test)] mod
   testy`) — obe funkcie sú čistá logika bez OS volania, bežia aj mimo
   `#[cfg(windows)]`. `ui` jsdom 51/51 bez zmeny (klient je čisto na strane
   Rustu). Samotný pulz a fallback nemajú ako sa overiť automatizovane mimo
   živej hry na Windowse — Richard to overí naživo.

Release `tli-v0.19.1`, CI aj verify-install zelené, oznam v `#torchlight`.

## 0.19.0 — 18.10.2026 (TLI 43 — ceny z titrack.ninja)

Úloha `2026-10-18-2043-mcp.md`. Richard: žiadne návrhy správ autorom TITracku,
chce funkčný zdroj cien — partia sama cez `XchgSearchPrice` prakticky nič
nevyhľadáva (TLI 20), takže cenník bol riedky. Zdroj: verejná crowd-sourced
ekonomika `titrack.ninja` (poe.ninja pre Torchlight Infinite, server „SS14
Tartarus"), objavená a zdokumentovaná v `docs/fakty/.../README.md`
18.10.2026 20:5x.

**Toto NIE JE návrat rozhodnutia z TLI 20** („na komunitné ceny TITracku sa
nenapájame") — tamojší zákaz mieril na iný prístup: priamy Supabase backend
projektu TITrack s natvrdo zapísaným kľúčom v zdrojáku, bez verejného API a
bez podmienok pre tretie strany. `titrack.ninja` je bežná verejná webstránka
(iný branding, Next.js, Vercel) — appka na ňu robí len obyčajné GET, presne
ako prehliadač, bez prihlásenia, bez robots.txt zákazu (overené, vracia 404).

**Zmeny sú celé na SERVERI** (`stacks/games/tli-web/app.py`, ops repo) —
appka hráča naďalej berie ceny výhradne z `/api/ceny`, nikdy priamo z
cudzieho servera:
- Nové vlákno na pozadí sťahuje `https://www.titrack.ninja/?season=1601` á
  25 min (slušné `User-Agent`), parsuje Next.js server-render RSC payload
  (žiadne samostatné `/api/prices` neexistuje), a drží cache na disku — pri
  výpadku siete alebo zmene tvaru stránky appka tíško pokračuje s poslednou
  známou cache, nikdy nespadne na nulu.
- `config_base_id` z titracku JE priamo naše ConfigBaseId (obe pochádzajú z
  tej istej TITrack komunity, overené krížovo oproti `polozky-seed-en.json`)
  — žiadne mapovanie podľa mena netreba, čo je presnejšie než pôvodné
  zadanie predpokladalo. Malá kontrola zhody mena beží pre prípad budúceho
  rozchodu ID priestorov, len loguje, nič nevyraďuje.
- `/api/ceny` teraz zlučuje partiu s titrackom: keď má položku oboje,
  vyhráva silnejšie skóre `dôveryhodnosť_istoty × váha_veku` (rovnaký
  princíp ako klientovo `Cennik::vloz_v_case` z TLI 20, len medzi dvoma
  stiahnutými zdrojmi). Odpoveď nesie nové pole `zdroj` (`"partia"` alebo
  `"titrack"`) pri každej položke.
- Atribúcia „Ceny: titrack.ninja (komunita) a hráči partie." je v pätičke
  `base.html` na celom webe.

**Klient** dostal `Zdroj::Titrack` (dôveryhodnosť 0,7 — medzi `Partia` 0,8 a
`Rucne` 0,6, keďže ide o externý, nie vlastný pozorovaný zdroj) a `/api/ceny`
odpoveď prevezme nové pole `zdroj` priamo (predtým malo appka natvrdo
„partia" aj pre budúce iné zdroje — opravené aj v lokálnej perzistencii
`prijmi_ceny_partie`). Mini-panel „čo padlo" (TLI 25) ukáže pri cenách z
titracku tooltip `titrack.ninja · N prisp. · pred X` — viditeľný text sa
nemenil, panel je príliš úzky na tretí riadok. Nastavenia (prepínač „Zdieľať
ceny s partiou") dostali vetu o titracku.

**Bod 3 zadania — „prepočítaj ocenenie behov druhým prechodom":** po každom
`sync_cien_kolo`, ktorý cenník naozaj zmenil, appka spustí `db::
precen_nevycislene` — rovnaký princíp ako druhý prechod pri importe logu
(TLI 20), len nad CELOU históriou behov s `cisty_zisk_fe IS NULL`, nie len
nad práve prečítaným úsekom. Bežia v krátkych dávkach (200 behov), zvyšok
doženie ďalšie kolo o 5 min.

Testy: `tli-web` 197/197 (+13 nových — parsovanie RSC payloadu vrátane
vnorených polí, výpadok siete/zmena tvaru stránky necháva starú cache,
mergovanie skóre, krížová kontrola mien, perzistencia cache na disk). `core`
392/392 (+8 — nový `Zdroj::Titrack`, `precen_nevycislene`, `top_dropy` nesie
atribúciu), `src-tauri` `cargo check`/`cargo check --tests` zelené, `ui`
jsdom 51/51 (+1 — tooltip). `core` bump `0.13.7` → `0.13.8` (zmenené
`ceny.rs`/`ceny_sync.rs`/`agregacie.rs`/`db.rs`).

Rollback: `tli-web.yml` → `titrack.zapnute: false` + redeploy — `/api/ceny`
sa vráti k samotnej partii, posledná cache na disku ostáva nedotknutá
(nič sa nemaže).

## 0.18.16 — 18.10.2026 (TLI 42 — 0.18.14/0.18.15 sa nespustia, oprava)

Úloha `2026-10-18-2042-mcp.md`. Richard nahlásil, že appka 0.18.14 sa u neho
VÔBEC NESPUSTÍ (opakovateľné 6×, aj po zabití osirelých `msedgewebview2`
procesov): panika hneď pri štarte, `Failed to setup app: ... WebView2 error:
WindowsError(HRESULT(0x8007139F) "The group or resource is not in the
correct state to perform the requested operation.")`. **Rollback bol prvý
krok** — `latest-tli.json` aj `/pripojenie` (`stacks/games/tli-web`, ops repo)
sa hneď vrátili na poslednú fungujúcu `0.18.13`, aby auto-updater medzitým
nerozdával rozbitú appku.

**Príčina:** TLI 40 (0.18.14) pridal `additionalBrowserArgs` len oknu
`overlay` v `tauri.conf.json` — zvyšné štyri okná (`nastavenia`, `mini`,
`hlavne`, `pripomienka`) ho nemali vôbec. Všetkých päť okien je vytváraných
hneď pri štarte appky (len `visible: false`) a zdieľa jeden WebView2
user-data-folder — a WebView2 odmietne vytvoriť druhé okno nad tým istým
user-data-folderom, keď dostane INÉ argumenty prehliadača než to prvé, presne
s chybou `0x8007139F`. 0.18.15 (TLI 41) tauri.conf.json nemenil, takže zdedil
rovnakú chybu a bol rozbitý rovnako.

**Oprava:** rovnaké `additionalBrowserArgs` (disable occlusion
calculation/backgrounding) teraz majú VŠETKÝCH päť okien, nie len `overlay` —
žiadna appka v procese tak nežiada o iný WebView2 environment. Pôvodná oprava
z TLI 40 (`SetForegroundWindow` pre F8 render, diagnostika zdroja popudu) sa
nemení.

**Nová poistka v CI:** `verify-install.yml` doteraz overoval len inštaláciu/
odinštaláciu MSI, appku nikdy nespustil — presne preto tento pád neodhalil.
Pribudol krok, ktorý appku po inštalácii na Windows runneri spustí, počká
10 s, overí, že proces stále beží (appka s touto chybou panikári do pár
stoviek ms) a že jej denník (`%APPDATA%\sk.tvojtatko.tli-overlay\logs\
tli-overlay.log`) obsahuje aspoň jednu úspešnú registráciu klávesovej
skratky (`"registrácia OK"`) — dôkaz, že setup appky dobehol. Release, ktorý
sa nespustí, teraz CI nepustí ďalej.

Testy: `cargo check` aj `cargo check --tests` v `core/` a `src-tauri/`
zelené, `ui` jsdom 50/50 (UI sa nemenilo). `core` sa nemenil, verzia ostáva
`0.13.7`.

## 0.18.15 — 18.10.2026 (TLI 41, časť 1/2 — opakované Ctrl+Q nehlási chybu)

Úloha `2026-10-18-1816-mcp.md`. Audit 162 reálnych F8 snímok Richarda
(`docs/fakty/hry/torchlight-infinite/tartarus-audit.md` v repe `ops`) odhalil
viacero konkrétnych príčin „rady sú zlé", riešených hlavne na serveri
`tli-web` (samostatný deploy, bez novej verzie tohto klienta) — táto časť je
JEDINÁ zmena priamo v `tli-overlay`: zadania bod 4, opakované Ctrl+Q počas
prebiehajúcej analýzy.

**Predtým:** appka mala zámok proti dvojitému spusteniu (`Stav.beh`), ale
druhé stlačenie počas prebiehajúcej analýzy TICHO zahodila (len riadok
v logu, žiadna spätná väzba hráčovi) — vyzeralo to, akoby appka na rýchle
opakované Ctrl+Q nereagovala alebo „vypísala chybu" (Richard, 18.10.2026).

**Teraz:** požiadavka, ktorá príde počas prebiehajúcej analýzy, sa uloží do
nového poľa `Stav.cakajuci_dalsi` (jedno miesto, nie front) a spustí sa HNEĎ
po dobehnutí tej prebiehajúcej. Ďalšie stlačenie medzitým nahradí predošlé
čakajúce — vždy vyhráva POSLEDNÁ snímka, presne ako žiada zadanie bod 4.
Žiadna viditeľná chyba, žiadne zahodenie poslednej požiadavky.

Testy: `cargo check` aj `cargo check --tests` v `src-tauri` zelené (čistá
Linux kontrola typov — logika zámku beží len pod `#[cfg(windows)]`, naživo ju
overí Richard). `core` sa nemenil, verzia ostáva `0.13.7`.

## 0.18.14 — 18.10.2026 (TLI 40 — F8 rada sa nevykreslí, hotfix)

Úloha `2026-10-18-1622-mcp.md`. TLI 39 opravil, že F8 vôbec spustí analýzu
(zlá klávesová skratka) — toto je ĎALŠÍ, hlbší problém, ktorý Richard našiel
hneď po nej: F8 analýzu spustí, log ukáže „analyza OK" aj „vykreslený panel…
viditelny=true", ale nad hrou sa NIČ nezobrazí. Ručné spustenie (tray/tlačidlo
„Odfotiť Tartarus teraz") rovnakú analýzu ukáže bez problémov.

**Príčina:** F8 aj ručné spustenie volajú v Ruste úplne rovnakú funkciu
(`spusti_analyzu` → `ukaz_overlay_nad` → `overlay.show()`) a rovnaký
`requestAnimationFrame` v `overlay.js` — kód sa nelíši. Rozdiel je vo
WebView2 samotnom: overlay je `WS_EX_NOACTIVATE` okno, ktoré hráč nikdy
neaktivoval, a Chromium v takom okne vie DOM „vykresliť" interne a napriek
tomu ho NIKDY neprezentovať na obrazovku (správanie okolo occlusion
detekcie). Klik na tray/tlačidlo okno nášho procesu krátko aktivuje (a tým
WebView2 „zobudí"), globálna klávesová skratka (`RegisterHotKey`) také okno
nikdy nemá.

**Oprava (tri časti):**
1. Nové `additionalBrowserArgs` pre overlay okno v `tauri.conf.json`
   (`--disable-features=…,CalculateNativeWinOcclusion
   --disable-backgrounding-occluded-windows --disable-renderer-backgrounding`)
   — rovnaká hodnota, ktorú appka predtým skúšala presadiť cez premennú
   prostredia `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` bez úspechu (Tauri ju
   ignoruje, nastavuje si vlastné argumenty); teraz ide správnou cestou.
2. Appka po `overlay.show()` sama zavolá `SetForegroundWindow` na overlay
   (dokumentovaný spôsob, ako aktivovať aj `WS_EX_NOACTIVATE` okno) a HNEĎ
   nato vráti popredie späť na to, čo bolo aktívne predtým (typicky hra) —
   žiadny vstup sa nesimuluje, hráč stratu klávesnice nezbadá.
3. Diagnostika: log teraz zapisuje ZDROJ popudu (`zdroj=hotkey/rucne/auto`),
   nielen to, že sa niečo stlačilo — predtým sa z logu nedalo odlíšiť, ktorou
   cestou analýza prišla. Appka navyše 400 ms po zobrazení panela SAMA odfotí
   ten istý obdĺžnik znova a porovná ho so snímkou spred overlayu
   (`core::obraz::percento_odlisnych_pixelov`) — zapíše „frame prezentovaný:
   ÁNO/NIE" podľa toho, či sa na obrazovke NAOZAJ niečo zmenilo. Je to prvá
   automatická kontrola „viditeľné naozaj", nezávislá od toho, čo si appka
   sama o sebe myslí.

Testy: `core` 26 testov (+5, `percento_odlisnych_pixelov`), `cargo check` aj
`cargo check --tests` v `src-tauri` zelené, `ui` jsdom 50/50 (bez zmeny,
JS sa nedotýkalo). Oprava sama (časti 1 a 2) nemá ako sa overiť automatizovane
mimo živej hry na Windowse — Richard to overí naživo, časť 3 (diagnostika)
je navrhnutá presne na to, aby pri ďalšom podobnom probléme dala odpoveď bez
nutnosti sedieť pri hre s Richardom.

## 0.18.13 — 18.10.2026 (TLI 39 — skutočná príčina „F8 nefunguje", klávesové skratky)

Úloha `2026-10-18-1613-mcp.md`. Skutočná príčina incidentu z 18.10. popoludní:
Tartarus skratka bola v nastaveniach omylom prepísaná na holé „DIGIT3" (cez
„Nahrať…" v Nastaveniach, bez modifikátora), čím appka F8 prestala počúvať a
navyše hre ukradla klávesu „3". Súbežne bežiaca automatická detekcia cien
z trhu zalogovala „Ceny stlačené" v tom istom čase, čo pôsobilo, akoby F8
spúšťalo nesprávny modul — v skutočnosti sú to dve nezávislé veci.

**Klávesová skratka už neprijme holú klávesu bez modifikátora** — platné sú
len F1–F12 samostatne alebo ľubovoľná klávesa držaná spolu s Ctrl/Alt/Shift/
Super. Platí pri nahrávaní stlačením aj pri priamom prepísaní poľa (validácia
pri uložení formulára). Nahrávanie stlačením navyše vyžaduje výslovné
potvrdenie pred zápisom — jedno fatfingernutie klávesy appku už neprekonfiguruje.

**`hotkey_trh` (ceny z trhu) má odteraz vlastnú predvolenú hodnotu „F10"**
namiesto prázdneho reťazca — appka ju nikdy nenechá bez vlastnej klávesy.
Pridaná kontrola konfliktov MEDZI VLASTNÝMI tromi skratkami appky (Tartarus/
Frozen Canvas/ceny z trhu): keď sú dve nastavené na rovnakú klávesu, appka tú
nižšie prioritnú vôbec nezaregistruje a ukáže presné červené hlásenie, ktorý
modul s ktorým koliduje.

**Toast pri štarte appky a nová položka „Klávesy: …" v tray menu** ukazujú
zoznam aktívne zaregistrovaných skratiek (Tartarus/Frozen Canvas/ceny z trhu/
Upraviť rozloženie) — presne to, čo pri incidente chýbalo: appka sa so žiadnou
chybou nehlásila (DIGIT3 sa zaregistrovať podarilo), len to nebola klávesa,
ktorú hráč čakal pod F8.

Testy: `core` 24 testov v `nastavenia.rs` (+1), `src-tauri` nový
`#[cfg(test)] mod testy` s 10 testami na `konfliktne_moduly`/
`text_prehladu_klaves` (overené mimo Tauri, `rustc --test` nad extrahovanou
kópiou funkcií — plný `cargo test` v `src-tauri` sa na tomto hostiteľovi
nerobí, len `cargo check --tests`, pozri `project-tli-overlay-release-flow`),
`ui/testy-nastavenia-hotkeys.e2e.mjs` nový (8 testov nad skutočným
`nastavenia.html`/`nastavenia.js` v jsdom).

## 0.18.12 — 18.10.2026 (TLI 37 — Objective: cieľ behu riadi stratégiu)

Úloha `2026-10-18-1324-mcp.md`. Richard: na začiatku každého behu Tartarusu je
obrazovka „Objective: <názov>" s podmienkou (napr. *Consume 4 times*) a
odmenou — a „podľa objective sa má upraviť celá stratégia".

**Nová obrazovka na F8.** Server posiela `obrazovka: "objective"` s poľom
`objective` (cieľ, podmienka, počet, priebeh), `potvrdenie` („Objective:
Požretie (Consume) 4× — stratégia upravená") a `plan` (2–3 vety po slovensky).
Appka ich ukáže vo vlastnej fáze `hotovo_objective` a orámuje tlačidlo
Confirm. Panel má vlastnú uloženú polohu (`panel:objective`, v prepínači
režimu úpravy ako „F8 — objective (cieľ behu)").

**Cieľ je odteraz vidieť na KAŽDEJ obrazovke**, nie len na tej, kde sa zadal:
nový riadok panela ukazuje napr. `🎯 Prežije najsilnejší — Požretie (Consume)
2/4`. Priebeh je **odhad zdola z tvojich F8 snímok dosky** (appka do hry
nevidí) a hovorí to nahlas — keď medzi dvoma snímkami preskočíš deň, ten deň
sa nezapočíta. Cieľ, ktorého priebeh sa zmerať nedá, to napíše namiesto
vymysleného čísla.

Odporúčania (Consume/Replace/End, Go to Cell, Torture Instrument, Warden's
Treasure) cieľ zohľadňujú a **povedia to v dôvode** („Berieme Požretie
(Consume) — chýbajú 2 do objective „Prežije najsilnejší" (2/4)"). Je to
preferencia, nie príkaz: zjavne horší ťah sa tým dopredu nedostane.

**Dve opravy z tej istej snímky:**
* Mini-panel (aj záložka Tracker) už neukazuje dropy ako holé `#115` —
  položka bez známeho mena sa volá „neznáma položka #115". Tie id naozaj nie
  sú v balíku mien (overené aj proti zoznamom FurTorch a TITrack), takže
  menný balík to nevyrieši; aspoň sa to dá prečítať.
* **Prázdny rám pripomienky uprostred hore sa už nekreslí.** Toast bez textu
  sa nezobrazí vôbec, a keby obsah doletel skôr, než sa okno stihlo prihlásiť
  na udalosť, okno si ho po načítaní vypýta samo (`posledna_pripomienka`).

## 0.18.11 — 18.10.2026 (TLI 36 — F8 poradí pri KAŽDOM výbere v Tartaruse)

Úloha `2026-10-18-1323-mcp.md`. Richard: „chcem mačknúť F8 a nech mi vždy
pomôže pri hocijakom výbere v tej minihre". Doteraz appka na obrazovke bez
špecializovaného modulu ukázala len toast „Túto obrazovku zatiaľ neviem
prečítať" a nenakreslila nič.

Odteraz server (`tli-web`, iný repo) v takom prípade pustí **univerzálneho
poradcu** a pošle `obrazovka: "poradca"` s poľom `moznosti` — zoznam toho,
na čo sa dá na obrazovke kliknúť, s bounding boxmi, poradím a slovenským
dôvodom. Appka to kreslí **tou istou cestou ako výber nástroja a daru**:
zlatý rámik na odporúčanú možnosť, slabší na druhú, panel s dôvodom. Nová
fáza `hotovo_poradca` len volá rovnaký vykresľovač — tvar položky je zhodný
s `nastroje`/`poklady`.

Čo je na poradcovi inak než na ostatných moduloch: `tier` nie je tier karty,
ale poradie (S = ber túto, A = druhá), a istota sa hlási **slovom**
(„nízka/stredná/vysoká"), lebo je to odhad nad pravidlami hry, nie Monte
Carlo nad známym stavom — percento by predstieralo presnosť, ktorú tá rada
nemá. Poradca má **vlastnú uloženú polohu panela** (`panel:poradca`) a
pribudol do prepínača modulov v režime „Upraviť rozloženie".

Druhá vec, ktorú to opravilo: na paneli „Make Your Decision"
(Consume/Replace/End) solver odporúčal ŤAH, ktorý sa k žiadnemu z tých
tlačidiel nepriradil (3 z 5 reálnych snímok z 18.10.2026), a appka zvýraznila
celu, na ktorú sa v tom modále kliknúť nedalo. Teraz to preberá poradca
a vráti boxy tlačidiel.

Keďže poradca je DRUHÉ volanie modelu nad tou istou snímkou, odpoveď občas
presiahne cieľových 8 s (nameraných 9,4 s nad doskou). Indikátor preto po
8 sekundách zmení text z „Odoslané, čakám na radu…" na „Pracujem…" —
nič iné sa nemení, odpoveď sa čaká ďalej.

`Analyza` dostala polia `moznosti`, `typ_obrazovky`, `nazov_obrazovky`
a `istota_slovom`, všetky `#[serde(default)]` — proti staršiemu serveru bez
nich sa appka správa presne ako doteraz.

## 0.18.10 — 18.10.2026 (TLI 35 — nová obrazovka Tartarus „Warden's Treasure")

Úloha `2026-10-18-1321-mcp.md`. F8 teraz rozpoznáva aj tretiu obrazovku
mechaniky Tartarus — „Select a Warden's Treasure" (výber 1 zo 4 darov po
dosiahnutí Chromatic: Ring/Horn/Wine/Brand), rovnako ako doteraz doska
(TLI 1–24) a výber nástroja (TLI 25). `Analyza` dostala pole `poklady`
(`#[serde(default)]`, rovnaký tvar položky ako `nastroje` — starší server bez
neho sa ďalej číta ako predtým). Appka ju kreslí tou istou cestou ako výber
nástroja (rámik s tierom a poradím nad každou kartičkou, odporúčanú zvýrazní
zlato, panel s poradím a dôvodom) — nová fáza `hotovo_poklad` len volá
rovnaký vykresľovač, lebo tvar dát je zhodný. Výber pokladu má vlastnú
uloženú polohu panela (`panel:poklad`), rovnako ako výber nástroja — pribudol
do prepínača modulov v režime „Upraviť rozloženie". Server (`tli-web`, iný
repo) pri tomto type obrazovky — na rozdiel od výberu nástroja — prepočíta
odporúčanie Monte Carlom priamo nad poslednou známou doskou toho istého
hráča, keď je po ruke; appka to nerieši, len zobrazí, čo príde.

## 0.18.9 — 18.10.2026 (TLI 34 — doladenie rady Tartarus: šum z rámikov, nepoužiteľná istota, prekrývajúce sa panely)

Úloha `2026-10-18-1319-mcp.md`. Richardov F8 screenshot z fázy „Make Your
Decision" (18.10. ~13:40) ukázal tri problémy naraz, všetky opravené:

- **Všetkých 16 ciel kreslilo oranžový rámik vždy, aj keď sa odporúčanie
  netýkalo cely vôbec** (napr. odporúčanie mierilo na tlačidlo spodnej lišty,
  TLI 32) — šum namiesto informácie. `.pole` (`ui/overlay.css`) už nemá
  predvolený rámik; kreslí ho len `.pole.ciel` (cieľová cela odporúčania) a
  `.pole.neiste` (cela s neistým čítaním). Tool-select kartičky (`.karticka`)
  a Frozen Canvas kroky (`.krok-box`) dostali vlastnú `border-color`/`border`,
  nech sa nezmenia (zdieľali predtým farbu cez `.pole`).
- **Istota ťahu bola nepoužiteľných 6 %, hoci odporúčanie bolo rozumné** —
  skutočná príčina bola v serveri (`tli-web`/`engine/solver.py`, samostatný
  projekt, nie tento repo): `istota` meralo len to, ako predvídateľný je
  výsledok JEDNEJ možnosti (podiel Monte Carlo rolloutov nad jej vlastným
  priemerom), nie či je táto možnosť lepšia než alternatívy. Pri šikmom
  rozdelení skóre to vychádzalo jednociferne aj pri jasnej voľbe. Opravené:
  nová istota porovnáva najlepšiu možnosť s druhou najlepšou (rovnaký princíp
  ako výber nástroja, TLI 25); „End Day" je odteraz tiež reálna vyhodnocovaná
  alternatíva (predtým len núdzová záchrana, keď nebolo čo iné urobiť), takže
  appka má konečne s čím porovnávať a `alternativy` prestávajú byť prázdne
  zakaždým, keď bol dostupný devour/swap. Klient (tento repo) sa nemenil —
  kontrakt `Analyza.istota`/`Alternativa` ostáva rovnaký, len číslo, ktoré
  doň príde, je teraz zmysluplné.
- **Panel rady sa predvolene prekrýval s herným textom „Objective / Trigger"
  (vľavo hore) aj s mini-panelom trackera** (ten má vlastnú predvolenú polohu
  tiež vpravo hore) — predvolená poloha `#panel` (CSS) aj editačného režimu
  (`predvolena_poloha` v `src-tauri/src/main.rs`) presunutá vpravo, pod
  mini-panel. Hráč s už uloženou polohou (ťahaním v „Upraviť rozloženie")
  nie je dotknutý — mení sa len predvoľba pre nikdy neuloženú/obnovenú polohu.
- **„Select a Destination" („Go to Cell N")** — overené, že appka ju už
  rozpoznáva (TLI 32, `akcie_dole` + `_najdi_tlacidlo` na serveri), žiadna
  zmena tu nebola potrebná.

## 0.18.8 — 18.10.2026 (TLI 33 — F8 rada sa nezobrazí, hoci server odpovie)

Úloha `2026-10-18-1318-mcp.md`. Richard na 0.18.7 dostal po F8 trikrát (13:15:16,
13:16:33, 13:16:55) kompletný beh — okno hry nájdené, `analyza OK` za 7–8 s,
log „doska: 16 polí … emitujem stav=hotovo" — ale nad hrou sa neukázalo ÚPLNE
NIČ (žiadna rada, žiadne zvýraznenie, žiadny panel). Z Windows strany (Claude
na Richardovom PC, 13:17) EnumWindows potvrdil, že okno overlaya JE nad hrou,
topmost a viditeľné — TLI 31 (vynútenie z-poradia) teda funguje, problém je
v niečom ĎALEJ (JS/dáta), nie v okne samotnom.

- **Reálne nájdená a opravená príčina: mini-panel (NIE hlavný panel) sa pri
  ŠTARTE appky s už zapnutým mini-overlayom (uložené z minulého behu)
  zobrazil na defaultnej OS pozícii, nie na uloženej** — `setup()` doteraz pri
  štarte volal len `okno.show()`, zatiaľ čo jedinou cestou, ktorá polohu
  naozaj prepočíta (`umiestni_native_okno` podľa uloženého zlomku okna hry),
  bol prepínač v Nastaveniach (`nastav_mini`). Presne to sedí s Richardovým
  pozorovaním „mini sa kreslí na zlom mieste (~182,182), ignoruje uloženú
  polohu vpravo hore" a s empty-toast nálezom zapísaným 18.10. 13:23 do faktov
  (`docs/fakty/hry/torchlight-infinite/tartarus-pravidla.md`). Oprava: štart
  teraz volá `nastav_mini(&handle, mini_zapnuty)` namiesto ručného show/hide.
- **Hlavný panel (F8/F9): príčinu sa cez statický code review a 30/30 zelených
  JS e2e testov NEPODARILO jednoznačne izolovať** — `overlay.js` aj
  `ukaz_overlay_nad`/emit `stav` zodpovedajú rovnakému, už testovanému tvaru
  dát. Namiesto ďalšieho hádania pribudli DVE diagnostiky, ktoré nabudúce
  rozlíšia „event nedorazil do JS" od „event dorazil, vykreslenie zlyhalo":
  1. **`overlay.js` loguje späť do Rustu cez nový príkaz `zapis_ui_log`**
     (→ `denik::zapis("ui", …)`, teda do toho istého `logs/aktualny.log` ako
     `[hotkey]`/`[zobraz]`/`[http]`) — `[ui] prijaté stav=<fáza>` pri KAŽDOM
     evente a `[ui] vykreslené panel (<zdroj>) x=… y=… w=… h=… viditelny=…`
     tesne po `requestAnimationFrame` pri každom skutočnom vykreslení (doska,
     výber nástroja, Frozen Canvas, aj ich prázdne/bez-polohy varianty).
  2. **Tray → „Test overlaya"** (`test_overlay`) — prejde PRESNE tou istou
     cestou ako reálny F8 (`ukaz_overlay_nad` so `s_rozlozenim=true`, teda aj
     `posli_polohu_panela`, + `app.emit("stav", faza=hotovo)`) s vymyslenými
     dátami namiesto volania servera, zobrazí vzorovú radu na 10 s. Funguje aj
     bez bežiacej hry (spadne na primárny monitor). Keď sa po kliknutí
     nezobrazí nič, chyba je v okne/evente a dá sa to zopakovať kedykoľvek bez
     hry; keď sa vzorová rada ukáže, ale pri reálnom F8 nie, chyba je v
     niečom, čo robí/posiela appka LEN v tej reálnej vetve.
- **Rozpoznanie „Select a Destination"/„Go to Cell N" overené, nie zmenené** —
  `_najdi_tlacidlo` (server, `app.py`) aj `_moznosti_pre_stav` (`engine/
  solver.py`) už párujú podľa ČÍSLA cely z textu tlačidla (nie podľa poradia
  v zozname) a solver pri otvorených `destinations` vracia VÝHRADNE patrolu na
  ne — logika teda bola správna už pred touto úlohou. Pridané regresné testy
  (`testy-tli-web.py`) nad presne číslami z Richardovej snímky (Go to Cell
  11/12/14, mimo poradia aj mimo rozsahu 1-3), aby prípadná budúca zmena
  tento prípad nerozbila ticho.
- Appka sa server vôbec nepýtala na nič nové — zmeny sú v `tli-overlay`
  (appka) a testoch v `stacks/games/tli-web` (ops repo), nie v produkčnom
  `app.py`.

## 0.18.7 — 18.10.2026 (TLI 32 — zvýrazniť tlačidlo akcie dole, nie šípku na celu)

Úloha `2026-10-18-1101-mcp.md`. Richard: overlay dnes ukazuje šípku na konkrétnu celu
dosky, ale ťah sa v hre robí stlačením tlačidla v spodnej lište ("Select a
Destination"/"Make Your Decision"). Chce zvýrazniť TO tlačidlo, cela nech je najviac
jemne doplnková alebo vôbec.

- **Rozpoznanie lišty je teraz súčasť analýzy, nie natvrdo pixely.** Gemini
  (`VISION_PROMPT` na serveri) vracia bounding box každého viditeľného tlačidla
  spodnej lišty v novom poli `akcie_dole[]` (`{"text","x","y","w","h"}`) — pre
  panel "Select a Destination" (`"Go to Cell N"` / pomenovaná miestnosť) aj
  "Make Your Decision" (`"Consume"/"Replace"/"End"`). Z piatich reálnych F8
  snímok Richarda (18.10.2026 10:54–10:58) a dvoch živých volaní
  `gemini-beh.py --obrazok` overené naživo, že rozpoznanie sedí.
- **Server páruje odporúčanie s tlačidlom** (`_najdi_tlacidlo` v `app.py`):
  `patrol` podľa čísla cely v texte (`Cell N` = `ciel + 1`), `devour`/`swap`/
  `koniec_dna` podľa doslovného textu (`Consume`/`Replace`/`End`). Index do
  `akcie_dole` sa vracia ako nové pole `tlacidlo` v `odporucanie` aj v prvej
  `alternativa` — `null`, keď sa zhoda nenašla.
- **Appka (core/src-tauri):** nové `AkciaDole`, `Odporucanie.tlacidlo`/
  `Alternativa.tlacidlo` (rovnaká `Option` opatrnosť ako `cez_scratch`, TLI 29 —
  appka nikdy nepadne na chýbajúcom/neznámom poli). V `doska` vetve appka
  prepočíta box tou istou funkciou ako cely (`box01_na_css`) a pošle
  `tlacidlo`/`tlacidlo_alt` (druhé najlepšie, len keď je to INÉ tlačidlo).
- **Overlay (`overlay.js`/`overlay.css`):** keď server vie tlačidlo priradiť,
  appka ho zvýrazní (zlatá žiara + krátky SK popis akcie + istota) a **celu
  nezvýrazní vôbec** (žiadna trieda `.ciel`, žiadna šípka) — presne zadanie.
  Keď sa zhoda nenašla (iný panel, neznáma akcia), appka sa správa presne ako
  predtým: zvýrazní celu a nakreslí šípku. Výber „Torture Instrument" (kartička
  + Confirm) sa nemenil — bol už hotový.
- **Webový simulátor** (`tli.tvojtatko.sk/simulator`) hovorí rovnakými pojmami
  tlačidiel ako hra: „Stlač tlačidlo „Go to Cell 7““ namiesto „Patrola na
  políčko 07", aby tréning zodpovedal tomu, čo appka reálne zvýrazňuje.
- **Testy na reálnych snímkach:** `testy-tli-web.py` +6 (132/132, vrátane
  doslovných `akcie_dole` z dvoch reálnych F8 snímok), core Rust +4 (386
  testov), nový E2E `e2e_doska_s_tlacidlom_spodnej_listy` (telo presne z
  `2026-10-18-105603-67f8c864.json`), UI jsdom `testy-tlacidlo-akcie.e2e.mjs`
  (3 testy, vrátane fallbacku bez tlačidla).
- **Rollback:** `latest-tli.json`/`tli-web.yml` späť na `tli-v0.18.6`. Zmena je
  aditívna (nové voliteľné polia), žiadna zmena formátu uložených dát.

## 0.18.6 — 18.10.2026 (TLI 31 — F8 analýza prejde, ale výsledok sa nezobrazí nad hrou)

Úloha `2026-10-18-1057-mcp.md`. Richard na 0.18.5 dostal po F8 štyrikrát (10:54:58,
10:55:32, 10:56:04, 10:56:45) kompletný beh — hotkey zaregistrovaný, okno hry
nájdené, snímka odoslaná, `analyza OK` za 7–10 s — ale panel s výsledkom sa nad
hrou neukázal; cez tray „Odfotiť Tartarus teraz" (rovnaká `spusti_analyzu`,
rovnaké `stav` eventy) to fungovalo.

- **Príčina: `.show()` v tao na Windowse z-poradie NEMENÍ** — volá sa so
  `SWP_NOZORDER` (rovnaký mechanizmus ako pri `.set_position()`/`.set_size()`,
  zdokumentovaný už v TLI 26 bode 1 pre mini-panel/pripomienku). Keď medzitým
  iný „vždy navrchu" program (Overwolf, Discord overlay, Logitech G-HUB — podľa
  TLI 28 bežia naraz) dostal svoj vlastný `SetWindowPos(HWND_TOPMOST, …)`,
  predbehol overlay appky v pásme topmost — ten ostal nad hrou, ale POD tým
  druhým oknom, teda v praxi neviditeľný. Prečo to fungovalo cez tray: žiadny
  rozdiel v kóde (`spusti_analyzu` je identická funkcia), len náhoda poradia
  v topmost pásme v danom okamihu.
- **Oprava: `vynut_navrch()`** — pred/po `.show()` appka prepne
  `set_always_on_top(false)` → `true`. Samotné volanie `set_always_on_top(true)`,
  keď je príznak už `true`, je v tao no-op (XOR starého/nového stavu); prepnutím
  cez `false` appka vynúti nový `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`,
  ktorý okno reálne vráti na vrch topmost pásma. **Zámerne nie `.set_focus()`**
  (ako pri mini/pripomienke v režime úpravy) — to by hre odobralo klávesnicu
  uprostred hrania; `SWP_NOACTIVATE` je presne to, čo `.set_focus()` nerobí.
  Použité v `ukaz_overlay_nad` (hlavný panel — doska, výber nástroja, Frozen
  Canvas) aj `ukaz_pripomienku_toast` (toast „Enable Log", ceny z trhu).
- **Pridané logovanie `[zobraz]`** — pri každom zobrazení panela/toastu appka
  zapíše okno, polohu, veľkosť, monitor a (pri chybe) ktoré volanie zlyhalo;
  doštová (najčastejšia) vetva F8 predtým po „analyza OK" nelogovala vôbec nič,
  na rozdiel od vetiev „výber nástroja"/„ine".
- **E2E (`src-tauri/tests/e2e_mock.rs`)**: rovnaká odpoveď servera (doska, výber
  nástroja, neznáma obrazovka) overená cez obe cesty vyvolania —
  `spusti_analyzu` priamo aj cez `naplanuj_odfotenie` — vedie k rovnakému
  `stav` eventu; nemožno z Linux CI overiť samotné Windows z-poradie, len že
  appka sa správa rovnako bez ohľadu na to, čo ju spustilo.
- Skutočné overenie „vidno to nad hrou" je na Richardovom ďalšom teste —
  logy `[zobraz]` ukážu, či `vynut_navrch()` stačil, alebo treba ísť ďalej
  (napr. vypnúť Windows „Fullscreen optimizations" pre hru).

## 0.18.5 — 18.10.2026 (TLI 29 — F8 dorazí, ale appka hlási „Server odpovedal niečím, čomu nerozumiem.")

Úloha `2026-10-18-0135-mcp.md`. Richard na 0.18.4 dostal po F8 dvakrát (01:34:21
a 01:36:45) nečitateľnú odpoveď namiesto odporúčania, hoci server podľa
uloženej analýzy dosku rozpoznal správne (istota rozpoznania 0,98) a vrátil
HTTP 200. **Skutočná príčina NIE JE v Gemini ani v module `tartarus-nastroj`**
(pôvodné podozrenie z faktu zapísaného pri hlásení) — je to nezhoda typu medzi
serverom a appkou:

- **Server (`tli-web/app.py`):** keď je top odporúčanie iný ťah než „devour"
  (typicky „patrol", keď má doska otvorené `destinations` — bežný stav, nie
  okrajový prípad), pole `cez_scratch` v odpovedi chýba a server ho posielal
  ako `null`. Appka mala `cez_scratch` ako **neoptionálny** `bool` — `null`
  proti nemu zhodil parsovanie CELEJ odpovede, nielen toho poľa. Opravené na
  serveri (vždy `bool`) aj v appke (`Option<bool>`, defenzívne, pre prípad
  podobnej chyby nabudúce). Pridaný regresný test na oboch stranách s presne
  týmto reálnym tvarom odpovede.
- **Server:** jeden retry volania `gemini-beh.py` pri rýchlom zlyhaní
  (nenulový návratový kód, nečitateľný vonkajší JSON, `is_error`) — NIE pri
  timeoute, aby druhý pokus spolu s prvým nepresiahol 60 s, ktoré appka na
  odpoveď čaká.
- **Appka:** keď sa telo úspešnej (2xx) odpovede nedá rozobrať, appka teraz
  do `tli-overlay.log` zapíše HTTP kód aj prvých 200 znakov tela a v toaste
  ukáže kód a odkaz na log namiesto generickej vety bez stopy.
- **Overené, NIE je to bug:** Richardovo podozrenie, že F8 funguje len s
  otvoreným oknom Nastavení, log appky z 01:31:15–01:34:21 vyvracia — obe
  volania (01:31:31 aj 01:34:21) prebehli bez akejkoľvek interakcie s
  Nastaveniami medzi nimi. Globálne skratky (`tauri-plugin-global-shortcut`)
  sa registrujú pri štarte appky, nie pri otvorení okna.
- Screenshot a JSON z 01:34:21 (`2026-10-18-013422-16348a9c.{jpg,json}` na
  CT 114) ostávajú uložené ako doteraz (analýzy sa nemažú) a počítajú do
  50 potrebných pre TLI 5 (lokálne rozpoznávanie).

## 0.18.4 — 18.10.2026 (TLI 28 — F8 nič neodošle: diagnostika registrácie, odfotenie bez klávesy)

Úloha `2026-10-18-0048-mcp.md`, nahlásené Richardom naživo z PC: 0.18.3 beží,
hra beží, ale po F8 v logu nie je ani riadok — appka doteraz logovala len
STLAČENIE klávesy (`spusti_analyzu`), nie pokus appku o REGISTRÁCIU hotkey pri
štarte, takže sa nedalo odlíšiť „hotkey nedorazil" od „hotkey sa ani
nezaregistroval" (Overwolf, AION2, Discord aj Logitech boli naraz spustené a
F8/F9/Ctrl+F8 môžu byť obsadené ktorýmkoľvek z nich).

- **Registrácia každej skratky (Tartarus, Frozen Canvas, ceny z trhu, Ctrl+F8)
  sa od tejto verzie VŽDY loguje** — OK / „OBSADENÁ iným programom" / iná
  CHYBA, vrátane textu od Windows (ten v sebe nesie aj kód chyby, ekvivalent
  `GetLastError`, cez `global-hotkey`/`std::io::Error::last_os_error()`).
  Ďalší beh appky u Richarda už ukáže, ktorý presne program F8 drží.
- **Nastavenia ukážu červený stav pod poľom** („Skratku „F8“ Windows neprijal…")
  hneď pri OTVORENÍ okna, nie len po uložení formulára — appka o kolízii vie
  už od vlastného štartu. Pri kolízii appka navyše pošle Windows notifikáciu
  (toast mimo appky, viditeľný aj keď appka sedí v tray) aj toast nad hrou
  (rovnaký mechanizmus ako pripomienka „Enable Log").
- **Výber klávesy nahrávaním** — tlačidlo „Nahrať…" vedľa každého z troch polí
  v Nastaveniach: klikni a stlač kombináciu priamo, appka ju sama poskladá do
  textového tvaru (Escape nahrávanie zruší bez zmeny).
- **Tray menu aj hlavné okno majú „Odfotiť Tartarus teraz"/„Odfotiť Canvas
  teraz"** — spustí analýzu bez klávesy vôbec, s oneskorením 2 s na prepnutie
  do hry. Funguje aj keď je RegisterHotKey úplne zablokovaný.
- **Nízkoúrovňový klávesový hook (WH_KEYBOARD_LL / `SetWindowsHookEx`) sa
  NEPOUŽIL** — CI test `pravidla` (`.github/workflows/ci.yml`) má
  `SetWindowsHookEx` v zozname zakázaných API (zásah/sledovanie mimo appky);
  appka sa ho preto ani nepokúša obísť. Diagnostika + manuálna záloha
  (tlačidlá „Odfotiť teraz") rieši symptóm bez neho.
- **Oprava kódovania `tli-overlay.log`** — súbor je UTF-8, ale bez BOM si ho
  staršie prehliadače logov (Poznámkový blok a pod.) prečítali ako
  ANSI/CP1250 a diakritika vyšla ako „Upravi��". Appka teraz na začiatok
  ČERSTVÉHO/PRÁVE ROTOVANÉHO súboru zapíše UTF-8 BOM (`EF BB BF`).
- **Overené (nie zmenené):** poradie `setup()` vs. vytvorenie okien a bežiaci
  event loop (podozrenie na regresiu z TLI 23) — `RegisterHotKey` potrebuje len
  existujúce okno (vytvorené pri inicializácii pluginu, PRED `setup()`), nie
  bežiaci message pump; ten sa rozbehne hneď po návrate zo `setup()`. Žiadna
  zmena kódu, len komentár s odôvodnením pri registrácii. Skutočné overenie v
  hre (čo presne F8 držalo) je na ďalšom Richardovom teste — appka to po tejto
  verzii uvidí v logu.

Úloha `2026-10-17-2231-mcp.md`, nahlásené Richardom naživo z PC: v režime
„Upraviť rozloženie" (Ctrl+F8) sa dal ťahať len ľavý panel, mini-panel (zisk/čo
padlo) a pripomienka „Enable Log" sa nehýbali; overlay sa pri vypnutej hre
postavil na nesprávny monitor a nedal sa presunúť na primárny; priečinok
`logs` neexistoval, hoci 0.17.2 ho malo pridať.

- **⚠️ `pripomienka.js` mal od 0.17.0 (TLI 21, keď pribudlo ťahanie okien)
  SYNTAKTICKÚ CHYBU, ktorá appke bránila spustiť z neho VÔBEC ČOKOĽVEK** —
  reťazec `"Náhľad pripomienky „Enable Log" — presuň…"` používal obyčajnú
  ASCII úvodzovku `"` namiesto správnej uzatváracej slovenskej `“`, takže
  JS reťazec sa skončil uprostred vety a zvyšok (`— presuň ma…`) bol mimo
  reťazca — neplatný JavaScript. Prehliadač/WebView2 pri takejto chybe
  NESPUSTÍ CELÝ SÚBOR, nielen ten jeden riadok — **toast „Enable Log“ teda
  od 0.17.0 nefungoval vôbec, ani mimo edit módu** (žiadne drôtovanie
  `DOMContentLoaded`, žiadne úchyty, žiadne upozornenie na vypnuté
  logovanie), nielen že sa nedal ťahať. Odhalil to nový JS test
  (`ui/testy-rezim-upravy-prenosne-okna.e2e.mjs`) hneď pri prvom spustení
  v CI — doteraz nad `pripomienka.js` žiadny test nebežal. Opravené na
  jednoduchú úvodzovku okolo celého reťazca.
- **Prečo sa mini-panel nedal ťahať (jeho JS chybu nemal):** `.show()`
  v tao/Windows z princípu NEMENÍ poradie okien (`SetWindowPos` sa volá
  s `SWP_NOZORDER`), takže keď mal hráč mini-panel zapnutý natrvalo (bol
  teda viditeľný UŽ PRED vstupom do edit módu), čerstvo zobrazený priehľadný
  `overlay` nad ním ostal navrchu a prekryl mu klik. Appka teraz po
  zobrazení mini-panela aj pripomienky v edit móde explicitne zavolá
  `.set_focus()`, nech sú vždy nad `overlay`.
- **Overlay na nesprávnom monitore, keď hra nebeží:** appka padala na
  `okno::popredie()` (naposledy aktívne okno) — čokoľvek malo focus
  (prehliadač, editor), tam sa postavil aj edit mód, pokojne na úplne inom
  monitore. Teraz padá na **primárny monitor** — rovnaký zdroj pravdy, aký už
  appka používala pri chybovej hláške a pri toaste „Enable Log".
- **Dva monitory s rovnakým rozlíšením si prepisovali polohu panelov** —
  kľúč uloženého rozloženia bol len `rozlíšenie@mierka`, bez identity
  monitora. Pridané meno monitora od OS (`\\.\DISPLAY1` a podobne) do kľúča;
  staré uložené polohy sa nestrácajú — appka ich pri hľadaní pod novým kľúčom
  skúsi nájsť aj pod starým (čistým) tvarom.
- **Log appky (`%APPDATA%\sk.tvojtatko.tli-overlay\logs`) sa nezapisoval**,
  kým hráč nestlačil F8/F9 — pri appke bežiacej bez tých kláves (hra
  nespustená) tak priečinok vôbec nevznikol. Appka teraz zapíše prvý riadok
  (`"start"`) hneď pri spustení, nezávisle od toho, čo hráč stlačí; pribudol
  aj kontext `"monitor"` (hra nájdená / primárny monitor / nič).

## 0.18.2 — 17.10.2026 (TLI 25, mini-panel po druhý raz)

Po 0.18.1 vyzeral mini-panel na snímke z CI tak, že v ňom chýba hodnota **FE/h**
aj **ceny dropov**. Dve verzie sa to hľadalo v CSS — a **chyba tam nebola**:
Edge headless má minimálnu šírku okna (~500 px) a menšie `--window-size` ticho
ignoruje, takže sa panel vykreslil 500 px široký a jeho pravý okraj padol mimo
snímku. Appka bola celý čas v poriadku (`ui/mini.js` spustené nad vlastným DOM
shimom vracia všetky tri hodnoty aj všetky ceny). Opravený je **náhľad**, nie
appka: `nahlad-generuj.py` drží šírku stránky na 280 px a okno v CI je väčšie.

Pri tom hľadaní sa aj tak dve veci zlepšili a ostávajú:

- **Horný riadok (zisk/čas/FE/h) aj riadky dropov kreslí CSS grid, nie flex.**
  Každá hodnota má vlastný stĺpec a dlhší text sa oreže v ňom, namiesto aby
  vytlačil susedov. Pri úzkom paneli je to predvídateľnejšie.
- **Ikona dropu je vnútri bunky s menom**, nie vo vlastnom stĺpci — keď sa
  nenačíta, riadky sa už nerozídu.

## 0.18.1 — 17.10.2026 (TLI 25, oprava mini-panela)

Náhľad z CI ukázal, čo testy ukázať nevedeli: mini-panel má vo verzii 0.18.0
okno z čias, keď v ňom boli len tri čísla (260×92 px), takže nový zoznam
„čo padlo" ho pretiekol a **vytlačil z neho hodnotu FE/h**.

- Predvolené okno mini-panela je **280×196 px** a zoznam dropov je jediná časť,
  ktorá sa pri zmenšení panela oreže — čísla nad ním ostanú.
- Kto si mini-panel posunul alebo zmenšil ešte pred 0.18.0, má uloženú vlastnú
  veľkosť a tá sa mu **nepremaže**; zväčší si ho Ctrl+F8.
- Náhľad výberu nástroja a prepínača modulov pribudol medzi snímky z CI —
  obe nové obrazovky sa tak kontrolujú okom, nie len testami.

## 0.18.0 — 17.10.2026 (TLI 25 — výber „Torture Instrument" na F8)

Úloha `2026-10-17-1242-mcp.md`. Sezóna SS14 začína obrazovkou **Select a Torture
Instrument**: tri kartičky, z ktorých si hráč vyberie jednu a späť sa to vziať nedá.

- **F8 na tejto obrazovke konečne niečo robí.** Nad každou kartičkou je rámik
  s tierom (S/A/B/C), poradím 1–3 a jednou vetou prečo po slovensky;
  odporúčaná je zlatá a hrubá. Netreba na to novú klávesu ani prepínač —
  server z jedného volania modelu sám pozná, či pred sebou máš dosku alebo
  výber nástroja (pole `obrazovka` v odpovedi). Nástroj, ktorý server nepozná,
  sa **nikdy neodporučí**: ide na koniec poradia, kreslí sa sivo a zníži
  celkovú istotu.
- **Tier list má pod sebou dve nezávislé merania**, nie dojem: vlastnú
  simuláciu 8 000 celých 20-dňových drilov na každý z 31 nástrojov
  a zverejnené výsledky fanúšikovského simulátora (22 mil. behov). Kde si
  odporujú, je pri nástroji napísané „sporné" — nezhoda sa nespriemerovala
  do ticha. Celý zoznam s vysvetlením je na `tli.tvojtatko.sk/nastroje`.
- **Odporúčanie berie do úvahy, čo už máš.** Niektoré dvojice sú oveľa silnejšie
  než súčet ich častí (`Squirming Iron Box` je sama najhoršia karta v hre, ale
  k `Skewered Rat Heads` patrí k najlepším dvojiciam sezóny) — appka to vie
  z nameranej synergie a napíše to do dôvodu.
- **Appka sa učí z toho, ako to dopadlo.** Každá F8 snímka výberu sa uloží,
  a keď na ďalšej snímke dosky pribudne v lište nový nástroj, server si sám
  zapíše, ktorý si vzal a koľko Chromatic väzňov ten dril nakoniec dal.
  Nič nemusíš klikať. Reálne výsledky partie postupne prebíjajú simuláciu.
- **Obrazovka, ktorej appka nerozumie, sa už nezahodí** — uloží sa a objaví sa
  v nočnom zozname v `#torchlight`, aby sa dala podporiť.
- **Mini-panel ukazuje „čo padlo"** — najhodnotnejšie dropy aktuálnej mapy
  s ikonou, počtom a cenou; po návrate do hubu ostane zoznam z poslednej mapy.
  Položka bez známej ceny sa píše „bez ceny", nie „0 FE". Ikony chodia z
  `tli.tvojtatko.sk`, nie z cudzieho CDN — z tvojho počítača nikam inam nič
  nejde.
- **Ctrl+F8 posúva každý modul zvlášť.** Doteraz mali F8 doska, F8 výber
  nástroja, F9 Canvas aj Artistry jednu spoločnú polohu panela; teraz je
  v náhľade prepínač a každý si drží svoju. Kto mal rozloženie uložené pred
  touto verziou, nájde ho po aktualizácii tam, kde ho nechal.
- **Hlavné okno appky si pamätá, kde a aké veľké bolo.** Dovtedy sa pri každom
  otvorení vycentrovalo.
- *Zvýraznenia nad hrou (rámiky ciel, rámiky kartičiek) sa posúvať NEDAJÚ
  a nikdy nebudú* — sedia presne na tom, na čo ukazujú; keby sa dali posunúť,
  ukazovali by vedľa.

## 0.17.2 — 17.10.2026 (TLI 24 — F8 „nič nespraví" u Richarda)

Vyšetrenie `2026-10-17-1238-mcp.md`: na serveri (CT 114) neprišiel za 17.10.
ani jeden `POST /api/analyza` — appka teda zlyhala skôr, než stihla čokoľvek
poslať, a dovtedy nemala ŽIADEN spôsob, ako to ukázať alebo zapísať.

- **Lokálny rotovaný denník appky** (`%APPDATA%\sk.tvojtatko.tli-overlay\logs\`,
  5× 2 MB) — zapisuje hotkey, nájdené/nenájdené okno hry, snímku, HTTP
  požiadavky aj kolá uploadu logu. Tlačidlo „Otvoriť log" v Nastaveniach.
  Rozhodovacia logika rotácie je v `core/src/denik.rs` (jednotkové testy).
- **Zámok `beh` (ochrana proti dvojitému F8) je teraz jediné miesto, kde appka
  vedela úplne potichu „nič nespraviť"** — keby raz zostal `true` navždy
  (napr. nečakaná panika vo vlákne analýzy pred jeho uvoľnením), KAŽDÝ ďalší
  F8/F9 by mlčal presne takto. Celé telo vlákna je teraz v `catch_unwind`:
  pri panike appka ukáže chybu a zámok vždy uvoľní, namiesto ticha navždy.
- **Oprava: upload logu (`tracker_upload_stav`) bol v DB prázdny aj v 0.17.1**
  — kolo uploadu aj synchronizácia cien partie boli omylom za tým istým
  `continue`, ktorý závisel od farm trackera (`tracker_zapnuty` + nájdený
  herný log). Keď farm tracker nebol zapnutý alebo log ešte nenašiel, upload
  sa nikdy nespustil, hoci je to vlastné, nezávislé nastavenie. Obe teraz
  bežia na vlastnom časovači nezávisle od farm trackera.
  (`src-tauri/src/tracker.rs::spusti_vlakno`)
- **Párovanie okna hry je teraz PRIMÁRNE podľa mena procesu**
  (`torchlight_infinite.exe`), titulok okna („Torchlight: Infinite") je už len
  záloha pre prípad, že sa meno procesu nedá zistiť. Titulok sa vie medzi
  verziami hry zmeniť a appka ho vidí rovnako ako titulky iných bežiacich
  programov (Overwolf, AION 2, launcher).
- **Vždy viditeľná spätná väzba po F8/F9** — pribudla samostatná fáza
  „Odoslané, čakám na radu…" medzi snímkou („Snímam…") a výsledkom/chybou;
  predtým bola jedna spoločná fáza pre oba kroky.

## 0.16.0 — 16.10.2026 (TLI 19 dokončenie + TLI 14)

**TLI 19 akceptačné kritérium OVERENÉ nad reálnym celodenným logom.** Richard
16.10.2026 skopíroval celý `UE_game.log` session 07:35–10:08 (200 MB, 1,69 mil.
riadkov, zazipovaný kvôli limitu WebDAV) na zdieľaný disk. Spätný import +
dump nad ním ukázal, že **všetkých 8 máp s beaconom (08:29–08:51) má presne
zadaný `LevelUid`** (1001001/1001004/1001005/1001000/1011004/1011001/1011000/
1011002), **boss YongShuangBingPo má `LevelUid 1001099`/`LevelId 3000` a typ
boss** (`podpis_vstupov = "boss-alebo-pinnacle"`), kampaňové mapy ostávajú
kampaň (`bez-vstupu`) — opravy A/B/C/E z `tli-v0.14.0` fungujú presne tak, ako
mali, aj nad celou 2,5-hodinovou session, nie len nad úryvkom.

- **Bug D (nerozpoznaný hub) sa v tomto logu NEPREJAVIL ani raz.** Nad všetkými
  188 behmi tejto session nie je jediný prípad, že by vstup skončil na riadku,
  ktorý je `je_hub() == true` — hub `XZ_YuJinZhiXiBiNanSuo200` (zápis
  `yujinzhixibinansuo`) sa rozpoznal správne pri KAŽDOM návrate do mesta. Jediná
  neštandardná cesta v logu (`/Game/Art/Season/S16/Maps/ZhongZuiDiYu/ZhongZuiDiYu`,
  pravdepodobne tohtosezónna väzenská mechanika — hudba `Play_Mus_Gameplay_
  S16_Prison_Card`, skutoční nepriatelia umierajú vnútri) je a má byť `je_hub()
  == false`, lebo ide o skutočnú mapu s korisťou, nie o hub. Záver: hypotéza
  „nerozpoznaná cesta hubu tejto sezóny" sa týmto konkrétnym logom NEPOTVRDILA —
  buď sa Richardovo pozorovanie z 16.10. ~10:55 týkalo inej (neskoršej/inej)
  časti session, než akú má tento zip, alebo problém odvtedy zmizol vďaka
  opravám A/B/C/E (nesprávne `LevelUid` z bugu C mohlo pri pohľade na UI
  vyzerať ako „vstup pripísaný hubu"). Bez ďalšieho konkrétneho prípadu sa
  `je_hub()` nemení naslepo — zostáva otvorené v `BACKLOG.md`, zúžené o tento
  nález.
- **Mená MÁP (zadanie bod `PREKLAD_MAPY`) zostávajú NEDOPLNENÉ, vedome.** Tento
  log priniesol 69 nových pinyin kódov (`core/src/tracker/stav.rs::je_hub` ich
  nepotrebuje, ale zobrazenie v UI áno). Web search pre jednotlivé kódy
  (overené živo, napr. `YuanGuTongDao`) opäť nenašiel nič — fan guides
  vymenúvajú anglické mená zón (Netherrealm, God of Hunting Arena…), ale nie
  jednotlivé interné kódy levelov. Vymýšľať preklad z pinyinu bez obrázka by
  znamenalo ukázať Richardovi NEOVERENÚ vec ako hotovú — presne to si TLI 17
  chcelo vyhnúť zavedením čitateľného rozpadu (`citatelnyKodMapy`), ktorý
  funguje aj naďalej. Zoznam všetkých 69 kódov je v `BACKLOG.md`, čaká na
  Gemini nad screenshotom/videom s viditeľným menom mapy.

**TLI 14 — klientsky upload logov, konečne zapojený.** Server (`POST /api/tracker/upload`)
bol hotový od `2026-10-15-2129-mcp.md`, appka ho ale nikdy nevolala — Richard
musel log kopírovať ručne (a pri väčšom logu to ani nešlo, WebDAV má limit
50 MB na súbor). Od tejto verzie appka log nahráva **sama, po kúskoch, na
pozadí**:

- **Nikdy netreba celý súbor v pamäti.** `core/src/tracker/upload.rs` bol
  prepísaný tak, aby pracoval len s OKNAMI: rozpoznanie rotácie potrebuje len
  prvých ≤256 B súboru, kúsok na odoslanie je najviac `KUSOK_BAJTOV` (8 MiB).
  200 MB log by appku predtým prinútil držať celý obsah v RAM pri každom kole.
- **Gzip po kúskoch, nie celý súbor naraz** (presne znenie zadania
  `2026-10-16-1208-mcp.md`) — server limituje gzip telo na 16 MB a rozbalený
  obsah na 128 MB; appka preto posiela postupné 8 MiB surové kúsky (á 10 minút,
  `upload::INTERVAL_S`, hneď aj po štarte appky), nikdy jeden obrovský POST.
  Veľký nedohnaný rozdiel (appka dlho vypnutá) sa rozloží na viacero kôl
  automaticky.
- **Stav uploadu PRETRVÁVA reštart appky** — nová tabuľka `tracker_upload_stav`
  v lokálnej DB (schéma v4). Bez perzistencie by appka po reštarte poslala
  `pozicia_od = 0` nad súborom, ktorý server už čiastočne má, a server by to
  vyhodnotil ako rotáciu a existujúci log zbytočne archivoval.
  Multipart telo (server čaká `multipart/form-data`) je poskladané ručne v
  `src-tauri/src/klient.rs` — jednoduchý a stabilný formát, appka preň
  nepridáva novú závislosť.
- **Predvolene ZAPNUTÉ** (na rozdiel od `tracker_sync`/`mini_overlay`) —
  nejde o nové zdieľanie niečoho osobného s partiou, len o náhradu ručného
  kopírovania, ktoré Richard aj tak robí. Nový prepínač „Nahrávať log na
  server" v záložke Tracker + tlačidlo „Nahrať log teraz".
  DB export (`db::export_pre_upload`, iný typ uploadu) ostáva zatiaľ len
  ručný (`db_export_teraz`) — automatický denný export čaká na nočné
  serverové vyhodnotenie (TLI 14, časť 3, `BACKLOG.md`), ktoré ešte nebeží.
- Testy: `upload.rs` prepísaný na okná (testy prerobené na nový tvar API,
  pribudol regresný test na delenie veľkého rozdielu na kúsky), 4 nové testy
  perzistencie v `db.rs` (302 testov jadra spolu), 2 nové testy
  `src-tauri/src/klient.rs` (gzip round-trip, multipart telo) — `cargo check
  --tests` nad `src-tauri` čisté (CI testy aplikačnej vrstvy bežia na
  Windowse, lokálne na CT 111 sa z pravidla o cargo test na src-tauri
  nespúšťajú).

## 0.15.0 — 16.10.2026 (TLI 20)

**Ceny bez ručnej práce.** Richard ceny ručne aktualizovať nebude a doteraz mal tabuľku
`ceny` prázdnu a všetky behy za 0 FE. Táto verzia rieši celú cestu od vyhľadania na
in-game Exchange po ocenenie behu u všetkých troch v partii.

- **Z jedného vyhľadania sa drží viac než jedno číslo.** Doteraz sa z dvojice správ
  `XchgSearchPrice` brala len p10 (10. percentil ponúk) a zvyšok sa zahodil. Teraz sa
  ukladá aj **medián**, **počet ponúk** a **sezóna** — medián je odolnejší voči jednej
  extrémne drahej ponuke a počet ponúk hovorí, nakoľko sa dá cene veriť.
- **⚠️ Oprava, ktorá by inak ticho vypla ceny úplne.** Druhá cenová skupina sa v reálnom
  logu zapisuje SKRÁTENE, bez zopakovania slova `prices` (`|      +2+currency [100200]`).
  Parser taký riadok nerozpoznal, nechal index skupiny na predošlej hodnote, mena druhej
  skupiny tak prepísala menu prvej a **všetky FE ponuky sa odfiltrovali ako „iná mena"**.
  Tvar je doložený vzorkami reálnych logov z 26./28.1.2026 (testy TITracku) a je na neho
  regresný test. Pokrytý je aj „nový" tvar, kde `+currency` stojí AŽ ZA zoznamom ponúk.
- **Ceny starnú.** Do 24 h má cena plnú váhu, potom lineárne klesá na 25 % počas siedmich
  dní a nižšie už nie — stará cena je lepší odhad než žiadna. Váha sa používa pri
  rozhodovaní, ktorá cena vyhrá: **čerstvá cena od partie prebije týždeň starú vlastnú**,
  čerstvú vlastnú neprebije. Dovtedy platilo tvrdé „menej dôveryhodný zdroj neprepíše
  nikdy", takže stará vlastná cena blokovala čerstvú partiovú navždy.
- **Zdieľanie cien s partiou (predvolene ZAPNUTÉ).** Klient posiela rozdielovo len ceny
  z vlastného vyhľadania (`POST /api/ceny`) a sťahuje agregát partie
  (`GET /api/ceny?od=`) á 5 minút. Server počíta **medián cez hráčov** z ich najnovšej
  ceny na položku (päť vyhľadaní toho istého hráča = jeden hlas) a pripisuje **istotu**
  `overena`/`dobra`/`slaba` podľa počtu nezávislých zdrojov. Cena od partie sa späť
  neodosiela — inak by si agregát potvrdzoval sám seba.
- **Offline cache.** Cenník sa po štarte appky postaví z lokálnej DB. Dovtedy mal hráč po
  každom reštarte prázdny cenník a behy za 0 FE, kým si znova niečo nevyhľadal.
- **Spätný import starých logov konečne oceňuje behy.** Druhý prechod: po dočítaní súboru
  sa z nájdených cien postaví cenník a dopočíta sa hodnota behov, ktoré sa ukladali skôr,
  než bola cena známa. Vek ceny sa pritom berie z **herného času riadku**, nie z „teraz" —
  inak by mesiac starý log vyzeral ako čerstvé ceny. Beh, ktorého položky cenník nepozná
  ani potom, ostane **bez hodnoty (`NULL`), nie na nule**.
- **Výzva „vyhľadaj na Exchange".** Záložka Tracker ukáže, koľko položiek z posledných máp
  nemá cenu mladšiu ako 24 h, a vymenuje dve-tri najcennejšie. Je to **návod pre človeka** —
  appka do hry neposiela nič a ani nemôže.
- **Mená položiek namiesto `#400021`.** Nový balík `ui/polozky-seed-en.json` (1 933
  položiek) prevzatý z projektu TITrack pod licenciou MIT; rozsah a atribúcia sú v novom
  súbore [`NOTICE.md`](NOTICE.md). Pokrýva aj tohtosezónne beacony `400000`/`400001`, ktoré
  staršia tabuľka `en_id_table.json` nemala. ID ostáva v `title` odznaku.
- **Na komunitné ceny TITracku sa NENAPÁJAME.** Ich crowd backend je Supabase projekt
  autora s natvrdo zapísaným kľúčom v zdrojáku, bez verejného API a bez podmienok použitia
  pre tretie strany — podrobne v
  `docs/fakty/hry/torchlight-infinite/README.md` (zápis 16.10.2026).

Testy: jadro 297 + 12 integračných (bolo 288 + 12), UI jsdom 7 (bolo 4), `tli-web` 97 (bolo 84).

## 0.14.0 — 16.10.2026 (TLI 19, 1/N)

Opravy parsera/trackera nad REÁLNYM Richardovým živým logom (overené priamo na jeho PC
16.10.2026, zadanie `2026-10-16-1055-mcp-2.md`) — tracker.sqlite3 behy 97–111 mali štyri
samostatné chyby:

- **Bug A — spotreba PO prechode do mapy sa strácala.** `BagMgr@:Modfy` niekedy dobehne do
  logu AŽ PO `SceneLevelMgr@` prechode do mapy, ktorú otvoril (poradie sa môže líšiť od behu
  k behu). Doteraz sa spotreba vtedy nikdy nedostala do behu, ktorý už bežal, a beh ostal
  navždy „bez-vstupu". `Sledovac::zaznamenaj_zmenu` teraz priraďuje priamo bežiacemu
  (nehubovému, čerstvému) behu, keď spotreba dobehne až počas neho.
- **Bug B — boss/pinnacle bez spotreby sa zobrazoval ako „kampaň".** `Spv3Open`/
  `ClimbTowerOpen` sa pri bossoch/pinnacle mapách spustí BEZ zmeny batohu (žiadny chart ani
  kompas), čo predtým vyzeralo rovnako ako skutočná kampaň. `Beh` má nové pole
  `otvorenie_bez_spotreby` a `podpis_vstupov()` vracia `"boss-alebo-pinnacle"` namiesto
  `"bez-vstupu"`, keď sa mapa otvárala, ale nič sa pritom nespotrebovalo — `"bez-vstupu"`
  (zobrazené v UI ako „kampaň") ostáva vyhradené pre behy, kde sa `Spv3Open`/`ClimbTowerOpen`
  nespustil VÔBEC.
- **Bug C — `LevelUid`/`LevelType`/`LevelId` boli posunuté o jeden beh.** `LevelMgr@` prichádza
  TESNE PRED prechodom do novej zóny (nie počas nej) — priamy zápis do `aktualny_beh` preto
  vždy trafil PREDOŠLÝ, ešte neuzavretý beh. Identita úrovne sa teraz drží ako čakajúca
  (`Sledovac::cakajuca_uroven`) a odovzdá sa až behu, ktorý naozaj vznikne pri najbližšom
  prechode zóny.
- **Bug E — `player_id`/`season_id` prázdne vo všetkých živých behoch.** „Enable Log" sa
  zapína na začiatku KAŽDEJ session a identita (`GetPlayerData`/`+player+...`) chodí pri
  PRIHLÁSENÍ — teda typicky PRED tým, než ju hráč stihne zapnúť; živé čítanie preto v danej
  session nemusí vidieť identitu vôbec (spätný import to nemá, lebo vidí logy od úplného
  začiatku). Nová `db::posledna_znama_identita()` je záloha: keď `Sledovac::hrac_snimka()`
  nemá nič, `uloz_beh_do_db` použije poslednú známu identitu z tej istej lokálnej DB.
- **Bug D (ČIASTOČNE OTVORENÉ)** — posledný vstup bol priradený hub riadku namiesto
  nasledujúcej mapy; koreňová príčina je nerozpoznaná cesta tohtosezónneho hubu v `je_hub()`.
  Bez reálneho prihrávaného logu (pozri nižšie) sa nedalo bezpečne uhádnuť, akú cestu doplniť —
  detail a ďalší krok v `BACKLOG.md`.

Nové regresné testy priamo nad vzorom z Richardovho 16.10. zistenia (`core/src/tracker/stav.rs`,
`tli19_*`, aj dva `db.rs::posledna_znama_identita_*`) a dva integračné nad `uloz_beh_do_db`
(`src-tauri/src/tracker.rs`) — `cargo test --lib` v `core`: 277 zelených (bolo 270),
`cargo check --tests` v `src-tauri` čisté (testy v `src-tauri` sa spúšťajú len v CI na
Windows, nie tu — pozri `[[feedback-tli-cargo-test-disk]]`).

**Mapy a položky (zadanie body 2–3) nie sú v tomto release** — pinyin kódy máp sa cez
websearch nedajú spárovať s anglickými menami (nie sú nikde indexované) a ikony položiek
vyžadujú samostatnú sťahovaciu/cachovaciu infraštruktúru; nájdený zdroj
(`TLI-Tracker-main/en_id_table.json` na zdieľanom disku, 240 položiek, MIT) je zdokumentovaný
v `BACKLOG.md` ako štartovací bod ďalšej časti TLI 19.

**Dnešný live log stále nie je na zdieľanom disku** (len včerajšia, 15.10. kópia) — Richard
dostal správu na Discord, nech ho skopíruje, aby sa dali tieto opravy overiť nad SKUTOČNÝM
dnešným logom (akceptačné kritérium zo zadania).

## 0.13.1 — 16.10.2026 (hotfix)

- **Opravené: hlavné záložky (Solvery/Analýza/Guide/Nastavenia) nereagovali na
  klik.** `ui/hlavne.js` mal dve top-level deklarácie `function prepni(...)` —
  prepínanie záložiek a ukladanie prepínačov (mini overlay/sync/pripomienka).
  Súbor je obyčajný klasický `<script>`, nie modul, takže druhá deklarácia tú
  prvú v behu ticho prepísala. Klik na záložku tak skončil ako
  `invoke("uloz_prepinac", { meno: <DOM element>, zapnuty: undefined })` a
  appka to odmietla vetou „invalid type: map, expected a string" — fungovala
  len záložka Tracker (tá bola aktívna hneď pri štarte, bez potreby kliku).
  Prepínanie záložiek je teraz `prepniZalozku(tlacidlo)`, meno jasne odlíšené
  od `prepni(meno, zapnuty)`.
- Nový E2E test `ui/testy-zalozky.e2e.mjs` (`node --test`, jsdom, nová CI úloha
  `ui_e2e`): simuluje reálny klik na KAŽDÚ záložku nad ostrým `hlavne.html`/
  `hlavne.js` a overuje, že (a) sa prepne správny panel, (b) klik na záložku
  nikdy nezavolá `uloz_prepinac`, (c) `uloz_prepinac` dostáva string `meno` a
  boolean `zapnuty`, nikdy DOM element. Existujúci CI náhľad (screenshoty
  záložiek) tento bug chytiť nevedel — aktívnu záložku prepína priamo v HTML,
  JS klik sa tam nespúšťa.
- **Behy bez spotrebovaného chartu/kompasu („bez vstupu") sa teraz označujú
  „kampaň".** Endgame mapu nejde otvoriť inak než chartom/kompasom
  (`Spv3Open`/`ClimbTowerOpen`) — beh bez nich je vždy kampaňová/sezónna/
  leveling zóna, nikdy „mapa zadarmo". Overené na Richardovom živom
  `UE_game.log` (zdieľaný disk, session z 14.10.2026 večer): 13 pozorovaných
  behov bolo PRESNE takýchto, všetky reálne kampaňové/sezónne zóny (napr.
  `SD_ShouGuSiDi000`, `DD_ZaWuJieQu000`, `SQ_MingShaJuLuo100`,
  `Season/S8/.../XueYuRongLu_*`). Konkrétne kódy máp z Discord zadania
  (`XZ_YuJinZhiXi200`, `SD_YuanGuTongDao201`, `SD_ShangGuZhanChang301`) sa
  v žiadnom dostupnom logu nenašli doslovne (najbližšia reálna zhoda je
  `XZ_YuJinZhiXiBiNanSuo200`) — pravidlo je preto odvodené zo skutočného
  správania hry (chart/kompas = jediný spôsob vstupu do mapy), nie z
  konkrétneho zoznamu kódov.
- **Mapy sa zobrazujú čitateľnejšie, nie ako surový engine kód.** Nová
  `PREKLAD_MAPY` v `ui/hlavne.js` s overenými menami (zatiaľ len
  `YuJinZhiXiBiNanSuo` → „Ember's Rest", zdroj
  `docs/fakty/hry/torchlight-infinite/log-format.md`); pre ostatné kódy sa
  oblastný prefix a koncové číslo odstránia a zvyšok (pinyin) sa rozdelí na
  slová podľa veľkých písmen (`ShouGuSiDi` → „Shou Gu Si Di") — nie je to
  preklad, len čitateľnejší zápis, kým nepribudnú ďalšie overené mená.
- Debug nástroj `core/examples/zony_dump.rs` (`cargo run --example
  zony_dump -- <log>`) — vypíše zónu/level_typ/podpis vstupov pre každý beh
  zo zadaného logu; použitý na overenie vyššie, ponechaný pre budúce otázky
  o konkrétnych mapách.

## 0.13.0 — 16.10.2026

Analýza podľa taktiky (TLI 13b, 1. časť, doplnok k TLI 13).

- **Taktika** = podpis celého setupu behu: štartovací chart a kompasy, mapa
  a mód, modifikátory mapy a build (hrdina, pact konfigurácia, talentové
  stromy). Appka behy do taktík zoskupí sama; hráč môže skupinu **pomenovať**
  alebo **zlúčiť** viac podpisov do jednej (nové tabuľky `taktiky`/
  `taktiky_priradenie`, schéma DB v2).
- Nová sekcia **„Taktiky"** v záložke Analýza: tabuľka zoradená podľa
  priemerného FE/h (= rebríček „najlepšia taktika za hodinu"), s počtom
  behov, priemerným aj mediánovým ziskom, min/max, p10/p90, smerodajnou
  odchýlkou (rozptyl — stabilná taktika vs. „lotéria"), priemerným nákladom
  na beh a ROI, a pomerom zisk/riziko (rebríček „najlepší pomer zisk/riziko").
  Taktiky s menej než 5 ocenenými behmi sú zreteľne označené „málo dát".
- Panel **„Odporúčanie"**: deterministicky (žiadne AI) vyberie najlepšiu
  taktiku spomedzi tých s dosť dátami, s dôvodom po slovensky. Vedľa neho
  zoznam „čo vyskúšať viac" — taktiky s málo dátami, ktoré sa oplatí
  otestovať (odvodené z reálnych dát appky, nie z katalógu kombinácií, ktorý
  appka nemá — BACKLOG.md).
- Export CSV a JSON zoznamu taktík.
- Nový core modul `tracker::taktiky` (24 nových unit testov) — čisto, bez
  DB, rovnaký vzor ako `agregacie.rs`.
- **Čo v tejto časti ZÁMERNE chýba** (BACKLOG.md „TLI 13b — …"): rozpad zisku
  podľa zdroja (mobky/rares/boss/mechanika/truhlice — log zatiaľ nedáva
  spoľahlivo rozlíšiť zdroj dropu), graf vývoja FE/h v čase, prepočet starých
  behov podľa DNEŠNÝCH cien, filter podľa obdobia/hrdinu/ligy v UI (server už
  pripravený, `db::FilterTaktik`), voliteľný sync agregátov na
  `tli.tvojtatko.sk`, Artistry/Snowpaper tier a sezónne mechaniky (Tartarus,
  Followers) v podpise taktiky.

## 0.12.0 — 16.10.2026

Lokálna SQLite databáza všetkého, čo sa dá z logu vyčítať, a záložka
„Analýza" (TLI 13, 1. časť). Tento záznam pôvodne v `CHANGELOG.md` chýbal
(release aj `latest-tli.json` vyšli, zápis sem sa vynechal) — doplnené
dodatočne úlohou TLI 13b, obsah nemenený, len dopísaný.

- Nová lokálna databáza (`tracker::db`, SQLite cez `rusqlite`/`bundled`):
  behy, položky, monštrá, modifikátory mapy, ceny, zostatky, bossovia,
  Frozen Canvas akcie, build snapshoty a surový catch-all pre nerozobrané
  riadky. Zápisy idempotentné — live čítanie aj spätný import sa dajú volať
  opakovane bez duplicít.
- Nové typy udalostí v parseri: spawny monštier s raritou a affixom, textové
  modifikátory mapy, zostatky mien, stav bossov, pact konfigurácia, Frozen
  Canvas akcie priamo z logu.
- Tlačidlo **„Načítať staré logy"** — spätný import aktuálneho aj
  `UE_game-backup-*.log` súborov, overené na reálnom 19 MB logu za 115 ms.
- Nová záložka **„Analýza"**: vyhľadávanie podľa mapy a minimálneho zisku,
  detail behu (monštrá podľa rarity), export CSV a JSON.
- Appka naďalej iba číta súbor na disku, nič v hre nemení.

## 0.11.0 — 16.10.2026

Pripomienka „Enable Log" na začiatku každej session (TLI 11d, doplnok k TLI 11c).

- Hra si nastavenie Settings → Other Settings → „Enable Log" NEPAMÄTÁ medzi
  spusteniami — appka teraz na to sama upozorní namiesto toho, aby sa to
  zistilo až spätne, keď je neskoro.
- **Nová session** sa pozná podľa rotácie `UE_game.log` (hra ho pri reštarte
  premenuje/skráti) — vtedy sa lehota na zapnutie logovania počíta odznova.
- **Do 60 sekúnd** má appka vidieť aspoň jeden riadok, ktorý sa v logu objaví
  LEN keď je logovanie zapnuté (zmena batohu, `ItemChange@`/`Spv3Open`).
  Prechod medzi mapami (`SceneLevelMgr@`) sa loguje nezávisle od tohto
  nastavenia — appka to využíva na **druhé pripomenutie presne pri vstupe
  do prvej mapy**, keď hráč prvé upozornenie prehliadol.
- **Tri súbežné upozornenia**: toast priamo nad hrou (zelený/oranžový rámik
  podľa stavu), Windows notifikácia a červený stav v záložke Tracker — ten
  istý „živý pás", čo inak ukazuje bežiacu mapu. Všetky zmiznú samé, len čo
  začnú prichádzať herné riadky („✅ Logovanie beží").
- Nový prepínač **„Pripomínať Enable Log"** v záložke Tracker, predvolene
  zapnutý (na rozdiel od ostatných prepínačov tejto záložky).
- Nový core modul `tracker::pripomienka` — celý stavový automat nad
  injektovaným unixovým časom, žiadne reálne čakanie v testoch.

## 0.10.0 — 15.10.2026

Tracker si sám nájde log a číta ho naživo (TLI 11c).

- **Automatická detekcia cesty**: appka prečíta Steam inštaláciu z registra
  (`HKCU\Software\Valve\Steam`, záložne `HKLM`) a zo `steamapps/libraryfolders.vdf`
  prejde všetky knižnice, kým nenájde `UE_game.log`. Keď Steam nenájde nič,
  padá na krátky zoznam bežných ciest (ako doteraz). Záložka Tracker má nové
  pole „Cesta k logu" — predvyplnené nájdenou cestou, zmena len cez tlačidlo
  „Vybrať…" (natívny výber súboru), a stavový riadok s ikonou: ✅ (veľkosť,
  kedy sa naposledy zmenil), alebo ⚠️ keď súbor neexistuje, je skoro prázdny,
  alebo v ňom chýbajú očakávané záznamy (návod zapnúť v hre
  Settings → Other Settings → „Enable Log").
- **Živé čítanie na pozadí, nezávislé od okien**: appka doteraz čítala log len
  vtedy, keď bolo otvorené hlavné okno alebo mini-panel a ich JS si o dáta
  samo povedalo (`setInterval`). Teraz beží vlastné vlákno od štartu appky —
  sleduje priečinok s logom cez `notify` (reakcia pod sekundu) a záložne
  dopĺňa pollingom raz za sekundu, takže tracker funguje aj schovaný v tray.
  Okná dostanú udalosť `tracker-zmena` a prekreslia sa okamžite namiesto
  čakania na vlastný interval.
- **Pozícia v logu sa ukladá** (`tracker-pozicia.json` vedľa `nastavenia.json`)
  — appka po reštarte nemusí reparsovať od začiatku aj to, čo už raz videla.
  Keď sa uložená cesta nezhoduje s aktuálnou (iná hra/iný disk), číta sa od
  nuly, nie „pokračuje" v cudzom súbore.
- **Oprava kódovania naprieč dávkami**: appka číta log po kúskoch (2 MB), ale
  UTF-16 BOM je v súbore len raz na úplnom začiatku. Pôvodný kód si kódovanie
  nepamätal medzi dávkami, takže pri logu s UTF-16 (keď sa doň dostal znak
  mimo ANSI, napr. emoji v mene postavy) sa po prvej dávke dekódovalo zle.
  Teraz sa kódovanie zistí raz a platí pre celý súbor; rieši aj prípad, keď
  dávka skončí uprostred 2-bajtovej jednotky.
- Rotácia (hra pri reštarte log premenuje/skráti) bola otestovaná — predtým
  mala appka logiku, ale ani jeden test.
- Nové core moduly: `tracker::steam` (parsovanie `libraryfolders.vdf`,
  skladanie kandidátnych ciest — testované na Linuxe s falošnými hodnotami)
  a `tracker::chvost` (kódovanie a delenie na riadky naprieč dávkami, tiež
  bez dotyku s diskom).

## 0.9.0 — 15.10.2026

Synchronizácia s partiou a mini-overlay počas mapy (TLI 11, krok 5/5 — posledný).

- **Posielanie výsledkov partii, predvolene VYPNUTÉ.** Prepínač „Posielať výsledky
  partii" v záložke Tracker; keď je zapnutý, odfarmené mapy idú na
  `tli.tvojtatko.sk` a objavia sa v rebríčku na **`/tracker`** (kto koľko zarobí
  za hodinu a ktorá kombinácia chart + kompasy + mapa sa oplatí). Bez zapnutia
  neodíde zo stroja **nič**.
- Odosiela sa len **výsledok mapy** (zóna, podpis chartu a kompasov, trvanie,
  hodnota dropov/vstupov, čistý zisk) — žiadny log, žiadne screenshoty, žiadne
  zoznamy položiek.
- Každá mapa má **stabilný podpis** (FNV-1a nad časom začiatku, zónou, kombináciou
  a trvaním), takže po reštarte appky sa tá istá mapa pošle znova a server ju
  podľa toho istého podpisu ticho zahodí — v rebríčku sa nezdvojí.
- Odosiela sa automaticky **raz za tri minúty**, kým je hlavné okno otvorené,
  plus tlačidlom „Poslať teraz". Dávka je zhora obmedzená na 200 máp.
- **Mini-overlay počas mapy**, tiež predvolene vypnutý: malé priehľadné okno
  260 × 92 px vždy navrchu s čistým ziskom tejto mapy, časom a FE/h. Je
  priechodné pre myš — klikne sa cezeň do hry.
- Server: `POST /api/tracker/sync`, `GET /api/tracker/rebricek` a stránka
  `/tracker` (rola `Torchlight` ako ostatné). **Discord ID sa v rebríčku
  neposiela** — porovnávajú sa výkony, nie identifikátory.
- Nové nastavenia `tracker_sync` a `mini_overlay` (obe `false`); staré
  `nastavenia.json` sa načíta bez zmeny a oba prepínače ostanú vypnuté.

## 0.8.0 — 15.10.2026

Záložka **Guide** — sprievodca Erika 1 priamo v appke (TLI 11, krok 4/5).

- Nový modul `core/src/guide.rs`: parsovanie odpovede servera, lokálna cache,
  príprava HTML na vykreslenie (ikony sa vkladajú ako `data:` URI, `<script>`
  a `<iframe>` sa odstraňujú, externé odkazy dostanú `data-externy="1"`).
- Nový `src-tauri/src/guide.rs`: `guide.json` vedľa `nastavenia.json`
  v `%APPDATA%`, príkazy **`guide_data`** (nič nesťahuje, funguje offline)
  a **`guide_sync`** (pozrie sa na server a stiahne, len keď je novšia verzia).
- Server `tli.tvojtatko.sk` má nový **`GET /api/guide`** (rola `Torchlight` ako
  ostatné API): ťahák + kapitoly ako HTML aj s hernými ikonami vloženými priamo
  v tele. Verzia je **hash obsahu**, nie čas súborov — redeploy bez zmeny textu
  klientom nepovie, že je vonku nový sprievodca, a `?ak_novsie_ako=` vtedy vráti
  prázdnu odpoveď 204.
- **Ikony chodia ako `data:` URI zámerne:** okno má CSP `img-src 'self' data:`,
  takže obrázok ťahaný z cudzej adresy by sa aj tak nenakreslil — a guide takto
  funguje celý aj bez internetu.
- **Appka neotvára cudzie adresy ani odtiaľto.** Odkaz na `tlicompendium.com`
  alebo YouTube sa ukáže v lište pod obsahom a dá sa skopírovať; `otvor_web`
  ostáva obmedzený na náš server a náš repozitár.
- CI: náhľad hlavného okna sa **generuje z ostrého `ui/hlavne.html`**. Do 0.8.0
  to bola ručná kópia (`ui/nahlad-hlavne.html`) a rozišla sa s originálom hneď,
  ako pribudla táto záložka — screenshot by ukazoval staré okno.
- Tracker, F8/F9, overlay ani nastavenia sa **nemenili**.

## 0.7.0 — 15.10.2026

Tracker farmenia: parser `UE_game.log`, ceny z trhu, agregácie (TLI 11 kroky 2 a 3,
celé zadanie TLI 11b). *(Zápis doplnený spätne 15.10.2026 — pri vydaní 0.7.0 sa do
tohto súboru nedostal.)*

- `core/src/tracker/` — `log.rs` (riadky, UTF-16/BOM), `stav.rs` (delty inventára,
  hranice behov, ceny z `XchgSearchPrice`), `ceny.rs` (cenník a vek ceny),
  `agregacie.rs` (súhrny). 106 unit testov + 8 E2E nad vzorkou session s 3 mapami.
- **Čistý zisk = dropy − spotrebované vstupy**; nákup na trhu uprostred mapy sa
  do zisku nepočíta.
- **Štartovací chart a kompasy** sa čítajú zo spotreby v bloku `Spv3Open`;
  výsledky sa delia podľa kombinácie chart + kompasy + mapa.
- **Ceny z vlastných vyhľadávaní hráča na trhu** (len mena FE `100300`,
  10. percentil), pri každej cene sa drží jej vek. Na cudzí server nejde nič.
- Svetlá téma ako **predvolená**, prepínač v hlavičke okna.
- `src-tauri/src/tracker.rs` — hľadanie logu, čítanie po 2 MB dávkach od poslednej
  pozície, rotácia logu, UTF-16 podľa BOM.

## 0.6.0 — 15.10.2026

Kostra hlavného okna so záložkami Tracker/Solvery/Guide/Nastavenia (TLI 11, krok 1/5).

- Nové okno `hlavne` (`ui/hlavne.html`), otvára sa novou položkou **„Otvoriť"** v menu
  tray ikony (nad existujúcou „Nastavenia"). Štyri záložky: **Tracker** (zoznam
  plánovaných metrík, dáta pridá krok 2 — parser `UE_game.log`), **Solvery** (len
  prehľad — Tartarus aj Frozen Canvas bežia ďalej presne tak, ako doteraz, na F8/F9,
  táto záložka im nič nemení), **Guide** (placeholder, obsah pridá krok 4) a
  **Nastavenia** (zatiaľ len tlačidlo, ktoré otvorí existujúce samostatné okno
  nastavení — zlúčenie je mimo rozsahu tohto kroku).
- Nový príkaz `otvor_nastavenia` (zdieľaný s tray handlerom cez `ukaz_a_zaostri`) —
  žiadna zmena správania existujúceho okna nastavení ani jeho príkazov.
- F8/F9 hotkey, overlay, screenshot, posielanie na server — **nič z toho sa nemenilo**.
- Analýza troch cudzích trackerov (TLI 10,
  `docs/fakty/hry/torchlight-infinite/addony-analyza.md` v repe `ops`) potvrdila, že
  dáta sa dajú čítať bezpečne (len z `UE_game.log` na disku, žiadna pamäť procesu,
  injekcia ani pakety) a že kód žiadneho z nich sa nedá legálne prevziať (chýbajúca
  alebo neoverená licencia) — ďalšie kroky preto stavajú vlastný parser od nuly,
  inšpirovaný len vizuálom a zoznamom metrík.

## 0.5.0 — 15.10.2026

Oprava farieb/názvov rarity v paneli s dôvodom (TLI 9b).

- Video 8SESQy9Pe70 + Richardov screenshot ukázali, že skutočná hra má **5 stupňov
  rarity** (White/Blue/Purple/Orange/Chromatic), nie 6 (Normal..Rainbow) a doska je
  **obdĺžniková**, nie kruhová — detail
  `docs/fakty/hry/torchlight-infinite/tartarus-pravidla.md` v repe `ops`.
  Server (`tli-web`) aj engine sú na nový model premigrované.
- `ui/ikony.js`: `TIER_FARBA`/`TIER_SK` skrátené na 5 položiek so správnymi menami —
  predtým mal panel pri tieroch 2-4 PRESUNUTÉ (nesprávne) farby a mená, lebo pole malo
  staré indexy 0-5.
- Siluety väzňov v paneli ZOSTÁVAJÚ jednoduché SVG (nie reálne výrezy ako na webe) —
  appka ich musí vykresliť okamžite aj offline, bez čakania na súbor/sieť; pri ~28 px
  veľkosti sa detail aj tak stratí.
- Zelené rámiky, čísla krokov ani žiadna iná funkcia sa nemenia.

## 0.4.0 — 15.10.2026

Ikony v paneli s dôvodom (TLI 8).

- Nad dôvodom odporúčania je nový riadok s ikonami: **koho sa ťah týka** (portrét väzňa
  v farbe jeho rarity pri Tartarus, štvorček paintu s jeho levelom pri Frozen Canvas)
  a **akú akciu s ním urobiť** (patrola, požretie, výmena, nástroj, dar, položenie na
  plátno). Rovnaký vizuálny jazyk ako doska na `tli.tvojtatko.sk`.
- Ikony sú kreslené v appke ako SVG (`ui/ikony.js`), NIE sťahované zo servera — panel sa
  musí vykresliť aj vtedy, keď server práve nedosiahnuteľný je, a bez requestu navyše.
- Keď sa ikona nedá zostaviť (neznámy väzeň, chýbajúci paint), riadok ostane skrytý —
  panel sa kvôli tomu nikdy nerozbije.
- **Zelené rámiky a čísla krokov sa NEMENIA** — mení sa len panel.
- Vnútorne: `AnalyzaCanvas` prenáša zo servera aj `platno` (bez interpretácie), inak by
  sa paint zdrojového slotu do UI nedostal.

## 0.3.0 — 15.10.2026

Frozen Canvas (TLI 4c) — režim zberu dát a priznanie neoverených pravidiel.

- Nové nastavenie „Zber dát (Frozen Canvas)" (vypnuté predvolene): keď je zapnuté, hotkey
  Frozen Canvas (predvolene F9) nevykreslí radu solvera — len odfotí, pošle na server (ten
  uloží stav VŽDY, bez ohľadu na istotu — nezmenené od 0.2.0) a ukáže počítadlo
  „Zber dát — uložené #N". Žiadny tretí hotkey, nech sa počas hrania netreba učiť novú
  klávesu.
- Keď smer aktívneho cieľa ešte nemá dosť potvrdených párov pred/po (server o tom rozhoduje
  cez nové pole `pravidlo_overene`), nadpis rady dostane prefix „⚠️ " a panel „Neisté" ukáže
  dôvod — solver síce radu dá (implementuje len smery s overeným pravidlom), ale hráč má
  vidieť, že ešte nie je potvrdená dátami.

## 0.2.0 — 15.10.2026

Frozen Canvas (TLI 4b) — druhá, nezávislá mechanika vedľa Tartarus.

- Nová klávesová skratka (predvolene F9, nastaviteľná vedľa F8) pre analýzu obrazovky
  Frozen Canvas — iný endpoint servera (`?modul=canvas`), iný tvar odpovede.
- Overlay vykresľuje SEKVENCIU krokov ČO→KAM: pre každý krok dva zelené rámiky (paint
  v ruke aj cieľová bunka na plátne) s rovnakým číslom v rohu; prvý krok plný a hrubý,
  ďalšie tenšie a priehľadnejšie, nech je poradie jasné na prvý pohľad.
- Panel s dôvodom zobrazuje prvý krok hlavne, ďalšie kroky ako očíslovaný zoznam pod ním —
  rovnaká poloha ako pri Tartarus, nikdy sa neprekrýva s plátnom ani s rukou.
- Solver na serveri rieši VÝHRADNE smer miešania "adjacent" (jediný s overeným presným
  pravidlom z reálnych screenshotov) — iné smery zatiaľ vedome odmietne, kým sa dáta
  nepotvrdia (Richard zbiera dáta 16.10.2026).
- Nastavenia appky majú druhé pole pre hotkey Frozen Canvas, oddelené od Tartarus.

## 0.1.0 — 14.10.2026

Prvé vydanie aplikácie Tvoj Tatko TLI Overlay pre sezónnu mechaniku Tartarus (SS14).

- Pridaná automatická detekcia a robenie snímky okna (GDI BitBlt) klávesovou skratkou F8.
- Pridaná HTTP integrácia s API pre získavanie ťahov (`ureq`).
- Implementované samostatné, priehľadné overlay okno bez rámu, ktoré je priechodné pre myš a zobrazuje odporúčaný ťah, dôvod po slovensky, percentuálnu istotu a 2 alternatívne ťahy.
- Pridané vizuálne zvýraznenie cieľového políčka v hernej doske.
- Podpora pre monitory s upraveným DPI (100 %, 125 %, 150 %) a logika pre výber správneho monitora podľa toho, kde je umiestnené okno hry (rieši problémy na multi-monitor setupoch).
- Pridaný slovenský preklad pre chybové stavy: 401 (zlý kľúč), 403 (chýba rola), 422 (doska nenájdená), 429 (rate limit 30 za minútu).
- Podpora dynamického a bezpečného používania klávesu Esc, ktorý overlay zatvára bez toho, aby trvale narušoval herné skratky (kradnutie inputu hre).
- Implementovaná kontrola exkluzívneho fullscreenu (detekcia čiernej snímky) so zobrazením varovania s radou pre zmenu nastavení hry.
- Implementované statické upozorňovanie na aktualizácie formou prečítania JSON manifestu, bez priamej inštalácie aktualizácie aplikáciou.
- Nasadené prísne bezpečnostné CI kontroly, ktoré v zdrojovom kóde a závislostiach strážia absenciu akýchkoľvek hookov a memory read/write funkcií, čo zaisťuje plný súlad s podmienkami hry.
- Pevne fixovaný WiX UpgradeCode inštalátora v systéme (zabránenie chybného prepisovania od cudzích repozitárov).
