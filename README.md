# 🛠️ Tvoj Tatko TLI Overlay

Technická dokumentácia k aplikácii pre Torchlight: Infinite (sezóna Tartarus, SS14). 

## Čo to robí
Aplikácia po stlačení klávesovej skratky (predvolene F8) zistí pozíciu okna hry, urobí jeho snímku cez GDI BitBlt a ako JPEG ju odošle na náš server (`POST https://tli.tvojtatko.sk/api/analyza`). Odpoveď servera sa následne vykreslí v samostatnom priehľadnom okne priamo nad hrou. 

Overlay zobrazuje:
- Odporúčaný ťah a zvýraznené cieľové políčko.
- Zdôvodnenie ťahu v slovenčine.
- Istotu výpočtu (%).
- Dve alternatívne možnosti ťahu.

## Pre koho je to
Aplikácia je neverejná, určená výhradne pre našu partiu (Tvoj Tatko). Na jej fungovanie potrebuješ:
1. Rolu **Torchlight** na našom Discorde.
2. Svoj **osobný kľúč** (Bearer token). Získaš ho na adrese `tli.tvojtatko.sk/kluc` alebo priamo na Discorde príkazom `/tli kluc`.

## Inštalácia v 3 krokoch
1. Stiahni si inštalátor (súbor `.msi`) z releasov na GitHube a nainštaluj ho.
2. Spusti aplikáciu. Zobrazí sa tray ikona (pri hodinách). Klikni na ňu, otvor Nastavenia a vlož svoj osobný kľúč.
3. Otvor hru a stlač **F8**.

## Ako sa to používa
Aplikácia beží na pozadí a čaká na klávesovú skratku. 

- **Zobrazenie:** Stlačením F8 (alebo tvojej vlastnej skratky) sa vykoná snímka a zobrazí overlay.
- **Skrytie:** Overlay sa skryje druhým stlačením hotkeyu, klávesom **Esc**, alebo po uplynutí doby autoskrytia (pozri Nastavenia).
- **Zdieľanie klávesu Esc:** Esc sa v systéme registruje ako globálna skratka VÝHRADNE na dobu, kým je overlay reálne viditeľný na obrazovke. Hneď po jeho skrytí aplikácia kláves uvoľní, aby ho mohla používať hra (na zatváranie menu).
- **Zatvorenie aplikácie:** Pravý klik na tray ikonu v systémovej lište -> Ukončiť.

## Nastavenia

Konfigurácia sa ukladá do `%APPDATA%\sk.tvojtatko.tli-overlay\nastavenia.json`. Hodnoty sa menia cez okno Nastavenia, prístupné z tray ikony.

| Pole | Čo to je | Predvolené |
| :--- | :--- | :--- |
| Adresa servera | Kde beží `tli-web`. Mení sa len pri sťahovaní služby inam. | `https://tli.tvojtatko.sk` |
| Osobný kľúč | Bearer kľúč z `/kluc` alebo `/tli kluc`. V appke je uložený ako text v `nastavenia.json` — nie je to heslo k účtu, ruší sa vydaním nového. | (prázdne) |
| Klávesová skratka | Hotkey na spustenie analýzy. Zápis napr. `F8`, `CommandOrControl+Shift+T`. | `F8` |
| Okno hry | Zoznam oddelený čiarkami; porovnáva sa s názvom okna **aj** s menom procesu, bez ohľadu na veľkosť písmen, medzery a interpunkciu. Položky kratšie než 3 znaky sa ignorujú. | `Torchlight Infinite, Torchlight` |
| Automatické skrytie | Po koľkých sekundách overlay sám zhasne. `0` = nikdy. | `20` |
| Klávesová skratka — Frozen Canvas | Druhá, nezávislá skratka pre inú obrazovku hry (mriežka miešania paintov, samostatný cieľ Richardovho testovania 16.10.2026). | `F9` |
| Zber dát (Frozen Canvas) | Keď zapnuté, skratka pre Frozen Canvas nevykreslí radu — len odfotí, pošle na server (uloží sa VŽDY) a ukáže počítadlo „Uložené #N". Zapni počas zberu dát, inak nech je vypnuté. | vypnuté |

## Hlavné okno

