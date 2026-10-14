// Získame nástroje Tauri (musí byť withGlobalTauri: true)
const { listen } = window.__TAURI__.event;

// Referencie na DOM prvky
const elIndikator = document.getElementById("indikator");
const elChyba = document.getElementById("chyba");
const elVrstva = document.getElementById("vrstva");
const elPanel = document.getElementById("panel");
const elSipka = document.getElementById("sipka");
const elSipkaCiara = document.getElementById("sipka-ciara");

const pNadpis = document.getElementById("panel-nadpis");
const pIstota = document.getElementById("panel-istota");
const pDovod = document.getElementById("panel-dovod");
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

    if (payload.faza === "analyzujem") {
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
});

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
