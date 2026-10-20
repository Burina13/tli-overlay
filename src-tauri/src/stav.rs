use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;
use tauri_plugin_global_shortcut::Shortcut;
use tli_overlay_core::nastavenia::Nastavenia;
use tli_overlay_core::tracker::chvost::StavChvosta;

/// Jedna prebiehajúca analýza (TLI 67b) — jej poradové číslo a kedy začala.
/// Čas je tu preto, že podľa neho sa rozhoduje, či je nové stlačenie „nový
/// zámer" (zruš a foť odznova), alebo len netrpezlivé ťukanie do klávesy.
#[derive(Debug, Clone, Copy)]
pub struct BehAnalyzy {
    pub generacia: u64,
    pub zacata: Instant,
    /// Ktorý modul sa analyzuje. Nové stlačenie INÉHO modulu znamená, že si to
    /// hráč rozmyslel (F8 a hneď Ctrl+W) — to nie je netrpezlivosť a nesmie sa
    /// ignorovať ani v prvej sekunde.
    pub modul: crate::Modul,
    /// Spustil to hráč (hotkey/tlačidlo), alebo automat na pozadí? Stlačenie
    /// hráča smie zrušiť automatickú analýzu HNEĎ, bez čakania na sekundu —
    /// inak by sa F8 do sekundy po automatickom scane cien ticho stratilo.
    pub rucne: bool,
}

