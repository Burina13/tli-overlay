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
        assert.equal(pitaUpload.textContent, "⚠ Log sa neposiela na server — chyba (pred 1 min)");
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

test("footer varuje, keď upload mlčí > 15 min pri bežiacej hre, ale nie mimo hry", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        const pitaUpload = document.getElementById("pita-upload");

        // Hra beží (zivo != null) a posledný úspešný upload je dávno — varovanie.
        window.vykresliPatickaUploadu({ zivo: { zona: "x" }, posledna_chyba_uploadu: null, posledny_upload_pred_s: 20 * 60, posledna_kontrola_uploadu_pred: "pred 20 min" });
        assert.match(pitaUpload.textContent, /^⚠ Log sa neposiela na server/);
        assert.ok(pitaUpload.classList.contains("hodnota-zaporna"));

        // To isté ticho, ale hra NEBEŽÍ (zivo je null) — nie je dôvod strašiť.
        window.vykresliPatickaUploadu({ zivo: null, posledna_chyba_uploadu: null, posledny_upload_pred_s: 20 * 60, posledna_kontrola_uploadu_pred: "pred 20 min" });
        assert.equal(pitaUpload.textContent, "Posledný upload: pred 20 min");
        assert.equal(pitaUpload.classList.contains("hodnota-zaporna"), false);
    } finally {
        window.close();
    }
});

// TLI 47 (`2026-10-19-0744-mcp.md` bod 1c) — appka vidí CURRENT INVENTORY
// len z InitBagData (Sort v hre); kým sa to nestane, treba o tom vedieť.
test("footer vyzve na Sort, keď appka hrá, ale batoh je ešte prázdny", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        const vyzva = document.getElementById("pita-sort-vyzva");
        // Náhľadové dáta už majú neprázdny batoh — žiadna výzva.
        assert.equal(vyzva.textContent, "");

        window.vykresliVyzvuNaSort({ zivo: { zona: "x" }, batoh: [] });
        assert.match(vyzva.textContent, /Sortni inventár/);

        window.vykresliVyzvuNaSort({ zivo: { zona: "x" }, batoh: [{ config_base_id: 1, mnozstvo: 1 }] });
        assert.equal(vyzva.textContent, "");

        window.vykresliVyzvuNaSort({ zivo: null, behy: [], batoh: [] });
        assert.equal(vyzva.textContent, "");
    } finally {
        window.close();
    }
});

// TLI 47 (`2026-10-19-0744-mcp.md` bod 2) — CURRENT INVENTORY: zoznam,
// triedenie, filter, Hide/Unhide a NET WORTH zo skutočného batohu.
test("CURRENT INVENTORY: zobrazí batoh, 'bez ceny' pre neznámu položku, a NET WORTH použije net_worth_fe", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        const riadky = [...document.querySelectorAll("#tabulka-inventar tbody tr")];
        assert.equal(riadky.length, 3); // presne toľko, koľko má nahlad-stub.js v `batoh`

        const bezCenyRiadok = riadky.find((r) => r.textContent.includes("neznáma položka #999999"));
        assert.ok(bezCenyRiadok, "položka bez ceny musí byť v tabuľke, nie vynechaná");
        assert.match(bezCenyRiadok.textContent, /bez ceny/);

        const netWorth = [...document.querySelectorAll("#karty-statistik .karta-popis")]
            .find((e) => e.textContent === "NET WORTH").closest(".karta-statistika");
        assert.match(netWorth.querySelector(".karta-hodnota").textContent, /972/);
    } finally {
        window.close();
    }
});

test("CURRENT INVENTORY: prázdny batoh ukáže vysvetlivku a NET WORTH odznak namiesto predstierania hodnoty", async () => {
    const { window, document } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);
        window.vykresliKarty({ prehlad: { celkom: { cisty_zisk: 42, cas_ms: 0, pocet_map: 0 } }, batoh: [] });
        window.vykresliInventar({ batoh: [] });

        assert.equal(document.getElementById("prazdne-inventar").classList.contains("skryte"), false);
        assert.equal(document.getElementById("tabulka-inventar").classList.contains("skryte"), true);
        const netWorth = [...document.querySelectorAll("#karty-statistik .karta-popis")]
            .find((e) => e.textContent === "NET WORTH").closest(".karta-statistika");
        assert.match(netWorth.querySelector(".karta-detail").textContent, /sortni inventár/);
    } finally {
        window.close();
    }
});

test("CURRENT INVENTORY: filter, triedenie podľa počtu a Hide/Unhide", async () => {
    const { window, document, volania } = postavApp();
    try {
        await pockajNaPrvePrekreslenie(window);

        // Filter podľa mena (zo stubu MENA_POLOZIEK, 400021 má meno, 500101 nie).
        document.getElementById("inventar-filter").value = "Steel Forge";
        document.getElementById("inventar-filter").dispatchEvent(new window.Event("input", { bubbles: true }));
        let riadky = [...document.querySelectorAll("#tabulka-inventar tbody tr")];
        assert.equal(riadky.length, 1);
        assert.match(riadky[0].textContent, /Steel Forge/);

        // Vyprázdni filter, zoraď podľa počtu zostupne — 500101 (24 ks) musí byť prvý.
        document.getElementById("inventar-filter").value = "";
        document.getElementById("inventar-filter").dispatchEvent(new window.Event("input", { bubbles: true }));
        document.getElementById("inventar-triedenie").value = "mnozstvo";
        document.getElementById("inventar-triedenie").dispatchEvent(new window.Event("change", { bubbles: true }));
        riadky = [...document.querySelectorAll("#tabulka-inventar tbody tr")];
        assert.match(riadky[0].textContent, /24/);

        // Hide zavolá tracker_prepni_skrytu_polozku so správnymi argumentmi.
        const btnHide = riadky[0].querySelector(".btn-skryt-polozku");
        btnHide.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
        await new Promise((hotovo) => setTimeout(hotovo, 10));
        const volanie = volania.find((v) => v.prikaz === "tracker_prepni_skrytu_polozku");
        assert.ok(volanie);
        assert.equal(volanie.argy.configBaseId, 500101);
        assert.equal(volanie.argy.skryta, true);
    } finally {
        window.close();
    }
});
