const { invoke } = window.__TAURI__.core;

// ─── Záložky ────────────────────────────────────────────────────────────────
const zalozky = document.querySelectorAll(".zalozka");
const panely = document.querySelectorAll(".panel");

// Pozor: meno `prepniZalozku` je zámerne ODLIŠNÉ od `prepni(meno, zapnuty)`
// nižšie (ukladanie prepínačov). Obe boli donedávna `function prepni`, a keďže
// tento súbor je obyčajný klasický <script> (nie modul), druhá deklarácia tú
// prvú v behu ticho prepísala — klik na záložku tak skončil ako
// `prepni(tlacidloZaložky)`, teda volanie `uloz_prepinac` s DOM elementom
// namiesto `meno`/`zapnuty` („invalid type: map, expected a string"). Hotfix
// v0.13.1, 16.10.2026.
function prepniZalozku(tlacidlo) {
    zalozky.forEach((t) => t.classList.remove("aktivna"));
    panely.forEach((p) => p.classList.add("skryte"));
    tlacidlo.classList.add("aktivna");
    document.getElementById(tlacidlo.dataset.panel).classList.remove("skryte");
}

zalozky.forEach((tlacidlo) => {
    tlacidlo.addEventListener("click", () => prepniZalozku(tlacidlo));
});

// `?tab=panel-solvery` otvorí rovno danú záložku — slúži len CI náhľadu
// (screenshot každej záložky zvlášť), appka sama o sebe URL parametre nepoužíva.
const parametre = new URLSearchParams(window.location.search);
const ziadanaZalozka = parametre.get("tab");
if (ziadanaZalozka) {
    const tlacidlo = document.querySelector(`.zalozka[data-panel="${ziadanaZalozka}"]`);
    if (tlacidlo) {
        prepniZalozku(tlacidlo);
    }
}

// ─── Svetlá a tmavá téma ────────────────────────────────────────────────────
// Predvolená je SVETLÁ (Richard 15.10.2026: „nemá to byť celé tmavé"). Hodnota
// sa ukladá do nastavení, nie do localStorage — appka má jedno miesto pravdy a
// po preinštalovaní to má ostať.
const prepinacTemy = document.getElementById("prepinac-temy");

// Ikony sú vložené SVG, nie emoji: emoji font nie je na každom Windowse ani
// na CI runneri a prepínač by sa zobrazil ako prázdny štvorček.
const IKONA_MESIAC = '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path fill="currentColor" d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 1 0 9.8 9.8z"/></svg>';
const IKONA_SLNKO = '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><circle cx="12" cy="12" r="4.2" fill="currentColor"/><g stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5.2 5.2l1.6 1.6M17.2 17.2l1.6 1.6M18.8 5.2l-1.6 1.6M6.8 17.2l-1.6 1.6"/></g></svg>';

function nastavTemu(tema) {
    const t = tema === "tmava" ? "tmava" : "svetla";
    document.documentElement.dataset.tema = t;
    prepinacTemy.innerHTML = t === "tmava" ? IKONA_SLNKO : IKONA_MESIAC;
    return t;
}

prepinacTemy.addEventListener("click", async () => {
    const nova = document.documentElement.dataset.tema === "tmava" ? "svetla" : "tmava";
    nastavTemu(nova);
    try {
        await invoke("uloz_temu", { tema: nova });
    } catch (e) {
        console.error(e);
    }
});

// `?tema=tmava` je opäť len pre CI náhľad, nech sa dajú odfotiť obe témy.
if (parametre.get("tema")) {
    nastavTemu(parametre.get("tema"));
}

// ─── Tracker ────────────────────────────────────────────────────────────────
const NEZNAMA_MAPA = "Neznáma mapa";

// Cesta zóny je interný čínsky (pinyin) názov z enginu hry. Pre overené zóny
// má vlastnú tabuľku (zdroj: docs/fakty/hry/torchlight-infinite/log-format.md
// §4, TLI 11b) — pre ostatné sa kód aspoň rozbije na čitateľné slová namiesto
// zlepenej skratky, nikdy sa nezobrazuje surová cesta (TLI 17, 16.10.2026).
const PREKLAD_MAPY = {
    yujinzhixibinansuo: "Ember's Rest",
};

// Mená položiek (TLI 19 bod 3, TLI 20 bod 7). Balík `polozky-seed-en.json`
// pochádza z TITracku (MIT, podrobne v NOTICE.md) a pokrýva aj tohtosezónne
// beacony 400000/400001, ktoré staršia tabuľka `en_id_table.json` nemala.
// Načítava sa raz a asynchrónne: keď sa nenačíta, zobrazí sa `#400021` ako
// doteraz — appka na mene položky nesmie stáť.
// `window.__MENA_POLOZIEK__` podsúva náhľad pre CI (`nahlad-stub.js`) —
// ten beží z `file://`, kde `fetch` neprejde, a screenshot by inak ukazoval
// holé `#400021` namiesto mien.
let MENA_POLOZIEK = window.__MENA_POLOZIEK__ || {};

// Odkiaľ brať ikony položiek (TLI 48 bod 3, rovnaký zdroj ako mini-panel,
// `ui/mini.js`) — z NÁŠHO servera (`/ikony/polozka/<id>`), nie z cudzieho CDN.
// `null`, kým sa nenačítajú Nastavenia — dovtedy appka Economy ukáže bez ikon.
let ZAKLAD_IKON = null;

// `typeof fetch` sa kontroluje zámerne: v jsdom teste (`ui/testy-zalozky.e2e.mjs`)
// `fetch` neexistuje a holé volanie by zhodilo CELÝ skript pri načítaní —
// teda aj všetko ostatné v okne.
if (typeof fetch === "function") {
    fetch("polozky-seed-en.json")
        .then((o) => (o.ok ? o.json() : null))
        .then((d) => {
            if (d && d.polozky) {
                MENA_POLOZIEK = d.polozky;
                // Tabuľky už mohli byť vykreslené s holými číslami — prekresli ich.
                obnovTracker().catch(() => {});
            }
        })
        .catch(() => {});
}

/// `400021` → `Steel Forge Beacon (Timemark 7)`, alebo `neznáma položka
/// #400021`, keď meno nepoznáme (TLI 37 — holé `#115` hráčovi nič nepovie
/// a vyzerá ako chyba appky; v skutočnosti to id v balíku mien nie je).
/// Nikdy nevracia prázdny reťazec.
function nazovPolozky(id) {
    const meno = MENA_POLOZIEK[String(id)];
    return meno || `neznáma položka #${id}`;
}

/// Posledný kus cesty zóny bez prípony, napr. `XZ_ShouGuSiDi000`.
function kodMapy(cesta) {
    const kus = cesta.split("/").pop() || cesta;
    return kus.split(".").pop() || kus;
}

/// `XZ_ShouGuSiDi000` → `Shou Gu Si Di`: odstráni 1–3-písmenový oblastný
/// prefix a koncové číslo úrovne/inštancie, zvyšok rozdelí na slová podľa
/// veľkých písmen. Keď je jadro v `PREKLAD_MAPY`, použije sa overené meno
/// namiesto tohto odhadu.
function citatelnyKodMapy(kod) {
    const jadro = kod.replace(/^[A-Za-z]{1,3}_/, "").replace(/(_?\d+)+$/, "") || kod;
    const preklad = PREKLAD_MAPY[jadro.toLowerCase()];
    if (preklad) {
        return preklad;
    }
    return jadro.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/_/g, " ").trim() || kod;
}

/// Interná cesta zóny je dlhá a čínska — do tabuľky ide čitateľné meno mapy,
/// nie surový engine kód.
function nazovMapy(cesta) {
    if (!cesta) {
        return NEZNAMA_MAPA;
    }
    return citatelnyKodMapy(kodMapy(cesta)) || NEZNAMA_MAPA;
}

function cas(ms) {
    if (!ms || ms <= 0) {
        return "—";
    }
    const s = Math.round(ms / 1000);
    const m = Math.floor(s / 60);
    return m > 0 ? `${m} m ${String(s % 60).padStart(2, "0")} s` : `${s} s`;
}

function fe(hodnota) {
    if (hodnota === null || hodnota === undefined || !isFinite(hodnota)) {
        return "—";
    }
    const zaokruhlene = Math.abs(hodnota) >= 100 ? hodnota.toFixed(0) : hodnota.toFixed(2);
    return `${zaokruhlene} FE`;
}

function triedaZnamienka(hodnota) {
    if (hodnota > 0) return "hodnota-kladna";
    if (hodnota < 0) return "hodnota-zaporna";
    return "";
}

