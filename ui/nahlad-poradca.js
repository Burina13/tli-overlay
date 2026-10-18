// Ukážková odpoveď servera pre UNIVERZÁLNEHO PORADCU (TLI 36,
// `2026-10-18-1323-mcp.md`) — to, čo appka naživo pošle do `overlay.js` cez
// udalosť "stav" s `faza: "hotovo_poradca"`, teda UŽ po prepočte
// normalizovaných boxov na CSS pixely.
//
// Dáta sú z REÁLNEHO behu nad Richardovou snímkou 18.10.2026 13:16:48
// (panel „Make Your Decision" — Consume / Replace / End): presne tie boxy
// a to poradie, ktoré poradca naživo vrátil na CT 114. Snímka z CI je tak
// dôkaz nad skutočným prípadom, nie nad vymyslenými dátami.
//
// `tier` tu NIE JE tier karty — server ho odvodil z poradia (S = ber túto,
// A = druhá), aby `overlay.js` nepotreboval druhú farebnú logiku.
const UKAZKA_PORADCA = {
  faza: "hotovo_poradca",
  mierka: 1.0,
  sirka: 1600,
  vyska: 900,
  trvanie_ms: 9405,
  karticky: [
    {
      index: 0, poradie: 1, tier: "S", skore: null,
      nazov: "Consume", anglicky: "Consume",
      dovod: "Väzeň zožerie svoj cieľ rovnakej rarity a stúpne o stupeň vyššie.",
      odporucany: true, neznamy: false, malo_dat: false, sporne: false,
      x: 368, y: 621, w: 272, h: 144,
    },
    {
      index: 1, poradie: 2, tier: "A", skore: null,
      nazov: "Replace", anglicky: "Replace",
      dovod: "Vymení cieľ za iný — má zmysel len vtedy, keď je terajší bezcenný.",
      odporucany: false, neznamy: false, malo_dat: false, sporne: false,
      x: 656, y: 621, w: 272, h: 144,
    },
    {
      index: 2, poradie: 3, tier: "C", skore: null,
      nazov: "End", anglicky: "End",
      dovod: "Ukončí ťah bez efektu — deň prepadne nazmar.",
      odporucany: false, neznamy: false, malo_dat: false, sporne: false,
      x: 944, y: 621, w: 272, h: 144,
    },
  ],
  analyza: {
    ok: true,
    modul: "tartarus-poradca",
    obrazovka: "poradca",
    typ_obrazovky: "make_your_decision",
    nazov_obrazovky: "Rozhodnutie o cieli väzňa",
    istota: 0.8,
    istota_slovom: "vysoka",
    odporucanie: {
      index: 0,
      nazov: "Consume",
      dovod: "Väzeň má cieľ rovnakej rarity, takže požretie mu zdvihne raritu o stupeň — "
           + "to je priamy krok k Chromatic a do trezora. Výmena by ten istý ťah premrhala.",
    },
    skontroluj: [],
  },
};
UKAZKA_PORADCA.analyza.moznosti = UKAZKA_PORADCA.karticky;

document.body.classList.add("nahlad");
const posli = () => {
  const f = (window.__nahlad_poslucháči || {})["stav"];
  if (f) f({ payload: UKAZKA_PORADCA });
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
// Tá istá poistka ako pri ostatných náhľadoch: Edge headless vracia prvé
// poslanie občas prázdne (overené 17.10.2026 na `overlay-nahlad-nastroj.png`).
setTimeout(posli, 300);
