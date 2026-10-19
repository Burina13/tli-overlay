// Náhľadová náhrada za Tauri. Existuje LEN kvôli CI: UI sa na začiatku prihlási
// na udalosti a volá `invoke`, a aby sa dalo vykresliť v obyčajnom prehliadači
// (a odfotiť ako artefakt behu), treba mu tie odpovede vedieť podsunúť.
// V nainštalovanej appke sa tento súbor nikdy nenačíta — `index.html` ani
// `hlavne.html` ho nemajú.

// Ukážková session pre záložku Tracker: tri dokončené mapy s dvoma rôznymi
// kombináciami chartu a kompasov. Čísla sú tie isté, aké vracia E2E test
// `core/tests/e2e_tracker.rs` nad vzorkou `session-3-mapy.log` — screenshot
// z CI tak ukazuje presne to, čo appka naozaj spočíta, nie vymyslené pekné čísla.
const OAZA = "/Game/Art/Season/S10/Maps/YunDuanLvZhou/YunDuanLvZhou.YunDuanLvZhou";
const LAVA = "/Game/Art/Maps/KD_YuanSuKuangDong/KD_YuanSuKuangDong.KD_YuanSuKuangDong";

const UKAZKA_TRACKER = {
  zapnuty: true,
  cesta_logu: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Torchlight Infinite\\UE_Game\\Torchlight\\Saved\\Logs\\UE_game.log",
  chyba: null,
  neznamych_riadkov: 4,
  zivo: {
    zona: OAZA,
    trvanie_ms: 96000,
    cisty_zisk: 118.4,
    podpis_vstupov: "400021x1+500101x2",
    // TLI 25 — „čo padlo" v mini-paneli. Bez týchto dát by bol nový blok
    // na screenshote z CI prázdny a nikto by nevidel, či sa vôbec kreslí.
    top_dropy: [
      { polozka: 400021, pocet: 2, hodnota: 86.0 },
      { polozka: 500101, pocet: 5, hodnota: 24.5 },
      { polozka: 500102, pocet: 1, hodnota: null },
    ],
  },
  prehlad: {
    celkom: {
      pocet_map: 3,
      cas_ms: 720000,
      hodnota_dropov: 878.0,
      hodnota_vstupov: 9.3,
      cisty_zisk: 868.7,
    },
    podla_kombinacie: [
      {
        kluc: "400021x1+500102x1@" + LAVA,
        suhrn: { pocet_map: 1, cas_ms: 240000, hodnota_dropov: 306.0, hodnota_vstupov: 3.1, cisty_zisk: 302.9 },
      },
      {
        kluc: "400021x1+500101x2@" + OAZA,
        suhrn: { pocet_map: 2, cas_ms: 480000, hodnota_dropov: 572.0, hodnota_vstupov: 6.2, cisty_zisk: 565.8 },
      },
    ],
    podla_vstupov: [],
    podla_mapy: [],
    dropy_spolu: { 5080: 220, 140005: 300, 100300: 850 },
    bez_ceny_spolu: { 999999: 3 },
  },
  // `zaciatok`/`top_dropy` pribudli v TLI 45 (dashboard grafy + „Recent Runs"
  // rozbalenie „čo padlo") — bez nich by graf ukázal „zatiaľ málo dát" a
  // rozbalenie by bolo prázdne na každom riadku, hoci appka to reálne posiela.
  behy: [
    { id: 1, zona: OAZA, podpis_vstupov: "400021x1+500101x2", podpis_kombinacie: "", zaciatok: "2026-10-19 08:00:00", trvanie_ms: 300000, hodnota_dropov: 468.0, hodnota_vstupov: 3.1, cisty_zisk: 464.9, bez_ceny: [], najstarsia_cena: null,
      top_dropy: [{ polozka: 400021, pocet: 1, hodnota: 420.0 }, { polozka: 500101, pocet: 2, hodnota: 48.0 }] },
    { id: 2, zona: LAVA, podpis_vstupov: "400021x1+500102x1", podpis_kombinacie: "", zaciatok: "2026-10-19 08:05:30", trvanie_ms: 240000, hodnota_dropov: 306.0, hodnota_vstupov: 3.1, cisty_zisk: 302.9, bez_ceny: [[999999, 3]], najstarsia_cena: null, zastarane: [[5080, 12]],
      top_dropy: [{ polozka: 400021, pocet: 1, hodnota: 280.0, zdroj: "titrack.ninja", prispievatelia: 12, vek: "pred 4 min" }] },
    { id: 3, zona: OAZA, podpis_vstupov: "400021x1+500101x2", podpis_kombinacie: "", zaciatok: "2026-10-19 08:10:00", trvanie_ms: 180000, hodnota_dropov: 104.0, hodnota_vstupov: 3.1, cisty_zisk: 100.9, bez_ceny: [], najstarsia_cena: null,
      top_dropy: [] },
  ],
  vek_cien: { 400021: "pred 4 min", 500101: "pred 4 min", 5080: "pred 12 min" },
  pocet_cien: 5,
  // TLI 22 — koľko cien appka sama prečítala z obrazovky trhu za 24 h.
  pocet_cien_z_obrazovky_24h: 2,
  // TLI 20 — výzva „vyhľadaj na Exchange". V náhľade je zámerne vyplnená,
  // nech je na screenshote z CI vidieť, ako vyzerá, keď sa zobrazí.
  vyzva_ceny: { pocet: 4, polozky: [999999, 140005, 5080] },
  // TLI 21 bod 4 — appka naposledy skúsila kolo uploadu pred chvíľou a prešlo
  // bez chyby; náhľad tak ukazuje, ako appka vyzerá v BEŽNOM (fungujúcom)
  // stave, nielen po chybe.
  posledna_kontrola_uploadu_pred: "pred 2 min",
  posledny_upload_pred_s: 120,
  posledna_chyba_uploadu: null,
  // CURRENT INVENTORY (TLI 47) — niekoľko vzorových položiek po Sorte.
  batoh: [
    { config_base_id: 400021, mnozstvo: 3, cena_za_kus_fe: 140.0, hodnota_fe: 420.0, cena_zastarana: false, skryte: false, cena_vek: "pred 4 min" },
    { config_base_id: 500101, mnozstvo: 24, cena_za_kus_fe: 2.0, hodnota_fe: 48.0, cena_zastarana: false, skryte: false, cena_vek: "pred 4 min" },
    { config_base_id: 999999, mnozstvo: 3, cena_za_kus_fe: null, hodnota_fe: null, cena_zastarana: false, skryte: false, cena_vek: null },
  ],
  net_worth_fe: 971.8,
  // TLI 49 bod 2 — footer „ceny aktualizované HH:MM".
  ceny_synchronizovane_o: 1760866800,
};

