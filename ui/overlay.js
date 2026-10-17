// Získame nástroje Tauri (musí byť withGlobalTauri: true)
const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;

// Referencie na DOM prvky
const elIndikator = document.getElementById("indikator");
const elIndikatorText = document.getElementById("indikator-text");
const elChyba = document.getElementById("chyba");
const elVrstva = document.getElementById("vrstva");
const elPanel = document.getElementById("panel");
const elSipka = document.getElementById("sipka");
const elSipkaCiara = document.getElementById("sipka-ciara");
const elUchytPresun = document.getElementById("panel-uchyt-presun");
const elUchytVelkost = document.getElementById("panel-uchyt-velkost");

const pNadpis = document.getElementById("panel-nadpis");
const pIstota = document.getElementById("panel-istota");
const pDovod = document.getElementById("panel-dovod");
const pIkony = document.getElementById("panel-ikony");
const pSkontroluj = document.getElementById("panel-skontroluj");
const pAlternativy = document.getElementById("panel-alternativy");

// Resetovanie zobrazenia všetkých okien
function skryVsetko() {
    elIndikator.classList.add("skryte");
    elChyba.classList.add("skryte");
    elPanel.classList.add("skryte");
    elSipka.classList.add("skryte");
    elVrstva.innerHTML = ""; // Vymažeme staré obdĺžniky
}

// Počúvame udalosti zo servera (Tauri okno zasiela eventy 'stav')
listen("stav", (event) => {
    const payload = event.payload;
    skryVsetko();

    // TLI 24 (`2026-10-17-1238-mcp.md` bod 3): VŽDY viditeľná spätná väzba —
    // "Snímam…" hneď po stlačení F8/F9 (okno hry nájdené, snímka urobená),
    // "Odoslané, čakám na radu…" len čo appka pošle HTTP požiadavku. Predtým
    // bola jediná fáza "analyzujem" pre oba kroky naraz.
    if (payload.faza === "analyzujem") {
        elIndikatorText.textContent = "Snímam…";
        elIndikator.classList.remove("skryte");
    }
    else if (payload.faza === "odosielam") {
        elIndikatorText.textContent = "Odoslané, čakám na radu…";
        elIndikator.classList.remove("skryte");
    }
    else if (payload.faza === "chyba") {
        // VŽDY používame textContent pre ochranu pred XSS. 
        // Sprava môže prísť z neovereného zdroja alebo z jazykového modelu.
        elChyba.textContent = payload.sprava;
        elChyba.classList.remove("skryte");
    } 
    else if (payload.faza === "hotovo") {
        vykresliHotovo(payload);
    }
    else if (payload.faza === "hotovo_canvas") {
        vykresliHotovoCanvas(payload);
    }
    else if (payload.faza === "zber_ulozene") {
        vykresliZberUlozene(payload);
    }
});

// ── TLI 21 — uložená poloha/veľkosť/priehľadnosť panela (mimo režimu úpravy).
// `poloha` je `null`, keď preň nie je nič uložené — vtedy sa vráti na pôvodné
// CSS hodnoty (24px od rohu, auto výška) odstránením inline štýlu.
listen("panel-poloha", (event) => {
    const p = event.payload || {};
    nastavPriehladnost(p.priehladnost);
    if (p.poloha) {
        aplikujPolohuPanela(p.poloha);
    } else {
        vymazPolohuPanela();
    }
});

listen("priehladnost", (event) => {
    nastavPriehladnost((event.payload || {}).priehladnost);
});

function nastavPriehladnost(hodnota) {
    if (typeof hodnota === "number" && isFinite(hodnota)) {
        document.documentElement.style.setProperty("--panel-opacity", String(hodnota));
    }
}

function aplikujPolohuPanela(r) {
    elPanel.style.left = r.x + "px";
    elPanel.style.top = r.y + "px";
    elPanel.style.width = r.w + "px";
    elPanel.style.height = r.h + "px";
    document.documentElement.style.setProperty("--panel-maxwidth", "none");
}

