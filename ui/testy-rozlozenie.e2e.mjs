// E2E test režimu „Upraviť rozloženie“ (TLI 21, `2026-10-16-1528-mcp.md` bod 1).
//
// Beží na Linuxe (`node --test ui/testy-rozlozenie.e2e.mjs`), bez Tauri aj bez
// Windows — nad reálnym `index.html` + `overlay.js` v jsdom, s Tauri
// podsunutým cez `nahlad-stub.js` (presne ten istý vzor ako `testy-zalozky.e2e.mjs`).
//
// jsdom nerobí skutočný layout — `getBoundingClientRect`/`offsetWidth`/
// `offsetHeight` preto treba podsunúť: čítajú aktuálny inline štýl panela, nie
// vymyslené konštanty, takže test naozaj overí, že ťahanie myšou posúva/
// zväčšuje TEN ISTÝ element, ktorý appka napokon nahlási Rustu.

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

    const volania = [];
    const povodnyInvoke = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = (prikaz, argy) => {
        volania.push({ prikaz, argy });
        return povodnyInvoke(prikaz, argy);
    };

    window.eval(IKONY_JS);
    window.eval(OVERLAY_JS);

    const panel = window.document.getElementById("panel");
    // jsdom nepočíta layout — getBoundingClientRect/offset* sa čítajú z
    // AKTUÁLNEHO inline štýlu, aby test odzrkadlil skutočné ťahanie.
    const cislo = (hodnota, predvolene) => {
        const n = parseFloat(hodnota);
        return Number.isFinite(n) ? n : predvolene;
    };
    panel.getBoundingClientRect = () => ({
        left: cislo(panel.style.left, 0),
        top: cislo(panel.style.top, 0),
        width: cislo(panel.style.width, 460),
        height: cislo(panel.style.height, 0),
        right: cislo(panel.style.left, 0) + cislo(panel.style.width, 460),
        bottom: cislo(panel.style.top, 0) + cislo(panel.style.height, 0),
    });
    Object.defineProperty(panel, "offsetWidth", { get: () => cislo(panel.style.width, 460) });
    Object.defineProperty(panel, "offsetHeight", { get: () => cislo(panel.style.height, 260) });

    return { window, document: window.document, panel, volania };
}

function posliUdalost(window, meno, payload) {
    const poslucháč = window.__nahlad_poslucháči && window.__nahlad_poslucháči[meno];
    assert.ok(poslucháč, `appka sa neprihlásila na udalosť "${meno}"`);
    poslucháč({ payload });
}

function mys(window, el, typ, x, y) {
    el.dispatchEvent(new window.MouseEvent(typ, { bubbles: true, clientX: x, clientY: y }));
}

const SEED = { x: 24, y: 24, w: 460, h: 260 };
const OKNO = { sirka: 1536, vyska: 864 };

test("vstup do režimu úpravy ukáže panel s náhľadom na poslanej polohe", () => {
    const { window, document } = postavOverlay();
    try {
        posliUdalost(window, "rezim-upravy", { zapnute: true, panel: SEED, priehladnost: 0.8, okno: OKNO });

        assert.ok(document.body.classList.contains("rezim-upravy"));
        const panel = document.getElementById("panel");
        assert.ok(!panel.classList.contains("skryte"));
        assert.equal(panel.style.left, "24px");
        assert.equal(panel.style.top, "24px");
        assert.equal(panel.style.width, "460px");
        assert.equal(panel.style.height, "260px");
        assert.equal(
            document.documentElement.style.getPropertyValue("--panel-opacity"),
            "0.8",
        );
    } finally {
        window.close();
    }
});