// Mená položiek pre náhľad (TLI 20 bod 7). Náhľad beží z `file://`, kde
// `fetch("polozky-seed-en.json")` neprejde — bez tohto by bolo na screenshote
// `#400021` namiesto skutočného mena. Stačí hrsť ID, ktoré sa v náhľade
// naozaj vyskytujú.
// Hodnoty sú DOSLOVA z `polozky-seed-en.json` — náhľad nesmie ukazovať meno,
// ktoré appka v skutočnosti nepozná. `500101`/`500102`/`999999` v seede nie sú,
// preto sa tu neuvádzajú a v náhľade ostanú ako `#500101` (presne tak, ako to
// hráč uvidí).
window.__MENA_POLOZIEK__ = {
  400021: "Steel Forge Beacon (Timemark 7)",
  140005: "Shallow Dream Talking - Armor",
  5080: "Energy Core",
  100300: "Flame Elementium",
};

// Kategórie pre náhľad Economy (TLI 49 bod 1) — rovnaký dôvod ako mená
// vyššie: `fetch("polozky-kategorie-en.json")` z `file://` neprejde.
window.__KATEGORIE_POLOZIEK__ = {
  400021: "beacons",
  140005: "legendary_gear",
  5080: "general",
  100300: "general",
  999999: "general",
};

// Ukážkový sprievodca pre záložku Guide (TLI 11, krok 4). Tvar je presne ten,
// čo vracia príkaz `guide_data` — vrátane ikony ako `data:` URI, lebo tak ju
// appka naozaj dostane (`img-src 'self' data:` v CSP nič iné nepustí).
const IKONA_UKAZKA =
  "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9" +
  "IjAgMCAzMiAzMiI+PHJlY3Qgd2lkdGg9IjMyIiBoZWlnaHQ9IjMyIiByeD0iNiIgZmlsbD0iI2ZmOTYyYyIvPjx0ZXh0" +
  "IHg9IjE2IiB5PSIyMiIgZm9udC1zaXplPSIxNiIgdGV4dC1hbmNob3I9Im1pZGRsZSIgZmlsbD0iIzFjMWYyNCI+VDwv" +
  "dGV4dD48L3N2Zz4=";

