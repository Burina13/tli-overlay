// TLI 72 — náhľad obrazovky KONCA BEHU Frozen Canvasu („All Paint has been
// added. You can start painting now.", počítadlo 20/20, tlačidlá Start Over
// za 5 Colored Snow a Complete). Tvar je presne ten, ktorý appka naživo
// pošle do `overlay.js` udalosťou "stav" (`faza: "hotovo_canvas"` +
// `dokoncenie` + `analyza.dokoncenie`), len po prepočte na CSS pixely
// okna 1600×900.
//
// Dáta sú z REÁLNEJ Richardovej snímky `2026-10-20-141014` — toho behu,
// ktorý ~14:1x poslal do Coworku: 25 obsadených buniek, súčet levelov 59,
// najvyšší paint Lv6, teda ani jeden dúhový. Priemer z 11 jeho behov je
// 0,09 dúhového a súčet 45,3, takže rozhodnutie vychádza tesne na Complete.
const UKAZKA_KONIEC = {
  faza: "hotovo_canvas",
  mierka: 1.0,
  sirka: 1600,
  vyska: 900,
  trvanie_ms: 2400,
  kroky: [],
  karty: [],
  dokoncenie: {
    akcia: "complete",
    dovod: "",
    cena_start_over: 5,
    odporucane: { x: 1161, y: 677, w: 195, h: 48 },
    complete: { x: 1161, y: 677, w: 195, h: 48 },
    start_over: { x: 989, y: 675, w: 168, h: 68 },
  },
  analyza: {
    ok: true,
    modul: "canvas",
    istota: 0.4,
    istota_rozpoznania: 0.85,
    pravidlo_overene: null,
    zdroj_dosahu: "",
    skontroluj: [
      "priemer je spočítaný z 11 behov, ale ani jeden z nich nemá potvrdený koniec " +
      "z počítadla — poskladal sa z „plné plátno na konci série snímok“, takže je to " +
      "odhad, nie meranie",
      "appka nevidí tvoj zostatok Colored Snow — Start Over stojí 5 a keď na to nemáš, " +
      "voľba je aj tak len Complete",
    ],
    kroky: [],
    faza: "vsetko_polozene",
    polozene: 20,
    celkom: 20,
    platno: { ciel: null, ruka: [] },
    dokoncenie: {
      akcia: "complete",
      dovod: "Na plátne máš 0 paintov na Max Paint Level 7 a súčet levelov 59. " +
             "Tvoj priemer z 11 behov je 0.09 paintu na max a súčet 45; najlepší " +
             "doterajší beh mal 1 na max. Paintov na max máš pod priemerom, ale súčet " +
             "levelov je nad ním — to sú protichodné signály a istý výsledok je viac " +
             "než 5 Colored Snow za nový pokus. Dávam Complete, rozhodnutie je tesné.",
      cena_start_over: 5,
      skore: { na_max: 0, sucet: 59, obsadenych: 25, dostupnych: 25, max_level: 7 },
      istota: 0.4,
      skontroluj: [],
    },
  },
};

document.body.classList.add("nahlad");
const posli = () => {
  const f = (window.__nahlad_poslucháči || {})["stav"];
  if (f) f({ payload: UKAZKA_KONIEC });
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
// Tá istá poistka ako v `nahlad-canvas.js`: náhľady kreslené cez udalosť
// "stav" vyšli z Edge headless prázdne, druhé poslanie po 300 ms to rieši.
setTimeout(posli, 300);