/// Podpis `400021x1+500101x2` je presný, ale nečitateľný — rozbijeme ho na odznaky.
///
/// `"bez-vstupu"` (žiadny chart/kompas spotrebovaný pri otvorení mapy) sa od
/// TLI 17 (16.10.2026) zobrazuje ako „kampaň" — endgame mapy sa inak než cez
/// chart/kompas (`Spv3Open`/`ClimbTowerOpen`) otvoriť nedajú, takže beh bez
/// nich je vždy kampaňová/sezónna/leveling zóna, nikdy nie mapa „zadarmo".
///
/// **TLI 19 bug B (16.10.2026):** toto pravidlo bolo pre bossov zlé — boss a
/// pinnacle mapy sa otvárajú ROVNAKOU správou (`Spv3Open`/`ClimbTowerOpen`),
/// len bez spotreby chartu/kompasu, takže vychádzali tiež ako „bez-vstupu" a
/// zobrazovali sa nesprávne ako kampaň. Jadro (`core/src/tracker/stav.rs`,
/// `Beh::podpis_vstupov`) teraz tento prípad rozlišuje vlastnou hodnotou
/// `"boss-alebo-pinnacle"` — `"bez-vstupu"` ostáva vyhradené pre skutočnú
/// kampaň, kde sa `Spv3Open`/`ClimbTowerOpen` nespustil vôbec.
/// Volá sa len na už hub-filtrovaných behoch (`behy`, `zivo`, `podla_kombinacie`
/// — pozri `src-tauri/src/tracker.rs`), takže sa tu netreba pýtať na `je_hub`.
function odznakyVstupov(podpis) {
    if (!podpis) {
        return '<span class="odznak">bez vstupu</span>';
    }
    if (podpis === "bez-vstupu") {
        return '<span class="odznak">kampaň</span>';
    }
    if (podpis === "boss-alebo-pinnacle") {
        return '<span class="odznak">boss / pinnacle</span>';
    }
    return podpis
        .split("+")
        .map((kus) => {
            const [id, pocet] = kus.split("x");
            const krat = pocet && pocet !== "1" ? ` ×${pocet}` : "";
            // `title` nesie ID aj vtedy, keď sa zobrazuje meno — pri hlásení
            // chyby je číslo to jediné, čo sa dá jednoznačne dohľadať.
            return `<span class="odznak" title="#${id}">${nazovPolozky(id)}${krat}</span>`;
        })
        .join(" ");
}

function karta(popis, hodnota, detail, trieda) {
    return `<div class="karta-statistika">
        <div class="karta-popis">${popis}</div>
        <div class="karta-hodnota ${trieda || ""}">${hodnota}</div>
        <div class="karta-detail">${detail || ""}</div>
    </div>`;
}

// TLI 45/47 — sedem kariet presne podľa TITracku (zadanie bod 1). Od TLI 47
// appka ČÍTA celý batoh (CURRENT INVENTORY, cez InitBagData po Sorte) a NET
// WORTH je skutočný súčet batohu (bez skrytých položiek) + doterajší zisk
// — kým appka ešte nevidela ani jeden Sort tejto session (`p.batoh` prázdny),
// karta to hovorí nahlas odznakom namiesto predstierania hodnoty, ktorú
// nepozná (viď aj pätička „Sortni inventár").
function vykresliKarty(p) {
    const c = p.prehlad.celkom;
    const zaHodinu = c.cas_ms > 0 ? (c.cisty_zisk * 3600000) / c.cas_ms : 0;
    const priemer = c.pocet_map > 0 ? c.cisty_zisk / c.pocet_map : 0;
    const priemernyCas = c.pocet_map > 0 ? c.cas_ms / c.pocet_map : 0;
    const maBatoh = p.batoh && p.batoh.length > 0;
    const netWorth = maBatoh ? p.net_worth_fe : c.cisty_zisk;
    const netWorthDetail = maBatoh ? "batoh (bez skrytých) + doterajší zisk" : "⏳ ešte bez batohu — sortni inventár v hre";

    document.getElementById("karty-statistik").innerHTML = [
        karta("NET WORTH", fe(netWorth), netWorthDetail, triedaZnamienka(netWorth)),
        karta("CUMULATIVE VALUE", fe(c.cisty_zisk), "dropy mínus vstupy, od spustenia appky", triedaZnamienka(c.cisty_zisk)),
        karta("FE/HOUR", fe(zaHodinu), "", triedaZnamienka(zaHodinu)),
        karta("FE/MAP", fe(priemer), "priemer na dokončenú mapu", triedaZnamienka(priemer)),
        karta("RUNS", String(c.pocet_map), "dokončených máp", ""),
        karta("AVG RUN TIME", cas(priemernyCas), "", ""),
        karta("TOTAL TIME", cas(c.cas_ms), "", ""),
    ].join("");
}

// ─── Grafy Cumulative value a FE/hour (TLI 45) ──────────────────────────────
// Čisté inline SVG bez knižnice — appka smie kresliť len to, čo sama spočíta
// z dát, ktoré už aj tak má (`p.behy`), nikam sa kvôli tomu nepripája.
function behyChronologicky(p) {
    return (p.behy || [])
        .filter((b) => b.zaciatok)
        .slice()
        .sort((a, b) => a.zaciatok.localeCompare(b.zaciatok));
}

function miniGraf(hodnoty, zaporneSuMozne) {
    if (hodnoty.length < 2) {
        return '<p class="graf-prazdny">Zatiaľ málo dokončených máp na graf.</p>';
    }
    const sirka = 600;
    const vyska = 90;
    const min = Math.min(0, ...hodnoty);
    const max = Math.max(0, ...hodnoty);
    const rozsah = max - min || 1;
    const bod = (i, v) => {
        const x = (i / (hodnoty.length - 1)) * sirka;
        const y = vyska - ((v - min) / rozsah) * vyska;
        return `${x.toFixed(1)},${y.toFixed(1)}`;
    };
    const body = hodnoty.map((v, i) => bod(i, v)).join(" ");
    const posledna = hodnoty[hodnoty.length - 1];
    const farba = zaporneSuMozne && posledna < 0 ? "var(--farba-strata)" : "var(--farba-zisk)";
    // Nulová os sa kreslí len vtedy, keď sa dáta cez ňu naozaj preklápajú —
    // inak by bola buď zbytočne na okraji, alebo úplne mimo rozsahu.
    const nulaY = vyska - ((0 - min) / rozsah) * vyska;
    const osNula = min < 0 && max > 0
        ? `<line x1="0" y1="${nulaY.toFixed(1)}" x2="${sirka}" y2="${nulaY.toFixed(1)}" stroke="var(--farba-okraj)" stroke-width="1"/>`
        : "";
    return `<svg viewBox="0 0 ${sirka} ${vyska}" preserveAspectRatio="none">
        ${osNula}
        <polyline points="${body}" fill="none" stroke="${farba}" stroke-width="2"
            stroke-linejoin="round" stroke-linecap="round"/>
    </svg>`;
}

function vykresliGrafy(p) {
    const behy = behyChronologicky(p);
    let bezik = 0;
    const kumulativ = behy.map((b) => (bezik += b.cisty_zisk));
    const feZaHodinu = behy.map((b) => (b.trvanie_ms > 0 ? (b.cisty_zisk * 3600000) / b.trvanie_ms : 0));
    document.getElementById("graf-kumulativ").innerHTML = miniGraf(kumulativ, true);
    document.getElementById("graf-fe-za-hodinu").innerHTML = miniGraf(feZaHodinu, true);
}

function vykresliKombinacie(p) {
    const telo = document.querySelector("#tabulka-kombinacie tbody");
    const skupiny = p.prehlad.podla_kombinacie || [];
    document.getElementById("prazdne-kombinacie").classList.toggle("skryte", skupiny.length > 0);
    document.getElementById("tabulka-kombinacie").classList.toggle("skryte", skupiny.length === 0);

    telo.innerHTML = skupiny
        .map((s, i) => {
            const [vstupy, zona] = s.kluc.split("@");
            const su = s.suhrn;
            const zaHodinu = su.cas_ms > 0 ? (su.cisty_zisk * 3600000) / su.cas_ms : 0;
            const priemer = su.pocet_map > 0 ? su.cisty_zisk / su.pocet_map : 0;
            const priemernyCas = su.pocet_map > 0 ? su.cas_ms / su.pocet_map : 0;
            return `<tr class="${i === 0 ? "skupina-najlepsia" : ""}">
                <td>${odznakyVstupov(vstupy)}</td>
                <td>${nazovMapy(zona)}</td>
                <td class="cislo">${su.pocet_map}</td>
                <td class="cislo ${triedaZnamienka(zaHodinu)}">${fe(zaHodinu)}</td>
                <td class="cislo ${triedaZnamienka(priemer)}">${fe(priemer)}</td>
                <td class="cislo">${cas(priemernyCas)}</td>
            </tr>`;
        })
        .join("");
}