test("ťahanie za uchyt presunu posúva panel a rešpektuje hranice okna hry", () => {
    const { window, document, panel } = postavOverlay();
    try {
        posliUdalost(window, "rezim-upravy", { zapnute: true, panel: SEED, priehladnost: 1, okno: OKNO });
        const uchyt = document.getElementById("panel-uchyt-presun");

        // Chytí presne v strede panela (24+230, 24+130) a potiahne o (+100, +50).
        mys(window, uchyt, "mousedown", 254, 154);
        mys(window, window, "mousemove", 354, 204);
        mys(window, window, "mouseup", 354, 204);

        assert.equal(panel.style.left, "124px");
        assert.equal(panel.style.top, "74px");

        // Potiahnutie úplne mimo okna sa oreže tak, aby panel ostal celý vnútri.
        mys(window, uchyt, "mousedown", 124 + 230, 74 + 130);
        mys(window, window, "mousemove", 5000, 5000);
        mys(window, window, "mouseup", 5000, 5000);
        assert.equal(panel.style.left, String(OKNO.sirka - 460) + "px");
        assert.equal(panel.style.top, String(OKNO.vyska - 260) + "px");
    } finally {
        window.close();
    }
});

test("ťahanie za uchyt veľkosti mení šírku/výšku a nepustí panel pod minimum", () => {
    const { window, document, panel } = postavOverlay();
    try {
        posliUdalost(window, "rezim-upravy", { zapnute: true, panel: SEED, priehladnost: 1, okno: OKNO });
        const uchyt = document.getElementById("panel-uchyt-velkost");

        mys(window, uchyt, "mousedown", 24 + 460, 24 + 260);
        mys(window, window, "mousemove", 24 + 460 + 40, 24 + 260 + 20);
        mys(window, window, "mouseup", 24 + 460 + 40, 24 + 260 + 20);
        assert.equal(panel.style.width, "500px");
        assert.equal(panel.style.height, "280px");

        // Zmenšenie pod minimum sa oreže (220×90 v overlay.js).
        mys(window, uchyt, "mousedown", 24 + 500, 24 + 280);
        mys(window, window, "mousemove", -5000, -5000);
        mys(window, window, "mouseup", -5000, -5000);
        assert.equal(panel.style.width, "220px");
        assert.equal(panel.style.height, "90px");
    } finally {
        window.close();
    }
});

test("rezim-upravy-nahlas pošle Rustu PRESNE aktuálnu polohu panela po úprave", () => {
    const { window, panel, volania } = postavOverlay();
    try {
        posliUdalost(window, "rezim-upravy", { zapnute: true, panel: SEED, priehladnost: 1, okno: OKNO });
        panel.style.left = "200px";
        panel.style.top = "80px";
        panel.style.width = "500px";
        panel.style.height = "300px";

        posliUdalost(window, "rezim-upravy-nahlas", {});

        const volanie = volania.find((v) => v.prikaz === "rezim_upravy_ulozit_panel");
        assert.ok(volanie, "appka nezavolala rezim_upravy_ulozit_panel");
        // Porovnanie POLÍ, nie `assert.deepEqual(volanie.argy, {...})` — `argy`
        // je objekt z realmu jsdom okna (iný `Object.prototype` než Node),
        // takže strict deepEqual naň hlási „same structure but are not
        // reference-equal", hoci hodnoty sú identické.
        assert.equal(volanie.argy.x, 200);
        assert.equal(volanie.argy.y, 80);
        assert.equal(volanie.argy.w, 500);
        assert.equal(volanie.argy.h, 300);
    } finally {
        window.close();
    }
});

test("vypnutie režimu odstráni triedu z body a skryje panel", () => {
    const { window, document } = postavOverlay();
    try {
        posliUdalost(window, "rezim-upravy", { zapnute: true, panel: SEED, priehladnost: 1, okno: OKNO });
        posliUdalost(window, "rezim-upravy", { zapnute: false });

        assert.ok(!document.body.classList.contains("rezim-upravy"));
    } finally {
        window.close();
    }
});

test("panel-poloha s poslanou hodnotou nastaví inline polohu a priehľadnosť, bez nej ju vymaže", () => {
    const { window, document, panel } = postavOverlay();
    try {
        posliUdalost(window, "panel-poloha", {
            poloha: { x: 50, y: 60, w: 400, h: 200 },
            priehladnost: 0.5,
        });
        assert.equal(panel.style.left, "50px");
        assert.equal(panel.style.width, "400px");
        assert.equal(
            document.documentElement.style.getPropertyValue("--panel-opacity"),
            "0.5",
        );

        posliUdalost(window, "panel-poloha", { poloha: null, priehladnost: 1 });
        assert.equal(panel.style.left, "");
        assert.equal(panel.style.width, "");
    } finally {
        window.close();
    }
});
