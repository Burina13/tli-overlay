//! Automatické učenie na pozadí (TLI 60 Frozen Canvas, TLI 70 + doska
//! Tartaru). Richard 19.10.2026: „kľudne nech si všetko, čo spravím, fotí
//! a na pozadí pošle agentovi na učenie, a na konci dňa nech sa naučí."
//!
//! Dve vlákna, obe sa štartujú raz pri spustení appky:
//!
//! * **sledovač** — pozerá sa na tlačidlá myši (`GetAsyncKeyState`, rovnaké
//!   čítanie stavovej tabuľky ako TLI 58, nič neodchytáva a do cesty kliku
//!   nevstupuje). Po stlačení počká `ONESKORENIE_PO_KLIKU_MS`, odfotí okno
//!   hry, zistí, KTORÚ obrazovku vidí, a odloží zmenšený JPEG do priečinka
//!   príslušnej fronty.
//! * **odosielač** — fronty na disku, jeden súbor po druhom na
//!   `?modul=canvas&zber=1` resp. `?zber=1`. Keď server nie je dostupný,
//!   súbor v priečinku ostane a skúša sa znova s rastúcim odstupom.
//!
//! Hráčovi sa pritom NEUKAZUJE NIČ — žiadny panel, žiadny toast, žiadne
//! čakanie na odpoveď servera. Stopa je denník appky a od TLI 70 aj riadok
//! „Učenie: …" v Nastaveniach (bod 5 zadania).
//!
//! ## Čo zmenila TLI 70 (`2026-10-20-1334-mcp.md`) a prečo
//!
//! Richard odohral 20.10.2026 celý deň Frozen Canvasu, server dostal 91
//! canvas analýz z Ctrl+W — a priečinok `canvas-zber` ostal PRÁZDNY. Z appky
//! sa pritom nedalo zistiť prečo: každá zo šiestich zamietacích podmienok
//! vracala `false` **ticho**, takže v denníku nebol ani jeden riadok. Odtiaľ
//! tri zmeny:
//!
//! 1. **Druhý zdroj pravdy o obrazovke.** Keď lokálna detekcia z pixelov
//!    (`canvas_detekcia`) nepovie „Canvas", appka sa oprie o to, čo server
//!    naposledy naozaj PREČÍTAL (`Stav::potvrdena_obrazovka`, platnosť
//!    `zber_stav::PLATNOST_POTVRDENIA_S` = 5 min). Rozhoduje
//!    `zber_stav::co_zbierat`.
//! 2. **Doska Tartaru sa zbiera tiež** (bod 3 zadania) — vlastná fronta
//!    `tartarus-zber`, vlastný endpoint, inak presne tá istá cesta.
//! 3. **Každé zamietnutie má dôvod, ktorý sa počíta a dá sa prečítať**
//!    (`StavZberu::dovody`) a raz za `ODSTUP_DIAGNOSTIKY_S` sa zhrnie do
//!    denníka. Dovtedy sa dalo len hádať.
//!
//! **Čo appka zámerne NEFOTÍ** (poradie je aj poradím kontrol v kóde, od
//! najlacnejšej):
//! 1. keď je prepínač „Učenie Canvas na pozadí" vypnutý;
//! 2. keď je vyčerpaný denný strop `MAX_ZA_DEN`;
//! 3. keď od poslednej kontroly neprešiel dosť dlhý čas
//!    (`zber_fronta::ma_sa_kontrolovat` — mimo rozpoznanej obrazovky
//!    nanajvýš raz za 2 s, nech sa pri bežnom hraní nefotí pri každom kliku
//!    v boji);
//! 4. keď okno hry **nie je v popredí** — bez tejto kontroly by appka
//!    odfotila obdĺžnik, na ktorom má hráč práve prehliadač alebo rozhovor;
//! 5. keď nad hrou práve svieti vlastný panel appky (bola by na snímke rada,
//!    nie čistá obrazovka hry) alebo keď práve beží ručná analýza. Od TLI 70
//!    sa na to **chvíľu počká** (`CAKANIE_NA_PANEL_MS`) namiesto okamžitého
//!    zahodenia: klik, ktorý panel zhasína, je ten istý klik, ktorý chceme
//!    odfotiť, a dovtedy sa stabilne strácal;
//! 6. keď snímka nie je ani Canvas, ani doska — a to je ten najčastejší prípad.
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
use tli_overlay_core::zber_stav::{CoZbierat, Podpis, Potvrdenie};

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

