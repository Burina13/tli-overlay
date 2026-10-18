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
const elRezimLista = document.getElementById("rezim-lista");

const pNadpis = document.getElementById("panel-nadpis");
const pIstota = document.getElementById("panel-istota");
const pDovod = document.getElementById("panel-dovod");
const pIkony = document.getElementById("panel-ikony");
const pSkontroluj = document.getElementById("panel-skontroluj");
const pAlternativy = document.getElementById("panel-alternativy");

// Resetovanie zobrazenia všetkých okien
// TLI 36 bod 4 — cieľová latencia je do 8 s, ale pri neznámej obrazovke sa
// model volá DVA razy (rozpoznanie + poradca) a to sa občas natiahne. Aby
// hráč nemal pocit, že appka zamrzla, text indikátora sa po `PRAH_PRACUJEM_MS`
// zmení z „Odoslané, čakám na radu…" na „Pracujem…". Je to JEDINE text —
// žiadne ďalšie volanie, žiadny timeout, odpoveď sa čaká ďalej.
const PRAH_PRACUJEM_MS = 8000;
let casovacPracujem = null;

function zrusCasovacPracujem() {
    if (casovacPracujem !== null) {
        clearTimeout(casovacPracujem);
        casovacPracujem = null;
    }
}

function skryVsetko() {
    zrusCasovacPracujem();
    elIndikator.classList.add("skryte");
    elChyba.classList.add("skryte");
    elPanel.classList.add("skryte");
    elSipka.classList.add("skryte");
    elVrstva.innerHTML = ""; // Vymažeme staré obdĺžniky
}

// TLI 33 (`2026-10-18-1318-mcp.md`, bod 1) — appka u hráča „nič nezobrazila“
// nad hrou, hoci Rust log ukázal „emitujem stav=hotovo“; z toho sa nedalo
// zistiť, či event vôbec dorazil sem do JS, alebo dorazil a vykreslenie
// zlyhalo/skrylo sa. Obe miesta logujú cez `invoke` späť do Rustu
// (`zapis_ui_log` → `denik::zapis("ui", …)`), teda do TOHO ISTÉHO súboru ako
// `[hotkey]`/`[zobraz]`/`[http]` — žiadny nový log, len nová trasa doň.
// `.catch(() => {})`: zlyhanie zápisu diagnostiky nesmie appke nič rozbiť.
function logUi(text) {
    invoke("zapis_ui_log", { text }).catch(() => {});
}

// Zápis skutočne vykresleného stavu panela — AŽ PO `requestAnimationFrame`,
// nech `getBoundingClientRect()` vráti rozmery PO rozložení prehliadačom
// (rovnaký dôvod, prečo na to čaká aj vykreslenie šípky nižšie), nie hodnoty
// spred vykreslenia, ktoré by vyzerali v poriadku aj keby panel ostal prázdny.
function logPanelVykreslene(zdroj) {
    requestAnimationFrame(() => {
        const r = elPanel.getBoundingClientRect();
        const viditelny = !elPanel.classList.contains("skryte");
        logUi(`vykreslené panel (${zdroj}) x=${Math.round(r.x)} y=${Math.round(r.y)} ` +
              `w=${Math.round(r.width)} h=${Math.round(r.height)} viditelny=${viditelny}`);
    });
}

