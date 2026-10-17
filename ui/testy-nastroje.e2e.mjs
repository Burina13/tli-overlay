// E2E test overlay vykreslenia výberu „Torture Instrument" (TLI 25,
// `2026-10-17-1242-mcp.md` bod 1) a prepínača modulov v režime úpravy
// rozloženia (bod 5).
//
// Beží na Linuxe (`node --test ui/testy-nastroje.e2e.mjs`), bez Tauri aj bez
// Windows — nad reálnym `index.html` + `overlay.js` v jsdom, s Tauri
// podsunutým cez `nahlad-stub.js` (ten istý vzor ako `testy-rozlozenie.e2e.mjs`).

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
    return { window, document: window.document, panel, volania };
}

function posliUdalost(window, meno, payload) {
    const poslucháč = window.__nahlad_poslucháči && window.__nahlad_poslucháči[meno];
    assert.ok(poslucháč, `appka sa neprihlásila na udalosť "${meno}"`);
    poslucháč({ payload });
}

const KARTICKY = [
    { index: 0, poradie: 1, nazov: "Rozseknutá železná panna",
      anglicky: "Severed Iron Maiden", tier: "S", dovod: "Najlepšia z troch.",
      skore: 2.08, odporucany: true, neznamy: false, malo_dat: true, sporne: false,
      x: 300, y: 250, w: 280, h: 420 },
    { index: 2, poradie: 2, nazov: "Katova čierna listina",
      anglicky: "Executioner's Blacklist", tier: "A", dovod: "Dobrá do druhej polovice.",
      skore: 1.73, odporucany: false, neznamy: false, malo_dat: true, sporne: false,
      x: 900, y: 250, w: 280, h: 420 },
    { index: 1, poradie: 3, nazov: "Čosi neznáme", anglicky: "Čosi neznáme",
      tier: "?", dovod: "Nepoznám.", skore: null, odporucany: false, neznamy: true,
      malo_dat: true, sporne: false, x: 600, y: 250, w: 280, h: 420 },
];

test("F8 na výbere nástroja orámuje kartičky a odporúčanú zvýrazní", () => {
    const { window, document } = postavOverlay();
    try {
        posliUdalost(window, "stav", {
            faza: "hotovo_nastroj",
            karticky: KARTICKY,
            analyza: { istota: 0.84, skontroluj: [], nastroje: KARTICKY },
        });

        const ramiky = document.querySelectorAll("#vrstva .karticka");
        assert.equal(ramiky.length, 3);
        const odporucane = document.querySelectorAll("#vrstva .karticka-odporucana");
        assert.equal(odporucane.length, 1, "odporúčaná smie byť práve jedna");
        assert.equal(odporucane[0].style.left, "300px");

        // Štítok nesie tier aj poradie; pri odporúčanej aj výzvu.
        const stitky = [...document.querySelectorAll(".karticka-stitok")]
            .map((s) => s.textContent);
        assert.ok(stitky.some((t) => t.includes("S") && t.includes("ber túto")));
        assert.ok(stitky.some((t) => t.includes("?") && t.includes("nepoznám")));

        // Dôvod je pri kartičke, nielen v paneli — hráč sa nemusí pozerať inam.
        const dovody = [...document.querySelectorAll(".karticka-dovod")]
            .map((d) => d.textContent);
        assert.ok(dovody.includes("Najlepšia z troch."));

        const panel = document.getElementById("panel");
        assert.ok(!panel.classList.contains("skryte"));
        assert.equal(document.getElementById("panel-nadpis").textContent,
                     "Ber: Rozseknutá železná panna");
        assert.match(document.getElementById("panel-istota").textContent, /84 %/);
        // V paneli je zoznam všetkých troch, zoradený podľa poradia.
        const vPaneli = [...document.querySelectorAll("#panel-alternativy .alt-nazov")]
            .map((e) => e.textContent);
        assert.deepEqual(vPaneli, ["Rozseknutá železná panna", "Katova čierna listina",
                                   "Čosi neznáme"]);
    } finally {
        window.close();
    }
});