/// Ako dlho sa po kliku počká, kým zhasne panel appky / dobehne ručná
/// analýza (TLI 70). Klik, ktorým hráč zhasína radu, je ten istý klik,
/// ktorý chceme odfotiť — dovtedy sa stabilne strácal na `appka_prekaza`.
#[cfg(windows)]
const CAKANIE_NA_PANEL_MS: u64 = 900;
#[cfg(windows)]
const KROK_CAKANIA_NA_PANEL_MS: u64 = 100;

/// Odstup medzi dvoma odoslanými snímkami zberu — pozri komentár v
/// `kolo_odosielania`. Musí sedieť s `limity.zber_za_minutu` na serveri
/// (30/min = 2 s); 3 s dáva rezervu na to, že v tom istom kole ide Canvas
/// aj Tartarus.
const ODSTUP_MEDZI_ODOSLANIAMI_MS: u64 = 3_000;

/// Ako často sa do denníka zapíše zhrnutie dôvodov, prečo sa nefotilo
/// (TLI 70). Nie pri každom kliku — to by denník zaplavilo; nie nikdy —
/// presne tým sa porucha z 20.10.2026 nedala diagnostikovať.
const ODSTUP_DIAGNOSTIKY_S: u64 = 300;

/// Priečinok fronty vedľa nastavení appky. Každý druh zberu má vlastný.
pub fn cesta_fronty_pre(co: CoZbierat) -> PathBuf {
    let mut c = crate::stav::cesta_suboru();
    c.pop(); // preč "nastavenia.json"
    c.push(match co {
        CoZbierat::Canvas => "canvas-zber",
        CoZbierat::Tartarus => "tartarus-zber",
    });
    c
}

fn cesta_denneho_stavu(co: CoZbierat) -> PathBuf {
    cesta_fronty_pre(co).join("stav.json")
}

fn nacitaj_denny_stav(co: CoZbierat) -> fronta::DennyStav {
    std::fs::read_to_string(cesta_denneho_stavu(co))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn uloz_denny_stav(co: CoZbierat, s: &fronta::DennyStav) {
    // Best-effort, presne ako denník: keď sa strop nepodarí zapísať, appka
    // kvôli tomu nesmie prestať fungovať. Najhoršie, čo sa stane, je, že sa
    // po reštarte počítadlo vráti o kúsok späť.
    if let Ok(json) = serde_json::to_string(s) {
        let _ = std::fs::write(cesta_denneho_stavu(co), json);
    }
}

/// Zoradený zoznam čakajúcich snímok. Zoradenie je podstatné: mená nesú čas,
/// takže abecedné poradie = poradie vzniku, a server tak dostane ťahy v tom
/// poradí, v akom ich hráč spravil.
fn cakajuce(co: CoZbierat) -> Vec<PathBuf> {
    let Ok(polozky) = std::fs::read_dir(cesta_fronty_pre(co)) else {
        return Vec::new();
    };
    let mut subory: Vec<PathBuf> = polozky
        .flatten()
        .map(|p| p.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| fronta::je_subor_fronty_pre(co.kluc(), n))
                .unwrap_or(false)
        })
        .collect();
    subory.sort();
    subory
}

/// Koľko snímok práve čaká v oboch frontách — pre riadok „Učenie: …"
/// v Nastaveniach (TLI 70, bod 5).
pub fn pocty_cakajucich() -> (u32, u32) {
    (
        cakajuce(CoZbierat::Canvas).len() as u32,
        cakajuce(CoZbierat::Tartarus).len() as u32,
    )
}

fn nastavenia_appky(app: &AppHandle) -> Nastavenia {
    app.state::<Stav>().nastavenia.lock().unwrap().clone()
}

/// Posledné serverové potvrdenie obrazovky, prepočítané na „pred koľkými
/// sekundami" (TLI 70).
fn potvrdenie(app: &AppHandle) -> Option<Potvrdenie> {
    let stav = app.state::<Stav>();
    let m = stav.potvrdena_obrazovka.lock().unwrap();
    m.as_ref().map(|(co, kedy, podpis)| Potvrdenie {
        co: co.clone(),
        pred_s: kedy.elapsed().as_secs(),
        podpis: *podpis,
    })
}

fn zapocitaj_dovod(app: &AppHandle, dovod: &str) {
    app.state::<Stav>().stav_zberu.lock().unwrap().zapocitaj_dovod(dovod);
}

