const { invoke } = window.__TAURI__.core;

// Odkazy na formuláre a prvky
const formular = document.getElementById("formular");
const inpUrl = document.getElementById("inp-url");
const inpKluc = document.getElementById("inp-kluc");
const inpHotkey = document.getElementById("inp-hotkey");
const inpHotkeyCanvas = document.getElementById("inp-hotkey-canvas");
const inpHotkeyTrh = document.getElementById("inp-hotkey-trh");
const btnNahrajHotkey = document.getElementById("btn-nahraj-hotkey");
const btnNahrajHotkeyCanvas = document.getElementById("btn-nahraj-hotkey-canvas");
const btnNahrajHotkeyTrh = document.getElementById("btn-nahraj-hotkey-trh");
const elStavHotkey = document.getElementById("stav-hotkey");
const elStavHotkeyCanvas = document.getElementById("stav-hotkey-canvas");
const elStavHotkeyTrh = document.getElementById("stav-hotkey-trh");
const inpProces = document.getElementById("inp-proces");
const inpAutoskrytie = document.getElementById("inp-autoskrytie");
const inpSkrytieRezim = document.getElementById("inp-skrytie-rezim");
const inpZvuk = document.getElementById("inp-zvuk");
const inpZberDat = document.getElementById("inp-zber-dat");
const inpUcenieCanvas = document.getElementById("inp-ucenie-canvas");
const inpMinimapaZber = document.getElementById("inp-minimapa-zber");
const inpMinimapaRezim = document.getElementById("inp-minimapa-rezim");

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

// TLI 62/2b — vzhľad trasy nad minimapou (doplnok 1 k TLI 62, bod 4).
const inpTrasaOpacitaVrstvy = document.getElementById("inp-trasa-opacita-vrstvy");
const inpTrasaOpacitaCiary = document.getElementById("inp-trasa-opacita-ciary");
const inpTrasaHrubka = document.getElementById("inp-trasa-hrubka");
const inpTrasaFarba = document.getElementById("inp-trasa-farba");
const hodTrasaOpacitaVrstvy = document.getElementById("hod-trasa-opacita-vrstvy");
const hodTrasaOpacitaCiary = document.getElementById("hod-trasa-opacita-ciary");
const hodTrasaHrubka = document.getElementById("hod-trasa-hrubka");

/// Predvolené hodnoty sú tu ZDUPLIKOVANÉ z `core/src/minimapa_vzhlad.rs`
/// zámerne: formulár musí vedieť, čo ukázať, aj keď nastavenia ešte pole
/// `vzhlad_trasy` nemajú (starý `nastavenia.json`). Rust ich pri uložení aj
/// tak znormalizuje, takže toto nie je druhý zdroj pravdy, len predvoľba
/// zobrazenia — rovnaký vzor ako `|| "F8"` pri hotkeyoch vyššie.
const TRASA_PREDVOLENE = { opacita_ciary: 85, opacita_vrstvy: 100, hrubka_px: 3, farba: "#FFD24A" };

/// Popisky vedľa posuvníkov musia ísť s hodnotou, inak hráč nevie, čo si
/// nastavil — `<input type=range>` svoju hodnotu sám neukazuje.
function zosuladPopiskyTrasy() {
    hodTrasaOpacitaVrstvy.textContent = inpTrasaOpacitaVrstvy.value;
    hodTrasaOpacitaCiary.textContent = inpTrasaOpacitaCiary.value;
    hodTrasaHrubka.textContent = inpTrasaHrubka.value;
}

for (const prvok of [inpTrasaOpacitaVrstvy, inpTrasaOpacitaCiary, inpTrasaHrubka]) {
    prvok.addEventListener("input", zosuladPopiskyTrasy);
}

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

// --- Nahratie klávesovej skratky priamo stlačením (TLI 28, bod 2b; TLI 39,
// bod 1 pridáva validáciu proti holej klávese hry a potvrdenie) ---
// Hráč doteraz musel poznať presný textový formát (`CommandOrControl+Shift+T`)
// — teraz stačí kliknúť na „Nahrať…“ a stlačiť kombináciu. `event.code` sa
// posiela appke takmer bez úprav: Rust (`global-hotkey`) parsuje presne tie
// isté názvy („F8“, „KeyA“, „Digit1“, „ArrowUp“…), takže tu nie je žiadny
// preklad okrem zoradenia modifikátorov pred hlavnú klávesu.
const KODY_SAMOTNYCH_MODIFIKATOROV = [
    "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight",
    "AltLeft", "AltRight", "MetaLeft", "MetaRight",
];

