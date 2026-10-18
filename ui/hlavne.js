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
        vykresliZivo(p);
        vykresliKarty(p);
        vykresliKombinacie(p);
        vykresliBehy(p);
        vykresliStav(p);
        vykresliVyzvuCien(p);
        vykresliStavUploadu(p);

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
});

btnUploadTeraz.addEventListener("click", async () => {
    btnUploadTeraz.disabled = true;
    try {
        await nahrajLogTeraz();
    } finally {
        btnUploadTeraz.disabled = false;
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
        // To isté platí pre zdieľanie cien (TLI 20) — predvolene zapnuté.
        chkCeny.checked = nastavenia.ceny_sync !== false;
        // A pre upload logov (TLI 14) — tiež predvolene zapnuté.
        chkUpload.checked = nastavenia.upload_logov !== false;
        // A pre ceny z obrazovky trhu (TLI 22) — tiež predvolene zapnuté.
        chkCenyObrazovka.checked = nastavenia.ceny_obrazovka_zapnute !== false;
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
