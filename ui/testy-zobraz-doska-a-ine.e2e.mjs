// E2E test: rovnaká odpoveď servera → rovnaké zobrazenie, nezávisle od toho,
// ČO analýzu spustilo (TLI 31, `2026-10-18-1057-mcp.md`).
//
// Prečo sa tu netestuje osobitne „cez F8" a osobitne „cez tray/Odfotiť teraz":
// v `src-tauri/src/main.rs` obe cesty volajú TÚ ISTÚ funkciu `spusti_analyzu`
// (hotkey handler priamo, tray/tlačidlo cez `naplanuj_odfotenie`, ktorá len
// pridá 2 s oneskorenie a potom zavolá rovnaké `spusti_analyzu`) — ktorákoľvek
// z nich pre rovnakú odpoveď servera vyprodukuje identický `stav` event.
// Rozdiel teda nemôže byť v tom, KTO analýzu spustil, len v tom, čo appka
// s rovnakým eventom spraví na strane JS/okna — presne to je to, čo tento
// súbor overuje nad reálnym `overlay.js` (doska, výber nástroja aj neznáma
// obrazovka). Skutočné Windows z-poradie (dôvod, prečo sa panel v TLI 31
// nezobrazoval, pozri CHANGELOG 0.18.6) jsdom na Linuxe overiť nevie — to je
// mimo appky na Richardovom ďalšom teste, logy `[zobraz]` to ukážu.
//
// Beží na Linuxe (`node --test ui/testy-zobraz-doska-a-ine.e2e.mjs`), rovnaký
// vzor ako `testy-nastroje.e2e.mjs`/`testy-stav-fazy.e2e.mjs`: index.html +
// overlay.js v jsdom, Tauri podsunuté cez `nahlad-stub.js`.

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

    const panel = window.document.getElementById("panel");
    const cislo = (hodnota, predvolene) => {
        const n = parseFloat(hodnota);
        return Number.isFinite(n) ? n : predvolene;
    };
    panel.getBoundingClientRect = () => ({
        left: cislo(panel.style.left, 0),
        top: cislo(panel.style.top, 0),
        width: cislo(panel.style.width, 460),
        height: cislo(panel.style.height, 260),
        right: cislo(panel.style.left, 0) + cislo(panel.style.width, 460),
        bottom: cislo(panel.style.top, 0) + cislo(panel.style.height, 260),
    });
    return { window, document: window.document, panel };
}

function posliUdalost(window, meno, payload) {
    const poslucháč = window.__nahlad_poslucháči && window.__nahlad_poslucháči[meno];
    assert.ok(poslucháč, `appka sa neprihlásila na udalosť "${meno}"`);
    poslucháč({ payload });
}

// Tvar presne podľa kontraktu `tli-web/app.py` (`ODPOVED` v `e2e_mock.rs`) po
// prepočte na CSS súradnice appkou — rovnaký tvar, aký appka pošle do
// `stav`/`hotovo` nezávisle od toho, či beh spustil F8 alebo tray tlačidlo.
const ANALYZA_DOSKA = {
    obrazovka: "doska",
    istota: 0.41,
    skontroluj: ["cells[7].inmate.tier"],
    odporucanie: {
        akcia: "devour", nazov: "Požretie", ciel: 6, cez_scratch: false,
        nastroj: null, dar: null, dovod: "Požretie zvyšuje raritu a nestojí nič.",
        hodnota: 15.25,
    },
    alternativy: [
        { akcia: "swap", nazov: "Výmena", ciel: null, hodnota: 14.57,
          istota: 0.32, dovod: "Výmena pripraví lepšiu pozíciu." },
    ],
    board: { cells: [] },
};

const OBDLZNIKY_DOSKA = [
    { index: 6, x: 200, y: 40, w: 90, h: 90, stredX: 245, stredY: 85, neiste: false },
    { index: 7, x: 300, y: 40, w: 90, h: 90, stredX: 345, stredY: 85, neiste: true },
];

test("F8 aj tray na doske rovnako: panel sa ukáže, cieľové políčko a šípka sedia na odporúčaní", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo",
        analyza: ANALYZA_DOSKA,
        obdlzniky: OBDLZNIKY_DOSKA,
        mierka: 1,
        sirka: 2560, vyska: 1440,
        trvanie_ms: 4120,
    });

    const panel = document.getElementById("panel");
    assert.ok(!panel.classList.contains("skryte"), "panel s výsledkom sa má ukázať");
    assert.equal(
        document.getElementById("panel-nadpis").textContent,
        "Požretie → políčko 6",
    );
    assert.match(document.getElementById("panel-istota").textContent, /41 %/);

    const polia = document.querySelectorAll("#vrstva .pole");
    assert.equal(polia.length, 2, "obe vrátené políčka sa majú vykresliť");
    const cielove = document.querySelectorAll("#vrstva .pole.ciel");
    assert.equal(cielove.length, 1, "práve jedno políčko (odporúčaný ciel) je zvýraznené");
    assert.equal(cielove[0].style.left, "200px");

    const neisteEl = document.querySelectorAll("#vrstva .pole.neiste");
    assert.equal(neisteEl.length, 1, "políčko so sporným zápisom má triedu 'neiste'");

    const skontroluj = document.getElementById("panel-skontroluj");
    assert.ok(!skontroluj.classList.contains("skryte"));
    assert.match(skontroluj.textContent, /cells\[7\]\.inmate\.tier/);

    const alternativy = document.querySelectorAll("#panel-alternativy .alternativa");
    assert.equal(alternativy.length, 1);
});

test("neznáma obrazovka ('ine'): appka len schová indikátor, veľký panel nekreslí nič", () => {
    const { window, document } = postavOverlay();
    // Rovnaká postupnosť ako pri doske — appka najprv ukáže „Snímam…".
    posliUdalost(window, "stav", { faza: "analyzujem" });
    posliUdalost(window, "stav", { faza: "odosielam" });
    posliUdalost(window, "stav", { faza: "skry" });

    assert.ok(
        document.getElementById("indikator").classList.contains("skryte"),
        "'skry' má schovať indikátor spolu so všetkým ostatným",
    );
    assert.ok(
        document.getElementById("panel").classList.contains("skryte"),
        "'ine' nemá čo kresliť do veľkého panela (toast ide cez samostatné okno pripomienky)",
    );
    assert.equal(document.getElementById("vrstva").innerHTML, "");
});

test("druhá doska PO prvej prekreslí panel namiesto ponechania starých políčok", () => {
    // Regresný test pre prípad, že by druhé 'hotovo' (napr. z ďalšieho F8 po
    // tom, čo prvé okno ešte nestihlo zaniknúť) len PRIDÁVALO k starému obsahu.
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo", analyza: ANALYZA_DOSKA, obdlzniky: OBDLZNIKY_DOSKA,
        mierka: 1, sirka: 2560, vyska: 1440, trvanie_ms: 4120,
    });
    posliUdalost(window, "stav", {
        faza: "hotovo",
        analyza: { ...ANALYZA_DOSKA, skontroluj: [], odporucanie: null },
        obdlzniky: [OBDLZNIKY_DOSKA[0]],
        mierka: 1, sirka: 2560, vyska: 1440, trvanie_ms: 3900,
    });

    assert.equal(document.querySelectorAll("#vrstva .pole").length, 1);
    assert.equal(document.getElementById("panel-nadpis").textContent, "Ťah sa nenašiel");
    assert.ok(document.getElementById("panel-skontroluj").classList.contains("skryte"));
});
