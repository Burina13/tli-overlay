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

// `window.close()` v `t.after` je POVINNÉ v každom teste, ktorý skončí vo fáze
// `analyzujem`/`odosielam`: od TLI 67b beží počas čakania `setInterval`
// počítadla sekúnd a bez zavretia okna by sa proces `node --test` nikdy
// neskončil.

test("fáza 'analyzujem' ukáže text 'Snímam…'", (t) => {
    const { window, document } = postavOverlay();
    t.after(() => window.close());
    posliUdalost(window, "stav", { faza: "analyzujem" });

    const indikator = document.getElementById("indikator");
    assert.ok(!indikator.classList.contains("skryte"), "indikátor má byť viditeľný");
    assert.equal(document.getElementById("indikator-text").textContent, "Snímam…");
});

test("fáza 'odosielam' ukáže text 'Odoslané, čakám na radu…'", (t) => {
    const { window, document } = postavOverlay();
    t.after(() => window.close());
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

// ── TLI 67b (`2026-10-20-1348-mcp.md`, bod 3) — počítadlo sekúnd ────────────
// Richard 20.10.2026: Canvas analýza trvala 34 s a overlay celý ten čas
// ukazoval nemenné „Odoslané, čakám na radu…" — nedalo sa odlíšiť „pracuje"
// od „zamrzlo".

test("do prvej sekundy sa počítadlo nepíše (len by blikalo)", () => {
    const { window } = postavOverlay();
    assert.equal(window.textSPocitadlom("Snímam…", 1000, 1000), "Snímam…");
    assert.equal(window.textSPocitadlom("Snímam…", 1000, 1999), "Snímam…");
    window.close();
});

test("od prvej sekundy pribúda za textom počet sekúnd", () => {
    const { window } = postavOverlay();
    assert.equal(window.textSPocitadlom("Snímam…", 1000, 2000), "Snímam… (1 s)");
    assert.equal(
        window.textSPocitadlom("Odoslané, čakám na radu…", 0, 34_267),
        "Odoslané, čakám na radu… (34 s)",
    );
    window.close();
});

test("počítadlo beží od stlačenia klávesy, nie od odoslania na server", (t) => {
    const { window, document } = postavOverlay();
    t.after(() => window.close());
    // Vnútorné premenné overlay.js sú `let`, teda mimo `window` — čas sa
    // preto posúva podvrhnutým `Date.now`, nie siahnutím do nich.
    let teraz = 1_000_000;
    window.Date.now = () => teraz;

    posliUdalost(window, "stav", { faza: "analyzujem" });
    teraz += 5000; // snímanie + kódovanie JPEGu
    posliUdalost(window, "stav", { faza: "odosielam" });

    assert.equal(
        document.getElementById("indikator-text").textContent,
        "Odoslané, čakám na radu… (5 s)",
        "fáza `odosielam` nesmie počítadlo vrátiť na nulu",
    );
});

test("výsledok aj chyba počítadlo zastavia a zabudnú jeho začiatok", (t) => {
    const { window, document } = postavOverlay();
    t.after(() => window.close());
    let teraz = 1_000_000;
    window.Date.now = () => teraz;

    posliUdalost(window, "stav", { faza: "analyzujem" });
    teraz += 9000;
    posliUdalost(window, "stav", {
        faza: "chyba",
        sprava: "Server teraz neodpovedá – skús o chvíľu",
    });
    // Ďalšia analýza po chybe začína od nuly — keby si počítadlo začiatok
    // pamätalo, ukázalo by hneď „(9 s)".
    teraz += 1000;
    posliUdalost(window, "stav", { faza: "odosielam" });

    assert.equal(
        document.getElementById("indikator-text").textContent,
        "Odoslané, čakám na radu…",
    );
});
