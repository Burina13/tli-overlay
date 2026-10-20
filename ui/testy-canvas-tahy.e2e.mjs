// TLI 71 (20.10.2026): na podobrazovke „Select the Brushstroke direction"
// (a „Select Brushstroke Effect") musí appka orámovať ODPORÚČANÚ kartu aj
// tlačidlo „Select" a v paneli povedať PREČO — do tohto dňa tam napísala
// „Frozen Canvas — žiadny krok", čo bola pravda a zároveň bezcenná rada
// (Richard, 20.10. ~16:3x). Rovnaký testovací vzor ako
// `testy-canvas-ramy.e2e.mjs` (jsdom, index.html + overlay.js, Tauri cez
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

/** Richardova snímka 2026-10-20-163311: ponuka [Reimagine] + [Spread],
 *  v dosahu nie je voľná bunka, server odporúča druhú kartu. */
function udalostVyberTahu(odporucanyIndex = 1) {
    const hodnotenia = [
        { index: 0, druh: "reimagine", ucel: "", skore: 70, oznacenie: "[Reimagine]",
          dovod: "Úzke hrdlo sú BUNKY: voľných cieľových buniek máš 0 z potrebných 3." },
        { index: 1, druh: "spread", ucel: "", skore: 95, oznacenie: "[Spread]",
          dovod: "Presne na to je [Spread] — maľuje na Blocked/Void/Discolored bunku." },
    ];
    return {
        faza: "hotovo_canvas",
        kroky: [],
        karty: hodnotenia.map((h) => ({
            index: h.index,
            odporucana: h.index === odporucanyIndex,
            oznacenie: h.oznacenie,
            dovod: h.dovod,
            skore: h.skore,
            x: 1040, y: 448 + h.index * 112, w: 560, h: 100,
        })),
        tlacidlo: { x: 1393, y: 813, w: 234, h: 57 },
        analyza: {
            istota: 0.75,
            pravidlo_overene: null,
            zdroj_dosahu: "smer",
            skontroluj: [],
            kroky: [],
            faza: "vyber_tahu",
            platno: { ciel: { smer: "adjacent" }, ruka: [] },
            tah: {
                faza: "vyber_tahu",
                odporucany_index: odporucanyIndex,
                druh: "spread",
                dovod: "Vyber kartu 2 — [Spread]. Úzke hrdlo sú BUNKY… Potom klikni na tlačidlo Select.",
                hodnotenia,
                istota: 0.75,
                skontroluj: [],
            },
        },
    };
}

test("odporúčaná karta je orámovaná nahrubo, ostatné nenápadne", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVyberTahu());
    const karty = [...document.querySelectorAll(".karta-box")];
    assert.equal(karty.length, 2);
    const odporucane = karty.filter((k) => k.classList.contains("karta-odporucana"));
    assert.equal(odporucane.length, 1);
    assert.equal(odporucane[0].style.top, "560px", "druhá karta má byť tá odporúčaná");
    assert.match(odporucane[0].textContent, /vyber túto/);
    const ostatne = karty.filter((k) => !k.classList.contains("karta-odporucana"));
    assert.match(ostatne[0].textContent, /70\/100/);
});

test("tlačidlo Select sa orámuje tiež — bez neho sa výber nepotvrdí", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVyberTahu());
    const t = document.querySelector(".karta-tlacidlo");
    assert.ok(t, "tlačidlo Select nie je orámované");
    assert.equal(t.style.left, "1393px");
    assert.match(t.textContent, /potvrď výber/);
});

test("panel povie ČO vybrať a PREČO, nie „žiadny krok“", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVyberTahu());
    const nadpis = document.getElementById("panel-nadpis").textContent;
    assert.match(nadpis, /výber ťahu štetcom/);
    assert.doesNotMatch(nadpis, /žiadny krok/);
    assert.match(document.getElementById("panel-dovod").textContent, /\[Spread\]/);
    assert.match(document.getElementById("panel-istota").textContent, /75 %/);
});

test("neodporúčané karty sú vypísané aj s dôvodom, prečo neuspeli", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVyberTahu());
    const alt = document.getElementById("panel-alternativy");
    assert.ok(!alt.classList.contains("skryte"));
    assert.match(alt.textContent, /Karta 1 — \[Reimagine\]/);
    assert.match(alt.textContent, /Úzke hrdlo sú BUNKY/);
});

test("keď server neodporučí nič, tlačidlo sa nezvýrazní", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", udalostVyberTahu(-1));
    assert.equal(document.querySelectorAll(".karta-odporucana").length, 0);
    assert.equal(document.querySelector(".karta-tlacidlo"), null);
    // panel aj tak niečo povie — fáza nesmie ostať bez textu
    assert.ok(document.getElementById("panel-dovod").textContent.length > 10);
});

test("rada sa nestratí ani keď karty nemajú súradnice", () => {
    // Nález z review diffu cez API kredit (20.10.2026): podmienka sa pôvodne
    // viazala na `data.karty`, takže odpoveď s `tah`, ale bez použiteľných
    // kariet prepadla do vetvy bežného plátna a hráč uvidel práve tú starú
    // hlášku „žiadny krok", ktorú TLI 71 odstraňuje.
    const { window, document } = postavOverlay();
    const u = udalostVyberTahu();
    u.karty = [];
    u.tlacidlo = null;
    u.analyza.tah.hodnotenia = [];
    posliUdalost(window, "stav", u);
    const nadpis = document.getElementById("panel-nadpis").textContent;
    assert.match(nadpis, /výber ťahu štetcom/);
    assert.doesNotMatch(nadpis, /žiadny krok/);
    assert.match(document.getElementById("panel-dovod").textContent, /\[Spread\]/);
    assert.equal(document.querySelectorAll(".karta-box").length, 0);
});

test("druhá podobrazovka má vlastný nadpis", () => {
    const { window, document } = postavOverlay();
    const u = udalostVyberTahu();
    u.analyza.tah.faza = "vyber_efektu";
    posliUdalost(window, "stav", u);
    assert.match(document.getElementById("panel-nadpis").textContent, /výber efektu ťahu/);
});

test("bežné plátno sa nemení — kreslia sa kroky, nie karty (regresia TLI 69)", () => {
    const { window, document } = postavOverlay();
    posliUdalost(window, "stav", {
        faza: "hotovo_canvas",
        kroky: [{ poradie: 1, dovod: "Vezmi ghost Lv1 a polož ho na bunku.",
                  zdroj: { x: 10, y: 700, w: 20, h: 20 },
                  ciel: { x: 300, y: 100, w: 20, h: 20 } }],
        analyza: { istota: 0.9, skontroluj: [], kroky: [{ poradie: 1, zdroj: { typ_slotu: "ruka", index: 0 } }],
                   platno: { ciel: { smer: "adjacent" }, ruka: [] } },
    });
    assert.equal(document.querySelectorAll(".karta-box").length, 0);
    assert.equal(document.querySelectorAll(".krok-box").length, 2);
    assert.match(document.getElementById("panel-nadpis").textContent, /1 krok/);
});

test("neisté veci sa zobrazia aj vo fáze kariet", () => {
    const { window, document } = postavOverlay();
    const u = udalostVyberTahu();
    u.analyza.skontroluj = ["plátno nemá rozpoznaný miešací cieľ — rada je VŠEOBECNÁ"];
    posliUdalost(window, "stav", u);
    const sk = document.getElementById("panel-skontroluj");
    assert.ok(!sk.classList.contains("skryte"));
    assert.match(sk.textContent, /VŠEOBECNÁ/);
});
