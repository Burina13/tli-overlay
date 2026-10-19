// TLI 57: Zber dát už nemá blokovať radu solvera (predtým fáza "zber_ulozene"
// zobrazila len "uložené #N" a žiadnu radu) a nerozpoznané plátno má hráčovi
// povedať zrozumiteľnú vetu namiesto ticha — rovnaký testovací vzor ako
// `testy-stav-fazy.e2e.mjs` (jsdom, index.html + overlay.js, Tauri cez
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

test("zber dát zapnuté A ZÁROVEŇ je krok: panel ukáže počítadlo aj radu (nie len 'uložené')", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo_canvas",
        zber_pocet: 7,
        kroky: [{
            poradie: 1,
            dovod: "Over F9 pred aj po položení paintu.",
            zdroj: { x: 10, y: 10, w: 20, h: 20 },
            ciel: { x: 50, y: 50, w: 20, h: 20 },
        }],
        analyza: { istota: 0.9, pravidlo_overene: true, platno: { ciel: { smer: "adjacent" } } },
    });

    const nadpis = document.getElementById("panel-nadpis").textContent;
    assert.match(nadpis, /Zber dát #7/, "počítadlo uloženia má ostať vidieť");
    assert.match(nadpis, /Frozen Canvas — 1 krok/, "rada sa má vykresliť, nie zablokovať");
    assert.equal(document.getElementById("panel-dovod").textContent, "Over F9 pred aj po položení paintu.");
});

test("zber dát vypnuté, žiadny krok, plátno nerozpoznané: zrozumiteľná veta, nie ticho", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo_canvas",
        kroky: [],
        analyza: { istota: 0, skontroluj: ["ciel"], platno: { ciel: null } },
    });

    assert.equal(document.getElementById("panel-nadpis").textContent, "Frozen Canvas — nerozpoznané");
    assert.equal(
        document.getElementById("panel-dovod").textContent,
        "Nerozpoznal som plátno – otvor Frozen Canvas na celú obrazovku."
    );
});

test("žiadny krok, ale plátno JE rozpoznané (cieľ asi splnený): pôvodná, menej alarmujúca veta", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo_canvas",
        kroky: [],
        analyza: { istota: 0, skontroluj: [], platno: { ciel: { smer: "adjacent" } } },
    });

    assert.equal(document.getElementById("panel-nadpis").textContent, "Frozen Canvas — žiadny krok");
    assert.match(document.getElementById("panel-dovod").textContent, /Server nenavrhol/);
});
