// E2E test overlay vykreslenia obrazovky OBJECTIVE (TLI 37,
// `2026-10-18-1324-mcp.md`). Richard: „podľa objective sa má upraviť celá
// stratégia" — tento súbor overuje to, čo z toho vidí hráč: potvrdenie cieľa,
// 2–3 vety plánu, rámik na tlačidle Confirm a riadok s priebehom, ktorý je
// vidieť aj na INÝCH obrazovkách (doska, kartičky).
//
// Beží na Linuxe (`node --test ui/testy-objective.e2e.mjs`), bez Tauri aj bez
// Windows — nad reálnym `index.html` + `overlay.js` v jsdom, rovnaký vzor ako
// `testy-poradca.e2e.mjs`.

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
    window.setTimeout = () => 1;
    window.clearTimeout = () => {};
    window.eval(IKONY_JS);
    window.eval(OVERLAY_JS);
    return { window, document: window.document };
}

function posliUdalost(window, meno, payload) {
    const poslucháč = window.__nahlad_poslucháči && window.__nahlad_poslucháči[meno];
    assert.ok(poslucháč, `appka sa neprihlásila na udalosť "${meno}"`);
    poslucháč({ payload });
}

// Reálna snímka Richarda 18.10.2026 13:23:25 (Stage Level 77).
const OBJECTIVE = {
    kod: "survival_of_the_fittest",
    anglicky: "Survival of the Fittest",
    nazov: "Prežije najsilnejší",
    podmienka: "Consume 4 times",
    akcia_sk: "Požretie (Consume)",
    ciel_pocet: 4, aktualny: 0, zostava: 4, splneny: false,
    progres_text: "0/4", progres_presnost: "odhad", v_katalogu: true,
    odmena_sk: ["Vyberieš si o 1 nástroj navyše", "Ultimate alebo Chromatic Warden's Cache"],
};

const PAYLOAD = {
    faza: "hotovo_objective",
    mierka: 1.0, sirka: 1600, vyska: 900,
    karticky: [{
        index: 0, poradie: 1, tier: "S", skore: null,
        nazov: "Confirm", anglicky: "Confirm",
        dovod: "Potvrdíš cieľ a pustíš sa do drilu.",
        odporucany: true, neznamy: false, malo_dat: false, sporne: false,
        x: 928, y: 612, w: 256, h: 54,
    }],
    analyza: {
        ok: true, modul: "tartarus-objective", obrazovka: "objective",
        nazov_obrazovky: "Objective: Survival of the Fittest",
        istota: 0.75,
        potvrdenie: "Objective: Požretie (Consume) 4× — stratégia upravená",
        plan: ["Požretie je aj tak hlavný spôsob, ako dvíhať raritu.",
               "Keď sa dá vybrať medzi požretím a koncom dňa, ber požretie.",
               "Nástroj, ktorý pridáva požretia, má teraz hodnotu navyše."],
        objective: OBJECTIVE,
        odporucanie: { index: 0, nazov: "Confirm", dovod: "Potvrď a hraj." },
        skontroluj: [],
    },
};

test("obrazovka Objective ukáže potvrdenie, plán aj rámik na Confirm", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", PAYLOAD);

    assert.equal(document.getElementById("panel-nadpis").textContent,
                 "Objective: Požretie (Consume) 4× — stratégia upravená");
    const dovod = document.getElementById("panel-dovod").textContent;
    assert.ok(dovod.includes("Požretie je aj tak hlavný spôsob"), dovod);
    assert.ok(dovod.includes("hodnotu navyše"), "plán má byť celý, nie prvá veta");

    const ramiky = document.querySelectorAll("#vrstva .karticka");
    assert.equal(ramiky.length, 1);
    assert.equal(ramiky[0].style.left, "928px");
    assert.ok(ramiky[0].className.includes("karticka-odporucana"));
    assert.ok(ramiky[0].textContent.includes("klikni sem"));

    // Odmena ide do zoznamu pod panelom, nie do dôvodu.
    const odmeny = document.getElementById("panel-alternativy");
    assert.ok(!odmeny.className.includes("skryte"));
    assert.ok(odmeny.textContent.includes("Ultimate alebo Chromatic"));

    assert.ok(!document.getElementById("panel").className.includes("skryte"));
});

test("priebeh cieľa je vidieť aj na doske, nie len na obrazovke Objective", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo",
        mierka: 1.0, sirka: 1600, vyska: 900,
        obdlzniky: [], analyza: {
            ok: true, obrazovka: "doska", istota: 0.7,
            odporucanie: { akcia: "devour", nazov: "Požretie", ciel: 3,
                           dovod: "Berieme Consume — chýbajú 2 do objective." },
            alternativy: [], skontroluj: [],
            objective: { ...OBJECTIVE, aktualny: 2, zostava: 2, progres_text: "2/4" },
        },
    });
    const riadok = document.getElementById("panel-objective");
    assert.ok(!riadok.className.includes("skryte"), "riadok cieľa má byť viditeľný");
    assert.ok(riadok.textContent.includes("Prežije najsilnejší"), riadok.textContent);
    assert.ok(riadok.textContent.includes("2/4"), riadok.textContent);
    assert.ok(riadok.textContent.includes("odhad"),
              "appka musí priznať, že priebeh je odhad z F8 snímok");
});

test("bez bežiaceho cieľa sa riadok nekreslí vôbec", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo", mierka: 1.0, sirka: 1600, vyska: 900, obdlzniky: [],
        analyza: { ok: true, obrazovka: "doska", istota: 0.7,
                   odporucanie: { akcia: "devour", nazov: "Požretie", dovod: "x" },
                   alternativy: [], skontroluj: [] },
    });
    const riadok = document.getElementById("panel-objective");
    assert.ok(riadok.className.includes("skryte"));
    assert.equal(riadok.textContent, "");
});

test("splnený cieľ sa hlási ako splnený a inou farbou", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo", mierka: 1.0, sirka: 1600, vyska: 900, obdlzniky: [],
        analyza: { ok: true, obrazovka: "doska", istota: 0.7,
                   odporucanie: { akcia: "patrol", nazov: "Patrola", dovod: "x" },
                   alternativy: [], skontroluj: [],
                   objective: { ...OBJECTIVE, aktualny: 4, zostava: 0, splneny: true,
                                progres_text: "4/4" } },
    });
    const riadok = document.getElementById("panel-objective");
    assert.ok(riadok.textContent.includes("splnené"), riadok.textContent);
    assert.ok(riadok.className.includes("splnene"));
});

test("cieľ, ktorého priebeh sa merať nedá, to povie namiesto vymysleného čísla", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo", mierka: 1.0, sirka: 1600, vyska: 900, obdlzniky: [],
        analyza: { ok: true, obrazovka: "doska", istota: 0.7,
                   odporucanie: { akcia: "patrol", nazov: "Patrola", dovod: "x" },
                   alternativy: [], skontroluj: [],
                   objective: { kod: "zahada", anglicky: "Riddle", nazov: "Riddle",
                                podmienka: "Do something odd", akcia_sk: "neznáma akcia",
                                ciel_pocet: null, aktualny: null, zostava: null,
                                splneny: false, progres_text: "",
                                progres_presnost: "nemerateľné", v_katalogu: false } },
    });
    const riadok = document.getElementById("panel-objective");
    assert.ok(riadok.textContent.includes("Do something odd"), riadok.textContent);
    assert.ok(riadok.textContent.includes("neviem merať"), riadok.textContent);
});
