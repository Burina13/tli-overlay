// E2E testy (TLI 58, `2026-10-19-2134-mcp.md`): nové nastavenie „Skryť radu:
// po kliku myšou (predvolené) / po čase". Richard 19.10.2026: „po stlačení
// solvera nech mi svieti overlay, kým nespravím klik myškou, až potom zhasne,
// namiesto času." Overuje sa nad skutočným `nastavenia.html` + `nastavenia.js`
// v jsdom, rovnaký vzor ako `testy-nastavenia-hotkeys.e2e.mjs`.

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
                if (meno === "verzia") return Promise.resolve("0.21.3");
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

test("staré nastavenia bez poľa sa zobrazia ako režim kliku, nie ako čas", async (t) => {
    // Presne stav po aktualizácii z verzie spred TLI 58: `nastavenia.json`
    // pole `skryt_radu_klikom` vôbec nemá. Keby formulár padol na „cas",
    // prvé uloženie Nastavení by hráčovi potichu vyplo novú funkciu.
    const { document, dom } = postav({ ...ZAKLAD });
    t.after(() => dom.window.close());
    await pockajNaInit();

    assert.equal(document.getElementById("inp-skrytie-rezim").value, "klik");
});

test("režim kliku zašedne a zablokuje pole „Automatické skrytie (sekundy)“", async (t) => {
    const { document, dom } = postav({ ...ZAKLAD, skryt_radu_klikom: true });
    t.after(() => dom.window.close());
    await pockajNaInit();

    const pole = document.getElementById("inp-autoskrytie");
    assert.equal(pole.disabled, true, "v režime kliku sekundy nemajú účinok");
    assert.ok(pole.closest(".skupina").classList.contains("nedostupne"));
});

test("prepnutie na „po čase“ pole so sekundami znova sprístupní", async (t) => {
    const { window, document, dom } = postav({ ...ZAKLAD, skryt_radu_klikom: true });
    t.after(() => dom.window.close());
    await pockajNaInit();

    const vyber = document.getElementById("inp-skrytie-rezim");
    vyber.value = "cas";
    vyber.dispatchEvent(new window.Event("change", { bubbles: true }));

    const pole = document.getElementById("inp-autoskrytie");
    assert.equal(pole.disabled, false);
    assert.ok(!pole.closest(".skupina").classList.contains("nedostupne"));
});

test("uloženie pošle skryt_radu_klikom podľa zvoleného režimu", async (t) => {
    const invokeVolania = [];
    const { window, document, dom } = postav({ ...ZAKLAD, skryt_radu_klikom: false }, invokeVolania);
    t.after(() => dom.window.close());
    await pockajNaInit();

    // Načítané ako „po čase" — formulár to musí ukázať a uložiť nezmenené.
    assert.equal(document.getElementById("inp-skrytie-rezim").value, "cas");
    invokeVolania.length = 0;
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
    await pockajNaInit();
    let volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    assert.ok(volanie);
    assert.equal(volanie.args.nove.skryt_radu_klikom, false);

    // A po prepnutí na klik to musí ísť na server ako `true`.
    const vyber = document.getElementById("inp-skrytie-rezim");
    vyber.value = "klik";
    vyber.dispatchEvent(new window.Event("change", { bubbles: true }));
    invokeVolania.length = 0;
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
    await pockajNaInit();
    volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    assert.ok(volanie);
    assert.equal(volanie.args.nove.skryt_radu_klikom, true);
});