test("keď server nepozná ani jednu kartičku, appka nič neodporučí", () => {
    const { window, document } = postavOverlay();
    try {
        const nezname = KARTICKY.map((k) => ({ ...k, odporucany: false, neznamy: true,
                                               tier: "?", skore: null }));
        posliUdalost(window, "stav", {
            faza: "hotovo_nastroj", karticky: nezname,
            analyza: { istota: 0.15, skontroluj: ["neznámy nástroj"], nastroje: nezname },
        });
        assert.equal(document.querySelectorAll(".karticka-odporucana").length, 0);
        assert.match(document.getElementById("panel-nadpis").textContent, /nepoznám/);
        assert.match(document.getElementById("panel-dovod").textContent, /#torchlight/);
    } finally {
        window.close();
    }
});

test("fáza „skry“ (neznáma obrazovka) nenechá na obrazovke nič visieť", () => {
    const { window, document } = postavOverlay();
    try {
        posliUdalost(window, "stav", {
            faza: "hotovo_nastroj", karticky: KARTICKY,
            analyza: { istota: 0.8, skontroluj: [], nastroje: KARTICKY },
        });
        assert.equal(document.querySelectorAll("#vrstva .karticka").length, 3);

        posliUdalost(window, "stav", { faza: "skry" });
        assert.equal(document.getElementById("vrstva").children.length, 0);
        assert.ok(document.getElementById("panel").classList.contains("skryte"));
        assert.ok(document.getElementById("chyba").classList.contains("skryte"));
    } finally {
        window.close();
    }
});

test("režim úpravy ponúka prepínač modulov a prepnutie nahlási Rustu", () => {
    const { window, document, volania } = postavOverlay();
    try {
        posliUdalost(window, "rezim-upravy", {
            zapnute: true,
            panel: { x: 24, y: 24, w: 460, h: 260 },
            priehladnost: 1,
            okno: { sirka: 1536, vyska: 864 },
            moduly: [
                { kluc: "panel:tartarus", popis: "F8 — doska" },
                { kluc: "panel:nastroj", popis: "F8 — výber nástroja" },
                { kluc: "panel:canvas", popis: "F9 — Frozen Canvas" },
                { kluc: "panel:artistry", popis: "Artistry" },
            ],
            aktivny_modul: "panel:tartarus",
        });

        // Lišta je VON z panela (TLI 25) — keby sa vrátila dovnútra, zmenšenie
        // panela by ju orezalo a tento test by to zachytil.
        const lista = document.getElementById("rezim-lista");
        assert.ok(lista && !lista.classList.contains("skryte"));
        assert.ok(!document.getElementById("panel").contains(lista));

        const tlacidla = [...document.querySelectorAll("#rezim-lista .prepinac-tlacidlo")];
        assert.equal(tlacidla.length, 4);
        assert.ok(tlacidla[0].classList.contains("aktivny"));
        assert.equal(tlacidla[2].textContent, "F9 — Frozen Canvas");

        tlacidla[2].dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        const volanie = volania.find((v) => v.prikaz === "rezim_upravy_prepni_panel");
        assert.ok(volanie, "prepnutie sa musí nahlásiť Rustu");
        assert.equal(volanie.argy.kluc, "panel:canvas");
        // Posiela sa ŽIVÁ poloha panela, aby sa predošlý modul uložil tam,
        // kde ho hráč práve nechal.
        assert.equal(volanie.argy.x, 24);
        assert.equal(volanie.argy.w, 460);

        // Ukončenie režimu lištu upratá — nesmie ostať visieť nad hrou.
        posliUdalost(window, "rezim-upravy", { zapnute: false });
        assert.ok(lista.classList.contains("skryte"));
        assert.equal(lista.innerHTML, "");
    } finally {
        window.close();
    }
});
