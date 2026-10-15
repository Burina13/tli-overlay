# Zoznam zmien

Projekt využíva sémantické verzovanie (Semantic Versioning). Verzie sú označované v tvare `<major>.<minor>.<patch>` a git tagy na releasy v tvare `tli-v<verzia>`.

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
