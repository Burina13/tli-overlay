// TLI 61 (zadanie `2026-10-19-2241-mcp.md`) — regresné testy pre:
//   1. FE/MIN vedľa FE/h (aktuálna mapa aj session),
//   2. „session od HH:MM" zo `session_od_unix`,
//   3. tlačidlo „⟲ Reset session" v pásiku úchytu — podržanie
//      `MINI_RESET_DRZAT_MS` zavolá `tracker_reset_zive_staty`, kratšie
//      podržanie alebo pustenie myši mimo tlačidla nespraví nič.
// Auto-reset na strane backendu (nečinnosť > 30 min) má vlastné testy
// v `src-tauri/src/tracker.rs` — toto overuje len JS vrstvu mini-panela.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));

// `const MINI_RESET_DRZAT_MS` v `mini.js` je top-level deklarácia v klasickom
// skripte — na rozdiel od `function obnov()`/`function tiknutie()` sa teda
// `window.eval()` nižšie NEPRIPOJÍ ako vlastnosť `window` (to platí len pre
// `var`/deklarácie funkcií). Test preto drží vlastnú kópiu rovnakej hodnoty.
const MINI_RESET_DRZAT_MS_OCAKAVANE = 1000;

function postav(predPrikazy = {}, { zaznamenajInvoke } = {}) {
    const dom = new JSDOM(readFileSync(path.join(TU, "mini.html"), "utf8"), {
        url: "file://" + path.join(TU, "mini.html"),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    window.__TAURI__ = {
        core: {
            invoke: (meno, ...args) => {
                if (zaznamenajInvoke) zaznamenajInvoke(meno, ...args);
                return meno in predPrikazy
                    ? Promise.resolve(predPrikazy[meno])
                    : Promise.reject(new Error("neznámy príkaz " + meno));
            },
        },
        event: { listen: () => Promise.resolve(() => {}) },
        window: {
            getCurrentWindow: () => ({
                startDragging: () => Promise.resolve(),
                startResizeDragging: () => Promise.resolve(),
            }),
        },
    };
    window.fetch = undefined;
    window.eval(readFileSync(path.join(TU, "mini.js"), "utf8"));
    return { window, document: window.document };
}

test("FE/MIN za aktuálnu mapu sa počíta vedľa FE/h", async (t) => {
    const { window, document } = postav({
        tracker_prehlad: {
            zivo: {
                zona: "/Game/Art/Maps/01SD/SD_DuiLongJuQiang211/SD_DuiLongJuQiang211",
                trvanie_ms: 90_000,
                cisty_zisk: 19.15,
                podpis_vstupov: "400000x1",
                top_dropy: [],
            },
            posledna_mapa: null,
            prehlad: { celkom: { pocet_map: 3, cas_ms: 300_000, hodnota_dropov: 50, hodnota_vstupov: 10, cisty_zisk: 40 } },
            session_od_unix: null,
        },
    });
    t.after(() => window.close());

    await window.obnov();

    // (19.15 * 60000) / 90000 = 12,77 FE/min.
    assert.equal(document.getElementById("mini-zamin").textContent, "12.77");
    // (40 * 60000) / 300000 = 8 FE/min za session.
    assert.equal(document.getElementById("mini-session-zamin").textContent, "8.00");
});

test("bez odohranej mapy FE/MIN ukazuje '—' rovnako ako ostatné hodnoty", async (t) => {
    const { window, document } = postav({ tracker_prehlad: null });
    t.after(() => window.close());

    await window.obnov();

    assert.equal(document.getElementById("mini-zamin").textContent, "—");
    assert.equal(document.getElementById("mini-session-zamin").textContent, "—");
});

test("'session od HH:MM' sa ukáže zo session_od_unix a zmizne, keď appka marker ešte nemá", async (t) => {
    const znacka = Date.UTC(2026, 9, 19, 21, 40) / 1000; // 19.10.2026 21:40 UTC
    const { window, document } = postav({
        tracker_prehlad: {
            zivo: null,
            posledna_mapa: null,
            prehlad: { celkom: { pocet_map: 0, cas_ms: 0, hodnota_dropov: 0, hodnota_vstupov: 0, cisty_zisk: 0 } },
            session_od_unix: znacka,
        },
    });
    t.after(() => window.close());

    await window.obnov();
    assert.match(document.getElementById("mini-session-od").textContent, /session od \d{2}:\d{2}/);

    // Appka ešte nikdy nič nereportovala — label má ostať prázdny, nie „session od Invalid Date".
    const { window: w2, document: d2 } = postav({
        tracker_prehlad: {
            zivo: null,
            posledna_mapa: null,
            prehlad: { celkom: { pocet_map: 0, cas_ms: 0, hodnota_dropov: 0, hodnota_vstupov: 0, cisty_zisk: 0 } },
            session_od_unix: null,
        },
    });
    t.after(() => w2.close());
    await w2.obnov();
    assert.equal(d2.getElementById("mini-session-od").textContent, "");
});

test("podržanie tlačidla Reset session MINI_RESET_DRZAT_MS zavolá tracker_reset_zive_staty", async (t) => {
    const volania = [];
    const { window, document } = postav(
        {
            tracker_prehlad: {
                zivo: null,
                posledna_mapa: null,
                prehlad: { celkom: { pocet_map: 0, cas_ms: 0, hodnota_dropov: 0, hodnota_vstupov: 0, cisty_zisk: 0 } },
                session_od_unix: null,
            },
            tracker_reset_zive_staty: null,
        },
        { zaznamenajInvoke: (meno) => volania.push(meno) }
    );
    t.after(() => window.close());

    window.nastavResetSession();
    await window.obnov();
    volania.length = 0; // zahoď invoke z DOMContentLoaded/obnov vyššie

    const btn = document.getElementById("mini-reset-session");
    assert.ok(btn, "tlačidlo reset session musí existovať v pásiku úchytu");

    btn.dispatchEvent(new window.MouseEvent("mousedown", { bubbles: true }));
    assert.ok(btn.classList.contains("drzi"), "počas podržania má byť vidieť vizuálnu spätnú väzbu");

    await new Promise((r) => setTimeout(r, MINI_RESET_DRZAT_MS_OCAKAVANE + 50));

    assert.ok(volania.includes("tracker_reset_zive_staty"), "po " + MINI_RESET_DRZAT_MS_OCAKAVANE + " ms podržania sa mal zavolať reset");
});

test("pustenie tlačidla PRED uplynutím podržania nič nespraví", async (t) => {
    const volania = [];
    const { window, document } = postav(
        {
            tracker_prehlad: {
                zivo: null,
                posledna_mapa: null,
                prehlad: { celkom: { pocet_map: 0, cas_ms: 0, hodnota_dropov: 0, hodnota_vstupov: 0, cisty_zisk: 0 } },
                session_od_unix: null,
            },
        },
        { zaznamenajInvoke: (meno) => volania.push(meno) }
    );
    t.after(() => window.close());

    window.nastavResetSession();
    await window.obnov();
    volania.length = 0;

    const btn = document.getElementById("mini-reset-session");
    btn.dispatchEvent(new window.MouseEvent("mousedown", { bubbles: true }));
    btn.dispatchEvent(new window.MouseEvent("mouseup", { bubbles: true }));
    assert.ok(!btn.classList.contains("drzi"), "pustenie pred limitom má zrušiť vizuálny stav držania");

    await new Promise((r) => setTimeout(r, MINI_RESET_DRZAT_MS_OCAKAVANE + 50));

    assert.ok(!volania.includes("tracker_reset_zive_staty"), "krátke podržanie nesmie vynulovať session (poistka proti náhodnému kliku)");
});

test("mousedown na tlačidle Reset session nespustí presun celého okna (stopPropagation)", async (t) => {
    const { window, document } = postav({
        tracker_prehlad: {
            zivo: null,
            posledna_mapa: null,
            prehlad: { celkom: { pocet_map: 0, cas_ms: 0, hodnota_dropov: 0, hodnota_vstupov: 0, cisty_zisk: 0 } },
            session_od_unix: null,
        },
    });
    t.after(() => window.close());

    let presunSpusteny = false;
    document.getElementById("mini-uchyt-presun").addEventListener("mousedown", () => {
        presunSpusteny = true;
    });

    window.nastavResetSession();
    await window.obnov();

    document.getElementById("mini-reset-session").dispatchEvent(new window.MouseEvent("mousedown", { bubbles: true }));

    assert.equal(presunSpusteny, false, "klik na tlačidlo reset nesmie zároveň potiahnuť okno mini-panela");
});
