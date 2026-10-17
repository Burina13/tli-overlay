// Ukážková odpoveď servera pre obrazovku „Select a Torture Instrument"
// (TLI 25, `2026-10-17-1242-mcp.md` bod 1) — presne to, čo appka naživo pošle
// do `overlay.js` cez udalosť "stav" s `faza: "hotovo_nastroj"`, teda UŽ po
// prepočte normalizovaných boxov na CSS pixely.
//
// Tri kartičky sú TIE, ktoré Richard naozaj odfotil 17.10.2026 o 12:40,
// vrátane poradia a tierov, aké im dá katalóg `nastroje-ss14.json` — snímka
// z CI je tak dôkaz nad reálnym prípadom, nie nad vymyslenými dátami.
const UKAZKA_NASTROJ = {
  faza: "hotovo_nastroj",
  mierka: 1.0,
  sirka: 1600,
  vyska: 900,
  trvanie_ms: 5200,
  karticky: [
    {
      index: 0, poradie: 1, tier: "S", skore: 2.08,
      nazov: "Rozseknutá železná panna",
      anglicky: "Severed Iron Maiden",
      dovod: "Keď je na doske rovnako väzňov nad aj pod raritou práve vymeneného, dostane +3 stupne.",
      odporucany: true, neznamy: false, malo_dat: true, sporne: false,
      x: 230, y: 250, w: 320, h: 400,
    },
    {
      index: 2, poradie: 2, tier: "A", skore: 1.73,
      nazov: "Katova čierna listina",
      anglicky: "Executioner's Blacklist",
      dovod: "V druhej polovici drilu dvíha každé ďalšie otvorenie dvoch náhodných väzňov.",
      odporucany: false, neznamy: false, malo_dat: true, sporne: false,
      x: 1010, y: 250, w: 320, h: 400,
    },
    {
      index: 1, poradie: 3, tier: "A", skore: 1.67,
      nazov: "Nenásytné reťaze divej beštie",
      anglicky: "Ferocious Beast's Ravenous Chains",
      dovod: "Najsilnejší väzeň každé ráno zmetie oboch susedov — dvojsečné, dosku to vyprázdňuje.",
      odporucany: false, neznamy: false, malo_dat: true, sporne: false,
      x: 620, y: 250, w: 320, h: 400,
    },
  ],
  analyza: {
    ok: true,
    modul: "tartarus-nastroj",
    obrazovka: "vyber_nastroja",
    istota: 0.79,
    skontroluj: [],
  },
};
UKAZKA_NASTROJ.analyza.nastroje = UKAZKA_NASTROJ.karticky;

document.body.classList.add("nahlad");
const posli = () => {
  const f = (window.__nahlad_poslucháči || {})["stav"];
  if (f) f({ payload: UKAZKA_NASTROJ });
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
