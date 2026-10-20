// E2E testy (TLI 62/2b, doplnok 1 k TLI 62 `2026-10-20-0158-mcp.md`, bod 4):
// posuvníky priehľadnosti/hrúbky/farby trasy nad minimapou v Nastaveniach.
// Richard 20.10.2026: „nech sa tam dá nastaviť opacita."
//
// Dôvod, prečo tu test je: sú tu presne dve miesta, kde sa to dá potichu
// pokaziť, a obe už appku raz stáli zápis v CLAUDE.md:
//   1. `vzhlad.opacita_vrstvy || 100` — NULA je platná hodnota („dočasne to
//      nechcem vidieť"), a `||` by ju vrátilo na 100 %. Hráč by si trasu
//      nevedel stíšiť na nulu, hoci posuvník ide od nuly.
//   2. zabudnuté pole v tele `uloz_nastavenia` — uloženie formulára by
//      nastavený vzhľad zahodilo.
// Oboje sa overuje nad SKUTOČNÝM `nastavenia.html` + `nastavenia.js` v jsdom,
// rovnakým vzorom ako `testy-nastavenia-ucenie-canvas.e2e.mjs`.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));
const NASTAVENIA_HTML = readFileSync(path.join(TU, "nastavenia.html"), "utf8");
const NASTAVENIA_JS = readFileSync(path.join(TU, "nastavenia.js"), "utf8");
const OVERLAY_HTML = readFileSync(path.join(TU, "index.html"), "utf8");
const OVERLAY_JS = readFileSync(path.join(TU, "overlay.js"), "utf8");

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
                if (meno === "verzia") return Promise.resolve("0.23.0");
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

function odosli(window, document) {
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
}

test("staré nastavenia bez poľa `vzhlad_trasy` ukážu viditeľnú predvoľbu", async (t) => {
    // Presne stav po aktualizácii z verzie spred TLI 62.
    const { document, dom } = postav({ ...ZAKLAD });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-trasa-opacita-vrstvy").value, "100");
    assert.equal(document.getElementById("inp-trasa-opacita-ciary").value, "85");
    assert.equal(document.getElementById("inp-trasa-hrubka").value, "3");
    assert.equal(document.getElementById("inp-trasa-farba").value, "#ffd24a");
});

test("NULA sa nesmie „opraviť“ na predvolených 100 %", async (t) => {
    const { document, dom } = postav({
        ...ZAKLAD,
        vzhlad_trasy: { opacita_ciary: 0, opacita_vrstvy: 0, hrubka_px: 6, farba: "#00FF99" },
    });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-trasa-opacita-vrstvy").value, "0");
    assert.equal(document.getElementById("inp-trasa-opacita-ciary").value, "0");
    assert.equal(document.getElementById("inp-trasa-hrubka").value, "6");
    assert.equal(document.getElementById("inp-trasa-farba").value, "#00ff99");
});

test("popisky vedľa posuvníkov idú s hodnotou", async (t) => {
    const { window, document, dom } = postav({ ...ZAKLAD });
    t.after(() => dom.window.close());
    await pockajNaInit();

    const posuvnik = document.getElementById("inp-trasa-opacita-vrstvy");
    posuvnik.value = "40";
    posuvnik.dispatchEvent(new window.Event("input", { bubbles: true }));
    assert.equal(document.getElementById("hod-trasa-opacita-vrstvy").textContent, "40");
});

test("uloženie pošle `vzhlad_trasy` ako čísla, nie reťazce", async (t) => {
    const invokeVolania = [];
    const { window, document, dom } = postav({ ...ZAKLAD }, invokeVolania);
    t.after(() => dom.window.close());
    await pockajNaInit();

    document.getElementById("inp-trasa-opacita-vrstvy").value = "35";
    document.getElementById("inp-trasa-opacita-ciary").value = "0";
    document.getElementById("inp-trasa-hrubka").value = "8";
    document.getElementById("inp-trasa-farba").value = "#112233";

    invokeVolania.length = 0;
    odosli(window, document);
    await pockajNaInit();

    const volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    assert.ok(volanie, "uloz_nastavenia sa malo zavolať");
    // `{ ...x }`, nie priamo `x`: objekt vznikol v realme jsdom okna, takže
    // má iný `Object.prototype` a `deepStrictEqual` by ho odmietol aj pri
    // zhodnom obsahu („same structure but not reference-equal").
    assert.deepStrictEqual({ ...volanie.args.nove.vzhlad_trasy }, {
        opacita_ciary: 0,
        opacita_vrstvy: 35,
        hrubka_px: 8,
        farba: "#112233",
    });
});

