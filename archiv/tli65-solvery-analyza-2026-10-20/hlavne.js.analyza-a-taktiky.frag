// ─── Analýza (TLI 13) ───────────────────────────────────────────────────────

function analyzaFilter() {
    const mapa = document.getElementById("analyza-f-mapa").value.trim();
    const minZiskTxt = document.getElementById("analyza-f-min-zisk").value.trim();
    const filter = { limit: 500 };
    // Appka hľadá PODREŤAZEC cesty mapy (napr. "XueYuRongLu" nájde všetky mapy
    // Frozen Canvas bez toho, aby hráč poznal presnú internú cestu), kým
    // `db::vyhladaj_behy` porovnáva `zona` na presnú zhodu — filtrovanie podľa
    // podreťazca sa preto robí tu v JS nad výsledkom, nie na serveri.
    if (minZiskTxt !== "") {
        const cislo = Number(minZiskTxt);
        if (!Number.isNaN(cislo)) {
            filter.min_zisk_fe = cislo;
        }
    }
    return { filter, mapaPodretazec: mapa.toLowerCase() };
}

function vykresliAnalyzu(riadky) {
    const telo = document.querySelector("#tabulka-analyza tbody");
    document.getElementById("analyza-prazdne").classList.toggle("skryte", riadky.length > 0);
    document.getElementById("tabulka-analyza").classList.toggle("skryte", riadky.length === 0);

    telo.innerHTML = riadky
        .map(
            (r) => `<tr data-beh-id="${r.id}" data-zona="${nazovMapy(r.zona)}" class="riadok-klikatelny">
                <td>${nazovMapy(r.zona)}</td>
                <td>${odznakyVstupov(r.podpis_vstupov)}</td>
                <td>${r.zaciatok_cas || "—"}</td>
                <td class="cislo">${cas(r.trvanie_ms)}</td>
                <td class="cislo ${triedaZnamienka(r.cisty_zisk_fe || 0)}">${fe(r.cisty_zisk_fe)}</td>
                <td class="cislo">${fe(r.fe_za_hodinu)}</td>
                <td class="cislo">${r.pocet_monster}</td>
                <td class="cislo">${r.pocet_rares_a_bossov}</td>
            </tr>`
        )
        .join("");

    telo.querySelectorAll("tr[data-beh-id]").forEach((riadok) => {
        riadok.addEventListener("click", () => zobrazDetailBehu(Number(riadok.dataset.behId), riadok.dataset.zona));
    });
}

async function zobrazDetailBehu(behId, nazovZony) {
    try {
        const podlaRarity = await invoke("analyza_monstra_podla_rarity", { behId });
        const detail = document.getElementById("analyza-detail");
        document.getElementById("analyza-detail-mapa").textContent = nazovZony;
        const telo = document.querySelector("#tabulka-analyza-detail tbody");
        const MENA_RARIT = { 1: "White", 2: "Blue", 3: "Purple", 4: "Orange", 5: "Chromatic" };
        telo.innerHTML = (podlaRarity || [])
            .map(([rarita, pocet]) => `<tr><td>${MENA_RARIT[rarita] || rarita}</td><td class="cislo">${pocet}</td></tr>`)
            .join("");
        detail.classList.remove("skryte");
    } catch (e) {
        console.error(e);
    }
}

async function analyzaHladaj() {
    try {
        const { filter, mapaPodretazec } = analyzaFilter();
        let riadky = await invoke("analyza_vyhladaj", { filter });
        if (mapaPodretazec) {
            riadky = riadky.filter((r) => r.zona.toLowerCase().includes(mapaPodretazec));
        }
        vykresliAnalyzu(riadky);
    } catch (e) {
        console.error(e);
        document.getElementById("analyza-import-stav").textContent = `Vyhľadávanie zlyhalo: ${e}`;
    }
}

function stiahni(obsah, typ, nazovSuboru) {
    const odkaz = document.createElement("a");
    odkaz.href = URL.createObjectURL(new Blob([obsah], { type: typ }));
    odkaz.download = nazovSuboru;
    odkaz.click();
    URL.revokeObjectURL(odkaz.href);
}

document.getElementById("btn-analyza-hladaj").addEventListener("click", analyzaHladaj);

document.getElementById("btn-analyza-csv").addEventListener("click", async () => {
    try {
        const { filter } = analyzaFilter();
        const csv = await invoke("analyza_export_csv", { filter });
        stiahni(csv, "text/csv;charset=utf-8", "tli-analyza.csv");
    } catch (e) {
        console.error(e);
    }
});