Okrem overlayu a nastavení pribudlo od verzie 0.6.0 tretie okno — otvára sa položkou
**„Otvoriť"** v menu tray ikony (nad „Nastavenia"). Má štyri záložky: **Tracker**
(farm tracker — appka si od verzie 0.10.0 cestu k `UE_game.log` nájde sama cez Steam
register a `libraryfolders.vdf`, pole „Cesta k logu" ju len ukáže na kontrolu;
číta log naživo na pozadí aj keď je appka schovaná v tray, a počíta čistý zisk
z máp — FE, trvanie, dropy, agregácie podľa chart + kompasy + mapa, história,
export CSV, voliteľné posielanie výsledkov partii a mini-panel nad hrou),
**Solvery** (prehľad Tartarus/Frozen Canvas/Artistry — F8/F9 a overlay fungujú
ďalej presne tak, ako doteraz, táto záložka im nič nemení), **Guide** (sprievodca
Erika 1, stiahnutý zo servera a funguje aj offline) a **Nastavenia** (zatiaľ len
tlačidlo, ktoré otvorí existujúce samostatné okno nastavení).

## Bezpečnosť a pravidlá hry

Toto sú nemenné technické pravidlá projektu, zaručujúce súlad s ToS (Terms of Service) hry.

1. **Aplikácia IBA číta obrazovku a posiela snímku na náš server.** Získavanie obrazu prebieha štandardným Windows GDI API (BitBlt) nad virtuálnou plochou.
2. **Aplikácia NEROBÍ a nikdy robiť nebude:** čítanie pamäte procesu hry, injekciu kódu, hooky DirectX, zachytávanie paketov na sieti, simuláciu vstupu (klik, stlačenie klávesy) ani automatizáciu hry (makrá).
3. **Overlay je samostatné okno na úrovni OS.** Nejedná sa o prekreslenie (hook) samotného herného klienta. Je to nezávislé priehľadné okno.
4. **Hráč musí klikať sám.** Aplikácia poskytuje výhradne vizuálne odporúčanie, čo si o danom ťahu myslí server.

Dodržiavanie týchto pravidiel vynucuje krok `pravidla` v automatizácii (súbor `ci.yml`). Pri každom pushi sa kód skenuje na zakázané systémové API (`SendInput`, `mouse_event`, `keybd_event`, `ReadProcessMemory`, `WriteProcessMemory`, `VirtualAllocEx`, `CreateRemoteThread`, `SetWindowsHookEx`, `D3D11`, `pcap`). Ak sa nájde zhoda, build okamžite zlyhá.

## Ako je to postavené

Aplikácia používa framework Tauri v2 a Rust. Architektúra sa skladá z nasledujúcich častí:

- `ui/` – Frontend. Čisté HTML, CSS a Vanilla JS. Žiadny npm a žiadny build krok.
- `core/` – Knižnica `tli-overlay-core`. Obsahuje čistú logiku bez Windows API. Dôvodom oddelenia je testovateľnosť mimo Windows (funguje a testuje sa aj na Linuxe). Zabezpečuje prepočet normalizovaných súradníc (0–1) na CSS pixely overlayu podľa DPI (podpora pre 100/125/150 %), správny výber monitora podľa najväčšieho prekryvu okna hry (kvôli Richardovým trom monitorom), zmenšovanie snímky na 1920 px pred odoslaním, správu nastavení, parsovanie odpovedí zo servera a zarážku pre auto-update.
- `src-tauri/` – Samotná Windows aplikácia.
  - `okno.rs` – Detekcia a zameranie okna hry (`EnumWindows`, `DwmGetWindowAttribute`).
  - `snimka.rs` – Vytvorenie screenshotu (`BitBlt`) a kódovanie do formátu JPEG.
  - `klient.rs` – Synchrónna HTTP komunikácia (knižnica `ureq`).
  - `stav.rs` – Načítavanie a ukladanie JSON konfigurácie.
  - `main.rs` – Registrácia tray ikony, globálnych skratiek a správa troch okien aplikácie (`overlay` - bez rámu, vždy navrchu, priechodné pre myš; `nastavenia` a `hlavne` - štandardné OS okná).
- `tauri.conf.json` – Definícia identity. Obsahuje `productName` ("Tvoj Tatko TLI Overlay"), `identifier` a dôležitý **pevne definovaný WiX upgrade code `d4dac792-734c-507d-9004-d1dae51a2874`** (UUIDv5 z identifikátora).
  - *Dôvod pripnutia kódu:* Dňa 13.10.2026 vydal upstream projekt A2Tools inštalátor, ktorý vďaka zhodnému productName (a z neho odvodeného upgrade code) začal prepisovať náš fork. Fixný kód to rieši.
- **Git** – Hlavný repozitár je u nás na Forgejo (`burina/tli-overlay`). GitHub repozitár (`Burina13/tli-overlay`) je zrkadlo, slúžiace len pre beh GitHub Actions a hosťovanie release inštalátorov.