// TLI 45 — „Recent Runs": riadok histórie sa dá rozbaliť kliknutím a ukáže
// „čo reálne padlo" (server to teraz počíta pre KAŽDÝ beh, nie len pre ten
// živý — `VysledokBehu.top_dropy`) a tlačidlo Report na export jednej mapy.
let POSLEDNY_PREHLAD = null;

function rozbalenieBehu(b) {
    const dropy = b.top_dropy || [];
    const zoznam = dropy.length
        ? dropy
              .map((d) => {
                  const hodnota = d.hodnota != null ? fe(d.hodnota) : "bez ceny";
                  return `<span class="odznak" title="#${d.polozka}">${nazovPolozky(d.polozka)} ×${d.pocet} — ${hodnota}</span>`;
              })
              .join("")
        : '<span class="rozbalenie-prazdne">Z tejto mapy nepadlo nič oceniteľné.</span>';
    return `<div class="rozbalenie-dropy">${zoznam}<button type="button" class="btn-vedlajsi btn-report" data-beh-report="${b.id}">Report</button></div>`;
}

function vykresliBehy(p) {
    const telo = document.querySelector("#tabulka-behov tbody");
    const behy = (p.behy || []).slice().reverse();
    document.getElementById("prazdne-behy").classList.toggle("skryte", behy.length > 0);
    document.getElementById("tabulka-behov").classList.toggle("skryte", behy.length === 0);

    telo.innerHTML = behy
        .map((b) => {
            const bezCeny = (b.bez_ceny || []).length
                ? ` <span class="odznak odznak-bez-ceny">bez ceny: ${b.bez_ceny.length}</span>`
                : "";
            return `<tr class="riadok-behu" data-beh-id="${b.id}" title="Klikni pre „čo padlo" (Recent Runs)">
                <td class="cislo">${b.id}</td>
                <td>${nazovMapy(b.zona)}</td>
                <td>${odznakyVstupov(b.podpis_vstupov)}</td>
                <td class="cislo">${cas(b.trvanie_ms)}</td>
                <td class="cislo">${fe(b.hodnota_dropov)}${bezCeny}</td>
                <td class="cislo">${fe(b.hodnota_vstupov)}</td>
                <td class="cislo ${triedaZnamienka(b.cisty_zisk)}">${fe(b.cisty_zisk)}</td>
            </tr>
            <tr class="riadok-rozbalenia skryte" data-beh-rozbalenie="${b.id}">
                <td colspan="7">${rozbalenieBehu(b)}</td>
            </tr>`;
        })
        .join("");
}

function textReportuBehu(b) {
    const dropy = (b.top_dropy || [])
        .map((d) => `  - ${nazovPolozky(d.polozka)} ×${d.pocet}: ${d.hodnota != null ? fe(d.hodnota) : "bez ceny"}`)
        .join("\n") || "  (nič oceniteľné)";
    return [
        `Report mapy #${b.id} — ${nazovMapy(b.zona)}`,
        `Začiatok: ${b.zaciatok || "neznámy"}`,
        `Trvanie: ${cas(b.trvanie_ms)}`,
        `Vstupy: ${odznakyVstupov(b.podpis_vstupov).replace(/<[^>]+>/g, "")}`,
        `Hodnota dropov: ${fe(b.hodnota_dropov)}`,
        `Hodnota vstupov: ${fe(b.hodnota_vstupov)}`,
        `Čistý zisk: ${fe(b.cisty_zisk)}`,
        `Čo padlo:`,
        dropy,
    ].join("\n");
}

// ─── CURRENT INVENTORY (TLI 47, `2026-10-19-0744-mcp.md` bod 2) ────────────
// Len batoh hráča (appka ho vidí z `BagMgr` PageId „ostatné" po Sorte v hre),
// NIE sklad — appka do skladu (WareHouseMgr) ani nenazerá. Appka tu nič
// nepredáva ani neposiela — je to zoznam na POZRETIE pred predajom v hre.
let inventarFilterText = "";
let inventarTriedenie = "hodnota";
let inventarZobrazSkryte = false;

function vykresliInventar(p) {
    const telo = document.querySelector("#tabulka-inventar tbody");
    const vsetky = p.batoh || [];
    document.getElementById("prazdne-inventar").classList.toggle("skryte", vsetky.length > 0);
    document.getElementById("tabulka-inventar").classList.toggle("skryte", vsetky.length === 0);

    const hladane = inventarFilterText.trim().toLowerCase();
    let zoznam = vsetky.filter((pol) => inventarZobrazSkryte || !pol.skryte);
    if (hladane) {
        zoznam = zoznam.filter((pol) => nazovPolozky(pol.config_base_id).toLowerCase().includes(hladane));
    }
    zoznam = zoznam.slice().sort((a, b) => {
        if (inventarTriedenie === "mnozstvo") {
            return b.mnozstvo - a.mnozstvo;
        }
        if (inventarTriedenie === "nazov") {
            return nazovPolozky(a.config_base_id).localeCompare(nazovPolozky(b.config_base_id));
        }
        // "hodnota" (predvolené) — neznáma cena (`null`) ide na koniec, nie hore ako „najväčšia".
        return (b.hodnota_fe ?? -1) - (a.hodnota_fe ?? -1);
    });

    telo.innerHTML = zoznam
        .map((pol) => {
            const zastarana = pol.cena_zastarana ? ' <span class="odznak odznak-bez-ceny">stará cena</span>' : "";
            // TLI 49 bod 2: appka pozná vek KAŽDEJ ceny (`Cena::kedy`) — vedľa
            // samotnej ceny ho appka zobrazí ako „(pred X min)", rovnaký text
            // ako stĺpec „Vek" v Economy (`vek_text` v Rust).
            const vek = pol.cena_vek ? ` <span class="vek-ceny">(${pol.cena_vek})</span>` : "";
            const cena = pol.cena_za_kus_fe != null ? fe(pol.cena_za_kus_fe) + vek : "bez ceny";
            const hodnota = pol.hodnota_fe != null ? fe(pol.hodnota_fe) + zastarana : "bez ceny";
            return `<tr class="${pol.skryte ? "riadok-skryty" : ""}">
                <td>${nazovPolozky(pol.config_base_id)}</td>
                <td class="cislo">${pol.mnozstvo}</td>
                <td class="cislo">${cena}</td>
                <td class="cislo">${hodnota}</td>
                <td><button type="button" class="btn-vedlajsi btn-skryt-polozku" data-id="${pol.config_base_id}" data-skryte="${pol.skryte}">${pol.skryte ? "Unhide" : "Hide"}</button></td>
            </tr>`;
        })
        .join("");
}

document.getElementById("inventar-filter").addEventListener("input", (e) => {
    inventarFilterText = e.target.value;
    vykresliInventar(POSLEDNY_PREHLAD || {});
});
document.getElementById("inventar-triedenie").addEventListener("change", (e) => {
    inventarTriedenie = e.target.value;
    vykresliInventar(POSLEDNY_PREHLAD || {});
});
document.getElementById("inventar-zobraz-skryte").addEventListener("change", (e) => {
    inventarZobrazSkryte = e.target.checked;
    vykresliInventar(POSLEDNY_PREHLAD || {});
});
document.querySelector("#tabulka-inventar tbody").addEventListener("click", async (e) => {
    const btn = e.target.closest(".btn-skryt-polozku");
    if (!btn) {
        return;
    }
    const configBaseId = Number(btn.dataset.id);
    const skryteTeraz = btn.dataset.skryte === "true";
    try {
        await invoke("tracker_prepni_skrytu_polozku", { configBaseId, skryta: !skryteTeraz });
        await obnovTracker();
    } catch (err) {
        console.error(err);
    }
});

// ─── Economy (TLI 48 bod 3, kategórie TLI 49 bod 1) — plný prehliadač cien ──
// Kategórie (`ui/polozky-kategorie-en.json`, odvodené z TITracku typ_cn,
// NOTICE.md bod 4) sú záložky nad tabuľkou — rovnaký vzor ako titrack.ninja.
let ECONOMY_DATA = [];
let economyHladanie = "";
let economyZdroj = "";
let economyKategoria = ""; // "" = Všetky
let economyTriedenie = "cena"; // "cena" | "zmena" | "vek"
let economyTrieditVzostupne = false;
let KATEGORIE_POLOZIEK = window.__KATEGORIE_POLOZIEK__ || {};

