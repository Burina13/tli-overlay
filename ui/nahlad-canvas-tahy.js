// TLI 71 — náhľad podobrazovky „Select the Brushstroke direction". Tvar je
// presne ten, ktorý appka naživo pošle do `overlay.js` udalosťou "stav"
// (`faza: "hotovo_canvas"` + `karty`/`tlacidlo`/`analyza.tah`), len po
// prepočte na CSS pixely okna 1600×900.
//
// Dáta sú z REÁLNEJ Richardovej snímky `2026-10-20-163311` (ponuka
// [Reimagine] + [Spread], v dosahu niet voľnej bunky), nie vymyslené —
// na tej sa 20.10.2026 ukázalo, že appka túto fázu vôbec nepozná.
const KARTY = [
  { index: 0, odporucana: false, oznacenie: "[Reimagine]", skore: 70,
    dovod: "Úzke hrdlo sú BUNKY: voľných cieľových buniek máš 0 z potrebných 3. " +
           "Priamo v dosahu sú 2 zablokované bunky. [Reimagine] to vie obísť — prehodí " +
           "smer efektu inam alebo zníži požadovaný počet.",
    x: 866, y: 373, w: 467, h: 83 },
  { index: 1, odporucana: true, oznacenie: "[Spread]", skore: 95,
    dovod: "Úzke hrdlo sú BUNKY: cieľ chce ešte 3 umiestnenia, ale voľných cieľových " +
           "buniek máš 0. Priamo v dosahu sú 2 zablokované bunky. Presne na to je " +
           "[Spread] — maľuje na Blocked/Void/Discolored bunku alebo ju premení na prázdnu.",
    x: 866, y: 467, w: 467, h: 83 },
];

const UKAZKA_TAHY = {
  faza: "hotovo_canvas",
  mierka: 1.0,
  sirka: 1600,
  vyska: 900,
  trvanie_ms: 2600,
  kroky: [],
  karty: KARTY,
  tlacidlo: { x: 1161, y: 677, w: 195, h: 48 },
  analyza: {
    ok: true,
    modul: "canvas",
    istota: 0.75,
    istota_rozpoznania: 0.85,
    pravidlo_overene: null,
    zdroj_dosahu: "smer",
    skontroluj: [],
    kroky: [],
    faza: "vyber_tahu",
    platno: { ciel: { smer: "adjacent" }, ruka: [] },
    tah: {
      faza: "vyber_tahu",
      odporucany_index: 1,
      druh: "spread",
      dovod: "Vyber kartu 2 — [Spread]. Úzke hrdlo sú BUNKY: cieľ chce ešte 3 umiestnenia, " +
             "ale voľných cieľových buniek máš 0. Priamo v dosahu sú 2 zablokované bunky. " +
             "Presne na to je [Spread] — maľuje na Blocked/Void/Discolored bunku alebo ju " +
             "premení na prázdnu. Potom klikni na tlačidlo Select. Poradie kariet podľa " +
             "vhodnosti: 2. [Spread] (95/100), 1. [Reimagine] (70/100).",
      istota: 0.75,
      skontroluj: [],
      hodnotenia: KARTY.map((k) => ({
        index: k.index, druh: k.oznacenie.replace(/[[\]]/g, "").toLowerCase(), ucel: "",
        text: "", skore: k.skore, dovod: k.dovod, oznacenie: k.oznacenie,
      })),
    },
  },
};

document.body.classList.add("nahlad");
const posli = () => {
  const f = (window.__nahlad_poslucháči || {})["stav"];
  if (f) f({ payload: UKAZKA_TAHY });
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
// Tá istá poistka ako v `nahlad-canvas.js`: náhľady kreslené cez udalosť
// "stav" vyšli z Edge headless prázdne, druhé poslanie po 300 ms to rieši.
setTimeout(posli, 300);
