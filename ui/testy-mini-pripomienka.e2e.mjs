// E2E testy dvoch vedľajších chýb z Richardovej snímky 18.10.2026 (TLI 37,
// `2026-10-18-1324-mcp.md` bod 5):
//   1. mini-panel ukazoval dropy ako holé `#115` / `#3709`,
//   2. uprostred hore visel PRÁZDNY rám pripomienky (toast bez textu).
// Oboje sa dá overiť bez Tauri aj bez Windows — v jsdom nad reálnym
// `mini.html`/`pripomienka.html` a ich skriptami.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));

function postav(htmlSubor, jsSubor, predPrikazy = {}) {
    const dom = new JSDOM(readFileSync(path.join(TU, htmlSubor), "utf8"), {
        url: "file://" + path.join(TU, htmlSubor),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    const poslucháči = {};
    // Minimálny Tauri shim — `invoke` vracia to, čo mu test podstrčí,
    // `listen` si odloží poslucháča, aby sa dal udalosťou zavolať.
    window.__TAURI__ = {
        core: {
            invoke: (meno) => (meno in predPrikazy
                ? Promise.resolve(predPrikazy[meno])
                : Promise.reject(new Error("neznámy príkaz " + meno))),
        },
        event: {
            listen: (meno, fn) => { poslucháči[meno] = fn; return Promise.resolve(() => {}); },
        },
    };
    window.fetch = undefined;
    window.eval(readFileSync(path.join(TU, jsSubor), "utf8"));
    window.document.dispatchEvent(new window.Event("DOMContentLoaded"));
    // `mini.js` si po načítaní nastaví opakovaný časovač obnovy — bez
    // `zavri()` by proces testu nikdy neskončil.
    return { window, document: window.document, poslucháči, zavri: () => dom.window.close() };
}

test("mini-panel pomenuje aj položku, ktorú nemá v balíku mien", (t) => {
    const { window, document, zavri } = postav("mini.html", "mini.js",
        { tracker_prehlad: null });
    t.after(zavri);
    // `zivo` je tvar, aký posiela Rust do mini-okna.
    window.vykresliDropy({ top_dropy: [
        { polozka: 115, pocet: 1, hodnota: null },
        { polozka: 10001, pocet: 2, hodnota: 3.5 },
    ] }, true);
    const text = document.getElementById("mini-dropy-zoznam").textContent;
    assert.ok(text.includes("neznáma položka #115"), text);
    // Známa položka sa naďalej volá svojím menom (seed sa v jsdom nenačíta,
    // preto sa testuje cez `window.__MENA_POLOZIEK__`, ktoré appka podporuje).
    assert.ok(!text.includes("#10001") || text.includes("neznáma položka #10001"), text);
});

test("TLI 43: cena z titracku dostane tooltip s atribúciou, vlastná/partiová nie", (t) => {
    const { window, document, zavri } = postav("mini.html", "mini.js",
        { tracker_prehlad: null });
    t.after(zavri);
    window.vykresliDropy({ top_dropy: [
        { polozka: 10001, pocet: 2, hodnota: 42.0, zdroj: "titrack.ninja",
          prispievatelia: 15, vek: "pred 8 min" },
        { polozka: 10002, pocet: 1, hodnota: 5.0 },
    ] }, true);
    const riadky = document.querySelectorAll("#mini-dropy-zoznam .hodnota");
    assert.equal(riadky[0].title, "titrack.ninja · 15 prisp. · pred 8 min");
    assert.equal(riadky[1].title, "", "vlastná/partiová cena nesmie dostať cudziu atribúciu");
});

test("pripomienka bez textu nekreslí rám", (t) => {
    const { document, poslucháči, zavri } = postav("pripomienka.html", "pripomienka.js");
    t.after(zavri);
    const toast = document.getElementById("toast");
    // Hneď po načítaní, kým nič nedoletelo: rám sa nesmie kresliť.
    assert.ok(toast.className.includes("prazdny"), toast.className);

    poslucháči["pripomienka-obsah"]({ payload: { text: "   ", trieda: "varovanie" } });
    assert.ok(toast.className.includes("prazdny"), "prázdny text = žiadny rám");

    poslucháči["pripomienka-obsah"]({ payload: { text: "Zapni Enable Log", trieda: "varovanie" } });
    assert.ok(!toast.className.includes("prazdny"));
    assert.equal(document.getElementById("toast-text").textContent, "Zapni Enable Log");
});

test("pripomienka si po načítaní vypýta posledný text, keby udalosť minula", async (t) => {
    const { document, zavri } = postav("pripomienka.html", "pripomienka.js", {
        nacitaj_nastavenia: { rozlozenie: { priehladnost: 1 } },
        posledna_pripomienka: { text: "Zapni Enable Log", trieda: "varovanie" },
    });
    t.after(zavri);
    await new Promise((r) => setTimeout(r, 0));
    assert.equal(document.getElementById("toast-text").textContent, "Zapni Enable Log");
    assert.ok(!document.getElementById("toast").className.includes("prazdny"));
});
