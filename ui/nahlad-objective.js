// Ukážková odpoveď servera pre obrazovku OBJECTIVE (TLI 37,
// `2026-10-18-1324-mcp.md`) — to, čo appka naživo pošle do `overlay.js` cez
// udalosť "stav" s `faza: "hotovo_objective"`, teda UŽ po prepočte
// normalizovaných boxov na CSS pixely.
//
// Dáta sú z REÁLNEJ snímky Richarda 18.10.2026 13:23:25 (Stage Level 77,
// „Objective: Survival of the Fittest" — Consume 4 times): ten istý text,
// tá istá odmena a box tlačidla Confirm prepočítaný na 1600×900.
const UKAZKA_OBJECTIVE = {
  faza: "hotovo_objective",
  mierka: 1.0,
  sirka: 1600,
  vyska: 900,
  trvanie_ms: 5210,
  karticky: [
    {
      index: 0, poradie: 1, tier: "S", skore: null,
      nazov: "Confirm", anglicky: "Confirm",
      dovod: "Potvrdíš cieľ a pustíš sa do drilu.",
      odporucany: true, neznamy: false, malo_dat: false, sporne: false,
      x: 928, y: 612, w: 256, h: 54,
    },
  ],
  analyza: {
    ok: true,
    modul: "tartarus-objective",
    obrazovka: "objective",
    typ_obrazovky: "objective",
    nazov_obrazovky: "Objective: Survival of the Fittest",
    istota: 0.75,
    istota_slovom: "vysoka",
    potvrdenie: "Objective: Požretie (Consume) 4× — stratégia upravená",
    plan: [
      "Požretie je aj tak hlavný spôsob, ako dvíhať raritu — tento cieľ ťa nenúti hrať inak, len nenechaj deň prejsť naprázdno.",
      "Keď sa dá vybrať medzi požretím a výmenou/koncom dňa, ber požretie, kým nemáš 4 z 4.",
      "Nástroj alebo dar, ktorý pridáva požretia alebo reťaz, má teraz hodnotu navyše.",
    ],
    objective: {
      kod: "survival_of_the_fittest",
      anglicky: "Survival of the Fittest",
      nazov: "Prežije najsilnejší",
      podmienka: "Consume 4 times",
      akcia_sk: "Požretie (Consume)",
      ciel_pocet: 4,
      aktualny: 0,
      zostava: 4,
      splneny: false,
      progres_text: "0/4",
      progres_presnost: "odhad",
      v_katalogu: true,
      odmena_sk: [
        "Vyberieš si o 1 nástroj (Torture Instrument) navyše",
        "Pri vstupe do Tartaru v Riote hneď dostaneš Ultimate alebo Chromatic Warden's Cache",
      ],
    },
    odporucanie: { index: 0, nazov: "Confirm", tlacidlo: 0, dovod: "Potvrď a hraj." },
    skontroluj: [],
  },
};
UKAZKA_OBJECTIVE.analyza.moznosti = UKAZKA_OBJECTIVE.karticky;

document.body.classList.add("nahlad");
const posli = () => {
  const f = (window.__nahlad_poslucháči || {})["stav"];
  if (f) f({ payload: UKAZKA_OBJECTIVE });
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
// Tá istá poistka ako pri ostatných náhľadoch: Edge headless vracia prvé
// poslanie občas prázdne (overené 17.10.2026 na `overlay-nahlad-nastroj.png`).
setTimeout(posli, 300);
