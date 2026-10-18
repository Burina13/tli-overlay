// E2E test overlay vykreslenia UNIVERZÁLNEHO PORADCU (TLI 36,
// `2026-10-18-1323-mcp.md`). Richard: „chcem mačknúť F8 a nech mi vždy pomôže
// pri hocijakom výbere v tej minihre" — tento súbor overuje to, čo z toho
// vidí hráč: zlatý rámik na odporúčanej možnosti podľa bounding boxu,
// slabší na druhej, panel s dôvodom. NIKDY nie len „neviem".
//
// Beží na Linuxe (`node --test ui/testy-poradca.e2e.mjs`), bez Tauri aj bez
// Windows — nad reálnym `index.html` + `overlay.js` v jsdom, rovnaký vzor ako
// `testy-nastroje.e2e.mjs`.

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
    // Časovače si zachytávame, nech sa „po 8 sekundách" dá otestovať bez
    // osemsekundového čakania. `overlay.js` je vyhodnotený v scope okna,
    // takže holé `setTimeout(...)` v ňom siahne práve sem.
    const naplanovane = [];
    window.setTimeout = (fn, ms) => {
        naplanovane.push({ fn, ms, zrusene: false });
        return naplanovane.length;
    };
    window.clearTimeout = (id) => {
        if (naplanovane[id - 1]) naplanovane[id - 1].zrusene = true;
    };
    window.eval(IKONY_JS);
    window.eval(OVERLAY_JS);
    return { window, document: window.document, naplanovane };
}

function posliUdalost(window, meno, payload) {
    const poslucháč = window.__nahlad_poslucháči && window.__nahlad_poslucháči[meno];
    assert.ok(poslucháč, `appka sa neprihlásila na udalosť "${meno}"`);
    poslucháč({ payload });
}

// Reálny prípad: panel „Make Your Decision" (Consume/Replace/End) z Richardovej
// snímky 18.10.2026 13:16:48. Solver na nej odporučil patrolu, ktorá sa
// k žiadnemu z tých tlačidiel nepriradila — preto obrazovku prevzal poradca.
const KARTICKY = [
    {
        index: 0, poradie: 1, tier: "S", skore: null,
        nazov: "Consume", anglicky: "Consume",
        dovod: "Väzeň zožerie cieľ rovnakej rarity a stúpne o stupeň.",
        odporucany: true, neznamy: false, malo_dat: false, sporne: false,
        x: 368, y: 621, w: 272, h: 144,
    },
    {
        index: 1, poradie: 2, tier: "A", skore: null,
        nazov: "Replace", anglicky: "Replace",
        dovod: "Vymení cieľ za iný.",
        odporucany: false, neznamy: false, malo_dat: false, sporne: false,
        x: 656, y: 621, w: 272, h: 144,
    },
    {
        index: 2, poradie: 3, tier: "C", skore: null,
        nazov: "End", anglicky: "End",
        dovod: "Ukončí ťah bez efektu.",
        odporucany: false, neznamy: false, malo_dat: false, sporne: false,
        x: 944, y: 621, w: 272, h: 144,
    },
];

function udalostPoradcu(zmeny = {}) {
    const analyza = {
        ok: true,
        modul: "tartarus-poradca",
        obrazovka: "poradca",
        typ_obrazovky: "make_your_decision",
        nazov_obrazovky: "Rozhodnutie o cieli väzňa",
        istota: 0.8,
        istota_slovom: "vysoka",
        odporucanie: {
            index: 0, nazov: "Consume",
            dovod: "Cieľ má rovnakú raritu, požretie je priamy krok k Chromatic.",
        },
        skontroluj: [],
        moznosti: KARTICKY,
        ...(zmeny.analyza || {}),
    };
    return {
        faza: "hotovo_poradca",
        analyza,
        karticky: zmeny.karticky !== undefined ? zmeny.karticky : KARTICKY,
        mierka: 1, sirka: 1600, vyska: 900, trvanie_ms: 9405,
    };
}