// TLI 39, bod 1 (`2026-10-18-1613-mcp.md`): 18.10.2026 sa Tartarus skratka
// omylom zapísala ako holé „DIGIT3" (bez modifikátora) — appka to zoberie aj
// naďalej (appka s hocijakým reťazcom pôjde do Rustu, ktorý má vlastnú
// validáciu formátu), ale appka ju odteraz sama neponúkne ako platnú voľbu:
// holá klávesa bez modifikátora v hre niečo znamená (tu konkrétne číslo „3"
// v akčnej lište) a appka by jej stlačenie hre ukradla. Povolené sú F1–F12
// samostatne (tie hra nepoužíva) alebo ĽUBOVOĽNÁ klávesa držaná spolu
// aspoň s jedným modifikátorom (Ctrl/Alt/Shift/Super).
const F_KLAVESA = /^F([1-9]|1[0-2])$/;

function jePlatnaSkratka(text) {
    const t = (text || "").trim().toUpperCase();
    if (!t) return false;
    const casti = t.split("+").map((c) => c.trim()).filter(Boolean);
    if (casti.length === 0) return false;
    const hlavna = casti[casti.length - 1];
    const maModifikator = casti.length > 1;
    return maModifikator || F_KLAVESA.test(hlavna);
}

// Prepojenie poľa na jeho vlastný riadok so stavom — chybu z VALIDÁCIE počas
// nahrávania treba ukázať v TOM ISTOM mieste, kam appka píše stav z backendu
// (`aktualizujStavHotkeys`), inak by hráč pozeral na dve rôzne miesta.
function elementStavuPre(input) {
    if (input === inpHotkeyCanvas) return elStavHotkeyCanvas;
    if (input === inpHotkeyTrh) return elStavHotkeyTrh;
    return elStavHotkey;
}

function nahrajSkratku(input, button) {
    const povodnyText = button.textContent;
    const elStav = elementStavuPre(input);
    button.textContent = "Stlač klávesu…";
    button.disabled = true;

    const zachyt = (udalost) => {
        udalost.preventDefault();
        if (udalost.code === "Escape") {
            koniec(); // zrušenie nahrávania bez zmeny — Escape appka používa inde
            return;
        }
        // Čaká sa na HLAVNÚ klávesu — samotné stlačenie modifikátora
        // (napr. držanie Ctrl pred druhou klávesou) ešte nahrávanie neukončí.
        if (KODY_SAMOTNYCH_MODIFIKATOROV.includes(udalost.code)) {
            return;
        }
        const casti = [];
        if (udalost.ctrlKey) casti.push("CONTROL");
        if (udalost.altKey) casti.push("ALT");
        if (udalost.shiftKey) casti.push("SHIFT");
        if (udalost.metaKey) casti.push("SUPER");
        casti.push(udalost.code);
        const navrh = casti.join("+");

        if (!jePlatnaSkratka(navrh)) {
            elStav.textContent =
                "Táto klávesa by appke zobrala klávesu, ktorú používa hra — použi F1–F12, " +
                "alebo ju drž spolu s Ctrl, Alt alebo Shift. Skús inú kombináciu (Escape zruší).";
            elStav.classList.add("chyba");
            return; // appka ostáva v režime nahrávania, nič sa nezapísalo
        }

        // TLI 39, bod 1: „zachytávanie… s potvrdením" — jedno stlačenie je
        // príliš ľahké fatfingerovať (presne to sa stalo 18.10.2026), preto
        // appka pred zápisom vždy vyžiada výslovné potvrdenie.
        if (!window.confirm(`Nastaviť klávesovú skratku na „${navrh}"?`)) {
            return; // zrušené, appka ostáva v režime nahrávania
        }

        input.value = navrh;
        koniec();
    };
    const koniec = () => {
        window.removeEventListener("keydown", zachyt, true);
        button.textContent = povodnyText;
        button.disabled = false;
    };
    window.addEventListener("keydown", zachyt, true);
}

