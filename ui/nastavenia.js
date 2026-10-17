const { invoke } = window.__TAURI__.core;

// Odkazy na formuláre a prvky
const formular = document.getElementById("formular");
const inpUrl = document.getElementById("inp-url");
const inpKluc = document.getElementById("inp-kluc");
const inpHotkey = document.getElementById("inp-hotkey");
const inpHotkeyCanvas = document.getElementById("inp-hotkey-canvas");
const inpHotkeyTrh = document.getElementById("inp-hotkey-trh");
const inpProces = document.getElementById("inp-proces");
const inpAutoskrytie = document.getElementById("inp-autoskrytie");
const inpZvuk = document.getElementById("inp-zvuk");
const inpZberDat = document.getElementById("inp-zber-dat");

const btnUkazatKluc = document.getElementById("btn-ukazat-kluc");
const btnTest = document.getElementById("btn-test");
const elVysledokTestu = document.getElementById("vysledok-testu");
const btnOtvorLog = document.getElementById("btn-otvor-log");
const elVysledokLogu = document.getElementById("vysledok-logu");
const elVysledokUlozenia = document.getElementById("vysledok-ulozenia");

const odkazKluc = document.getElementById("odkaz-kluc");
const odkazSprievodca = document.getElementById("odkaz-sprievodca");
const odkazAnalyzer = document.getElementById("odkaz-analyzer");

const elVerzia = document.getElementById("verzia-text");
const btnAktualizacia = document.getElementById("btn-aktualizacia");
const elVysledokAktualizacie = document.getElementById("vysledok-aktualizacie");
const btnStiahnut = document.getElementById("btn-stiahnut");

let urlAktualizacie = null;

// Posledné načítané nastavenia z `nacitaj_nastavenia` — ulož_nastavenia pri
// odoslaní formulára z nich vychádza a prepíše len polia, ktoré má tento
// formulár (hotkeys, URL, kľúč, proces, autoskrytie, zvuk, zber dát).
// DÔVOD (oprava nájdená pri TLI 22): appka `Nastavenia` má aj polia, ktoré
// tento formulár vôbec nezobrazuje (téma, uložené rozloženie okien z TLI 21,
// prepínače v záložke Tracker — tracker_sync/ceny_sync/upload_logov/
// pripominat_enable_log/ceny_obrazovka_zapnute, cesta k logu...). Keby `nove`
// vznikalo len z polí formulára, `#[serde(default)]` na strane Rustu by
// chýbajúce polia pri uložení potichu vrátilo na predvolené — hráč by
// uložením pripojenia v Nastaveniach prišiel o posunuté panely aj o všetky
// prepínače v Trackeri.
let nacitaneNastavenia = {};

// --- TLI 21 — Rozloženie okien nad hrou ---
const btnRezimUpravy = document.getElementById("btn-rezim-upravy");
const btnObnovPredvolene = document.getElementById("btn-obnov-predvolene");
const inpPriehladnost = document.getElementById("inp-priehladnost");

function nastavTlacidloRezimu(aktivny) {
    btnRezimUpravy.textContent = aktivny ? "Hotovo (uložiť rozloženie)" : "Upraviť rozloženie (Ctrl+F8)";
    btnObnovPredvolene.classList.toggle("skryte", !aktivny);
}

btnRezimUpravy.addEventListener("click", async () => {
    const aktivny = await invoke("rezim_upravy_je_aktivny");
    try {
        if (aktivny) {
            await invoke("rezim_upravy_hotovo");
            nastavTlacidloRezimu(false);
        } else {
            await invoke("rezim_upravy_zapni");
            nastavTlacidloRezimu(true);
        }
    } catch (chyba) {
        console.error("Prepnutie režimu úpravy rozloženia zlyhalo:", chyba);
    }
});

btnObnovPredvolene.addEventListener("click", async () => {
    try {
        await invoke("rezim_upravy_obnov_predvolene");
    } catch (chyba) {
        console.error("Obnovenie predvoleného rozloženia zlyhalo:", chyba);
    }
});

inpPriehladnost.addEventListener("input", async () => {
    try {
        await invoke("uloz_priehladnost", { hodnota: parseFloat(inpPriehladnost.value) });
    } catch (chyba) {
        console.error("Uloženie priehľadnosti zlyhalo:", chyba);
    }
});

// --- Inicializácia ---
document.addEventListener("DOMContentLoaded", async () => {
    try {
        // Načítanie existujúcich nastavení
        const nastavenia = await invoke("nacitaj_nastavenia");
        nacitaneNastavenia = nastavenia;
        inpUrl.value = nastavenia.url_servera || "";
        inpKluc.value = nastavenia.kluc || "";
        inpHotkey.value = nastavenia.hotkey || "F8";
        inpHotkeyCanvas.value = nastavenia.hotkey_canvas || "F9";
        inpHotkeyTrh.value = nastavenia.hotkey_trh || "";
        inpProces.value = nastavenia.proces_hry || "";
        inpAutoskrytie.value = nastavenia.autoskrytie_s || 0;
        inpZvuk.checked = !!nastavenia.zvuk;
        inpZberDat.checked = !!nastavenia.zber_dat_zapnute;
        const priehladnost = nastavenia.rozlozenie && nastavenia.rozlozenie.priehladnost;
        inpPriehladnost.value = typeof priehladnost === "number" ? priehladnost : 1;

        // Keď hráč zavrie Nastavenia uprostred úpravy rozloženia a znova ich
        // otvorí, tlačidlo musí ukázať aktuálny stav, nie vždy „Upraviť".
        nastavTlacidloRezimu(await invoke("rezim_upravy_je_aktivny"));

        // Načítanie verzie
        const verzia = await invoke("verzia");
        elVerzia.textContent = `Verzia ${verzia}`;
    } catch (chyba) {
        console.error("Chyba pri načítaní nastavení:", chyba);
    }
});

