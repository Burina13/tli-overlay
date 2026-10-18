// Ukážková odpoveď servera podľa kontraktu `POST /api/analyza`. Čísla sú vymyslené,
// ale tvar je presne ten, ktorý vracia `tli-web` — keď sa kontrakt zmení, náhľad
// v CI to ukáže ako prvý.
const UKAZKA = {
  faza: "hotovo",
  mierka: 1.0,
  sirka: 1600,
  vyska: 900,
  trvanie_ms: 4120,
  obdlzniky: [
    { index: 6, x: 752, y: 120, w: 150, h: 150, stredX: 827, stredY: 195, neiste: false },
    { index: 7, x: 912, y: 120, w: 150, h: 150, stredX: 987, stredY: 195, neiste: true },
    { index: 5, x: 592, y: 120, w: 150, h: 150, stredX: 667, stredY: 195, neiste: false },
  ],
  // TLI 32: tlačidlo spodnej lišty ("Make Your Decision" → "Consume", ktoré
  // zodpovedá odporúčanej akcii "devour" nižšie) namiesto zvýraznenia cely —
  // súradnice prepočítané z reálnej analýzy (`2026-10-18-105532-76921df2.json`,
  // panel "Replace"/"End" má rovnaké rozloženie, len iný text na tlačidlách).
  // `tlacidlo_alt` je `null` — alternatíva nižšie ("swap") na TOMTO paneli
  // (Consume/End) nemá zodpovedajúce tlačidlo, appka preto druhé tlačidlo
  // nezvýrazňuje (presne správanie `_najdi_tlacidlo` v `app.py`).
  tlacidlo: { x: 517, y: 624, w: 275, h: 143, text: "Consume" },
  tlacidlo_alt: null,
  analyza: {
    ok: true,
    odporucanie: {
      akcia: "devour",
      nazov: "Požretie",
      ciel: 6,
      cez_scratch: false,
      nastroj: null,
      dar: null,
      dovod: "Mamut na políčku 6 má najvyššiu raritu v dosahu a požretie nestojí nič. " +
             "Získaš tým jeden stupeň rarity navyše ešte pred príchodom dozorcu.",
      hodnota: 15.25,
    },
    alternativy: [
      { akcia: "swap", nazov: "Výmena", ciel: null, hodnota: 14.57, istota: 0.32,
        dovod: "Výmena pripraví lepšiu pozíciu na ďalší deň, ale dnes nič nezíska." },
      { akcia: "scratch", nazov: "Škrabanec", ciel: 9, hodnota: 12.1, istota: 0.21,
        dovod: "Bezpečnejšie, ale pomalšie." },
    ],
    // `board` je v kontrakte od začiatku; náhľad ho pridal až s TLI 8, keď
    // panel začal podľa neho kresliť portrét väzňa na cieľovom políčku.
    board: {
      day: 7, drill_days: 20, food_chain: "single", warden: 3, vault: 2,
      destinations: [6], tools: [], used: { devour: false, swap: false },
      prey: null, pending_event: null,
      cells: Array.from({ length: 16 }, (_, i) => ({
        index: i, sealed: false,
        inmate: i === 6 ? { beast: "mammoth", tier: 3 }
              : i === 7 ? { beast: "rat", tier: 1 } : null,
      })),
    },
    istota: 0.41,
    istota_rozpoznania: 0.87,
    skontroluj: ["cells[7].inmate.tier", "warden"],
    polia: [],
  },
};

document.body.classList.add("nahlad");
const posli = () => {
  const f = (window.__nahlad_poslucháči || {})["stav"];
  if (f) f({ payload: UKAZKA });
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
