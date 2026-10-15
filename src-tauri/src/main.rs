// Appka beží bez konzolového okna — inak by pri každom spustení (aj z tray ikony)
// vyskočilo čierne okno `cmd`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod guide;
mod klient;
#[cfg(windows)]
mod okno;
#[cfg(windows)]
mod snimka;
mod stav;
mod tracker;

use serde_json::json;
use stav::Stav;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};
use tli_overlay_core::nastavenia::Nastavenia;
use tli_overlay_core::suradnice::OknoFyz;
use tli_overlay_core::{api, suradnice, zmensenie};

/// Okno overlayu. Je priehľadné, bez rámu, vždy navrchu a priechodné pre myš —
/// hráč cezeň klikne do hry, akoby tam nebolo.
const OVERLAY: &str = "overlay";
/// Okno nastavení. Obyčajné okno s rámom, otvára sa z tray ikony.
const NASTAVENIA: &str = "nastavenia";
/// Hlavné okno so záložkami Tracker/Solvery/Guide/Nastavenia. Otvára sa z tray
/// ikony alebo položkou „Otvoriť".
const HLAVNE: &str = "hlavne";
/// Mini-overlay počas mapy (TLI 11, krok 5): malý panel vždy navrchu s FE tejto
/// mapy, časom a FE/h. Predvolene vypnutý, zapína sa v záložke Tracker.
const MINI: &str = "mini";

/// Keď sa okno hry nenájde, chybová hláška sa aj tak musí mať kde zobraziť —
/// overlay sa preto postaví na túto veľkosť do stredu hornej časti obrazovky.
const CHYBA_SIRKA: u32 = 760;
const CHYBA_VYSKA: u32 = 300;

// ─────────────────────────────────────────────────────────────────────────────
// Klávesové skratky
// ─────────────────────────────────────────────────────────────────────────────

/// Escape sa registruje LEN na čas, kým je overlay viditeľný, a hneď po skrytí
/// sa zruší. Globálna skratka totiž klávesu hre zoberie — a Esc je v Torchlighte
/// zatváranie menu, takže držať ju zabratú stále by hru reálne obmedzilo.
fn esc() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

/// Prepíše globálnu skratku na novú (Tartarus — hlavná, historicky jediná).
/// Pri nezmyselnom zápise sa vracia k F8, nech appka neostane bez jedinej
/// ovládacej klávesy. Zruší VŠETKY skratky vrátane canvas — volajúci preto
/// musí po nej zavolať aj `prehlas_hotkey_canvas`, inak by canvas ostal
/// nezaregistrovaný (pozri `uloz_nastavenia` a `main`).
pub fn prehlas_hotkey(app: &AppHandle, novy: &str) -> Result<(), String> {
    let skratky = app.global_shortcut();
    let _ = skratky.unregister_all();

    if skratky.register(novy).is_err() {
        let _ = skratky.register("F8");
        return Err(format!(
            "Skratku „{novy}“ Windows neprijal (môže ju držať iná appka). Vraciam sa k F8."
        ));
    }
    Ok(())
}