// Počúvame udalosti zo servera (Tauri okno zasiela eventy 'stav')
listen("stav", (event) => {
    const payload = event.payload;
    logUi(`prijaté stav=${payload.faza}`);
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
        zrusCasovacPracujem();
        casovacPracujem = setTimeout(() => {
            elIndikatorText.textContent = "Pracujem… (neznámu obrazovku rozoberám podrobnejšie)";
            casovacPracujem = null;
        }, PRAH_PRACUJEM_MS);
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
    else if (payload.faza === "hotovo_nastroj") {
        vykresliHotovoNastroj(payload);
    }
    else if (payload.faza === "hotovo_poradca") {
        // TLI 36 — univerzálny poradca. Vedome TÁ ISTÁ funkcia ako pri
        // nástroji a dare: zadanie žiada „rovnaké zobrazenie ako pri známych
        // typoch", nie druhý vzhľad pre neznáme obrazovky.
        vykresliHotovoNastroj(payload);
    }
    else if (payload.faza === "hotovo_poklad") {
        // TLI 35 — rovnaké vykreslenie ako výber nástroja (`vykresliHotovoNastroj`
        // je generické na `analyza.nastroje`/`analyza.poklady`, viď nižšie),
        // len iná obrazovka a iný panel pod ňou.
        vykresliHotovoNastroj(payload);
    }
    else if (payload.faza === "skry") {
        // Všetko už schoval `skryVsetko()` vyššie — appka len povedala, že
        // nad touto obrazovkou nemá čo kresliť (TLI 25, neznámy typ).
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
        if (elRezimLista) {
            elRezimLista.innerHTML = "";
            elRezimLista.classList.add("skryte");
        }
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
    // TLI 25: prepínač, KTORÉMU modulu sa poloha práve nastavuje. Do TLI 25
    // mali F8 doska, F9 Canvas aj Artistry jednu spoločnú polohu, takže sa
    // nedali rozmiestniť po obrazovke zvlášť.
    vykresliPrepinacModulov(p.moduly || [], p.aktivny_modul || "");
    elPanel.classList.remove("skryte");
});

function vykresliPrepinacModulov(moduly, aktivny) {
    pAlternativy.classList.add("skryte");
    if (!elRezimLista) return;
    elRezimLista.innerHTML = "";
    if (moduly.length === 0) {
        elRezimLista.classList.add("skryte");
        return;
    }
    const popis = document.createElement("span");
    popis.className = "prepinac-popis";
    popis.textContent = "Nastavuješ polohu pre:";
    elRezimLista.appendChild(popis);

    const riadok = document.createElement("div");
    riadok.className = "prepinac";
    moduly.forEach((m) => {
        const tlacidlo = document.createElement("button");
        tlacidlo.type = "button";
        tlacidlo.className = "prepinac-tlacidlo" + (m.kluc === aktivny ? " aktivny" : "");
        tlacidlo.textContent = m.popis;
        tlacidlo.addEventListener("click", () => {
            if (!rezimUpravy) return;
            const r = elPanel.getBoundingClientRect();
            invoke("rezim_upravy_prepni_panel",
                   { kluc: m.kluc, x: r.left, y: r.top, w: r.width, h: r.height })
                .then((odpoved) => {
                    if (odpoved && odpoved.panel) aplikujPolohuPanela(odpoved.panel);
                    riadok.querySelectorAll(".prepinac-tlacidlo")
                        .forEach((t) => t.classList.remove("aktivny"));
                    tlacidlo.classList.add("aktivny");
                })
                .catch((e) => console.error("Prepnutie panela zlyhalo:", e));
        });
        riadok.appendChild(tlacidlo);
    });
    elRezimLista.appendChild(riadok);
    elRezimLista.classList.remove("skryte");
}

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
    // TLI 32: keď server vie odporúčanie priradiť ku KONKRÉTNEMU tlačidlu
    // spodnej lišty ("Go to Cell N", "Consume"/"Replace"/"End"...), hráč má
    // kliknúť TAM, nie na celu — cela sa preto v tomto prípade vôbec
    // nezvýrazňuje (žiadna trieda "ciel", žiadna šípka). Keď sa zhoda
    // nenašla (iný panel, neznáma akcia), appka sa správa presne ako
    // predtým TLI 32: zvýrazní celu a nakreslí šípku.
    const maTlacidlo = !!(data.tlacidlo && data.tlacidlo.w > 0 && data.tlacidlo.h > 0);
    const ciel = (!maTlacidlo && odporucanie && odporucanie.ciel !== null && odporucanie.ciel !== undefined)
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
    } else if (maTlacidlo && data.tlacidlo.text) {
        pNadpis.textContent = `${odporucanie.nazov} → tlačidlo „${data.tlacidlo.text}“`;
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

    // --- 2b. Tlačidlo spodnej lišty (TLI 32) — žiara priamo na mieste, kam
    // má hráč kliknúť. Druhé najlepšie (keď je to iné tlačidlo) slabšie,
    // bez popisu — rovnaká konvencia ako pri kartičkách nástrojov.
    vykresliTlacidloAkcie(data, odporucanie);

    // Zobrazíme panel, aby sme vedeli vypočítať jeho pozíciu pre šípku
    elPanel.classList.remove("skryte");
    logPanelVykreslene("doska");

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

// TLI 32 — zvýraznenie tlačidla spodnej lišty ("Go to Cell N",
// "Consume"/"Replace"/"End"...), ktoré server priradil odporúčaniu
// (`data.tlacidlo`) a prípadne prvej alternatíve (`data.tlacidlo_alt`).
// Súradnice prichádzajú UŽ prepočítané na CSS pixely (Rust, `suradnice::box01_na_css`
// — rovnaký prepočet ako pre cely), appka ich len nakreslí.
function vykresliTlacidloAkcie(data, odporucanie) {
    if (data.tlacidlo && data.tlacidlo.w > 0 && data.tlacidlo.h > 0) {
        const div = document.createElement("div");
        div.className = "tlacidlo-akcie";
        div.style.left = data.tlacidlo.x + "px";
        div.style.top = data.tlacidlo.y + "px";
        div.style.width = data.tlacidlo.w + "px";
        div.style.height = data.tlacidlo.h + "px";

        const istotaPercenta = Math.round(((data.analyza && data.analyza.istota) || 0) * 100);
        const popis = document.createElement("div");
        popis.className = "tlacidlo-popis";
        popis.textContent = `${(odporucanie && odporucanie.nazov) || "Odporúčaný ťah"} — ${istotaPercenta} %`;
        div.appendChild(popis);

        elVrstva.appendChild(div);
    }
    if (data.tlacidlo_alt && data.tlacidlo_alt.w > 0 && data.tlacidlo_alt.h > 0) {
        const div = document.createElement("div");
        div.className = "tlacidlo-akcie tlacidlo-alt";
        div.style.left = data.tlacidlo_alt.x + "px";
        div.style.top = data.tlacidlo_alt.y + "px";
        div.style.width = data.tlacidlo_alt.w + "px";
        div.style.height = data.tlacidlo_alt.h + "px";
        elVrstva.appendChild(div);
    }
}

// ── Výber „Torture Instrument" (TLI 25) ────────────────────────────────────
// Nad každou kartičkou rámik s tierom a poradím, odporúčaná zlatá a hrubá.
// Nekreslí sa šípka ani cieľové políčko — hráč nevyberá KAM klikať na doske,
// ale KTORÚ z troch kariet pred sebou si vziať.
const FARBY_TIEROV = {
    S: "#ffce54",   // zlatá — ber takmer vždy
    A: "#a0d468",
    B: "#7fb3d5",
    C: "#b0b4bd",
};
const FARBA_NEZNAMY = "#8a8f9a";

function farbaTieru(tier, neznamy) {
    if (neznamy) return FARBA_NEZNAMY;
    return FARBY_TIEROV[tier] || FARBA_NEZNAMY;
}

function vykresliHotovoNastroj(data) {
    const karticky = data.karticky || [];
    const analyza = data.analyza || {};

    // TLI 35: rovnaké vykreslenie pre výber nástroja aj daru — rozlišuje sa
    // len slovíčkom v texte, polia kartičiek majú rovnaký tvar.
    const jePoklad = analyza.obrazovka === "vyber_pokladu";
    // TLI 36: poradca posiela možnosti v `analyza.moznosti` a má vlastný
    // nadpis — názov obrazovky, ktorý rozpoznal, nech hráč vidí, že appka
    // vie, kde je, aj keď je to obrazovka bez špecializovaného modulu.
    const jePoradca = analyza.obrazovka === "poradca";
    const nazovObrazovky = (analyza.nazov_obrazovky || "").trim() || "Neznáma obrazovka";
    if (karticky.length === 0) {
        pNadpis.textContent = jePoradca
            ? `${nazovObrazovky} — možnosti sa nepodarilo umiestniť`
            : (jePoklad
                ? "Výber pokladu — kartičky sa nepodarilo umiestniť"
                : "Výber nástroja — kartičky sa nepodarilo umiestniť");
        pDovod.textContent = "Server kartičky prečítal, ale nevrátil ich polohu na obrazovke, "
            + "takže ich neviem orámovať. Poradie je v paneli nižšie.";
        pIstota.textContent = "";
        pIkony.classList.add("skryte");
        pSkontroluj.classList.add("skryte");
        vypisKartickyDoPanela(analyza.moznosti || analyza.nastroje || analyza.poklady || []);
        elPanel.classList.remove("skryte");
        logPanelVykreslene(jePoradca ? "poradca-bez-polohy"
            : (jePoklad ? "poklad-bez-polohy" : "nastroj-bez-polohy"));
        return;
    }

    // --- 1. Rámik + štítok nad každou kartičkou ---
    karticky.forEach((k) => {
        const farba = farbaTieru(k.tier, k.neznamy);
        const div = document.createElement("div");
        div.className = "pole karticka" + (k.odporucany ? " karticka-odporucana" : "");
        div.style.left = k.x + "px";
        div.style.top = k.y + "px";
        div.style.width = k.w + "px";
        div.style.height = k.h + "px";
        div.style.borderColor = farba;
        if (k.odporucany) {
            div.style.boxShadow = `inset 0 0 26px ${farba}55, 0 0 14px ${farba}88`;
        }

        const stitok = document.createElement("div");
        stitok.className = "karticka-stitok";
        stitok.style.background = farba;

        const znakTieru = document.createElement("span");
        znakTieru.className = "karticka-tier";
        znakTieru.textContent = k.neznamy ? "?" : k.tier;
        stitok.appendChild(znakTieru);

        const text = document.createElement("span");
        text.className = "karticka-poradie";
        text.textContent = k.neznamy
            ? "nepoznám"
            : (k.odporucany ? `${k.poradie}. — ber túto` : `${k.poradie}.`);
        stitok.appendChild(text);
        div.appendChild(stitok);

        // Dôvod priamo pod štítkom — hráč sa nemusí pozerať do panela.
        const dovod = document.createElement("div");
        dovod.className = "karticka-dovod";
        dovod.textContent = k.dovod || "";
        div.appendChild(dovod);

        elVrstva.appendChild(div);
    });

    // --- 2. Panel: poradie všetkých kartičiek pod sebou ---
    const najlepsia = karticky.find((k) => k.odporucany);
    if (jePoradca) {
        pNadpis.textContent = najlepsia
            ? `${nazovObrazovky} — klikni: ${najlepsia.nazov}`
            : nazovObrazovky;
    } else {
        pNadpis.textContent = najlepsia
            ? `Ber: ${najlepsia.nazov}`
            : "Ani jednu z týchto kartičiek nepoznám";
    }

    const istotaPercenta = Math.round((analyza.istota || 0) * 100);
    // Poradca neráta Monte Carlo nad známym stavom — je to odhad nad
    // pravidlami hry, a tak sa aj hlási. Percento by predstieralo presnosť,
    // ktorú tá rada nemá.
    pIstota.textContent = jePoradca
        ? `Istota rady: ${analyza.istota_slovom || "nízka"} (${istotaPercenta} %) — poradca, nie presný výpočet`
        : `Istota odporúčania: ${istotaPercenta} %`;
    pIstota.classList.toggle("nizka", istotaPercenta < 50);

    pDovod.textContent = najlepsia
        ? ((jePoradca && analyza.odporucanie && analyza.odporucanie.dovod)
            || najlepsia.dovod)
        : (jePoradca
            ? "Na tejto obrazovke som nenašiel, na čo sa dá kliknúť."
            : jePoklad
            ? "Tieto dary nemám v katalógu — rozhodni sa sám a pošli screenshot do #torchlight."
            : "Tieto nástroje nemám v katalógu — rozhodni sa sám a pošli screenshot do #torchlight.");
    pIkony.classList.add("skryte");

    vypisKartickyDoPanela(karticky);

    if (analyza.skontroluj && analyza.skontroluj.length > 0) {
        pSkontroluj.textContent = analyza.skontroluj.slice(0, 3).join("; ");
        pSkontroluj.classList.remove("skryte");
    } else {
        pSkontroluj.classList.add("skryte");
    }
    elPanel.classList.remove("skryte");
    logPanelVykreslene(jePoradca ? "poradca" : (jePoklad ? "poklad" : "nastroj"));
}

// Zoznam pod hlavným textom panela — používa sa aj vtedy, keď sa rámiky
// nedajú nakresliť, preto je to samostatná funkcia.
function vypisKartickyDoPanela(karticky) {
    pAlternativy.innerHTML = "";
    const zoradene = karticky.slice().sort((a, b) => (a.poradie || 99) - (b.poradie || 99));
    zoradene.forEach((k) => {
        const div = document.createElement("div");
        div.className = "alternativa";

        const znak = document.createElement("span");
        znak.className = "alt-tier";
        znak.style.background = farbaTieru(k.tier, k.neznamy);
        znak.textContent = k.neznamy ? "?" : k.tier;
        div.appendChild(znak);

        const nazov = document.createElement("span");
        nazov.className = "alt-nazov";
        nazov.textContent = k.nazov;
        div.appendChild(nazov);

        const pozn = [];
        if (typeof k.skore === "number") pozn.push(`${k.skore.toFixed(2)}×`);
        if (k.malo_dat && !k.neznamy) pozn.push("málo dát");
        if (k.sporne) pozn.push("sporné");
        if (pozn.length > 0) {
            const span = document.createElement("span");
            span.className = "alt-pozn";
            span.textContent = ` (${pozn.join(", ")})`;
            div.appendChild(span);
        }
        pAlternativy.appendChild(div);
    });
    pAlternativy.classList.remove("skryte");
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
        logPanelVykreslene("canvas-bez-kroku");
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
    logPanelVykreslene("canvas");
}
