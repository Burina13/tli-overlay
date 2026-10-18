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

// Ziska cisty nazov zlozky z celej cesty zadanej hrou.
// Hľadá sa posledný kus cesty a následne sa odstráni koncovka.
function analyzujCestuMapy(cesta) {
    if (!cesta) return "Neznáma mapa";
    
    // Ziskanie poslednej casti cesty
    const casti = cesta.split("/");
    const subor = casti[casti.length - 1];
    
    // Odstranenie vsetkeho za poslednou bodkou
    const indexBodky = subor.lastIndexOf(".");
    if (indexBodky > 0) {
        return subor.substring(0, indexBodky);
    }
    
    return subor;
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

// Ikony sa ťahajú z NÁŠHO servera (`/ikony/polozka/<id>`), nie priamo z
// cudzieho CDN: appka hráča tak nikam inam nechodí a obrázok sa sťahuje raz
// za celú partiu. Keď ikona nie je, `onerror` ju schová a ostane len text.
let ZAKLAD_IKON = null;

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
        // keď sa nenačíta (alebo adresa servera nie je známa), prázdny stĺpec
        // by rozhodil zarovnanie ostatných riadkov.
        const meno = document.createElement("span");
        meno.className = "meno";
        if (ZAKLAD_IKON) {
            const img = document.createElement("img");
            img.className = "ikona";
            img.loading = "lazy";
            img.alt = "";
            img.src = ZAKLAD_IKON + "/ikony/polozka/" + d.polozka;
            img.addEventListener("error", () => img.remove());
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
            if (d.zdroj === "titrack.ninja") {
                const prisp = typeof d.prispievatelia === "number" ? d.prispievatelia : "?";
                const vek = d.vek || "";
                hodnota.title = `${d.zdroj} · ${prisp} prisp.${vek ? " · " + vek : ""}`;
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

// Stiahne udaje z backandu a prekresli panel.
async function obnov() {
    try {
        const p = await invoke("tracker_prehlad");
        
        const panel = document.getElementById("mini");
        const mapaEl = document.getElementById("mini-mapa");
        const ziskEl = document.getElementById("mini-zisk");
        const casEl = document.getElementById("mini-cas");
        const zahEl = document.getElementById("mini-zah");

        // Ked nebezi mapa — čísla sú prázdne, ale „čo padlo" z POSLEDNEJ
        // mapy ostáva (TLI 25): hráč sa práve vrátil do hubu a chce vidieť,
        // čo si z mapy odniesol.
        if (!p || !p.zivo) {
            panel.classList.add("necinny");
            mapaEl.textContent = "Žiadna mapa nebeží";
            ziskEl.textContent = "—";
            ziskEl.className = "cislo"; // Zrusime farebne triedy
            casEl.textContent = "—";
            zahEl.textContent = "—";
            vykresliDropy(p && p.posledna_mapa, false);
            return;
        }

        // Ked mapa bezi, aktivujeme vzhlad a vyplnime data
        panel.classList.remove("necinny");
        
        const zivo = p.zivo;
        mapaEl.textContent = analyzujCestuMapy(zivo.zona);
        
        // Nastavenie cisteho zisku vratane farby textu
        ziskEl.textContent = fe(zivo.cisty_zisk);
        if (zivo.cisty_zisk > 0) {
            ziskEl.className = "cislo zisk-kladny";
        } else if (zivo.cisty_zisk < 0) {
            ziskEl.className = "cislo zisk-zaporny";
        } else {
            ziskEl.className = "cislo";
        }

        // Nastavenie ziveho casu
        casEl.textContent = cas(zivo.trvanie_ms);

        // Vypocet FE za hodinu. Pripona "FE" sa tu UZ NEPISE — popisok nad
        // cislom je "FE/h" a tri hodnoty s priponou sa do 260 px nezmestia.
        if (zivo.trvanie_ms > 0) {
            const ziskZaHodinu = (zivo.cisty_zisk * 3600000) / zivo.trvanie_ms;
            zahEl.textContent = fe(ziskZaHodinu).replace(" FE", "");
        } else {
            zahEl.textContent = "—";
        }

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
    if (window.__TAURI__.event) {
        window.__TAURI__.event.listen("tracker-zmena", () => obnov());
    }

    // TLI 21 — posúvanie/zmena veľkosti tohto okna v režime „Upraviť rozloženie".
    invoke("nacitaj_nastavenia")
        .then((n) => {
            nastavPriehladnost(n && n.rozlozenie && n.rozlozenie.priehladnost);
            // Odkiaľ brať ikony položiek (TLI 25) — z nášho servera, nie
            // z cudzieho CDN. Keď sa adresa nenačíta, zoznam dropov sa
            // vykreslí bez obrázkov; text je to podstatné.
            if (n && n.url_servera) {
                ZAKLAD_IKON = String(n.url_servera).replace(/\/+$/, "");
            }
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
            uchytPresun.addEventListener("mousedown", () => {
                okno.startDragging().catch((e) => console.error("Presun mini-panela zlyhal:", e));
            });
        }
        if (uchytVelkost) {
            uchytVelkost.addEventListener("mousedown", () => {
                okno.startResizeDragging("SouthEast")
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