btnNahrajHotkey.addEventListener("click", () => nahrajSkratku(inpHotkey, btnNahrajHotkey));
btnNahrajHotkeyCanvas.addEventListener("click", () => nahrajSkratku(inpHotkeyCanvas, btnNahrajHotkeyCanvas));
btnNahrajHotkeyTrh.addEventListener("click", () => nahrajSkratku(inpHotkeyTrh, btnNahrajHotkeyTrh));

// --- Červený stav „F8 používa iný program" pod poľom (TLI 28, bod 2a) ---
// Appka vie o kolízii už od ŠTARTU (nielen po uložení formulára) — preto sa
// toto volá aj hneď pri otvorení okna, nie len z `catch` pri uložení.
function aktualizujStavHotkeys(chyby) {
    const priraduj = (el, zhoda) => {
        const text = chyby.find(zhoda) || "";
        el.textContent = text;
        el.classList.toggle("chyba", !!text);
    };
    priraduj(elStavHotkey, (t) => !t.includes("Frozen Canvas") && !t.includes("ceny z trhu"));
    priraduj(elStavHotkeyCanvas, (t) => t.includes("Frozen Canvas"));
    priraduj(elStavHotkeyTrh, (t) => t.includes("ceny z trhu"));
}

async function obnovStavHotkeys() {
    try {
        aktualizujStavHotkeys(await invoke("hotkey_upozornenia"));
    } catch (chyba) {
        console.error("Načítanie stavu klávesových skratiek zlyhalo:", chyba);
    }
}

/// Pole „Automatické skrytie (sekundy)" má zmysel len v režime „po čase" —
/// v režime kliku je zašednuté, nech hráč nehľadá, prečo sa číslom nič nemení.
/// Exportované cez `window` len kvôli jsdom testom (rovnaký vzor ako inde v UI).
function zosuladSkrytieRezim() {
    const poCase = inpSkrytieRezim.value === "cas";
    inpAutoskrytie.disabled = !poCase;
    inpAutoskrytie.closest(".skupina").classList.toggle("nedostupne", !poCase);
}

