// E2E test (TLI 32, `2026-10-18-1101-mcp.md`): keď server vie odporúčanie
// priradiť ku konkrétnemu tlačidlu spodnej lišty ("Go to Cell N",
// "Consume"/"Replace"/"End"...), appka zvýrazní TO tlačidlo a cela sa
// nezvýrazňuje vôbec (žiadna trieda "ciel", žiadna šípka) — presne zadanie
// Richarda: "zvýrazniť tlačidlo akcie dole, nie šípku na celu". Keď sa zhoda
// nenašla, appka sa vráti k pôvodnému správaniu (TLI 8+): cela + šípka.
//
// Rovnaký vzor ako `testy-zobraz-doska-a-ine.e2e.mjs`: index.html + overlay.js
// v jsdom, Tauri podsunuté cez `nahlad-stub.js`.

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

// Tvar presne podľa reálnej analýzy `2026-10-18-105603-67f8c864.json` (CT 114)
// po prepočte appkou (`e2e_doska_s_tlacidlom_spodnej_listy` v `e2e_mock.rs`) —
// patrol na cieľ 6 ("Go to Cell 7"), alternatíva na cieľ 7 ("Go to Cell 8").
const ANALYZA_S_TLACIDLOM = {
    obrazovka: "doska",
    istota: 0.332,
    skontroluj: [],
    odporucanie: {
        akcia: "patrol", nazov: "Patrola", ciel: 6, cez_scratch: false,
        nastroj: null, dar: null,
        dovod: "Zamknuté dvere prinášajú nový materiál pre reťaz.",
        hodnota: 33.4667,
    },
    alternativy: [
        { akcia: "patrol", nazov: "Patrola", ciel: 7, hodnota: 33.05,
          istota: 0.1833, dovod: "..." },
    ],
    board: { cells: [] },
};

const OBDLZNIKY_S_TLACIDLOM = [
    { index: 6, x: 980, y: 290, w: 118, h: 166, stredX: 1039, stredY: 373, neiste: false },
    { index: 7, x: 980, y: 440, w: 118, h: 166, stredX: 1039, stredY: 523, neiste: false },
];

const TLACIDLO = { x: 420, y: 996, w: 250, h: 255, text: "Go to Cell 7" };
const TLACIDLO_ALT = { x: 681, y: 996, w: 250, h: 255, text: "Go to Cell 8" };

test("tlačidlo nájdené: zvýrazní sa tlačidlo, cela ostáva bez zvýraznenia, šípka sa nekreslí", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo",
        analyza: ANALYZA_S_TLACIDLOM,
        obdlzniky: OBDLZNIKY_S_TLACIDLOM,
        tlacidlo: TLACIDLO,
        tlacidlo_alt: TLACIDLO_ALT,
        mierka: 1, sirka: 2560, vyska: 1440, trvanie_ms: 11368,
    });

    assert.equal(
        document.getElementById("panel-nadpis").textContent,
        "Patrola → tlačidlo „Go to Cell 7“",
        "panel má hovoriť v pojmoch tlačidla, nie cely",
    );

    // Cela sa NEzvýrazňuje — žiadna trieda "ciel", žiadna šípka.
    assert.equal(document.querySelectorAll("#vrstva .pole.ciel").length, 0);
    assert.ok(document.getElementById("sipka").classList.contains("skryte"));
    // Obe cely sa aj tak nakreslia (len bez zvláštneho zvýraznenia).
    assert.equal(document.querySelectorAll("#vrstva .pole").length, 2);

    const najlepsie = document.querySelectorAll("#vrstva .tlacidlo-akcie:not(.tlacidlo-alt)");
    assert.equal(najlepsie.length, 1, "najlepšie tlačidlo sa má zvýrazniť");
    assert.equal(najlepsie[0].style.left, "420px");
    assert.equal(najlepsie[0].style.width, "250px");
    assert.match(najlepsie[0].textContent, /Patrola/);
    assert.match(najlepsie[0].textContent, /33 %/);

    const alternativne = document.querySelectorAll("#vrstva .tlacidlo-akcie.tlacidlo-alt");
    assert.equal(alternativne.length, 1, "druhé najlepšie tlačidlo sa má zvýrazniť slabšie");
    assert.equal(alternativne[0].style.left, "681px");
});

test("tlačidlo NEnájdené (iný panel/akcia): appka sa vráti k pôvodnému zvýrazneniu cely a šípke", async () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo",
        analyza: ANALYZA_S_TLACIDLOM,
        obdlzniky: OBDLZNIKY_S_TLACIDLOM,
        tlacidlo: null,
        tlacidlo_alt: null,
        mierka: 1, sirka: 2560, vyska: 1440, trvanie_ms: 11368,
    });

    assert.equal(
        document.getElementById("panel-nadpis").textContent,
        "Patrola → políčko 6",
    );
    const cielove = document.querySelectorAll("#vrstva .pole.ciel");
    assert.equal(cielove.length, 1, "bez tlačidla sa cela zvýrazní ako doteraz");
    assert.equal(document.querySelectorAll("#vrstva .tlacidlo-akcie").length, 0);
    // Šípka sa kreslí v `requestAnimationFrame` (overlay.js) — počkáme na tik,
    // rovnako ako by appka čakala na vykreslenie prehliadačom.
    await new Promise((r) => window.requestAnimationFrame(() => setTimeout(r, 0)));
    assert.ok(!document.getElementById("sipka").classList.contains("skryte"));
});

test("druhé 'hotovo' bez tlačidla zmaže staré zvýraznenie tlačidla z predošlej snímky", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo", analyza: ANALYZA_S_TLACIDLOM, obdlzniky: OBDLZNIKY_S_TLACIDLOM,
        tlacidlo: TLACIDLO, tlacidlo_alt: TLACIDLO_ALT,
        mierka: 1, sirka: 2560, vyska: 1440, trvanie_ms: 11368,
    });
    assert.equal(document.querySelectorAll("#vrstva .tlacidlo-akcie").length, 2);

    posliUdalost(window, "stav", {
        faza: "hotovo", analyza: ANALYZA_S_TLACIDLOM, obdlzniky: OBDLZNIKY_S_TLACIDLOM,
        tlacidlo: null, tlacidlo_alt: null,
        mierka: 1, sirka: 2560, vyska: 1440, trvanie_ms: 9000,
    });
    assert.equal(document.querySelectorAll("#vrstva .tlacidlo-akcie").length, 0);
    assert.equal(document.querySelectorAll("#vrstva .pole.ciel").length, 1);
});
