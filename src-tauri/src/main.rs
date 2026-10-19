// Appka beží bez konzolového okna — inak by pri každom spustení (aj z tray ikony)
// vyskočilo čierne okno `cmd`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod denik;
mod guide;
mod klient;
mod okna;
#[cfg(windows)]
mod ocr;
#[cfg(windows)]
mod okno;
#[cfg(windows)]
mod snimka;
mod stav;
mod tracker;
mod zber_canvas;

use serde_json::json;
use stav::{KontrolaAktualizacie, RezimUpravyKontext, Stav};
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tli_overlay_core::nastavenia::Nastavenia;
use tli_overlay_core::nastavenia::PolohaOkna;
use tli_overlay_core::rozlozenie::{
    Obdlznik01, MODULOVE_PANELY, PANEL_CANVAS, PANEL_HLAVNY, PANEL_MINI, PANEL_NASTROJ,
    PANEL_OBJECTIVE, PANEL_POKLAD, PANEL_PORADCA, PANEL_PRIPOMIENKA, PANEL_TARTARUS,
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

/// Zaregistruje jednu skratku a VŽDY zaloguje presný výsledok (TLI 28, bod 1):
/// appka doteraz logovala len samotné STLAČENIE klávesy (`spusti_analyzu`),
/// nie pokus o registráciu — keď ju Windows pri štarte odmietol, log o tom
/// nehovoril vôbec nič a nedalo sa odlíšiť „hotkey nedorazil" od „hotkey sa
/// ani nezaregistroval". Vracia chybový text aj volajúcemu (nielen do logu),
/// nech ho appka vie ukázať aj hráčovi — ten text obsahuje dôvod od Windows
/// vrátane kódu chyby (`GetLastError`, cez `std::io::Error::last_os_error()`
/// v `global-hotkey`), nielen „nejde to".
fn zaregistruj_a_zaloguj(app: &AppHandle, novy: &str, nazov: &str) -> Result<(), String> {
    match app.global_shortcut().register(novy) {
        Ok(()) => {
            denik::zapis("hotkey", &format!("{nazov} „{novy}“ registrácia OK"));
            Ok(())
        }
        Err(chyba) => {
            let text = chyba.to_string();
            // `global-hotkey` na Windowse vracia pre tento prípad samostatný
            // variant `AlreadyRegistered`, ktorého `Display` obsahuje presne
            // túto frázu — bezpečnejšie rozlíšenie, keď sa k nám (cez
            // `tauri_plugin_global_shortcut::Error::GlobalHotkey(String)`)
            // dostane už len hotový text, nie pôvodný enum.
            let obsadena = text.to_lowercase().contains("already registered");
            denik::zapis(
                "hotkey",
                &format!(
                    "{nazov} „{novy}“ {}: {text}",
                    if obsadena { "OBSADENÁ iným programom" } else { "CHYBA registrácie" }
                ),
            );
            Err(text)
        }
    }
}

/// Prepíše globálnu skratku na novú (Tartarus — hlavná, historicky jediná).
/// Pri nezmyselnom zápise sa vracia k F8, nech appka neostane bez jedinej
/// ovládacej klávesy. Zruší VŠETKY skratky vrátane canvas — volajúci preto
/// musí po nej zavolať aj `prehlas_hotkey_canvas`, inak by canvas ostal
/// nezaregistrovaný (pozri `uloz_nastavenia` a `main`).
pub fn prehlas_hotkey(app: &AppHandle, novy: &str) -> Result<(), String> {
    let skratky = app.global_shortcut();
    let _ = skratky.unregister_all();

    if let Err(dovod) = zaregistruj_a_zaloguj(app, novy, "Tartarus") {
        let _ = zaregistruj_a_zaloguj(app, "F8", "Tartarus (záložná F8)");
        return Err(format!(
            "Skratku „{novy}“ Windows neprijal ({dovod}). Vraciam sa k F8."
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

    if let Err(dovod) = zaregistruj_a_zaloguj(app, novy, "Frozen Canvas") {
        let _ = zaregistruj_a_zaloguj(app, "F9", "Frozen Canvas (záložná F9)");
        return Err(format!(
            "Skratku pre Frozen Canvas „{novy}“ Windows neprijal ({dovod}) — môže kolidovať \
s hlavnou skratkou. Vraciam sa k F9."
        ));
    }
    Ok(())
}

/// Tretia skratka — ceny z obrazovky trhu (TLI 22). Na rozdiel od
/// `prehlas_hotkey`/`prehlas_hotkey_canvas` je to RUČNÝ fallback — hlavný
/// spôsob spustenia je automatická detekcia z logu (`tracker.rs`,
/// `odober_trh_otvoreny`), tento hotkey len zodpovedá zneniu zadania
/// („alebo na hotkey"). Od TLI 39 (`2026-10-18-1613-mcp.md`, bod 2) má
/// `Nastavenia::hotkey_trh` vlastnú predvolenú hodnotu „F10" a
/// `normalizuj()` prázdny reťazec na ňu vždy premení — guard na `is_empty()`
/// tu ostáva len ako obrana pri priamom volaní bez prechodu cez
/// `normalizuj()` (napr. budúci test), nie ako bežná prevádzková cesta.
/// Volá sa vždy AŽ PO `prehlas_hotkey` (ten `unregister_all()`-uje).
pub fn prehlas_hotkey_trh(app: &AppHandle, novy: &str) -> Result<(), String> {
    let novy = novy.trim();
    if novy.is_empty() {
        return Ok(());
    }
    if let Err(dovod) = zaregistruj_a_zaloguj(app, novy, "ceny z trhu") {
        return Err(format!(
            "Skratku pre ceny z trhu „{novy}“ Windows neprijal ({dovod}) — môže kolidovať \
s inou skratkou. Automatická detekcia z logu funguje ďalej bez nej."
        ));
    }
    Ok(())
}

/// Nájde konflikt MEDZI VLASTNÝMI tromi skratkami appky — teda dve polia
/// nastavené na ROVNAKÚ klávesu (TLI 39, bod 2, `2026-10-18-1613-mcp.md`).
/// Toto je iný prípad než to, čo rieši `zaregistruj_a_zaloguj`/`obsadena`:
/// ten rozpozná až odmietnutie od Windows (cudzí program ALEBO aj toto), ale
/// nevie pomenovať KTORÉ dva moduly appky si prekážajú a hlási to ako
/// „OBSADENÁ iným programom", aj keď ide o appku samu. Táto funkcia je čisto
/// textové porovnanie bez volania OS, beží PRED akoukoľvek registráciou a
/// pozná presné mená oboch strán konfliktu.
///
/// Priorita je Tartarus > Frozen Canvas > ceny z trhu — v tomto poradí boli
/// skratky do appky historicky pridané a Tartarus má navyše vlastný fallback
/// na F8 priamo v `prehlas_hotkey`, takže sa tu nikdy nevynecháva. Vracia
/// dvojicu (interné meno modulu pre `zaregistruj_vsetky_hotkeys`, hotové
/// slovenské hlásenie) pre KAŽDÝ nájdený konflikt — prázdny `hotkey_trh`
/// sa nikdy nepočíta ako konflikt (po `normalizuj()` sa to ani nestane, ale
/// funkcia nesmie spadnúť, ak ju niekto zavolá priamo na nenormalizovaný vstup).
fn konfliktne_moduly(hotkey: &str, hotkey_canvas: &str, hotkey_trh: &str) -> Vec<(&'static str, String)> {
    let mut sprava = Vec::new();
    if !hotkey_canvas.is_empty() && hotkey_canvas.eq_ignore_ascii_case(hotkey) {
        sprava.push((
            "canvas",
            format!(
                "Frozen Canvas „{hotkey_canvas}“ koliduje s klávesou pre Tartarus — appka ju \
teraz nezaregistrovala, zmeň jednu z klávesových skratiek v Nastaveniach (dovtedy funguje \
tray menu „Odfotiť Canvas teraz“)."
            ),
        ));
    }
    if !hotkey_trh.is_empty() {
        if hotkey_trh.eq_ignore_ascii_case(hotkey) {
            sprava.push((
                "trh",
                format!(
                    "Skratka pre ceny z trhu „{hotkey_trh}“ koliduje s klávesou pre Tartarus — \
appka ju teraz nezaregistrovala, zmeň jednu z klávesových skratiek v Nastaveniach \
(automatická detekcia z logu funguje ďalej bez nej)."
                ),
            ));
        } else if hotkey_trh.eq_ignore_ascii_case(hotkey_canvas) {
            sprava.push((
                "trh",
                format!(
                    "Skratka pre ceny z trhu „{hotkey_trh}“ koliduje s klávesou pre Frozen \
Canvas — appka ju teraz nezaregistrovala, zmeň jednu z klávesových skratiek v Nastaveniach \
(automatická detekcia z logu funguje ďalej bez nej)."
                ),
            ));
        }
    }
    sprava
}

/// Text zoznamu AKTÍVNYCH klávesových skratiek (TLI 39, bod 3) — spoločný
/// zdroj pre toast pri štarte aj pre položku v tray menu, nech oboje vždy
/// hovoria presne to isté. Modul, ktorý `konfliktne_moduly` vyradila z
/// registrácie, sa tu neukáže ako jeho (nefunkčná) klávesa, ale ako otvorený
/// konflikt — zoznam má hovoriť pravdu o tom, čo appka reálne počúva, nie
/// opisovať surové nastavenia.
fn text_prehladu_klaves(hotkey: &str, hotkey_canvas: &str, hotkey_trh: &str) -> String {
    let konflikty = konfliktne_moduly(hotkey, hotkey_canvas, hotkey_trh);
    let je_v_konflikte = |modul: &str| konflikty.iter().any(|(m, _)| *m == modul);
    let zobraz = |hodnota: &str, modul: &str| {
        if je_v_konflikte(modul) {
            "(konflikt — pozri Nastavenia)".to_string()
        } else {
            hodnota.to_string()
        }
    };
    format!(
        "{} Tartarus, {} Frozen Canvas, {} ceny z trhu, CONTROL+F8 Upraviť rozloženie",
        hotkey,
        zobraz(hotkey_canvas, "canvas"),
        zobraz(hotkey_trh, "trh"),
    )
}

/// Spoločné miesto, odkiaľ sa registrujú VŠETKY štyri skratky naraz (Tartarus,
/// Frozen Canvas, ceny z trhu, Ctrl+F8) — pri štarte appky aj pri každom
/// uložení Nastavení (TLI 28, bod 2a). Zapamätá si prípadné upozornenia do
/// `Stav::hotkey_upozornenia`, nech ich Nastavenia vedia ukázať červeným aj
/// vtedy, keď sa otvoria AŽ PO štarte appky — nielen priamo pri uložení
/// formulára, ktoré malo chybovú hlášku už predtým.
///
/// `posli_toast` je zapnuté LEN pri štarte: pri uložení Nastavení chybu
/// uvidí hráč rovno vo formulári, druhý toast navyše by bol len hluk.
fn zaregistruj_vsetky_hotkeys(
    app: &AppHandle,
    hotkey: &str,
    hotkey_canvas: &str,
    hotkey_trh: &str,
    posli_toast: bool,
) -> Vec<String> {
    // TLI 39, bod 2: konflikt MEDZI VLASTNÝMI skratkami sa zisťuje PRED
    // akoukoľvek registráciou — modul, ktorý tu prehrá, sa nižšie vôbec
    // neregistruje (nie je čo nechať „spadnúť" na inú klávesu, appka si
    // žiadnu vlastnú náhradu nevymýšľa, nech hráč jasne vidí, že treba ísť
    // do Nastavení).
    let konflikty = konfliktne_moduly(hotkey, hotkey_canvas, hotkey_trh);
    let preskocit_canvas = konflikty.iter().any(|(m, _)| *m == "canvas");
    let preskocit_trh = konflikty.iter().any(|(m, _)| *m == "trh");

    let mut chyby: Vec<String> = konflikty.into_iter().map(|(_, sprava)| sprava).collect();
    chyby.extend(prehlas_hotkey(app, hotkey).err());

    if preskocit_canvas {
        denik::zapis(
            "hotkey",
            &format!("Frozen Canvas „{hotkey_canvas}“ NEZAREGISTROVANÁ — koliduje s inou skratkou appky"),
        );
    } else {
        chyby.extend(prehlas_hotkey_canvas(app, hotkey_canvas).err());
    }

    if preskocit_trh {
        denik::zapis(
            "hotkey",
            &format!("ceny z trhu „{hotkey_trh}“ NEZAREGISTROVANÁ — koliduje s inou skratkou appky"),
        );
    } else {
        chyby.extend(prehlas_hotkey_trh(app, hotkey_trh).err());
    }

    // Ctrl+F8 (TLI 21) sa registruje cez typovaný `Shortcut`, nie cez reťazec
    // ako predošlé tri — ide o tú istú hodnotu, akú porovnáva handler
    // (`skratka == &ctrl_f8()`), preto sa tu nepoužíva `zaregistruj_a_zaloguj`
    // (ten by skratku musel znova naparsovať z textu).
    match app.global_shortcut().register(ctrl_f8()) {
        Ok(()) => denik::zapis("hotkey", "Upraviť rozloženie „CONTROL+F8“ registrácia OK"),
        Err(e) => denik::zapis(
            "hotkey",
            &format!("Upraviť rozloženie „CONTROL+F8“ CHYBA registrácie: {e}"),
        ),
    }

    *app.state::<Stav>().hotkey_upozornenia.lock().unwrap() = chyby.clone();
    let _ = app.emit("hotkey-upozornenia", json!({ "chyby": chyby }));
    if posli_toast {
        // TLI 39, bod 3: prehľad aktívnych kláves ide VŽDY, bez ohľadu na to,
        // či je niečo v chybe — presne toto bolo 18.10.2026 chýbajúce: appka
        // bežala s Tartarus na „DIGIT3" a hráč sa to nemal odkiaľ dozvedieť,
        // lebo žiadna chyba sa nehlásila (klávesa sa zaregistrovala v poriadku,
        // len to nebola tá, ktorú hráč čakal pod F8).
        posli_toast_hotkey(
            app,
            &format!("Aktívne klávesy — {}", text_prehladu_klaves(hotkey, hotkey_canvas, hotkey_trh)),
        );
        if !chyby.is_empty() {
            let text = chyby.join(" ");
            posli_toast_hotkey(app, &text);
            ukaz_pripomienku_toast(app, &text, "upozornenie", PRIPOMIENKA_UPOZORNENIE_S);
        }
    }
    chyby
}

/// Windows notifikácia (mimo appky, teda viditeľná aj keď appka sedí len
/// v tray a žiadne jej okno nie je zobrazené) — rovnaký vzor ako
/// `tracker::posli_windows_notifikaciu` pre pripomienku „Enable Log".
#[cfg(windows)]
fn posli_toast_hotkey(app: &AppHandle, text: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("Tvoj Tatko TLI Overlay")
        .body(text)
        .show();
}

#[cfg(not(windows))]
fn posli_toast_hotkey(_app: &AppHandle, _text: &str) {}

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
            // TLI 26, bod 2: meno monitora od OS (na Windowse `\\.\DISPLAY1`
            // a podobne) je súčasťou kľúča uloženej polohy — bez neho dva
            // fyzicky odlišné monitory s rovnakým rozlíšením (Richardove dva
            // 2560×1440) zdieľali jedno uložené rozloženie.
            nazov: m.name().cloned(),
        })
        .collect();
    suradnice::monitor_okna(obd, &monitory).cloned()
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
        // TLI 25: vyššie než pôvodných 92 px — pribudol zoznam „čo padlo".
        // TLI 50: ešte vyššie (196→220) — horný pásik na presun dostal
        // vlastný riadok namiesto prekrývania obsahu, takže potrebuje o kus
        // viac miesta navyše, aby sa doň zmestila aj hlavička a 3 riadky
        // dropov bez scrollovania pri predvolenej veľkosti.
        // Kto si mini-panel posunul ešte PRED touto zmenou, má uloženú
        // vlastnú (nižšiu) veľkosť — appka mu ju nemení, zväčší si ju sám
        // za úchyt v pravom dolnom rohu (funguje teraz aj mimo Ctrl+F8).
        PANEL_MINI => Obdlznik01 { x: 1620.0 / 1920.0, y: 20.0 / 1080.0, w: 280.0 / 1920.0, h: 220.0 / 1080.0 },
        PANEL_PRIPOMIENKA => {
            Obdlznik01 { x: 720.0 / 1920.0, y: 40.0 / 1080.0, w: 480.0 / 1920.0, h: 110.0 / 1080.0 }
        }
        // Hlavný panel aj všetky jeho modulové varianty (TLI 25) štartujú
        // z toho istého miesta — líšiť sa začnú až tým, kam ich hráč posunie.
        // TLI 34: vpravo (predtým ľavý horný roh 24/24, kde je herný text
        // „Objective / Trigger"), a POD predvolenou polohou mini-panela
        // (PANEL_MINI vyššie: y 20..216 v 1080 logických px), nie cez ňu —
        // musí sedieť s CSS defaultom `#panel` v ui/overlay.css (top 240px,
        // right 24px), inak by editačný režim štartoval inde než bežné
        // zobrazenie (nahlásil Richard, F8 screenshot 18.10. ~13:40).
        _ => Obdlznik01 {
            x: (1920.0 - 24.0 - 460.0) / 1920.0,
            y: 240.0 / 1080.0,
            w: 460.0 / 1920.0,
            h: 260.0 / 1080.0,
        },
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

/// Vynúti okno na VRCH pásma „vždy navrchu" bez toho, aby mu to vzalo focus
/// hre (TLI 31, `2026-10-18-1057-mcp.md`). `.show()` samo o sebe z-poradie
/// NEMENÍ — tao na Windowse ho volá s `SWP_NOZORDER` (rovnaký dôvod ako pri
/// `.set_position()`/`.set_size()`, TLI 26 bod 1). Okno preto zostáva tam, kde
/// bolo v pásme topmost naposledy — keď medzitým iný topmost (Overwolf,
/// Discord overlay, Logitech G-HUB — všetky bežia súčasne, TLI 28) dostal
/// vlastné `SetWindowPos(HWND_TOPMOST, …)`, posunul SA pred náš overlay a ten
/// potom sedí pod ním, aj keď je stále nad hrou.
///
/// `set_always_on_top(true)`, keď je príznak UŽ `true`, je v tao no-op —
/// `apply_diff` reaguje len na ZMENU hodnoty (XOR starého/nového stavu), takže
/// bez vypnutia najprv by sa `SetWindowPos` vôbec nezavolalo. Prepnutím
/// false→true appka vynúti nové `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`
/// (tao `window_state.rs`), ktoré okno vráti na vrch topmost pásma — a
/// `SWP_NOACTIVATE` je presne to, prečo sa namiesto toho nepoužíva
/// `.set_focus()` (ako pri mini/pripomienke v režime úpravy, TLI 26): tam je
/// focus-stealing prijateľný (hráč vtedy ťahá myšou panely), tu by hre odobral
/// klávesnicu uprostred hrania.
fn vynut_navrch(okno: &tauri::WebviewWindow) {
    let _ = okno.set_always_on_top(false);
    let _ = okno.set_always_on_top(true);
}

/// TLI 40 (`2026-10-18-1622-mcp.md`): koreň „F8 rada sa nevykreslí, ručné
/// spustenie áno". Rust aj JS stranu robia OBE cesty identicky (rovnaká
/// `spusti_analyzu` → `ukaz_overlay_nad` → `overlay.show()`, rovnaký
/// `requestAnimationFrame` v `overlay.js`) — rozdiel je v samotnom WebView2:
/// Chromium v topmost/`WS_EX_NOACTIVATE` okne, ktoré hráč nikdy neaktivoval,
/// vie DOM „vykresliť" interne a napriek tomu ho nikdy neprezentovať na
/// obrazovku (známe správanie okolo occlusion detekcie). Ručná cesta (klik na
/// tray/tlačidlo) funguje, lebo samotné kliknutie myšou cestou cez okno
/// nášho procesu ho krátko aktivuje a WebView2 sa „zobudí" — F8 ako globálna
/// skratka (`RegisterHotKey`) také okno nikdy nemá.
///
/// Oprava: urobíme to isté SAMI. `SetForegroundWindow` je dokumentovaný
/// spôsob, ako aktivovať aj `WS_EX_NOACTIVATE` okno („To activate the window,
/// use the SetActiveWindow or SetForegroundWindow function" — MSDN k danému
/// štýlu), a keďže ide o reakciu na práve stlačenú klávesu, Windows appke
/// právo na zmenu popredia priznáva (rovnaký dôvod, prečo to isté API bez
/// problémov používajú iné hotkey-spúšťané nástroje). Hneď potom sa popredie
/// vráti späť na to, čo bolo aktívne predtým (typicky hra) — žiadny vstup sa
/// nesimuluje a hráč stratu klávesnice nezbadá, trvá to mikrosekundy.
#[cfg(windows)]
fn aktivuj_a_vrat_fokus(overlay: &tauri::WebviewWindow) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

    // `WebviewWindow::hwnd()` by vrátilo HWND z inej (vnútornej) verzie kraty
    // `windows`, než akú máme pripnutú my — preto sa HWND berie cez
    // `raw-window-handle` (verzovo nezávislé) ako obyčajné číslo a z neho sa
    // postaví HWND z NAŠEJ verzie.
    let Ok(handle) = overlay.window_handle() else { return };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else { return };
    let hwnd_overlay = HWND(win32.hwnd.get() as *mut std::ffi::c_void);

    // BEZPECNOST: len prepnutie, ktore okno je aktivne — ziadna cudzia pamat,
    // ziadny simulovany vstup.
    unsafe {
        let hwnd_povodne = GetForegroundWindow();
        let _ = SetForegroundWindow(hwnd_overlay);
        if !hwnd_povodne.is_invalid() && hwnd_povodne != hwnd_overlay {
            let _ = SetForegroundWindow(hwnd_povodne);
        }
    }
}

#[cfg(not(windows))]
fn aktivuj_a_vrat_fokus(_overlay: &tauri::WebviewWindow) {}

/// Oneskorenie pred kontrolou skutočnej prezentácie (TLI 40, bod 3) — nech
/// WebView2/DWM stihnú doručiť frame na obrazovku skôr, než appka odfotí
/// druhý raz. Rádovo menej než `autoskrytie_s` (sekundy), takže na celkovú
/// dobu zobrazenia panela nemá prakticky žiadny vplyv.
const OVERENIE_PREZENTACIE_ONESKORENIE_MS: u64 = 400;
/// TLI 44, bod 3: druhé `WM_HOTKEY` na tú istú skratku do tohto okna od
/// predošlého sa ignoruje úplne potichu (žiadna analýza, žiadne skrytie) —
/// je to key-repeat Windowsu, nie druhé stlačenie. 500 ms je výrazne nad
/// bežným key-repeat intervalom (~30-50 ms) a výrazne pod tým, čo by hráč
/// vnímal ako samostatný druhý stlak.
const HOTKEY_DEBOUNCE_MS: u64 = 500;

/// Čistá logika debounce (TLI 44, bod 3) — žiadne OS volanie, testovateľné aj
/// mimo Windowsu. `posledny` je (skratka, kedy) z MINULÉHO doručenia, `nova`
/// je skratka, ktorá práve prišla, `teraz` je jej čas. Vráti `true`, keď ide
/// o echo tej istej skratky do `HOTKEY_DEBOUNCE_MS` od predošlej.
fn hotkey_debounovany(posledny: Option<(Shortcut, Instant)>, nova: &Shortcut, teraz: Instant) -> bool {
    match posledny {
        Some((predosla, kedy)) => {
            &predosla == nova && teraz.duration_since(kedy) < Duration::from_millis(HOTKEY_DEBOUNCE_MS)
        }
        None => false,
    }
}
/// Nad touto hranicou podielu odlišných pixelov appka usúdi, že sa panel na
/// obrazovke NAOZAJ objavil. Hra sama osebe (bez overlayu) sa za 400 ms zvykne
/// zmeniť len nepatrne (kurzor, drobná animácia na pozadí) — samotný panel
/// rady prekrýva stovky až tisíce pixelov, takže práh je nastavený nízko, ale
/// nie na nulu (nulová zmena by vedela nastať aj pri úplne statickej hre).
const PREZENTACIA_PODIEL_PRAH: f64 = 0.0005;
/// Rozdiel na kanál, od ktorého sa pixel počíta ako „zmenený" — šum snímania
/// obrazovky (dithering, kompresia farieb pri HDR/ICC profiloch) vie bežne
/// hodiť ±5-10, bez prahu by appka hlásila zmenu aj nad úplne pokojnou hrou.
const PREZENTACIA_ROZDIEL_PRAH: u8 = 20;

/// Podiel odlišných pixelov voči snímke spred zobrazenia — `None`, keď sa
/// obdĺžnik nedá znova odfotiť (napr. zmenil sa rozmer okna hry medzitým).
#[cfg(windows)]
fn zmena_oproti(obdlznik: &OknoFyz, pred: &[u8]) -> Result<Option<f64>, String> {
    let po = snimka::zachyt(obdlznik)?;
    Ok(tli_overlay_core::obraz::percento_odlisnych_pixelov(
        pred,
        &po.rgb,
        PREZENTACIA_ROZDIEL_PRAH,
    ))
}

/// Čisté vyhodnotenie podielu zmeny (TLI 44, bod 2) — žiadne OS volanie,
/// testovateľné aj mimo Windowsu. Oddelené od `over_prezentaciu`, nech sa
/// hranica ÁNO/NIE/nedá-sa-overiť dá overiť bez skutočného snímania obrazovky.
#[derive(Debug, Clone, Copy, PartialEq)]
enum VysledokPrezentacie {
    Ano,
    Nie,
    NedaSaOverit,
}

fn vyhodnot_prezentaciu(podiel: Option<f64>) -> VysledokPrezentacie {
    match podiel {
        Some(p) if p > PREZENTACIA_PODIEL_PRAH => VysledokPrezentacie::Ano,
        Some(_) => VysledokPrezentacie::Nie,
        None => VysledokPrezentacie::NedaSaOverit,
    }
}

/// TLI 40, bod 3 + TLI 44, bod 2: appka nevie priamo nazrieť do WebView2, či
/// sa panel naozaj zobrazil — vie len to, čo je NAOZAJ na obrazovke. Odfotí
/// ten istý obdĺžnik znova (`pred` je snímka spred zobrazenia overlayu,
/// zachytená ešte pred prvým `ukaz_overlay_nad` v `spusti_analyzu`) a porovná.
/// Beží už v pozadí (appka je v tomto bode mimo `beh` zámku), takže krátky
/// `sleep` nezdrží ani tray, ani ďalší hotkey.
///
/// TLI 44: keď prvá kontrola povie NIE, appka to už neberie len na vedomie —
/// skúsi presne tú istú poistku ako pri prvom zobrazení (`aktivuj_a_vrat_fokus`,
/// TLI 40) ešte raz a po rovnakom oneskorení overí znova. Je to fallback pre
/// prípad, že by WebView2 zaspalo aj NAPRIEK pulzu v `zobraz_vysledok` — appka
/// vtedy neostane ticho stáť pri známom zlyhaní, skúsi sa prebrať sama.
#[cfg(windows)]
fn over_prezentaciu(app: &AppHandle, obdlznik: &OknoFyz, pred: &[u8], zdroj: &'static str) {
    std::thread::sleep(std::time::Duration::from_millis(OVERENIE_PREZENTACIE_ONESKORENIE_MS));
    let podiel = match zmena_oproti(obdlznik, pred) {
        Ok(podiel) => podiel,
        Err(e) => {
            denik::zapis("zobraz", &format!("frame prezentovaný: odfotenie na overenie zlyhalo ({e})"));
            return;
        }
    };
    match vyhodnot_prezentaciu(podiel) {
        VysledokPrezentacie::Ano => denik::zapis(
            "zobraz",
            &format!(
                // `podiel` je vždy `Some` tu — `Ano` vzniká len z `Some(_)` v `vyhodnot_prezentaciu`.
                "frame prezentovaný: ÁNO ({:.3} % obrazovky sa zmenilo, zdroj={zdroj})",
                podiel.unwrap_or(0.0) * 100.0
            ),
        ),
        VysledokPrezentacie::NedaSaOverit => denik::zapis(
            "zobraz",
            "frame prezentovaný: nedá sa overiť (zmenil sa rozmer okna hry medzitým)",
        ),
        VysledokPrezentacie::Nie => {
            denik::zapis(
                "zobraz",
                &format!(
                    "frame prezentovaný: NIE — WebView2 asi stále neprezentuje ({:.3} %, zdroj={zdroj}), \
skúšam fallback aktiváciu",
                    podiel.unwrap_or(0.0) * 100.0
                ),
            );
            // TLI 44, bod 2: presne tá istá poistka ako pri prvom zobrazení
            // (`aktivuj_a_vrat_fokus`, TLI 40), skúsená ešte raz — pre prípad,
            // že by WebView2 zaspalo aj NAPRIEK pulzu v `zobraz_vysledok`.
            if let Some(overlay) = app.get_webview_window(OVERLAY) {
                aktivuj_a_vrat_fokus(&overlay);
            }
            std::thread::sleep(std::time::Duration::from_millis(OVERENIE_PREZENTACIE_ONESKORENIE_MS));
            match zmena_oproti(obdlznik, pred) {
                Ok(podiel2) => denik::zapis(
                    "zobraz",
                    &match vyhodnot_prezentaciu(podiel2) {
                        VysledokPrezentacie::NedaSaOverit => {
                            "frame prezentovaný po fallbacku: nedá sa overiť (zmenil sa rozmer okna hry medzitým)"
                                .to_string()
                        }
                        v => format!(
                            "frame prezentovaný po fallbacku: {} ({:.3} %, zdroj={zdroj})",
                            if v == VysledokPrezentacie::Ano { "ÁNO" } else { "NIE" },
                            podiel2.unwrap_or(0.0) * 100.0
                        ),
                    },
                ),
                Err(e) => denik::zapis(
                    "zobraz",
                    &format!("frame prezentovaný po fallbacku: odfotenie zlyhalo ({e})"),
                ),
            }
        }
    }
}

#[cfg(not(windows))]
fn over_prezentaciu(_app: &AppHandle, _obdlznik: &OknoFyz, _pred: &[u8], _zdroj: &'static str) {}

/// Postaví overlay presne nad okno hry a ukáže ho. Pozícia aj veľkosť idú vo
/// FYZICKÝCH pixeloch — logické by sa na monitore so 150 % DPI rozišli s hrou.
///
/// `s_rozlozenim` rozlišuje skutočné okno hry od núdzového nahradného
/// obdĺžnika v `ukaz_chybu` (keď sa hra vôbec nenašla) — uložené rozloženie
/// (TLI 21) je zlomok SKUTOČNÉHO okna hry, aplikovať ho na malý núdzový box by
/// dalo nezmyselné súradnice.
fn ukaz_overlay_nad(app: &AppHandle, obd: &OknoFyz, s_rozlozenim: bool, panel_kluc: &str) {
    if let Some(overlay) = app.get_webview_window(OVERLAY) {
        let poz_chyba = overlay.set_position(PhysicalPosition::new(obd.x, obd.y)).err();
        let vel_chyba = overlay.set_size(PhysicalSize::new(obd.sirka, obd.vyska)).err();
        let _ = overlay.set_ignore_cursor_events(true);
        let show_chyba = overlay.show().err();
        vynut_navrch(&overlay);
        aktivuj_a_vrat_fokus(&overlay);
        let _ = app.global_shortcut().register(esc());
        *app.state::<Stav>().zobrazene_od.lock().unwrap() = Some(Instant::now());

        let monitor = monitor_okna_app(app, obd)
            .and_then(|m| m.nazov)
            .unwrap_or_else(|| "neznámy".to_string());
        match (poz_chyba, vel_chyba, show_chyba) {
            (None, None, None) => denik::zapis(
                "zobraz",
                &format!(
                    "overlay panel={panel_kluc} @ ({},{}) {}x{}, monitor {monitor}, rozlozenie={s_rozlozenim}",
                    obd.x, obd.y, obd.sirka, obd.vyska
                ),
            ),
            (p, v, s) => denik::zapis(
                "zobraz",
                &format!(
                    "overlay panel={panel_kluc}: CHYBA pri zobrazení — poloha={p:?} velkost={v:?} show={s:?}"
                ),
            ),
        }

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

/// TLI 44 (`2026-10-18-2331-mcp.md`): JEDNA funkcia, ktorou ide KAŽDÝ typ
/// výsledku Tartarusu (doska, výber nástroja/pokladu, objective, poradca) —
/// predtým každá vetva v `spusti_analyzu` len poslala polohu panela a
/// rovno emitovala `stav`, zatiaľ čo WebView2 sa „zobudilo" (`aktivuj_a_vrat_fokus`,
/// TLI 40) LEN RAZ, na úplnom začiatku analýzy (prvý `ukaz_overlay_nad` s
/// placeholderom „Snímam…"). Pri rýchlych typoch (doska, nástroj) to stíhalo —
/// pri `poradca` (volá model DVA razy, pozri `ui/overlay.js` komentár o
/// cieľovej latencii do 8 s) a `objective` (katalógová zhoda navyše) odpoveď
/// prichádzala neskôr, než WebView2 v `WS_EX_NOACTIVATE` okne znova „zaspalo"
/// (tá istá occlusion ako v TLI 40) — panel sa interne vykreslil (DOM aj CSS
/// sedeli, `ui/overlay.js::logPanelVykreslene` to aj zalogovalo), ale sa
/// NEPREZENTOVAL na obrazovku. Táto funkcia preto pulzne zobudí WebView2
/// ZNOVA, tesne PRED odoslaním reálneho obsahu — nie na začiatku — takže
/// funguje bez ohľadu na to, ako dlho server trval.
fn zobraz_vysledok(app: &AppHandle, panel_kluc: &str, obd: &OknoFyz, mierka: f64, stav_json: serde_json::Value) {
    posli_polohu_panela(app, obd, mierka, panel_kluc);
    if let Some(overlay) = app.get_webview_window(OVERLAY) {
        vynut_navrch(&overlay);
        aktivuj_a_vrat_fokus(&overlay);
    }
    let _ = app.emit("stav", stav_json);
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
///
/// `dovod` ide do denníka (TLI 58) — bez neho sa v logu nedalo rozlíšiť, či
/// panel zhasol na klik hráča, na Esc, alebo ho zhasol poistný časovač; a
/// práve to je prvá otázka, keď hráč hlási „zmizlo mi to samo".
fn skry(app: &AppHandle, dovod: &str) {
    let bol_viditelny = app
        .get_webview_window(OVERLAY)
        .and_then(|o| o.is_visible().ok())
        .unwrap_or(false);
    if let Some(overlay) = app.get_webview_window(OVERLAY) {
        let _ = overlay.hide();
    }
    let _ = app.global_shortcut().unregister(esc());
    *app.state::<Stav>().zobrazene_od.lock().unwrap() = None;
    // Logovať aj skrytie už skrytého okna by denník zaplavilo (Esc/hotkey
    // chodia aj nad prázdnou plochou) — zapisuje sa len skutočná zmena.
    if bol_viditelny {
        denik::zapis("zobraz", &format!("skryté (dôvod={dovod})"));
    }
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
            skry(&app, "timeout");
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// TLI 58 — skrytie rady až po kliku myšou
// ─────────────────────────────────────────────────────────────────────────────

/// Poistka pre režim „skry až po kliku" — po tomto čase bez kliku panel
/// zhasne sám. Zámerne NIE je to `autoskrytie_s` z nastavení: tam smie byť
/// nula („nikdy sám"), a keby detekcia kliku zlyhala (vzdialená plocha, iný
/// vstup než myš), ostal by panel nad hrou svietiť navždy.
const SKRYTIE_KLIKOM_POISTKA_S: u64 = 120;

/// Ako často sa pýtame Windowsu na stav tlačidiel myši. 25 ms je pod prahom
/// postrehnuteľnosti a stojí rádovo nič — `GetAsyncKeyState` je len čítanie
/// stavovej tabuľky, nie práca so vstupnou frontou.
const KROK_POLLINGU_MS: u64 = 25;

/// Fáza čakania na klik. Rozdelené na dve preto, že overlay sa často zobrazí
/// PRÁVE počas kliku (hráč klikne na položku v tray menu „Odfotiť teraz",
/// alebo drží tlačidlo z hry) — keby sa bralo prosté „tlačidlo je dole",
/// panel by zhasol v tej istej milisekunde, v ktorej sa objavil. Preto sa
/// najprv čaká, kým sú obe tlačidlá hore, a až potom sa počíta prvý stlak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FazaKliku {
    /// Niektoré tlačidlo bolo dole už v okamihu zobrazenia — čaká sa na pustenie.
    CakamNaPustenie,
    /// Obe tlačidlá sú hore — najbližší stlak panel zhasne.
    CakamNaStlacenie,
}

/// Jeden krok stavového automatu: vráti novú fázu a `true`, keď sa má skryť.
///
/// Oddelené od Windows API zámerne — takto sa dá otestovať na Linuxe v CI
/// (rovnaký vzor ako `hotkey_debounovany`), bez myši a bez `GetAsyncKeyState`.
pub(crate) fn krok_cakania_na_klik(faza: FazaKliku, stlacene: bool) -> (FazaKliku, bool) {
    match (faza, stlacene) {
        (FazaKliku::CakamNaPustenie, true) => (FazaKliku::CakamNaPustenie, false),
        (FazaKliku::CakamNaPustenie, false) => (FazaKliku::CakamNaStlacenie, false),
        (FazaKliku::CakamNaStlacenie, true) => (FazaKliku::CakamNaStlacenie, true),
        (FazaKliku::CakamNaStlacenie, false) => (FazaKliku::CakamNaStlacenie, false),
    }
}

/// Je práve stlačené ľavé alebo pravé tlačidlo myši?
///
/// Zadanie TLI 58 pripúšťalo globálny low-level hook myši AJ polling cez
/// `GetAsyncKeyState` — zvolený je polling, a to z dvoch dôvodov. (1) Hook sa
/// inštaluje funkciou, ktorú CI krok „Pravidlá ToS" zakazuje menovite spolu so
/// simuláciou vstupu; appka sa k vstupu hráča nesmie priblížiť ani omylom.
/// (2) Hook navyše vyžaduje VLASTNÚ message loop v tom istom vlákne a Windows
/// ho pri prekročení `LowLevelHooksTimeout` ticho odpojí — teda presne tá
/// trieda chýb, pri ktorej by appka hráčovi spomalila vstup do hry. Polling je
/// len ČÍTANIE stavovej tabuľky: do cesty kliku nevstupuje vôbec, nič
/// neodchytáva a nič nemení, takže klik ide do hry ďalej bez zmeny (zadanie:
/// „hook nič neblokuje").
#[cfg(windows)]
pub(crate) fn tlacidlo_mysi_dole() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    // Vysoký bit = klávesa/tlačidlo je PRÁVE dole. Nízky bit („stlačené od
    // posledného volania") sa zámerne nepoužíva — ten si Windows nuluje pri
    // každom čítaní, takže by sa oň pretekali iné procesy aj naše vlastné
    // vlákno a stlak by sa občas stratil.
    unsafe {
        (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_RBUTTON.0 as i32) as u16 & 0x8000) != 0
    }
}

/// Mimo Windowsu sa klik nedá zistiť — overlay tam aj tak nebeží, appka sa
/// na Linuxe kompiluje len kvôli testom a `cargo check` v CI.
#[cfg(not(windows))]
pub(crate) fn tlacidlo_mysi_dole() -> bool {
    false
}

/// Overlay ostane svietiť, kým hráč neklikne (ľavým alebo pravým tlačidlom
/// kdekoľvek na obrazovke). Poistky: `SKRYTIE_KLIKOM_POISTKA_S` sekúnd bez
/// kliku a Esc (ten rieši handler skratiek, nie toto vlákno).
///
/// Rovnako ako `naplanuj_skrytie` porovnáva značku času zobrazenia — keď
/// medzitým príde nová analýza, toto vlákno novší panel nezhasne.
fn naplanuj_skrytie_po_kliku(app: AppHandle, znacka: Instant) {
    std::thread::spawn(move || {
        let zaciatok = Instant::now();
        let poistka = std::time::Duration::from_secs(SKRYTIE_KLIKOM_POISTKA_S);
        let mut faza = if tlacidlo_mysi_dole() {
            FazaKliku::CakamNaPustenie
        } else {
            FazaKliku::CakamNaStlacenie
        };
        loop {
            // Panel medzitým zhasol (Esc, hotkey) alebo ho nahradila novšia
            // analýza — toto vlákno nemá čo riešiť a končí.
            if *app.state::<Stav>().zobrazene_od.lock().unwrap() != Some(znacka) {
                return;
            }
            if zaciatok.elapsed() >= poistka {
                skry(&app, "timeout");
                return;
            }
            let (nova, skryt) = krok_cakania_na_klik(faza, tlacidlo_mysi_dole());
            faza = nova;
            if skryt {
                skry(&app, "klik");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(KROK_POLLINGU_MS));
        }
    });
}

/// Naplánuje skrytie podľa nastavenia hráča (TLI 58): buď čaká na klik myšou
/// (predvolené), alebo odpočítava `autoskrytie_s` sekúnd ako doteraz.
fn naplanuj_skrytie_podla_nastavenia(app: AppHandle, znacka: Instant, n: &Nastavenia) {
    if n.skryt_radu_klikom {
        naplanuj_skrytie_po_kliku(app, znacka);
    } else {
        naplanuj_skrytie(app, znacka, n.autoskrytie_s);
    }
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

/// Odkiaľ prišiel popud na analýzu — TLI 40 (`2026-10-18-1622-mcp.md`, bod 3):
/// predtým appka logovala rovnaké „Tartarus stlačené" pre hotkey AJ pre ručné
/// spustenie z tray/tlačidla, takže sa z logu nedalo zistiť, ktorou cestou
/// analýza prišla — presne to bolo treba odlíšiť pri hľadaní, prečo F8 funguje
/// inak než ručné tlačidlo.
#[cfg(windows)]
pub(crate) fn spusti_analyzu(app: AppHandle, modul: Modul, zdroj: &'static str) {
    // Zapísané VŽDY, ako prvé — nezávisle od všetkých early-returnov nižšie.
    // Predtým appka nemala žiadnu stopu po tom, že hotkey vôbec dorazil do
    // Rustu (TLI 24, `2026-10-17-1238-mcp.md` bod 1/4): keď appka „nič
    // nespraví", nedalo sa ani odlíšiť „hotkey nedorazil" od „appka ho
    // dostala a niečo potichu zlyhalo".
    denik::zapis("hotkey", &format!("{modul:?} stlačené (zdroj={zdroj})"));

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
    // požiadavky naraz — server púšťa 30 za minútu a zámok je zdieľaný medzi
    // Tartarus aj Frozen Canvas, nemajú bežať naraz nad tým istým oknom.
    //
    // TLI 24: tento zámok je JEDINÉ miesto, kde appka vie úplne potichu
    // „nič nespraviť" (žiadny event, žiadny log) — keby z nejakého dôvodu
    // ostal `true` navždy (napr. nečakaný panic vo vlákne nižšie pred jeho
    // vrátením na `false`), appka by od tej chvíle na KAŽDÝ ďalší F8/F9
    // mlčala presne tak, ako to opísal Richard. Preto je zápis do logu tu
    // PRED early-returnom — keby sa to niekedy zopakovalo, log to ukáže.
    //
    // TLI 41: požiadavka, ktorá príde, kým predošlá ešte nedobehla, sa už
    // NEZAHADZUJE ticho — uloží sa ako `cakajuci_dalsi` a spustí sa HNEĎ po
    // dobehnutí tej prebiehajúcej (koniec funkcie nižšie). Keď medzitým
    // príde ešte jedna, nahradí tú predchádzajúcu — vždy vyhráva POSLEDNÁ
    // snímka, nie prvé stlačenie (Richard 18.10.2026: rýchle opakované
    // Ctrl+Q hlásilo chybu namiesto toho, aby jednoducho počkalo).
    {
        let stav = app.state::<Stav>();
        let mut beh = stav.beh.lock().unwrap();
        if *beh {
            *stav.cakajuci_dalsi.lock().unwrap() = Some((modul, zdroj));
            denik::zapis(
                "hotkey",
                &format!("{modul:?} čaká na dobehnutie predošlej analýzy (zdroj={zdroj})"),
            );
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
                        let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obdlznik, mierka);
                        zobraz_vysledok(
                            &app,
                            PANEL_NASTROJ,
                            &obdlznik,
                            mierka,
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
                    // TLI 35: obrazovka „Select a Warden's Treasure" — rovnaký
                    // princíp ako výber nástroja vyššie, len vlastná uložená
                    // poloha panela a vlastné pole v odpovedi (`poklady`, nie
                    // `nastroje`) — appka síce kreslí oboje rovnako (JS si
                    // zoberie, ktoré z dvoch polí príde), ale server ich drží
                    // oddelené, lebo výber daru sa počíta inak (Monte Carlo nad
                    // AKTUÁLNOU doskou, keď je po ruke, nie bayesovský katalóg).
                    Ok(v) if v.analyza.obrazovka == "vyber_pokladu" => {
                        denik::zapis(
                            "poklad",
                            &format!(
                                "výber pokladu: {} kartičiek, odporúčam {}",
                                v.analyza.poklady.len(),
                                v.analyza
                                    .odporucanie
                                    .as_ref()
                                    .map(|o| o.nazov.clone())
                                    .unwrap_or_else(|| "nič (nepoznám ani jednu)".to_string())
                            ),
                        );
                        let karticky: Vec<_> = api::pouzitelne_karticky_pokladu(&v.analyza)
                            .into_iter()
                            .map(|k| {
                                let b = k.box_.expect("filtrované pouzitelne_karticky_pokladu");
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
                        let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obdlznik, mierka);
                        zobraz_vysledok(
                            &app,
                            PANEL_POKLAD,
                            &obdlznik,
                            mierka,
                            json!({
                                "faza": "hotovo_poklad",
                                "analyza": v.analyza,
                                "karticky": karticky,
                                "mierka": mierka,
                                "sirka": sirka_log,
                                "vyska": vyska_log,
                                "trvanie_ms": v.trvanie_ms,
                            }),
                        );
                    }
                    // TLI 37: obrazovka „Objective" (cieľ behu) na začiatku drilu.
                    // Kliknúť sa dá len na Confirm, takže rámik je triviálny —
                    // cenné je, že si appka cieľ zapamätala a od tej chvíle
                    // podľa neho radí. Kreslí sa TOU ISTOU cestou ako kartičky
                    // nástroja/daru/poradcu (`moznosti` + bounding box), len
                    // s vlastným panelom a vlastnou fázou, aby sa v nej dalo
                    // ukázať potvrdenie a plán.
                    Ok(v) if v.analyza.obrazovka == "objective" => {
                        denik::zapis(
                            "objective",
                            &format!(
                                "cieľ behu: {} ({}), {}",
                                v.analyza
                                    .objective
                                    .as_ref()
                                    .map(|o| o.anglicky.clone())
                                    .unwrap_or_else(|| "neprečítaný".to_string()),
                                v.analyza
                                    .objective
                                    .as_ref()
                                    .map(|o| o.podmienka.clone())
                                    .unwrap_or_default(),
                                v.analyza.potvrdenie
                            ),
                        );
                        let karticky: Vec<_> = api::pouzitelne_moznosti(&v.analyza)
                            .into_iter()
                            .map(|k| {
                                let b = k.box_.expect("filtrované pouzitelne_moznosti");
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
                        let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obdlznik, mierka);
                        zobraz_vysledok(
                            &app,
                            PANEL_OBJECTIVE,
                            &obdlznik,
                            mierka,
                            json!({
                                "faza": "hotovo_objective",
                                "analyza": v.analyza,
                                "karticky": karticky,
                                "mierka": mierka,
                                "sirka": sirka_log,
                                "vyska": vyska_log,
                                "trvanie_ms": v.trvanie_ms,
                            }),
                        );
                    }
                    // TLI 36: univerzálny poradca. Server obrazovku nezaradil pod
                    // žiadny špecializovaný modul (alebo pod ním nemal čo poradiť)
                    // a poslal namiesto toho možnosti s bounding boxmi. Appka ich
                    // kreslí PRESNE tak ako kartičky nástroja a daru — tá istá
                    // funkcia v `overlay.js`, len iné pole v odpovedi. Richard
                    // chcel „nech mi vždy pomôže pri hocijakom výbere", nie druhú
                    // sadu grafiky.
                    Ok(v) if v.analyza.obrazovka == "poradca" => {
                        denik::zapis(
                            "poradca",
                            &format!(
                                "neznáma obrazovka ({}): {} možností, odporúčam {}",
                                if v.analyza.typ_obrazovky.is_empty() {
                                    "bez typu"
                                } else {
                                    &v.analyza.typ_obrazovky
                                },
                                v.analyza.moznosti.len(),
                                v.analyza
                                    .odporucanie
                                    .as_ref()
                                    .map(|o| o.nazov.clone())
                                    .unwrap_or_else(|| "nič".to_string())
                            ),
                        );
                        let karticky: Vec<_> = api::pouzitelne_moznosti(&v.analyza)
                            .into_iter()
                            .map(|k| {
                                let b = k.box_.expect("filtrované pouzitelne_moznosti");
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
                        let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obdlznik, mierka);
                        zobraz_vysledok(
                            &app,
                            PANEL_PORADCA,
                            &obdlznik,
                            mierka,
                            json!({
                                "faza": "hotovo_poradca",
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

                        // TLI 32: tlačidlo spodnej lišty namiesto zvýraznenia
                        // cely — server priradil odporúčaniu (a prípadne prvej
                        // alternatíve) index do `akcie_dole`, appka ho len
                        // premení na CSS súradnice tým istým prepočtom ako pri
                        // celách. Keď sa zhoda nenašla (iný panel, neznáma
                        // akcia), `tlacidlo_best`/`tlacidlo_alt` ostanú `None`
                        // a `overlay.js` sa vráti k pôvodnému zvýrazneniu cely.
                        let najdi_tlacidlo_json = |index: Option<i64>| {
                            api::tlacidlo_podla_indexu(&v.analyza, index).map(|t| {
                                let css = suradnice::box01_na_css(t.x, t.y, t.w, t.h, &obdlznik, mierka);
                                json!({"x": css.x, "y": css.y, "w": css.w, "h": css.h,
                                       "text": t.text.clone()})
                            })
                        };
                        let best_idx = v.analyza.odporucanie.as_ref().and_then(|o| o.tlacidlo);
                        let alt_idx = v.analyza.alternativy.first().and_then(|a| a.tlacidlo);
                        let tlacidlo = najdi_tlacidlo_json(best_idx);
                        // Druhé najlepšie sa kreslí len vtedy, keď je to INÉ
                        // tlačidlo než najlepšie — inak by sa dve žiary
                        // prekrývali nad tým istým miestom.
                        let tlacidlo_alt = if alt_idx.is_some() && alt_idx != best_idx {
                            najdi_tlacidlo_json(alt_idx)
                        } else {
                            None
                        };

                        let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obdlznik, mierka);
                        // TLI 31: predtým táto, najčastejšia vetva (doska) na
                        // rozdiel od „vyber_nastroja"/„ine" nelogovala vôbec
                        // nič — po „analyza OK" tak v logu nebola žiadna stopa
                        // o tom, že appka sa chystá niečo vykresliť.
                        denik::zapis(
                            "zobraz",
                            &format!("doska: {} polí, tlačidlo {}, emitujem stav=hotovo",
                                     obdlzniky.len(), if tlacidlo.is_some() { "nájdené" } else { "nenájdené" }),
                        );
                        zobraz_vysledok(
                            &app,
                            PANEL_TARTARUS,
                            &obdlznik,
                            mierka,
                            json!({
                                "faza": "hotovo",
                                "analyza": v.analyza,
                                "obdlzniky": obdlzniky,
                                "tlacidlo": tlacidlo,
                                "tlacidlo_alt": tlacidlo_alt,
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
                    Ok(v) => {
                        // TLI 57: Zber dát (TLI 4c) predtým BLOKOVAL radu —
                        // keď bolo zapnuté, appka ukázala len "uložené" a
                        // rada sa nevykreslila vôbec (hráč to vnímal, akoby
                        // appka vôbec nerozpoznávala, čo má robiť). Server
                        // ULOŽIL stav vždy, bez ohľadu na istotu solvera
                        // (nezmenené od TLI 4b) — appka preto odteraz robí
                        // OBOJE: počíta uložené snímky A ZÁROVEŇ vykreslí
                        // radu solvera z tej istej odpovede.
                        let zber_pocet = if nastavenia.zber_dat_zapnute {
                            let stav = app.state::<Stav>();
                            let mut p = stav.pocitadlo_zber.lock().unwrap();
                            *p += 1;
                            Some(*p)
                        } else {
                            None
                        };

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
                                "zber_pocet": zber_pocet,
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
            over_prezentaciu(&app, &obdlznik, &snimok.rgb, zdroj);
            naplanuj_skrytie_podla_nastavenia(app.clone(), znacka, &nastavenia);
        }

        // TLI 41: požiadavka zdržaná počas tohto behu (pozri `cakajuci_dalsi`
        // vyššie) sa spustí TERAZ, hneď po uvoľnení zámku — appka teda nikdy
        // nehlási chybu na rýchle opakované stlačenie, len dorieši poslednú
        // snímku o pár sekúnd neskôr.
        let dalsi = app.state::<Stav>().cakajuci_dalsi.lock().unwrap().take();
        if let Some((dalsi_modul, dalsi_zdroj)) = dalsi {
            denik::zapis(
                "hotkey",
                &format!("spúšťam zdržanú požiadavku ({dalsi_modul:?}, zdroj={dalsi_zdroj})"),
            );
            spusti_analyzu(app.clone(), dalsi_modul, dalsi_zdroj);
        }
    });
}

/// Mimo Windowsu appka nevie odfotiť okno hry — kompiluje sa len preto, aby sa
/// dali spúšťať testy a `cargo check` na Linuxe v CI.
#[cfg(not(windows))]
pub(crate) fn spusti_analyzu(app: AppHandle, _modul: Modul, _zdroj: &'static str) {
    ukaz_chybu(&app, None, "Appka funguje len na Windowse.");
}

/// Oneskorenie pred „Odfotiť teraz" z tray menu/hlavného okna (TLI 28, bod 3)
/// — nech hráč stihne kliknúť späť do hry predtým, než appka odfotí.
const ODFOTENIE_TERAZ_ONESKORENIE_S: u64 = 2;

/// Spustí analýzu bez klávesy, s oneskorením (TLI 28, bod 3: „funguje bez
/// klávesy" — poistka pre prípad, že RegisterHotKey zlyhá, obsadí ju iný
/// program, alebo appka ešte nemá vyriešený spôsob, ako klávesu zistiť).
/// Rovnaký vzor ako `naplanuj_skrytie` — appka už takto plánuje prácu na
/// pozadí z handleru, ktorý sám nesmie čakať (tray menu, príkaz z JS).
fn naplanuj_odfotenie(app: AppHandle, modul: Modul) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(ODFOTENIE_TERAZ_ONESKORENIE_S));
        spusti_analyzu(app, modul, "rucne");
    });
}

#[tauri::command]
fn odfot_tartarus_teraz(app: AppHandle) -> Result<(), String> {
    naplanuj_odfotenie(app, Modul::Tartarus);
    Ok(())
}

#[tauri::command]
fn odfot_canvas_teraz(app: AppHandle) -> Result<(), String> {
    naplanuj_odfotenie(app, Modul::Canvas);
    Ok(())
}

/// TLI 33 (`2026-10-18-1318-mcp.md`, bod 2) — „Test overlaya“ z tray menu.
/// Richard nahlásil, že po F8 appka doreší analýzu a log ukáže „emitujem
/// stav=hotovo“, ale nad hrou sa nezobrazí ÚPLNE NIČ — bez servera (appka ho
/// aj tak vôbec nevolá) sa to doteraz nedalo overiť nezávisle od toho, či je
/// hra zapnutá a v správnej fáze. Táto funkcia prejde PRESNE tou istou cestou
/// ako reálny F8/F9 (`ukaz_overlay_nad` s `s_rozlozenim=true`, teda aj
/// `posli_polohu_panela`, + `app.emit("stav", faza=hotovo)`), len s
/// vymyslenými dátami namiesto volania servera:
///   - keď sa po kliknutí nezobrazí nič, chyba je v OKNE/EVENTE (Rust ↔ JS),
///     rovnako ako pri reálnom F8 — a dá sa to zopakovať kedykoľvek, bez hry;
///   - keď sa ukáže vzorová rada, ale pri skutočnom F8/F9 nie, chyba je v
///     niečom, čo posiela/robí appka LEN v tej reálnej vetve (napr. v tvare
///     dát zo servera pre konkrétnu obrazovku).
/// Funguje aj bez bežiacej hry (spadne na primárny monitor, rovnaký vzor ako
/// `okna::okno_pre_rezim_upravy`), nech appka test nerefúzne len preto, že
/// Richard chce rýchlo overiť renderovanie bez toho, aby musel spúšťať hru.
fn test_overlay(app: &AppHandle) {
    let Some(obd) = okno_hry_pre_rozlozenie(app).or_else(|| primarny_monitor_ako_okno(app)) else {
        denik::zapis("test-overlay", "ani hra, ani primárny monitor sa nenašli — nemám kam kresliť");
        return;
    };
    denik::zapis(
        "test-overlay",
        &format!("spúšťam nad ({},{}) {}x{}", obd.x, obd.y, obd.sirka, obd.vyska),
    );
    ukaz_overlay_nad(app, &obd, true, PANEL_TARTARUS);

    let mierka = mierka_okna(app, &obd);
    let (sirka_log, vyska_log) = suradnice::logicka_velkost(&obd, mierka);
    let obdlzniky = json!([
        {"index": 5, "x": sirka_log * 0.30, "y": vyska_log * 0.20,
         "w": vyska_log * 0.10, "h": vyska_log * 0.10,
         "stredX": sirka_log * 0.35, "stredY": vyska_log * 0.25, "neiste": false},
        {"index": 8, "x": sirka_log * 0.55, "y": vyska_log * 0.20,
         "w": vyska_log * 0.10, "h": vyska_log * 0.10,
         "stredX": sirka_log * 0.60, "stredY": vyska_log * 0.25, "neiste": true},
    ]);
    let tlacidlo = json!({
        "x": sirka_log * 0.30, "y": vyska_log * 0.75,
        "w": sirka_log * 0.15, "h": vyska_log * 0.12,
        "text": "Test tlačidla",
    });
    let analyza = json!({
        "obrazovka": "doska",
        "istota": 0.77,
        "skontroluj": [],
        "odporucanie": {
            "akcia": "devour", "nazov": "TEST — Požretie", "ciel": 5, "cez_scratch": false,
            "nastroj": null, "dar": null,
            "dovod": "Toto je vzorová rada z tray menu „Test overlaya“ — žiadny server sa nevolal.",
            "hodnota": 10.0,
        },
        "alternativy": [],
        "board": {"cells": []},
    });
    let _ = app.emit(
        "stav",
        json!({
            "faza": "hotovo",
            "analyza": analyza,
            "obdlzniky": obdlzniky,
            "tlacidlo": tlacidlo,
            "tlacidlo_alt": null,
            "mierka": mierka,
            "sirka": sirka_log,
            "vyska": vyska_log,
            "trvanie_ms": 0,
        }),
    );
    // TLI 58: aj test z tray menu sa správa podľa nastavenia — má ukázať to,
    // čo hráč naozaj dostane pri F8, nie vlastný desaťsekundový režim.
    let nastavenia = app.state::<Stav>().nastavenia.lock().unwrap().clone();
    denik::zapis(
        "test-overlay",
        if nastavenia.skryt_radu_klikom {
            "stav=hotovo (testovací) odoslaný, zhasne po kliku myšou".to_string()
        } else {
            format!("stav=hotovo (testovací) odoslaný, mizne o {} s", nastavenia.autoskrytie_s)
        }
        .as_str(),
    );

    let znacka = *app.state::<Stav>().zobrazene_od.lock().unwrap();
    if let Some(znacka) = znacka {
        naplanuj_skrytie_podla_nastavenia(app.clone(), znacka, &nastavenia);
    }
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
        return Ok(tracker::prehlad_pre_ui(&t, false, n.tracker_dan, &n.batoh_skryte_polozky));
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

    Ok(tracker::prehlad_pre_ui(&t, true, n.tracker_dan, &n.batoh_skryte_polozky))
}

/// Plný zoznam cien pre pohľad „Economy" (TLI 48 bod 3) — na rozdiel od
/// `tracker_prehlad` nečíta log nanovo (to robí vlákno na pozadí,
/// `tracker::spusti_vlakno`), len vráti aktuálny stav cenníka a jeho históriu
/// z lokálnej DB.
#[tauri::command]
fn tracker_economy(tracker: tauri::State<tracker::Tracker>) -> Vec<tracker::RiadokEconomy> {
    let t = tracker.0.lock().unwrap();
    tracker::economy_prehlad(&t, tracker::unixovy_cas())
}

/// Export histórie máp do CSV. Vracia obsah súboru — uloženie rieši UI.
#[tauri::command]
fn tracker_csv(
    stav: tauri::State<Stav>,
    tracker: tauri::State<tracker::Tracker>,
) -> Result<String, String> {
    let n = stav.nastavenia.lock().unwrap().clone();
    let t = tracker.0.lock().unwrap();
    let prehlad = tracker::prehlad_pre_ui(&t, n.tracker_zapnuty, n.tracker_dan, &n.batoh_skryte_polozky);
    Ok(tracker::csv(&prehlad.behy))
}

/// „Reset Stats" z dashboardu (TLI 45) — vymaže len živé behy tejto relácie
/// v pamäti, nič na disku. Potvrdenie rieši UI (`window.confirm`), appka tu
/// už žiadnu ďalšiu otázku nekladie.
#[tauri::command]
fn tracker_reset_zive_staty(tracker: tauri::State<tracker::Tracker>) -> Result<(), String> {
    tracker.0.lock().unwrap().resetuj_zive_staty();
    Ok(())
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
        tracker::prehlad_pre_ui(&t, n.tracker_zapnuty, n.tracker_dan, &n.batoh_skryte_polozky).behy
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
            // TLI 60 — automatické učenie Frozen Canvasu na pozadí.
            "ucenie_canvas_zapnute" => n.ucenie_canvas_zapnute = zapnuty,
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

/// CURRENT INVENTORY (TLI 47) — „Hide" v dashboarde pre jednu položku.
/// Appka nič nemaže ani neposiela, len si pre `config_base_id` zapamätá, že
/// nemá vstupovať do zoznamu na predaj ani do net worth (pravidlo appky
/// „Nič nemaž" z úlohy `2026-10-19-0744-mcp.md`).
#[tauri::command]
fn tracker_prepni_skrytu_polozku(stav: tauri::State<Stav>, config_base_id: u32, skryta: bool) -> Result<(), String> {
    let mut n = stav.nastavenia.lock().unwrap();
    if skryta {
        if !n.batoh_skryte_polozky.contains(&config_base_id) {
            n.batoh_skryte_polozky.push(config_base_id);
        }
    } else {
        n.batoh_skryte_polozky.retain(|&p| p != config_base_id);
    }
    let kopia = n.clone();
    drop(n);
    stav::uloz(&kopia)
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

    // TLI 37 — toast bez textu sa nezobrazuje vôbec. Prázdny rám uprostred
    // hore nehovorí hráčovi nič a vyzerá ako zaseknutá appka (hlásil Richard
    // 18.10.2026). Okno sa pritom ani nebudí — `show()` nižšie sa nevykoná.
    if text.trim().is_empty() {
        denik::zapis("zobraz", "pripomienka bez textu — nezobrazujem");
        return;
    }

    *app.state::<Stav>().pripomienka_obsah.lock().unwrap() =
        Some((text.to_string(), trieda.to_string()));
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
    vynut_navrch(&okno);
    // TLI 44: `pripomienka` je `focus: false` rovnako ako `overlay` — ten istý
    // WebView2-occlusion problém (TLI 40) sa ho teda vie týkať rovnako, keby
    // appka niekedy posielala obsah toastu s odstupom po `show()`. Pulz je
    // lacný a tu sa už obsah aj tak posiela PRED týmto `show()` (vyššie), nie
    // po ňom — ide o rovnakú poistku, nie o opravu konkrétne nahláseného bugu.
    aktivuj_a_vrat_fokus(&okno);
    denik::zapis("zobraz", &format!("pripomienka trieda={trieda}, text={text}"));

    let znacka = Instant::now();
    *app.state::<Stav>().pripomienka_zobrazena_od.lock().unwrap() = Some(znacka);
    naplanuj_skrytie_pripomienky(app.clone(), znacka, sekund);
}

/// TLI 57: keď podmienka opakovaného toastu (sort) prestane platiť, toast má
/// zmiznúť HNEĎ (prvý `InitBagData`), nie až po svojom vlastnom časovači —
/// predtým mohol „Sortni inventár v hre" visieť ešte desiatky sekúnd po
/// tom, čo si hráč batoh reálne sortol. Hýbe sa len vtedy, keď je PRÁVE
/// zobrazený TENTO konkrétny text — iný, novší toast (napr. upload) sa
/// nedotkne.
pub(crate) fn skry_pripomienku_ak_text(app: &AppHandle, text: &str) {
    let obsah = app.state::<Stav>().pripomienka_obsah.lock().unwrap().clone();
    if obsah.map(|(t, _)| t).as_deref() != Some(text) {
        return;
    }
    *app.state::<Stav>().pripomienka_obsah.lock().unwrap() = None;
    *app.state::<Stav>().pripomienka_zobrazena_od.lock().unwrap() = None;
    if let Some(okno) = app.get_webview_window(PRIPOMIENKA) {
        let _ = okno.hide();
    }
}

/// Okno hry pre umiestnenie panelov nad ňou (TLI 21: mini-panel, toast,
/// a „seed" pri vstupe do režimu úpravy) — nájde ho PODĽA NASTAVENÉHO NÁZVU
/// PROCESU, nič iné.
///
/// **TLI 26, bod 2 — zmena oproti 0.18.2:** do tejto verzie mal pád aj na
/// `okno::popredie()` (aktuálne aktívne okno) — v praxi to znamenalo, že keď
/// hra nebežala, overlay/mini/pripomienka sa postavili nad ČOKOĽVEK, čo malo
/// focus (prehliadač, editor), teda pokojne na úplne inom monitore, než kde
/// hráč čaká. `None` tu teraz úprimne znamená „hra sa nenašla" a volajúci
/// (`zapni_rezim_upravy`) si poradí PRIMÁRNYM monitorom — nie náhodným oknom.
/// `nastav_mini`/`ukaz_pripomienku_toast` na `None` reagujú rovnako rozumne
/// už dnes (nechajú polohu tak / padnú na primárny monitor), len doteraz bol
/// ten kód prakticky nedosiahnuteľný, lebo `okno::popredie()` skoro vždy
/// niečo vrátil.
#[cfg(windows)]
fn okno_hry_pre_rozlozenie(app: &AppHandle) -> Option<OknoFyz> {
    let proces_hry = app.state::<Stav>().nastavenia.lock().unwrap().proces_hry.clone();
    okno::najdi_hru(&proces_hry).map(|o| o.obdlznik)
}

#[cfg(not(windows))]
fn okno_hry_pre_rozlozenie(_app: &AppHandle) -> Option<OknoFyz> {
    None
}

/// TLI 48 bod 2 — výška horného pásika mini-panela v LOGICKÝCH pixeloch.
/// MUSÍ sedieť s `#mini-uchyt-presun { height: … }` v `ui/mini.css` — appka
/// touto hodnotou (prenásobenou `scale_factor()`) rozhoduje, kde presne nad
/// panelom má prestať byť klikateľná cez hru a začať brať myš pre seba.
const MINI_PASIK_VYSKA_LOGICKA: f64 = 18.0;

/// Jediný spôsob, ako na Windows urobiť JEDNU časť okna (horný pásik)
/// klikateľnú a zvyšok priechodný pre myš (click-through) NARAZ: Tauri aj OS
/// pod ním poznajú `ignore_cursor_events` len pre CELÉ okno, nie pre
/// podoblasť. Appka preto na pozadí sleduje skutočnú polohu kurzora
/// (`GetCursorPos`, obrazovkové/fyzické súradnice — rovnaký súradnicový
/// priestor ako `outer_position()`) a porovnáva ju s aktuálnym obdĺžnikom
/// pásika; len keď je nad ním, prepne okno na klikateľné, inak ho nechá
/// priechodné pre myš (klik ide ďalej do hry). V režime úpravy (Ctrl+F8) je
/// `mini` beztak celé klikateľné (`zapni_rezim_upravy`/`ukonci_rezim_upravy`)
/// — vlákno sa doňho vtedy nemieša, aby si navzájom neprepisovali
/// `ignore_cursor_events`.
///
/// 40 ms (25×/s) je dosť rýchlo, aby prechod cez hranicu pásika nebolo vidieť
/// ako záseknutie kurzora, a dosť lacné (jedno porovnanie čísel + jedno
/// `GetCursorPos`) na to, aby bežalo nepretržite na pozadí.
#[cfg(windows)]
fn spusti_mini_presun_vlakno(app: AppHandle) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    std::thread::spawn(move || {
        let mut klikatelne = false;
        loop {
            std::thread::sleep(Duration::from_millis(40));

            if *app.state::<Stav>().rezim_upravy.lock().unwrap() {
                continue;
            }
            let Some(okno) = app.get_webview_window(MINI) else { continue };
            let Ok(viditelne) = okno.is_visible() else { continue };
            if !viditelne {
                if klikatelne {
                    let _ = okno.set_ignore_cursor_events(true);
                    klikatelne = false;
                }
                continue;
            }
            let (Ok(pos), Ok(size), Ok(mierka)) =
                (okno.outer_position(), okno.outer_size(), okno.scale_factor())
            else {
                continue;
            };
            let vyska_pasika = (MINI_PASIK_VYSKA_LOGICKA * mierka).round() as i32;

            let mut bod = POINT::default();
            if unsafe { GetCursorPos(&mut bod) }.is_err() {
                continue;
            }
            let nad_pasikom = bod.x >= pos.x
                && bod.x < pos.x + size.width as i32
                && bod.y >= pos.y
                && bod.y < pos.y + vyska_pasika;

            if nad_pasikom != klikatelne {
                let _ = okno.set_ignore_cursor_events(!nad_pasikom);
                klikatelne = nad_pasikom;
            }
        }
    });
}

#[cfg(not(windows))]
fn spusti_mini_presun_vlakno(_app: AppHandle) {}

/// Celý primárny monitor vyjadrený ako `OknoFyz` — fallback pre „Upraviť
/// rozloženie" (TLI 26, bod 2), keď sa okno hry nenašlo. Rovnaký zdroj
/// pravdy („primárny monitor od OS"), aký appka už používa v `ukaz_chybu`
/// a `ukaz_pripomienku_toast`, len vrátený ako obdĺžnik namiesto súradníc
/// jedného rohu.
fn primarny_monitor_ako_okno(app: &AppHandle) -> Option<OknoFyz> {
    let m = app.get_webview_window(OVERLAY)?.primary_monitor().ok().flatten()?;
    Some(OknoFyz {
        x: m.position().x,
        y: m.position().y,
        sirka: m.size().width,
        vyska: m.size().height,
    })
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

/// Posledný obsah toastu pripomienky (TLI 37). Okno si ho vypýta po načítaní,
/// keby udalosť `pripomienka-obsah` prišla skôr, než sa stihlo prihlásiť —
/// bez toho by ostal visieť prázdny rám. `None` znamená „nič sa nehlásilo".
#[tauri::command]
fn posledna_pripomienka(app: AppHandle) -> Option<serde_json::Value> {
    app.state::<Stav>()
        .pripomienka_obsah
        .lock()
        .unwrap()
        .clone()
        .map(|(text, trieda)| json!({ "text": text, "trieda": trieda }))
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
            // TLI 26, bod 1: `.show()` NEMENÍ z-poradie (tao na Windowse
            // volá `SetWindowPos` s `SWP_NOZORDER` naschvál, aby sa pri
            // bežnom pohybe okna nič neprestavovalo) — keď bolo toto okno
            // (typicky mini-panel, keď ho má hráč zapnutý natrvalo) viditeľné
            // UŽ PRED vstupom do režimu úpravy, zostalo by pod čerstvo
            // zobrazeným `overlay`, ktorý práve dostal `ignore_cursor_events
            // (false)` a prekryl by ho na klik. `.set_focus()` ho explicitne
            // vynesie navrch — presne preto sa v 0.18.2 dal ťahať len ľavý
            // panel (vnútri `overlay`), nie mini-panel ani pripomienka.
            let _ = okno.set_focus();
            let _ = app.emit_to(label, "rezim-upravy", json!({"zapnute": true, "priehladnost": priehladnost}));
        }
    }
}

/// Vstup do režimu úpravy (Ctrl+F8 z prázdneho stavu, alebo tlačidlo
/// v Nastaveniach). Bez nájdeného okna hry sa nedá vypočítať zlomok, voči
/// ktorému sa polohy ukladajú — appka to poctivo povie namiesto tichého
/// zlyhania.
fn zapni_rezim_upravy(app: &AppHandle) {
    // TLI 26, bod 2: hra nebeží → primárny monitor, NIE naposledy aktívne
    // okno. Rozhodovanie je v čistej funkcii `okna::okno_pre_rezim_upravy`
    // (testovateľná bez Windows), tu sa len zoženú jej dva vstupy.
    let okno_hry_najdena = okno_hry_pre_rozlozenie(app);
    denik::zapis(
        "monitor",
        if okno_hry_najdena.is_some() {
            "Upraviť rozloženie: okno hry nájdené"
        } else {
            "Upraviť rozloženie: okno hry sa nenašlo, skúšam primárny monitor"
        },
    );
    let Some(okno_hry) = okna::okno_pre_rezim_upravy(okno_hry_najdena, primarny_monitor_ako_okno(app))
    else {
        denik::zapis("monitor", "Upraviť rozloženie: ani primárny monitor sa nenašiel");
        ukaz_chybu(
            app,
            None,
            "Upraviť rozloženie sa nedá zobraziť — appka nenašla ani okno hry, ani primárny monitor.",
        );
        return;
    };
    let mierka = mierka_okna(app, &okno_hry);
    let sig = monitor_signatura_app(app, &okno_hry);
    denik::zapis("monitor", &format!("Upraviť rozloženie beží na monitore „{sig}“"));

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

/// TLI 48 bod 2 — uloženie novej polohy mini-panela PO presune mimo režimu
/// úpravy (pásik je odteraz ťahateľný aj bez Ctrl+F8). JS zavolá tento príkaz
/// hneď po tom, čo `startDragging()` resolvne (teda hráč pustil tlačidlo).
///
/// Rovnaká matematika ako `rezim_upravy_ulozit_panel` pre mini/pripomienku,
/// len bez `RezimUpravyKontext` — ten tu nie je (žiaden režim úpravy nebeží),
/// takže sa okno hry aj monitor zisťujú ČERSTVO, rovnako ako pri `nastav_mini`.
/// Keď sa okno hry práve nenájde (hráč medzitým prepol do menu a späť),
/// appka ticho nič neuloží — poloha v pamäti okna ostáva, len sa nezapíše na
/// disk; ďalší úspešný presun to dobehne.
#[tauri::command]
fn mini_ulozit_polohu(app: AppHandle) -> Result<(), String> {
    let Some(okno_hry) = okno_hry_pre_rozlozenie(&app) else {
        return Ok(());
    };
    let Some(okno) = app.get_webview_window(MINI) else {
        return Ok(());
    };
    let (Ok(pos), Ok(size)) = (okno.outer_position(), okno.outer_size()) else {
        return Ok(());
    };
    let sig = monitor_signatura_app(&app, &okno_hry);
    let css_fyz = suradnice::ObdlznikCss {
        x: (pos.x - okno_hry.x) as f64,
        y: (pos.y - okno_hry.y) as f64,
        w: size.width as f64,
        h: size.height as f64,
    };
    let p = suradnice::css_na_box01(&css_fyz, &okno_hry, 1.0);

    let stav = app.state::<Stav>();
    let mut n = stav.nastavenia.lock().unwrap();
    n.rozlozenie.uloz_polohu(PANEL_MINI, &sig, Obdlznik01 { x: p.x, y: p.y, w: p.w, h: p.h });
    n.normalizuj();
    let kopia = n.clone();
    drop(n);
    stav::uloz(&kopia)
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

/// Upozornenia z POSLEDNÉHO pokusu zaregistrovať klávesové skratky (TLI 28,
/// bod 2a) — Nastavenia si ich vypýtajú pri OTVORENÍ okna, nie len po uložení
/// formulára, lebo appka ich vie zistiť (napr. že F8 drží Overwolf) už pri
/// vlastnom štarte, dávno predtým, než hráč Nastavenia vôbec otvorí.
#[tauri::command]
fn hotkey_upozornenia(stav: tauri::State<Stav>) -> Vec<String> {
    stav.hotkey_upozornenia.lock().unwrap().clone()
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
    // rozloženie" nenávratne pripravila až do reštartu. `posli_toast = false`
    // — formulár chybu ukáže rovno sám, druhý toast navyše by bol len hluk.
    let chyby = zaregistruj_vsetky_hotkeys(&app, &hotkey, &hotkey_canvas, &hotkey_trh, false);
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
    skry(&app, "ui");
    Ok(())
}

/// TLI 33 (`2026-10-18-1318-mcp.md`, bod 1) — denník doteraz vedel len to, čo
/// urobil RUST (okno pozícia/veľkosť/show, emitovanie `stav`), nie čo sa s tým
/// udalosťou stalo na strane JS vo webview `overlay`. Keď appka u hráča „nič
/// nezobrazí", nedalo sa z logu rozlíšiť „event nedorazil do JS" od „event
/// dorazil, ale vykreslenie zlyhalo/skrylo sa" — práve toto rozlíšenie bolo
/// chýbajúce pri `2026-10-18-1318-mcp.md`. `overlay.js` volá tento príkaz pri
/// prijatí KAŽDÉHO `stav` eventu a znova hneď po tom, čo si (ne)myslí, že
/// panel ukázal — oba zápisy idú do toho istého `denik::zapis`, teda do
/// `logs/aktualny.log`, ktorý appka posiela ako diagnostiku (rovnaký súbor
/// ako `[hotkey]`/`[zobraz]`/`[http]`).
#[tauri::command]
fn zapis_ui_log(text: String) -> Result<(), String> {
    denik::zapis("ui", &text);
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
/// Volá ju aj tlačidlo „Check for Updates" (Nastavenia aj footer „Collector"),
/// aj periodické vlákno nižšie — obe idú cez `vykonaj_kontrolu_aktualizacie`,
/// nech sa loguje a do footera zapisuje úplne rovnako bez ohľadu na to, kto
/// kontrolu spustil (TLI 52, bod 2).
#[tauri::command]
fn skontroluj_aktualizaciu(app: AppHandle) -> Result<Option<String>, String> {
    let kontrola = vykonaj_kontrolu_aktualizacie(&app);
    match kontrola.chyba {
        Some(chyba) => Err(chyba),
        None => Ok(kontrola.msi_url),
    }
}

/// Appka doteraz kontrolu aktualizácie VÔBEC nespúšťala sama — ani pri
/// štarte, ani periodicky — hráč musel kliknúť „Check for Updates" ručne,
/// inak appka o novej verzii nevedela NIKDY a jej log o tom nemal ani
/// riadok. Presne to sa stalo Richardovi 19.10.2026: bežal tri vydania
/// pozadu (0.20.1) a prišiel na to až náhodou. `spusti_aktualizacia_vlakno`
/// to opravuje kontrolou hneď pri štarte a potom á 30 minút.
const AKTUALIZACIA_INTERVAL_S: u64 = 30 * 60;

fn spusti_aktualizacia_vlakno(app: AppHandle) {
    std::thread::spawn(move || loop {
        vykonaj_kontrolu_aktualizacie(&app);
        std::thread::sleep(Duration::from_secs(AKTUALIZACIA_INTERVAL_S));
    });
}

/// TLI 52, časť 2/2 — jadro Richardovej pôvodnej žiadosti („self-update musí
/// fungovať bez UAC, potichu, aj keď appka beží celý deň"). Spustí sa VÝHRADNE
/// vtedy, keď manifest ponúka `installerUrl`/`installerSha256` (NSIS
/// `-setup.exe`) — starý manifest aj appky, ktoré tieto polia nepoznajú, sa
/// touto vetvou vôbec neprejdú a appka sa správa presne ako doteraz (odkaz na
/// MSI, ručné stiahnutie). **Toto je zámerná poistka, nie dočasný stav:** do
/// živého `latest-tli.json` sa `installerUrl` nedopĺňa, kým krok 3 zo zadania
/// (migrácia z perMachine MSI na perUser NSIS) nie je osobne overený na
/// Richardovom reálnom PC — pozri CHANGELOG k tejto verzii.
///
/// Vracia `true`, len keď sa inštalátor naozaj podarilo spustiť (appka sa v
/// tom prípade sama ukončí, `app.exit(0)` nižšie) — volajúci s návratovou
/// hodnotou ďalej nič nerobí, len to umožňuje test bez skutočného Windows
/// procesu (vracia `false`, keď polia chýbajú, ešte pred akýmkoľvek I/O).
#[cfg(windows)]
fn pokus_o_tichu_aktualizaciu(app: &AppHandle, manifest: &tli_overlay_core::aktualizacia::Manifest) -> bool {
    let (Some(url), Some(sha)) = (&manifest.installer_url, &manifest.installer_sha256) else {
        return false;
    };

    denik::zapis("update", &format!("tichá inštalácia {}: sťahujem {url}", manifest.version));

    let bajty = match klient::stiahni_installer(url) {
        Ok(b) => b,
        Err(chyba) => {
            denik::zapis("update", &format!("tichá inštalácia zlyhala (stiahnutie): {chyba}"));
            return false;
        }
    };

    if !tli_overlay_core::aktualizacia::over_sha256(&bajty, sha) {
        denik::zapis(
            "update",
            "tichá inštalácia PRESKOČENÁ: SHA256 stiahnutého inštalátora nesedí s manifestom",
        );
        return false;
    }

    let cesta = std::env::temp_dir().join(format!("tli-overlay-update-{}.exe", manifest.version));
    if let Err(chyba) = std::fs::write(&cesta, &bajty) {
        denik::zapis("update", &format!("tichá inštalácia zlyhala (zápis na disk): {chyba}"));
        return false;
    }

    // Presne tie isté argumenty, aké posiela oficiálny `tauri-plugin-updater`
    // pri `WindowsUpdateInstallMode::Quiet` — overené priamo v nainštalovanej
    // verzii `tauri-bundler` 2.12.3 (`installer.nsi` zo šablóny) a v zdroji
    // pluginu (`crates/.../updater.rs::updater_parameters`), nie z pamäte:
    // `/S` = úplne ticho bez okien, `/UPDATE` = nemaž nastavenia hráča,
    // `/R /ARGS ""` = appku po dokončení znova spusti. perUser (`installMode:
    // currentUser`) inštalátor pritom NEŽIADA UAC vôbec — jediná výnimka je
    // JEDNORAZOVÁ migrácia zo starej perMachine MSI, ktorú rieši VSTAVANÁ
    // logika šablóny NSIS (porovnáva `DisplayName`+`Publisher` v
    // `HKLM\...\Uninstall` s našimi a spustí `msiexec /X{GUID}` zo starej
    // `UninstallString`) — nie vlastný kód tejto appky. Tá jedna UAC výzva je
    // presne to, čo zadanie výslovne akceptovalo ako OK (CHANGELOG, bod 3).
    match std::process::Command::new(&cesta)
        .args(["/S", "/UPDATE", "/R", "/ARGS", ""])
        .spawn()
    {
        Ok(_) => {
            denik::zapis(
                "update",
                &format!("tichá inštalácia {} spustená, appka sa ukončuje", manifest.version),
            );
            app.exit(0);
            true
        }
        Err(chyba) => {
            denik::zapis("update", &format!("tichá inštalácia zlyhala (spustenie inštalátora): {chyba}"));
            false
        }
    }
}

#[cfg(not(windows))]
fn pokus_o_tichu_aktualizaciu(_app: &AppHandle, _manifest: &tli_overlay_core::aktualizacia::Manifest) -> bool {
    false
}

/// Jedno miesto, kadiaľ prechádza KAŽDÁ kontrola aktualizácie (periodická aj
/// ručná) — zapíše riadok `[update]` do `tli-overlay.log` s nájdenou verziou
/// alebo chybou, uloží výsledok do `Stav` (aby ho footer okna `hlavne` vedel
/// zobraziť, aj keď sa okno otvorí až PO kontrole) a pošle udalosť
/// `aktualizacia-kontrola` pre okná, ktoré sú práve otvorené.
fn vykonaj_kontrolu_aktualizacie(app: &AppHandle) -> KontrolaAktualizacie {
    let cas = crate::tracker::unixovy_cas();

    let vysledok = klient::stiahni_manifest().and_then(|json| {
        tli_overlay_core::aktualizacia::skontroluj_manifest(&json, env!("CARGO_PKG_VERSION"))
    });

    let kontrola = match vysledok {
        Ok(Some(manifest)) => {
            denik::zapis(
                "update",
                &format!(
                    "kontrola: nájdená verzia {} (aktuálna {})",
                    manifest.version,
                    env!("CARGO_PKG_VERSION")
                ),
            );
            pokus_o_tichu_aktualizaciu(app, &manifest);
            KontrolaAktualizacie {
                cas,
                najdena_verzia: Some(manifest.version),
                msi_url: Some(manifest.msi_url),
                chyba: None,
            }
        }
        Ok(None) => {
            denik::zapis(
                "update",
                &format!("kontrola: beží najnovšia verzia ({})", env!("CARGO_PKG_VERSION")),
            );
            KontrolaAktualizacie { cas, najdena_verzia: None, msi_url: None, chyba: None }
        }
        Err(chyba) => {
            denik::zapis("update", &format!("kontrola zlyhala: {chyba}"));
            KontrolaAktualizacie { cas, najdena_verzia: None, msi_url: None, chyba: Some(chyba) }
        }
    };

    *app.state::<Stav>().posledna_kontrola_aktualizacie.lock().unwrap() = Some(kontrola.clone());
    let _ = app.emit("aktualizacia-kontrola", &kontrola);
    kontrola
}

/// Footer okna `hlavne` si toto vypýta po načítaní (rovnaký vzor ako
/// `posledna_pripomienka`), nech vidí posledný výsledok aj vtedy, keď sa
/// okno otvorilo až po tom, čo periodické vlákno už raz skontrolovalo.
#[tauri::command]
fn posledna_kontrola_aktualizacie(app: AppHandle) -> Option<KontrolaAktualizacie> {
    app.state::<Stav>().posledna_kontrola_aktualizacie.lock().unwrap().clone()
}

// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    // TLI 26, bod 3: do tejto verzie `denik::zapis` volal VÝHRADNE
    // `spusti_analyzu` (teda až po prvom stlačení F8/F9) — kým appka bežala
    // a hráč ju nestihol/nemusel použiť, priečinok `logs` sa vôbec
    // nevytvoril, hoci appka bola spustená celé hodiny. Tento riadok ide
    // úplne PRVÝ, pred čítaním nastavení aj pred Tauri builderom, nech log
    // existuje od prvej milisekundy behu appky.
    denik::zapis("start", &format!("tli-overlay {} spustené", env!("CARGO_PKG_VERSION")));

    let nacitane = stav::nacitaj();
    let hotkey_pri_starte = nacitane.hotkey.clone();
    let hotkey_canvas_pri_starte = nacitane.hotkey_canvas.clone();
    let hotkey_trh_pri_starte = nacitane.hotkey_trh.clone();

    let skratky = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, skratka, udalost| {
            if udalost.state() != ShortcutState::Pressed {
                return;
            }

            // TLI 44, bod 3: `RegisterHotKey` na Windowse vie tú istú
            // skratku doručiť viackrát za sebou (key-repeat), kým je
            // kombinácia držaná — Richard 18.10.2026 videl v logu dve
            // „Tartarus stlačené" v tej istej sekunde na jeden stlak.
            // Druhé (a ďalšie) doručenie TEJ ISTEJ skratky do 500 ms od
            // predošlej sa tu zahodí úplne potichu, pred akýmkoľvek ďalším
            // rozhodovaním (vrátane Ctrl+F8/Esc/Enter nižšie) — nejde o
            // druhé stlačenie, len o echo toho istého.
            {
                let stav = app.state::<Stav>();
                let mut posledny = stav.posledny_hotkey.lock().unwrap();
                let teraz = Instant::now();
                if hotkey_debounovany(*posledny, skratka, teraz) {
                    denik::zapis("hotkey", "ignorované — duplicitné doručenie (debounce 500 ms)");
                    return;
                }
                *posledny = Some((*skratka, teraz));
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
                // Hotkey aj Esc robia to isté: zhasnú overlay. Dôvod sa v
                // denníku rozlišuje (TLI 58) — „esc" je podľa zadania jedna
                // z poistiek, stlačenie hotkey je vedomé zhasnutie hráčom.
                skry(app, if skratka == &esc() { "esc" } else { "hotkey" });
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
                spusti_analyzu(app.clone(), modul, "hotkey");
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
            cakajuci_dalsi: Mutex::new(None),
            zobrazene_od: Mutex::new(None),
            pocitadlo_zber: Mutex::new(0),
            poslane_behy: Mutex::new(std::collections::BTreeSet::new()),
            pripomienka_zobrazena_od: Mutex::new(None),
            pripomienka_obsah: Mutex::new(None),
            rezim_upravy: Mutex::new(false),
            rezim_upravy_kontext: Mutex::new(None),
            hotkey_upozornenia: Mutex::new(Vec::new()),
            posledny_hotkey: Mutex::new(None),
            posledna_kontrola_aktualizacie: Mutex::new(None),
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
            posledna_kontrola_aktualizacie,
            tracker_prehlad,
            tracker_economy,
            tracker_csv,
            tracker_reset_zive_staty,
            tracker_prepni_skrytu_polozku,
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
            posledna_pripomienka,
            rezim_upravy_zapni,
            rezim_upravy_hotovo,
            rezim_upravy_prepni_panel,
            rezim_upravy_je_aktivny,
            rezim_upravy_obnov_predvolene,
            rezim_upravy_ulozit_panel,
            mini_ulozit_polohu,
            uloz_priehladnost,
            otvor_priecinok_logov,
            hotkey_upozornenia,
            odfot_tartarus_teraz,
            odfot_canvas_teraz,
            zapis_ui_log
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
            // `posli_toast = true` — pri štarte appky hráč inak upozornenie
            // neuvidí vôbec (Nastavenia ešte nemusí mať otvorené). TLI 28, bod 2a.
            //
            // Poznámka k poradiu (TLI 28, bod 4 — regresný test na TLI 23):
            // `setup()` beží AŽ PO tom, čo Tauri vytvorilo všetky okná z
            // `tauri.conf.json` (dôkaz je pár riadkov nižšie — `app.get_webview_
            // window(OVERLAY)` tu už vracia `Some`, rovnako ako predtým), takže
            // `zaregistruj_vsetky_hotkeys` (cezeň prípadný toast cez okno
            // `pripomienka`) má nad čím pracovať. Samotné `RegisterHotKey` navyše
            // nepotrebuje bežiaci event loop — len existujúce okno (vytvorené už
            // pri inicializácii pluginu `tauri_plugin_global_shortcut`, teda PRED
            // týmto `setup()`); hlavná slučka tao/winit, ktorá mu po `run()`
            // začne doručovať `WM_HOTKEY`, sa rozbehne hneď po návrate z tohto
            // uzáveru. TLI 23 (zatváranie okien na `hide` namiesto zničenia)
            // tento vzťah nemenilo — žiadna regresia tu nie je.
            let _ = zaregistruj_vsetky_hotkeys(
                &handle,
                &hotkey_pri_starte,
                &hotkey_canvas_pri_starte,
                &hotkey_trh_pri_starte,
                true,
            );

            // Tracker (TLI 11c): číta log na pozadí nezávisle od toho, či je
            // niektoré okno otvorené, aby appka fungovala aj schovaná v tray.
            tracker::spusti_vlakno(handle.clone());

            // TLI 48 bod 2: horný pásik mini-panela je ťahateľný aj mimo
            // režimu úpravy — vlákno nižšie je na to potrebné (bod 2 nižšie
            // pri definícii funkcie vysvetľuje prečo).
            spusti_mini_presun_vlakno(handle.clone());

            // TLI 52 bod 2: kontrola aktualizácie hneď pri štarte a potom
            // á 30 min — appka predtým kontrolu nespúšťala sama vôbec.
            spusti_aktualizacia_vlakno(handle.clone());

            // TLI 60: automatické učenie Frozen Canvasu na pozadí — klik myšou
            // nad otvoreným Canvasom odfotí okno hry a snímka ide frontou na
            // server. Vlákna bežia vždy; samotný prepínač sa číta pri každom
            // kliku, takže zapnutie/vypnutie v Nastaveniach nepotrebuje reštart.
            zber_canvas::spusti(handle.clone());

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
            //
            // TLI 33 (`2026-10-18-1318-mcp.md`, bod 3): predtým tu bolo len
            // `okno.show()` — pri zapnutom mini-overlayi (uložené z MINULÉHO
            // behu appky) sa okno ukázalo na mieste, kde ho necháva Tauri bez
            // vlastného `set_position()` (defaultné umiestnenie OS pre nové
            // okno, pozorované Richardom cca na (182,182), prekrývajúce text
            // hry) — NIE na uloženej polohe. `nastav_mini()` (volaná doteraz
            // len z prepínača v Nastaveniach) polohu vždy prepočíta podľa
            // `poloha_alebo_predvolena(PANEL_MINI, …)` pred `.show()`; tu sa
            // len volá namiesto ručného show/hide, aby pri štarte s už
            // zapnutým mini-overlayom skočilo rovno na uloženú polohu.
            if let Some(okno) = app.get_webview_window(MINI) {
                let _ = okno.set_ignore_cursor_events(true);
            }
            let mini_zapnuty = app.state::<Stav>().nastavenia.lock().unwrap().mini_overlay;
            nastav_mini(&handle, mini_zapnuty);

            let polozka_hlavne =
                tauri::menu::MenuItem::with_id(app, "hlavne", "Otvoriť", true, None::<&str>)?;
            let polozka_nastavenia =
                tauri::menu::MenuItem::with_id(app, "nastavenia", "Nastavenia", true, None::<&str>)?;
            // TLI 28, bod 3: funguje bez klávesy, aj keď RegisterHotKey zlyhá
            // alebo F8/F9 drží iný program.
            let polozka_odfot_tartarus = tauri::menu::MenuItem::with_id(
                app,
                "odfot_tartarus",
                "Odfotiť Tartarus teraz",
                true,
                None::<&str>,
            )?;
            let polozka_odfot_canvas = tauri::menu::MenuItem::with_id(
                app,
                "odfot_canvas",
                "Odfotiť Canvas teraz",
                true,
                None::<&str>,
            )?;
            // TLI 33 (`2026-10-18-1318-mcp.md`, bod 2): vzorová rada bez
            // servera, nech sa dá overiť, či appka nad hrou vôbec KRESLÍ,
            // bez ohľadu na to, čo robí hra alebo server.
            let polozka_test_overlay = tauri::menu::MenuItem::with_id(
                app,
                "test_overlay",
                "Test overlaya",
                true,
                None::<&str>,
            )?;
            // TLI 39, bod 3 (`2026-10-18-1613-mcp.md`): rovnaký dôvod ako
            // toast pri štarte — hráč nemá kde inde skontrolovať, ktorá
            // klávesa patrí ktorému modulu, bez otvárania Nastavení. Položka
            // je len na čítanie (`false` = disabled), rovnako ako „Verzia".
            // Ukazuje hodnoty PLATNÉ OD ŠTARTU appky — zmena v Nastaveniach
            // bez reštartu sa sem nedotiahne (text menu sa po zostavení
            // nemení), čerstvý stav má vždy samotné Nastavenia.
            let polozka_klavesy = tauri::menu::MenuItem::with_id(
                app,
                "klavesy",
                format!(
                    "Klávesy: {}",
                    text_prehladu_klaves(&hotkey_pri_starte, &hotkey_canvas_pri_starte, &hotkey_trh_pri_starte)
                ),
                false,
                None::<&str>,
            )?;
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
                &[
                    &polozka_hlavne,
                    &polozka_nastavenia,
                    &polozka_odfot_tartarus,
                    &polozka_odfot_canvas,
                    &polozka_test_overlay,
                    &polozka_klavesy,
                    &polozka_verzia,
                    &polozka_koniec,
                ],
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
                    "odfot_tartarus" => naplanuj_odfotenie(app.clone(), Modul::Tartarus),
                    "odfot_canvas" => naplanuj_odfotenie(app.clone(), Modul::Canvas),
                    "test_overlay" => test_overlay(app),
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

#[cfg(test)]
mod testy {
    use super::*;

    // ── konfliktne_moduly (TLI 39, bod 2) ──────────────────────────────────

    #[test]
    fn predvolene_klavesy_su_bez_konfliktu() {
        assert!(konfliktne_moduly("F8", "F9", "F10").is_empty());
    }

    #[test]
    fn canvas_rovnaka_ako_tartarus_je_konflikt_canvas() {
        let k = konfliktne_moduly("F8", "F8", "F10");
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].0, "canvas");
        assert!(k[0].1.contains("Frozen Canvas"));
        assert!(k[0].1.contains("Tartarus"));
    }

    #[test]
    fn trh_rovnaka_ako_tartarus_je_konflikt_trh() {
        let k = konfliktne_moduly("F8", "F9", "F8");
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].0, "trh");
        assert!(k[0].1.contains("ceny z trhu"));
        assert!(k[0].1.contains("Tartarus"));
    }

    #[test]
    fn trh_rovnaka_ako_canvas_je_konflikt_trh() {
        let k = konfliktne_moduly("F8", "F9", "F9");
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].0, "trh");
        assert!(k[0].1.contains("Frozen Canvas"));
    }

    /// Presne scenár z 18.10.2026 (`2026-10-18-1613-mcp.md`): keby sa niekto
    /// pokúsil dať Tartarus aj ceny z trhu na tú istú klávesu, appka to musí
    /// vedieť pomenovať — nie len nechať jednu z nich ticho prepadnúť.
    #[test]
    fn konflikt_rozozna_aj_tri_na_rovnakej_klavese() {
        let k = konfliktne_moduly("DIGIT3", "F9", "DIGIT3");
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].0, "trh");
    }

    #[test]
    fn konflikt_je_necitlivy_na_velkost_pismen() {
        let k = konfliktne_moduly("F8", "f8", "F10");
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].0, "canvas");
    }

    #[test]
    fn prazdny_hotkey_trh_sa_neberie_ako_konflikt() {
        // Po TLI 39 by `normalizuj()` prázdny hotkey_trh nikdy nepustila sem
        // (vždy doplní „F10"), ale funkcia nesmie spadnúť ani na priamy,
        // nenormalizovaný vstup.
        assert!(konfliktne_moduly("F8", "F9", "").is_empty());
    }

    #[test]
    fn tri_rozne_klavesy_su_bez_konfliktu() {
        assert!(konfliktne_moduly("CONTROL+F7", "F9", "F10").is_empty());
    }

    // ── text_prehladu_klaves (TLI 39, bod 3) ───────────────────────────────

    #[test]
    fn text_prehladu_klaves_bez_konfliktu_ukazuje_vsetky_tri() {
        let t = text_prehladu_klaves("F8", "F9", "F10");
        assert_eq!(
            t,
            "F8 Tartarus, F9 Frozen Canvas, F10 ceny z trhu, CONTROL+F8 Upraviť rozloženie"
        );
    }

    #[test]
    fn text_prehladu_klaves_ukaze_konflikt_namiesto_klavesy() {
        let t = text_prehladu_klaves("F8", "F8", "F10");
        assert!(t.contains("F8 Tartarus"));
        assert!(t.contains("(konflikt"));
        assert!(!t.contains("F8 Frozen Canvas"));
    }

    // ── hotkey_debounovany (TLI 44, bod 3) ─────────────────────────────────

    fn ctrl_q() -> Shortcut {
        Shortcut::new(Some(Modifiers::CONTROL), Code::KeyQ)
    }

    #[test]
    fn prve_doruceny_nie_je_debounovane() {
        assert!(!hotkey_debounovany(None, &ctrl_q(), Instant::now()));
    }

    /// Presne scenár z 18.10.2026: dve „Tartarus stlačené" na tú istú
    /// skratku v tej istej sekunde — druhé je echo Windowsu, nie druhý stlak.
    #[test]
    fn echo_tej_istej_skratky_do_500ms_je_debounovane() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(50);
        assert!(hotkey_debounovany(Some((ctrl_q(), t0)), &ctrl_q(), t1));
    }

    #[test]
    fn ta_ista_skratka_po_500ms_uz_nie_je_debounovana() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(500);
        assert!(!hotkey_debounovany(Some((ctrl_q(), t0)), &ctrl_q(), t1));
    }

    #[test]
    fn ina_skratka_tesne_po_predoslej_nie_je_debounovana() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(10);
        assert!(!hotkey_debounovany(Some((ctrl_q(), t0)), &ctrl_f8(), t1));
    }

    // ── vyhodnot_prezentaciu (TLI 44, bod 2) ───────────────────────────────

    #[test]
    fn podiel_nad_prahom_je_ano() {
        assert_eq!(vyhodnot_prezentaciu(Some(PREZENTACIA_PODIEL_PRAH + 0.001)), VysledokPrezentacie::Ano);
    }

    #[test]
    fn podiel_presne_na_prahu_je_nie() {
        // Práh je prísne „väčší než", nie „aspoň" — nulová/hraničná zhoda sa
        // nemá tváriť ako úspešné zobrazenie (TLI 40, bod 3).
        assert_eq!(vyhodnot_prezentaciu(Some(PREZENTACIA_PODIEL_PRAH)), VysledokPrezentacie::Nie);
    }

    #[test]
    fn nulovy_podiel_je_nie() {
        assert_eq!(vyhodnot_prezentaciu(Some(0.0)), VysledokPrezentacie::Nie);
    }

    #[test]
    fn ziadny_podiel_sa_neda_overit() {
        assert_eq!(vyhodnot_prezentaciu(None), VysledokPrezentacie::NedaSaOverit);
    }

    // ── krok_cakania_na_klik (TLI 58) ──────────────────────────────────────

    /// Pomocník: prehrá postupnosť stavov tlačidla a vráti, po koľkých
    /// krokoch sa panel skryl (`None` = nikdy).
    fn prehraj(start: FazaKliku, stavy: &[bool]) -> Option<usize> {
        let mut faza = start;
        for (i, stlacene) in stavy.iter().enumerate() {
            let (nova, skryt) = krok_cakania_na_klik(faza, *stlacene);
            faza = nova;
            if skryt {
                return Some(i);
            }
        }
        None
    }

    #[test]
    fn prvy_klik_po_zobrazeni_skryje() {
        assert_eq!(prehraj(FazaKliku::CakamNaStlacenie, &[false, false, true]), Some(2));
    }

    #[test]
    fn bez_kliku_sa_neskryje_nikdy() {
        assert_eq!(prehraj(FazaKliku::CakamNaStlacenie, &[false; 50]), None);
    }

    /// Jadro veci: overlay sa často zobrazí PRÁVE v okamihu, keď hráč drží
    /// tlačidlo (klik na „Odfotiť teraz" v tray menu). Keby sa bralo prosté
    /// „tlačidlo je dole", panel by zhasol v tej istej milisekunde, v ktorej
    /// sa objavil — preto sa najprv čaká na pustenie.
    #[test]
    fn drzane_tlacidlo_pri_zobrazeni_panel_nezhasne() {
        // dole, dole, pustené, hore, a až potom nový stlak
        assert_eq!(
            prehraj(FazaKliku::CakamNaPustenie, &[true, true, false, false, true]),
            Some(4)
        );
    }

    #[test]
    fn drzane_tlacidlo_bez_pustenia_nikdy_neskryje() {
        assert_eq!(prehraj(FazaKliku::CakamNaPustenie, &[true; 50]), None);
    }

    /// Po pustení sa fáza už nevracia — druhé „hore" nesmie automat zresetovať.
    #[test]
    fn opakovane_hore_drzi_fazu_cakania_na_stlacenie() {
        let (f1, _) = krok_cakania_na_klik(FazaKliku::CakamNaPustenie, false);
        assert_eq!(f1, FazaKliku::CakamNaStlacenie);
        let (f2, skryt) = krok_cakania_na_klik(f1, false);
        assert_eq!(f2, FazaKliku::CakamNaStlacenie);
        assert!(!skryt);
    }

    /// Poistka musí byť kratšia než „navždy" a dlhšia než bežné prečítanie
    /// rady — 120 s zo zadania TLI 58.
    #[test]
    fn poistka_je_120_sekund() {
        assert_eq!(SKRYTIE_KLIKOM_POISTKA_S, 120);
    }
}