document.getElementById("btn-analyza-json").addEventListener("click", async () => {
    try {
        const { filter } = analyzaFilter();
        const json = await invoke("analyza_export_json", { filter });
        stiahni(json, "application/json;charset=utf-8", "tli-analyza.json");
    } catch (e) {
        console.error(e);
    }
});

document.getElementById("btn-analyza-stare-logy").addEventListener("click", async () => {
    const stavEl = document.getElementById("analyza-import-stav");
    stavEl.textContent = "Načítavam staré logy… (môže to chvíľu trvať pri väčších súboroch)";
    try {
        const r = await invoke("tracker_nacitaj_stare_logy");
        stavEl.textContent =
            `Hotovo: ${r.pocet_novych_behov} nových behov, ${r.pocet_preskocenych_behov} už bolo uložených, ` +
            `${r.pocet_monster} monštier, ${r.pocet_cien} cien, ${r.pocet_zostatkov} zostatkov ` +
            `(${r.pocet_riadkov} riadkov spolu).`;
        await analyzaHladaj();
        await taktikyNacitaj();
    } catch (e) {
        stavEl.textContent = `Import zlyhal: ${e}`;
        console.error(e);
    }
});

// ─── Taktiky (TLI 13b) ──────────────────────────────────────────────────────
// Taktika = podpis setupu behu (chart+kompasy+mapa+mód+modifikátory+build),
// appka ich zoskupí a spočíta sama (core::tracker::taktiky, čisto a otestované
// bez DB) — tu sa to len vykreslí a prepojí na pomenovanie/zlúčenie a export.

let posledneTaktiky = [];

function fmtCislo(hodnota, desatiny) {
    if (hodnota === null || hodnota === undefined || !isFinite(hodnota)) {
        return "—";
    }
    return hodnota.toFixed(desatiny === undefined ? 1 : desatiny);
}

// Nepomenovaná taktika má za `nazov` dlhý surový podpis (presný, ale
// nečitateľný) — ukážeme namiesto neho kombináciu vstupov a mapy JEDNÉHO
// zástupcu skupiny, rovnakým vzorom ako tabuľka „Výsledky" vyššie.
function nazovTaktiky(t) {
    if (t.kluc.startsWith("t")) {
        return t.nazov;
    }
    return `${odznakyVstupov(t.ukazka_podpis_vstupov)} @ ${nazovMapy(t.ukazka_zona)}`;
}

function vykresliTaktiky(taktiky) {
    posledneTaktiky = taktiky;
    const telo = document.querySelector("#tabulka-taktiky tbody");
    document.getElementById("taktiky-prazdne").classList.toggle("skryte", taktiky.length > 0);
    document.getElementById("tabulka-taktiky").classList.toggle("skryte", taktiky.length === 0);

    telo.innerHTML = taktiky
        .map((t, i) => {
            const st = t.statistika;
            const malo = st.malo_dat ? ' <span class="odznak">málo dát</span>' : "";
            const roi = st.roi === null || st.roi === undefined ? "—" : `${fmtCislo(st.roi, 2)}×`;
            const pomer = st.pomer_zisk_riziko === null || st.pomer_zisk_riziko === undefined ? "—" : fmtCislo(st.pomer_zisk_riziko, 2);
            return `<tr>
                <td><input type="checkbox" class="taktika-checkbox" data-index="${i}"></td>
                <td>${nazovTaktiky(t)}${malo}</td>
                <td class="cislo">${st.pocet_behov}</td>
                <td class="cislo">${st.pocet_ocenenych}</td>
                <td class="cislo ${triedaZnamienka(st.priemerny_zisk || 0)}">${fe(st.priemerny_zisk)}</td>
                <td class="cislo">${fe(st.median_zisk)}</td>
                <td class="cislo">${fe(st.min_zisk)} / ${fe(st.max_zisk)}</td>
                <td class="cislo">${fe(st.p10_zisk)} / ${fe(st.p90_zisk)}</td>
                <td class="cislo">${fe(st.smerodajna_odchylka)}</td>
                <td class="cislo">${fe(st.priemerne_fe_za_hodinu)}</td>
                <td class="cislo">${fe(st.priemerny_naklad)}</td>
                <td class="cislo">${roi}</td>
                <td class="cislo">${pomer}</td>
            </tr>`;
        })
        .join("");
}

