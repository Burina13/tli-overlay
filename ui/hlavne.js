const { invoke } = window.__TAURI__.core;

// ─── Záložky ────────────────────────────────────────────────────────────────
const zalozky = document.querySelectorAll(".zalozka");
const panely = document.querySelectorAll(".panel");

function prepni(tlacidlo) {
    zalozky.forEach((t) => t.classList.remove("aktivna"));
    panely.forEach((p) => p.classList.add("skryte"));
    tlacidlo.classList.add("aktivna");
    document.getElementById(tlacidlo.dataset.panel).classList.remove("skryte");
}

zalozky.forEach((tlacidlo) => {
    tlacidlo.addEventListener("click", () => prepni(tlacidlo));
});

// `?tab=panel-solvery` otvorí rovno danú záložku — slúži len CI náhľadu
// (screenshot každej záložky zvlášť), appka sama o sebe URL parametre nepoužíva.
const parametre = new URLSearchParams(window.location.search);
const ziadanaZalozka = parametre.get("tab");
if (ziadanaZalozka) {
    const tlacidlo = document.querySelector(`.zalozka[data-panel="${ziadanaZalozka}"]`);
    if (tlacidlo) {
        prepni(tlacidlo);
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

/// Interná cesta zóny je dlhá a čínska — do tabuľky ide posledný kus názvu.
function nazovMapy(cesta) {
    if (!cesta) {
        return NEZNAMA_MAPA;
    }
    const kus = cesta.split("/").pop() || cesta;
    return kus.split(".").pop() || kus;
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
function odznakyVstupov(podpis) {
    if (!podpis || podpis === "bez-vstupu") {
        return '<span class="odznak">bez vstupu</span>';
    }
    return podpis
        .split("+")
        .map((kus) => {
            const [id, pocet] = kus.split("x");
            const krat = pocet && pocet !== "1" ? ` ×${pocet}` : "";
            return `<span class="odznak">#${id}${krat}</span>`;
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

function vykresliKarty(p) {
    const c = p.prehlad.celkom;
    const zaHodinu = c.cas_ms > 0 ? (c.cisty_zisk * 3600000) / c.cas_ms : 0;
    const mapZaHodinu = c.cas_ms > 0 ? (c.pocet_map * 3600000) / c.cas_ms : 0;
    const priemer = c.pocet_map > 0 ? c.cisty_zisk / c.pocet_map : 0;
    const priemernyCas = c.pocet_map > 0 ? c.cas_ms / c.pocet_map : 0;

    document.getElementById("karty-statistik").innerHTML = [
        karta("Čistý zisk", fe(c.cisty_zisk), "dropy mínus vstupy", triedaZnamienka(c.cisty_zisk)),
        karta("Čistý zisk za hodinu", fe(zaHodinu), "", triedaZnamienka(zaHodinu)),
        karta("Máp za hodinu", mapZaHodinu.toFixed(1), `spolu ${c.pocet_map}`, ""),
        karta("Priemer na mapu", fe(priemer), `priemerný čas ${cas(priemernyCas)}`, triedaZnamienka(priemer)),
        karta("Hodnota dropov", fe(c.hodnota_dropov), `vstupy ${fe(c.hodnota_vstupov)}`, ""),
    ].join("");
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
            return `<tr>
                <td class="cislo">${b.id}</td>
                <td>${nazovMapy(b.zona)}</td>
                <td>${odznakyVstupov(b.podpis_vstupov)}</td>
                <td class="cislo">${cas(b.trvanie_ms)}</td>
                <td class="cislo">${fe(b.hodnota_dropov)}${bezCeny}</td>
                <td class="cislo">${fe(b.hodnota_vstupov)}</td>
                <td class="cislo ${triedaZnamienka(b.cisty_zisk)}">${fe(b.cisty_zisk)}</td>
            </tr>`;
        })
        .join("");
}

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
    el.textContent =
        `Log: ${p.cesta_logu || "nenájdený"} · cien v cenníku: ${p.pocet_cien}` +
        (bez ? ` · položiek bez ceny: ${bez}` : "");
    document.getElementById("info-ceny").textContent =
        p.pocet_cien > 0
            ? "Ceny pochádzajú z tvojich vlastných vyhľadávaní na trhu; vek ceny je pri položke."
            : "Zatiaľ žiadna cena — vyhľadaj si v hre na trhu pár položiek a doplnia sa samy.";
}

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
        vykresliZivo(p);
        vykresliKarty(p);
        vykresliKombinacie(p);
        vykresliBehy(p);
        vykresliStav(p);

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
const chkPripomienka = document.getElementById("chk-pripomienka");
const btnSyncTeraz = document.getElementById("btn-sync-teraz");
const stavSync = document.getElementById("stav-sync");

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
chkPripomienka.addEventListener("change", () => prepni("pripominat_enable_log", chkPripomienka.checked));

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
    } catch (e) {
        console.error(e);
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
        ukazStavSyncu(chkSync.checked ? "Posiela sa automaticky každé tri minúty."
                                      : "Partii sa neposiela nič.", false);
        await pripravCestuLogu(nastavenia);
    } catch (e) {
        console.error(e);
    }

    await nacitajGuide();
    await analyzaHladaj();

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
    } catch (e) {
        stavEl.textContent = `Import zlyhal: ${e}`;
        console.error(e);
    }
});
