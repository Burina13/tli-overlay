//! Automatické učenie Frozen Canvasu na pozadí (TLI 60,
//! `2026-10-19-2135-mcp-2.md`). Richard 19.10.2026: „kľudne nech si všetko,
//! čo spravím, fotí a na pozadí pošle agentovi na učenie, a na konci dňa nech
//! sa naučí."
//!
//! Dve vlákna, obe sa štartujú raz pri spustení appky:
//!
//! * **sledovač** — pozerá sa na tlačidlá myši (`GetAsyncKeyState`, rovnaké
//!   čítanie stavovej tabuľky ako TLI 58, nič neodchytáva a do cesty kliku
//!   nevstupuje). Po stlačení počká `ONESKORENIE_PO_KLIKU_MS`, odfotí okno
//!   hry, LOKÁLNE si overí, že je naozaj otvorený Frozen Canvas
//!   (`tli_overlay_core::canvas_detekcia`), a keď áno, odloží zmenšený JPEG
//!   do priečinka fronty.
//! * **odosielač** — fronta na disku, jeden súbor po druhom na
//!   `?modul=canvas&zber=1`. Keď server nie je dostupný, súbor v priečinku
//!   ostane a skúša sa znova s rastúcim odstupom.
//!
//! Hráčovi sa pritom NEUKAZUJE NIČ — žiadny panel, žiadny toast, žiadne
//! čakanie na odpoveď servera. Jediná stopa je denník appky.
//!
//! **Čo appka zámerne NEFOTÍ** (poradie je aj poradím kontrol v kóde, od
//! najlacnejšej):
//! 1. keď je prepínač „Učenie Canvas na pozadí" vypnutý;
//! 2. keď je vyčerpaný denný strop `MAX_ZA_DEN`;
//! 3. keď od poslednej kontroly neprešiel dosť dlhý čas
//!    (`zber_fronta::ma_sa_kontrolovat` — mimo Canvasu nanajvýš raz za 2 s,
//!    nech sa pri bežnom hraní nefotí pri každom kliku v boji);
//! 4. keď okno hry **nie je v popredí** — bez tejto kontroly by appka
//!    odfotila obdĺžnik, na ktorom má hráč práve prehliadač alebo rozhovor;
//! 5. keď nad hrou práve svieti vlastný panel appky (bola by na snímke rada,
//!    nie čistá obrazovka hry) alebo keď práve beží ručná analýza (F8/F9);
//! 6. keď snímka nie je Frozen Canvas — a to je ten najčastejší prípad.
//!
//! **Nič sa nemaže** (zadanie): keď je fronta plná (`MAX_CAKAJUCICH`), appka
//! len prestane pridávať, kým sa nevyprázdni. Jediný súbor, ktorý zo spoolu
//! mizne, je ten, ktorý server PRÁVE prijal a trvalo archivoval — to je
//! bežný život fronty, nie mazanie dát.

use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tli_overlay_core::nastavenia::Nastavenia;
use tli_overlay_core::zber_fronta as fronta;

use crate::denik;
use crate::klient;
use crate::stav::Stav;

// `okno` aj `snimka` sú vo `main.rs` za `#[cfg(windows)]` (obe sú tenká vrstva
// nad Win32 GDI) — celá cesta „nájdi okno → odfoť → rozpoznaj" je preto
// Windows-only. Na Linuxe sa modul aj tak kompiluje len kvôli `cargo check`
// a testom jadra, appka tam nebeží.
#[cfg(windows)]
use crate::okno;
#[cfg(windows)]
use crate::snimka;
#[cfg(windows)]
use tli_overlay_core::canvas_detekcia;

/// Dlhšia strana zmenšenej snímky a kvalita JPEGu — obe priamo zo zadania
/// („Zmenšený JPEG (napr. 1280 px, q75)"). Je to menej než `MAX_DLZKA` z
/// `tli_overlay_core::zmensenie` (1920), ktorú posiela ručná analýza: tieto snímky nejdú do vision modelu
/// na radu v reálnom čase, ale do nočného učenia, kde ide o stav dosky, nie
/// o čitateľnosť drobného textu.
#[cfg(windows)]
const DLZKA_ZBERU: u32 = 1280;
#[cfg(windows)]
const KVALITA_ZBERU: u8 = 75;