// Poradie záložiek + slovenský/anglický popis + ikona. `general` je vždy
// posledná — je to zberná kategória pre všetko, čo appka nevedela zaradiť
// presnejšie, nie hlavný prípad.
const KATEGORIE_DEFINICIE = [
    { kod: "legendary_gear", nazov: "Legendary Gear", ikona: "🗡️" },
    { kod: "compasses", nazov: "Compasses", ikona: "🧭" },
    { kod: "beacons", nazov: "Beacons", ikona: "📡" },
    { kod: "skills", nazov: "Skills", ikona: "✨" },
    { kod: "destiny", nazov: "Destiny", ikona: "🔮" },
    { kod: "organs", nazov: "Organs", ikona: "🫀" },
    { kod: "followers", nazov: "Followers", ikona: "🐾" },
    { kod: "prisms", nazov: "Prisms", ikona: "🔷" },
    { kod: "divinity", nazov: "Divinity", ikona: "⚡" },
    { kod: "netherrealm", nazov: "Netherrealm", ikona: "🔥" },
    { kod: "season_items", nazov: "Season Items", ikona: "🗓️" },
    { kod: "general", nazov: "General", ikona: "📦" },
];

if (typeof fetch === "function") {
    fetch("polozky-kategorie-en.json")
        .then((o) => (o.ok ? o.json() : null))
        .then((d) => {
            if (d && d.kategorie) {
                KATEGORIE_POLOZIEK = d.kategorie;
                vykresliEconomy();
            }
        })
        .catch(() => {});
}

/// `400021` → `"general"` (predvolené, keď appka zaradenie nepozná).
function kategoriaPolozky(id) {
    return KATEGORIE_POLOZIEK[String(id)] || "general";
}

function vykresliKategorieZalozky() {
    const obal = document.getElementById("economy-kategorie");
    if (!obal || obal.childElementCount > 0) {
        return; // vykresľuje sa raz, filtrovanie potom len prepína „aktivna"
    }
    const vsetky = document.createElement("button");
    vsetky.type = "button";
    vsetky.className = "zalozka-kategoria aktivna";
    vsetky.textContent = "Všetky";
    vsetky.dataset.kategoria = "";
    obal.appendChild(vsetky);
    for (const k of KATEGORIE_DEFINICIE) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "zalozka-kategoria";
        btn.textContent = `${k.ikona} ${k.nazov}`;
        btn.dataset.kategoria = k.kod;
        obal.appendChild(btn);
    }
    obal.addEventListener("click", (e) => {
        const btn = e.target.closest(".zalozka-kategoria");
        if (!btn) return;
        obal.querySelectorAll(".zalozka-kategoria").forEach((b) => b.classList.remove("aktivna"));
        btn.classList.add("aktivna");
        economyKategoria = btn.dataset.kategoria;
        vykresliEconomy();
    });
}
vykresliKategorieZalozky();

const ZDROJ_POPIS = {
    vlastny_trh: "Vlastný trh",
    obrazovka: "Obrazovka trhu",
    partia: "Partia",
    titrack: "titrack.ninja",
    rucne: "Ručne",
};

function economyIkona(id) {
    if (!ZAKLAD_IKON) return "";
    return `<img class="economy-ikona" loading="lazy" alt="" src="${ZAKLAD_IKON}/ikony/polozka/${id}" onerror="this.remove()">`;
}

function economyZmenaText(percent) {
    if (typeof percent !== "number" || !isFinite(percent)) {
        return '<span class="odznak odznak-bez-ceny">—</span>';
    }
    const trieda = percent > 0 ? "hodnota-kladna" : percent < 0 ? "hodnota-zaporna" : "";
    const znamienko = percent > 0 ? "+" : "";
    return `<span class="${trieda}">${znamienko}${percent.toFixed(1)} %</span>`;
}

function vykresliEconomy() {
    const telo = document.getElementById("tabulka-economy-telo");
    const hladane = economyHladanie.trim().toLowerCase();

    let zoznam = ECONOMY_DATA;
    if (economyZdroj) {
        zoznam = zoznam.filter((r) => r.zdroj === economyZdroj);
    }
    if (economyKategoria) {
        zoznam = zoznam.filter((r) => kategoriaPolozky(r.polozka) === economyKategoria);
    }
    if (hladane) {
        zoznam = zoznam.filter((r) => nazovPolozky(r.polozka).toLowerCase().includes(hladane));
    }

    zoznam = zoznam.slice().sort((a, b) => {
        let rozdiel;
        if (economyTriedenie === "zmena") {
            // Položky bez zmeny (`null`) idú vždy na koniec, nezávisle od smeru triedenia.
            if (a.zmena_24h_percent == null) return 1;
            if (b.zmena_24h_percent == null) return -1;
            rozdiel = a.zmena_24h_percent - b.zmena_24h_percent;
        } else if (economyTriedenie === "vek") {
            rozdiel = a.vek_s - b.vek_s;
        } else {
            rozdiel = a.cena_fe - b.cena_fe;
        }
        return economyTrieditVzostupne ? rozdiel : -rozdiel;
    });

    document.getElementById("economy-pocet").textContent =
        `${zoznam.length} z ${ECONOMY_DATA.length} známych cien`;
    document.getElementById("economy-prazdne").classList.toggle("skryte", ECONOMY_DATA.length > 0);
    document.getElementById("tabulka-economy").classList.toggle("skryte", ECONOMY_DATA.length === 0);

    telo.innerHTML = zoznam
        .map((r) => `<tr>
            <td>${economyIkona(r.polozka)}${nazovPolozky(r.polozka)}</td>
            <td>${ZDROJ_POPIS[r.zdroj] || r.zdroj}</td>
            <td class="cislo">${fe(r.cena_fe)}</td>
            <td class="cislo">${economyZmenaText(r.zmena_24h_percent)}</td>
            <td class="cislo">${vekText(r.vek_s)}</td>
        </tr>`)
        .join("");
}

async function economyNacitaj() {
    try {
        ECONOMY_DATA = (await invoke("tracker_economy")) || [];
    } catch (e) {
        console.error("Chyba pri načítaní Economy:", e);
        ECONOMY_DATA = [];
    }
    vykresliEconomy();
}

document.getElementById("economy-hladanie").addEventListener("input", (e) => {
    economyHladanie = e.target.value;
    vykresliEconomy();
});
document.getElementById("economy-zdroj").addEventListener("change", (e) => {
    economyZdroj = e.target.value;
    vykresliEconomy();
});
document.querySelectorAll("#tabulka-economy .economy-triedit").forEach((th) => {
    th.addEventListener("click", () => {
        const stlpec = th.dataset.stlpec;
        if (economyTriedenie === stlpec) {
            economyTrieditVzostupne = !economyTrieditVzostupne;
        } else {
            economyTriedenie = stlpec;
            economyTrieditVzostupne = false;
        }
        vykresliEconomy();
    });
});
document.getElementById("btn-nav-economy").addEventListener("click", () => economyNacitaj());

// Delegovaný listener (tabuľka sa prekresľuje á 3 s, takže sa pripája raz na
// tbody, nie na každý riadok pri každom obnovení).
document.querySelector("#tabulka-behov tbody").addEventListener("click", (e) => {
    const btnReport = e.target.closest("[data-beh-report]");
    if (btnReport) {
        const id = btnReport.dataset.behReport;
        const beh = (POSLEDNY_PREHLAD?.behy || []).find((b) => String(b.id) === id);
        if (!beh) return;
        const odkaz = document.createElement("a");
        odkaz.href = URL.createObjectURL(new Blob([textReportuBehu(beh)], { type: "text/plain;charset=utf-8" }));
        odkaz.download = `tli-report-mapa-${id}.txt`;
        odkaz.click();
        URL.revokeObjectURL(odkaz.href);
        return;
    }
    const riadok = e.target.closest("tr.riadok-behu");
    if (!riadok) return;
    const rozbalenie = document.querySelector(`tr[data-beh-rozbalenie="${riadok.dataset.behId}"]`);
    rozbalenie?.classList.toggle("skryte");
});

function vykresliZivo(p) {
    const pas = document.getElementById("zivy-pas");
    const text = document.getElementById("zivy-text");

    // Pripomienka „Enable Log" (TLI 11d) — červený stav má prednosť pred
    // bežným obsahom pásu, inak by si ho hráč pri bežiacej mape nevšimol.
    pas.classList.toggle("varovanie", Boolean(p.pripomienka_aktivna));
    if (p.pripomienka_aktivna) {
        pas.classList.remove("nekativny");
        text.textContent = "⚠️ Logovanie evidentne nebeží — zapni v hre Settings → Other Settings → „Enable Log“.";
        return;
    }

    if (p.zivo) {
        pas.classList.remove("nekativny");
        text.innerHTML = `<strong>${nazovMapy(p.zivo.zona)}</strong> · ${cas(p.zivo.trvanie_ms)}
            · <span class="${triedaZnamienka(p.zivo.cisty_zisk)}">${fe(p.zivo.cisty_zisk)}</span>
            · ${odznakyVstupov(p.zivo.podpis_vstupov)}`;
    } else {
        pas.classList.add("nekativny");
        text.textContent = p.zapnuty ? "Žiadna mapa nebeží." : "Tracker je vypnutý v nastaveniach.";
    }
}

