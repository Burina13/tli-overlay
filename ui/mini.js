const { invoke } = window.__TAURI__.core;

// Prevedie milisekundy na citatelny text: "3 m 05 s" alebo "42 s".
// Pre zapornu hodnotu alebo nulu vracia pomlcku.
function cas(ms) {
    if (typeof ms !== "number" || ms <= 0) {
        return "—";
    }
    
    const celkoveSekundy = Math.floor(ms / 1000);
    const minuty = Math.floor(celkoveSekundy / 60);
    const sekundy = celkoveSekundy % 60;

    if (minuty > 0) {
        // Zabezpecenie formatu dvoch cislic pre sekundy (napr. 05 s)
        const sekundyFormat = sekundy < 10 ? "0" + sekundy : sekundy;
        return minuty + " m " + sekundyFormat + " s";
    }
    
    return sekundy + " s";
}

// Formatuje hodnotu na pozadovany FE format.
// Ziska hodnoty nad 100 bez desatinnych miest, do 100 s dvomi.
function fe(h) {
    if (h === null || h === undefined || !isFinite(h)) {
        return "—";
    }
    
    if (Math.abs(h) >= 100) {
        return Math.round(h) + " FE";
    }
    
    return h.toFixed(2) + " FE";
}

// TLI 55 — Richard 19.10.2026: nadpis loot overlaya počas mapy ukazoval
// surové interné meno enginu („SD_DuiLongJuQiang211") namiesto čitateľného
// mena mapy. Appka tento preklad UŽ MÁ pre hlavný dashboard (`nazovMapy` v
// `ui/hlavne.js`) — tu je to ZÁMERNE rovnaká logika duplikovaná priamo do
// tohto súboru (rovnaký vzor ako `cas()`/`fe()` vyššie, ktoré sú tiež
// vlastnou kópiou v oboch oknách, nie zdieľaný modul): mini-panel je
// samostatné Tauri okno s vlastným `<script>`, žiadny bundler medzi oknami
// nič nezdieľa. `PREKLAD_MAPY` tu drž V ZHODE s `ui/hlavne.js` — pribudne-li
// tam nová položka, pridaj ju aj sem.
const PREKLAD_MAPY = {
    yujinzhixibinansuo: "Ember's Rest",
};

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

// Interná cesta zóny je dlhá a čínska (pinyin) — do panela ide čitateľné
// meno mapy, nikdy surová cesta (TLI 17/TLI 55).
function nazovMapy(cesta) {
    if (!cesta) return "Neznáma mapa";
    return citatelnyKodMapy(kodMapy(cesta)) || "Neznáma mapa";
}

// ── „Čo padlo" (TLI 25) ───────────────────────────────────────────────────
// Mená položiek z toho istého balíka, aký používa záložka Tracker
// (`polozky-seed-en.json`, pôvod v NOTICE.md). Panel na mene položky nesmie
// stáť — keď sa balík nenačíta alebo id v ňom nie je, ukáže sa aspoň
// „neznáma položka #115".
let MENA_POLOZIEK = window.__MENA_POLOZIEK__ || {};
if (typeof fetch === "function") {
    fetch("polozky-seed-en.json")
        .then((o) => (o.ok ? o.json() : null))
        .then((d) => { if (d && d.polozky) MENA_POLOZIEK = d.polozky; })
        .catch(() => {});
}

// TLI 37 — Richard 18.10.2026: mini-panel ukazoval dropy ako holé `#115`,
// `#815`, `#3709`, `#3909`. Nebola to chyba načítania balíka: tie id v ňom
// naozaj NIE SÚ (balík má 1 933 položiek a ani jedno z nich; overené aj proti
// zoznamom FurTorch a TITrack zo zdieľaného disku — tam tiež nie sú). Kým
// sa meno nenájde, appka aspoň povie, že ide o neznámu položku, a nenechá
// hráča hádať, čo znamená `#115`.
function nazovPolozky(id) {
    const meno = MENA_POLOZIEK[String(id)];
    return meno || `neznáma položka #${id}`;
}