/// Druhá skratka — Frozen Canvas (TLI 4b). NEruší existujúce skratky (na
/// rozdiel od `prehlas_hotkey`), len pridáva/nahrádza svoju vlastnú — volá sa
/// vždy AŽ PO `prehlas_hotkey`.
pub fn prehlas_hotkey_canvas(app: &AppHandle, novy: &str) -> Result<(), String> {
    let skratky = app.global_shortcut();
    let _ = skratky.unregister(novy);

    if skratky.register(novy).is_err() {
        let _ = skratky.register("F9");
        return Err(format!(
            "Skratku pre Frozen Canvas „{novy}“ Windows neprijal (môže ju držať iná appka \
alebo kolíduje s hlavnou skratkou). Vraciam sa k F9."
        ));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Overlay
// ─────────────────────────────────────────────────────────────────────────────

/// Mierka monitora, na ktorom leží okno hry. Počíta sa cez `suradnice::monitor_okna`,
/// teda cez tú istú funkciu, ktorú pokrývajú jednotkové testy — nie zvlášť „naostro“
/// a zvlášť „v teste“.
fn mierka_okna(app: &AppHandle, obd: &OknoFyz) -> f64 {
    let Some(overlay) = app.get_webview_window(OVERLAY) else {
        return 1.0;
    };
    let monitory: Vec<suradnice::Monitor> = overlay
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|m| suradnice::Monitor {
            x: m.position().x,
            y: m.position().y,
            sirka: m.size().width,
            vyska: m.size().height,
            mierka: m.scale_factor(),
        })
        .collect();

    suradnice::monitor_okna(obd, &monitory)
        .map(|m| m.mierka)
        .unwrap_or_else(|| overlay.scale_factor().unwrap_or(1.0))
}

/// Postaví overlay presne nad okno hry a ukáže ho. Pozícia aj veľkosť idú vo
/// FYZICKÝCH pixeloch — logické by sa na monitore so 150 % DPI rozišli s hrou.
fn ukaz_overlay_nad(app: &AppHandle, obd: &OknoFyz) {
    if let Some(overlay) = app.get_webview_window(OVERLAY) {
        let _ = overlay.set_position(PhysicalPosition::new(obd.x, obd.y));
        let _ = overlay.set_size(PhysicalSize::new(obd.sirka, obd.vyska));
        let _ = overlay.set_ignore_cursor_events(true);
        let _ = overlay.show();
        let _ = app.global_shortcut().register(esc());
        *app.state::<Stav>().zobrazene_od.lock().unwrap() = Some(Instant::now());
    }
}

/// Chybu musí hráč vidieť aj vtedy, keď sa okno hry vôbec nenašlo — vtedy nie je
/// kam overlay postaviť, tak sa dá navrch hlavného monitora.
fn ukaz_chybu(app: &AppHandle, obd: Option<&OknoFyz>, sprava: &str) {
    match obd {
        Some(o) => ukaz_overlay_nad(app, o),
        None => {
            if let Some(overlay) = app.get_webview_window(OVERLAY) {
                let (x, y) = overlay
                    .primary_monitor()
                    .ok()
                    .flatten()
                    .map(|m| {
                        (
                            m.position().x + (m.size().width as i32 - CHYBA_SIRKA as i32) / 2,
                            m.position().y + 80,
                        )
                    })
                    .unwrap_or((80, 80));
                ukaz_overlay_nad(
                    app,
                    &OknoFyz { x, y, sirka: CHYBA_SIRKA, vyska: CHYBA_VYSKA },
                );
            }
        }
    }
    let _ = app.emit("stav", json!({ "faza": "chyba", "sprava": sprava }));
}

/// Skryje overlay a uvoľní Esc späť hre.
fn skry(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window(OVERLAY) {
        let _ = overlay.hide();
    }
    let _ = app.global_shortcut().unregister(esc());
    *app.state::<Stav>().zobrazene_od.lock().unwrap() = None;
}

/// Overlay sa po `autoskrytie_s` sekundách schová sám. Porovnáva sa značka času,
/// kedy bol zobrazený — inak by starší časovač zhasol novší, práve zobrazený overlay.
fn naplanuj_skrytie(app: AppHandle, znacka: Instant, sekundy: u64) {
    if sekundy == 0 {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(sekundy));
        let drzi = *app.state::<Stav>().zobrazene_od.lock().unwrap();
        if drzi == Some(znacka) {
            skry(&app);
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Hlavný beh: hotkey → snímka → server → overlay
// ─────────────────────────────────────────────────────────────────────────────

/// Ktorý modul sa má analyzovať — jediný rozdiel je endpoint/vision prompt na
/// serveri a tvar odpovede, zvyšok (snímka, overlay nad oknom hry, zámok proti
/// dvojitému stlačeniu) je spoločné (TLI 4b, `2026-10-15-0025-mcp.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modul {
    Tartarus,
    Canvas,
}

#[cfg(windows)]
fn spusti_analyzu(app: AppHandle, modul: Modul) {
    let nastavenia = {
        let stav = app.state::<Stav>();
        let mut n = stav.nastavenia.lock().unwrap();
        n.normalizuj();
        n.clone()
    };

    if let Some(chyba) = nastavenia.chyba_konfiguracie() {
        ukaz_chybu(&app, None, &chyba);
        return;
    }

    // Dvojité stlačenie hotkey (ktoréhokoľvek z dvoch) nemá poslať dve
    // požiadavky — server púšťa 30 za minútu a zámok je zdieľaný medzi
    // Tartarus aj Frozen Canvas, nemajú bežať naraz nad tým istým oknom.
    {
        let stav = app.state::<Stav>();
        let mut beh = stav.beh.lock().unwrap();
        if *beh {
            return;
        }
        *beh = true;
    }

    let koniec = |app: &AppHandle| {
        *app.state::<Stav>().beh.lock().unwrap() = false;
    };

    let okno_hry = match okno::najdi_hru(&nastavenia.proces_hry) {
        Some(o) => Some(o),
        None => okno::popredie(),
    };
    let Some(okno_hry) = okno_hry else {
        ukaz_chybu(
            &app,
            None,
            "Nenašiel som okno hry. Spusti Torchlight: Infinite a nechaj ho otvorené; \
ak sa hra volá inak, dopíš jej názov v nastaveniach do poľa „Okno hry“.",
        );
        koniec(&app);
        return;
    };
    let obdlznik = okno_hry.obdlznik;

    // Fotí sa EŠTE PRED zobrazením overlayu — inak by bol overlay na snímke
    // a server by rozpoznával vlastné rámčeky namiesto dosky.
    let snimok = match snimka::zachyt(&obdlznik) {
        Ok(s) => s,
        Err(e) => {
            ukaz_chybu(&app, Some(&obdlznik), &e);
            koniec(&app);
            return;
        }
    };

    if snimka::je_cierna(&snimok) {
        ukaz_chybu(
            &app,
            Some(&obdlznik),
            "Obrazovka hry prišla čierna — to robí exkluzívny fullscreen. \
V hre prepni Nastavenia → Zobrazenie → Režim okna na „Okno bez okrajov“ \
(borderless windowed) alebo „V okne“ a skús to znova.",
        );
        koniec(&app);
        return;
    }

    let mierka = mierka_okna(&app, &obdlznik);
    ukaz_overlay_nad(&app, &obdlznik);
    let _ = app.emit("stav", json!({ "faza": "analyzujem" }));

    // Kódovanie JPEGu aj sieť idú mimo hlavného vlákna — inak by sa na tých
    // pár sekúnd zasekla celá appka vrátane tray ikony.
    std::thread::spawn(move || {
        match modul {
            Modul::Tartarus => {
                let vysledok = snimka::do_jpegu(&snimok, zmensenie::MAX_DLZKA, 80)
                    .and_then(|jpeg| klient::posli_analyzu(&nastavenia, jpeg));
                match vysledok {
                    Ok(v) => {
                        let obdlzniky: Vec<_> = api::pouzitelne_polia(&v.analyza)
                            .into_iter()
                            .map(|pole| {
                                let css = suradnice::pole_na_css(pole, &obdlznik, mierka);
                                let (sx, sy) = suradnice::stred_css(&css);
                                json!({
                                    "index": pole.index,
                                    "x": css.x, "y": css.y, "w": css.w, "h": css.h,
                                    "stredX": sx, "stredY": sy,
                                    "neiste": api::je_neiste(&v.analyza, pole.index),
                                })
                            })
                            .collect();

                        let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obdlznik, mierka);
                        let _ = app.emit(
                            "stav",
                            json!({
                                "faza": "hotovo",
                                "analyza": v.analyza,
                                "obdlzniky": obdlzniky,
                                "mierka": mierka,
                                "sirka": sirka_log,
                                "vyska": vyska_log,
                                "trvanie_ms": v.trvanie_ms,
                            }),
                        );
                    }
                    Err(e) => {
                        let _ = app.emit("stav", json!({ "faza": "chyba", "sprava": e }));
                    }
                }
            }
            Modul::Canvas => {
                let vysledok = snimka::do_jpegu(&snimok, zmensenie::MAX_DLZKA, 80)
                    .and_then(|jpeg| klient::posli_analyzu_canvas(&nastavenia, jpeg));
                match vysledok {
                    // Zber dát (TLI 4c): server ULOŽIL stav (robí to vždy, bez
                    // ohľadu na istotu solvera — nezmenené od TLI 4b), takže
                    // KAŽDÉ Ok() tu znamená "uložené", nezávisle od toho, či
                    // solver vôbec niečo navrhol. Žiadna rada sa nevykresľuje —
                    // zámerne, nech zber dát nepokazí nehádaný smer tým, že by
                    // vnucoval neoverenú radu.
                    Ok(_v) if nastavenia.zber_dat_zapnute => {
                        let pocet = {
                            let stav = app.state::<Stav>();
                            let mut p = stav.pocitadlo_zber.lock().unwrap();
                            *p += 1;
                            *p
                        };
                        let _ = app.emit("stav", json!({ "faza": "zber_ulozene", "pocet": pocet }));
                    }
                    Ok(v) => {
                        // Frozen Canvas (TLI 4b): KAŽDÝ krok nesie DVA páry
                        // súradníc (zdroj v ruke + cieľ na plátne), prepočítané
                        // tou istou funkciou ako Tartarus políčka.
                        let kroky: Vec<_> = api::pouzitelne_kroky(&v.analyza)
                            .into_iter()
                            .map(|k| {
                                let zb = k.zdroj.box_.expect("filtrované pouzitelne_kroky");
                                let cb = k.ciel.box_.expect("filtrované pouzitelne_kroky");
                                let zdroj_css = suradnice::box01_na_css(zb.x, zb.y, zb.w, zb.h, &obdlznik, mierka);
                                let ciel_css = suradnice::box01_na_css(cb.x, cb.y, cb.w, cb.h, &obdlznik, mierka);
                                json!({
                                    "poradie": k.poradie,
                                    "dovod": k.dovod.clone(),
                                    "zdroj": {"x": zdroj_css.x, "y": zdroj_css.y,
                                              "w": zdroj_css.w, "h": zdroj_css.h},
                                    "ciel": {"x": ciel_css.x, "y": ciel_css.y,
                                             "w": ciel_css.w, "h": ciel_css.h},
                                })
                            })
                            .collect();

                        let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obdlznik, mierka);
                        let _ = app.emit(
                            "stav",
                            json!({
                                "faza": "hotovo_canvas",
                                "analyza": v.analyza,
                                "kroky": kroky,
                                "mierka": mierka,
                                "sirka": sirka_log,
                                "vyska": vyska_log,
                                "trvanie_ms": v.trvanie_ms,
                            }),
                        );
                    }
                    Err(e) => {
                        let _ = app.emit("stav", json!({ "faza": "chyba", "sprava": e }));
                    }
                }
            }
        }

        *app.state::<Stav>().beh.lock().unwrap() = false;

        let znacka = *app.state::<Stav>().zobrazene_od.lock().unwrap();
        if let Some(znacka) = znacka {
            naplanuj_skrytie(app.clone(), znacka, nastavenia.autoskrytie_s);
        }
    });
}

