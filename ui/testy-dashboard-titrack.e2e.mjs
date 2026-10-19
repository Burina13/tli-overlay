// E2E test dashboardu podľa TITracku (TLI 45, `2026-10-19-0024-mcp.md`).
//
// Pokrýva prvú (frontendovú) časť prestavby: sedem kariet s menami z
// TITracku, grafy Cumulative value/FE per hour, rozbalenie „čo padlo" pri
// Recent Runs, hlavičkové tlačidlá (Overlay/Economy/Cloud Sync/⚙/
// Instructions/Reset Stats) a footer „Collector". Časti 2 (CURRENT
// INVENTORY) a loot overlay bez Ctrl+F8 ešte nie sú hotové — pozri
// STAV.md `ROZPRACOVANE` a BACKLOG.md.
//
// Beží na Linuxe cez jsdom, rovnaký vzor ako `testy-zalozky.e2e.mjs`.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));
const HLAVNE_HTML = readFileSync(path.join(TU, "hlavne.html"), "utf8");
const HLAVNE_JS = readFileSync(path.join(TU, "hlavne.js"), "utf8");
const NAHLAD_STUB_JS = readFileSync(path.join(TU, "nahlad-stub.js"), "utf8");

function postavApp() {
    const dom = new JSDOM(HLAVNE_HTML, {
        url: "file://" + path.join(TU, "hlavne.html"),
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

    window.eval(HLAVNE_JS);
    return { window, document: window.document, volania };
}

async function pockajNaPrvePrekreslenie(window) {
    window.document.dispatchEvent(new window.Event("DOMContentLoaded", { bubbles: true }));
    await new Promise((hotovo) => setTimeout(hotovo, 50));
}

test("dashboard ukazuje presne sedem kariet s menami z TITracku", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        const popisy = [...document.querySelectorAll("#karty-statistik .karta-popis")].map((e) => e.textContent);
        assert.deepEqual(popisy, [
            "NET WORTH", "CUMULATIVE VALUE", "FE/HOUR", "FE/MAP", "RUNS", "AVG RUN TIME", "TOTAL TIME",
        ]);
    } finally {
        window.close();
    }
});

test("graf Cumulative value aj FE/hour sa nakreslia, keď majú behy začiatok", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        assert.ok(document.querySelector("#graf-kumulativ svg polyline"), "graf kumulatívnej hodnoty chýba");
        assert.ok(document.querySelector("#graf-fe-za-hodinu svg polyline"), "graf FE/hodinu chýba");
    } finally {
        window.close();
    }
});

test("Recent Runs: klik na riadok rozbalí zoznam dropov, druhý klik to schová", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        const riadok = document.querySelector('tr.riadok-behu[data-beh-id="1"]');
        assert.ok(riadok, "riadok behu #1 sa nevykreslil");
        const rozbalenie = document.querySelector('tr[data-beh-rozbalenie="1"]');
        assert.ok(rozbalenie.classList.contains("skryte"), "rozbalenie má byť na začiatku skryté");

        riadok.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        assert.ok(!rozbalenie.classList.contains("skryte"), "klik mal rozbalenie zobraziť");
        assert.match(rozbalenie.textContent, /Steel Forge Beacon/, "meno dropnutej položky chýba");

        riadok.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        assert.ok(rozbalenie.classList.contains("skryte"), "druhý klik mal rozbalenie znova schovať");
    } finally {
        window.close();
    }
});

test("Recent Runs: beh bez dropov ukáže vysvetľujúci text, nie prázdno", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        const rozbalenie = document.querySelector('tr[data-beh-rozbalenie="3"]');
        assert.match(rozbalenie.textContent, /nepadlo nič oceniteľné/);
    } finally {
        window.close();
    }
});

