// Toast pripomienky „Enable Log" (TLI 11d). Dve udalosti, dva smery:
//
// 1. `tracker-pripomienka` — globálna udalosť z vlákna trackera na pozadí
//    (`src-tauri/src/tracker.rs`). Toto okno na ňu reaguje volaním príkazu
//    `ukaz_pripomienku`, ktorý v Ruste rozhodne o POLOHE (nad hrou, keď sa
//    nájde) a naplánuje automatické skrytie — tá logika sa nedubluje v JS.
// 2. `pripomienka-obsah` — Rust ju pošle TOMUTO oknu späť (`emit_to`) hneď,
//    ako ho ukáže, s textom a farbou, ktoré sa majú vykresliť.
const { invoke } = window.__TAURI__.core;

function vykresli(text, trieda) {
    const toast = document.getElementById("toast");
    const textEl = document.getElementById("toast-text");
    textEl.textContent = text || "";
    toast.classList.remove("varovanie", "ok");
    toast.classList.add(trieda === "ok" ? "ok" : "varovanie");
}

document.addEventListener("DOMContentLoaded", () => {
    if (!window.__TAURI__ || !window.__TAURI__.event) {
        return;
    }

    window.__TAURI__.event.listen("pripomienka-obsah", (udalost) => {
        const p = udalost.payload || {};
        vykresli(p.text, p.trieda);
    });

    window.__TAURI__.event.listen("tracker-pripomienka", (udalost) => {
        const p = udalost.payload || {};
        const trieda = p.typ === "ok" ? "ok" : "varovanie";
        invoke("ukaz_pripomienku", { text: p.text, trieda }).catch((e) => {
            console.error("Pripomienka Enable Log sa nedala zobraziť:", e);
        });
    });
});
