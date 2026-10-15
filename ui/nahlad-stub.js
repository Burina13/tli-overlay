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
  behy: [
    { id: 1, zona: OAZA, podpis_vstupov: "400021x1+500101x2", podpis_kombinacie: "", trvanie_ms: 300000, hodnota_dropov: 468.0, hodnota_vstupov: 3.1, cisty_zisk: 464.9, bez_ceny: [], najstarsia_cena: null },
    { id: 2, zona: LAVA, podpis_vstupov: "400021x1+500102x1", podpis_kombinacie: "", trvanie_ms: 240000, hodnota_dropov: 306.0, hodnota_vstupov: 3.1, cisty_zisk: 302.9, bez_ceny: [[999999, 3]], najstarsia_cena: null },
    { id: 3, zona: OAZA, podpis_vstupov: "400021x1+500101x2", podpis_kombinacie: "", trvanie_ms: 180000, hodnota_dropov: 104.0, hodnota_vstupov: 3.1, cisty_zisk: 100.9, bez_ceny: [], najstarsia_cena: null },
  ],
  vek_cien: { 400021: "pred 4 min", 500101: "pred 4 min", 5080: "pred 12 min" },
  pocet_cien: 5,
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
  verzia: "0.9.0",
  nacitaj_nastavenia: { hotkey: "F8", hotkey_canvas: "F9", tema: "svetla",
                        mini_overlay: true, tracker_sync: false,
                        cesta_logu: CESTA_LOGU_UKAZKA },
  tracker_prehlad: UKAZKA_TRACKER,
  tracker_csv: "id;mapa\n1;YunDuanLvZhou\n",
  guide_data: UKAZKA_GUIDE,
  guide_sync: UKAZKA_GUIDE,
  uloz_prepinac: true,
  tracker_sync_teraz: "Odoslané: 3 nových, 1 už server mal.",
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
