# NOTICE — prevzaté cudzie dielo

Tento projekt používa časti cudzích diel. Tu je presne čo, odkiaľ a pod akou
licenciou. Súbor sa dopĺňa pri KAŽDOM ďalšom prevzatí — nie spätne.

## TITrack (astockman99/TITrack)

- **Zdroj:** https://github.com/astockman99/TITrack
- **Snímka:** commit `93b3fc3548995ab2c54f377f9619b984399dcae8` (2026-07-17),
  vetva `master`, stiahnuté 16. 10. 2026.
- **Licencia:** **MIT.** Repozitár ju deklaruje v `README.md`
  (odznak „license-MIT") a v `pyproject.toml`
  (`license = {text = "MIT"}`, klasifikátor `License :: OSI Approved :: MIT
  License`). ⚠️ **Samotný súbor `LICENSE` v repozitári k tomuto dátumu NIE JE**
  (`GET /repos/astockman99/TITrack/license` vracia 404 a v archíve vetvy chýba),
  hoci README naň odkazuje. Preto sa tu neuvádza doslovný text licencie ani meno
  držiteľa autorských práv — nemáme ich z autoritatívneho zdroja. Keby autor
  `LICENSE` doplnil, prepíš tento odsek podľa neho.

### Čo je prevzaté

1. **`ui/polozky-seed-en.json`** — dvojice `ConfigBaseId → anglické meno
   položky` (1 933 položiek), odvodené z `tlidb_items_seed_en.json`.
   Prevzali sa IBA tieto dvojice; čínske názvy, typy, odkazy a odmeny z
   pôvodného súboru nie.
2. **`data/tlidb-ikony-url.json`** — dvojice `ConfigBaseId → URL ikony na
   `cdn.tlidb.com`` z toho istého súboru. **Zatiaľ sa nepoužíva a nič sa
   nesťahuje** — je to podklad pre TLI 19 bod 3c (ikony položiek).
3. **Vzorky formátu tela správy `XchgSearchPrice`** použité v testoch
   `core/src/tracker/stav.rs` (`test_xchg_realny_tvar_*`) — riadky pochádzajú
   z `tests/unit/test_exchange_parser.py` v TITracku a sú to výpisy z reálnych
   herných logov z 26./28. 1. 2026.
4. **Rozloženie a pomenovanie hlavného dashboardu** (TLI 45, 19. 10. 2026) —
   Richard chcel záložku Tracker vzhľadovo aj funkčne bližšie k desktopovej
   appke TITracku. Prevzaté sú anglické NÁZVY siedmich štatistických kariet
   (NET WORTH, CUMULATIVE VALUE, FE/HOUR, FE/MAP, RUNS, AVG RUN TIME, TOTAL
   TIME) a koncept rozloženia (karty hore, grafy pod nimi, história máp s
   rozbaliteľným „čo padlo", tlačidlá Overlay/Economy/Cloud Sync/Instructions/
   Reset Stats, pätička „Collector"). **Žiadny kód ani obrázok prevzatý nie
   je** — grafy sú vlastné inline SVG, výpočty idú z vlastného `core`
   (`agregacie.rs`), CSS je vlastné (`ui/hlavne.css`, existujúce premenné
   témy). TITrack layout/pomenovania samy osebe nie sú chránené dielo v
   zmysle kódu, ale zápis je tu pre úplnosť a transparentnosť rovnako ako pri
   bodoch 1–3.

### Čo prevzaté NIE JE

Žiadny kód. Parser `XchgSearchPrice` v `core/src/tracker/stav.rs` je vlastný
(Rust, iný prístup: stavový prechod riadkov namiesto regulárnych výrazov nad
celým telom správy). Referenčná cena ako 10. percentil ponúk je **zhodný
postup**, nie prevzatý kód — a bol v tomto projekte už od TLI 13.

Na **komunitné ceny TITracku (Supabase) sa tento projekt NENAPÁJA.** Dôvod je
v `docs/fakty/hry/torchlight-infinite/README.md` (zápis zo 16. 10. 2026).
