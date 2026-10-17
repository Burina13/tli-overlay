# Zoznam zmien

Projekt využíva sémantické verzovanie (Semantic Versioning). Verzie sú označované v tvare `<major>.<minor>.<patch>` a git tagy na releasy v tvare `tli-v<verzia>`.

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
