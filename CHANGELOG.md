# Zoznam zmien

Projekt využíva sémantické verzovanie (Semantic Versioning). Verzie sú označované v tvare `<major>.<minor>.<patch>` a git tagy na releasy v tvare `tli-v<verzia>`.

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