/// Zhrnutie dôvodov do denníka — nie častejšie než raz za
/// `ODSTUP_DIAGNOSTIKY_S` a len o tom, čo PRIBUDLO od posledného zhrnutia.
///
/// Prečo delta a nie celkový súčet: appka beží aj vtedy, keď hráč nehrá,
/// a vtedy by každý klik v prehliadači pridal `hra-nie-je-v-popredi`. Keby
/// sa vypisoval súčet, denník by sa plnil riadkami, ktoré nič nehovoria.
/// Z toho istého dôvodu sa zhrnutie, v ktorom je LEN „hra nie je v popredí"
/// (alebo „prepínač je vypnutý"), nevypisuje vôbec — to nie je porucha,
/// to je normálny stav mimo hry.
fn zhrn_dovody(
    app: &AppHandle,
    posledne: &mut Instant,
    predosle: &mut std::collections::BTreeMap<String, u32>,
) {
    if posledne.elapsed().as_secs() < ODSTUP_DIAGNOSTIKY_S {
        return;
    }
    *posledne = Instant::now();
    let dovody = {
        let stav = app.state::<Stav>();
        let s = stav.stav_zberu.lock().unwrap();
        s.dovody.clone()
    };
    let mut delta: Vec<(String, u32)> = Vec::new();
    for (k, v) in &dovody {
        let pred = predosle.get(k).copied().unwrap_or(0);
        if *v > pred {
            delta.push((k.clone(), v - pred));
        }
    }
    *predosle = dovody;
    const TICHE: [&str; 2] = ["hra-nie-je-v-popredi", "prepinac-ucenia-je-vypnuty"];
    if delta.is_empty() || delta.iter().all(|(k, _)| TICHE.contains(&k.as_str())) {
        return;
    }
    let zoznam: Vec<String> = delta.iter().map(|(k, v)| format!("{k}×{v}")).collect();
    let obr = potvrdenie(app)
        .map(|p| format!("{} (pred {} s)", p.co, p.pred_s))
        .unwrap_or_else(|| "žiadna".to_string());
    denik::zapis(
        "zber",
        &format!(
            "za posledných {ODSTUP_DIAGNOSTIKY_S} s sa nefotilo: {} | posledná potvrdená obrazovka: {obr}",
            zoznam.join(", ")
        ),
    );
}

/// Svieti práve nad hrou vlastný panel appky, alebo beží ručná analýza?
/// Vtedy sa nefotí — na snímke by bola rada appky, nie čistá obrazovka hry,
/// a presne taká snímka by učenie pokazila.
#[cfg(windows)]
fn appka_prekaza(app: &AppHandle) -> bool {
    let stav = app.state::<Stav>();
    let panel_svieti = stav.zobrazene_od.lock().unwrap().is_some();
    let analyza_bezi = stav.beh.lock().unwrap().is_some();
    panel_svieti || analyza_bezi
}