// Ikony položiek (TLI 53 bod B). Balík `polozky-ikony.json` mapuje id na
// LOKÁLNY súbor v `ui/ikony/polozky/` — appke sa teda sťahuje naraz s ňou
// (build time), nie za behu z nášho servera. Doterajšia verzia (TLI 48 bod
// 3) ťahala `<img src="${ZAKLAD_IKON}/ikony/polozka/${id}">` z iného pôvodu
// než appka sama, čo CSP `img-src 'self' data:` ticho blokovala — ikony sa
// preto v tomto paneli nikdy nezobrazili, `onerror` len schoval prázdny
// obrázok. `window.__IKONY_POLOZIEK__` podsúva náhľad pre CI.
let IKONY_POLOZIEK = window.__IKONY_POLOZIEK__ || {};
if (typeof fetch === "function") {
    fetch("polozky-ikony.json")
        .then((o) => (o.ok ? o.json() : null))
        .then((d) => { if (d && d.ikony) IKONY_POLOZIEK = d.ikony; })
        .catch(() => {});
}

function vykresliDropy(zivo, bezi) {
    const obal = document.getElementById("mini-dropy");
    const zoznam = document.getElementById("mini-dropy-zoznam");
    const popis = document.getElementById("mini-dropy-popis");
    if (!obal || !zoznam) return;

    const dropy = (zivo && zivo.top_dropy) || [];
    if (dropy.length === 0) {
        obal.classList.add("skryte");
        zoznam.textContent = "";
        return;
    }
    popis.textContent = bezi ? "čo padlo" : "čo padlo (posledná mapa)";
    zoznam.textContent = "";
    dropy.forEach((d) => {
        const li = document.createElement("li");

        // Ikona ide DOVNÚTRA bunky s menom, nie do vlastného stĺpca mriežky:
        // keď appka pre dané id ikonu nemá, prázdny stĺpec by rozhodil
        // zarovnanie ostatných riadkov.
        const meno = document.createElement("span");
        meno.className = "meno";
        const cestaIkony = IKONY_POLOZIEK[String(d.polozka)];
        if (cestaIkony) {
            const img = document.createElement("img");
            img.className = "ikona";
            img.loading = "lazy";
            img.alt = "";
            img.src = cestaIkony;
            meno.appendChild(img);
        }
        const text = document.createElement("span");
        text.className = "meno-text";
        text.textContent = nazovPolozky(d.polozka);
        meno.appendChild(text);
        li.appendChild(meno);

        const pocet = document.createElement("span");
        pocet.className = "pocet";
        pocet.textContent = "×" + d.pocet;
        li.appendChild(pocet);

        const hodnota = document.createElement("span");
        // `null` = položku nemáme v cenníku. To NIE JE nula a panel to musí
        // rozlíšiť, inak by hráč videl „0 FE" pri veci, ktorá môže byť drahá.
        if (typeof d.hodnota === "number") {
            hodnota.className = "hodnota";
            hodnota.textContent = fe(d.hodnota);
            // TLI 43: cena z titracku dostane atribúciu ako tooltip, nie ako
            // viditeľný text — panel je úzky (viď komentár vyššie pri FE/h)
            // a tri riadky s dlhým textom by sa doň nezmestili.
            // TLI 49 bod 2: appka pozná vek KAŽDEJ ceny, nielen titrackovej —
            // vlastná/partiová cena preto dostane kratší tooltip „cena pred
            // X min", nie žiadny.
            if (d.zdroj === "titrack.ninja") {
                const prisp = typeof d.prispievatelia === "number" ? d.prispievatelia : "?";
                const vek = d.vek || "";
                hodnota.title = `${d.zdroj} · ${prisp} prisp.${vek ? " · " + vek : ""}`;
            } else if (d.vek) {
                hodnota.title = `cena ${d.vek}`;
            }
        } else {
            hodnota.className = "hodnota bez-ceny";
            hodnota.textContent = "bez ceny";
        }
        li.appendChild(hodnota);

        zoznam.appendChild(li);
    });
    obal.classList.remove("skryte");
}

// TLI 55 — živé tikanie ČASU/FE-h PO SEKUNDÁCH (presne ako TITrack), nielen
// pri každom 2s polli backendu. `obnov()` nižšie nastaví/zmaže `zivaZakladna`
// pri každom fetchi; `tiknutie()` medzitým dopočíta uplynuté ms z vlastného
// `Date.now()` bez toho, aby čakala na ďalší `invoke`.
let zivaZakladna = null; // { cistyZisk, trvanieMsVZakladni, ziskanaO } | null