function vymazPolohuPanela() {
    elPanel.style.removeProperty("left");
    elPanel.style.removeProperty("top");
    elPanel.style.removeProperty("width");
    elPanel.style.removeProperty("height");
    document.documentElement.style.removeProperty("--panel-maxwidth");
}

// ── TLI 21 — režim „Upraviť rozloženie“ ─────────────────────────────────────
// Kým je zapnutý, overlay prestane byť priechodný pre myš (rieši appka na
// úrovni OS) a panel sa dá ťahať/zväčšovať priamo tu v DOM-e — Rust pozná len
// okno hry a monitor, živú polohu panela počas úpravy drží výhradne JS.
let rezimUpravy = false;
let rezimOkno = { sirka: 1920, vyska: 1080 };

listen("rezim-upravy", (event) => {
    const p = event.payload || {};
    rezimUpravy = !!p.zapnute;
    document.body.classList.toggle("rezim-upravy", rezimUpravy);

    if (!rezimUpravy) {
        return;
    }

    skryVsetko();
    if (p.okno) {
        rezimOkno = p.okno;
    }
    nastavPriehladnost(p.priehladnost);
    if (p.panel) {
        aplikujPolohuPanela(p.panel);
    }

    pNadpis.textContent = "Upraviť rozloženie";
    pIstota.textContent = "";
    pDovod.textContent = "Ťahaj za pásik hore — presunieš panel. Ťahaj za pravý dolný roh — " +
        "zmeníš veľkosť. Enter alebo Esc uloží a vráti hru.";
    if (pIkony) pIkony.classList.add("skryte");
    pSkontroluj.classList.add("skryte");
    pAlternativy.classList.add("skryte");
    elPanel.classList.remove("skryte");
});

// Rust požiada o nahlásenie živej polohy tesne pred uložením (Esc/Enter alebo
// tlačidlo „Hotovo“ v Nastaveniach) — JS je jediné miesto, ktoré ju pozná.
listen("rezim-upravy-nahlas", () => {
    const r = elPanel.getBoundingClientRect();
    invoke("rezim_upravy_ulozit_panel", { x: r.left, y: r.top, w: r.width, h: r.height })
        .catch((e) => console.error("Uloženie rozloženia zlyhalo:", e));
});

let tahaniePresunu = null;
let tahanieVelkosti = null;

elUchytPresun.addEventListener("mousedown", (e) => {
    if (!rezimUpravy) return;
    const r = elPanel.getBoundingClientRect();
    tahaniePresunu = { dx: e.clientX - r.left, dy: e.clientY - r.top };
    e.preventDefault();
});

elUchytVelkost.addEventListener("mousedown", (e) => {
    if (!rezimUpravy) return;
    const r = elPanel.getBoundingClientRect();
    tahanieVelkosti = { sirka: r.width, vyska: r.height, px: e.clientX, py: e.clientY };
    e.preventDefault();
});

window.addEventListener("mousemove", (e) => {
    if (tahaniePresunu) {
        const sirkaOkna = rezimOkno.sirka || window.innerWidth;
        const vyskaOkna = rezimOkno.vyska || window.innerHeight;
        let x = e.clientX - tahaniePresunu.dx;
        let y = e.clientY - tahaniePresunu.dy;
        x = Math.max(0, Math.min(x, sirkaOkna - elPanel.offsetWidth));
        y = Math.max(0, Math.min(y, vyskaOkna - elPanel.offsetHeight));
        elPanel.style.left = x + "px";
        elPanel.style.top = y + "px";
    } else if (tahanieVelkosti) {
        const MIN_SIRKA = 220;
        const MIN_VYSKA = 90;
        const sirkaOkna = rezimOkno.sirka || window.innerWidth;
        const vyskaOkna = rezimOkno.vyska || window.innerHeight;
        const r = elPanel.getBoundingClientRect();
        let w = tahanieVelkosti.sirka + (e.clientX - tahanieVelkosti.px);
        let h = tahanieVelkosti.vyska + (e.clientY - tahanieVelkosti.py);
        w = Math.max(MIN_SIRKA, Math.min(w, sirkaOkna - r.left));
        h = Math.max(MIN_VYSKA, Math.min(h, vyskaOkna - r.top));
        elPanel.style.width = w + "px";
        elPanel.style.height = h + "px";
    }
});