/// Mimo Windowsu appka nevie odfotiť okno hry — kompiluje sa len preto, aby sa
/// dali spúšťať testy a `cargo check` na Linuxe v CI.
#[cfg(not(windows))]
fn spusti_analyzu(app: AppHandle, _modul: Modul) {
    ukaz_chybu(&app, None, "Appka funguje len na Windowse.");
}


// ─────────────────────────────────────────────────────────────────────────────
// Farm tracker (TLI 11/11b)
// ─────────────────────────────────────────────────────────────────────────────

/// Prečíta nové riadky logu a vráti hotový prehľad pre záložku Tracker.
///
/// Volá to UI v pravidelnom takte — jedno volanie teda musí byť lacné. Preto sa
/// číta len prírastok od poslednej pozície a prepočet beží nad už rozparsovanými
/// behmi, nie nad celým logom odznova.
#[tauri::command]
fn tracker_prehlad(
    stav: tauri::State<Stav>,
    tracker: tauri::State<tracker::Tracker>,
) -> Result<tracker::PrehladProUi, String> {
    let n = stav.nastavenia.lock().unwrap().clone();
    let mut t = tracker.0.lock().unwrap();

    if !n.tracker_zapnuty {
        return Ok(tracker::prehlad_pre_ui(&t, false, n.tracker_dan));
    }

    match tracker::najdi_log(&n.cesta_logu) {
        Some(cesta) => {
            t.chyba = match tracker::precitaj(&mut t, &cesta) {
                Ok(_) => None,
                Err(e) => Some(e),
            };
        }
        None => {
            t.chyba = Some(
                "Herný log sa nenašiel. Zapni si v hre Settings → Other Settings → \"Enable Log\"                  a ak hra nie je v štandardnom priečinku, zadaj cestu k `UE_game.log` v nastaveniach."
                    .to_string(),
            );
        }
    }

    Ok(tracker::prehlad_pre_ui(&t, true, n.tracker_dan))
}

