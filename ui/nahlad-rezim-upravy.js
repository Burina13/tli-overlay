// Náhľad režimu „Upraviť rozloženie" (TLI 21, `2026-10-16-1528-mcp.md` bod 1)
// — rovnaký vzor ako `nahlad-canvas.js`: reálne `overlay.js` sa zaregistruje
// na udalosť `rezim-upravy` cez `nahlad-stub.js`, tento súbor ju po načítaní
// zavolá s ukážkovým payloadom presne v tvare, aký appka naživo posiela pri
// vstupe do režimu (Ctrl+F8). Screenshot z tohto súboru je dôkaz, že panel aj
// úchyty na presun/zmenu veľkosti sa naozaj vykreslia — nie len že kód prešiel
// testami.
const UKAZKA_REZIM_UPRAVY = {
  zapnute: true,
  panel: { x: 60, y: 48, w: 420, h: 250 },
  priehladnost: 0.85,
  okno: { sirka: 1600, vyska: 900 },
  // TLI 25 — prepínač, ktorému modulu sa poloha práve nastavuje. Bez týchto
  // dvoch polí by bol na snímke z CI panel presne ako pred TLI 25 a nikto by
  // nevidel, či sa prepínač vôbec kreslí. Zoznam musí sedieť s
  // `rozlozenie::MODULOVE_PANELY` v jadre.
  moduly: [
    { kluc: "panel:tartarus", popis: "F8 — doska" },
    { kluc: "panel:nastroj", popis: "F8 — výber nástroja" },
    { kluc: "panel:canvas", popis: "F9 — Frozen Canvas" },
    { kluc: "panel:artistry", popis: "Artistry" },
  ],
  aktivny_modul: "panel:tartarus",
};

document.body.classList.add("nahlad");
const posli = () => {
  const f = (window.__nahlad_poslucháči || {})["rezim-upravy"];
  if (f) f({ payload: UKAZKA_REZIM_UPRAVY });
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