window.addEventListener("mouseup", () => {
    tahaniePresunu = null;
    tahanieVelkosti = null;
});

// ── Zber dát (TLI 4c) — žiadna rada, len potvrdenie že sa snímka odoslala
// a server ju uložil (robí to vždy, bez ohľadu na istotu solvera).
function vykresliZberUlozene(data) {
    pNadpis.textContent = `Zber dát — uložené #${data.pocet}`;
    pDovod.textContent = "Snímka odoslaná a uložená na serveri. Over F9 pred aj po "
        + "každom položení paintu.";
    pIstota.textContent = "";
    pSkontroluj.classList.add("skryte");
    pAlternativy.classList.add("skryte");
    elPanel.classList.remove("skryte");
}

/* TLI 8 — riadok s ikonami nad dôvodom: koho sa odporúčanie týka
   (väzeň/paint) a akú akciu s ním urobiť. Keď sa ani jedna ikona nedá
   zostaviť (neznámy väzeň, chýbajúci typ akcie), riadok ostane skrytý —
   panel sa kvôli tomu nikdy nerozbije. */
function vykresliIkonyPanela(ikony) {
    if (!pIkony) return;
    pIkony.innerHTML = "";
    const platne = (ikony || []).filter((x) => x);
    if (!platne.length) {
        pIkony.classList.add("skryte");
        return;
    }
    platne.forEach((el, i) => {
        if (i > 0) {
            const sipka = document.createElement("span");
            sipka.className = "panel-ikona-sipka";
            sipka.textContent = "→";
            pIkony.appendChild(sipka);
        }
        pIkony.appendChild(el);
    });
    pIkony.classList.remove("skryte");
}


