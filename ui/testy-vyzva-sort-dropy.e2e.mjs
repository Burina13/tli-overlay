// E2E test výzvy „Sortni inventár pre presné dropy" (TLI 68,
// `2026-10-20-1213-mcp.md`).
//
// Richard 20.10.2026 nahlásil mapu s dropom „Voidlands Beacon ×49", hoci
// padol jeden kus a 49 mal v batohu. Príčina: appka sa o 11:54 reštartovala
// uprostred hrania, nepoznala stav batohu a prvú zmenu každého slotu
// vyrátala ako `Num − 0`, teda celý stack. Oprava takú zmenu zámerne
// NEzapočíta — a presne o tom musí hráča spraviť hlavné okno, inak by len
// videl, že sa drop „nezobrazil".
//
// Beží na Linuxe cez jsdom, rovnaký vzor ako `testy-dashboard-titrack.e2e.mjs`.

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

function postavApp(upravPrehlad) {
    const dom = new JSDOM(HLAVNE_HTML, {
        url: "file://" + path.join(TU, "hlavne.html"),
        runScripts: "outside-only",
        pretendToBeVisual: true,
    });
    const { window } = dom;
    window.eval(NAHLAD_STUB_JS);

    const povodnyInvoke = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = async (prikaz, argy) => {
        const odpoved = await povodnyInvoke(prikaz, argy);
        if (prikaz === "tracker_prehlad" && upravPrehlad) {
            return upravPrehlad({ ...odpoved });
        }
        return odpoved;
    };

    window.eval(HLAVNE_JS);
    return { window, document: window.document };
}

async function pockajNaPrvePrekreslenie(window) {
    window.document.dispatchEvent(new window.Event("DOMContentLoaded", { bubbles: true }));
    await new Promise((hotovo) => setTimeout(hotovo, 50));
}

test("bez potvrdeného základu dropov sa ukáže výzva na Sort aj s počtom vynechaných zmien", async () => {
    const { window, document } = postavApp((p) => ({
        ...p,
        zaklad_dropov_uplny: false,
        dropov_bez_zakladu: 4,
    }));
    try {
        await pockajNaPrvePrekreslenie(window);
        const text = document.getElementById("pita-sort-vyzva").textContent;
        assert.match(text, /Sortni inventár pre presné dropy/);
        assert.match(text, /4/, "počet nezapočítaných zmien má byť vidieť");
    } finally {
        window.close();
    }
});

test("výzva o dropoch má prednosť pred staršou výzvou o materiáloch", async () => {
    const { window, document } = postavApp((p) => ({
        ...p,
        zaklad_dropov_uplny: false,
        dropov_bez_zakladu: 0,
        materialy_sortovane: false,
    }));
    try {
        await pockajNaPrvePrekreslenie(window);
        const text = document.getElementById("pita-sort-vyzva").textContent;
        assert.match(text, /presné dropy/, "nesprávne čísla sú horšie než neúplný Net Worth");
        assert.doesNotMatch(text, /Net Worth/);
    } finally {
        window.close();
    }
});

test("so základom zo session je riadok výzvy prázdny", async () => {
    const { window, document } = postavApp((p) => ({ ...p, zaklad_dropov_uplny: true }));
    try {
        await pockajNaPrvePrekreslenie(window);
        assert.equal(document.getElementById("pita-sort-vyzva").textContent, "");
    } finally {
        window.close();
    }
});
