# Zoznam zmien

Projekt využíva sémantické verzovanie (Semantic Versioning). Verzie sú označované v tvare `<major>.<minor>.<patch>` a git tagy na releasy v tvare `tli-v<verzia>`.

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