/// Ako často sa pýtame Windowsu na tlačidlá myši. Rovnaká hodnota ako v
/// TLI 58 (`KROK_POLLINGU_MS`) a z rovnakého dôvodu — je to len čítanie
/// stavovej tabuľky, stojí rádovo nič.
const KROK_POLLINGU_MS: u64 = 25;

/// Ako dlho spí odosielač, keď nie je čo posielať.
const ODOSIELAC_NECINNY_S: u64 = 10;

/// Priečinok fronty vedľa nastavení appky.
pub fn cesta_fronty() -> PathBuf {
    let mut c = crate::stav::cesta_suboru();
    c.pop(); // preč "nastavenia.json"
    c.push("canvas-zber");
    c
}

fn cesta_denneho_stavu() -> PathBuf {
    cesta_fronty().join("stav.json")
}

fn nacitaj_denny_stav() -> fronta::DennyStav {
    std::fs::read_to_string(cesta_denneho_stavu())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn uloz_denny_stav(s: &fronta::DennyStav) {
    // Best-effort, presne ako denník: keď sa strop nepodarí zapísať, appka
    // kvôli tomu nesmie prestať fungovať. Najhoršie, čo sa stane, je, že sa
    // po reštarte počítadlo vráti o kúsok späť.
    if let Ok(json) = serde_json::to_string(s) {
        let _ = std::fs::write(cesta_denneho_stavu(), json);
    }
}

/// Zoradený zoznam čakajúcich snímok. Zoradenie je podstatné: mená nesú čas,
/// takže abecedné poradie = poradie vzniku, a server tak dostane ťahy v tom
/// poradí, v akom ich hráč spravil.
fn cakajuce() -> Vec<PathBuf> {
    let Ok(polozky) = std::fs::read_dir(cesta_fronty()) else {
        return Vec::new();
    };
    let mut subory: Vec<PathBuf> = polozky
        .flatten()
        .map(|p| p.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(fronta::je_subor_fronty)
                .unwrap_or(false)
        })
        .collect();
    subory.sort();
    subory
}

fn nastavenia_appky(app: &AppHandle) -> Nastavenia {
    app.state::<Stav>().nastavenia.lock().unwrap().clone()
}

/// Svieti práve nad hrou vlastný panel appky, alebo beží ručná analýza?
/// Vtedy sa nefotí — na snímke by bola rada appky, nie čistá obrazovka hry,
/// a presne taká snímka by učenie pokazila.
#[cfg(windows)]
fn appka_prekaza(app: &AppHandle) -> bool {
    let stav = app.state::<Stav>();
    let panel_svieti = stav.zobrazene_od.lock().unwrap().is_some();
    let analyza_bezi = *stav.beh.lock().unwrap();
    panel_svieti || analyza_bezi
}

/// Okno hry — a len vtedy, keď je naozaj v popredí. Na rozdiel od ručnej
/// analýzy (F8) sa tu ZÁMERNE nerobí fallback na „čokoľvek je v popredí":
/// toto beží bez vedomia hráča, takže sa nesmie odfotiť nič, čo nie je hra.
#[cfg(windows)]
fn okno_hry_v_popredi(proces: &str) -> Option<okno::NajdeneOkno> {
    let hra = okno::najdi_hru(proces)?;
    let vpredu = okno::popredie()?;
    if vpredu.hwnd == hra.hwnd {
        Some(hra)
    } else {
        None
    }
}