const UKAZKA_GUIDE = {
  verzia: "a1b2c3d4e5f6",
  zdroj: "https://tlicompendium.com/en/build-guides/erika1-lvl1-to-ss20-ultimate-guide",
  stiahnuty: 1760000000,
  vek: "pred 2 h",
  je_prazdny: false,
  tahak: {
    slug: "tahak-na-start",
    nazov: "Erika 1 — ťahák na štart",
    html:
      "<h2>1. Prvé hodiny a leveling v kampani</h2>" +
      "<ul><li><strong>Mule skilly:</strong> na lvl 3 odomkni " +
      '<span class="tli-ikona" title="Haunt"><img src="' + IKONA_UKAZKA + '" alt="Haunt">Haunt</span>' +
      " a presuň ho cez truhlu na Erika1.</li>" +
      "<li><strong>Koniec kampane:</strong> vyzdvihni si <em>Activation Medium: Motionless</em>.</li></ul>" +
      "<h2>2. Skilly a progres</h2>" +
      "<p>Na bossoch ručne obnovuj <em>Timid</em>. Zdroj: " +
      '<a href="https://tlicompendium.com/en/build-guides/erika1-lvl1-to-ss20-ultimate-guide"' +
      ' data-externy="1">tlicompendium.com</a>.</p>',
  },
  kapitoly: [
    { slug: "uvod", nazov: "Úvod", html: "<h2>Úvod</h2><p>Hero Traits a Memories pre Eriku.</p>" },
    { slug: "lvl1-45", nazov: "Build Version: lvl 1-45", html: "<h2>lvl 1-45</h2><p>Kampaň.</p>" },
    { slug: "t1-t7-maps", nazov: "Build Version: T1-T7 Maps", html: "<h2>T1-T7</h2><p>Mapy.</p>" },
    { slug: "t8", nazov: "Build Version: T8", html: "<h2>T8</h2><p>Min. 1B DPS.</p>" },
  ],
};

const CESTA_LOGU_UKAZKA =
  "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Torchlight Infinite\\UE_Game\\Torchlight\\Saved\\Logs\\UE_game.log";