pub struct Stav {
    pub nastavenia: Mutex<Nastavenia>,
    /// Práve bežiaca analýza, alebo `None`, keď nebeží žiadna.
    ///
    /// **TLI 67b (`2026-10-20-1348-mcp.md`, bod 2) — bývalo to `Mutex<bool>`
    /// plus fronta `cakajuci_dalsi` (TLI 41) a práve tá fronta bola to, čo
    /// Richard 20.10.2026 o 13:48 opísal ako „celé sa mi to seklo":** Canvas
    /// požiadavka na zaneprázdnenom serveri trvala 34 s, on medzitým stlačil
    /// Ctrl+W asi osemkrát, appka každé stlačenie poslušne „zaradila" a po
    /// dobehnutí spustila ĎALŠIU, už dávno neaktuálnu snímku. Čakanie sa tým
    /// nezrušilo, ale predĺžilo.
    ///
    /// Odteraz sa nič nezaraďuje: nové stlačenie **zruší** tú bežiacu
    /// (`generacia` sa zvýši, výsledok starej sa zahodí) a odfotí odznova —
    /// aktuálny stav obrazovky je dôležitejší než dobehnutie starej
    /// požiadavky. Stlačenie do `OPAKOVANIE_IGNOROVANIE_MS` od štartu
    /// bežiacej sa ignoruje úplne (to je už len netrpezlivosť, nie nový zámer).
    pub beh: Mutex<Option<BehAnalyzy>>,
    /// Rastúce poradové číslo analýz (TLI 67b). Vlákno si pri štarte vezme
    /// svoje a po návrate zo siete sa spýta, či je ešte to platné — keď nie,
    /// nekreslí, neemituje a nesiaha na `beh`. Je to JEDINÝ spôsob, ako sa
    /// „zrušenie" dá s blokujúcim `ureq`-om spraviť: samotné HTTP spojenie
    /// sa prerušiť nedá, ale jeho výsledok sa dá zahodiť a zámok uvoľniť
    /// okamžite (zvyšné vlákno dobehne do 10 s na timeout a ticho skončí).
    pub beh_generacia: Mutex<u64>,
    pub zobrazene_od: Mutex<Option<Instant>>,
    /// Zber dát (TLI 4c) — koľko snímok sa za tento beh appky úspešne poslalo
    /// a uložilo na server v režime "Zber dát". Len v pamäti (reštart appky
    /// počítadlo vynuluje) — Richard má za jeden deň zberu jedno spustenie,
    /// presný súčet beztak drží server (DB `analyzy`), toto je len okamžitá
    /// spätná väzba "fungovalo mi to práve teraz".
    pub pocitadlo_zber: Mutex<u32>,
    /// Kľúče máp, ktoré už partiový server dostal (TLI 11, krok 5). Len v pamäti:
    /// po reštarte appky sa dávka pošle znova a server ju podľa toho istého
    /// kľúča ticho zahodí (`INSERT OR IGNORE`) — druhý zoznam na disku by bol
    /// len ďalšie miesto, ktoré sa vie rozísť s pravdou na serveri.
    pub poslane_behy: Mutex<BTreeSet<String>>,
    /// Značka času, odkedy je viditeľný toast pripomienky „Enable Log"
    /// (TLI 11d) — rovnaký vzor ako `zobrazene_od` pri overlayi: časovač na
    /// automatické skrytie porovná značky, aby novší toast nezhasol starší
    /// časovač práve zobrazeného okna.
    pub pripomienka_zobrazena_od: Mutex<Option<Instant>>,
    /// Posledný text a trieda toastu pripomienky (TLI 37). Rust posiela obsah
    /// udalosťou `pripomienka-obsah` PRED `show()`, takže ju okno, ktoré sa
    /// práve načítava, môže minúť — a hráčovi ostane visieť prázdny rám
    /// (hlásil Richard 18.10.2026). Okno si preto po načítaní vypýta posledný
    /// obsah príkazom `posledna_pripomienka`. Len v pamäti; po reštarte appky
    /// niet čo dokresľovať.
    pub pripomienka_obsah: Mutex<Option<(String, String)>>,
    /// Režim „Upraviť rozloženie" (TLI 21) — kým je zapnutý, F8/F9 analýzu
    /// nespúšťajú a Esc/Enter namiesto skrytia overlayu uloží polohu panelov.
    pub rezim_upravy: Mutex<bool>,
    /// Okno hry a monitor, nad ktorými sa režim úpravy rozloženia spustil —
    /// uloženie na konci (`rezim_upravy_ulozit_panel`) musí počítať s PRESNE
    /// tým istým oknom/monitorom, nie ho zisťovať znova (hráč mohol medzitým
    /// prepnúť okno alebo presunúť myš na iný monitor).
    pub rezim_upravy_kontext: Mutex<Option<RezimUpravyKontext>>,
    /// Upozornenia z POSLEDNÉHO pokusu zaregistrovať klávesové skratky (TLI 28,
    /// bod 2a) — prázdne, keď všetky tri prešli. Nastavuje sa pri štarte appky
    /// aj pri každom uložení Nastavení (`zaregistruj_vsetky_hotkeys`), aby
    /// Nastavenia vedeli ukázať červený stav aj vtedy, keď sa otvoria AŽ PO
    /// štarte appky, nie len priamo pri uložení formulára.
    pub hotkey_upozornenia: Mutex<Vec<String>>,
    /// TLI 44 (`2026-10-18-2331-mcp.md`, bod 3) — debounce na JEDNU skratku,
    /// od poslednej „stlačenej" rovnakej klávesy. Richard 18.10.2026 naživo
    /// videl v logu DVE „Tartarus stlačené (zdroj=hotkey)" v TEJ ISTEJ
    /// sekunde na jediný stlak — `RegisterHotKey` na Windowse posiela
    /// `WM_HOTKEY` opakovane, kým je kombinácia držaná (rovnako ako bežný
    /// key-repeat), appka to doteraz brala ako dve samostatné analýzy.
    /// Drží sa len POSLEDNÁ dvojica (skratka, čas) — nie front — presne tak,
    /// ako `cakajuci_dalsi` vyššie drží len poslednú zdržanú požiadavku.
    pub posledny_hotkey: Mutex<Option<(Shortcut, Instant)>>,
    /// Výsledok POSLEDNEJ kontroly aktualizácie (TLI 52, bod 2) — periodickej
    /// aj ručnej z tlačidla „Check for Updates", obe idú cez to isté miesto.
    /// `None`, kým appka ešte ani raz neskontrolovala. Len v pamäti (rovnaký
    /// vzor ako `posledna_synchronizacia_cien` v trackeri) — po reštarte sa
    /// kontrola spustí hneď znova, netreba to prenášať cez reštart appky.
    pub posledna_kontrola_aktualizacie: Mutex<Option<KontrolaAktualizacie>>,
    /// TLI 70 — POSLEDNÁ obrazovka, ktorú server naozaj prečítal, a kedy.
    /// Zapisuje sa po každej úspešnej ručnej analýze (Ctrl+Q/Ctrl+W/F8/F9);
    /// číta to zber na pozadí (`zber_canvas`), keď mu lokálna detekcia
    /// z pixelov nepovie „toto je Canvas".
    ///
    /// Richard 20.10.2026 v zadaní: „Canvas je otvorený, keď posledná Ctrl+W
    /// analýza (alebo server) vrátila obrazovku canvas, a to platí do zmeny
    /// obrazovky alebo 5 min bez kliku." Platnosť je v
    /// `zber_stav::PLATNOST_POTVRDENIA_S`, „zmena obrazovky" je tým, že sem
    /// každá ďalšia analýza prepíše novú hodnotu.
    /// Trojica: názov obrazovky, kedy to server potvrdil, a ako obrazovka
    /// v tej chvíli VYZERALA (hrubý pomer farieb). Podpis je tam preto,
    /// že „platí do zmeny obrazovky" sa inak splniť nedá — čas sám o sebe
    /// nevie, že hráč Canvas medzitým zavrel.
    pub potvrdena_obrazovka:
        Mutex<Option<(String, Instant, Option<tli_overlay_core::zber_stav::Podpis>)>>,
    /// TLI 70 (bod 5) — čo zber na pozadí odoslal, čo mu čaká a prečo
    /// prípadne nefotí. Len v pamäti; `caka` sa pri čítaní dopočíta z disku.
    pub stav_zberu: Mutex<tli_overlay_core::zber_stav::StavZberu>,
}

