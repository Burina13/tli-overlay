// Ukážková odpoveď servera pre Frozen Canvas (TLI 4b), tvar presne podľa
// `docs/fakty/hry/torchlight-infinite/canvas-api-kontrakt.md` PO prepočte na
// CSS pixely (presne to, čo appka naživo pošle do `overlay.js` cez udalosť
// "stav" s `faza: "hotovo_canvas"`). Súradnice zodpovedajú syntetickému boardu
// z `stacks/games/tli/engine/testy-canvas-solver.py` (Dwarf Lv3 na r4c3, 3×
// voľná susedná bunka, 3× Dwarf Lv1 v ruke).
const UKAZKA_CANVAS = {
  faza: "hotovo_canvas",
  mierka: 1.0,
  sirka: 1600,
  vyska: 900,
  trvanie_ms: 4300,
  kroky: [
    {
      poradie: 1,
      dovod: "Vezmi dwarf Lv1 zo slotu 0 v ruke a polož ho na bunku (riadok 3, stĺpec 3) " +
             "vedľa dwarf Lv3 (r4c3) — splní krok 1/3 cieľa 'Fill a Adjacent Cell with a " +
             "Monster Paint' a umiestnený paint vyskočí na Lv2 (Level up the Adjacent " +
             "Monster Paint by 1 level(s)).",
      zdroj: { x: 760, y: 740, w: 90, h: 90 },
      ciel: { x: 870, y: 360, w: 90, h: 90 },
    },
    {
      poradie: 2,
      dovod: "Vezmi dwarf Lv1 zo slotu 1 v ruke a polož ho na bunku (riadok 4, stĺpec 2).",
      zdroj: { x: 860, y: 740, w: 90, h: 90 },
      ciel: { x: 780, y: 450, w: 90, h: 90 },
    },
    {
      poradie: 3,
      dovod: "Vezmi dwarf Lv1 zo slotu 2 v ruke a polož ho na bunku (riadok 4, stĺpec 4).",
      zdroj: { x: 960, y: 740, w: 90, h: 90 },
      ciel: { x: 960, y: 450, w: 90, h: 90 },
    },
  ],
  analyza: {
    ok: true,
    modul: "canvas",
    istota: 0.9,
    istota_rozpoznania: 0.95,
    skontroluj: [],
    // TLI 69: cieľové bunky orámovala sama hra (appka ich rozpozná lokálne),
    // takže panel napíše „isté" a nie „odhadnuté z názvu smeru".
    zdroj_dosahu: "ram",
    // TLI 8: panel z `platno.ruka` berie paint zdrojového slotu a ukáže jeho
    // ikonu; index slotu je v `kroky[].zdroj` (pôvodné, neprepočítané).
    kroky: [
      { poradie: 1, zdroj: { typ_slotu: "ruka", index: 0 } },
      { poradie: 2, zdroj: { typ_slotu: "ruka", index: 1 } },
      { poradie: 3, zdroj: { typ_slotu: "ruka", index: 2 } },
    ],
    platno: {
      ciel: { smer: "adjacent" },
      ruka: [
        { index: 0, paint: { kategoria: "monster", typ: "dwarf", level: 1 } },
        { index: 1, paint: { kategoria: "monster", typ: "dwarf", level: 1 } },
        { index: 2, paint: { kategoria: "monster", typ: "dwarf", level: 1 } },
      ],
    },
  },
};

document.body.classList.add("nahlad");
const posli = () => {
  const f = (window.__nahlad_poslucháči || {})["stav"];
  if (f) f({ payload: UKAZKA_CANVAS });
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
// Poistka pre snímku z CI: náhľady, ktoré kreslia cez udalosť "stav", vyšli
// z Edge headless prázdne (overené 17.10.2026 na `overlay-nahlad-canvas.png`
// aj na novom `overlay-nahlad-nastroj.png`), hoci tie isté dáta v jsdom
// testoch panel vykreslia. Druhé poslanie po 300 ms nič nepokazí —
// `skryVsetko()` na začiatku poslucháča zmaže predošlé vykreslenie.
setTimeout(posli, 300);