/// Export histórie máp do CSV. Vracia obsah súboru — uloženie rieši UI.
#[tauri::command]
fn tracker_csv(
    stav: tauri::State<Stav>,
    tracker: tauri::State<tracker::Tracker>,
) -> Result<String, String> {
    let n = stav.nastavenia.lock().unwrap().clone();
    let t = tracker.0.lock().unwrap();
    let prehlad = tracker::prehlad_pre_ui(&t, n.tracker_zapnuty, n.tracker_dan);
    Ok(tracker::csv(&prehlad.behy))
}

/// Cesta, ktorú appka navrhne do poľa „Cesta k logu" v záložke Tracker, keď
/// je uložená hodnota prázdna — Steam register/vdf, inak krátky zoznam
/// známych miest (TLI 11c). Nič nezapisuje, to urobí `uloz_cestu_logu`.
#[tauri::command]
fn tracker_navrh_cestu() -> Option<String> {
    tracker::najdi_log("").map(|p| p.display().to_string())
}

/// Stav cesty pre ikonku a hlášku v záložke Tracker: existuje/veľkosť/vek a
/// upozornenie, keď log vyzerá, že v hre nie je zapnuté „Enable Log" (TLI 11c).
#[tauri::command]
fn tracker_stav_cesty(rucna: String) -> tracker::StavCesty {
    tracker::stav_cesty(&rucna)
}