function vykresliStav(p) {
    const el = document.getElementById("stav-trackera");
    if (p.chyba) {
        el.innerHTML = `<span class="odznak odznak-bez-ceny">pozor</span> ${p.chyba}`;
        return;
    }
    const bez = Object.keys(p.prehlad.bez_ceny_spolu || {}).length;
    // TLI 22: koľko cien appka sama prečítala z obrazovky trhu za posledných
    // 24 h (automaticky, bez hotkey) — „dnešných cien" zo zadania bod 3.
    const zObrazovky = p.pocet_cien_z_obrazovky_24h || 0;
    el.textContent =
        `Log: ${p.cesta_logu || "nenájdený"} · cien v cenníku: ${p.pocet_cien}` +
        (bez ? ` · položiek bez ceny: ${bez}` : "") +
        (zObrazovky > 0 ? ` · dnešných cien z obrazovky trhu: ${zObrazovky}` : "");
    document.getElementById("info-ceny").textContent =
        p.pocet_cien > 0
            ? "Ceny pochádzajú z tvojich vlastných vyhľadávaní na trhu; vek ceny je pri položke."
            : "Zatiaľ žiadna cena — vyhľadaj si v hre na trhu pár položiek a doplnia sa samy.";
}

// ─── Výzva „vyhľadaj na Exchange" (TLI 20) ──────────────────────────────────
// Appka do hry NIČ neposiela a ani nemôže — toto je návod, čo si má hráč
// vyhľadať sám. Zmysel: jedno vyhľadanie na in-game Exchange zapíše cenu do
// logu a cez zdieľanie ju dostane celá partia, takže dva-tri kliky doplnia
// ceny všetkým trom naraz.
const elVyzvaCeny = document.getElementById("vyzva-ceny");
const elVyzvaCenyText = document.getElementById("vyzva-ceny-text");
const btnAkoNaExchange = document.getElementById("btn-ako-na-exchange");

function vykresliVyzvuCien(p) {
    const v = p.vyzva_ceny;
    if (!v || !v.polozky || v.polozky.length === 0) {
        elVyzvaCeny.classList.add("skryte");
        return;
    }
    elVyzvaCeny.classList.remove("skryte");
    const zoznam = v.polozky.map(nazovPolozky).join(", ");
    const zvysok = v.pocet > v.polozky.length ? ` (a ďalších ${v.pocet - v.polozky.length})` : "";
    elVyzvaCenyText.textContent =
        `${v.pocet} ${slovoPoloziek(v.pocet)} z posledných máp nemá cenu mladšiu ako 24 h. ` +
        `Vyhľadaj na Exchange: ${zoznam}${zvysok}.`;
}

function slovoPoloziek(n) {
    if (n === 1) return "položka";
    return n >= 2 && n <= 4 ? "položky" : "položiek";
}

btnAkoNaExchange.addEventListener("click", () => {
    const v = elVyzvaCenyText.textContent;
    elVyzvaCenyText.textContent =
        "V hre otvor Exchange (Trade House), do hľadania napíš meno položky a " +
        "potvrď. Hra si cenu zapíše do logu, appka ju zachytí sama a " +
        (chkCeny.checked ? "pošle ju celej partii." : "ostane len u teba (zdieľanie cien máš vypnuté).");
    // Pôvodný text sa vráti pri najbližšom prekreslení (á 3 s) — netreba
    // zatvárať nič ručne.
    setTimeout(() => {
        if (elVyzvaCenyText.textContent.startsWith("V hre otvor")) {
            elVyzvaCenyText.textContent = v;
        }
    }, 12000);
});


// ─── Cesta k hernému logu (TLI 11c) ─────────────────────────────────────────
// Pole je READONLY zámerne — zmena ide len cez natívny výber súboru
// (`btn-vyber-logu`), nech appka nedostane preklep, ktorý nevie overiť za letu.
const inpCestaLogu = document.getElementById("inp-cesta-logu");
const btnVyberLogu = document.getElementById("btn-vyber-logu");
const elStavCestyLogu = document.getElementById("stav-cesty-logu");