function vykresliOdporucanieANavrhy(analyza) {
    document.getElementById("taktiky-odporucanie").textContent = `Odporúčanie: ${analyza.odporucanie.text}`;
    const navrhyEl = document.getElementById("taktiky-navrhy");
    if (!analyza.navrhy_na_otestovanie.length) {
        navrhyEl.innerHTML = "";
        return;
    }
    navrhyEl.innerHTML =
        '<p class="vysvetlivka">Čo vyskúšať viac:</p><ul>' +
        analyza.navrhy_na_otestovanie.map((n) => `<li>${n}</li>`).join("") +
        "</ul>";
}

function taktikyFilter() {
    // Zatiaľ bez vlastných polí vo filtri — appka zoskupí VŠETKY behy v DB.
    // Filtrovanie podľa obdobia/hrdinu/ligy je pripravené na serveri
    // (`db::FilterTaktik`), UI preň pribudne samostatne (BACKLOG.md „TLI 13b").
    return {};
}

async function taktikyNacitaj() {
    try {
        const filter = taktikyFilter();
        const analyza = await invoke("analyza_taktiky", { filter });
        vykresliTaktiky(analyza.taktiky);
        vykresliOdporucanieANavrhy(analyza);
    } catch (e) {
        console.error(e);
        document.getElementById("taktiky-odporucanie").textContent = `Taktiky sa nepodarilo načítať: ${e}`;
    }
}

document.getElementById("btn-taktiky-obnov").addEventListener("click", taktikyNacitaj);

document.getElementById("btn-taktiky-csv").addEventListener("click", async () => {
    try {
        const csv = await invoke("analyza_taktiky_export_csv", { filter: taktikyFilter() });
        stiahni(csv, "text/csv;charset=utf-8", "tli-taktiky.csv");
    } catch (e) {
        console.error(e);
    }
});

document.getElementById("btn-taktiky-json").addEventListener("click", async () => {
    try {
        const json = await invoke("analyza_taktiky_export_json", { filter: taktikyFilter() });
        stiahni(json, "application/json;charset=utf-8", "tli-taktiky.json");
    } catch (e) {
        console.error(e);
    }
});

document.getElementById("btn-taktiky-pomenuj").addEventListener("click", async () => {
    const stavEl = document.getElementById("taktiky-pomenovanie-stav");
    const nazov = document.getElementById("taktiky-f-nazov").value.trim();
    const vybrane = Array.from(document.querySelectorAll(".taktika-checkbox:checked")).map((c) => Number(c.dataset.index));
    if (!nazov) {
        stavEl.textContent = "Zadaj názov.";
        return;
    }
    if (vybrane.length === 0) {
        stavEl.textContent = "Vyber aspoň jednu taktiku (zaškrtni riadok vľavo).";
        return;
    }
    const podpisy = vybrane.flatMap((i) => posledneTaktiky[i].podpisy);
    try {
        await invoke("taktika_pomenuj_alebo_zluc", { podpisy, nazov });
        stavEl.textContent = `Hotovo — „${nazov}“ uložené.`;
        document.getElementById("taktiky-f-nazov").value = "";
        } catch (e) {
        stavEl.textContent = `Zlyhalo: ${e}`;
        console.error(e);
    }

document.getElementById("btn-taktiky-pomenuj").addEventListener("click", async () => {
    const stavEl = document.getElementById("taktiky-pomenovanie-stav");
    const nazov = document.getElementById("taktiky-f-nazov").value.trim();
    const vybrane = Array.from(document.querySelectorAll(".taktika-checkbox:checked")).map((c) => Number(c.dataset.index));
    if (!nazov) {
        stavEl.textContent = "Zadaj názov.";
        return;
    }
    if (vybrane.length === 0) {
        stavEl.textContent = "Vyber aspoň jednu taktiku (zaškrtni riadok vľavo).";
        return;
    }
    const podpisy = vybrane.flatMap((i) => posledneTaktiky[i].podpisy);
    try {
        await invoke("taktika_pomenuj_alebo_zluc", { podpisy, nazov });
        stavEl.textContent = `Hotovo — „${nazov}“ uložené.`;
        document.getElementById("taktiky-f-nazov").value = "";
        } catch (e) {
        stavEl.textContent = `Zlyhalo: ${e}`;
        console.error(e);
    }
});