## Vývoj a testy

Aplikáciu je možné lokálne zostaviť iba na operačnom systéme Windows.
Dostupné príkazy:

- Testy jadra (dajú sa bežať aj na Linuxe): `cargo test --manifest-path core/Cargo.toml`
- Testy aplikačnej vrstvy: `cargo test --manifest-path src-tauri/Cargo.toml`
- Zostavenie projektu: `cargo tauri build`

## Release

1. Vytvorenie git tagu v tvare `tli-v<verzia>` (napr. `tli-v1.0.0`) automaticky spustí na GitHube workflow `release.yml` (na `windows-latest`).
2. Vygeneruje sa `.msi` inštalátor a súbor so súčtami `SHA256SUMS.txt`.
3. Po úspešnom releasi je **nutné manuálne spustiť** workflow `verify-install.yml`. Ten na čistom systéme otestuje inštaláciu, overí SHA256 inštalátora, vyčíta z neho priamo UpgradeCode (pre overenie s pripnutým UUID) a odinštaluje ho.
4. **Dôležité (Aktualizácia klienta):** Po vydaní novej verzie sa MUSÍ ručne upraviť súbor `latest-tli.json` v repozitári na vetve `main`. Auto-update totiž sťahuje manifest *výhradne* z adresy `https://raw.githubusercontent.com/Burina13/tli-overlay/main/latest-tli.json` (stráži to konštanta a funkcia `je_povolena_adresa()` v `core/src/aktualizacia.rs`, na čo existuje CI krok aj test v jadre). Aplikácia update sama neinštaluje, len podľa tohto súboru deteguje, že existuje novšia verzia, a otvorí webový odkaz hráčovi. Ak súbor neaktualizujete, klienti o novej verzii nezistia.

Každý push (okrem release) spúšťa `ci.yml`, ktorý robí: testy `core` na Linuxe, testy `src-tauri` na Windowse, E2E testy voči mock API, kontrolu ToS pravidiel (vyhľadávanie zakázaných systémových hovorov), verifikáciu adresy manifestu a vygenerovanie náhľadu overlayu ako artefaktu (vykresľuje sa pomocou Edge prehliadača zo súboru `ui/nahlad.html`).

## Riešenie problémov

Chyby prichádzajúce zo servera alebo z aplikácie sú vždy zobrazované ako ucelené slovenské vety.

| Príznak / Chyba | Príčina | Čo s tým |
| :--- | :--- | :--- |
| **Čierna snímka** | Hra beží v režime exkluzívneho fullscreenu, GDI BitBlt funkcia nedokáže zachytiť obraz. Appka to spozná (`je_takmer_cierny`) | Prepnite hru v menu (Nastavenia -> Zobrazenie) z režimu "Celá obrazovka" na **"Okno bez okrajov"** (Borderless windowed) alebo "Okno" |
| **"Nenašiel som okno hry"** | Herný klient nezodpovedá predvolenému názvu okna | Otvor nastavenia aplikácie a do poľa "Okno hry" dopíš názov okna alebo meno `.exe` súboru hry (položky sa oddeľujú čiarkou) |
| **Chyba 401** | Zlý alebo chýbajúci osobný kľúč | Získaj nový kľúč cez príkaz `/tli kluc` na Discorde a vlož ho do nastavení |
| **Chyba 403** | Používateľovi chýba požadovaná rola | Požiadaj administrátora na Discorde o pridelenie roly **Torchlight** |
| **Chyba 422** | Hracia doska sa na snímke nenašla | Zabezpeč, aby rozhranie Tartarus mechaniky nebolo ničím prekryté a celá doska bola viditeľná. |
| **Chyba 429** | Prekročený rate limit API | Limit je **30 dopytov za minútu**. Chvíľu počkaj a skús to znova |
| **Chyba siete** | Server je nedostupný alebo nie je prístup na internet | Skontroluj pripojenie, prípadne počkaj, kým Richard nerestartuje server |

## Rollback

Aplikácia nerobí v systéme žiadne nevratné zmeny a nemení iné procesy. Všetky vydania (releasy) aj tagy sa zrkadlia a archivujú na GitHube, nikdy sa nemažú. Ak nastane fatálny problém s novou verziou:
1. Otvor vo Windowse "Pridať alebo odstrániť programy".
2. Nájdi "Tvoj Tatko TLI Overlay" a klikni na Odinštalovať.
3. Stiahni staršiu `.msi` verziu z archívu repozitára a nainštaluj tú.
