// E2E test viditeľnej spätnej väzby po F8/F9 (TLI 24, `2026-10-17-1238-mcp.md`
// bod 3) — appka predtým mala len JEDNU fázu "analyzujem" pre snímku aj pre
// čakanie na odpoveď servera naraz; teraz má dve: "Snímam…" hneď po snímke,
// "Odoslané, čakám na radu…" tesne pred HTTP požiadavkou.
//
// Beží na Linuxe (`node --test ui/testy-stav-fazy.e2e.mjs`), rovnaký vzor ako
// `testy-rozlozenie.e2e.mjs`: index.html + overlay.js v jsdom, Tauri podsunuté
// cez `nahlad-stub.js`.

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

test("fáza 'analyzujem' ukáže text 'Snímam…'", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", { faza: "analyzujem" });

    const indikator = document.getElementById("indikator");
    assert.ok(!indikator.classList.contains("skryte"), "indikátor má byť viditeľný");
    assert.equal(document.getElementById("indikator-text").textContent, "Snímam…");
});

test("fáza 'odosielam' ukáže text 'Odoslané, čakám na radu…'", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", { faza: "odosielam" });

    const indikator = document.getElementById("indikator");
    assert.ok(!indikator.classList.contains("skryte"), "indikátor má byť viditeľný");
    assert.equal(document.getElementById("indikator-text").textContent, "Odoslané, čakám na radu…");
});

test("postupnosť analyzujem -> odosielam -> chyba skryje indikátor a ukáže chybu", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", { faza: "analyzujem" });
    posliUdalost(window, "stav", { faza: "odosielam" });
    posliUdalost(window, "stav", { faza: "chyba", sprava: "Server neprijal tvoj kľúč." });

    const indikator = document.getElementById("indikator");
    assert.ok(indikator.classList.contains("skryte"), "indikátor sa má pri chybe skryť");
    const chyba = document.getElementById("chyba");
    assert.ok(!chyba.classList.contains("skryte"));
    assert.equal(chyba.textContent, "Server neprijal tvoj kľúč.");
});