const ODPOVEDE = {
  verzia: "0.15.0",
  // Footer „posledná kontrola HH:MM" (TLI 52 bod 2) — náhľad ukazuje stav
  // „beží najnovšia verzia", nech je na screenshote z CI vidieť bežný
  // (neporuchový) stav footera.
  posledna_kontrola_aktualizacie: { cas: 1760867400, najdena_verzia: null, msi_url: null, chyba: null },
  nacitaj_nastavenia: { hotkey: "F8", hotkey_canvas: "F9", hotkey_trh: "F10", tema: "svetla",
                        mini_overlay: true, tracker_sync: false,
                        ceny_sync: true, upload_logov: true, ceny_obrazovka_zapnute: true,
                        cesta_logu: CESTA_LOGU_UKAZKA, url_servera: "https://tli.tvojtatko.sk" },
  tracker_prehlad: UKAZKA_TRACKER,
  // Economy (TLI 48 bod 3) — plný prehliadač cien. Zámerne rôzne zdroje aj
  // znamienka „zmena 24h" (kladné/záporné/chýbajúce), nech je na screenshote
  // z CI vidieť všetky tri stavy, nie len ten pekný.
  tracker_economy: [
    { polozka: 400021, cena_fe: 140.0, zdroj: "vlastny_trh", vek_s: 240, zmena_24h_percent: 12.5 },
    { polozka: 140005, cena_fe: 2.4, zdroj: "titrack", vek_s: 1800, zmena_24h_percent: -8.1 },
    { polozka: 5080, cena_fe: 0.73, zdroj: "partia", vek_s: 9000, zmena_24h_percent: 0 },
    { polozka: 100300, cena_fe: 1.0, zdroj: "obrazovka", vek_s: 300, zmena_24h_percent: null },
    { polozka: 999999, cena_fe: 0.05, zdroj: "rucne", vek_s: 172800, zmena_24h_percent: null },
  ],
  tracker_csv: "id;mapa\n1;YunDuanLvZhou\n",
  guide_data: UKAZKA_GUIDE,
  guide_sync: UKAZKA_GUIDE,
  uloz_prepinac: true,
  tracker_sync_teraz: "Odoslané: 3 nových, 1 už server mal.",
  ceny_sync_teraz: "Ceny: odoslaných 2, od partie prišlo 37 (z toho 5 nových).",
  upload_teraz: "Upload je dohnaný, nič nové na poslanie.",
  uloz_temu: "svetla",
  // TLI 11c — automatická detekcia cesty k logu (náhľad ukazuje stav „nájdené a v poriadku").
  tracker_navrh_cestu: CESTA_LOGU_UKAZKA,
  tracker_stav_cesty: {
    cesta: CESTA_LOGU_UKAZKA,
    existuje: true,
    velkost_b: 19_300_000,
    upravene_pred_s: 42,
    takmer_prazdny: false,
    chyba_vypnuty_log: false,
  },
  tracker_vyber_subor: CESTA_LOGU_UKAZKA,
  uloz_cestu_logu: CESTA_LOGU_UKAZKA,
  // Záložka Analýza (TLI 13) — rovnaké dve mapy ako vyššie, len z pohľadu DB.
  analyza_vyhladaj: [
    { id: 1, zona: OAZA, podpis_vstupov: "400021x1+500101x2", zaciatok_cas: "2026-10-16 10:00:00",
      trvanie_ms: 300000, cisty_zisk_fe: 464.9, fe_za_hodinu: 5578.8, pocet_monster: 42, pocet_rares_a_bossov: 5 },
    { id: 2, zona: LAVA, podpis_vstupov: "400021x1+500102x1", zaciatok_cas: "2026-10-16 09:55:00",
      trvanie_ms: 240000, cisty_zisk_fe: 302.9, fe_za_hodinu: 4543.5, pocet_monster: 31, pocet_rares_a_bossov: 3 },
  ],
  analyza_monstra_podla_rarity: [[1, 25], [2, 12], [3, 4], [4, 1]],
  analyza_export_csv: "id;zona\n1;YunDuanLvZhou\n",
  analyza_export_json: "[]",
  // Taktiky (TLI 13b) — rovnaké čísla, aké dá appka nad vzorkou `session-3-mapy.log`
  // (`core/tests/e2e_taktiky.rs`): LAVA má vyšší priemerný FE/h, hoci OAZA má
  // viac behov aj vyšší súhrnný zisk — presne tento rozdiel má byť na snímke vidieť.
  analyza_taktiky: {
    taktiky: [
      {
        kluc: "400021x1+500102x1@" + LAVA + "@mod-@map[]@build[||]",
        nazov: "400021x1+500102x1@" + LAVA + "@mod-@map[]@build[||]",
        podpisy: ["400021x1+500102x1@" + LAVA + "@mod-@map[]@build[||]"],
        ukazka_podpis_vstupov: "400021x1+500102x1",
        ukazka_zona: LAVA,
        statistika: {
          pocet_behov: 1, pocet_ocenenych: 1,
          priemerny_zisk: 302.9, median_zisk: 302.9, min_zisk: 302.9, max_zisk: 302.9,
          p10_zisk: 302.9, p90_zisk: 302.9, smerodajna_odchylka: null,
          priemerne_fe_za_hodinu: 4543.5, priemerny_naklad: 3.1, roi: 97.7,
          pomer_zisk_riziko: null, priemerne_trvanie_ms: 240000, malo_dat: true,
        },
      },
      {
        kluc: "400021x1+500101x2@" + OAZA + "@mod-@map[]@build[||]",
        nazov: "400021x1+500101x2@" + OAZA + "@mod-@map[]@build[||]",
        podpisy: ["400021x1+500101x2@" + OAZA + "@mod-@map[]@build[||]"],
        ukazka_podpis_vstupov: "400021x1+500101x2",
        ukazka_zona: OAZA,
        statistika: {
          pocet_behov: 2, pocet_ocenenych: 2,
          priemerny_zisk: 282.9, median_zisk: 282.9, min_zisk: 100.9, max_zisk: 464.9,
          p10_zisk: 100.9, p90_zisk: 464.9, smerodajna_odchylka: 257.2,
          priemerne_fe_za_hodinu: 3798.4, priemerny_naklad: 3.1, roi: 91.3,
          pomer_zisk_riziko: 1.1, priemerne_trvanie_ms: 240000, malo_dat: true,
        },
      },
    ],
    odporucanie: {
      text: "Zatiaľ málo dát na odporúčanie — žiadna taktika nemá aspoň 5 ocenených behov.",
      taktika_kluc: null,
    },
    navrhy_na_otestovanie: [
      "400021x1+500102x1@.../LAVA — zatiaľ len 1 ocenený beh (z 5 potrebných), oplatí sa otestovať viac",
    ],
  },
  analyza_taktiky_export_csv: "nazov;pocet_behov\nLAVA;1\n",
  analyza_taktiky_export_json: "[]",
  taktika_pomenuj_alebo_zluc: 1,
  tracker_nacitaj_stare_logy: { pocet_riadkov: 163339, pocet_novych_behov: 9, pocet_preskocenych_behov: 0,
                                pocet_monster: 4168, pocet_cien: 1, pocet_zostatkov: 374, neznamych_riadkov: 148724 },
};

window.__TAURI__ = {
  event: {
    listen: (meno, funkcia) => {
      window.__nahlad_poslucháči = window.__nahlad_poslucháči || {};
      window.__nahlad_poslucháči[meno] = funkcia;
      return Promise.resolve(() => {});
    },
  },
  core: {
    invoke: (meno) => Promise.resolve(meno in ODPOVEDE ? ODPOVEDE[meno] : null),
  },
};
