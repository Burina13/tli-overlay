// Appka beží bez konzolového okna — inak by pri každom spustení (aj z tray ikony)
// vyskočilo čierne okno `cmd`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod denik;
mod guide;
mod klient;
mod okna;
#[cfg(windows)]
mod okno;
#[cfg(windows)]
mod snimka;
mod stav;
mod tracker;

use serde_json::json;
use stav::{RezimUpravyKontext, Stav};
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tli_overlay_core::nastavenia::Nastavenia;
use tli_overlay_core::nastavenia::PolohaOkna;
use tli_overlay_core::rozlozenie::{
    Obdlznik01, MODULOVE_PANELY, PANEL_CANVAS, PANEL_HLAVNY, PANEL_MINI, PANEL_NASTROJ,
    PANEL_PRIPOMIENKA, PANEL_TARTARUS,
};
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
/// Toast pripomienky „Enable Log" (TLI 11d): malé priehľadné okno navrchu,
/// ktoré sa ukáže len na pár sekúnd, keď appka zistí, že logovanie evidentne
/// nebeží (alebo keď sa práve rozbehlo po predošlom upozornení).
const PRIPOMIENKA: &str = "pripomienka";
/// Ako dlho ostane toast upozornenia viditeľný, kým ho prípadný ďalší herný
/// riadok nezhasne skôr (TLI 11d, bod 3: „upozornenie zmizne samo, keď začnú
/// prichádzať riadky").
const PRIPOMIENKA_UPOZORNENIE_S: u64 = 12;
/// „Beží OK" je len krátke potvrdenie, nie treba ho držať tak dlho ako samotné
/// upozornenie.
const PRIPOMIENKA_OK_S: u64 = 4;

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

/// Enter — registruje sa LEN počas režimu „Upraviť rozloženie" (TLI 21), z
/// rovnakého dôvodu ako `esc()`: globálna skratka by inak hre zobrala klávesu,
/// ktorú v Torchlighte bežne používa na potvrdenie v dialógoch.
fn enter() -> Shortcut {
    Shortcut::new(None, Code::Enter)
}

/// Ctrl+F8 — „Upraviť rozloženie" (TLI 21, `2026-10-16-1528-mcp.md` bod 1).
/// Na rozdiel od `esc()`/`enter()` je zaregistrovaná TRVALO (nemení sa podľa
/// toho, čo je práve viditeľné) — hráč ju musí vedieť stlačiť kedykoľvek, aj
/// keď nad hrou práve nič nie je.
fn ctrl_f8() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL), Code::F8)
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