/// Pozri `Stav::posledna_kontrola_aktualizacie`. `cas` je unixový čas
/// v sekundách — appka časové pásmo nikde inde neprevádza, rovnaký vzor ako
/// `ceny_synchronizovane_o` v `tracker.rs`; footer si z neho sám urobí
/// lokálny čas hráča.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KontrolaAktualizacie {
    pub cas: u64,
    pub najdena_verzia: Option<String>,
    pub msi_url: Option<String>,
    pub chyba: Option<String>,
}

/// Kontext jedného behu režimu úpravy rozloženia (TLI 21) — pozri
/// `Stav::rezim_upravy_kontext`.
#[derive(Debug, Clone)]
pub struct RezimUpravyKontext {
    pub okno_hry: tli_overlay_core::suradnice::OknoFyz,
    pub mierka: f64,
    pub monitor_sig: String,
    /// Kľúč panela, ktorý sa PRÁVE upravuje (TLI 25) — hlavný panel má od
    /// TLI 25 vlastnú polohu pre každý modul a prepínač v náhľade ho mení
    /// za behu. Pri vstupe do režimu je to `PANEL_TARTARUS`.
    pub panel_kluc: String,
}

/// Sluzi na bezpecne ziskanie zlozky na ulozenie konfiguracie pre hracie konto z operacneho systemu.
/// DOVOD: Aplikacia si potrebuje ukladat nastavenia trvalo mimo instalacneho priestoru, pretoze v dalsej verzii by ju instalator prepisal.
pub fn cesta_suboru() -> PathBuf {
    let mut cesta = if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
    } else {
        // Fallback prevazne pre CI a testovacie prostredia
        if let Ok(home) = std::env::var("HOME") {
            let mut p = PathBuf::from(home);
            p.push(".config");
            p
        } else {
            PathBuf::from(".")
        }
    };
    cesta.push("sk.tvojtatko.tli-overlay");
    cesta.push("nastavenia.json");
    cesta
}

/// Ziska lokalnu kopiu suboru s konfiguraciou v systeme po starte.
/// DOVOD: Inicializuje aplikaciu klucom pre pristup. Bez neho nastroj nefunguje a pri nenajdeni
/// poskytne defaultnu (vycistenu) hodnotu struktury.
pub fn nacitaj() -> Nastavenia {
    let cesta = cesta_suboru();
    if let Ok(obsah) = std::fs::read_to_string(&cesta) {
        Nastavenia::z_jsonu(&obsah)
    } else {
        Nastavenia::z_jsonu("{}")
    }
}