function vykresliHotovo(data) {
    const analyza = data.analyza;
    if (!analyza) return;

    // Server môže vrátiť odpoveď bez odporúčania (žiadny legálny ťah) — vtedy
    // sa kreslia len políčka a panel povie, že sa ťah nenašiel.
    const odporucanie = analyza.odporucanie || null;
    const ciel = (odporucanie && odporucanie.ciel !== null && odporucanie.ciel !== undefined)
        ? odporucanie.ciel : null;
    
    // --- 1. Vykreslenie políčok ---
    let cielX = null;
    let cielY = null;

    data.obdlzniky.forEach((obd) => {
        const div = document.createElement("div");
        div.className = "pole";
        div.style.left = obd.x + "px";
        div.style.top = obd.y + "px";
        div.style.width = obd.w + "px";
        div.style.height = obd.h + "px";

        if (obd.neiste) {
            div.classList.add("neiste");
        }

        // Ak je toto políčko cieľom nášho odporúčania, zvýrazníme ho
        if (ciel !== null && obd.index === ciel) {
            div.classList.add("ciel");
            cielX = obd.stredX;
            cielY = obd.stredY;
        }

        elVrstva.appendChild(div);
    });

    // --- 2. Naplnenie textov v paneli (VŽDY textContent!) ---
    if (!odporucanie) {
        pNadpis.textContent = "Ťah sa nenašiel";
    } else if (ciel !== null) {
        pNadpis.textContent = `${odporucanie.nazov} → políčko ${ciel}`;
    } else {
        pNadpis.textContent = odporucanie.nazov;
    }
    
    const istotaPercenta = Math.round((analyza.istota || 0) * 100);
    pIstota.textContent = `Istota ťahu: ${istotaPercenta} %`;
    if (istotaPercenta < 50) {
        pIstota.classList.add("nizka");
    } else {
        pIstota.classList.remove("nizka");
    }

    pDovod.textContent = odporucanie
        ? odporucanie.dovod
        : "Na tejto doske som nenašiel žiadny legálny ťah.";

    // Ikony: väzeň na cieľovom políčku (keď ho poznáme) a cieľová akcia.
    if (odporucanie && window.TLIIkony) {
        const bunky = (analyza.board && analyza.board.cells) || [];
        const bunka = ciel !== null ? bunky.find((c) => c && c.index === ciel) : null;
        vykresliIkonyPanela([
            bunka ? window.TLIIkony.ikonaVazna(bunka.inmate) : null,
            window.TLIIkony.ikonaAkcie(odporucanie.akcia, odporucanie.nazov),
        ]);
    } else {
        vykresliIkonyPanela([]);
    }

    // Neistý zápis dosky
    if (analyza.skontroluj && analyza.skontroluj.length > 0) {
        let textSkontrolovat = "Neistý zápis dosky: ";
        const zoznam = analyza.skontroluj.slice(0, 6);
        textSkontrolovat += zoznam.join(", ");
        if (analyza.skontroluj.length > 6) {
            textSkontrolovat += ` (+${analyza.skontroluj.length - 6} ďalších)`;
        }
        pSkontroluj.textContent = textSkontrolovat;
        pSkontroluj.classList.remove("skryte");
    } else {
        pSkontroluj.classList.add("skryte");
    }

    // Alternatívy
    pAlternativy.innerHTML = ""; // Bezpečné vymazanie
    if (analyza.alternativy && analyza.alternativy.length > 0) {
        const altZoznam = analyza.alternativy.slice(0, 2); // Max 2 alternatívy
        altZoznam.forEach(alt => {
            const div = document.createElement("div");
            div.className = "alternativa";
            
            const spanNazov = document.createElement("span");
            spanNazov.className = "alt-nazov";
            spanNazov.textContent = alt.nazov;
            
            const spanDovod = document.createElement("span");
            const hodnota = (typeof alt.hodnota === "number")
                ? ` (${alt.hodnota.toFixed(2)})` : "";
            spanDovod.textContent = `${hodnota} — ${alt.dovod || ""}`;
            
            div.appendChild(spanNazov);
            div.appendChild(spanDovod);
            pAlternativy.appendChild(div);
        });
        pAlternativy.classList.remove("skryte");
    } else {
        pAlternativy.classList.add("skryte");
    }

    // Zobrazíme panel, aby sme vedeli vypočítať jeho pozíciu pre šípku
    elPanel.classList.remove("skryte");

    // --- 3. Vykreslenie šípky ---
    if (cielX !== null && cielY !== null) {
        // Počkáme na vykreslenie prehliadačom, aby bounding box bol aktuálny
        requestAnimationFrame(() => {
            const rect = elPanel.getBoundingClientRect();
            // Šípku potiahneme z pravého dolného rohu panela
            const startX = rect.right;
            const startY = rect.bottom - (rect.height / 4);

            elSipkaCiara.setAttribute("x1", startX);
            elSipkaCiara.setAttribute("y1", startY);
            elSipkaCiara.setAttribute("x2", cielX);
            elSipkaCiara.setAttribute("y2", cielY);
            elSipka.classList.remove("skryte");
        });
    }
}

// ── Frozen Canvas (TLI 4b) — sekvencia krokov ČO (zdroj v ruke) → KAM (cieľ
// na plátne), očíslovaná zelenými rámikmi. Žiadna šípka — pri viacerých
// krokoch naraz by sa šípky prekrývali, číslo v rohu stačí.
const ZELENA_KROK = "#36e05f";