function vykresliHlavneCisla(trvanieMs, cistyZisk) {
    const ziskEl = document.getElementById("mini-zisk");
    const casEl = document.getElementById("mini-cas");
    const zahEl = document.getElementById("mini-zah");

    ziskEl.textContent = fe(cistyZisk);
    if (cistyZisk > 0) {
        ziskEl.className = "cislo zisk-kladny";
    } else if (cistyZisk < 0) {
        ziskEl.className = "cislo zisk-zaporny";
    } else {
        ziskEl.className = "cislo";
    }

    casEl.textContent = cas(trvanieMs);

    // Vypocet FE za hodinu. Pripona "FE" sa tu UZ NEPISE — popisok nad
    // cislom je "FE/h" a tri hodnoty s priponou sa do 260 px nezmestia.
    if (trvanieMs > 0) {
        const ziskZaHodinu = (cistyZisk * 3600000) / trvanieMs;
        zahEl.textContent = fe(ziskZaHodinu).replace(" FE", "");
    } else {
        zahEl.textContent = "—";
    }
}

// Dotikne čas/FE-h medzi dvoma pollami backendu — volá sa každú sekundu.
function tiknutie() {
    if (!zivaZakladna) return;
    const uplynuteMs = Date.now() - zivaZakladna.ziskanaO;
    const trvanieMs = zivaZakladna.trvanieMsVZakladni + Math.max(0, uplynuteMs);
    vykresliHlavneCisla(trvanieMs, zivaZakladna.cistyZisk);
}

// TLI 55 bod 2 — session (od posledného „Reset Stats"): rovnaké čísla ako
// RUNS/TOTAL TIME/FE-HOUR na hlavnom dashboarde (`Suhrn.celkom` z
// `core/src/tracker/agregacie.rs`), len v mini-paneli nad hrou, presne ako to
// ukazuje TITrack overlay. Nezávislé od toho, či práve beží mapa.
function vykresliSession(p) {
    const ziskEl = document.getElementById("mini-session-zisk");
    const casEl = document.getElementById("mini-session-cas");
    const zahEl = document.getElementById("mini-session-zah");
    const celkom = p && p.prehlad && p.prehlad.celkom;

    if (!celkom || celkom.pocet_map === 0) {
        ziskEl.textContent = "—";
        casEl.textContent = "—";
        zahEl.textContent = "—";
        return;
    }

    ziskEl.textContent = fe(celkom.cisty_zisk);
    casEl.textContent = cas(celkom.cas_ms);
    zahEl.textContent = celkom.cas_ms > 0
        ? fe((celkom.cisty_zisk * 3600000) / celkom.cas_ms).replace(" FE", "")
        : "—";
}

// Stiahne udaje z backandu a prekresli panel.
async function obnov() {
    try {
        const p = await invoke("tracker_prehlad");

        const panel = document.getElementById("mini");
        const mapaEl = document.getElementById("mini-mapa");

        vykresliSession(p);

        // Ked nebezi mapa — čísla ZOSTÁVAJÚ z POSLEDNEJ mapy, nie prázdne
        // pomlčky (TLI 55 bod 1: „po skončení mapy ostane finálny čas
        // poslednej mapy"), rovnaký princíp ako „čo padlo" nižšie (TLI 25):
        // hráč sa práve vrátil do hubu a chce vidieť, ako mapa dopadla.
        if (!p || !p.zivo) {
            zivaZakladna = null;
            panel.classList.add("necinny");
            mapaEl.textContent = "Žiadna mapa nebeží";
            const posledna = p && p.posledna_mapa;
            if (posledna) {
                vykresliHlavneCisla(posledna.trvanie_ms, posledna.cisty_zisk);
            } else {
                // Appka ešte NIKDY nevidela dokončenú mapu (čerstvá inštalácia) —
                // „0 FE"/„0 s" by tvrdilo niečo, čo appka nevie, rovnaký rozdiel
                // ako „bez ceny" vs. „0 FE" pri dropoch vyššie.
                document.getElementById("mini-zisk").textContent = "—";
                document.getElementById("mini-zisk").className = "cislo";
                document.getElementById("mini-cas").textContent = "—";
                document.getElementById("mini-zah").textContent = "—";
            }
            vykresliDropy(posledna, false);
            return;
        }

        // Ked mapa bezi, aktivujeme vzhlad a vyplnime data
        panel.classList.remove("necinny");

        const zivo = p.zivo;
        mapaEl.textContent = nazovMapy(zivo.zona);
        zivaZakladna = { cistyZisk: zivo.cisty_zisk, trvanieMsVZakladni: zivo.trvanie_ms, ziskanaO: Date.now() };
        vykresliHlavneCisla(zivo.trvanie_ms, zivo.cisty_zisk);

        vykresliDropy(zivo, true);

    } catch (chyba) {
        // Zachytavame a logujeme chyby bez padu skriptu, aby sa dalsi interval vykonal
        console.error("Chyba pri stahovani dat do mini-panela:", chyba);
    }
}

