// Náhľadová náhrada za Tauri. Existuje LEN kvôli CI: `overlay.js` sa na začiatku
// prihlási na udalosť `stav`, a aby sa dal overlay vykresliť v obyčajnom
// prehliadači (a odfotiť ako artefakt behu), treba mu tú udalosť vedieť podsunúť.
// V nainštalovanej appke sa tento súbor nikdy nenačíta — `index.html` ho nemá.
window.__TAURI__ = {
  event: {
    listen: (meno, funkcia) => {
      window.__nahlad_poslucháči = window.__nahlad_poslucháči || {};
      window.__nahlad_poslucháči[meno] = funkcia;
      return Promise.resolve(() => {});
    },
  },
  core: { invoke: () => Promise.resolve(null) },
};