function vykresliHotovoCanvas(data) {
    const kroky = data.kroky || [];
    const analyza = data.analyza || {};

    if (kroky.length === 0) {
        pNadpis.textContent = "Frozen Canvas — žiadny krok";
        pDovod.textContent = (analyza.skontroluj && analyza.skontroluj[0]) ||
            "Server nenavrhol žiadny ťah (cieľ môže byť už splnený, alebo smer miešania ešte nemá overené pravidlo).";
        pIstota.textContent = "";
        pSkontroluj.classList.add("skryte");
        pAlternativy.classList.add("skryte");
        elPanel.classList.remove("skryte");
        return;
    }

    // --- 1. Dva rámiky na krok: zdroj (ruka) + cieľ (plátno), spárované
    // ROVNAKÝM číslom aj ROVNAKOU opacitou/hrúbkou — prvý krok plný a hrubý,
    // ďalšie tenšie a priehľadnejšie, nech je poradie na prvý pohľad jasné.
    kroky.forEach((krok) => {
        const prvy = krok.poradie === 1;
        const opacita = prvy ? 1 : Math.max(0.4, 1 - (krok.poradie - 1) * 0.2);
        [krok.zdroj, krok.ciel].forEach((box) => {
            if (!box || box.w <= 0 || box.h <= 0) return;
            const div = document.createElement("div");
            div.className = "pole krok-box" + (prvy ? " krok-prvy" : "");
            div.style.left = box.x + "px";
            div.style.top = box.y + "px";
            div.style.width = box.w + "px";
            div.style.height = box.h + "px";
            div.style.borderColor = ZELENA_KROK;
            div.style.opacity = String(opacita);
            div.style.boxShadow = `inset 0 0 12px rgba(54, 224, 95, ${0.4 * opacita})`;

            const cislo = document.createElement("span");
            cislo.className = "krok-cislo";
            cislo.textContent = String(krok.poradie);
            div.appendChild(cislo);

            elVrstva.appendChild(div);
        });
    });

    // --- 2. Panel: prvý krok v hlavnom texte, ďalšie pod ním ako zoznam ---
    // TLI 4c: keď smer aktívneho cieľa ešte nemá dosť potvrdených párov
    // pred/po (canvas.yml: prah_overenia_parov), server to pošle v
    // `pravidlo_overene: false` — rada sa aj tak zobrazí (solver implementuje
    // len overené smery), ale nadpis to musí priznať, nech ju hráč neberie
    // ako isté pravidlo.
    const neoverene = analyza.pravidlo_overene === false;
    const prefixNadpisu = neoverene ? "⚠️ " : "";
    pNadpis.textContent = prefixNadpisu + (kroky.length === 1
        ? "Frozen Canvas — 1 krok"
        : `Frozen Canvas — ${kroky.length} krokov`);

    const istotaPercenta = Math.round((analyza.istota || 0) * 100);
    pIstota.textContent = `Istota: ${istotaPercenta} %`;
    pIstota.classList.toggle("nizka", istotaPercenta < 50);

    pDovod.textContent = kroky[0].dovod || "";

    // Ikony: paint, ktorý sa berie z ruky, a akcia „polož na plátno".
    // POZOR: `kroky` sú už prepočítané na CSS pixely a index slotu v nich nie
    // je — ten ostáva v pôvodnej `analyza.kroky` (spárované cez `poradie`).
    if (window.TLIIkony) {
        const surove = (analyza.kroky || []).find((k) => k && k.poradie === kroky[0].poradie);
        const zdroj = (surove && surove.zdroj) || {};
        const ruka = (analyza.platno && analyza.platno.ruka) || [];
        const slot = ruka.find((s) => s && s.index === zdroj.index);
        vykresliIkonyPanela([
            slot ? window.TLIIkony.ikonaPaintu(slot.paint) : null,
            window.TLIIkony.ikonaAkcie("polozit", "Polož na plátno"),
        ]);
    }

    pAlternativy.innerHTML = "";
    if (kroky.length > 1) {
        kroky.slice(1).forEach((krok) => {
            const div = document.createElement("div");
            div.className = "alternativa";
            const spanNazov = document.createElement("span");
            spanNazov.className = "alt-nazov";
            spanNazov.textContent = `Krok ${krok.poradie}`;
            const spanDovod = document.createElement("span");
            spanDovod.textContent = " — " + (krok.dovod || "");
            div.appendChild(spanNazov);
            div.appendChild(spanDovod);
            pAlternativy.appendChild(div);
        });
        pAlternativy.classList.remove("skryte");
    } else {
        pAlternativy.classList.add("skryte");
    }

    const sk = analyza.skontroluj || [];
    if (sk.length > 0) {
        const zoznam = sk.slice(0, 6);
        pSkontroluj.textContent = "Neisté: " + zoznam.join(", ") +
            (sk.length > 6 ? ` (+${sk.length - 6} ďalších)` : "");
        pSkontroluj.classList.remove("skryte");
    } else {
        pSkontroluj.classList.add("skryte");
    }

    elPanel.classList.remove("skryte");
}