// --- Ukázať/skryť heslo ---
btnUkazatKluc.addEventListener("click", () => {
    if (inpKluc.type === "password") {
        inpKluc.type = "text";
        btnUkazatKluc.textContent = "Skryť";
    } else {
        inpKluc.type = "password";
        btnUkazatKluc.textContent = "Ukázať";
    }
});

// --- Validácia a uloženie ---
formular.addEventListener("submit", async (e) => {
    e.preventDefault();
    elVysledokUlozenia.textContent = "";
    elVysledokUlozenia.className = "";

    const url = inpUrl.value.trim();
    const kluc = inpKluc.value.trim();

    // Lokálna validácia bez zásahu servera
    if (!url.startsWith("https://")) {
        zobrazSpravu(elVysledokUlozenia, "Adresa servera musí začínať s https://", false);
        return;
    }
    if (!kluc) {
        zobrazSpravu(elVysledokUlozenia, "Osobný kľúč nesmie byť prázdny.", false);
        return;
    }

    // Vychádza z POSLEDNÝCH načítaných nastavení (`nacitaneNastavenia`), nie z
    // prázdneho objektu — inak by uloženie tohto formulára potichu vrátilo na
    // predvolené všetko, čo tento formulár nezobrazuje (pozri komentár vyššie
    // pri `nacitaneNastavenia`).
    const nove = {
        ...nacitaneNastavenia,
        url_servera: url,
        kluc: kluc,
        hotkey: inpHotkey.value.trim(),
        hotkey_canvas: inpHotkeyCanvas.value.trim(),
        hotkey_trh: inpHotkeyTrh.value.trim(),
        proces_hry: inpProces.value.trim(),
        autoskrytie_s: parseInt(inpAutoskrytie.value, 10) || 0,
        zvuk: inpZvuk.checked,
        zber_dat_zapnute: inpZberDat.checked
    };

    try {
        await invoke("uloz_nastavenia", { nove });
        nacitaneNastavenia = nove;
        zobrazSpravu(elVysledokUlozenia, "Uložené.", true);
        // Po 3 sekundách zhasni text
        setTimeout(() => {
            elVysledokUlozenia.textContent = "";
        }, 3000);
    } catch (chybaText) {
        // Rust reject vracia priamo text
        zobrazSpravu(elVysledokUlozenia, chybaText, false);
    }
});

// --- Test pripojenia ---
btnTest.addEventListener("click", async () => {
    elVysledokTestu.textContent = "Testujem...";
    elVysledokTestu.className = "";
    
    try {
        const uspechVeta = await invoke("otestuj_pripojenie");
        zobrazSpravu(elVysledokTestu, uspechVeta, true);
    } catch (chybaText) {
        zobrazSpravu(elVysledokTestu, chybaText, false);
    }
});

// --- Diagnostika: otvoriť priečinok s logom (TLI 24, bod 4) ---
btnOtvorLog.addEventListener("click", async () => {
    try {
        await invoke("otvor_priecinok_logov");
        zobrazSpravu(elVysledokLogu, "Otvorené.", true);
    } catch (chybaText) {
        zobrazSpravu(elVysledokLogu, chybaText, false);
    }
});

// --- Odkazy (otvorenie systémového prehliadača) ---
odkazKluc.addEventListener("click", () => otvorWeb("/kluc"));
odkazSprievodca.addEventListener("click", () => otvorWeb("/sprievodca"));
odkazAnalyzer.addEventListener("click", () => otvorWeb("/analyzer"));

function otvorWeb(cesta) {
    invoke("otvor_web", { cesta }).catch(err => console.error(err));
}

// --- Kontrola aktualizácií ---
btnAktualizacia.addEventListener("click", async () => {
    elVysledokAktualizacie.textContent = "Kontrolujem...";
    elVysledokAktualizacie.className = "";
    btnStiahnut.classList.add("skryte");

    try {
        const urlNovejVerzie = await invoke("skontroluj_aktualizaciu");
        if (urlNovejVerzie === null) {
            zobrazSpravu(elVysledokAktualizacie, "Máš najnovšiu verziu.", true);
        } else {
            zobrazSpravu(elVysledokAktualizacie, "Je vonku novšia verzia — stiahni si ju.", false); // Oranžová/Červená farba v UI ako warning, použijeme chybu pre zvýraznenie
            elVysledokAktualizacie.classList.remove("chyba");
            elVysledokAktualizacie.classList.add("uspech");
            
            urlAktualizacie = urlNovejVerzie;
            btnStiahnut.classList.remove("skryte");
        }
    } catch (chyba) {
        zobrazSpravu(elVysledokAktualizacie, "Chyba pri kontrole: " + chyba, false);
    }
});

btnStiahnut.addEventListener("click", () => {
    if (urlAktualizacie) {
        // Použijeme rustovskú funkciu, cesta tu simuluje celú URL ak backend dovolí, 
        // ale pre plnú URL si vyžaduje appka buď priamu obsluhu v ruste alebo len `otvor_web`
        invoke("otvor_web", { cesta: urlAktualizacie }).catch(err => console.error(err));
    }
});

// --- Pomocná funkcia pre notifikácie ---
function zobrazSpravu(element, text, jeUspech) {
    element.textContent = text;
    element.className = jeUspech ? "uspech" : "chyba";
}