test("poradca: zlatý rámik na odporúčanej možnosti, panel s dôvodom, nikdy nie „neviem“", () => {
    const { window, document } = postavOverlay();
    try {
        posliUdalost(window, "stav", udalostPoradcu());

        const ramiky = [...document.querySelectorAll("#vrstva .karticka")];
        assert.equal(ramiky.length, 3, "každá možnosť dostane rámik podľa svojho boxu");

        const odporucane = document.querySelectorAll("#vrstva .karticka-odporucana");
        assert.equal(odporucane.length, 1, "odporúčaná smie byť práve jedna");
        assert.equal(odporucane[0].style.left, "368px");
        assert.equal(odporucane[0].style.width, "272px");

        // Rámiky sedia na bounding boxoch zo servera, nie na nejakom rastri.
        assert.deepEqual(ramiky.map((r) => r.style.left), ["368px", "656px", "944px"]);

        const stitky = [...document.querySelectorAll(".karticka-stitok")]
            .map((s) => s.textContent);
        assert.ok(stitky.some((s) => s.includes("S") && s.includes("ber túto")),
            "prvá možnosť je označená ako tá, ktorú má hráč kliknúť");

        const panel = document.getElementById("panel");
        assert.ok(!panel.classList.contains("skryte"), "panel s radou sa musí ukázať");
        const nadpis = document.getElementById("panel-nadpis").textContent;
        assert.match(nadpis, /Rozhodnutie o cieli väzňa/);
        assert.match(nadpis, /Consume/);

        const dovod = document.getElementById("panel-dovod").textContent;
        assert.match(dovod, /Chromatic/);
        assert.doesNotMatch(dovod, /neviem/i);

        // Istota sa hlási SLOVOM — poradca neráta Monte Carlo nad známym
        // stavom a percento by predstieralo presnosť, ktorú rada nemá.
        const istota = document.getElementById("panel-istota").textContent;
        assert.match(istota, /vysoka/);
        assert.match(istota, /poradca/);
    } finally {
        window.close();
    }
});

test("poradca bez umiestnených možností: panel aj tak poradí, len bez rámikov", () => {
    const { window, document } = postavOverlay();
    try {
        posliUdalost(window, "stav", udalostPoradcu({ karticky: [] }));
        assert.equal(document.querySelectorAll("#vrstva .karticka").length, 0);
        const panel = document.getElementById("panel");
        assert.ok(!panel.classList.contains("skryte"));
        assert.match(document.getElementById("panel-nadpis").textContent,
            /Rozhodnutie o cieli väzňa/);
        // Možnosti sa vypíšu aspoň do panela, nech hráč vie, čo má kliknúť.
        assert.ok(document.getElementById("panel-alternativy").textContent
            .includes("Consume"));
    } finally {
        window.close();
    }
});

test("dlhé čakanie zmení text indikátora na „Pracujem…“ a výsledok ho schová", () => {
    const { window, document, naplanovane } = postavOverlay();
    try {
        posliUdalost(window, "stav", { faza: "analyzujem" });
        posliUdalost(window, "stav", { faza: "odosielam" });
        const indikator = document.getElementById("indikator");
        assert.ok(!indikator.classList.contains("skryte"));
        assert.match(indikator.textContent, /čakám na radu/);

        // Poradca je DRUHÉ volanie modelu nad tou istou snímkou a vie
        // presiahnuť cieľových 8 s (reálne 9,4 s nad doskou 18.10.2026) —
        // hráč nesmie mať pocit, že appka zamrzla.
        const cakanie = naplanovane.find((n) => n.ms === 8000 && !n.zrusene);
        assert.ok(cakanie, "po odoslaní sa musí naplánovať prepnutie textu na „Pracujem…“");
        cakanie.fn();
        assert.match(indikator.textContent, /Pracujem…/);

        posliUdalost(window, "stav", udalostPoradcu());
        assert.ok(indikator.classList.contains("skryte"),
            "výsledok musí indikátor schovať");
    } finally {
        window.close();
    }
});