/// Zapisuje pouzivatelom modifikovane data a spristupnuje novu konfiguraciu pri dalsej analzye.
/// DOVOD: Perzistencia hesiel k API a klavesovych skratiek je nutna pre standardny operacny chod.
pub fn uloz(n: &Nastavenia) -> Result<(), String> {
    crate::denik::zapis("stav-debug", &format!("uloz(): minimapa_zber_zapnute={}", n.minimapa_zber_zapnute));
    let cesta = cesta_suboru();

    if let Some(priecinok) = cesta.parent() {
        if let Err(e) = std::fs::create_dir_all(priecinok) {
            return Err(format!("Nepodarilo sa vytvoriť priečinok pre nastavenia: {}", e));
        }
    }

    let json = n.do_jsonu();
    if let Err(e) = std::fs::write(&cesta, json) {
        return Err(format!("Nepodarilo sa uložiť nastavenia do súboru: {}", e));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Pozícia v hernom logu (TLI 11c) — nech appka po reštarte nemusí znova
// reparsovať aj to, čo už raz videla. Žije v samostatnom súbore vedľa
// `nastavenia.json`, nie v ňom: je to prevádzkový stav, nie nastavenie od
// hráča, a jeho strata (zmazaný/poškodený súbor) nesmie zobrať nastavenia.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct UlozenyChvost {
    cesta: String,
    chvost: StavChvosta,
}

fn cesta_chvosta_suboru() -> PathBuf {
    let mut c = cesta_suboru();
    c.set_file_name("tracker-pozicia.json");
    c
}

/// Cesta k lokálnej SQLite databáze trackera (TLI 13) — vedľa `nastavenia.json`,
/// z rovnakého dôvodu ako `tracker-pozicia.json`: je to prevádzkový stav/dáta,
/// nie nastavenie od hráča.
pub fn cesta_db_suboru() -> PathBuf {
    let mut c = cesta_suboru();
    c.set_file_name("tracker.sqlite3");
    c
}

/// Cesta k logu chýb pri práci s oknami (TLI 23) — vedľa `nastavenia.json`,
/// z rovnakého dôvodu ako ostatné prevádzkové súbory vyššie.
fn cesta_log_okien_suboru() -> PathBuf {
    let mut c = cesta_suboru();
    c.set_file_name("okna.log");
    c
}

/// Pripíše jeden riadok o chybe pri otváraní/zatváraní okna (TLI 23 — predtým
/// `let _ = okno.show()` a podobné volania chybu ticho zahodili, appka sa
/// navonok tvárila v poriadku a druhý klik na „Nastavenia" v skutočnosti
/// nerobil nič). Zápis je `best effort`: appka na chybe v logovaní nesmie
/// spadnúť, je to diagnostika, nie dáta.
pub fn zapis_chybu_okna(cas: u64, kontext: &str, chyba: &str) {
    use std::io::Write;
    let cesta = cesta_log_okien_suboru();
    if let Some(priecinok) = cesta.parent() {
        let _ = std::fs::create_dir_all(priecinok);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&cesta) {
        let _ = writeln!(f, "{cas} {kontext}: {chyba}");
    }
}

/// Načíta uloženú pozíciu v logu — ale LEN keď uložená cesta sedí s tou, čo
/// sa práve číta. Inak by appka po zmene cesty (iná hra, iný disk) omylom
/// „pokračovala" v súbore, ktorý vôbec nečíta, a buď preskočila jeho
/// začiatok, alebo (pri menšom súbore) zbytočne čakala na rotáciu.
pub fn nacitaj_chvost(cesta: &Path) -> StavChvosta {
    let Ok(obsah) = std::fs::read_to_string(cesta_chvosta_suboru()) else {
        return StavChvosta::novy();
    };
    chvost_z_obsahu(&obsah, cesta)
}

/// Rozhodnutie „sedí uložená cesta s tou, čo sa číta teraz" vytiahnuté do
/// čistej funkcie bez dotyku s diskom — dá sa otestovať priamo, bez súborov
/// aj bez `$APPDATA`/`$HOME` (na tie sa viaže `cesta_chvosta_suboru()`).
fn chvost_z_obsahu(obsah: &str, cesta: &Path) -> StavChvosta {
    let Ok(ulozeny) = serde_json::from_str::<UlozenyChvost>(obsah) else {
        return StavChvosta::novy();
    };
    if ulozeny.cesta == cesta.display().to_string() {
        ulozeny.chvost
    } else {
        StavChvosta::novy()
    }
}

/// Uloží pozíciu po každom úspešnom čítaní, ktoré posunulo stav ďalej.
/// Zápis je zámerne best-effort (chyba sa tichy ignoruje) — strata tohto
/// súboru appku nezastaví, len pri ďalšom reštarte prečíta log znova od
/// začiatku.
pub fn uloz_chvost(cesta: &Path, chvost: &StavChvosta) {
    let subor = cesta_chvosta_suboru();
    let ulozeny = UlozenyChvost { cesta: cesta.display().to_string(), chvost: chvost.clone() };
    let Ok(json) = serde_json::to_string(&ulozeny) else {
        return;
    };
    if let Some(priecinok) = subor.parent() {
        let _ = std::fs::create_dir_all(priecinok);
    }
    let _ = std::fs::write(&subor, json);
}

#[cfg(test)]
mod testy_chvosta {
    use super::*;

    #[test]
    fn chvost_sa_obnovi_len_pri_zhode_cesty() {
        let cesta = Path::new("/hra/UE_game.log");
        let mut chvost = StavChvosta::novy();
        chvost.priprav_riadky(b"abc\n");
        let ulozeny = UlozenyChvost { cesta: cesta.display().to_string(), chvost };
        let json = serde_json::to_string(&ulozeny).unwrap();

        let obnoveny = chvost_z_obsahu(&json, cesta);
        assert_eq!(obnoveny.pozicia, 4);

        // Iná cesta (iná hra/iný disk) sa nesmie „pokračovať" v cudzom súbore.
        let ina_cesta = Path::new("/ina/hra.log");
        let cerstvy = chvost_z_obsahu(&json, ina_cesta);
        assert_eq!(cerstvy.pozicia, 0);
    }

    #[test]
    fn poskodeny_alebo_chybajuci_obsah_da_cerstvy_stav() {
        let cesta = Path::new("/hra/UE_game.log");
        assert_eq!(chvost_z_obsahu("nie je to json", cesta).pozicia, 0);
        assert_eq!(chvost_z_obsahu("", cesta).pozicia, 0);
    }
}