/// Tlačidlo „Vybrať…" pri poli s cestou k logu — natívny výber súboru.
/// `None`, keď hráč dialóg zavrie bez výberu.
#[tauri::command]
fn tracker_vyber_subor(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .add_filter("Herný log", &["log"])
        .blocking_pick_file()
        .map(|cesta| cesta.to_string())
}

/// Uloženie cesty k logu zo záložky Tracker. Samostatný príkaz (nie uloženie
/// celých nastavení) z rovnakého dôvodu ako `uloz_temu`/`uloz_prepinac` —
/// nesmie prepísať kľúč ani skratky zastaraným objektom z inej časti UI.
#[tauri::command]
fn uloz_cestu_logu(stav: tauri::State<Stav>, cesta: String) -> Result<String, String> {
    let mut n = stav.nastavenia.lock().unwrap();
    n.cesta_logu = cesta;
    n.normalizuj();
    let hodnota = n.cesta_logu.clone();
    let kopia = n.clone();
    drop(n);
    stav::uloz(&kopia)?;
    Ok(hodnota)
}

/// Prepnutie svetlej a tmavej témy. Je to samostatný príkaz (a nie uloženie
/// celých nastavení), aby prepínač v hlavičke nemohol omylom prepísať kľúč,
/// adresu servera ani skratky tým, že by poslal zastaraný objekt.
#[tauri::command]
fn uloz_temu(stav: tauri::State<Stav>, tema: String) -> Result<String, String> {
    let mut n = stav.nastavenia.lock().unwrap();
    n.tema = tema;
    n.normalizuj();
    let hodnota = n.tema.clone();
    let kopia = n.clone();
    drop(n);
    stav::uloz(&kopia)?;
    Ok(hodnota)
}

// ─────────────────────────────────────────────────────────────────────────────
// Guide (TLI 11, krok 4) — sprievodca Erika 1 zo servera, uložený lokálne
// ─────────────────────────────────────────────────────────────────────────────

/// Vráti sprievodcu tak, ako ho má appka práve uloženého. NIČ nesťahuje —
/// záložka sa musí otvoriť aj bez siete a aj vtedy, keď server nebeží.
#[tauri::command]
fn guide_data(g: tauri::State<guide::StavGuide>) -> Result<guide::GuideProUi, String> {
    let ulozeny = g.0.lock().unwrap();
    Ok(guide::pre_ui(&ulozeny, tracker::unixovy_cas()))
}