function velkostText(b) {
    if (b < 1024) return `${b} B`;
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(0)} KB`;
    return `${(b / (1024 * 1024)).toFixed(1)} MB`;
}

function vekText(s) {
    if (s < 60) return "pred chvíľou";
    if (s < 3600) return `pred ${Math.floor(s / 60)} min`;
    return `pred ${Math.floor(s / 3600)} h`;
}

async function obnovStavCestyLogu() {
    try {
        const s = await invoke("tracker_stav_cesty", { rucna: inpCestaLogu.value || "" });
        elStavCestyLogu.classList.remove("stav-cesty-ok", "stav-cesty-varovanie");
        if (!s.existuje) {
            elStavCestyLogu.textContent =
                "⚠️ Log sa nenašiel. Klikni na „Vybrať…“ a ukáž appke súbor UE_game.log ručne.";
            elStavCestyLogu.classList.add("stav-cesty-varovanie");
            return;
        }
        if (s.takmer_prazdny) {
            elStavCestyLogu.textContent =
                "⚠️ Log je skoro prázdny. V hre zapni Settings → Other Settings → „Enable Log“ a odohraj kúsok mapy.";
            elStavCestyLogu.classList.add("stav-cesty-varovanie");
            return;
        }
        if (s.chyba_vypnuty_log) {
            elStavCestyLogu.textContent =
                "⚠️ V logu chýbajú očakávané záznamy. V hre over Settings → Other Settings → „Enable Log“.";
            elStavCestyLogu.classList.add("stav-cesty-varovanie");
            return;
        }
        const vek = typeof s.upravene_pred_s === "number" ? `, zmenené ${vekText(s.upravene_pred_s)}` : "";
        elStavCestyLogu.textContent = `✅ ${velkostText(s.velkost_b || 0)}${vek}`;
        elStavCestyLogu.classList.add("stav-cesty-ok");
    } catch (e) {
        console.error(e);
    }
}

btnVyberLogu.addEventListener("click", async () => {
    try {
        const cesta = await invoke("tracker_vyber_subor");
        if (!cesta) {
            return;
        }
        await invoke("uloz_cestu_logu", { cesta });
        inpCestaLogu.value = cesta;
        await obnovStavCestyLogu();
    } catch (e) {
        console.error(e);
        elStavCestyLogu.textContent = String(e);
    }
});

/// Pri prvom otvorení appky (prázdna uložená cesta) appka cestu nájde sama
/// (Steam register/vdf, inak krátky zoznam známych miest) a rovno ju aj
/// uloží — hráč ju tu len skontroluje, presne ako v jeho ostatných addonoch.
async function pripravCestuLogu(nastavenia) {
    let cesta = nastavenia.cesta_logu || "";
    if (!cesta) {
        try {
            const navrhnuta = await invoke("tracker_navrh_cestu");
            if (navrhnuta) {
                cesta = navrhnuta;
                await invoke("uloz_cestu_logu", { cesta });
            }
        } catch (e) {
            console.error(e);
        }
    }
    inpCestaLogu.value = cesta;
    await obnovStavCestyLogu();
}

async function obnovTracker() {
    try {
        const p = await invoke("tracker_prehlad");
        if (!p) {
            return;
        }
        POSLEDNY_PREHLAD = p;
        vykresliZivo(p);
        vykresliKarty(p);
        vykresliGrafy(p);
        vykresliKombinacie(p);
        vykresliBehy(p);
        vykresliInventar(p);
        vykresliStav(p);
        vykresliVyzvuCien(p);
        vykresliStavUploadu(p);
        vykresliPatickaUploadu(p);
        vykresliPatickaCien(p);
        vykresliVyzvuNaSort(p);

        // Automatické odoslanie partii — len keď si to hráč zapol a len raz za
        // SYNC_KAZDYCH_KOL kôl, nech sa pri každom takte nechodí na server.
        kolOdSyncu += 1;
        if (chkSync.checked && kolOdSyncu >= SYNC_KAZDYCH_KOL) {
            await posliPartii();
        }
    } catch (e) {
        console.error(e);
    }
}

document.getElementById("btn-csv").addEventListener("click", async () => {
    try {
        const csv = await invoke("tracker_csv");
        // Bez pluginu na dialógy: stiahnutie cez odkaz funguje aj vo WebView2.
        const odkaz = document.createElement("a");
        odkaz.href = URL.createObjectURL(new Blob([csv], { type: "text/csv;charset=utf-8" }));
        odkaz.download = "tli-tracker.csv";
        odkaz.click();
        URL.revokeObjectURL(odkaz.href);
    } catch (e) {
        console.error(e);
    }
});

// ─── Guide (TLI 11, krok 4) ─────────────────────────────────────────────────
// Dáta prichádzajú hotové z Rustu (`guide_data`) — vrátane ikon ako `data:` URI
// a bez `<script>`. JS tu nič nesťahuje a nič nečistí.
let guideData = null;
let guideVybrany = "";

function guideKapitoly(d) {
    if (!d) return [];
    const zoznam = [];
    
    if (d.tahak) {
        zoznam.push(d.tahak);
    }
    
    if (d.kapitoly && d.kapitoly.length > 0) {
        zoznam.push(...d.kapitoly);
    }
    
    return zoznam;
}

function ukazKapitolu(slug) {
    guideVybrany = slug;
    
    // Zvýrazní aktívne tlačidlo kapitoly v ľavom menu
    const tlacidla = document.querySelectorAll(".guide-polozka");
    for (const btn of tlacidla) {
        if (btn.dataset.slug === slug) {
            btn.classList.add("aktivna");
        } else {
            btn.classList.remove("aktivna");
        }
    }

    // Nájde obsah zvolenej kapitoly a vloží ho do dom (HTML už prešlo bezpečnostnou čistkou v Ruste)
    const elementObsah = document.getElementById("guide-obsah");
    const kapitoly = guideKapitoly(guideData);
    const najdenaKapitola = kapitoly.find(k => k.slug === slug);
    
    if (najdenaKapitola) {
        elementObsah.innerHTML = najdenaKapitola.html;
    } else {
        elementObsah.innerHTML = "";
    }

    // Skryje panel s externým odkazom, ak náhodou zostal otvorený z minulej kapitoly
    document.getElementById("guide-odkaz").classList.add("skryte");
}

function vykresliGuide(d) {
    guideData = d;

    const odsekPrazdny = document.getElementById("guide-prazdny");
    const divRozlozenie = document.querySelector(".guide-rozlozenie");
    const textStav = document.getElementById("guide-stav");
    const menuZoznam = document.getElementById("guide-zoznam");

    // Očistíme predchádzajúci stav chybovej hlášky
    textStav.classList.remove("hodnota-zaporna");

    // Ak sa sprievodca ešte nestiahol
    if (d.je_prazdny) {
        odsekPrazdny.classList.remove("skryte");
        divRozlozenie.classList.add("skryte");
        textStav.textContent = "Zatiaľ nestiahnuté";
        return;
    }

    // Dáta existujú, prepneme zobrazenie UI
    odsekPrazdny.classList.add("skryte");
    divRozlozenie.classList.remove("skryte");

    // Vypísanie informácie o veku a verzii dát
    if (d.vek === "nikdy") {
        textStav.textContent = "Zatiaľ nestiahnuté";
    } else {
        textStav.textContent = `Stiahnuté ${d.vek} · verzia ${d.verzia}`;
    }

    // Generovanie ľavého menu s kapitolami
    menuZoznam.innerHTML = "";
    const kapitoly = guideKapitoly(d);
    let vybranaStaleExistuje = false;

    for (const kap of kapitoly) {
        const tlacidlo = document.createElement("button");
        tlacidlo.type = "button";
        tlacidlo.className = "guide-polozka";
        tlacidlo.dataset.slug = kap.slug;
        tlacidlo.textContent = kap.nazov;

        tlacidlo.addEventListener("click", () => ukazKapitolu(kap.slug));
        menuZoznam.appendChild(tlacidlo);

        if (kap.slug === guideVybrany) {
            vybranaStaleExistuje = true;
        }
    }

    // Automatický výber prvej kapitoly, ak predtým nebola žiadna zvolená, alebo zmizla (napr. nová verzia)
    if (!vybranaStaleExistuje && kapitoly.length > 0) {
        ukazKapitolu(kapitoly[0].slug);
    } else if (vybranaStaleExistuje) {
        ukazKapitolu(guideVybrany); // Slúži na refresh obsahu v prípade aktualizácie
    }
}

async function nacitajGuide() {
    try {
        const data = await invoke("guide_data");
        vykresliGuide(data);
    } catch (chyba) {
        console.error("Chyba pri naťahovaní lokálnych dát pre Guide:", chyba);
    }
}

// Obsluha tlačidla na kontrolu aktualizácií Guide zo servera
const btnGuideSync = document.getElementById("btn-guide-sync");
if (btnGuideSync) {
    btnGuideSync.addEventListener("click", async () => {
        const povodnyText = btnGuideSync.textContent;
        const textStav = document.getElementById("guide-stav");
        
        // Zablokovanie tlačidla počas volania, aby používateľ neklikal viackrát
        btnGuideSync.disabled = true;
        btnGuideSync.textContent = "Sťahujem...";

        try {
            const noveData = await invoke("guide_sync");
            vykresliGuide(noveData);
        } catch (chybovaVeta) {
            // Rust vracia string pri chybe (napr. chybný kľúč) - ukážeme to červeno namiesto normálneho stavu
            textStav.textContent = chybovaVeta;
            textStav.classList.add("hodnota-zaporna");
        } finally {
            // Vždy odblokovať, aj pri chybe
            btnGuideSync.disabled = false;
            btnGuideSync.textContent = povodnyText;
        }
    });
}

// Delegácia eventov kliknutia na odkazy priamo z vykresleného HTML (obsah kapitol).
// Appka zámerne neotvára cudzie adresy (`otvor_web` ich odmieta) — odkaz na
// tlicompendium.com či YouTube sa preto ukáže v lište a dá sa skopírovať.
const divGuideObsah = document.getElementById("guide-obsah");
if (divGuideObsah) {
    divGuideObsah.addEventListener("click", (udalost) => {
        const odkaz = udalost.target.closest("a");
        if (!odkaz) return;

        // Vnútorné odkazy (bez data-externy) necháme na pokoji (napr. posun v rámci dokumentu)
        if (odkaz.dataset.externy === "1") {
            udalost.preventDefault();

            const panelOdkazu = document.getElementById("guide-odkaz");
            const urlText = document.getElementById("guide-odkaz-url");

            urlText.textContent = odkaz.href;
            panelOdkazu.classList.remove("skryte");
        }
    });
}

// Obsluha kopírovania externého odkazu zo zobrazenej lišty do schránky (clipboard)
const btnGuideKopiruj = document.getElementById("btn-guide-kopiruj");
if (btnGuideKopiruj) {
    btnGuideKopiruj.addEventListener("click", async () => {
        const elementUrl = document.getElementById("guide-odkaz-url");
        const hodnotaUrl = elementUrl.textContent;

        try {
            // Moderné API pre kopírovanie textu do schránky
            await navigator.clipboard.writeText(hodnotaUrl);
        } catch (chyba) {
            // Fallback pre prípad nedostatočných práv: namiesto uloženia do schránky
            // iba manuálne označí text, aby si ho používateľ mohol skopírovať sám (Ctrl+C).
            const range = document.createRange();
            range.selectNodeContents(elementUrl);
            
            const selekcia = window.getSelection();
            selekcia.removeAllRanges();
            selekcia.addRange(range);
        }
    });
}

// Schovanie lišty na externé odkazy
const btnGuideZavriOdkaz = document.getElementById("btn-guide-zavri-odkaz");
if (btnGuideZavriOdkaz) {
    btnGuideZavriOdkaz.addEventListener("click", () => {
        document.getElementById("guide-odkaz").classList.add("skryte");
    });
}


// ─── Prepínače a synchronizácia s partiou (TLI 11, krok 5) ──────────────────
// Oba prepínače ukladá Rust samostatným príkazom `uloz_prepinac` — checkbox
// nesmie prepísať kľúč ani skratky tým, že pošle zastarané celé nastavenia.

const chkMini = document.getElementById("chk-mini");
const chkSync = document.getElementById("chk-sync");
const chkCeny = document.getElementById("chk-ceny");
const chkPripomienka = document.getElementById("chk-pripomienka");
const chkUpload = document.getElementById("chk-upload");
const chkCenyObrazovka = document.getElementById("chk-ceny-obrazovka");
const btnSyncTeraz = document.getElementById("btn-sync-teraz");
const stavSync = document.getElementById("stav-sync");
const btnUploadTeraz = document.getElementById("btn-upload-teraz");
const stavUpload = document.getElementById("stav-upload");
const pitaUpload = document.getElementById("pita-upload");
const pitaCeny = document.getElementById("pita-ceny");
const pitaSortVyzva = document.getElementById("pita-sort-vyzva");

/// „Ceny aktualizované HH:MM" (TLI 49 bod 2) — `p.ceny_synchronizovane_o` je
/// unixový čas v sekundách, appka si z neho sama urobí lokálny čas hráča
/// (appka časové pásmo nikde inde neprevádza, nemá začínať práve tu).
function vykresliPatickaCien(p) {
    if (typeof p.ceny_synchronizovane_o !== "number") {
        pitaCeny.textContent = "Ceny aktualizované: zatiaľ nikdy";
        return;
    }
    const cas = new Date(p.ceny_synchronizovane_o * 1000);
    const hhmm = cas.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    pitaCeny.textContent = "Ceny aktualizované: " + hhmm;
    const ekonomySpan = document.getElementById("economy-synchronizovane");
    if (ekonomySpan) {
        ekonomySpan.textContent = hhmm;
    }
}

/// Nad koľkými sekundami bez ÚSPEŠNÉHO uploadu appka pri bežiacej hre ukáže
/// varovanie namiesto bežného „Posledný upload: …" (TLI 47, zadanie
/// `2026-10-19-0744-mcp.md` bod 1a). 15 minút — appka sa o upload pokúša
/// oveľa častejšie, takže toľko ticha znamená, že niečo nefunguje.
const UPLOAD_VAROVANIE_PO_S = 15 * 60;

/// Koľko kôl `obnovTracker` (á 3 s) prejde medzi dvoma automatickými odoslaniami.
/// 60 kôl = 3 minúty: dosť zriedka na to, aby to nikoho neobťažovalo, a dosť
/// často na to, aby partia videla výsledky ešte počas hrania.
const SYNC_KAZDYCH_KOL = 60;
let kolOdSyncu = 0;

function ukazStavSyncu(text, chyba) {
    stavSync.textContent = text;
    stavSync.classList.toggle("hodnota-zaporna", Boolean(chyba));
}

async function prepni(meno, zapnuty) {
    try {
        await invoke("uloz_prepinac", { meno, zapnuty });
    } catch (e) {
        console.error(e);
        ukazStavSyncu(String(e), true);
    }
}

chkMini.addEventListener("change", () => prepni("mini_overlay", chkMini.checked));

chkCeny.addEventListener("change", async () => {
    await prepni("ceny_sync", chkCeny.checked);
    if (chkCeny.checked) {
        // Po zapnutí hneď jedno kolo — inak by hráč klikol a päť minút
        // nevidel, či to vôbec funguje.
        try {
            ukazStavSyncu(await invoke("ceny_sync_teraz"), false);
        } catch (e) {
            ukazStavSyncu(String(e), true);
        }
    } else {
        ukazStavSyncu("Ceny sa s partiou nezdieľajú.", false);
    }
});
chkPripomienka.addEventListener("change", () => prepni("pripominat_enable_log", chkPripomienka.checked));
chkCenyObrazovka.addEventListener("change", () => prepni("ceny_obrazovka_zapnute", chkCenyObrazovka.checked));

chkSync.addEventListener("change", async () => {
    await prepni("tracker_sync", chkSync.checked);
    // Po zapnutí sa pošle hneď prvá dávka — inak by hráč klikol a pol hodiny
    // nevidel, či to vôbec funguje.
    if (chkSync.checked) {
        await posliPartii();
    } else {
        ukazStavSyncu("Partii sa neposiela nič.", false);
    }
});

async function posliPartii() {
    kolOdSyncu = 0;
    try {
        ukazStavSyncu(await invoke("tracker_sync_teraz"), false);
    } catch (e) {
        ukazStavSyncu(String(e), true);
    }
}

btnSyncTeraz.addEventListener("click", async () => {
    btnSyncTeraz.disabled = true;
    try {
        await posliPartii();
    } finally {
        btnSyncTeraz.disabled = false;
    }
});

function ukazStavUploadu(text, chyba) {
    stavUpload.textContent = text;
    stavUpload.classList.toggle("hodnota-zaporna", Boolean(chyba));
}

/// Rovnaký stav v pätičke dashboardu (TLI 47) — na rozdiel od `#stav-upload`
/// (panel Nastavení trackera, skrytý kým nerozbalíš sekciu) je toto vidieť
/// VŽDY, presne ako TITrack „Log was last updated" dole pod appkou.
function vykresliPatickaUploadu(p) {
    if (!chkUpload.checked) {
        pitaUpload.textContent = "Posledný upload: vypnutý";
        pitaUpload.classList.remove("hodnota-zaporna");
        return;
    }
    // Hra momentálne beží/mapuje — vtedy má zmysel pýtať sa „kedy naposledy
    // niečo odišlo", mimo hry môže byť ticho dlho a je to v poriadku.
    const hraBezi = Boolean(p.zivo);
    if (p.posledna_chyba_uploadu) {
        const kedy = p.posledna_kontrola_uploadu_pred ? ` (${p.posledna_kontrola_uploadu_pred})` : "";
        pitaUpload.textContent = "⚠ Log sa neposiela na server — chyba" + kedy;
        pitaUpload.classList.add("hodnota-zaporna");
    } else if (hraBezi && typeof p.posledny_upload_pred_s === "number" && p.posledny_upload_pred_s > UPLOAD_VAROVANIE_PO_S) {
        pitaUpload.textContent = "⚠ Log sa neposiela na server — posledný upload " + p.posledna_kontrola_uploadu_pred;
        pitaUpload.classList.add("hodnota-zaporna");
    } else if (p.posledna_kontrola_uploadu_pred) {
        pitaUpload.textContent = "Posledný upload: " + p.posledna_kontrola_uploadu_pred;
        pitaUpload.classList.remove("hodnota-zaporna");
    } else {
        pitaUpload.textContent = "Posledný upload: zatiaľ žiadny";
        pitaUpload.classList.remove("hodnota-zaporna");
    }
}

