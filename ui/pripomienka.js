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
    const cisty = (text || "").trim();
    textEl.textContent = cisty;
    toast.classList.remove("varovanie", "ok");
    toast.classList.add(trieda === "ok" ? "ok" : "varovanie");
    // TLI 37 — prázdny rám sa nekreslí. Okno môže byť zobrazené skôr, než
    // doletí text (Rust ho posiela pred `show()`), a rám bez textu vyzerá
    // ako zaseknutá appka.
    toast.classList.toggle("prazdny", cisty === "");
}

function nastavPriehladnost(hodnota) {
    if (typeof hodnota === "number" && isFinite(hodnota)) {
        document.documentElement.style.setProperty("--panel-opacity", String(hodnota));
    }
}

document.addEventListener("DOMContentLoaded", () => {
    // Kým nepríde text, okno nesmie kresliť rám (TLI 37).
    vykresli("", "varovanie");
    invoke("nacitaj_nastavenia")
        .then((n) => nastavPriehladnost(n && n.rozlozenie && n.rozlozenie.priehladnost))
        .catch(() => {});

    // Poistka proti pretekom: keď `pripomienka-obsah` doletela skôr, než sa
    // toto okno stihlo prihlásiť, payload je nenávratne preč a ostal by
    // prázdny rám. Rust si posledný text drží, takže si ho appka vypýta
    // sama. `.catch` — starší Rust tento príkaz nemá a nesmie to nič rozbiť.
    invoke("posledna_pripomienka")
        .then((p) => {
            if (p && (p.text || "").trim()) {
                vykresli(p.text, p.trieda);
            }
        })
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
            vykresli('Náhľad pripomienky „Enable Log" — presuň ma a stlač Enter alebo Esc.', "varovanie");
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
