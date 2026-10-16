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

function nastavPriehladnost(hodnota) {
    if (typeof hodnota === "number" && isFinite(hodnota)) {
        document.documentElement.style.setProperty("--panel-opacity", String(hodnota));
    }
}

document.addEventListener("DOMContentLoaded", () => {
    invoke("nacitaj_nastavenia")
        .then((n) => nastavPriehladnost(n && n.rozlozenie && n.rozlozenie.priehladnost))
        .catch(() => {});

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

    // TLI 21 — posúvanie/zmena veľkosti tohto okna v režime „Upraviť rozloženie".
    // V edit móde ukazuje appka NÁHĽAD (nie skutočný toast, ten zmizne sám).
    window.__TAURI__.event.listen("rezim-upravy", (udalost) => {
        const p = udalost.payload || {};
        const zapnute = !!p.zapnute;
        document.body.classList.toggle("rezim-upravy", zapnute);
        nastavPriehladnost(p.priehladnost);
        if (zapnute) {
            vykresli("Náhľad pripomienky „Enable Log" — presuň ma a stlač Enter alebo Esc.", "varovanie");
        }
    });
    window.__TAURI__.event.listen("priehladnost", (udalost) => {
        nastavPriehladnost((udalost.payload || {}).priehladnost);
    });

    if (window.__TAURI__.window) {
        const okno = window.__TAURI__.window.getCurrentWindow();
        const uchytPresun = document.getElementById("toast-uchyt-presun");
        const uchytVelkost = document.getElementById("toast-uchyt-velkost");
        if (uchytPresun) {
            uchytPresun.addEventListener("mousedown", () => {
                okno.startDragging().catch((e) => console.error("Presun pripomienky zlyhal:", e));
            });
        }
        if (uchytVelkost) {
            uchytVelkost.addEventListener("mousedown", () => {
                okno.startResizeDragging("SouthEast")
                    .catch((e) => console.error("Zmena veľkosti pripomienky zlyhala:", e));
            });
        }
    }
});
