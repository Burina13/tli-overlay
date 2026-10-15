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

const ODPOVEDE = {
  verzia: "0.7.0",
  nacitaj_nastavenia: { hotkey: "F8", hotkey_canvas: "F9", tema: "svetla" },
  tracker_prehlad: UKAZKA_TRACKER,
  tracker_csv: "id;mapa\n1;YunDuanLvZhou\n",
  uloz_temu: "svetla",
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