/// Pozrie sa na server, či nie je novší sprievodca, a keď je, stiahne ho a uloží.
/// Vracia ten istý tvar ako `guide_data`, takže UI po synchronizácii len
/// prekreslí to, čo dostane — nemusí volať dva príkazy za sebou.
#[tauri::command]
fn guide_sync(
    stav: tauri::State<Stav>,
    g: tauri::State<guide::StavGuide>,
) -> Result<guide::GuideProUi, String> {
    let n = stav.nastavenia.lock().unwrap().clone();
    if let Some(chyba) = n.chyba_konfiguracie() {
        return Err(chyba);
    }

    // Parameter sa berie z TEJ verzie, ktorú appka práve má — keď je rovnaká,
    // server vráti 204 a neprenáša sa ani bajt kapitol a ikon.
    let parameter = {
        let ulozeny = g.0.lock().unwrap();
        tli_overlay_core::guide::parameter_verzie(&ulozeny)
    };

    match klient::stiahni_guide(&n, &parameter)? {
        // 204 — máme najnovšie. Čas poslednej kontroly sa zámerne NEPREPISUJE:
        // „stiahnuty" hovorí, odkedy je text, nie kedy sme sa naposledy pýtali.
        None => guide_data(g),
        Some(telo) => {
            let novy = tli_overlay_core::guide::z_odpovede(&telo, tracker::unixovy_cas())?;
            guide::uloz(&novy)?;
            let pohlad = guide::pre_ui(&novy, tracker::unixovy_cas());
            *g.0.lock().unwrap() = novy;
            Ok(pohlad)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Synchronizácia s partiou a mini-overlay (TLI 11, krok 5)
// ─────────────────────────────────────────────────────────────────────────────

/// Pošle partii mapy, ktoré ešte nepozná. Vracia vetu pre UI.
///
/// Volá sa z UI (nie z vlákna na pozadí): tracker aj tak číta log len vtedy,
/// keď je hlavné okno otvorené, takže druhý mechanizmus by tu nič nepridal.
#[tauri::command]
fn tracker_sync_teraz(
    stav: tauri::State<Stav>,
    tracker: tauri::State<tracker::Tracker>,
) -> Result<String, String> {
    let n = stav.nastavenia.lock().unwrap().clone();
    if !n.tracker_sync {
        return Ok("Posielanie výsledkov partii je vypnuté.".to_string());
    }
    if let Some(chyba) = n.chyba_konfiguracie() {
        return Err(chyba);
    }

    let vysledky = {
        let t = tracker.0.lock().unwrap();
        tracker::prehlad_pre_ui(&t, n.tracker_zapnuty, n.tracker_dan).behy
    };

    // Dávka je zhora obmedzená rovnako ako na serveri (200) — pri prvom zapnutí
    // má hráč v logu kľudne stovky máp a nemá zmysel z toho robiť jednu obrovskú
    // požiadavku; zvyšok odíde pri ďalšom kole.
    let davka = {
        let poslane = stav.poslane_behy.lock().unwrap();
        tli_overlay_core::tracker::sync::vyber_neposlane(&vysledky, &poslane, 200)
    };
    if davka.is_empty() {
        return Ok("Partia už má všetko, čo si odfarmil.".to_string());
    }

    let telo = tli_overlay_core::tracker::sync::telo_poziadavky(&davka, env!("CARGO_PKG_VERSION"));
    let odpoved = klient::posli_tracker_sync(&n, &telo)?;
    let vysledok = tli_overlay_core::tracker::sync::z_odpovede(&odpoved)?;

    // Za odoslané sa považuje aj to, čo server „preskočil" — mal to už od nás,
    // takže posielať to znova nemá zmysel.
    let mut poslane = stav.poslane_behy.lock().unwrap();
    for b in &davka {
        poslane.insert(b.kluc.clone());
    }

    Ok(tli_overlay_core::tracker::sync::zhrnutie(&vysledok))
}

/// Prepínače v záložke Tracker. Samostatný príkaz (nie uloženie celých
/// nastavení) z toho istého dôvodu ako `uloz_temu`: checkbox nesmie prepísať
/// kľúč ani skratky zastaraným objektom.
#[tauri::command]
fn uloz_prepinac(
    app: AppHandle,
    stav: tauri::State<Stav>,
    meno: String,
    zapnuty: bool,
) -> Result<bool, String> {
    {
        let mut n = stav.nastavenia.lock().unwrap();
        match meno.as_str() {
            "tracker_sync" => n.tracker_sync = zapnuty,
            "mini_overlay" => n.mini_overlay = zapnuty,
            "tracker_zapnuty" => n.tracker_zapnuty = zapnuty,
            "tracker_dan" => n.tracker_dan = zapnuty,
            ine => return Err(format!("Neznámy prepínač: {ine}")),
        }
        n.normalizuj();
        let kopia = n.clone();
        drop(n);
        stav::uloz(&kopia)?;
    }

    if meno == "mini_overlay" {
        nastav_mini(&app, zapnuty);
    }
    Ok(zapnuty)
}

/// Ukáže alebo schová mini-overlay. Keď okno neexistuje (nemalo by sa stať),
/// appka sa nezhodí — panel je doplnok, nie podmienka chodu.
fn nastav_mini(app: &AppHandle, zapnuty: bool) {
    if let Some(okno) = app.get_webview_window(MINI) {
        if zapnuty {
            let _ = okno.show();
        } else {
            let _ = okno.hide();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Príkazy z okna nastavení
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
fn nacitaj_nastavenia(stav: tauri::State<Stav>) -> Result<Nastavenia, String> {
    Ok(stav.nastavenia.lock().unwrap().clone())
}

#[tauri::command]
fn uloz_nastavenia(
    app: AppHandle,
    stav: tauri::State<Stav>,
    mut nove: Nastavenia,
) -> Result<(), String> {
    nove.normalizuj();
    stav::uloz(&nove)?;
    let hotkey = nove.hotkey.clone();
    let hotkey_canvas = nove.hotkey_canvas.clone();
    *stav.nastavenia.lock().unwrap() = nove;
    // `prehlas_hotkey` ruší VŠETKY skratky — canvas sa preto registruje až
    // po nej, nie súbežne/pred.
    let chyba1 = prehlas_hotkey(&app, &hotkey).err();
    let chyba2 = prehlas_hotkey_canvas(&app, &hotkey_canvas).err();
    match (chyba1, chyba2) {
        (None, None) => Ok(()),
        (Some(a), None) => Err(a),
        (None, Some(b)) => Err(b),
        (Some(a), Some(b)) => Err(format!("{a} {b}")),
    }
}

#[tauri::command]
fn otestuj_pripojenie(stav: tauri::State<Stav>) -> Result<String, String> {
    let n = stav.nastavenia.lock().unwrap().clone();
    if let Some(chyba) = n.chyba_konfiguracie() {
        return Err(chyba);
    }
    klient::test_pripojenia(&n)
}

#[tauri::command]
fn skry_overlay(app: AppHandle) -> Result<(), String> {
    skry(&app);
    Ok(())
}

/// Ukáže a zaostrí dané okno (nastavenia/hlavné) — spoločné pre tray menu aj
/// pre príkaz volaný z JS (tlačidlo „Otvoriť nastavenia“ v záložke Nastavenia
/// hlavného okna).
fn ukaz_a_zaostri(app: &AppHandle, label: &str) {
    if let Some(okno) = app.get_webview_window(label) {
        let _ = okno.show();
        let _ = okno.unminimize();
        let _ = okno.set_focus();
    }
}

#[tauri::command]
fn otvor_nastavenia(app: AppHandle) -> Result<(), String> {
    ukaz_a_zaostri(&app, NASTAVENIA);
    Ok(())
}

/// Otvára sa VÝHRADNE adresa pod nastaveným serverom. Webview nemá dôvod vedieť
/// otvoriť hocijaký odkaz — a keby sa raz do UI dostal cudzí text, nech z toho
/// nevznikne klikateľný odkaz kamkoľvek.
#[tauri::command]
fn otvor_web(app: AppHandle, cesta: String) -> Result<(), String> {
    let n = app.state::<Stav>().nastavenia.lock().unwrap().clone();
    let plna = if cesta.starts_with("http") { cesta } else { n.adresa(&cesta) };
    // Dve povolené skupiny a nič viac: (a) náš server z nastavení, (b) adresa
    // inštalačky z manifestu, ktorú overuje tá istá pripnutá zarážka ako updater.
    let nase = plna.starts_with(&n.url_servera);
    let aktualizacia = tli_overlay_core::aktualizacia::je_povolena_adresa(&plna);
    if !nase && !aktualizacia {
        return Err("Takúto adresu appka neotvára — nepatrí ani tvojmu serveru, ani nášmu repozitáru.".to_string());
    }
    otvor_v_prehliadaci(&plna)
}

#[cfg(windows)]
fn otvor_v_prehliadaci(adresa: &str) -> Result<(), String> {
    // `start` je vstavaný príkaz `cmd`, preto sa volá cez `cmd /C`. Prázdny
    // druhý argument je nutný: `start` berie prvý reťazec ako titulok okna.
    std::process::Command::new("cmd")
        .args(["/C", "start", "", adresa])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Nepodarilo sa otvoriť prehliadač: {e}"))
}

#[cfg(not(windows))]
fn otvor_v_prehliadaci(_adresa: &str) -> Result<(), String> {
    Err("Otváranie prehliadača je dostupné len na Windowse.".to_string())
}

#[tauri::command]
fn verzia() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Vráti adresu nového MSI, keď je vonku novšia verzia. Ťahá sa VÝHRADNE náš
/// `latest-tli.json` — iný endpoint `klient::stiahni_manifest` ani nepustí.
#[tauri::command]
fn skontroluj_aktualizaciu() -> Result<Option<String>, String> {
    let json = klient::stiahni_manifest()?;
    let manifest =
        tli_overlay_core::aktualizacia::skontroluj_manifest(&json, env!("CARGO_PKG_VERSION"))?;
    Ok(manifest.map(|m| m.msi_url))
}

// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    let nacitane = stav::nacitaj();
    let hotkey_pri_starte = nacitane.hotkey.clone();
    let hotkey_canvas_pri_starte = nacitane.hotkey_canvas.clone();

    let skratky = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, skratka, udalost| {
            if udalost.state() != ShortcutState::Pressed {
                return;
            }
            let viditelny = app
                .get_webview_window(OVERLAY)
                .and_then(|o| o.is_visible().ok())
                .unwrap_or(false);

            if viditelny {
                // Hotkey aj Esc robia to isté: zhasnú overlay.
                skry(app);
            } else if skratka != &esc() {
                // Ktorý z dvoch hotkey práve spustil analýzu sa pozná podľa
                // zhody s AKTUÁLNE nastaveným `hotkey_canvas` (nie podľa
                // hodnoty pri štarte appky — nastavenia sa dajú zmeniť bez
                // reštartu). Keď sa reťazec nedá naparsovať, nikdy sa
                // nezhodne a padá sa na Tartarus (bezpečný predvolený stav).
                let hotkey_canvas = app.state::<Stav>().nastavenia.lock().unwrap().hotkey_canvas.clone();
                let modul = match Shortcut::from_str(&hotkey_canvas) {
                    Ok(s) if &s == skratka => Modul::Canvas,
                    _ => Modul::Tartarus,
                };
                spusti_analyzu(app.clone(), modul);
            }
        })
        .build();

    tauri::Builder::default()
        .plugin(skratky)
        .plugin(tauri_plugin_dialog::init())
        .manage(Stav {
            nastavenia: Mutex::new(nacitane),
            beh: Mutex::new(false),
            zobrazene_od: Mutex::new(None),
            pocitadlo_zber: Mutex::new(0),
            poslane_behy: Mutex::new(std::collections::BTreeSet::new()),
        })
        .manage(tracker::Tracker::default())
        // Sprievodca sa načíta z disku hneď pri štarte — záložka Guide
        // tak ukáže obsah okamžite a bez siete.
        .manage(guide::StavGuide(Mutex::new(guide::nacitaj())))
        .invoke_handler(tauri::generate_handler![
            nacitaj_nastavenia,
            uloz_nastavenia,
            otestuj_pripojenie,
            skry_overlay,
            otvor_web,
            otvor_nastavenia,
            verzia,
            skontroluj_aktualizaciu,
            tracker_prehlad,
            tracker_csv,
            tracker_navrh_cestu,
            tracker_stav_cesty,
            tracker_vyber_subor,
            uloz_cestu_logu,
            uloz_temu,
            guide_data,
            guide_sync,
            tracker_sync_teraz,
            uloz_prepinac
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Keď skratku z nastavení Windows neprijme, `prehlas_hotkey` sa sám
            // vráti k F8 — appka teda nikdy neostane bez ovládacej klávesy.
            // Canvas sa registruje AŽ PO nej (tá ruší všetky skratky).
            let _ = prehlas_hotkey(&handle, &hotkey_pri_starte);
            let _ = prehlas_hotkey_canvas(&handle, &hotkey_canvas_pri_starte);

            // Tracker (TLI 11c): číta log na pozadí nezávisle od toho, či je
            // niektoré okno otvorené, aby appka fungovala aj schovaná v tray.
            tracker::spusti_vlakno(handle.clone());

            if let Some(overlay) = app.get_webview_window(OVERLAY) {
                let _ = overlay.set_ignore_cursor_events(true);
                let _ = overlay.hide();
            }
            if let Some(okno) = app.get_webview_window(NASTAVENIA) {
                let _ = okno.hide();
            }
            if let Some(okno) = app.get_webview_window(HLAVNE) {
                let _ = okno.hide();
            }
            // Mini-overlay sa pri štarte ukáže len vtedy, keď si ho hráč zapol.
            let mini_zapnuty = app.state::<Stav>().nastavenia.lock().unwrap().mini_overlay;
            if let Some(okno) = app.get_webview_window(MINI) {
                let _ = okno.set_ignore_cursor_events(true);
                if mini_zapnuty {
                    let _ = okno.show();
                } else {
                    let _ = okno.hide();
                }
            }

            let polozka_hlavne =
                tauri::menu::MenuItem::with_id(app, "hlavne", "Otvoriť", true, None::<&str>)?;
            let polozka_nastavenia =
                tauri::menu::MenuItem::with_id(app, "nastavenia", "Nastavenia", true, None::<&str>)?;
            let polozka_verzia = tauri::menu::MenuItem::with_id(
                app,
                "verzia",
                format!("Verzia {}", env!("CARGO_PKG_VERSION")),
                false,
                None::<&str>,
            )?;
            let polozka_koniec =
                tauri::menu::MenuItem::with_id(app, "koniec", "Ukončiť", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(
                app,
                &[&polozka_hlavne, &polozka_nastavenia, &polozka_verzia, &polozka_koniec],
            )?;

            let mut tray = tauri::tray::TrayIconBuilder::with_id("tray")
                .tooltip("Tvoj Tatko TLI Overlay")
                .menu(&menu)
                .on_menu_event(|app, udalost| match udalost.id.as_ref() {
                    "koniec" => app.exit(0),
                    "hlavne" => ukaz_a_zaostri(app, HLAVNE),
                    "nastavenia" => ukaz_a_zaostri(app, NASTAVENIA),
                    _ => {}
                });
            if let Some(ikona) = app.default_window_icon().cloned() {
                tray = tray.icon(ikona);
            }
            tray.build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Appka sa nespustila");
}