/// Jeden pokus o zber po kliku. Vracia `true`, keď snímka naozaj pribudla do
/// fronty — volajúci podľa toho vie, či sa má zarátať do denného stropu.
/// `posledna_bola_canvas` sa prepíše podľa toho, čo sa na obrazovke našlo
/// (riadi odstup medzi kontrolami).
#[cfg(windows)]
fn pokus_o_zber(app: &AppHandle, n: &Nastavenia, posledna_bola_canvas: &mut bool) -> bool {
    if appka_prekaza(app) {
        return false;
    }
    let Some(hra) = okno_hry_v_popredi(&n.proces_hry) else {
        *posledna_bola_canvas = false;
        return false;
    };

    let snimok = match snimka::zachyt(&hra.obdlznik) {
        Ok(s) => s,
        Err(_) => {
            // Ticho. Zlyhanie BitBltu pri zbere na pozadí nie je nič, čo by
            // mal hráč riešiť, a do denníka by to pri každom kliku nasypalo
            // stovky riadkov.
            *posledna_bola_canvas = false;
            return false;
        }
    };
    if snimka::je_cierna(&snimok) {
        // Exkluzívny fullscreen — BitBlt vracia čierno. Ručná analýza na to
        // hráča upozorní, zber na pozadí sa k tomu nevyjadruje.
        *posledna_bola_canvas = false;
        return false;
    }

    let crty = canvas_detekcia::crty(&snimok.rgb, snimok.sirka, snimok.vyska);
    if !crty.je_canvas() {
        *posledna_bola_canvas = false;
        return false;
    }
    *posledna_bola_canvas = true;

    let cakajucich = cakajuce().len();
    if !fronta::smie_pribudnut(cakajucich) {
        denik::zapis(
            "zber",
            &format!("fronta je plná ({cakajucich} snímok) — nič sa nemaže, len sa nepridáva"),
        );
        return false;
    }

    let Ok(jpeg) = snimka::do_jpegu(&snimok, DLZKA_ZBERU, KVALITA_ZBERU) else {
        return false;
    };

    let cas = crate::tracker::unixovy_cas();
    let priecinok = cesta_fronty();
    if std::fs::create_dir_all(&priecinok).is_err() {
        return false;
    }
    // Poradie v rámci sekundy — pri rýchlom klikaní vznikne za sekundu aj
    // niekoľko snímok a rovnaké meno by jednu z nich prepísalo.
    let mut poradie = 0u64;
    let cesta = loop {
        let c = priecinok.join(fronta::nazov_snimky(cas, poradie));
        if !c.exists() {
            break c;
        }
        poradie += 1;
        if poradie >= 10_000 {
            return false;
        }
    };

    // Najprv `.tmp`, potom premenovanie — odosielač tak nikdy nenarazí na
    // polovičný súbor (`je_subor_fronty` `.tmp` neberie).
    let docasna = cesta.with_extension("jpg.tmp");
    if std::fs::write(&docasna, &jpeg).is_err() || std::fs::rename(&docasna, &cesta).is_err() {
        let _ = std::fs::remove_file(&docasna);
        return false;
    }

    denik::zapis(
        "zber",
        &format!(
            "Canvas rozpoznaný (ladovy {:.2}, mriežka modrá {:.2}, tmavá {:.2}) — {} B do fronty, čaká {}",
            crty.ladovy,
            crty.mriezka_modra,
            crty.mriezka_tmava,
            jpeg.len(),
            cakajucich + 1
        ),
    );
    true
}

/// Mimo Windowsu sa obrazovka ani okno hry nedajú čítať — zber tam nikdy nič
/// nezachytí. Appka na Linuxe nebeží, toto je len preto, aby `cargo check`
/// v CI prešiel nad celým súborom, nie nad jeho polovicou.
#[cfg(not(windows))]
fn pokus_o_zber(_app: &AppHandle, _n: &Nastavenia, posledna_bola_canvas: &mut bool) -> bool {
    *posledna_bola_canvas = false;
    false
}