test("Report stiahne .txt s obsahom mapy, nie je to isté ako CSV export celej histórie", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        let stiahnuteMeno = null;
        const povodnyCreateElement = document.createElement.bind(document);
        document.createElement = (tag) => {
            const el = povodnyCreateElement(tag);
            if (tag === "a") {
                const povodnyClick = el.click.bind(el);
                el.click = () => {
                    stiahnuteMeno = el.download;
                    povodnyClick();
                };
            }
            return el;
        };
        window.URL.createObjectURL = () => "blob:stub";
        window.URL.revokeObjectURL = () => {};

        document.querySelector('[data-beh-report="2"]').dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        assert.equal(stiahnuteMeno, "tli-report-mapa-2.txt");
    } finally {
        window.close();
    }
});

test("hlavičkové tlačidlo Overlay prepne na záložku Solvery", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        document.getElementById("btn-nav-overlay").dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        assert.ok(!document.getElementById("panel-solvery").classList.contains("skryte"));
        assert.ok(document.querySelector('.zalozka[data-panel="panel-solvery"]').classList.contains("aktivna"));
    } finally {
        window.close();
    }
});

test("Instructions sa otvorí a zavrie, obsahuje Enable Log aj Sort", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        const modal = document.getElementById("modal-instructions");
        assert.ok(modal.classList.contains("skryte"));

        document.getElementById("btn-nav-instructions").dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        assert.ok(!modal.classList.contains("skryte"));
        assert.match(modal.textContent, /Enable Log/);
        assert.match(modal.textContent, /Sort/);

        document.getElementById("btn-instructions-zavri").dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        assert.ok(modal.classList.contains("skryte"));
    } finally {
        window.close();
    }
});

test("Reset Stats: bez potvrdenia nevolá nič, s potvrdením zavolá tracker_reset_zive_staty", async () => {
    const { window, document, volania } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);

        window.confirm = () => false;
        document.getElementById("btn-nav-resetstats").dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        await new Promise((hotovo) => setTimeout(hotovo, 10));
        assert.equal(volania.filter((v) => v.prikaz === "tracker_reset_zive_staty").length, 0);

        window.confirm = () => true;
        document.getElementById("btn-nav-resetstats").dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        await new Promise((hotovo) => setTimeout(hotovo, 10));
        assert.equal(volania.filter((v) => v.prikaz === "tracker_reset_zive_staty").length, 1);
    } finally {
        window.close();
    }
});

test("footer Collector ukazuje verziu a vie skontrolovať aktualizáciu", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        assert.match(document.getElementById("pita-verzia").textContent, /^verzia /);

        document.getElementById("btn-pita-aktualizacia").dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        await new Promise((hotovo) => setTimeout(hotovo, 10));
        assert.match(document.getElementById("pita-aktualizacia-stav").textContent, /najnovšiu verziu|novšia verzia/);
    } finally {
        window.close();
    }
});

// TLI 47 (`2026-10-19-0731-mcp.md`, bod A) — „Posledný upload" musí byť
// vidieť v pätičke bez otvárania záložky Nastavenia (ako TITrack
// „Log was last updated").
test("footer ukazuje posledný upload a reaguje na chybu aj vypnutie", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        const pitaUpload = document.getElementById("pita-upload");
        assert.equal(pitaUpload.textContent, "Posledný upload: pred 2 min");
        assert.equal(pitaUpload.classList.contains("hodnota-zaporna"), false);

        window.vykresliPatickaUploadu({ posledna_chyba_uploadu: "HTTP 500", posledna_kontrola_uploadu_pred: "pred 1 min" });
        assert.equal(pitaUpload.textContent, "Posledný upload: chyba (pred 1 min)");
        assert.ok(pitaUpload.classList.contains("hodnota-zaporna"));

        document.getElementById("chk-upload").checked = false;
        document.getElementById("chk-upload").dispatchEvent(new window.Event("change", { bubbles: true }));
        await new Promise((hotovo) => setTimeout(hotovo, 10));
        assert.equal(pitaUpload.textContent, "Posledný upload: vypnutý");
        assert.equal(pitaUpload.classList.contains("hodnota-zaporna"), false);
    } finally {
        window.close();
    }
});
