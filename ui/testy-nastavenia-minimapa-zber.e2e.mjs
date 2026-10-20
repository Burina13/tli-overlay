// E2E testy (TLI 62/3, `inbox/2026-10-20-0600-agent.md`): nový prepínač
// „Zber minimapy" v Nastaveniach, PREDVOLENE VYPNUTÝ (na rozdiel od
// „Učenie Canvas na pozadí", ktoré je predvolene zapnuté) — appka nesmie
// fotiť minimapu nikomu, kto si to sám nezapol (ToS,
// `minimapa-api-kontrakt.md` § „Richard rozhodol").
//
// Rovnaký vzor ako `testy-nastavenia-ucenie-canvas.e2e.mjs`: jediné dve
// miesta, kde sa predvolená hodnota dá potichu pokaziť, sú `!!undefined`
// v JS (opačne než pri Canvase — tu MUSÍ byť `false`) a zabudnuté pole v
// tele `uloz_nastavenia`.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));
const NASTAVENIA_HTML = readFileSync(path.join(TU, "nastavenia.html"), "utf8");
const NASTAVENIA_JS = readFileSync(path.join(TU, "nastavenia.js"), "utf8");

function postav(nastavenia, invokeVolania = []) {
    const dom = new JSDOM(NASTAVENIA_HTML, {
        url: "file://" + path.join(TU, "nastavenia.html"),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    window.__TAURI__ = {
        core: {
            invoke: (meno, args) => {
                invokeVolania.push({ meno, args });
                if (meno === "nacitaj_nastavenia") return Promise.resolve(nastavenia);
                if (meno === "verzia") return Promise.resolve("0.22.0");
                if (meno === "rezim_upravy_je_aktivny") return Promise.resolve(false);
                if (meno === "hotkey_upozornenia") return Promise.resolve([]);
                if (meno === "uloz_nastavenia") return Promise.resolve(null);
                return Promise.resolve(null);
            },
        },
    };
    window.confirm = () => true;
    window.eval(NASTAVENIA_JS);
    window.document.dispatchEvent(new window.Event("DOMContentLoaded"));
    return { window, document: window.document, dom };
}

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

async function pockajNaInit() {
    await new Promise((r) => setTimeout(r, 0));
    await new Promise((r) => setTimeout(r, 0));
}

test("staré nastavenia bez poľa ukážu zber minimapy ako VYPNUTÝ", async (t) => {
    const { document, dom } = postav({ ...ZAKLAD });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-minimapa-zber").checked, false);
    assert.equal(document.getElementById("inp-minimapa-rezim").value, "stred");
});

test("výslovné true a režim roh sa rešpektujú", async (t) => {
    const { document, dom } = postav({ ...ZAKLAD, minimapa_zber_zapnute: true, minimapa_rezim: "roh" });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-minimapa-zber").checked, true);
    assert.equal(document.getElementById("inp-minimapa-rezim").value, "roh");
});

test("neznáma hodnota režimu padne na stred, nie na prázdny výber", async (t) => {
    const { document, dom } = postav({ ...ZAKLAD, minimapa_rezim: "nezmysel" });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-minimapa-rezim").value, "stred");
});

test("je to iný prepínač než „Učenie Canvas“", async (t) => {
    const { document, dom } = postav({ ...ZAKLAD });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-ucenie-canvas").checked, true);
    assert.equal(document.getElementById("inp-minimapa-zber").checked, false);
});

test("uloženie pošle minimapa_zber_zapnute a minimapa_rezim podľa formulára", async (t) => {
    const invokeVolania = [];
    const { window, document, dom } = postav({ ...ZAKLAD }, invokeVolania);
    t.after(() => dom.window.close());
    await pockajNaInit();

    // Nedotknutý formulár musí uložiť `false` — appka nesmie zber potichu zapnúť.
    invokeVolania.length = 0;
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
    await pockajNaInit();
    let volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    assert.ok(volanie, "uloz_nastavenia sa malo zavolať");
    assert.equal(volanie.args.nove.minimapa_zber_zapnute, false);
    assert.equal(volanie.args.nove.minimapa_rezim, "stred");

    // Po zaškrtnutí a zmene režimu musí ísť `true`/„roh".
    document.getElementById("inp-minimapa-zber").checked = true;
    document.getElementById("inp-minimapa-rezim").value = "roh";
    invokeVolania.length = 0;
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
    await pockajNaInit();
    volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    assert.ok(volanie);
    assert.equal(volanie.args.nove.minimapa_zber_zapnute, true);
    assert.equal(volanie.args.nove.minimapa_rezim, "roh");
});