/// Vlákno sledovača: klik myšou → 300 ms → prípadná snímka do fronty.
fn sledovac(app: AppHandle) {
    let mut bolo_dole = crate::tlacidlo_mysi_dole();
    let mut posledna_kontrola: Option<Instant> = None;
    let mut posledna_bola_canvas = false;
    let mut denny = nacitaj_denny_stav();

    loop {
        std::thread::sleep(Duration::from_millis(KROK_POLLINGU_MS));

        let teraz_dole = crate::tlacidlo_mysi_dole();
        let nabehova_hrana = teraz_dole && !bolo_dole;
        bolo_dole = teraz_dole;
        if !nabehova_hrana {
            continue;
        }

        let n = nastavenia_appky(&app);
        if !n.ucenie_canvas_zapnute {
            continue;
        }

        let od_poslednej_ms = posledna_kontrola
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(u64::MAX);
        if !fronta::ma_sa_kontrolovat(posledna_bola_canvas, od_poslednej_ms) {
            continue;
        }

        let cas = crate::tracker::unixovy_cas();
        if !denny.smie_zbierat(cas) {
            // Strop je denný, takže ďalšia kontrola má zmysel až zajtra —
            // ale stačí, že sa sem dostaneme pri ďalšom kliku; `smie_zbierat`
            // si prechod cez polnoc ustráži sám.
            continue;
        }

        // Zadanie: „odfotí okno hry 300 ms po kliku" — hra dovtedy dokreslí
        // výsledok ťahu (zafarbenú bunku, nový level painta).
        std::thread::sleep(Duration::from_millis(fronta::ONESKORENIE_PO_KLIKU_MS));
        posledna_kontrola = Some(Instant::now());

        if pokus_o_zber(&app, &n, &mut posledna_bola_canvas) {
            denny.zapocitaj(cas);
            uloz_denny_stav(&denny);
            if denny.zostava() == 0 {
                denik::zapis("zber", "denný strop 500 snímok vyčerpaný, dnes už nič");
            }
        }
    }
}

/// Vlákno odosielača: fronta z disku na server, jeden súbor po druhom.
fn odosielac(app: AppHandle) {
    let mut zlyhani: u32 = 0;
    loop {
        let n = nastavenia_appky(&app);
        // Vypnutý prepínač zastaví aj odosielanie — hráč, ktorý si zber
        // vypne, nečaká, že mu appka ešte pár hodín dosiela staré snímky.
        // Fronta na disku ostane nedotknutá a odíde, keď si to zapne späť.
        let pripravene = if n.ucenie_canvas_zapnute && n.chyba_konfiguracie().is_none() {
            cakajuce()
        } else {
            Vec::new()
        };

        let Some(cesta) = pripravene.first().cloned() else {
            std::thread::sleep(Duration::from_secs(ODOSIELAC_NECINNY_S));
            continue;
        };

        let Ok(jpeg) = std::fs::read(&cesta) else {
            // Súbor nám spod rúk zmizol (hráč si priečinok upratal) — nič sa
            // nedeje, ide sa ďalej.
            std::thread::sleep(Duration::from_millis(200));
            continue;
        };

        match klient::posli_canvas_zber(&n, jpeg) {
            Ok(()) => {
                zlyhani = 0;
                // Server snímku prijal a archivuje ju natrvalo — kópia vo
                // fronte na disku hráča už nemá čo robiť.
                let _ = std::fs::remove_file(&cesta);
                denik::zapis(
                    "zber",
                    &format!("odoslané, vo fronte ostáva {}", pripravene.len() - 1),
                );
                // Malý odstup medzi snímkami — server má limit požiadaviek
                // za minútu a fronta nikam nebeží.
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(e) => {
                zlyhani = zlyhani.saturating_add(1);
                let odstup = fronta::odstup_po_zlyhani(zlyhani);
                denik::zapis(
                    "zber",
                    &format!(
                        "odoslanie zlyhalo ({e}) — {} snímok čaká, skúsim o {odstup} s",
                        pripravene.len()
                    ),
                );
                std::thread::sleep(Duration::from_secs(odstup));
            }
        }
    }
}

/// Spustí obe vlákna. Volá sa raz pri štarte appky; prepínač v Nastaveniach
/// sa číta pri každom kliku a pri každom kole odosielača, takže zapnutie či
/// vypnutie zberu nepotrebuje reštart appky.
pub fn spusti(app: AppHandle) {
    let a = app.clone();
    std::thread::spawn(move || sledovac(a));
    std::thread::spawn(move || odosielac(app));
    denik::zapis(
        "zber",
        &format!(
            "automatické učenie Canvasu beží (fronta: {}, čaká {} snímok)",
            cesta_fronty().display(),
            cakajuce().len()
        ),
    );
}
