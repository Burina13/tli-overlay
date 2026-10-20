// E2E testy (TLI 70, `inbox/2026-10-20-1334-mcp.md`, bod 5): riadok „Stav
// zberu na učenie" v Nastaveniach.
//
// Prečo vôbec existuje: 20.10.2026 Richard odohral celý deň Frozen Canvasu
// a priečinok `canvas-zber` ostal prázdny — a z appky sa nedalo zistiť ani
// to, či zber vôbec beží, ani prečo nefotí. Tento riadok je jediné miesto,
// kde to hráč uvidí bez čítania denníka, takže sa oplatí ho otestovať.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));
const NASTAVENIA_HTML = readFileSync(path.join(TU, "nastavenia.html"), "utf8");
const NASTAVENIA_JS = readFileSync(path.join(TU, "nastavenia.js"), "utf8");

const ZAKLAD = {
    url_servera: "https://tli.tvojtatko.sk",
    kluc: "test1234",
    hotkey: "F8",
    hotkey_canvas: "F9",
    hotkey_trh: "F10",
    proces_hry: "Torchlight Infinite",
    autoskrytie_s: 20,
    zvuk: false,
    zber_dat_zapnute: false,
    rozlozenie: { priehladnost: 1 },
};

const PRAZDNY = { odoslane: 0, caka: 0, posledna_chyba: null };

function postav(stavZberu) {
    const dom = new JSDOM(NASTAVENIA_HTML, {
        url: "file://" + path.join(TU, "nastavenia.html"),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    window.__TAURI__ = {
        core: {
            invoke: (meno) => {
                if (meno === "nacitaj_nastavenia") return Promise.resolve({ ...ZAKLAD });
                if (meno === "verzia") return Promise.resolve("0.28.0");
                if (meno === "rezim_upravy_je_aktivny") return Promise.resolve(false);
                if (meno === "hotkey_upozornenia") return Promise.resolve([]);
                if (meno === "stav_zberu") return Promise.resolve(stavZberu);
                return Promise.resolve(null);
            },
        },
    };
    window.confirm = () => true;
    window.eval(NASTAVENIA_JS);
    window.document.dispatchEvent(new window.Event("DOMContentLoaded"));
    return { window, document: window.document, dom };
}

async function pockajNaInit() {
    for (let i = 0; i < 6; i++) await new Promise((r) => setTimeout(r, 0));
}

test("veta má tvar zo zadania a ukáže poslednú chybu", async (t) => {
    const { document, dom } = postav({
        canvas: { odoslane: 12, caka: 3, posledna_chyba: null },
        tartarus: { ...PRAZDNY, odoslane: 1 },
        minimapa: { odoslane: 0, caka: 1488, posledna_chyba: "Server má poruchu" },
        dovody: {},
        potvrdena_obrazovka: "canvas",
        potvrdena_pred_s: 7,
    });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(
        document.getElementById("stav-zberu").textContent,
        "Učenie: Canvas 12 odoslaných / 3 čaká, Tartarus 1 odoslaných / 0 čaká, "
        + "Minimapa 0 odoslaných / 1488 čaká, posledná chyba: Server má poruchu",
    );
    assert.match(document.getElementById("stav-zberu-detail").textContent,
                 /obrazovku „canvas“ \(pred 7 s\)/);
});

test("bez chyby sa napíše „žiadna“, nie prázdno", async (t) => {
    const { document, dom } = postav({
        canvas: { ...PRAZDNY }, tartarus: { ...PRAZDNY }, minimapa: { ...PRAZDNY },
        dovody: {}, potvrdena_obrazovka: null, potvrdena_pred_s: null,
    });
    t.after(() => dom.window.close());
    await pockajNaInit();
    assert.match(document.getElementById("stav-zberu").textContent,
                 /posledná chyba: žiadna$/);
    // Keď server ešte nič nepotvrdil, hráč má vedieť, čo s tým — nie prázdny riadok.
    assert.match(document.getElementById("stav-zberu-detail").textContent, /Ctrl\+W/);
});

test("dôvody, prečo sa nefotilo, sú vidieť a zoradené od najčastejšieho", async (t) => {
    const { document, dom } = postav({
        canvas: { ...PRAZDNY }, tartarus: { ...PRAZDNY }, minimapa: { ...PRAZDNY },
        dovody: { "obrazovka-sa-nezbiera": 2, "hra-nie-je-v-popredi": 9 },
        potvrdena_obrazovka: "doska", potvrdena_pred_s: 400,
    });
    t.after(() => dom.window.close());
    await pockajNaInit();
    const detail = document.getElementById("stav-zberu-detail").textContent;
    assert.match(detail, /hra-nie-je-v-popredi ×9, obrazovka-sa-nezbiera ×2/);
});
