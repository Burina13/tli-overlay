// E2E test kliknutia na záložky + volania `uloz_prepinac` (TLI 16, 16.10.2026).
//
// Vznikol po hotfixe v0.13.1: `ui/hlavne.js` mal DVE top-level deklarácie
// `function prepni(...)` (prepínanie záložiek a ukladanie prepínačov) — keďže
// súbor je obyčajný klasický <script>, nie modul, tá druhá tú prvú v behu
// ticho prepísala. Klik na záložku Solvery/Analýza/Guide/Nastavenia tak
// skončil ako `invoke("uloz_prepinac", { meno: <DOM element>, zapnuty: undefined })`
// — Tauri to odmietol vetou „invalid type: map, expected a string" a žiadna
// záložka okrem Trackera nereagovala na klik.
//
// Existujúci CI náhľad (`ui/nahlad-generuj.py` + `?tab=`) tento bug chytiť
// NEVIE — aktívnu záložku prepína priamo v HTML pred vykreslením, JS klik sa
// tam vôbec nespúšťa. Tento test preto klik SIMULUJE nad reálnym
// `hlavne.html` + `hlavne.js` cez jsdom a nad reálnym `window.__TAURI__.core.invoke`
// (podsunutým cez `nahlad-stub.js`, tak ako to appka naozaj volá).
//
// Beží na Linuxe (`node --test ui/testy-zalozky.e2e.mjs`) — nepotrebuje Tauri
// ani Windows, testuje len `ui/*.js` a `ui/*.html`.

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

const ZALOZKY = [
    "panel-tracker",
    "panel-solvery",
    "panel-analyza",
    "panel-guide",
    "panel-nastavenia",
];

/// Postaví appku presne tak, ako ju vidí appka samotná: HTML appky + stub
/// namiesto Tauri (ten istý stub, aký používa CI náhľad) + ostrý `hlavne.js`.
/// `invoke` sa navyše obaluje, aby test videl, ČO presne appka appku poslala.
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

function klikni(window, element) {
    element.dispatchEvent(new window.MouseEvent("click", { bubbles: true }));
}

// `hlavne.js` si na `DOMContentLoaded` zavesí `setInterval(obnovTracker, 3000)`
// (živé obnovovanie trackera) — bez `window.close()` po teste by ten časovač
// držal Node event loop naživo navždy a `node --test` by nikdy neskončil.

test("klik na KAŽDÚ hlavnú záložku prepne aktívnu triedu aj viditeľný panel", () => {
    for (const cielovyPanel of ZALOZKY) {
        const { window, document } = postavApp();
        try {
            const tlacidlo = document.querySelector(`.zalozka[data-panel="${cielovyPanel}"]`);
            assert.ok(tlacidlo, `záložka ${cielovyPanel} v hlavne.html neexistuje`);

            klikni(window, tlacidlo);

            for (const panel of ZALOZKY) {
                const tlacidloPanelu = document.querySelector(`.zalozka[data-panel="${panel}"]`);
                const sekciaPanelu = document.getElementById(panel);
                if (panel === cielovyPanel) {
                    assert.ok(
                        tlacidloPanelu.classList.contains("aktivna"),
                        `tlačidlo ${panel} malo byť po kliku aktívne`,
                    );
                    assert.ok(
                        !sekciaPanelu.classList.contains("skryte"),
                        `panel ${panel} mal byť po kliku viditeľný`,
                    );
                } else {
                    assert.ok(
                        !tlacidloPanelu.classList.contains("aktivna"),
                        `tlačidlo ${panel} nemalo zostať aktívne`,
                    );
                    assert.ok(
                        sekciaPanelu.classList.contains("skryte"),
                        `panel ${panel} mal zostať skrytý`,
                    );
                }
            }
        } finally {
            window.close();
        }
    }
});

test("klik na záložku NEVOLÁ uloz_prepinac (regresia presne tohto hotfixu)", () => {
    const { window, document, volania } = postavApp();
    try {
        for (const panel of ZALOZKY) {
            klikni(window, document.querySelector(`.zalozka[data-panel="${panel}"]`));
        }
        const volaniaPrepinaca = volania.filter((v) => v.prikaz === "uloz_prepinac");
        assert.deepEqual(
            volaniaPrepinaca,
            [],
            "klik na záložku nesmie nikdy zavolať uloz_prepinac",
        );
    } finally {
        window.close();
    }
});

test("uloz_prepinac dostane STRING meno a BOOLEAN zapnuty (nie DOM element)", () => {
    const { window, document, volania } = postavApp();
    try {
        const chkMini = document.getElementById("chk-mini");
        chkMini.checked = true;
        chkMini.dispatchEvent(new window.Event("change", { bubbles: true }));

        const volaniaPrepinaca = volania.filter((v) => v.prikaz === "uloz_prepinac");
        assert.equal(volaniaPrepinaca.length, 1, "chkMini mal vyvolať presne jedno uloz_prepinac");
        const { meno, zapnuty } = volaniaPrepinaca[0].argy;
        assert.equal(typeof meno, "string", `meno malo byť string, bolo ${typeof meno}: ${JSON.stringify(meno)}`);
        assert.equal(meno, "mini_overlay");
        assert.equal(typeof zapnuty, "boolean", `zapnuty malo byť boolean, bolo ${typeof zapnuty}`);
        assert.equal(zapnuty, true);
    } finally {
        window.close();
    }
});

test("uloz_prepinac: aj chk-pripomienka posiela string meno, nie element zo záložky", () => {
    const { window, document, volania } = postavApp();
    try {
        const chk = document.getElementById("chk-pripomienka");
        chk.checked = true;
        chk.dispatchEvent(new window.Event("change", { bubbles: true }));

        const volaniaPrepinaca = volania.filter((v) => v.prikaz === "uloz_prepinac");
        assert.equal(volaniaPrepinaca.length, 1);
        // Pozor: `argy` vzniklo vo vnútri jsdom window (iný JS realm), takže
        // `assert.deepEqual`/`deepStrictEqual` naň padá na nezhode prototypu
        // Objectu aj pri rovnakom tvare — porovnáva sa preto pole po poli
        // (rovnako ako v predošlom teste pre chk-mini).
        const { meno, zapnuty } = volaniaPrepinaca[0].argy;
        assert.equal(meno, "pripominat_enable_log");
        assert.equal(zapnuty, true);
    } finally {
        window.close();
    }
});