test("uloženie formulára nezhodí vzhľad, ktorého sa hráč nedotkol", async (t) => {
    const invokeVolania = [];
    const { window, document, dom } = postav(
        { ...ZAKLAD, vzhlad_trasy: { opacita_ciary: 20, opacita_vrstvy: 45, hrubka_px: 9, farba: "#ABCDEF" } },
        invokeVolania
    );
    t.after(() => dom.window.close());
    await pockajNaInit();

    invokeVolania.length = 0;
    odosli(window, document);
    await pockajNaInit();

    const volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    // `{ ...x }`, nie priamo `x`: objekt vznikol v realme jsdom okna, takže
    // má iný `Object.prototype` a `deepStrictEqual` by ho odmietol aj pri
    // zhodnom obsahu („same structure but not reference-equal").
    assert.deepStrictEqual({ ...volanie.args.nove.vzhlad_trasy }, {
        opacita_ciary: 20,
        opacita_vrstvy: 45,
        hrubka_px: 9,
        farba: "#abcdef",
    });
});

// ── Koliesko myši nad úchytom overlayu ──────────────────────────────────────

function postavOverlay(invokeVolania = []) {
    const dom = new JSDOM(OVERLAY_HTML, {
        url: "file://" + path.join(TU, "index.html"),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    const poslucháči = new Map();
    window.__TAURI__ = {
        event: {
            listen: (meno, f) => {
                poslucháči.set(meno, f);
                return Promise.resolve(() => {});
            },
        },
        core: {
            invoke: (meno, args) => {
                invokeVolania.push({ meno, args });
                if (meno === "opacita_trasy_kolieskom") return Promise.resolve(55);
                return Promise.resolve(null);
            },
        },
    };
    window.eval(OVERLAY_JS);
    return { window, document: window.document, dom, poslucháči };
}

function koliesko(window, prvok, deltaY) {
    const e = new window.Event("wheel", { bubbles: true, cancelable: true });
    Object.defineProperty(e, "deltaY", { value: deltaY });
    prvok.dispatchEvent(e);
}

test("koliesko mimo režimu úpravy nerobí nič", async (t) => {
    const invokeVolania = [];
    const { window, document, dom } = postavOverlay(invokeVolania);
    t.after(() => dom.window.close());

    koliesko(window, document.getElementById("panel-uchyt-presun"), 120);
    await pockajNaInit();

    assert.equal(
        invokeVolania.filter((v) => v.meno === "opacita_trasy_kolieskom").length,
        0,
        "mimo režimu úpravy je overlay click-through, koliesko sem nepatrí"
    );
});

test("koliesko v režime úpravy posiela SMER a ukáže bublinu", async (t) => {
    const invokeVolania = [];
    const { window, document, dom, poslucháči } = postavOverlay(invokeVolania);
    t.after(() => dom.window.close());

    poslucháči.get("rezim-upravy")({ payload: { zapnute: true, okno: { sirka: 1920, vyska: 1080 } } });
    const uchyt = document.getElementById("panel-uchyt-presun");

    koliesko(window, uchyt, 120); // dole = stíšiť
    await pockajNaInit();
    koliesko(window, uchyt, -120); // hore = zosilniť
    await pockajNaInit();

    const volania = invokeVolania.filter((v) => v.meno === "opacita_trasy_kolieskom");
    assert.equal(volania.length, 2);
    // Posiela sa iba ±1 — násobky z „rýchlych" myší (±120) by inak skočili
    // cez celý rozsah naraz.
    assert.equal(volania[0].args.delta, -1);
    assert.equal(volania[1].args.delta, 1);
    assert.ok(uchyt.classList.contains("ukaz-bublinu"));
    assert.match(uchyt.dataset.bublina, /55 %/);
});

test("udalosť `vzhlad-trasy` naplní CSS premenné pre kresliacu vrstvu", async (t) => {
    const { document, dom, poslucháči } = postavOverlay([]);
    t.after(() => dom.window.close());

    poslucháči.get("vzhlad-trasy")({
        payload: { opacita_vrstvy: 40, opacita_ciary: 80, hrubka_px: 7, farba: "#FFD24A", farba_rgba: "rgba(255, 210, 74, 0.320)" },
    });

    const koren = document.documentElement.style;
    assert.equal(koren.getPropertyValue("--trasa-opacita"), "0.4");
    assert.equal(koren.getPropertyValue("--trasa-hrubka"), "7px");
    assert.equal(koren.getPropertyValue("--trasa-farba"), "rgba(255, 210, 74, 0.320)");
});
