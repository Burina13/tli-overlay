// TLI 69 (20.10.2026): panel Frozen Canvasu musí pri NEOVERENOM smere radu
// UKÁZAŤ a pomenovať smer („⚠️ neoverené pre vertical"), nie napísať „žiadny
// krok". Zároveň musí rozlíšiť, či cieľové bunky orámovala sama hra (isté),
// alebo sa odhadli z názvu smeru. Rovnaký testovací vzor ako
// `testy-canvas-zber.e2e.mjs` (jsdom, index.html + overlay.js, Tauri cez
// `nahlad-stub.js`).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));
const OVERLAY_HTML = readFileSync(path.join(TU, "index.html"), "utf8");
const OVERLAY_JS = readFileSync(path.join(TU, "overlay.js"), "utf8");
const IKONY_JS = readFileSync(path.join(TU, "ikony.js"), "utf8");
const NAHLAD_STUB_JS = readFileSync(path.join(TU, "nahlad-stub.js"), "utf8");

function postavOverlay() {
    const dom = new JSDOM(OVERLAY_HTML, {
        url: "file://" + path.join(TU, "index.html"),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    window.eval(NAHLAD_STUB_JS);
    window.eval(IKONY_JS);
    window.eval(OVERLAY_JS);
    return { window, document: window.document };
}

function posliUdalost(window, meno, payload) {
    const poslucháč = window.__nahlad_poslucháči && window.__nahlad_poslucháči[meno];
    assert.ok(poslucháč, `appka sa neprihlásila na udalosť "${meno}"`);
    poslucháč({ payload });
}

/** Richardov prípad: cieľ „Fill a Vertical Cell…", zdroj r6c3, hra orámovala
 *  r1c3/r2c3/r4c3/r5c3. Tri kroky, smer neoverený. */
function udalostVertical(zdrojDosahu) {
    return {
        faza: "hotovo_canvas",
        kroky: [1, 2, 3].map((p) => ({
            poradie: p,
            dovod: `Vezmi ghost Lv${8 - p} zo slotu ${p} v ruke a polož ho na bunku.`,
            zdroj: { x: 10 * p, y: 700, w: 20, h: 20 },
            ciel: { x: 300, y: 100 * p, w: 20, h: 20 },
        })),
        analyza: {
            istota: 0.72,
            pravidlo_overene: false,
            zdroj_dosahu: zdrojDosahu,
            skontroluj: ["dosah je istý (hra bunky sama orámovala), ale EFEKT pre smer 'vertical' zatiaľ nie je overený"],
            kroky: [1, 2, 3].map((p) => ({ poradie: p, zdroj: { typ_slotu: "ruka", index: p } })),
            platno: { ciel: { smer: "vertical" }, ruka: [] },
        },
    };
}

test("neoverený smer: rada sa UKÁŽE a nadpis pomenuje smer, nie len ⚠️", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVertical("ram"));

    const nadpis = document.getElementById("panel-nadpis").textContent;
    assert.match(nadpis, /neoverené pre vertical/,
        "Richard žiadal menovite „neoverené pre vertical“, nie samotné ⚠️");
    assert.match(nadpis, /Frozen Canvas — 3 krokov/, "rada sa má vykresliť, nie zablokovať");
    assert.doesNotMatch(nadpis, /žiadny krok/);
    assert.ok(!document.getElementById("panel").classList.contains("skryte"));
});

test("dvojice zdroj→cieľ sú očíslované (3 kroky = 6 rámikov)", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVertical("ram"));

    const boxy = document.querySelectorAll("#vrstva .krok-box");
    assert.equal(boxy.length, 6, "každý krok má DVA rámiky — paint v ruke a cieľová bunka");
    const cisla = [...document.querySelectorAll("#vrstva .krok-cislo")].map((e) => e.textContent);
    assert.deepEqual(cisla, ["1", "1", "2", "2", "3", "3"], "čísla majú páriť zdroj s cieľom");
});

test("zdroj_dosahu 'ram': panel napíše, že cieľové bunky orámovala sama hra", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVertical("ram"));

    const neiste = document.getElementById("panel-skontroluj").textContent;
    assert.match(neiste, /orámovala sama hra/);
    assert.doesNotMatch(neiste, /odhadnuté z názvu smeru/);
});

test("zdroj_dosahu 'smer': panel prizná, že cieľové bunky sú odhadnuté", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVertical("smer"));

    const neiste = document.getElementById("panel-skontroluj").textContent;
    assert.match(neiste, /odhadnuté z názvu smeru/);
    assert.doesNotMatch(neiste, /orámovala sama hra/);
});

test("overený smer nadpis nešpiní — žiadne ⚠️ ani meno smeru", () => {
    const { window, document } = postavOverlay();
    const u = udalostVertical("ram");
    u.analyza.pravidlo_overene = true;
    u.analyza.platno.ciel.smer = "adjacent";
    posliUdalost(window, "stav", u);

    const nadpis = document.getElementById("panel-nadpis").textContent;
    assert.doesNotMatch(nadpis, /neoverené/);
    assert.doesNotMatch(nadpis, /⚠️/);
    assert.match(nadpis, /Frozen Canvas — 3 krokov/);
});

test("žiadny krok UŽ nesmie hráčovi tvrdiť, že smer nemá overené pravidlo", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo_canvas",
        kroky: [],
        analyza: { istota: 0, skontroluj: [], platno: { ciel: { smer: "vertical" } } },
    });

    const dovod = document.getElementById("panel-dovod").textContent;
    assert.doesNotMatch(dovod, /nemá overené pravidlo/);
    assert.match(dovod, /voľná cieľová bunka|splnený/);
});