/// Počká, kým panel zhasne / analýza dobehne — najviac `CAKANIE_NA_PANEL_MS`
/// (TLI 70). Vracia `true`, keď je cesta voľná.
#[cfg(windows)]
fn pockaj_na_volnu_obrazovku(app: &AppHandle) -> bool {
    let zaciatok = Instant::now();
    while appka_prekaza(app) {
        if zaciatok.elapsed().as_millis() as u64 >= CAKANIE_NA_PANEL_MS {
            return false;
        }
        std::thread::sleep(Duration::from_millis(KROK_CAKANIA_NA_PANEL_MS));
    }
    true
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

/// Jeden pokus o zber po kliku. Vracia, ČO sa odfotilo (aby to volajúci
/// zarátal do správneho denného stropu), alebo `None`.
/// `posledna_bola_zbierana` sa prepíše podľa toho, čo sa na obrazovke našlo
/// (riadi odstup medzi kontrolami).
#[cfg(windows)]
fn pokus_o_zber(
    app: &AppHandle,
    n: &Nastavenia,
    posledna_bola_zbierana: &mut bool,
    smie: &dyn Fn(CoZbierat) -> bool,
) -> Option<CoZbierat> {
    if !pockaj_na_volnu_obrazovku(app) {
        zapocitaj_dovod(app, "panel-appky-svieti");
        return None;
    }
    let Some(hra) = okno_hry_v_popredi(&n.proces_hry) else {
        *posledna_bola_zbierana = false;
        zapocitaj_dovod(app, "hra-nie-je-v-popredi");
        return None;
    };

    let snimok = match snimka::zachyt(&hra.obdlznik) {
        Ok(s) => s,
        Err(_) => {
            *posledna_bola_zbierana = false;
            zapocitaj_dovod(app, "snimka-zlyhala");
            return None;
        }
    };
    if snimka::je_cierna(&snimok) {
        // Exkluzívny fullscreen — BitBlt vracia čierno. Ručná analýza na to
        // hráča upozorní, zber na pozadí sa k tomu nevyjadruje.
        *posledna_bola_zbierana = false;
        zapocitaj_dovod(app, "cierna-snimka-exkluzivny-fullscreen");
        return None;
    }

    let crty = canvas_detekcia::crty(&snimok.rgb, snimok.sirka, snimok.vyska);
    let teraz = Podpis {
        ladovy: crty.ladovy,
        mriezka_modra: crty.mriezka_modra,
        mriezka_tmava: crty.mriezka_tmava,
    };
    let Some(co) = tli_overlay_core::zber_stav::co_zbierat(
        crty.je_canvas(), potvrdenie(app).as_ref(), Some(teraz))
    else {
        *posledna_bola_zbierana = false;
        zapocitaj_dovod(app, "obrazovka-sa-nezbiera");
        return None;
    };
    *posledna_bola_zbierana = true;

    // Denný strop sa kontroluje PRED zápisom na disk a pre TEN druh, ktorý
    // sa naozaj rozpoznal. Keby sa kontroloval až po zápise (ako v prvej
    // verzii TLI 70), vyčerpaný strop Canvasu by snímku stále zapísal do
    // fronty a odosielač by ju aj tak poslal — strop by neplatil vôbec.
    if !smie(co) {
        zapocitaj_dovod(app, "denny-strop-vycerpany");
        return None;
    }

    let cakajucich = cakajuce(co).len();
    if !fronta::smie_pribudnut(cakajucich) {
        zapocitaj_dovod(app, "fronta-je-plna");
        denik::zapis(
            "zber",
            &format!(
                "fronta {} je plná ({cakajucich} snímok) — nič sa nemaže, len sa nepridáva",
                co.kluc()
            ),
        );
        return None;
    }

    let Ok(jpeg) = snimka::do_jpegu(&snimok, DLZKA_ZBERU, KVALITA_ZBERU) else {
        zapocitaj_dovod(app, "jpeg-zlyhal");
        return None;
    };

    let cas = crate::tracker::unixovy_cas();
    let priecinok = cesta_fronty_pre(co);
    if std::fs::create_dir_all(&priecinok).is_err() {
        zapocitaj_dovod(app, "priecinok-fronty-sa-neda-vytvorit");
        return None;
    }
    // Poradie v rámci sekundy — pri rýchlom klikaní vznikne za sekundu aj
    // niekoľko snímok a rovnaké meno by jednu z nich prepísalo.
    let mut poradie = 0u64;
    let cesta = loop {
        let c = priecinok.join(fronta::nazov_snimky_pre(co.kluc(), cas, poradie));
        if !c.exists() {
            break c;
        }
        poradie += 1;
        if poradie >= 10_000 {
            zapocitaj_dovod(app, "vycerpane-poradie-v-sekunde");
            return None;
        }
    };

    // Najprv `.tmp`, potom premenovanie — odosielač tak nikdy nenarazí na
    // polovičný súbor (`je_subor_fronty` `.tmp` neberie).
    let docasna = cesta.with_extension("jpg.tmp");
    if std::fs::write(&docasna, &jpeg).is_err() || std::fs::rename(&docasna, &cesta).is_err() {
        let _ = std::fs::remove_file(&docasna);
        zapocitaj_dovod(app, "zapis-na-disk-zlyhal");
        return None;
    }

    denik::zapis(
        "zber",
        &format!(
            "{} rozpoznaný (ladovy {:.2}, mriežka modrá {:.2}, tmavá {:.2}) — {} B do fronty, čaká {}",
            co.kluc(),
            crty.ladovy,
            crty.mriezka_modra,
            crty.mriezka_tmava,
            jpeg.len(),
            cakajucich + 1
        ),
    );
    Some(co)
}

/// Mimo Windowsu sa obrazovka ani okno hry nedajú čítať — zber tam nikdy nič
/// nezachytí. Appka na Linuxe nebeží, toto je len preto, aby `cargo check`
/// v CI prešiel nad celým súborom, nie nad jeho polovicou.
#[cfg(not(windows))]
fn pokus_o_zber(
    _app: &AppHandle,
    _n: &Nastavenia,
    posledna_bola_zbierana: &mut bool,
    _smie: &dyn Fn(CoZbierat) -> bool,
) -> Option<CoZbierat> {
    *posledna_bola_zbierana = false;
    None
}

/// Vlákno sledovača: klik myšou → 300 ms → prípadná snímka do fronty.
fn sledovac(app: AppHandle) {
    let mut bolo_dole = crate::tlacidlo_mysi_dole();
    let mut posledna_kontrola: Option<Instant> = None;
    let mut posledna_bola_zbierana = false;
    let mut denny_canvas = nacitaj_denny_stav(CoZbierat::Canvas);
    let mut denny_tartarus = nacitaj_denny_stav(CoZbierat::Tartarus);
    let mut posledna_diagnostika = Instant::now();
    let mut dovody_naposledy: std::collections::BTreeMap<String, u32> = Default::default();

    loop {
        std::thread::sleep(Duration::from_millis(KROK_POLLINGU_MS));
        zhrn_dovody(&app, &mut posledna_diagnostika, &mut dovody_naposledy);

        let teraz_dole = crate::tlacidlo_mysi_dole();
        let nabehova_hrana = teraz_dole && !bolo_dole;
        bolo_dole = teraz_dole;
        if !nabehova_hrana {
            continue;
        }

        let n = nastavenia_appky(&app);
        if !n.ucenie_canvas_zapnute {
            zapocitaj_dovod(&app, "prepinac-ucenia-je-vypnuty");
            continue;
        }

        let od_poslednej_ms = posledna_kontrola
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(u64::MAX);
        if !fronta::ma_sa_kontrolovat(posledna_bola_zbierana, od_poslednej_ms) {
            continue;
        }

        let cas = crate::tracker::unixovy_cas();
        // Strop je denný a per-druh; keď je vyčerpaný ten pre Canvas, doska
        // sa ešte zbierať môže (a naopak). Tu sa len ušetrí odfotenie, keď
        // sú vyčerpané OBA — pre konkrétny druh rozhoduje `smie` nižšie,
        // ešte pred zápisom na disk.
        if !denny_canvas.clone().smie_zbierat(cas) && !denny_tartarus.clone().smie_zbierat(cas) {
            zapocitaj_dovod(&app, "denny-strop-vycerpany");
            continue;
        }

        // Zadanie: „odfotí okno hry 300 ms po kliku" — hra dovtedy dokreslí
        // výsledok ťahu (zafarbenú bunku, nový level painta).
        std::thread::sleep(Duration::from_millis(fronta::ONESKORENIE_PO_KLIKU_MS));
        posledna_kontrola = Some(Instant::now());

        let smie_canvas = denny_canvas.clone().smie_zbierat(cas);
        let smie_tartarus = denny_tartarus.clone().smie_zbierat(cas);
        let smie = move |co: CoZbierat| match co {
            CoZbierat::Canvas => smie_canvas,
            CoZbierat::Tartarus => smie_tartarus,
        };

        if let Some(co) = pokus_o_zber(&app, &n, &mut posledna_bola_zbierana, &smie) {
            let denny = match co {
                CoZbierat::Canvas => &mut denny_canvas,
                CoZbierat::Tartarus => &mut denny_tartarus,
            };
            denny.zapocitaj(cas);
            uloz_denny_stav(co, denny);
            if denny.zostava() == 0 {
                denik::zapis(
                    "zber",
                    &format!("denný strop {} snímok ({}) vyčerpaný, dnes už nič",
                             fronta::MAX_ZA_DEN, co.kluc()),
                );
            }
        }
    }
}

/// Jedno kolo odosielania jednej fronty. Vracia `true`, keď sa niečo poslalo
/// (volajúci vtedy nespí dlho).
fn kolo_odosielania(app: &AppHandle, n: &Nastavenia, co: CoZbierat, zlyhani: &mut u32) -> bool {
    let pripravene = cakajuce(co);
    let Some(cesta) = pripravene.first().cloned() else {
        return false;
    };
    let Ok(jpeg) = std::fs::read(&cesta) else {
        // Súbor nám spod rúk zmizol (hráč si priečinok upratal) — nič sa
        // nedeje, ide sa ďalej.
        std::thread::sleep(Duration::from_millis(200));
        return true;
    };

    let vysledok = match co {
        CoZbierat::Canvas => klient::posli_canvas_zber(n, jpeg),
        CoZbierat::Tartarus => klient::posli_tartarus_zber(n, jpeg),
    };

    match vysledok {
        Ok(()) => {
            *zlyhani = 0;
            // Server snímku prijal a archivuje ju natrvalo — kópia vo
            // fronte na disku hráča už nemá čo robiť.
            let _ = std::fs::remove_file(&cesta);
            {
                let stav = app.state::<Stav>();
                stav.stav_zberu.lock().unwrap().zapis_uspech(co);
            }
            denik::zapis(
                "zber",
                &format!("{} odoslané, vo fronte ostáva {}", co.kluc(), pripravene.len() - 1),
            );
            // Odstup medzi snímkami. TLI 70 ho zdvihla z 500 ms na 3 s:
            // KAŽDÁ takáto snímka stojí na serveri jedno vision volanie
            // (na rozdiel od minimapy, ktorá sa len uloží na disk), server
            // má na zber vlastný kôš `zber_za_minutu` = 30 a fronta nikam
            // nebeží. Pri 500 ms by appka narazila na 429 po pol minúte.
            std::thread::sleep(Duration::from_millis(ODSTUP_MEDZI_ODOSLANIAMI_MS));
        }
        Err(e) => {
            *zlyhani = zlyhani.saturating_add(1);
            let odstup = fronta::odstup_po_zlyhani(*zlyhani);
            {
                let stav = app.state::<Stav>();
                stav.stav_zberu.lock().unwrap().zapis_chybu(co, &e);
            }
            denik::zapis(
                "zber",
                &format!(
                    "{} odoslanie zlyhalo ({e}) — {} snímok čaká, skúsim o {odstup} s",
                    co.kluc(),
                    pripravene.len()
                ),
            );
            std::thread::sleep(Duration::from_secs(odstup));
        }
    }
    true
}

/// Vlákno odosielača: fronty z disku na server, jeden súbor po druhom.
fn odosielac(app: AppHandle) {
    // Každá fronta má VLASTNÉ počítadlo zlyhaní. So spoločným by úspech
    // jednej vynuloval backoff druhej: snímka, ktorú server trvale odmieta,
    // by sa skúšala donekonečna v najkratšom odstupe.
    let mut zlyhani_canvas: u32 = 0;
    let mut zlyhani_tartarus: u32 = 0;
    loop {
        let n = nastavenia_appky(&app);
        // Vypnutý prepínač zastaví aj odosielanie — hráč, ktorý si zber
        // vypne, nečaká, že mu appka ešte pár hodín dosiela staré snímky.
        // Fronta na disku ostane nedotknutá a odíde, keď si to zapne späť.
        if !n.ucenie_canvas_zapnute || n.chyba_konfiguracie().is_some() {
            std::thread::sleep(Duration::from_secs(ODOSIELAC_NECINNY_S));
            continue;
        }

        let mut nieco_islo =
            kolo_odosielania(&app, &n, CoZbierat::Canvas, &mut zlyhani_canvas);
        nieco_islo |= kolo_odosielania(&app, &n, CoZbierat::Tartarus, &mut zlyhani_tartarus);
        if !nieco_islo {
            std::thread::sleep(Duration::from_secs(ODOSIELAC_NECINNY_S));
        }
    }
}

/// Spustí obe vlákna. Volá sa raz pri štarte appky; prepínač v Nastaveniach
/// sa číta pri každom kliku a pri každom kole odosielača, takže zapnutie či
/// vypnutie zberu nepotrebuje reštart appky.
pub fn spusti(app: AppHandle) {
    let a = app.clone();
    std::thread::spawn(move || sledovac(a));
    let b = app.clone();
    std::thread::spawn(move || odosielac(b));
    let (c, t) = pocty_cakajucich();
    denik::zapis(
        "zber",
        &format!(
            "automatické učenie beží (fronty: {} / {}, čaká Canvas {c}, Tartarus {t})",
            cesta_fronty_pre(CoZbierat::Canvas).display(),
            cesta_fronty_pre(CoZbierat::Tartarus).display(),
        ),
    );
}
