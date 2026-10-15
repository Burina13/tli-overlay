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
    } catch (e) {
        console.error(e);
    }

    await obnovTracker();
    // Takt 3 s: dosť rýchle na to, aby hráč videl mapu pribudnúť, a dosť pomalé
    // na to, aby čítanie logu nestálo ani percento procesora.
    setInterval(obnovTracker, 3000);
});
