// TLI 55 (zadanie `2026-10-19-1715-mcp.md`) — regresné testy pre tri veci
// z Richardovej snímky 19.10.2026 počas bežiacej mapy:
//   1. nadpis ukazoval surové interné meno enginu („SD_DuiLongJuQiang211")
//      namiesto čitateľného mena mapy,
//   2. ČAS a FE/h ukazovali „—" (len ZISK bol vidieť správne),
//   3. po skončení mapy mali ostať posledné čísla, nie prázdne pomlčky.
// Plus nová session sekcia (bod 2 zadania). Backend (`trvanie_ms` naživo) má
// vlastné testy v `src-tauri/src/tracker.rs`; toto overuje len JS vrstvu.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { JSDOM } from "jsdom";

const TU = path.dirname(fileURLToPath(import.meta.url));

function postav(predPrikazy = {}) {
    const dom = new JSDOM(readFileSync(path.join(TU, "mini.html"), "utf8"), {
        url: "file://" + path.join(TU, "mini.html"),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    window.__TAURI__ = {
        core: {
            invoke: (meno) =>
                meno in predPrikazy
                    ? Promise.resolve(predPrikazy[meno])
                    : Promise.reject(new Error("neznámy príkaz " + meno)),
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
    // Nedispatchujeme DOMContentLoaded — tá si sama nastaví `setInterval`y,
    // ktoré by test proces nikdy neukončili. Testy nižšie volajú `obnov()`/
    // `tiknutie()` priamo (sú to top-level funkcie z klasického skriptu,
    // teda vlastnosti `window` po `eval`e).
    return { window, document: window.document };
}

test("nazovMapy preloží overený pinyin kód a inak aspoň rozdelí na slová, nikdy surovú cestu", (t) => {
    const { window } = postav();
    t.after(() => window.close());
    assert.equal(
        window.nazovMapy("/Game/Art/Maps/01SD/XZ_YuJinZhiXiBiNanSuo200/XZ_YuJinZhiXiBiNanSuo200"),
        "Ember's Rest"
    );
    // Neznámy kód: aspoň čitateľné slová, nie surová cesta so SD_/čínštinou.
    const citatelne = window.nazovMapy("/Game/Art/Maps/01SD/SD_DuiLongJuQiang211/SD_DuiLongJuQiang211");
    assert.ok(!citatelne.includes("/"), citatelne);
    assert.notEqual(citatelne, "SD_DuiLongJuQiang211", "nesmie zostať surové interné meno enginu");
    assert.equal(window.nazovMapy(""), "Neznáma mapa");
});

test("živá mapa: nadpis je preložený a ČAS/FE-h sa počítajú zo zivo.trvanie_ms, nie '—'", async (t) => {
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
        },
    });
    t.after(() => window.close());

    await window.obnov();

    assert.notEqual(document.getElementById("mini-mapa").textContent, "SD_DuiLongJuQiang211");
    assert.equal(document.getElementById("mini-zisk").textContent, "19.15 FE");
    assert.equal(document.getElementById("mini-cas").textContent, "1 m 30 s", "90000 ms živého behu sa už nesmie zobraziť ako '—'");
    // (19.15 * 3600000) / 90000 = 766 FE/h, nad 100 teda bez desatín.
    assert.equal(document.getElementById("mini-zah").textContent, "766");

    // Session (bod 2 zadania): celkový zisk/čas/FE-h z `prehlad.celkom`.
    assert.equal(document.getElementById("mini-session-zisk").textContent, "40.00 FE");
    assert.equal(document.getElementById("mini-session-cas").textContent, "5 m 00 s");
    assert.equal(document.getElementById("mini-session-zah").textContent, "480");
});

test("po skončení mapy ostanú posledné čísla namiesto '—' (bod 1 zadania)", async (t) => {
    const { window, document } = postav({
        tracker_prehlad: {
            zivo: null,
            posledna_mapa: {
                zona: "/Game/Art/Maps/01SD/SD_GeBuLinShanZhai/SD_GeBuLinShanZhai",
                trvanie_ms: 60_000,
                cisty_zisk: 12.5,
                podpis_vstupov: "400000x1",
                top_dropy: [],
            },
            prehlad: { celkom: { pocet_map: 1, cas_ms: 60_000, hodnota_dropov: 20, hodnota_vstupov: 7.5, cisty_zisk: 12.5 } },
        },
    });
    t.after(() => window.close());

    await window.obnov();

    assert.equal(document.getElementById("mini-mapa").textContent, "Žiadna mapa nebeží");
    assert.equal(document.getElementById("mini-zisk").textContent, "12.50 FE");
    assert.equal(document.getElementById("mini-cas").textContent, "1 m 00 s", "finálny čas poslednej mapy musí ostať, nie '—'");
    assert.equal(document.getElementById("mini-zah").textContent, "750");
});

test("bez akejkoľvek odohranej mapy session aj hlavné čísla ukazujú '—', nie chybu", async (t) => {
    const { window, document } = postav({ tracker_prehlad: null });
    t.after(() => window.close());

    await window.obnov();

    assert.equal(document.getElementById("mini-zisk").textContent, "—");
    assert.equal(document.getElementById("mini-cas").textContent, "—");
    assert.equal(document.getElementById("mini-zah").textContent, "—");
    assert.equal(document.getElementById("mini-session-zisk").textContent, "—");
    assert.equal(document.getElementById("mini-session-cas").textContent, "—");
    assert.equal(document.getElementById("mini-session-zah").textContent, "—");
});

test("tiknutie() dotiahne ČAS medzi dvoma pollami bez ďalšieho invoke", async (t) => {
    const { window, document } = postav({
        tracker_prehlad: {
            zivo: {
                zona: "/Game/Art/Maps/01SD/SD_DuiLongJuQiang211/SD_DuiLongJuQiang211",
                trvanie_ms: 10_000,
                cisty_zisk: 5,
                podpis_vstupov: "400000x1",
                top_dropy: [],
            },
            posledna_mapa: null,
            prehlad: { celkom: { pocet_map: 0, cas_ms: 0, hodnota_dropov: 0, hodnota_vstupov: 0, cisty_zisk: 0 } },
        },
    });
    t.after(() => window.close());

    await window.obnov();
    assert.equal(document.getElementById("mini-cas").textContent, "10 s");

    // Simuluje plynutie 3 sekúnd BEZ nového fetchu z backendu.
    const povodnyNow = Date.now;
    window.Date.now = () => povodnyNow() + 3000;
    window.tiknutie();
    window.Date.now = povodnyNow;

    assert.equal(document.getElementById("mini-cas").textContent, "13 s", "tiknutie malo pripočítať uplynulé sekundy lokálne");
});

test("tiknutie() po skončení mapy (zivaZakladna vynulovaná) nič neprepíše", async (t) => {
    const { window, document } = postav({
        tracker_prehlad: {
            zivo: null,
            posledna_mapa: {
                zona: "/Game/Art/Maps/01SD/SD_GeBuLinShanZhai/SD_GeBuLinShanZhai",
                trvanie_ms: 60_000,
                cisty_zisk: 12.5,
                podpis_vstupov: "400000x1",
                top_dropy: [],
            },
            prehlad: { celkom: { pocet_map: 1, cas_ms: 60_000, hodnota_dropov: 20, hodnota_vstupov: 7.5, cisty_zisk: 12.5 } },
        },
    });
    t.after(() => window.close());

    await window.obnov();
    window.tiknutie();
    assert.equal(document.getElementById("mini-cas").textContent, "1 m 00 s", "zmrazený čas poslednej mapy sa nesmie ďalej tikaním meniť");
});