// Spustenie hned po natiahnuti stranky a cyklenie kazde 2 sekundy — to je
// zalozka, zive citanie na pozadi (TLI 11c) posle udalost hned po novych
// riadkoch a tá nizsie spusti prekreslenie okamzite.
document.addEventListener("DOMContentLoaded", () => {
    obnov();
    setInterval(obnov, 2000);
    // TLI 55: samostatný 1s ticker na ČAS/FE-h medzi dvoma pollami vyššie —
    // `obnov()` ostáva na 2s (nezvyšuje sa frekvencia IPC volaní do Rustu).
    setInterval(tiknutie, 1000);
    if (window.__TAURI__.event) {
        window.__TAURI__.event.listen("tracker-zmena", () => obnov());
    }

    // TLI 21 — posúvanie/zmena veľkosti tohto okna v režime „Upraviť rozloženie".
    invoke("nacitaj_nastavenia")
        .then((n) => {
            nastavPriehladnost(n && n.rozlozenie && n.rozlozenie.priehladnost);
        })
        .catch(() => {});

    if (window.__TAURI__.event) {
        window.__TAURI__.event.listen("rezim-upravy", (udalost) => {
            const p = udalost.payload || {};
            document.body.classList.toggle("rezim-upravy", !!p.zapnute);
            nastavPriehladnost(p.priehladnost);
        });
        window.__TAURI__.event.listen("priehladnost", (udalost) => {
            nastavPriehladnost((udalost.payload || {}).priehladnost);
        });
    }

    if (window.__TAURI__.window) {
        const okno = window.__TAURI__.window.getCurrentWindow();
        const uchytPresun = document.getElementById("mini-uchyt-presun");
        const uchytVelkost = document.getElementById("mini-uchyt-velkost");
        if (uchytPresun) {
            // TLI 48 bod 2: pásik je ťahateľný AJ MIMO režimu úpravy (CSS vyššie
            // mu dovoľuje `pointer-events` vždy) — appka preto novú polohu musí
            // uložiť SAMA hneď po presune, nie až cez `rezim_upravy_ulozit_panel`
            // (ten beží len pri Ctrl+F8 Esc/Enter). `startDragging()` sa vracia
            // (resolvne), až keď hráč pustí tlačidlo — presne vtedy je nová
            // poloha konečná.
            uchytPresun.addEventListener("mousedown", () => {
                okno.startDragging()
                    .then(() => invoke("mini_ulozit_polohu"))
                    .catch((e) => console.error("Presun mini-panela zlyhal:", e));
            });
        }
        if (uchytVelkost) {
            // TLI 50: `startResizeDragging` doteraz resolvol bez toho, aby appka
            // novú veľkosť čokoľvek uložila — po reštarte appky sa mini-panel
            // vrátil na pôvodné rozmery, akoby sa zmena veľkosti vôbec nestala.
            // `mini_ulozit_polohu` ukladá polohu AJ veľkosť naraz (číta
            // `okno.outer_size()` čerstvo), rovnaký príkaz ako pri presune.
            uchytVelkost.addEventListener("mousedown", () => {
                okno.startResizeDragging("SouthEast")
                    .then(() => invoke("mini_ulozit_polohu"))
                    .catch((e) => console.error("Zmena veľkosti mini-panela zlyhala:", e));
            });
        }
    }
});

function nastavPriehladnost(hodnota) {
    if (typeof hodnota === "number" && isFinite(hodnota)) {
        document.documentElement.style.setProperty("--panel-opacity", String(hodnota));
    }
}