/// „Sortni inventár" (TLI 47 bod 1c) — appka vie o CURRENT INVENTORY len z
/// `InitBagData`, ktoré hra zapíše pri Sorte (aj pri prihlásení). Kým sa to
/// nestane, `p.batoh` je prázdny a zoznam na predaj aj NET WORTH klamú, že
/// hráč nemá nič — treba o tom vedieť, nie si to mýliť s „naozaj nič nemá".
function vykresliVyzvuNaSort(p) {
    const hraBezi = Boolean(p.zivo) || (p.behy && p.behy.length > 0);
    if (hraBezi && (!p.batoh || p.batoh.length === 0)) {
        pitaSortVyzva.textContent = "⚠ Sortni inventár v hre — appka ešte nevidela batoh";
    } else {
        pitaSortVyzva.textContent = "";
    }
}

/// Stav uploadu AJ z tichého behu na pozadí (TLI 21, `2026-10-16-1528-mcp.md`
/// bod 4) — predtým sa `stav-upload` menil len po ručnom kliku na „Nahrať log
/// teraz" a medzitým bolo vidieť naposledy zobrazenú (často už zastaranú)
/// hlášku, aj keby upload na pozadí celý čas tichy zlyhával na chybe servera.
function vykresliStavUploadu(p) {
    if (!chkUpload.checked) {
        return; // "Log sa na server nenahráva." z `chkUpload` change zostáva.
    }
    if (p.posledna_chyba_uploadu) {
        const kedy = p.posledna_kontrola_uploadu_pred ? ` (${p.posledna_kontrola_uploadu_pred})` : "";
        ukazStavUploadu("Chyba uploadu" + kedy + ": " + p.posledna_chyba_uploadu, true);
    } else if (p.posledna_kontrola_uploadu_pred) {
        ukazStavUploadu("Upload v poriadku — posledná kontrola " + p.posledna_kontrola_uploadu_pred + ".", false);
    }
    // Keď appka ešte ANI RAZ neskúsila (oba `null`), nechaj text tak, ako je
    // (napr. prázdny, alebo z manuálneho kliku pred chvíľou).
}

async function nahrajLogTeraz() {
    try {
        ukazStavUploadu(await invoke("upload_teraz"), false);
    } catch (e) {
        ukazStavUploadu(String(e), true);
    }
}

chkUpload.addEventListener("change", async () => {
    await prepni("upload_logov", chkUpload.checked);
    if (chkUpload.checked) {
        // Po zapnutí hneď jedno kolo — inak by hráč klikol a desať minút
        // nevidel, či to vôbec funguje.
        await nahrajLogTeraz();
    } else {
        ukazStavUploadu("Log sa na server nenahráva.", false);
    }
    vykresliPatickaUploadu(POSLEDNY_PREHLAD || {});
});

btnUploadTeraz.addEventListener("click", async () => {
    btnUploadTeraz.disabled = true;
    try {
        await nahrajLogTeraz();
    } finally {
        btnUploadTeraz.disabled = false;
    }
});


// ─── Hlavičkové tlačidlá podľa TITracku (TLI 45) ───────────────────────────
// Overlay/Economy/Cloud Sync/⚙/Instructions/Reset Stats — každé otvára
// existujúcu funkciu appky (prepnutie záložky, skok na existujúci prepínač,
// nový modál alebo nový príkaz), nič sa neduplikuje.
function skocNaZalozkuA(panel, cielId) {
    const tlacidloZalozky = document.querySelector(`.zalozka[data-panel="${panel}"]`);
    if (tlacidloZalozky) prepniZalozku(tlacidloZalozky);
    if (!cielId) return;
    const ciel = document.getElementById(cielId);
    if (!ciel) return;
    ciel.scrollIntoView({ behavior: "smooth", block: "center" });
    ciel.classList.remove("zvyraznene-skokom");
    // Reflow pred opätovným pridaním triedy, nech sa animácia prehrá aj pri
    // druhom kliku na to isté tlačidlo za sebou.
    void ciel.offsetWidth;
    ciel.classList.add("zvyraznene-skokom");
}

