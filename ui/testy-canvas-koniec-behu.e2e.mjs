// TLI 72 (20.10.2026): na obrazovke konca behu („All Paint has been added.
// You can start painting now.", počítadlo 20/20, tlačidlá Start Over za 5
// Colored Snow a Complete) sa hra NEPÝTA, kam položiť paint — pýta sa, ktoré
// z dvoch tlačidiel stlačiť. Richard po svojom behu ~14:1x: „Ukončilo to run,
// ale máme dve dúhové, a to je vlastne oveľa lepšie, než pokračovať."
// Bez tohto panela by appka na tej obrazovke napísala „Frozen Canvas — žiadny
// krok", teda pravdu bez hodnoty (presne tá chyba, ktorú odstraňovala TLI 71
// na podobrazovke s kartami).
//
// Rovnaký testovací vzor ako `testy-canvas-tahy.e2e.mjs`.

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

/** Richardova snímka 2026-10-20-141014 (20/20) — prepínateľná na variant
 *  s dvoma dúhovými paintmi, o ktorom hovoril v zadaní. */
function udalostKoniecBehu({ akcia = "complete", naMax = 0, sucet = 59 } = {}) {
    return {
        faza: "hotovo_canvas",
        kroky: [],
        karty: [],
        dokoncenie: {
            akcia,
            dovod: "",
            cena_start_over: 5,
            odporucane: akcia === "start_over"
                ? { x: 1186, y: 810, w: 202, h: 81 }
                : { x: 1393, y: 813, w: 234, h: 57 },
            complete: { x: 1393, y: 813, w: 234, h: 57 },
            start_over: { x: 1186, y: 810, w: 202, h: 81 },
        },
        analyza: {
            istota: akcia === "complete" ? 0.64 : 0.48,
            pravidlo_overene: null,
            zdroj_dosahu: "",
            skontroluj: ["appka nevidí tvoj zostatok Colored Snow — Start Over stojí 5"],
            kroky: [],
            faza: "vsetko_polozene",
            polozene: 20,
            celkom: 20,
            platno: { ciel: null, ruka: [] },
            dokoncenie: {
                akcia,
                dovod: `Na plátne máš ${naMax} paintov na Max Paint Level 7 a súčet levelov ${sucet}.`,
                cena_start_over: 5,
                skore: { na_max: naMax, sucet, obsadenych: 25, dostupnych: 25, max_level: 7 },
                istota: akcia === "complete" ? 0.64 : 0.48,
                skontroluj: [],
            },
        },
    };
}

test("odporúčané tlačidlo je orámované nahrubo, druhé nenápadne", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostKoniecBehu());
    const boxy = [...document.querySelectorAll(".karta-box")];
    assert.equal(boxy.length, 2, "orámovať treba OBE tlačidlá, nie len odporúčané");
    const odporucane = boxy.filter((b) => b.classList.contains("karta-odporucana"));
    assert.equal(odporucane.length, 1);
    assert.equal(odporucane[0].style.left, "1393px", "pri complete má byť označené Complete");
    assert.match(odporucane[0].textContent, /✓ Complete/);
    const druhe = boxy.find((b) => !b.classList.contains("karta-odporucana"));
    assert.match(druhe.textContent, /Start Over \(5\)/, "cena Start Over musí byť vidieť");
});

test("pri start_over sa zvýrazní druhé tlačidlo", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostKoniecBehu({ akcia: "start_over", naMax: 0, sucet: 12 }));
    const odporucane = document.querySelector(".karta-odporucana");
    assert.equal(odporucane.style.left, "1186px");
    assert.match(odporucane.textContent, /Start Over/);
    assert.match(document.getElementById("panel-nadpis").textContent, /daj Start Over/);
});

test("panel povie rozhodnutie aj skóre, nie „žiadny krok“", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostKoniecBehu({ naMax: 2, sucet: 71 }));
    const nadpis = document.getElementById("panel-nadpis").textContent;
    assert.match(nadpis, /koniec behu: daj Complete/);
    assert.doesNotMatch(nadpis, /žiadny krok/);
    assert.doesNotMatch(nadpis, /nerozpoznané/);
    // Hlavné kritérium (počet dúhových) musí byť vidieť v riadku istoty.
    const istota = document.getElementById("panel-istota").textContent;
    assert.match(istota, /2× Lv7 \(dúhový\)/);
    assert.match(istota, /súčet levelov 71/);
    assert.match(document.getElementById("panel-dovod").textContent, /Max Paint Level 7/);
});

test("upozornenie o zostatku meny sa hráčovi ukáže", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostKoniecBehu());
    const sk = document.getElementById("panel-skontroluj");
    assert.ok(!sk.classList.contains("skryte"));
    assert.match(sk.textContent, /Colored Snow/);
});

test("bez súradníc tlačidiel sa panel aj tak vykreslí", () => {
    // Rovnaká úvaha ako pri kartách (TLI 71): keby panel závisel od geometrie,
    // odpoveď bez boxov by prepadla do vetvy „žiadny krok" a hráč by na
    // obrazovke s Complete/Start Over čítal, že na plátne nie je voľná bunka.
    const { window, document } = postavOverlay();
    const u = udalostKoniecBehu();
    u.dokoncenie = {};
    posliUdalost(window, "stav", u);
    assert.equal(document.querySelectorAll(".karta-box").length, 0);
    assert.match(document.getElementById("panel-nadpis").textContent, /koniec behu/);
    assert.ok(document.getElementById("panel-dovod").textContent.length > 10);
});

test("bežné plátno sa týmto panelom nezmenilo", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo_canvas",
        kroky: [{ poradie: 1, zdroj: { x: 10, y: 10, w: 40, h: 40 },
                  ciel: { x: 100, y: 100, w: 40, h: 40 }, dovod: "Vezmi ratlin Lv2…" }],
        karty: [],
        analyza: {
            istota: 0.9, pravidlo_overene: true, zdroj_dosahu: "ram", skontroluj: [],
            kroky: [{ poradie: 1, dovod: "Vezmi ratlin Lv2…" }], faza: "kreslenie",
            platno: { ciel: { smer: "adjacent" }, ruka: [] },
        },
    });
    assert.equal(document.querySelectorAll(".krok-box").length, 2);
    assert.doesNotMatch(document.getElementById("panel-nadpis").textContent, /koniec behu/);
});

test("prázdna akcia neodporučí nič — ani nadpisom, ani rámikom", () => {
    // Server sa nemusí vyjadriť (`akcia: ""`). Dovtedy to UI tichom predvoľbou
    // premenilo na „daj Complete" a hráč dostal rozhodnutie, ktoré nikto
    // neurobil. Nález z review diffu cez API kredit (20.10.2026).
    const { window, document } = postavOverlay();
    const u = udalostKoniecBehu();
    u.dokoncenie.akcia = "";
    u.analyza.dokoncenie.akcia = "";
    posliUdalost(window, "stav", u);
    assert.equal(document.querySelectorAll(".karta-odporucana").length, 0);
    assert.equal(document.querySelectorAll(".karta-box").length, 2,
                 "obe tlačidlá sa aj tak ukážu, len neutrálne");
    const nadpis = document.getElementById("panel-nadpis").textContent;
    assert.match(nadpis, /neviem poradiť/);
    assert.doesNotMatch(nadpis, /daj Complete/);
    assert.doesNotMatch(nadpis, /daj Start Over/);
});
