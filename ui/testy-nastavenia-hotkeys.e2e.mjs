// E2E testy (TLI 39, bod 1 a bod 4, `2026-10-18-1613-mcp.md`): Nastavenia už
// neprijmú holú klávesu (písmeno/číslicu/medzerník) bez modifikátora pre
// žiadnu z troch klávesových skratiek — presne toto 18.10.2026 spôsobilo
// „F8 nefunguje" (Tartarus omylom prepísaný na holé „DIGIT3" cez nahrávanie
// stlačením, appka tým hre ukradla klávesu „3"). Over sa to nad skutočným
// `nastavenia.html` + `nastavenia.js` v jsdom, rovnaký vzor ako
// `testy-mini-pripomienka.e2e.mjs` (manuálny `DOMContentLoaded` po evale).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));
const NASTAVENIA_HTML = readFileSync(path.join(TU, "nastavenia.html"), "utf8");
const NASTAVENIA_JS = readFileSync(path.join(TU, "nastavenia.js"), "utf8");

const HOTOVE_NASTAVENIA = {
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

function postav(invokeVolania = []) {
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
                if (meno === "nacitaj_nastavenia") return Promise.resolve(HOTOVE_NASTAVENIA);
                if (meno === "verzia") return Promise.resolve("0.18.12");
                if (meno === "rezim_upravy_je_aktivny") return Promise.resolve(false);
                if (meno === "hotkey_upozornenia") return Promise.resolve([]);
                if (meno === "uloz_nastavenia") return Promise.resolve(null);
                return Promise.resolve(null);
            },
        },
    };
    // Appka pri potvrdení skratky volá natívny `window.confirm` — testy ho
    // defaultne akceptujú, jednotlivé testy si ho vedia prepísať.
    window.confirm = () => true;
    window.eval(NASTAVENIA_JS);
    window.document.dispatchEvent(new window.Event("DOMContentLoaded"));
    return { window, document: window.document, dom };
}

// Inicializácia z `DOMContentLoaded` je `async` (čaká na `invoke(...)`) —
// testy jej dajú pár tickov na dobehnutie.
async function pockajNaInit() {
    await new Promise((r) => setTimeout(r, 0));
    await new Promise((r) => setTimeout(r, 0));
}

test("holá číslica bez modifikátora sa pri nahrávaní odmietne (presne 18.10.2026 incident)", async (t) => {
    const { window, document, dom } = postav();
    t.after(() => dom.window.close());
    await pockajNaInit();

    const btn = document.getElementById("btn-nahraj-hotkey");
    const inp = document.getElementById("inp-hotkey");
    const stav = document.getElementById("stav-hotkey");
    btn.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
    assert.equal(btn.textContent, "Stlač klávesu…");

    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "Digit3", bubbles: true }));

    assert.equal(inp.value, "F8", "appka nesmie prijať holé číslo bez modifikátora");
    assert.match(stav.textContent, /klávesu, ktorú používa hra/);
    assert.ok(stav.classList.contains("chyba"));
    // appka ostáva v režime nahrávania — nie je to zrušené, dá sa skúsiť znova
    assert.equal(btn.textContent, "Stlač klávesu…");
});

test("holé písmeno aj medzerník bez modifikátora sa odmietnu", async (t) => {
    const { window, document, dom } = postav();
    t.after(() => dom.window.close());
    await pockajNaInit();

    const btn = document.getElementById("btn-nahraj-hotkey-canvas");
    const inp = document.getElementById("inp-hotkey-canvas");
    btn.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyA", bubbles: true }));
    assert.equal(inp.value, "F9");

    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "Space", bubbles: true }));
    assert.equal(inp.value, "F9");
    assert.equal(btn.textContent, "Stlač klávesu…");
});

test("F1-F12 bez modifikátora JE platná skratka a po potvrdení sa zapíše", async (t) => {
    const { window, document, dom } = postav();
    t.after(() => dom.window.close());
    await pockajNaInit();

    const btn = document.getElementById("btn-nahraj-hotkey-canvas");
    const inp = document.getElementById("inp-hotkey-canvas");
    btn.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "F11", bubbles: true }));

    assert.equal(inp.value, "F11");
    assert.notEqual(btn.textContent, "Stlač klávesu…");
});

test("klávesa s modifikátorom je platná aj keď nie je F-klávesa", async (t) => {
    const { window, document, dom } = postav();
    t.after(() => dom.window.close());
    await pockajNaInit();

    const btn = document.getElementById("btn-nahraj-hotkey-trh");
    const inp = document.getElementById("inp-hotkey-trh");
    btn.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyA", ctrlKey: true, bubbles: true }));

    assert.equal(inp.value, "CONTROL+KeyA");
});

test("zrušenie potvrdenia necháva pôvodnú hodnotu a appka ostáva v nahrávaní", async (t) => {
    const { window, document, dom } = postav();
    t.after(() => dom.window.close());
    await pockajNaInit();
    window.confirm = () => false;

    const btn = document.getElementById("btn-nahraj-hotkey");
    const inp = document.getElementById("inp-hotkey");
    btn.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "F7", bubbles: true }));

    assert.equal(inp.value, "F8");
    assert.equal(btn.textContent, "Stlač klávesu…");
});

test("Escape počas nahrávania zruší bez zmeny hodnoty", async (t) => {
    const { window, document, dom } = postav();
    t.after(() => dom.window.close());
    await pockajNaInit();

    const btn = document.getElementById("btn-nahraj-hotkey");
    const inp = document.getElementById("inp-hotkey");
    btn.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "Escape", bubbles: true }));

    assert.equal(inp.value, "F8");
    assert.notEqual(btn.textContent, "Stlač klávesu…");
});

test("uloženie formulára s priamo napísanou holou klávesou sa odmietne a appka neposiela nič serveru", async (t) => {
    const invokeVolania = [];
    const { window, document, dom } = postav(invokeVolania);
    t.after(() => dom.window.close());
    await pockajNaInit();
    invokeVolania.length = 0; // zahodiť volania z initu (nacitaj_nastavenia, verzia...)

    // Priame prepísanie poľa (bez tlačidla „Nahrať…") je rovnako nebezpečné
    // ako nahrávanie stlačením — appka to validuje aj pri submite formulára.
    document.getElementById("inp-hotkey").value = "3";
    const vysledok = document.getElementById("vysledok-ulozenia");
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
    await pockajNaInit();

    assert.match(vysledok.textContent, /Tartarus/);
    assert.ok(
        !invokeVolania.some((v) => v.meno === "uloz_nastavenia"),
        "appka nesmie poslať neplatnú skratku appke/serveru"
    );
});

test("uloženie formulára s platnými skratkami prejde a zavolá uloz_nastavenia", async (t) => {
    const invokeVolania = [];
    const { window, document, dom } = postav(invokeVolania);
    t.after(() => dom.window.close());
    await pockajNaInit();
    invokeVolania.length = 0;

    document.getElementById("inp-hotkey").value = "F8";
    document.getElementById("inp-hotkey-canvas").value = "F9";
    document.getElementById("inp-hotkey-trh").value = "";
    document.getElementById("formular").dispatchEvent(
        new window.Event("submit", { bubbles: true, cancelable: true })
    );
    await pockajNaInit();

    const volanie = invokeVolania.find((v) => v.meno === "uloz_nastavenia");
    assert.ok(volanie, "platný formulár sa musí uložiť");
    // Prázdny hotkey_trh je dovolený — appka na serveri doplní predvolenú „F10".
    assert.equal(volanie.args.nove.hotkey_trh, "");
});