/// Tretia skratka — ceny z obrazovky trhu (TLI 22), NEPOVINNÁ. Na rozdiel od
/// `prehlas_hotkey`/`prehlas_hotkey_canvas` nemá predvolenú hodnotu: hlavný
/// spôsob spustenia je automatická detekcia z logu (`tracker.rs`,
/// `odober_trh_otvoreny`), tento hotkey je len ručný fallback zo zadania
/// („alebo na hotkey"). Prázdny reťazec = nezaregistrovaná, appke sa nič
/// nemení. Volá sa vždy AŽ PO `prehlas_hotkey` (ten `unregister_all()`-uje).
pub fn prehlas_hotkey_trh(app: &AppHandle, novy: &str) -> Result<(), String> {
    let novy = novy.trim();
    if novy.is_empty() {
        return Ok(());
    }
    let skratky = app.global_shortcut();
    if skratky.register(novy).is_err() {
        return Err(format!(
            "Skratku pre ceny z trhu „{novy}“ Windows neprijal (môže ju držať iná appka \
alebo kolíduje s inou skratkou). Automatická detekcia z logu funguje ďalej bez nej."
        ));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Overlay
// ─────────────────────────────────────────────────────────────────────────────

/// Monitor, na ktorom leží okno hry. Počíta sa cez `suradnice::monitor_okna`,
/// teda cez tú istú funkciu, ktorú pokrývajú jednotkové testy — nie zvlášť „naostro“
/// a zvlášť „v teste“. Vracia `None`, keď sa monitor nedá určiť (napr. appka
/// beží v CI bez skutočnej obrazovky) — volajúci si vtedy poradí predvolenou
/// hodnotou, presne ako doteraz `mierka_okna`.
fn monitor_okna_app(app: &AppHandle, obd: &OknoFyz) -> Option<suradnice::Monitor> {
    let overlay = app.get_webview_window(OVERLAY)?;
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
    suradnice::monitor_okna(obd, &monitory).copied()
}

/// Mierka monitora, na ktorom leží okno hry — tenký obal nad `monitor_okna_app`
/// pre volania, ktorým stačí samotné číslo (CSS prepočet políčok solvera).
fn mierka_okna(app: &AppHandle, obd: &OknoFyz) -> f64 {
    monitor_okna_app(app, obd).map(|m| m.mierka).unwrap_or_else(|| {
        app.get_webview_window(OVERLAY)
            .and_then(|o| o.scale_factor().ok())
            .unwrap_or(1.0)
    })
}

/// Stabilný textový kľúč monitora pre uložené rozloženie (TLI 21) — keď sa
/// monitor nedá určiť (napr. appka beží bez reálnej obrazovky), padá sa na
/// kľúč odvodený priamo z rozmerov okna hry pri mierke 100 %, nech appka vie
/// aspoň niečo uložiť namiesto toho, aby úpravu rozloženia odmietla celkom.
fn monitor_signatura_app(app: &AppHandle, obd: &OknoFyz) -> String {
    monitor_okna_app(app, obd)
        .map(|m| suradnice::monitor_signatura(&m))
        .unwrap_or_else(|| format!("{}x{}@100", obd.sirka, obd.vyska))
}

/// Predvolená poloha/veľkosť panela, keď preň ešte nie je nič uložené (TLI 21)
/// — vyjadrené ako zlomok okna hry pri referenčnom rozlíšení 1920×1080, aby sa
/// pri inom rozlíšení držal rovnaký POMER, nie rovnaký počet pixelov. Hlavný
/// panel (F8/Tartarus aj F9/Canvas — zdieľajú jeden DOM prvok) v BEŽNOM behu
/// predvolenú hodnotu vôbec nepoužíva (necháva sa na CSS, pozri `ukaz_overlay_nad`)
/// — slúži len ako štartovací obdĺžnik pri prvom vstupe do režimu úpravy.
fn predvolena_poloha(panel: &str) -> Obdlznik01 {
    match panel {
        PANEL_MINI => Obdlznik01 { x: 1640.0 / 1920.0, y: 20.0 / 1080.0, w: 260.0 / 1920.0, h: 92.0 / 1080.0 },
        PANEL_PRIPOMIENKA => {
            Obdlznik01 { x: 720.0 / 1920.0, y: 40.0 / 1080.0, w: 480.0 / 1920.0, h: 110.0 / 1080.0 }
        }
        // Hlavný panel aj všetky jeho modulové varianty (TLI 25) štartujú
        // z toho istého rohu — líšiť sa začnú až tým, kam ich hráč posunie.
        _ => Obdlznik01 { x: 24.0 / 1920.0, y: 24.0 / 1080.0, w: 460.0 / 1920.0, h: 260.0 / 1080.0 },
    }
}

/// Uloženú polohu panela pre daný monitor, alebo predvolenú, keď nič uložené
/// nie je — používa režim úpravy, kde VŽDY treba konkrétny obdĺžnik (na rozdiel
/// od bežného zobrazenia panela, ktoré bez uloženej hodnoty necháva CSS default).
fn poloha_alebo_predvolena(n: &Nastavenia, panel: &str, monitor_sig: &str) -> Obdlznik01 {
    n.rozlozenie.poloha(panel, monitor_sig).unwrap_or_else(|| predvolena_poloha(panel))
}

/// To isté pre hlavný panel, ale s pádom na spoločný kľúč `panel` (TLI 25) —
/// modul, ktorý ešte nikdy nebol posunutý samostatne, štartuje tam, kde si
/// hráč nechal spoločný panel, nie v rohu.
fn poloha_alebo_predvolena_modulu(n: &Nastavenia, panel: &str, monitor_sig: &str) -> Obdlznik01 {
    n.rozlozenie
        .poloha_modulu(panel, monitor_sig)
        .unwrap_or_else(|| predvolena_poloha(panel))
}

/// Postaví overlay presne nad okno hry a ukáže ho. Pozícia aj veľkosť idú vo
/// FYZICKÝCH pixeloch — logické by sa na monitore so 150 % DPI rozišli s hrou.
///
/// `s_rozlozenim` rozlišuje skutočné okno hry od núdzového nahradného
/// obdĺžnika v `ukaz_chybu` (keď sa hra vôbec nenašla) — uložené rozloženie
/// (TLI 21) je zlomok SKUTOČNÉHO okna hry, aplikovať ho na malý núdzový box by
/// dalo nezmyselné súradnice.
fn ukaz_overlay_nad(app: &AppHandle, obd: &OknoFyz, s_rozlozenim: bool, panel_kluc: &str) {
    if let Some(overlay) = app.get_webview_window(OVERLAY) {
        let _ = overlay.set_position(PhysicalPosition::new(obd.x, obd.y));
        let _ = overlay.set_size(PhysicalSize::new(obd.sirka, obd.vyska));
        let _ = overlay.set_ignore_cursor_events(true);
        let _ = overlay.show();
        let _ = app.global_shortcut().register(esc());
        *app.state::<Stav>().zobrazene_od.lock().unwrap() = Some(Instant::now());

        if s_rozlozenim {
            let mierka = mierka_okna(app, obd);
            posli_polohu_panela(app, obd, mierka, panel_kluc);
        } else {
            let priehladnost =
                app.state::<Stav>().nastavenia.lock().unwrap().rozlozenie.priehladnost;
            let _ = app.emit_to(
                OVERLAY,
                "panel-poloha",
                json!({ "poloha": null, "priehladnost": priehladnost }),
            );
        }
    }
}

/// Pošle oknu `overlay` polohu hlavného panela pre KONKRÉTNY modul (TLI 25).
/// Volá sa dvakrát za beh, keď sa modul vyjasní až z odpovede servera
/// (F8: doska vs. výber nástroja) — druhé poslanie prvé prepíše.
fn posli_polohu_panela(app: &AppHandle, obd: &OknoFyz, mierka: f64, panel_kluc: &str) {
    let sig = monitor_signatura_app(app, obd);
    let (poloha, priehladnost) = {
        let stav = app.state::<Stav>();
        let n = stav.nastavenia.lock().unwrap();
        (
            n.rozlozenie
                .poloha_modulu(panel_kluc, &sig)
                .map(|o| suradnice::box01_na_css(o.x, o.y, o.w, o.h, obd, mierka))
                .map(|css| json!({"x": css.x, "y": css.y, "w": css.w, "h": css.h})),
            n.rozlozenie.priehladnost,
        )
    };
    let _ = app.emit_to(
        OVERLAY,
        "panel-poloha",
        json!({ "poloha": poloha, "priehladnost": priehladnost }),
    );
}

/// Uloží polohu a veľkosť hlavného okna do nastavení (TLI 25, bod 5). Zlyhanie
/// zápisu sa len zaloguje — je to pohodlie, nie dáta, a nesmie zabrániť
/// zatvoreniu okna.
fn zapamataj_polohu_hlavneho<R: tauri::Runtime>(okno: &tauri::Window<R>) {
    let (Ok(pozicia), Ok(velkost)) = (okno.outer_position(), okno.outer_size()) else {
        return;
    };
    if velkost.width == 0 || velkost.height == 0 {
        return;  // minimalizované okno nemá zmysel ukladať
    }
    let app = okno.app_handle();
    let stav = app.state::<Stav>();
    let kopia = {
        let mut n = stav.nastavenia.lock().unwrap();
        n.okno_hlavne = Some(PolohaOkna {
            x: pozicia.x,
            y: pozicia.y,
            sirka: velkost.width,
            vyska: velkost.height,
        });
        n.clone()
    };
    if let Err(e) = stav::uloz(&kopia) {
        loguj_chybu_okna("uloženie polohy hlavného okna", &e);
    }
}

/// Vráti hlavné okno tam, kde ho hráč nechal. Keď by padlo mimo viditeľných
/// monitorov (odpojený monitor), `ukaz_a_zaostri` ho sám vycentruje — preto sa
/// tu nič nekontroluje druhý raz.
fn obnov_polohu_hlavneho(app: &AppHandle) {
    let poloha = app.state::<Stav>().nastavenia.lock().unwrap().okno_hlavne;
    let (Some(p), Some(okno)) = (poloha, app.get_webview_window(HLAVNE)) else {
        return;
    };
    let _ = okno.set_position(PhysicalPosition::new(p.x, p.y));
    let _ = okno.set_size(PhysicalSize::new(p.sirka, p.vyska));
}

/// Chybu musí hráč vidieť aj vtedy, keď sa okno hry vôbec nenašlo — vtedy nie je
/// kam overlay postaviť, tak sa dá navrch hlavného monitora.
fn ukaz_chybu(app: &AppHandle, obd: Option<&OknoFyz>, sprava: &str) {
    match obd {
        Some(o) => ukaz_overlay_nad(app, o, true, PANEL_TARTARUS),
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
                    false,
                    PANEL_TARTARUS,
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
    /// Ceny z obrazovky trhu (TLI 22) — spúšťa sa HLAVNE automaticky z logu
    /// (`tracker.rs::odober_trh_otvoreny`), nepovinne aj cez `hotkey_trh`.
    Ceny,
}

#[cfg(windows)]
pub(crate) fn spusti_analyzu(app: AppHandle, modul: Modul) {
    // Zapísané VŽDY, ako prvé — nezávisle od všetkých early-returnov nižšie.
    // Predtým appka nemala žiadnu stopu po tom, že hotkey vôbec dorazil do
    // Rustu (TLI 24, `2026-10-17-1238-mcp.md` bod 1/4): keď appka „nič
    // nespraví", nedalo sa ani odlíšiť „hotkey nedorazil" od „appka ho
    // dostala a niečo potichu zlyhalo".
    denik::zapis("hotkey", &format!("{modul:?} stlačené"));

    // Kým hráč posúva/zväčšuje panely (TLI 21), F8/F9 nemajú spustiť analýzu
    // nad práve upravovaným rozložením — zmazalo by to náhľad aj vrátilo
    // priechodnosť myši uprostred ťahania.
    if *app.state::<Stav>().rezim_upravy.lock().unwrap() {
        denik::zapis("hotkey", "ignorované — beží režim úpravy rozloženia");
        return;
    }

    let nastavenia = {
        let stav = app.state::<Stav>();
        let mut n = stav.nastavenia.lock().unwrap();
        n.normalizuj();
        n.clone()
    };

    if let Some(chyba) = nastavenia.chyba_konfiguracie() {
        denik::zapis("hotkey", &format!("chyba konfigurácie: {chyba}"));
        ukaz_chybu(&app, None, &chyba);
        return;
    }

    // Dvojité stlačenie hotkey (ktoréhokoľvek z dvoch) nemá poslať dve
    // požiadavky — server púšťa 30 za minútu a zámok je zdieľaný medzi
    // Tartarus aj Frozen Canvas, nemajú bežať naraz nad tým istým oknom.
    //
    // TLI 24: tento zámok je JEDINÉ miesto, kde appka vie úplne potichu
    // „nič nespraviť" (žiadny event, žiadny log) — keby z nejakého dôvodu
    // ostal `true` navždy (napr. nečakaný panic vo vlákne nižšie pred jeho
    // vrátením na `false`), appka by od tej chvíle na KAŽDÝ ďalší F8/F9
    // mlčala presne tak, ako to opísal Richard. Preto je zápis do logu tu
    // PRED early-returnom — keby sa to niekedy zopakovalo, log to ukáže.
    {
        let stav = app.state::<Stav>();
        let mut beh = stav.beh.lock().unwrap();
        if *beh {
            denik::zapis("hotkey", "ignorované — predošlá analýza ešte nedobehla (beh=true)");
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
        denik::zapis("okno", "okno hry sa nenašlo (ani podľa nastavení, ani popredie)");
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
    denik::zapis(
        "okno",
        &format!(
            "nájdené: „{}“ ({}), {}x{} @ ({},{})",
            okno_hry.nazov, okno_hry.proces, obdlznik.sirka, obdlznik.vyska, obdlznik.x, obdlznik.y
        ),
    );

    // Fotí sa EŠTE PRED zobrazením overlayu — inak by bol overlay na snímke
    // a server by rozpoznával vlastné rámčeky namiesto dosky.
    let snimok = match snimka::zachyt(&obdlznik) {
        Ok(s) => s,
        Err(e) => {
            denik::zapis("snimka", &format!("zlyhala: {e}"));
            ukaz_chybu(&app, Some(&obdlznik), &e);
            koniec(&app);
            return;
        }
    };
    denik::zapis("snimka", &format!("zachytená, {}x{}", snimok.sirka, snimok.vyska));

    if snimka::je_cierna(&snimok) {
        denik::zapis("snimka", "obraz je takmer čierny (exkluzívny fullscreen?)");
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
    // Ceny z obrazovky trhu (TLI 22) bežia hlavne AUTOMATICKY, potichu na
    // pozadí (zakaždým, keď hráč otvorí detail položky) — veľký panel by pri
    // bežnom prezeraní trhu blikal nad hrou pri každom kliknutí. Výsledok
    // ukáže len toast (`ukaz_pripomienku_toast`), rovnako ako chyba.
    if modul != Modul::Ceny {
        ukaz_overlay_nad(
            &app,
            &obdlznik,
            true,
            if modul == Modul::Canvas { PANEL_CANVAS } else { PANEL_TARTARUS },
        );
        let _ = app.emit("stav", json!({ "faza": "analyzujem" }));
    }

    // Kódovanie JPEGu aj sieť idú mimo hlavného vlákna — inak by sa na tých
    // pár sekúnd zasekla celá appka vrátane tray ikony.
    //
    // TLI 24: celé telo je v `catch_unwind` — keby niečo vo vnútri nečakane
    // spanikárilo (dosiaľ sa to nestalo, žiadny `unwrap`/`expect` v tejto
    // ceste nie je, ale appka to nesmie len predpokladať navždy), `beh` by
    // bez tejto poistky ostal `true` navždy a appka by od tej chvíle na
    // KAŽDÝ ďalší F8/F9 ticho mlčala. Takto appka aj pri panike ukáže chybu
    // a zámok uvoľní.
    std::thread::spawn(move || {
        let vysledok_behu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match modul {
            Modul::Tartarus => {
                let vysledok = snimka::do_jpegu(&snimok, zmensenie::MAX_DLZKA, 80).and_then(|jpeg| {
                    denik::zapis("http", &format!("POST /api/analyza ({} B JPEG)", jpeg.len()));
                    let _ = app.emit("stav", json!({ "faza": "odosielam" }));
                    klient::posli_analyzu(&nastavenia, jpeg)
                });
                match &vysledok {
                    Ok(v) => denik::zapis("http", &format!("analyza OK, {} ms", v.trvanie_ms)),
                    Err(e) => denik::zapis("http", &format!("analyza CHYBA: {e}")),
                }
                match vysledok {
                    // TLI 25: tá istá klávesa chodí na viac obrazoviek mechaniky
                    // a server povie, ktorú prečítal. Doska sa kreslí ako doteraz,
                    // výber nástroja má vlastné vykreslenie, zvyšok len toast —
                    // appka nemá čo kresliť nad obrazovkou, ktorej nerozumie.
                    Ok(v) if v.analyza.obrazovka == "vyber_nastroja" => {
                        denik::zapis(
                            "nastroj",
                            &format!(
                                "výber nástroja: {} kartičiek, odporúčam {}",
                                v.analyza.nastroje.len(),
                                v.analyza
                                    .odporucanie
                                    .as_ref()
                                    .map(|o| o.nazov.clone())
                                    .unwrap_or_else(|| "nič (nepoznám ani jednu)".to_string())
                            ),
                        );
                        let karticky: Vec<_> = api::pouzitelne_karticky(&v.analyza)
                            .into_iter()
                            .map(|k| {
                                let b = k.box_.expect("filtrované pouzitelne_karticky");
                                let css = suradnice::box01_na_css(b.x, b.y, b.w, b.h, &obdlznik, mierka);
                                json!({
                                    "index": k.index,
                                    "poradie": k.poradie,
                                    "nazov": k.nazov.clone(),
                                    "anglicky": k.anglicky.clone(),
                                    "tier": k.tier.clone(),
                                    "dovod": k.dovod.clone(),
                                    "skore": k.skore,
                                    "odporucany": k.odporucany,
                                    "neznamy": k.neznamy,
                                    "malo_dat": k.malo_dat,
                                    "sporne": k.sporne,
                                    "x": css.x, "y": css.y, "w": css.w, "h": css.h,
                                })
                            })
                            .collect();
                        // Výber nástroja má vlastnú uloženú polohu panela
                        // (TLI 25) — že ide oň, sa vie až TERAZ, po odpovedi
                        // servera, takže sa poloha pošle druhý raz.
                        posli_polohu_panela(&app, &obdlznik, mierka, PANEL_NASTROJ);
                        let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obdlznik, mierka);
                        let _ = app.emit(
                            "stav",
                            json!({
                                "faza": "hotovo_nastroj",
                                "analyza": v.analyza,
                                "karticky": karticky,
                                "mierka": mierka,
                                "sirka": sirka_log,
                                "vyska": vyska_log,
                                "trvanie_ms": v.trvanie_ms,
                            }),
                        );
                    }
                    Ok(v) if v.analyza.obrazovka == "ine" => {
                        denik::zapis("nastroj", &format!("neznáma obrazovka: {}", v.analyza.popis));
                        // Žiadny veľký panel: hráč nič nerobí, len nech vie, že
                        // sa snímka uložila a appka ho neignorovala.
                        ukaz_pripomienku_toast(&app, &api::text_toastu_neznama(&v.analyza), "info", 8);
                        let _ = app.emit("stav", json!({ "faza": "skry" }));
                    }
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
                let vysledok = snimka::do_jpegu(&snimok, zmensenie::MAX_DLZKA, 80).and_then(|jpeg| {
                    denik::zapis("http", &format!("POST /api/analyza?modul=canvas ({} B JPEG)", jpeg.len()));
                    let _ = app.emit("stav", json!({ "faza": "odosielam" }));
                    klient::posli_analyzu_canvas(&nastavenia, jpeg)
                });
                match &vysledok {
                    Ok(v) => denik::zapis("http", &format!("canvas OK, {} ms", v.trvanie_ms)),
                    Err(e) => denik::zapis("http", &format!("canvas CHYBA: {e}")),
                }
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
            Modul::Ceny => {
                // TLI 22: žiadne súradnice, žiadne kreslenie nad hrou — len
                // zápis do cenníka (keď sa dá) a jeden toast. `.ok` je tu vždy
                // `true`: server pri zlyhaní rozpoznania vráti != 200, čo
                // `klient::posli_analyzu_ceny` premení na `Err`, nie na
                // `Ok(AnalyzaCeny { ok: false, .. })`.
                let vysledok = snimka::do_jpegu(&snimok, zmensenie::MAX_DLZKA, 80).and_then(|jpeg| {
                    denik::zapis("http", &format!("POST /api/analyza?modul=ceny ({} B JPEG)", jpeg.len()));
                    klient::posli_analyzu_ceny(&nastavenia, jpeg)
                });
                match vysledok {
                    Ok(v) => {
                        denik::zapis("http", &format!("ceny OK, {} ms", v.trvanie_ms));
                        let a = &v.analyza;
                        if let (Some(cena), Some(cbid)) = (a.p10, a.config_base_id) {
                            let tracker = app.state::<tracker::Tracker>();
                            let mut t = tracker.0.lock().unwrap();
                            t.uloz_cenu_z_obrazovky(cbid, cena, a.pocet_ponuk, tracker::unixovy_cas());
                        }
                        ukaz_pripomienku_toast(&app, &api::text_toastu_ceny(a), "cena", 8);
                    }
                    Err(e) => {
                        denik::zapis("http", &format!("ceny CHYBA: {e}"));
                        ukaz_pripomienku_toast(&app, &format!("Ceny z trhu: {e}"), "chyba", 8);
                    }
                }
            }
        }
        })); // koniec catch_unwind

        if let Err(panika) = vysledok_behu {
            let text = panika
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panika.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "neznáma panika".to_string());
            denik::zapis("hotkey", &format!("NEOČAKÁVANÁ CHYBA počas analýzy: {text}"));
            let _ = app.emit(
                "stav",
                json!({ "faza": "chyba", "sprava": "Nastala neočakávaná chyba. Skús hotkey znova; ak sa to opakuje, pošli log cez #torchlight." }),
            );
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
pub(crate) fn spusti_analyzu(app: AppHandle, _modul: Modul) {
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

// ─────────────────────────────────────────────────────────────────────────────
// Záložka „Analýza" (TLI 13)
// ─────────────────────────────────────────────────────────────────────────────

/// Vyhľadá behy v lokálnej databáze podľa filtra zo záložky Analýza.
#[tauri::command]
fn analyza_vyhladaj(
    tracker: tauri::State<tracker::Tracker>,
    filter: tli_overlay_core::tracker::db::FilterBehov,
) -> Result<Vec<tli_overlay_core::tracker::db::BehRiadok>, String> {
    let t = tracker.0.lock().unwrap();
    tracker::analyza_vyhladaj(&t, &filter)
}

/// Monštrá podľa rarity pre jeden beh — detail behu v záložke Analýza.
#[tauri::command]
fn analyza_monstra_podla_rarity(
    tracker: tauri::State<tracker::Tracker>,
    beh_id: i64,
) -> Result<Vec<(u8, i64)>, String> {
    let t = tracker.0.lock().unwrap();
    tracker::analyza_monstra_podla_rarity(&t, beh_id)
}

/// Export vyhľadaných behov do CSV. Vracia obsah súboru — uloženie rieši UI
/// (rovnaký vzor ako `tracker_csv`).
#[tauri::command]
fn analyza_export_csv(
    tracker: tauri::State<tracker::Tracker>,
    filter: tli_overlay_core::tracker::db::FilterBehov,
) -> Result<String, String> {
    let t = tracker.0.lock().unwrap();
    let riadky = tracker::analyza_vyhladaj(&t, &filter)?;
    Ok(tli_overlay_core::tracker::db::export_csv(&riadky))
}

/// Export vyhľadaných behov do JSON.
#[tauri::command]
fn analyza_export_json(
    tracker: tauri::State<tracker::Tracker>,
    filter: tli_overlay_core::tracker::db::FilterBehov,
) -> Result<String, String> {
    let t = tracker.0.lock().unwrap();
    let riadky = tracker::analyza_vyhladaj(&t, &filter)?;
    Ok(tli_overlay_core::tracker::db::export_json(&riadky))
}

/// Zoskupené taktiky so štatistikami pre záložku Analýza (TLI 13b).
#[tauri::command]
fn analyza_taktiky(
    tracker: tauri::State<tracker::Tracker>,
    filter: tli_overlay_core::tracker::db::FilterTaktik,
) -> Result<tli_overlay_core::tracker::taktiky::AnalyzaTaktik, String> {
    let t = tracker.0.lock().unwrap();
    tracker::analyza_taktiky(&t, &filter)
}

/// Export zoznamu taktík (s aktuálnym filtrom) do CSV.
#[tauri::command]
fn analyza_taktiky_export_csv(
    tracker: tauri::State<tracker::Tracker>,
    filter: tli_overlay_core::tracker::db::FilterTaktik,
) -> Result<String, String> {
    let t = tracker.0.lock().unwrap();
    let analyza = tracker::analyza_taktiky(&t, &filter)?;
    Ok(tli_overlay_core::tracker::taktiky::export_csv(&analyza.taktiky))
}

/// Export zoznamu taktík (s aktuálnym filtrom) do JSON.
#[tauri::command]
fn analyza_taktiky_export_json(
    tracker: tauri::State<tracker::Tracker>,
    filter: tli_overlay_core::tracker::db::FilterTaktik,
) -> Result<String, String> {
    let t = tracker.0.lock().unwrap();
    let analyza = tracker::analyza_taktiky(&t, &filter)?;
    Ok(tli_overlay_core::tracker::taktiky::export_json(&analyza.taktiky))
}

/// Pomenovanie jednej taktiky alebo zlúčenie viacerých podpisov do jednej
/// pomenovanej taktiky (tlačidlo pri vybraných riadkoch v záložke Taktiky).
#[tauri::command]
fn taktika_pomenuj_alebo_zluc(
    tracker: tauri::State<tracker::Tracker>,
    podpisy: Vec<String>,
    nazov: String,
) -> Result<i64, String> {
    let t = tracker.0.lock().unwrap();
    tracker::taktika_pomenuj_alebo_zluc(&t, &podpisy, &nazov)
}

/// Tlačidlo „Načítať staré logy" — spätný import aktuálneho aj
/// `UE_game-backup-*.log` súborov z priečinka aktuálneho logu.
#[tauri::command]
fn tracker_nacitaj_stare_logy(
    stav: tauri::State<Stav>,
    tracker: tauri::State<tracker::Tracker>,
) -> Result<tli_overlay_core::tracker::import::VysledokImportu, String> {
    let n = stav.nastavenia.lock().unwrap().clone();
    let cesta = tracker::najdi_log(&n.cesta_logu)
        .ok_or_else(|| "Herný log sa nenašiel — nastav cestu v záložke Tracker.".to_string())?;
    let mut t = tracker.0.lock().unwrap();
    tracker::nacitaj_stare_logy(&mut t, &cesta)
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

/// Ručné kolo synchronizácie cien s partiou (TLI 20). Na pozadí beží aj tak
/// á 5 minút; toto tlačidlo je na „chcem to hneď" a na to, aby hráč videl
/// chybu siete, keď nastane — automatické kolo mlčí zámerne.
#[tauri::command]
fn ceny_sync_teraz(app: AppHandle) -> Result<String, String> {
    tracker::sync_cien_kolo(&app)
}

/// Ručné kolo denného uploadu logu (TLI 14). Na pozadí beží aj tak á
/// `upload::INTERVAL_S`; toto tlačidlo je na „pošli to hneď" (napr. tesne
/// pred tým, než o log Richard žiada niekoho iného) a na viditeľnú chybu
/// siete — automatické kolo mlčí zámerne.
#[tauri::command]
fn upload_teraz(app: AppHandle) -> Result<String, String> {
    tracker::upload_kolo(&app)
}

/// Ručný export lokálnej DB na server (TLI 14) — na rozdiel od logu sa
/// export NEPOSIELA automaticky na pozadí: je to jeden celý JSON, nie
/// prírastkový tok, a server si z neho zatiaľ nič nevyhodnocuje
/// (`GET /api/tracker/prior` čaká na nočné vyhodnotenie, `BACKLOG.md`).
/// Tlačidlo je tu pre prípad, že si ho Richard/vývoj vyžiada ručne skôr.
#[tauri::command]
fn db_export_teraz(app: AppHandle) -> Result<String, String> {
    let n = {
        let stav = app.state::<Stav>();
        let n = stav.nastavenia.lock().unwrap().clone();
        n
    };
    if let Some(chyba) = n.chyba_konfiguracie() {
        return Err(chyba);
    }
    let export = {
        let tracker = app.state::<tracker::Tracker>();
        let t = tracker.0.lock().unwrap();
        let conn = t.db().ok_or_else(|| "Lokálna databáza nie je otvorená.".to_string())?;
        tli_overlay_core::tracker::db::export_pre_upload(conn)
            .map_err(|e| format!("Export sa nedá zostaviť: {e}"))?
    };
    let json = serde_json::to_vec(&export).map_err(|e| format!("Export sa nedá zapísať: {e}"))?;
    let odpoved = klient::posli_db_export(&n, &json)?;
    if !odpoved.ok {
        return Err(odpoved.chyba.unwrap_or_else(|| "Server export odmietol.".to_string()));
    }
    Ok(format!("Export uložený ako {}.", odpoved.ulozeny_ako))
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
            "pripominat_enable_log" => n.pripominat_enable_log = zapnuty,
            "ceny_sync" => n.ceny_sync = zapnuty,
            "upload_logov" => n.upload_logov = zapnuty,
            "ceny_obrazovka_zapnute" => n.ceny_obrazovka_zapnute = zapnuty,
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
///
/// Pri zapnutí sa umiestni nad okno hry (TLI 21) — uloženou polohou pre
/// aktuálny monitor, alebo predvolenou, keď hráč panel ešte nikdy nepresunul.
/// Keď sa okno hry nenájde, appka polohu nemení (nechá okno tam, kde bolo).
fn nastav_mini(app: &AppHandle, zapnuty: bool) {
    if let Some(okno) = app.get_webview_window(MINI) {
        if zapnuty {
            if let Some(okno_hry) = okno_hry_pre_rozlozenie(app) {
                let sig = monitor_signatura_app(app, &okno_hry);
                let obd = {
                    let stav = app.state::<Stav>();
                    let n = stav.nastavenia.lock().unwrap();
                    poloha_alebo_predvolena(&n, PANEL_MINI, &sig)
                };
                umiestni_native_okno(app, MINI, &okno_hry, obd);
            }
            let _ = okno.show();
        } else {
            let _ = okno.hide();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pripomienka „Enable Log" (TLI 11d)
// ─────────────────────────────────────────────────────────────────────────────

/// Postaví toast nad okno hry (keď sa dá nájsť) alebo hore na stred hlavného
/// monitora (rovnaký fallback ako `ukaz_chybu`), ukáže ho s textom pripomienky
/// a naplánuje jeho skrytie po `sekund` sekundách — pokiaľ dovtedy neprišiel
/// NOVŠÍ toast (vtedy by starší časovač zhasol ten nový).
///
/// Poloha/veľkosť nad oknom hry rešpektuje uložené rozloženie (TLI 21) — alebo
/// predvolenú hodnotu (rovnaký stred/odstup ako pred TLI 21, len vyjadrený ako
/// zlomok okna hry, nie pevný počet pixelov).
fn ukaz_pripomienku_toast(app: &AppHandle, text: &str, trieda: &str, sekund: u64) {
    let Some(okno) = app.get_webview_window(PRIPOMIENKA) else { return };

    let _ = app.emit_to(
        PRIPOMIENKA,
        "pripomienka-obsah",
        json!({ "text": text, "trieda": trieda }),
    );

    match okno_hry_pre_rozlozenie(app) {
        Some(okno_hry) => {
            let sig = monitor_signatura_app(app, &okno_hry);
            let obd = {
                let stav = app.state::<Stav>();
                let n = stav.nastavenia.lock().unwrap();
                poloha_alebo_predvolena(&n, PANEL_PRIPOMIENKA, &sig)
            };
            umiestni_native_okno(app, PRIPOMIENKA, &okno_hry, obd);
        }
        None => {
            let sirka = 480u32;
            let vyska = 110u32;
            let (x, y) = okno
                .primary_monitor()
                .ok()
                .flatten()
                .map(|m| {
                    (
                        m.position().x + (m.size().width as i32 - sirka as i32) / 2,
                        m.position().y + 60,
                    )
                })
                .unwrap_or((80, 80));
            let _ = okno.set_position(PhysicalPosition::new(x, y));
            let _ = okno.set_size(PhysicalSize::new(sirka, vyska));
        }
    }
    let _ = okno.set_ignore_cursor_events(true);
    let _ = okno.show();

    let znacka = Instant::now();
    *app.state::<Stav>().pripomienka_zobrazena_od.lock().unwrap() = Some(znacka);
    naplanuj_skrytie_pripomienky(app.clone(), znacka, sekund);
}

/// Okno hry pre umiestnenie panelov nad ňou (TLI 21: mini-panel, toast,
/// a „seed" pri vstupe do režimu úpravy) — nájde ho podľa nastaveného názvu
/// procesu, a keď to zlyhá, padá na aktuálne aktívne okno (rovnaký vzor ako
/// `spusti_analyzu`).
#[cfg(windows)]
fn okno_hry_pre_rozlozenie(app: &AppHandle) -> Option<OknoFyz> {
    let proces_hry = app.state::<Stav>().nastavenia.lock().unwrap().proces_hry.clone();
    okno::najdi_hru(&proces_hry)
        .or_else(okno::popredie)
        .map(|o| o.obdlznik)
}

#[cfg(not(windows))]
fn okno_hry_pre_rozlozenie(_app: &AppHandle) -> Option<OknoFyz> {
    None
}

/// Umiestni natívne okno (mini/pripomienka) nad okno hry podľa uloženého
/// zlomku (TLI 21) — tá istá matematika ako pri CSS prepočte políčok solvera
/// (`box01_na_css`), len s mierkou 1.0, lebo natívne okná sa posúvajú vo
/// FYZICKÝCH pixeloch okna hry, nie v logických CSS pixeloch webview.
fn umiestni_native_okno(app: &AppHandle, label: &str, okno_hry: &OknoFyz, obd: Obdlznik01) {
    if let Some(okno) = app.get_webview_window(label) {
        let fyz = suradnice::box01_na_css(obd.x, obd.y, obd.w, obd.h, okno_hry, 1.0);
        let _ = okno.set_position(PhysicalPosition::new(
            okno_hry.x + fyz.x.round() as i32,
            okno_hry.y + fyz.y.round() as i32,
        ));
        let _ = okno.set_size(PhysicalSize::new(
            fyz.w.round().max(1.0) as u32,
            fyz.h.round().max(1.0) as u32,
        ));
    }
}

/// Rovnaký vzor ako `naplanuj_skrytie` pri hlavnom overlayi — porovnáva sa
/// značka, kedy bol TENTO toast zobrazený.
fn naplanuj_skrytie_pripomienky(app: AppHandle, znacka: Instant, sekundy: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(sekundy));
        let drzi = *app.state::<Stav>().pripomienka_zobrazena_od.lock().unwrap();
        if drzi == Some(znacka) {
            if let Some(okno) = app.get_webview_window(PRIPOMIENKA) {
                let _ = okno.hide();
            }
        }
    });
}

/// Volá ho vlákno trackera na pozadí (`tracker::spusti_vlakno`) cez udalosť
/// `tracker-pripomienka` — JS v okne `pripomienka` ju zachytí a hneď zavolá
/// tento príkaz späť, nech sa umiestnenie a časovač riešia na jednom mieste
/// (v Ruste), nie duplicitne aj v JS.
#[tauri::command]
fn ukaz_pripomienku(app: AppHandle, text: String, trieda: String) -> Result<(), String> {
    let sekund = if trieda == "ok" { PRIPOMIENKA_OK_S } else { PRIPOMIENKA_UPOZORNENIE_S };
    ukaz_pripomienku_toast(&app, &text, &trieda, sekund);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Režim „Upraviť rozloženie" (TLI 21, `2026-10-16-1528-mcp.md` bod 1)
//
// Hráč stlačí Ctrl+F8 (alebo tlačidlo „Upraviť rozloženie" v Nastaveniach):
// overlay prestane byť priechodný pre myš, nad okno hry sa postaví náhľad
// panela (F8/F9), mini-panel a toast pripomienky sa ukážu na svojej aktuálnej
// (uloženej alebo predvolenej) polohe a všetky tri sa dajú ťahaním/za roh
// posunúť a zväčšiť/zmenšiť. Esc alebo Enter uloží a vráti priechodnosť.
//
// Mini a pripomienka sú SAMOSTATNÉ natívne okná — Rust po stlačení Esc/Enter
// jednoducho prečíta ich aktuálnu polohu (`outer_position`/`outer_size`).
// Hlavný panel je ale len DOM prvok VNÚTRI okna `overlay` — jeho živú polohu
// pozná len JavaScript, takže uloženie ide cez krátky výlet tam a späť:
// Rust požiada JS (`rezim-upravy-nahlas`), JS odpovie príkazom
// `rezim_upravy_ulozit_panel` a AŽ TEN dokončí zvyšok (mini/toast, zápis na
// disk, ukončenie režimu).
// ─────────────────────────────────────────────────────────────────────────────

/// Postaví/obnoví náhľad všetkých troch panelov v aktuálnej (uloženej alebo
/// predvolenej) polohe — spoločné pre vstup do režimu úpravy aj pre tlačidlo
/// „Obnoviť predvolené" (to iba vymaže uložené hodnoty a zavolá toto znova).
fn posli_seed_rozlozenia(app: &AppHandle, okno_hry: &OknoFyz, mierka: f64, sig: &str,
                         panel_kluc: &str) {
    let n = app.state::<Stav>().nastavenia.lock().unwrap().clone();
    let priehladnost = n.rozlozenie.priehladnost;

    if let Some(overlay) = app.get_webview_window(OVERLAY) {
        let _ = overlay.set_position(PhysicalPosition::new(okno_hry.x, okno_hry.y));
        let _ = overlay.set_size(PhysicalSize::new(okno_hry.sirka, okno_hry.vyska));
        let _ = overlay.set_ignore_cursor_events(false);
        let _ = overlay.show();

        let obd = poloha_alebo_predvolena_modulu(&n, panel_kluc, sig);
        let css = suradnice::box01_na_css(obd.x, obd.y, obd.w, obd.h, okno_hry, mierka);
        let (sirka_log, vyska_log) = suradnice::logicka_velkost(okno_hry, mierka);
        let _ = app.emit_to(
            OVERLAY,
            "rezim-upravy",
            json!({
                "zapnute": true,
                "panel": {"x": css.x, "y": css.y, "w": css.w, "h": css.h},
                "priehladnost": priehladnost,
                "okno": {"sirka": sirka_log, "vyska": vyska_log},
                // TLI 25 — prepínač modulov v náhľade. Zoznam ide z jadra,
                // nie z HTML: keď pribudne ďalší modul, prepínač sa doplní sám.
                "moduly": MODULOVE_PANELY.iter()
                    .map(|(kluc, popis)| json!({"kluc": kluc, "popis": popis}))
                    .collect::<Vec<_>>(),
                "aktivny_modul": panel_kluc,
            }),
        );
    }

    for (label, panel) in [(MINI, PANEL_MINI), (PRIPOMIENKA, PANEL_PRIPOMIENKA)] {
        let obd = poloha_alebo_predvolena(&n, panel, sig);
        umiestni_native_okno(app, label, okno_hry, obd);
        if let Some(okno) = app.get_webview_window(label) {
            let _ = okno.set_ignore_cursor_events(false);
            let _ = okno.show();
            let _ = app.emit_to(label, "rezim-upravy", json!({"zapnute": true, "priehladnost": priehladnost}));
        }
    }
}

/// Vstup do režimu úpravy (Ctrl+F8 z prázdneho stavu, alebo tlačidlo
/// v Nastaveniach). Bez nájdeného okna hry sa nedá vypočítať zlomok, voči
/// ktorému sa polohy ukladajú — appka to poctivo povie namiesto tichého
/// zlyhania.
fn zapni_rezim_upravy(app: &AppHandle) {
    let Some(okno_hry) = okno_hry_pre_rozlozenie(app) else {
        ukaz_chybu(
            app,
            None,
            "Upraviť rozloženie sa dá len s bežiacou hrou — spusti Torchlight: Infinite \
a nechaj ho otvorené.",
        );
        return;
    };
    let mierka = mierka_okna(app, &okno_hry);
    let sig = monitor_signatura_app(app, &okno_hry);

    {
        let stav = app.state::<Stav>();
        *stav.rezim_upravy.lock().unwrap() = true;
        *stav.rezim_upravy_kontext.lock().unwrap() =
            Some(RezimUpravyKontext {
                okno_hry,
                mierka,
                monitor_sig: sig.clone(),
                panel_kluc: PANEL_TARTARUS.to_string(),
            });
    }
    let _ = app.global_shortcut().register(esc());
    let _ = app.global_shortcut().register(enter());

    posli_seed_rozlozenia(app, &okno_hry, mierka, &sig, PANEL_TARTARUS);
}

/// Požiada JS v okne `overlay`, nech nahlási živú polohu panela — Rust vtedy
/// dokončí uloženie aj ukončenie (`rezim_upravy_ulozit_panel`). Volá sa pri
/// Esc/Enter (globálna skratka) aj z tlačidla „Hotovo" v Nastaveniach.
fn poziadaj_o_ulozenie_rozlozenia(app: &AppHandle) {
    let _ = app.emit_to(OVERLAY, "rezim-upravy-nahlas", json!({}));
}

/// Vypne režim úpravy: skryje overlay/mini/toast (mini len keď si ho hráč
/// sám nezapol natrvalo v Nastaveniach), vráti priechodnosť myši a uvoľní
/// Enter (Esc sa uvoľňuje rovnako ako po bežnom zobrazení analýzy).
fn ukonci_rezim_upravy(app: &AppHandle) {
    {
        let stav = app.state::<Stav>();
        *stav.rezim_upravy.lock().unwrap() = false;
        *stav.rezim_upravy_kontext.lock().unwrap() = None;
    }
    let _ = app.global_shortcut().unregister(enter());
    let _ = app.global_shortcut().unregister(esc());

    if let Some(overlay) = app.get_webview_window(OVERLAY) {
        let _ = overlay.set_ignore_cursor_events(true);
        let _ = overlay.hide();
    }
    *app.state::<Stav>().zobrazene_od.lock().unwrap() = None;
    let _ = app.emit_to(OVERLAY, "rezim-upravy", json!({"zapnute": false}));

    let mini_zapnuty = app.state::<Stav>().nastavenia.lock().unwrap().mini_overlay;
    if let Some(okno) = app.get_webview_window(MINI) {
        let _ = okno.set_ignore_cursor_events(true);
        let _ = app.emit_to(MINI, "rezim-upravy", json!({"zapnute": false}));
        if !mini_zapnuty {
            let _ = okno.hide();
        }
    }
    if let Some(okno) = app.get_webview_window(PRIPOMIENKA) {
        let _ = okno.set_ignore_cursor_events(true);
        let _ = okno.hide();
        let _ = app.emit_to(PRIPOMIENKA, "rezim-upravy", json!({"zapnute": false}));
    }
}

/// Ctrl+F8 — mimo režimu ho zapne, v režime je to skratka pre „Hotovo"
/// (uloženie a ukončenie), nech si hráč nemusí pamätať dve rôzne klávesy.
fn prepni_rezim_upravy(app: &AppHandle) {
    let aktivny = *app.state::<Stav>().rezim_upravy.lock().unwrap();
    if aktivny {
        poziadaj_o_ulozenie_rozlozenia(app);
    } else {
        zapni_rezim_upravy(app);
    }
}

/// Tlačidlo „Upraviť rozloženie" v Nastaveniach — rovnaká cesta ako Ctrl+F8,
/// len vyvolaná z JS namiesto globálnej skratky.
#[tauri::command]
fn rezim_upravy_zapni(app: AppHandle) -> Result<(), String> {
    if !*app.state::<Stav>().rezim_upravy.lock().unwrap() {
        zapni_rezim_upravy(&app);
    }
    Ok(())
}

/// Tlačidlo „Hotovo" v Nastaveniach.
#[tauri::command]
fn rezim_upravy_hotovo(app: AppHandle) -> Result<(), String> {
    if *app.state::<Stav>().rezim_upravy.lock().unwrap() {
        poziadaj_o_ulozenie_rozlozenia(&app);
    }
    Ok(())
}

/// Či appka práve je v režime úpravy — Nastavenia si tým pri otvorení obnovia
/// správny stav tlačidiel, keď hráč okno medzitým zavrel a znova otvoril.
#[tauri::command]
fn rezim_upravy_je_aktivny(stav: tauri::State<Stav>) -> bool {
    *stav.rezim_upravy.lock().unwrap()
}

/// Tlačidlo „Obnoviť predvolené" — zmaže uložené polohy VŠETKÝCH troch
/// panelov pre aktuálny monitor (kontext zapamätaný pri vstupe do režimu) a
/// hneď nanovo pošle predvolený náhľad, nech je zmena vidieť okamžite.
#[tauri::command]
fn rezim_upravy_obnov_predvolene(app: AppHandle) -> Result<(), String> {
    let kontext = app.state::<Stav>().rezim_upravy_kontext.lock().unwrap().clone();
    let Some(kontext) = kontext else {
        return Err("Režim úpravy rozloženia nie je zapnutý.".to_string());
    };
    {
        let stav = app.state::<Stav>();
        let mut n = stav.nastavenia.lock().unwrap();
        n.rozlozenie.obnov_predvolene_vsetky(&kontext.monitor_sig);
        let kopia = n.clone();
        drop(n);
        stav::uloz(&kopia)?;
    }
    posli_seed_rozlozenia(&app, &kontext.okno_hry, kontext.mierka, &kontext.monitor_sig,
                          &kontext.panel_kluc);
    Ok(())
}

/// Prepínač modulov v náhľade režimu úpravy (TLI 25). Uloží polohu práve
/// upravovaného modulu a vráti polohu toho, na ktorý sa prepína — hráč tak
/// vie posunúť panel F8 dosky inam než panel F9 Canvasu, bez toho, že by
/// musel režim opustiť a znova doň vojsť.
#[tauri::command]
fn rezim_upravy_prepni_panel(
    app: AppHandle,
    kluc: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<serde_json::Value, String> {
    if !MODULOVE_PANELY.iter().any(|(k, _)| *k == kluc) {
        return Err(format!("Panel „{kluc}“ nepoznám."));
    }
    let mut kontext = {
        let stav = app.state::<Stav>();
        let k = stav.rezim_upravy_kontext.lock().unwrap().clone();
        k.ok_or_else(|| "Režim úpravy rozloženia nie je zapnutý.".to_string())?
    };

    let novy_css = {
        let stav = app.state::<Stav>();
        let mut n = stav.nastavenia.lock().unwrap();
        let css = suradnice::ObdlznikCss { x, y, w, h };
        let p = suradnice::css_na_box01(&css, &kontext.okno_hry, kontext.mierka);
        n.rozlozenie.uloz_polohu(
            &kontext.panel_kluc,
            &kontext.monitor_sig,
            Obdlznik01 { x: p.x, y: p.y, w: p.w, h: p.h },
        );
        n.normalizuj();
        let kopia = n.clone();
        drop(n);
        // Zapisuje sa hneď pri prepnutí, nie až na konci: keby appka medzitým
        // spadla alebo hráč zavrel hru, prácu na predošlom module by stratil.
        stav::uloz(&kopia)?;
        let obd = poloha_alebo_predvolena_modulu(&kopia, &kluc, &kontext.monitor_sig);
        suradnice::box01_na_css(obd.x, obd.y, obd.w, obd.h, &kontext.okno_hry, kontext.mierka)
    };

    kontext.panel_kluc = kluc.clone();
    *app.state::<Stav>().rezim_upravy_kontext.lock().unwrap() = Some(kontext);
    Ok(json!({"kluc": kluc, "panel": {"x": novy_css.x, "y": novy_css.y,
                                      "w": novy_css.w, "h": novy_css.h}}))
}

/// JS v okne `overlay` odpovedá na `rezim-upravy-nahlas` touto hodnotou — CSS
/// pixely panela PRESNE tak, ako ich hráč naťahal/zmenšil. Dokončí celé
/// uloženie: prepočíta panel aj prečíta skutočnú polohu mini/toast okien,
/// zapíše všetko na disk a ukončí režim úpravy.
#[tauri::command]
fn rezim_upravy_ulozit_panel(app: AppHandle, x: f64, y: f64, w: f64, h: f64) -> Result<(), String> {
    let kontext = app.state::<Stav>().rezim_upravy_kontext.lock().unwrap().clone();
    let Some(kontext) = kontext else {
        // Druhé hlásenie po už ukončenom režime (napr. dvojité Enter) — nie je
        // to chyba, jednoducho už niet čo ukladať.
        return Ok(());
    };

    {
        let stav = app.state::<Stav>();
        let mut n = stav.nastavenia.lock().unwrap();

        let css = suradnice::ObdlznikCss { x, y, w, h };
        let p = suradnice::css_na_box01(&css, &kontext.okno_hry, kontext.mierka);
        // TLI 25: ukladá sa pod kľúč PRÁVE upravovaného modulu, nie pod
        // spoločný `panel` — ten ostáva ako záloha pre moduly, ktoré hráč
        // nikdy samostatne neposunul.
        n.rozlozenie.uloz_polohu(
            &kontext.panel_kluc,
            &kontext.monitor_sig,
            Obdlznik01 { x: p.x, y: p.y, w: p.w, h: p.h },
        );

        for (label, panel) in [(MINI, PANEL_MINI), (PRIPOMIENKA, PANEL_PRIPOMIENKA)] {
            if let Some(okno) = app.get_webview_window(label) {
                if let (Ok(pos), Ok(size)) = (okno.outer_position(), okno.outer_size()) {
                    let css_fyz = suradnice::ObdlznikCss {
                        x: (pos.x - kontext.okno_hry.x) as f64,
                        y: (pos.y - kontext.okno_hry.y) as f64,
                        w: size.width as f64,
                        h: size.height as f64,
                    };
                    let p = suradnice::css_na_box01(&css_fyz, &kontext.okno_hry, 1.0);
                    n.rozlozenie.uloz_polohu(panel, &kontext.monitor_sig, Obdlznik01 { x: p.x, y: p.y, w: p.w, h: p.h });
                }
            }
        }

        n.normalizuj();
        let kopia = n.clone();
        drop(n);
        stav::uloz(&kopia)?;
    }

    ukonci_rezim_upravy(&app);
    Ok(())
}

/// Posuvník priehľadnosti v Nastaveniach — jedna hodnota pre všetky tri
/// panely (0.3–1.0), uplatní sa hneď, aj mimo režimu úpravy.
#[tauri::command]
fn uloz_priehladnost(app: AppHandle, hodnota: f64) -> Result<f64, String> {
    let kopia = {
        let stav = app.state::<Stav>();
        let mut n = stav.nastavenia.lock().unwrap();
        n.rozlozenie.nastav_priehladnost(hodnota);
        n.clone()
    };
    stav::uloz(&kopia)?;
    for label in [OVERLAY, MINI, PRIPOMIENKA] {
        let _ = app.emit_to(label, "priehladnost", json!({"priehladnost": kopia.rozlozenie.priehladnost}));
    }
    Ok(kopia.rozlozenie.priehladnost)
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
    let hotkey_trh = nove.hotkey_trh.clone();
    *stav.nastavenia.lock().unwrap() = nove;
    // `prehlas_hotkey` ruší VŠETKY skratky — canvas aj trh sa preto
    // registrujú až po nej, nie súbežne/pred. Ctrl+F8 (TLI 21) z rovnakého
    // dôvodu tiež: inak by zmena F8/F9/F10 v Nastaveniach appku o „Upraviť
    // rozloženie" nenávratne pripravila až do reštartu.
    let chyba1 = prehlas_hotkey(&app, &hotkey).err();
    let chyba2 = prehlas_hotkey_canvas(&app, &hotkey_canvas).err();
    let chyba3 = prehlas_hotkey_trh(&app, &hotkey_trh).err();
    let _ = app.global_shortcut().register(ctrl_f8());
    let chyby: Vec<String> = [chyba1, chyba2, chyba3].into_iter().flatten().collect();
    if chyby.is_empty() {
        Ok(())
    } else {
        Err(chyby.join(" "))
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

/// Zapíše chybu z práce s oknom do `okna.log` namiesto jej ticheho zahodenia
/// (TLI 23, bod 2 zadania).
fn loguj_chybu_okna(kontext: &str, chyba: &str) {
    stav::zapis_chybu_okna(tracker::unixovy_cas(), kontext, chyba);
}

/// Ukáže a zaostrí dané okno (nastavenia/hlavné) — spoločné pre tray menu aj
/// pre príkaz volaný z JS (tlačidlo „Otvoriť nastavenia“ v záložke Nastavenia
/// hlavného okna). Samotná logika je v `okna::ukaz_a_zaostri` (TLI 23) —
/// vytiahnutá do vlastného súboru, aby sa dala otestovať v
/// `tests/e2e_okna.rs` bez behu celej appky; tu sa len doplní logovanie.
fn ukaz_a_zaostri(app: &AppHandle, label: &str) {
    for (kontext, chyba) in okna::ukaz_a_zaostri(app, label) {
        loguj_chybu_okna(&kontext, &chyba);
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

/// Tlačidlo „Otvoriť log" v Nastaveniach (TLI 24, bod 4) — appka doteraz
/// nemala žiadny viditeľný log, takže aj toto tlačidlo je nové.
#[tauri::command]
fn otvor_priecinok_logov() -> Result<(), String> {
    otvor_priecinok(&denik::cesta_na_otvorenie().display().to_string())
}

#[cfg(windows)]
fn otvor_priecinok(cesta: &str) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(cesta)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Nepodarilo sa otvoriť priečinok: {e}"))
}

#[cfg(not(windows))]
fn otvor_priecinok(_cesta: &str) -> Result<(), String> {
    Err("Otváranie priečinka je dostupné len na Windowse.".to_string())
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
    let hotkey_trh_pri_starte = nacitane.hotkey_trh.clone();

    let skratky = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, skratka, udalost| {
            if udalost.state() != ShortcutState::Pressed {
                return;
            }

            // Ctrl+F8 (TLI 21) je trvalo zaregistrovaná a funguje nezávisle od
            // toho, čo je práve viditeľné — vybavuje sa PRED všetkým ostatným.
            if skratka == &ctrl_f8() {
                prepni_rezim_upravy(app);
                return;
            }

            // Kým hráč upravuje rozloženie, Esc aj Enter znamenajú „ulož a
            // skonči" namiesto svojho bežného významu (skry overlay / nič).
            if *app.state::<Stav>().rezim_upravy.lock().unwrap() {
                if skratka == &esc() || skratka == &enter() {
                    poziadaj_o_ulozenie_rozlozenia(app);
                }
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
                // Ktorý z troch hotkey práve spustil analýzu sa pozná podľa
                // zhody s AKTUÁLNE nastaveným `hotkey_canvas`/`hotkey_trh` (nie
                // podľa hodnoty pri štarte appky — nastavenia sa dajú zmeniť
                // bez reštartu). Keď sa reťazec nedá naparsovať (vrátane
                // prázdneho `hotkey_trh`, ktorý je nepovinný), nikdy sa
                // nezhodne a padá sa na Tartarus (bezpečný predvolený stav).
                let (hotkey_canvas, hotkey_trh) = {
                    let stav = app.state::<Stav>();
                    let n = stav.nastavenia.lock().unwrap();
                    (n.hotkey_canvas.clone(), n.hotkey_trh.clone())
                };
                let modul = if matches!(Shortcut::from_str(&hotkey_canvas), Ok(s) if &s == skratka) {
                    Modul::Canvas
                } else if !hotkey_trh.is_empty()
                    && matches!(Shortcut::from_str(&hotkey_trh), Ok(s) if &s == skratka)
                {
                    Modul::Ceny
                } else {
                    Modul::Tartarus
                };
                spusti_analyzu(app.clone(), modul);
            }
        })
        .build();

    tauri::Builder::default()
        .plugin(skratky)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Stav {
            nastavenia: Mutex::new(nacitane),
            beh: Mutex::new(false),
            zobrazene_od: Mutex::new(None),
            pocitadlo_zber: Mutex::new(0),
            poslane_behy: Mutex::new(std::collections::BTreeSet::new()),
            pripomienka_zobrazena_od: Mutex::new(None),
            rezim_upravy: Mutex::new(false),
            rezim_upravy_kontext: Mutex::new(None),
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
            ceny_sync_teraz,
            upload_teraz,
            db_export_teraz,
            uloz_prepinac,
            analyza_vyhladaj,
            analyza_monstra_podla_rarity,
            analyza_export_csv,
            analyza_export_json,
            analyza_taktiky,
            analyza_taktiky_export_csv,
            analyza_taktiky_export_json,
            taktika_pomenuj_alebo_zluc,
            tracker_nacitaj_stare_logy,
            ukaz_pripomienku,
            rezim_upravy_zapni,
            rezim_upravy_hotovo,
            rezim_upravy_prepni_panel,
            rezim_upravy_je_aktivny,
            rezim_upravy_obnov_predvolene,
            rezim_upravy_ulozit_panel,
            uloz_priehladnost,
            otvor_priecinok_logov
        ])
        // TLI 23: Tauri v2 by zatvorením (X) KAŽDÉ okno ZNIČILO — appka by
        // potom na druhý klik (tray menu, `otvor_nastavenia`) narazila na
        // `get_webview_window` vracajúce `None` a ticho by neurobila nič.
        // Platí to pre VŠETKY okná appky (zadanie, bod 1), nielen Nastavenia:
        // zatvorenie sa preto všade mení na skrytie, appka ho pri ďalšom
        // otvorení (`ukaz_a_zaostri`) len znova ukáže namiesto stavania od
        // nuly. Chyba pri skrývaní sa zaloguje (bod 2), nie ticho zahodí.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                // TLI 25: polohu hlavného okna si appka zapamätá v momente
                // zatvorenia — nie pri každom posune, lebo `Moved` chodí počas
                // ťahania desiatky ráz za sekundu a zapisovať pri ňom na disk
                // by bol zbytočný tlak.
                if window.label() == HLAVNE {
                    zapamataj_polohu_hlavneho(window);
                }
                if let Err(e) = window.hide() {
                    loguj_chybu_okna(&format!("zatvorenie okna '{}' (hide)", window.label()), &e.to_string());
                }
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();

            // Keď skratku z nastavení Windows neprijme, `prehlas_hotkey` sa sám
            // vráti k F8 — appka teda nikdy neostane bez ovládacej klávesy.
            // Canvas aj trh sa registrujú AŽ PO nej (tá ruší všetky skratky).
            let _ = prehlas_hotkey(&handle, &hotkey_pri_starte);
            let _ = prehlas_hotkey_canvas(&handle, &hotkey_canvas_pri_starte);
            let _ = prehlas_hotkey_trh(&handle, &hotkey_trh_pri_starte);
            // Ctrl+F8 (TLI 21) — registruje sa AŽ PO predošlých riadkoch,
            // lebo `prehlas_hotkey` ruší VŠETKY skratky (`unregister_all`) bez
            // ohľadu na modifikátor. `uloz_nastavenia` volá tú istú trojicu
            // znova (zmena skratky v Nastaveniach za behu appky) — preto aj
            // ona po nich musí Ctrl+F8 zaregistrovať nanovo, inak by po zmene
            // F8/F9/F10 appka o Ctrl+F8 nenávratne prišla až do reštartu.
            let _ = handle.global_shortcut().register(ctrl_f8());

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
            if let Some(okno) = app.get_webview_window(PRIPOMIENKA) {
                let _ = okno.set_ignore_cursor_events(true);
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
                    "hlavne" => {
                        obnov_polohu_hlavneho(app);
                        ukaz_a_zaostri(app, HLAVNE)
                    }
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
