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

function postav(htmlSubor, jsSubor, predPrikazy = {}, predGlobal = {}) {
    const dom = new JSDOM(readFileSync(path.join(TU, htmlSubor), "utf8"), {
        url: "file://" + path.join(TU, htmlSubor),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    Object.assign(window, predGlobal);
    const poslucháči = {};
    const volania = [];
    // Minimálny Tauri shim — `invoke` vracia to, čo mu test podstrčí,
    // `listen` si odloží poslucháča, aby sa dal udalosťou zavolať.
    // `window.window` (TLI 50): `getCurrentWindow()` s `startDragging`/
    // `startResizeDragging`, ktoré hneď resolvnú — bez toho `mini.js` svoj
    // blok s úchytmi na presun/veľkosť vôbec nepripojí (`if (window.__TAURI__.
    // window)` by bolo `falsy`) a test by nemal čo kliknúť.
    window.__TAURI__ = {
        core: {
            invoke: (meno, argy) => {
                volania.push({ prikaz: meno, argy });
                return meno in predPrikazy
                    ? Promise.resolve(predPrikazy[meno])
                    : Promise.reject(new Error("neznámy príkaz " + meno));
            },
        },
        event: {
            listen: (meno, fn) => { poslucháči[meno] = fn; return Promise.resolve(() => {}); },
        },
        window: {
            getCurrentWindow: () => ({
                startDragging: () => Promise.resolve(),
                startResizeDragging: () => Promise.resolve(),
            }),
        },
    };
    window.fetch = undefined;
    window.eval(readFileSync(path.join(TU, jsSubor), "utf8"));
    window.document.dispatchEvent(new window.Event("DOMContentLoaded"));
    // `mini.js` si po načítaní nastaví opakovaný časovač obnovy — bez
    // `zavri()` by proces testu nikdy neskončil.
    return { window, document: window.document, poslucháči, volania, zavri: () => dom.window.close() };
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

test("TLI 49 bod 2: vlastná/partiová cena s vekom dostane kratší tooltip 'cena pred X min'", (t) => {
    const { window, document, zavri } = postav("mini.html", "mini.js",
        { tracker_prehlad: null });
    t.after(zavri);
    window.vykresliDropy({ top_dropy: [
        { polozka: 10002, pocet: 1, hodnota: 5.0, vek: "pred 2 min" },
    ] }, true);
    const riadky = document.querySelectorAll("#mini-dropy-zoznam .hodnota");
    assert.equal(riadky[0].title, "cena pred 2 min");
});

// ─── Ikony položiek (TLI 53 bod B) ──────────────────────────────────────────
// Doterajšia verzia (TLI 48 bod 3) ťahala `<img>` priamo z herného servera
// appky — iný pôvod než appka sama, čo CSP `img-src 'self' data:` ticho
// blokovala a ikona sa v mini-paneli nikdy nezobrazila. Teraz ide z lokálneho
// balíka `polozky-ikony.json` (`window.__IKONY_POLOZIEK__` v teste).

test("mini-panel vykreslí ikonu položky z lokálneho balíka, chýbajúca ikona nekreslí rozbitý <img>", (t) => {
    const { window, document, zavri } = postav("mini.html", "mini.js",
        { tracker_prehlad: null },
        { __IKONY_POLOZIEK__: { 10001: "data:image/svg+xml;base64,AAAA" } });
    t.after(zavri);
    window.vykresliDropy({ top_dropy: [
        { polozka: 10001, pocet: 2, hodnota: 3.5 },
        { polozka: 115, pocet: 1, hodnota: null },
    ] }, true);
    const polozky = [...document.querySelectorAll("#mini-dropy-zoznam li")];
    const sIkonou = polozky.find((li) => li.textContent.includes("#10001"));
    const img = sIkonou.querySelector("img.ikona");
    assert.ok(img, "10001 má ikonu v __IKONY_POLOZIEK__ stube");
    assert.equal(img.getAttribute("src"), "data:image/svg+xml;base64,AAAA");

    const bezIkony = polozky.find((li) => li.textContent.includes("#115"));
    assert.equal(bezIkony.querySelector("img.ikona"), null, "neznáma ikona nesmie nechať rozbitý <img>");
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

// ─── TLI 50 — zmena veľkosti mini-panela za úchyt v pravom dolnom rohu ─────

test("TLI 50: úchyt na presun je samostatný riadok, NIE absolútne položený cez obsah", (t) => {
    const { document, zavri } = postav("mini.html", "mini.js", { nacitaj_nastavenia: {} });
    t.after(zavri);
    const obal = document.querySelector(".mini-obsah");
    assert.ok(obal, "obsah musí byť vo vlastnom obale pod horným pásikom");
    assert.ok(obal.contains(document.getElementById("mini-mapa")), "nadpis mapy patrí do obalu obsahu");
    assert.ok(!obal.contains(document.getElementById("mini-uchyt-presun")), "pásik na presun NIE JE súčasťou obalu obsahu");
});

test("TLI 50: ťahanie úchytu na zmenu veľkosti po pustení uloží novú veľkosť", async (t) => {
    const { document, volania, zavri } = postav("mini.html", "mini.js", { nacitaj_nastavenia: {} });
    t.after(zavri);
    // DOMContentLoaded handler v `mini.js` si `invoke("nacitaj_nastavenia")`
    // vybavuje asynchrónne — počkaj naň, kým sa pripoja listenery na úchyty.
    await new Promise((r) => setTimeout(r, 10));

    const uchyt = document.getElementById("mini-uchyt-velkost");
    assert.ok(uchyt, "úchyt na zmenu veľkosti chýba v DOM");
    uchyt.dispatchEvent(new document.defaultView.MouseEvent("mousedown", { bubbles: true }));
    await new Promise((r) => setTimeout(r, 10));

    assert.ok(
        volania.some((v) => v.prikaz === "mini_ulozit_polohu"),
        "po zmene veľkosti (startResizeDragging resolvol) sa má zavolať mini_ulozit_polohu, inak sa veľkosť po reštarte stratí"
    );
});