document.getElementById("btn-nav-overlay").addEventListener("click", () => skocNaZalozkuA("panel-solvery"));
document.getElementById("btn-nav-economy").addEventListener("click", () => skocNaZalozkuA("panel-economy"));
document.getElementById("btn-nav-cloudsync").addEventListener("click", () => skocNaZalozkuA("panel-tracker", "skupina-cloudsync"));
document.getElementById("btn-nav-nastavenia").addEventListener("click", () => skocNaZalozkuA("panel-nastavenia"));

const modalInstructions = document.getElementById("modal-instructions");
document.getElementById("btn-nav-instructions").addEventListener("click", () => modalInstructions.classList.remove("skryte"));
document.getElementById("btn-instructions-zavri").addEventListener("click", () => modalInstructions.classList.add("skryte"));
modalInstructions.addEventListener("click", (e) => {
    if (e.target === modalInstructions) modalInstructions.classList.add("skryte");
});

// „Reset Stats" (TLI 45) — potvrdenie natívnym `confirm()`, appka na to
// nepotrebuje žiadny plugin (rovnaký prístup ako Blob-download exporty
// vyššie). Vymaže len živé behy tejto relácie — história v Analýze ostáva.
document.getElementById("btn-nav-resetstats").addEventListener("click", async () => {
    const ok = window.confirm(
        "Vynulovať zobrazené štatistiky (NET WORTH, CUMULATIVE VALUE, Recent Runs)?\n" +
        "Dlhodobá história v záložke Analýza sa tým NEMAŽE."
    );
    if (!ok) return;
    try {
        await invoke("tracker_reset_zive_staty");
        await obnovTracker();
    } catch (e) {
        console.error(e);
    }
});

// ─── Footer „Collector" (TLI 45) ────────────────────────────────────────────
document.getElementById("btn-pita-aktualizacia").addEventListener("click", async () => {
    const el = document.getElementById("pita-aktualizacia-stav");
    el.textContent = "Kontrolujem…";
    try {
        const urlNovejVerzie = await invoke("skontroluj_aktualizaciu");
        el.textContent = urlNovejVerzie === null ? "Máš najnovšiu verziu." : "Je vonku novšia verzia — otvor Nastavenia.";
    } catch (e) {
        el.textContent = "Chyba pri kontrole: " + e;
    }
    // Ručné tlačidlo ide cez to isté miesto v Ruste ako periodická kontrola
    // (TLI 52 bod 2), takže výsledok sa prejaví aj v `#pita-posledna-kontrola`
    // — vypýtať si ho rovno netreba, prišla/príde aj udalosť `aktualizacia-kontrola`.
});

/// „posledná kontrola HH:MM" (TLI 52 bod 2) — `k.cas` je unixový čas
/// v sekundách, rovnaký vzor ako `vykresliPatickaCien` vyššie. `k` je `null`,
/// kým appka ešte ani raz neskontrolovala (napr. offline štart bez pripojenia
/// predtým, než prvá kontrola stihla dobehnúť).
function vykresliPatickaAktualizacie(k) {
    const el = document.getElementById("pita-posledna-kontrola");
    if (!k) {
        el.textContent = "";
        return;
    }
    const hhmm = new Date(k.cas * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    if (k.chyba) {
        el.textContent = `posledná kontrola ${hhmm} (zlyhala)`;
    } else if (k.najdena_verzia) {
        el.textContent = `posledná kontrola ${hhmm} — je vonku verzia ${k.najdena_verzia}`;
    } else {
        el.textContent = `posledná kontrola ${hhmm}`;
    }
}

// ─── Štart ──────────────────────────────────────────────────────────────────
document.getElementById("btn-otvor-nastavenia").addEventListener("click", async () => {
    try {
        await invoke("otvor_nastavenia");
    } catch (e) {
        console.error(e);
    }
});

document.addEventListener("DOMContentLoaded", async () => {
    try {
        const v = await invoke("verzia");
        document.getElementById("verzia-text").textContent = `v${v}`;
        document.getElementById("pita-verzia").textContent = `verzia ${v}`;
    } catch (e) {
        console.error(e);
    }

    // TLI 52 bod 2: posledný výsledok periodickej kontroly — okno `hlavne`
    // sa mohlo otvoriť AŽ PO tom, čo vlákno na pozadí už raz skontrolovalo.
    try {
        vykresliPatickaAktualizacie(await invoke("posledna_kontrola_aktualizacie"));
    } catch (e) {
        console.error(e);
    }
    if (window.__TAURI__.event) {
        window.__TAURI__.event.listen("aktualizacia-kontrola", (udalost) => vykresliPatickaAktualizacie(udalost.payload));
    }

    try {
        const nastavenia = await invoke("nacitaj_nastavenia");
        document.getElementById("hotkey-tartarus").textContent = nastavenia.hotkey || "F8";
        document.getElementById("hotkey-canvas").textContent = nastavenia.hotkey_canvas || "F9";
        if (!parametre.get("tema")) {
            nastavTemu(nastavenia.tema || "svetla");
        }
        chkMini.checked = Boolean(nastavenia.mini_overlay);
        chkSync.checked = Boolean(nastavenia.tracker_sync);
        // Predvolené ZAPNUTÉ — staré uložené nastavenia toto pole nemajú vôbec,
        // `!== false` preto drží checkbox zaškrtnutý, kým ho hráč výslovne nevypne.
        chkPripomienka.checked = nastavenia.pripominat_enable_log !== false;
        // To isté platí pre zdieľanie cien (TLI 20) — predvolene zapnuté.
        chkCeny.checked = nastavenia.ceny_sync !== false;
        // A pre upload logov (TLI 14) — tiež predvolene zapnuté.
        chkUpload.checked = nastavenia.upload_logov !== false;
        // A pre ceny z obrazovky trhu (TLI 22) — tiež predvolene zapnuté.
        chkCenyObrazovka.checked = nastavenia.ceny_obrazovka_zapnute !== false;
        if (nastavenia.url_servera) {
            ZAKLAD_IKON = String(nastavenia.url_servera).replace(/\/+$/, "");
        }
        ukazStavSyncu(chkSync.checked ? "Posiela sa automaticky každé tri minúty."
                                      : "Partii sa neposiela nič.", false);
        ukazStavUploadu(chkUpload.checked ? "Nahráva sa automaticky na pozadí."
                                          : "Log sa na server nenahráva.", false);
        await pripravCestuLogu(nastavenia);
    } catch (e) {
        console.error(e);
    }

    await nacitajGuide();
    await analyzaHladaj();
    await taktikyNacitaj();
    await economyNacitaj();

    await obnovTracker();
    // Takt 3 s je len záložka — živé čítanie na pozadí (notify + 1s polling,
    // TLI 11c) pošle udalosť hneď, ako pribudnú nové riadky, a tá nižšie
    // spustí prekreslenie okamžite namiesto čakania na najbližší takt.
    setInterval(obnovTracker, 3000);
    if (window.__TAURI__.event) {
        window.__TAURI__.event.listen("tracker-zmena", () => obnovTracker());
    }
});

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

// --- „Odfotiť teraz" (TLI 28, bod 3) — funguje bez klávesy, s oneskorením
// 2 s, aby hráč stihol prepnúť späť do hry. Tlačidlo sa na tú chvíľu zablokuje
// a odpočítava, nech je jasné, že appka naozaj niečo robí. ---
function odfotTerazTlacidlo(button, prikaz, nazov) {
    button.addEventListener("click", async () => {
        const povodnyText = button.textContent;
        button.disabled = true;
        try {
            await invoke(prikaz);
            let zostava = 2;
            button.textContent = `Odfotím o ${zostava} s…`;
            const odpocet = setInterval(() => {
                zostava -= 1;
                if (zostava > 0) {
                    button.textContent = `Odfotím o ${zostava} s…`;
                } else {
                    clearInterval(odpocet);
                    button.textContent = povodnyText;
                    button.disabled = false;
                }
            }, 1000);
        } catch (e) {
            console.error(`Odfotenie (${nazov}) zlyhalo:`, e);
            button.textContent = povodnyText;
            button.disabled = false;
        }
    });
}
odfotTerazTlacidlo(document.getElementById("btn-odfot-tartarus"), "odfot_tartarus_teraz", "Tartarus");
odfotTerazTlacidlo(document.getElementById("btn-odfot-canvas"), "odfot_canvas_teraz", "Canvas");

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
        await taktikyNacitaj();
    } catch (e) {
        stavEl.textContent = `Zlyhalo: ${e}`;
        console.error(e);
    }
});
