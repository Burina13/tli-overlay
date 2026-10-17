// E2E test úchytov na posun/zmenu veľkosti NATÍVNYCH okien mini-panela a
// pripomienky v režime „Upraviť rozloženie“ (TLI 26, `2026-10-17-2231-mcp.md`
// bod 1 — "mini-panel a pripomienka sa nehýbu").
//
// `testy-rozlozenie.e2e.mjs` už pokrýva HLAVNÝ panel (ťahanie DOM elementu
// vnútri okna `overlay`) — tento súbor pokrýva DRUHÚ polovicu toho istého
// bodu zadania: `mini.js`/`pripomienka.js` neťahajú DOM, ale natívne OKNO
// cez `window.__TAURI__.window.getCurrentWindow().startDragging()` /
// `.startResizeDragging()`. Overuje sa teda len DRÔTOVANIE (klik na úchyt →
// appka naozaj zavolá správnu natívnu funkciu so správnym smerom) — SAMOTNÝ
// posun okna robí Windows, nie appka, a to sa z Linuxu/jsdom otestovať nedá
// (rovnaké obmedzenie ako pri `okna.rs`, pozri hlavičku toho súboru).
//
// Skutočná PRÍČINA, prečo sa mini-panel v edit móde reálne nedal ťahať (okno
// `overlay` nad ním prekrývalo klik, lebo `.show()` v tao/Windows z princípu
// NEMENÍ z-poradie — `SWP_NOZORDER`), je v Ruste (`main.rs::posli_seed_rozlozenia`,
// `.set_focus()`) a nie je to testovateľné bez živého Windows okna — tento
// test preto dokazuje, že DRÔTOVANIE úchytov existuje a nezmizne pri budúcej
// úprave, nie že sa bug z 0.18.2 opakuje (na ten niet spôsobu, ako tu padnúť).
//
// U `pripomienka.js` tento test NAOZAJ padol na niečo iné pri prvom spustení
// (TLI 26, 17.10.2026): `window.eval(JS)` hodil `SyntaxError: missing ) after
// argument list`, lebo reťazec v edit-módovom náhľade používal obyčajnú ASCII
// úvodzovku `"` namiesto uzatváracej slovenskej `“` — JS string sa tým skončil
// uprostred vety a zvyšok súboru bol neplatný JavaScript. To znamená, že CELÝ
// `pripomienka.js` (vrátane drôtovania úchytov aj bežného toastu „Enable Log")
// bol od 0.17.0 mŕtvy — žiadny test naň dovtedy neexistoval. Oprava je
// jednoriadková (iné úvodzovky okolo reťazca), ale tento test je jediná vec,
// čo takú chybu v budúcnosti ešte PRED vydaním odhalí.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));
const NAHLAD_STUB_JS = readFileSync(path.join(TU, "nahlad-stub.js"), "utf8");

/// Postaví dané okno (mini/pripomienka) presne tak, ako appka: reálne HTML +
/// reálny JS appky, Tauri `event`/`core` podsunuté cez `nahlad-stub.js`
/// (ten istý vzor ako ostatné `testy-*.e2e.mjs`) a NAVYŠE
/// `window.__TAURI__.window.getCurrentWindow()`, ktorý appka natívne okná
/// nemajú odkiaľ inak vziať.
function postavOkno(htmlSubor, jsSubor) {
    const HTML = readFileSync(path.join(TU, htmlSubor), "utf8");
    const JS = readFileSync(path.join(TU, jsSubor), "utf8");

    const dom = new JSDOM(HTML, {
        url: "file://" + path.join(TU, htmlSubor),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;

    window.eval(NAHLAD_STUB_JS);

    const volania = { startDragging: 0, startResizeDragging: [] };
    window.__TAURI__.window = {
        getCurrentWindow: () => ({
            startDragging: () => {
                volania.startDragging += 1;
                return Promise.resolve();
            },
            startResizeDragging: (smer) => {
                volania.startResizeDragging.push(smer);
                return Promise.resolve();
            },
        }),
    };

    window.eval(JS);
    // Oba skripty si drôtovanie úchytov vešajú až na `DOMContentLoaded` —
    // ten istý vzor/dôvod ako pri `hlavne.js` v `testy-zalozky.e2e.mjs`.
    window.document.dispatchEvent(new window.Event("DOMContentLoaded", { bubbles: true }));

    return { window, document: window.document, volania };
}

function mysDole(window, el) {
    el.dispatchEvent(new window.MouseEvent("mousedown", { bubbles: true }));
}

// `mini.js` si na `DOMContentLoaded` zavesí `setInterval(obnov, 2000)` —
// bez `window.close()` po teste by časovač držal Node event loop naživo.

test("mini-panel: úchyt presunu zavolá natívne startDragging()", () => {
    const { window, document, volania } = postavOkno("mini.html", "mini.js");
    try {
        const uchyt = document.getElementById("mini-uchyt-presun");
        assert.ok(uchyt, "mini.html nemá #mini-uchyt-presun");
        mysDole(window, uchyt);
        assert.equal(volania.startDragging, 1);
    } finally {
        window.close();
    }
});

test("mini-panel: úchyt veľkosti zavolá natívne startResizeDragging(\"SouthEast\")", () => {
    const { window, document, volania } = postavOkno("mini.html", "mini.js");
    try {
        const uchyt = document.getElementById("mini-uchyt-velkost");
        assert.ok(uchyt, "mini.html nemá #mini-uchyt-velkost");
        mysDole(window, uchyt);
        assert.equal(volania.startResizeDragging.length, 1);
        assert.equal(volania.startResizeDragging[0], "SouthEast");
    } finally {
        window.close();
    }
});

test("pripomienka: úchyt presunu zavolá natívne startDragging()", () => {
    const { window, document, volania } = postavOkno("pripomienka.html", "pripomienka.js");
    try {
        const uchyt = document.getElementById("toast-uchyt-presun");
        assert.ok(uchyt, "pripomienka.html nemá #toast-uchyt-presun");
        mysDole(window, uchyt);
        assert.equal(volania.startDragging, 1);
    } finally {
        window.close();
    }
});

test("pripomienka: úchyt veľkosti zavolá natívne startResizeDragging(\"SouthEast\")", () => {
    const { window, document, volania } = postavOkno("pripomienka.html", "pripomienka.js");
    try {
        const uchyt = document.getElementById("toast-uchyt-velkost");
        assert.ok(uchyt, "pripomienka.html nemá #toast-uchyt-velkost");
        mysDole(window, uchyt);
        assert.equal(volania.startResizeDragging.length, 1);
        assert.equal(volania.startResizeDragging[0], "SouthEast");
    } finally {
        window.close();
    }
});