inpSkrytieRezim.addEventListener("change", zosuladSkrytieRezim);

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
        inpHotkeyTrh.value = nastavenia.hotkey_trh || "F10";
        inpProces.value = nastavenia.proces_hry || "";
        inpAutoskrytie.value = nastavenia.autoskrytie_s || 0;
        // TLI 58: predvolený režim je klik. `undefined` (staré nastavenia bez
        // tohto poľa) sa musí správať ako `true` — rovnako ako `#[serde(default)]`
        // na strane Rustu, inak by formulár hráčovi potichu prepol režim na čas.
        inpSkrytieRezim.value = nastavenia.skryt_radu_klikom === false ? "cas" : "klik";
        zosuladSkrytieRezim();
        inpZvuk.checked = !!nastavenia.zvuk;
        inpZberDat.checked = !!nastavenia.zber_dat_zapnute;
        // TLI 60: predvolené je ZAPNUTÉ, takže `undefined` (staré nastavenia
        // bez tohto poľa) sa musí správať ako `true` — rovnako ako
        // `#[serde(default)]` na strane Rustu. Obyčajné `!!` by hráčovi
        // pri prvom otvorení Nastavení zber potichu vyplo.
        inpUcenieCanvas.checked = nastavenia.ucenie_canvas_zapnute !== false;
        // TLI 62/3: predvolené je VYPNUTÉ — na rozdiel od `ucenie_canvas_zapnute`
        // tu `undefined` (staré nastavenia bez poľa) musí zostať `false`.
        inpMinimapaZber.checked = !!nastavenia.minimapa_zber_zapnute;
        inpMinimapaRezim.value = nastavenia.minimapa_rezim === "roh" ? "roh" : "stred";
        const priehladnost = nastavenia.rozlozenie && nastavenia.rozlozenie.priehladnost;
        inpPriehladnost.value = typeof priehladnost === "number" ? priehladnost : 1;

        // TLI 62/2b. `|| TRASA_PREDVOLENE.x` je tu správne aj pre NULU: nula
        // je platná hodnota („dočasne to nechcem vidieť"), preto sa testuje
        // typ, nie pravdivosť — `0 || 85` by hráčovi stíšenú trasu potichu
        // vrátilo na 85 %.
        const vzhlad = nastavenia.vzhlad_trasy || {};
        const cislo = (hodnota, predvolene) => (typeof hodnota === "number" ? hodnota : predvolene);
        inpTrasaOpacitaVrstvy.value = cislo(vzhlad.opacita_vrstvy, TRASA_PREDVOLENE.opacita_vrstvy);
        inpTrasaOpacitaCiary.value = cislo(vzhlad.opacita_ciary, TRASA_PREDVOLENE.opacita_ciary);
        inpTrasaHrubka.value = cislo(vzhlad.hrubka_px, TRASA_PREDVOLENE.hrubka_px);
        inpTrasaFarba.value = (vzhlad.farba || TRASA_PREDVOLENE.farba).toLowerCase();
        zosuladPopiskyTrasy();

        // Keď hráč zavrie Nastavenia uprostred úpravy rozloženia a znova ich
        // otvorí, tlačidlo musí ukázať aktuálny stav, nie vždy „Upraviť".
        nastavTlacidloRezimu(await invoke("rezim_upravy_je_aktivny"));

        // Načítanie verzie
        const verzia = await invoke("verzia");
        elVerzia.textContent = `Verzia ${verzia}`;
    } catch (chyba) {
        console.error("Chyba pri načítaní nastavení:", chyba);
    }

    await obnovStavHotkeys();
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

    // TLI 39, bod 1: rovnaká validácia ako pri nahrávaní stlačením — toto tu
    // chytí aj priame PREPÍSANIE poľa (pole nie je `readonly`, appka vie
    // nahrať aj pokročilý formát ako `CommandOrControl+Shift+T` písaním),
    // nielen zachytávanie stlačením. hotkey_trh môže ostať prázdny (appka ho
    // pri uložení doplní na predvolenú „F10"), hotkey a hotkey_canvas nie.
    const novyHotkey = inpHotkey.value.trim();
    const novyHotkeyCanvas = inpHotkeyCanvas.value.trim();
    const novyHotkeyTrh = inpHotkeyTrh.value.trim();
    const CHYBA_SKRATKY =
        "môže kolidovať s hrou — použi F1–F12, alebo klávesu drž spolu s Ctrl, Alt alebo Shift.";
    for (const [popis, hodnota] of [["Tartarus", novyHotkey], ["Frozen Canvas", novyHotkeyCanvas]]) {
        if (!jePlatnaSkratka(hodnota)) {
            zobrazSpravu(elVysledokUlozenia, `Klávesová skratka pre ${popis} („${hodnota}") ${CHYBA_SKRATKY}`, false);
            return;
        }
    }
    if (novyHotkeyTrh && !jePlatnaSkratka(novyHotkeyTrh)) {
        zobrazSpravu(elVysledokUlozenia, `Klávesová skratka pre ceny z trhu („${novyHotkeyTrh}") ${CHYBA_SKRATKY}`, false);
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
        hotkey: novyHotkey,
        hotkey_canvas: novyHotkeyCanvas,
        hotkey_trh: novyHotkeyTrh,
        proces_hry: inpProces.value.trim(),
        autoskrytie_s: parseInt(inpAutoskrytie.value, 10) || 0,
        skryt_radu_klikom: inpSkrytieRezim.value !== "cas",
        zvuk: inpZvuk.checked,
        zber_dat_zapnute: inpZberDat.checked,
        ucenie_canvas_zapnute: inpUcenieCanvas.checked,
        minimapa_zber_zapnute: inpMinimapaZber.checked,
        minimapa_rezim: inpMinimapaRezim.value,
        // TLI 62/2b — farbu posiela `<input type=color>` vždy ako malé
        // písmená (`#ffd24a`); Rust `normalizuj()` ju zdvihne na veľké, aby
        // sa `nastavenia.json` nemenil len kvôli veľkosti písmen.
        vzhlad_trasy: {
            opacita_ciary: parseInt(inpTrasaOpacitaCiary.value, 10),
            opacita_vrstvy: parseInt(inpTrasaOpacitaVrstvy.value, 10),
            hrubka_px: parseInt(inpTrasaHrubka.value, 10),
            farba: inpTrasaFarba.value
        }
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
    } finally {
        // Backend (`zaregistruj_vsetky_hotkeys`) si upozornenia zapamätal
        // nezávisle od toho, či `uloz_nastavenia` skončilo OK alebo chybou —
        // červený stav pod poľom sa preto vždy dotiahne nanovo z neho, nielen
        // z `chybaText` vyššie (ten je jeden spojený text pre `elVysledokUlozenia`).
        obnovStavHotkeys();
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
