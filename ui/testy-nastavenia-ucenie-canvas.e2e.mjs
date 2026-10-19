// E2E testy (TLI 60, `2026-10-19-2135-mcp-2.md`, bod 4): nový prepínač
// „Učenie Canvas na pozadí" v Nastaveniach, PREDVOLENE ZAPNUTÝ. Richard
// 19.10.2026: „kľudne nech si všetko, čo spravím, fotí a na pozadí pošle
// agentovi na učenie."
//
// Dôvod, prečo tu test je: prepínač je predvolene ZAPNUTÝ, a jediné dve
// miesta, kde sa to dá potichu pokaziť, sú `!!undefined` v JS (vyplo by
// to každému, kto appku už má) a zabudnuté pole v tele `uloz_nastavenia`
// (uloženie formulára by prepínač zhodilo späť). Oboje overuje tento súbor
// nad skutočným `nastavenia.html` + `nastavenia.js` v jsdom, rovnakým
// vzorom ako `testy-nastavenia-skrytie.e2e.mjs`.

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

test("staré nastavenia bez poľa ukážu učenie Canvasu ako ZAPNUTÉ", async (t) => {
    // Presne stav po aktualizácii z verzie spred TLI 60 — `nastavenia.json`
    // pole `ucenie_canvas_zapnute` vôbec nemá. `!!undefined` by bolo `false`
    // a prvé uloženie Nastavení by hráčovi zber potichu vyplo navždy.
    const { document, dom } = postav({ ...ZAKLAD });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-ucenie-canvas").checked, true);
});

test("výslovné false sa rešpektuje", async (t) => {
    const { document, dom } = postav({ ...ZAKLAD, ucenie_canvas_zapnute: false });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-ucenie-canvas").checked, false);
});

test("je to iný prepínač než ručný „Zber dát“", async (t) => {
    // Dve nezávislé polia: automatický zber na pozadí (zapnutý) a ručný zber
    // klávesou (vypnutý). Keby sa zliali, prestane platiť jedno alebo druhé.
    const { document, dom } = postav({ ...ZAKLAD, zber_dat_zapnute: false });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-zber-dat").checked, false);
    assert.equal(document.getElementById("inp-ucenie-canvas").checked, true);
});

test("uloženie pošle ucenie_canvas_zapnute podľa zaškrtnutia", async (t) => {
    const invokeVolania = [];
    const { window, document, dom } = postav({ ...ZAKLAD }, invokeVolania);
    t.after(() => dom.window.close());
    await pockajNaInit();

    // Nedotknutý formulár musí uložiť `true` — nie `undefined` a nie `false`.
    invokeVolania.length = 0;
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
    await pockajNaInit();
    let volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    assert.ok(volanie, "uloz_nastavenia sa malo zavolať");
    assert.equal(volanie.args.nove.ucenie_canvas_zapnute, true);

    // A po odškrtnutí musí ísť `false` — hráč si to musí vedieť vypnúť.
    document.getElementById("inp-ucenie-canvas").checked = false;
    invokeVolania.length = 0;
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
    await pockajNaInit();
    volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    assert.ok(volanie);
    assert.equal(volanie.args.nove.ucenie_canvas_zapnute, false);
});
