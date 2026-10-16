//! Práca s oknami appky (TLI 23 — hotfix „okno sa po prvom zatvorení už
//! znova neotvorí"). Vytiahnuté z `main.rs` do vlastného súboru LEN preto,
//! aby sa dalo v `tests/e2e_okna.rs` zahrnúť cez `#[path]` a otestovať bez
//! behu celej appky — rovnaký trik ako `klient.rs` v `tests/e2e_mock.rs`.
//! Preto tu zámerne NIE JE žiadne `use crate::...` na súrodenské moduly
//! (`stav`, `tracker`) — test vidí len tento jeden súbor, nie celý strom,
//! a logovanie chýb (ktoré potrebuje `stav::zapis_chybu_okna` +
//! `tracker::unixovy_cas`) preto robí až tenký obal v `main.rs`.
//!
//! Funkcie sú generické cez `R: tauri::Runtime`, nie pevne na `tauri::Wry`
//! (predvolený runtime appky) — inak by sa nedali zavolať nad
//! `tauri::test::MockRuntime` v testoch vôbec.

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

/// Je stred daného okna aspoň na jednom pripojenom monitore? Keď hráč
/// odpojí druhý monitor alebo zmení rozlíšenie, uložená pozícia okna môže
/// ostať mimo akejkoľvek viditeľnej obrazovky — vtedy sa okno nedá nájsť
/// ani potiahnuť, appka sa tvári, že sa „nič neotvorilo" (TLI 23, bod 1).
pub fn je_na_viditelnom_monitore<R: Runtime>(okno: &WebviewWindow<R>) -> bool {
    let (Ok(pozicia), Ok(velkost), Ok(monitory)) =
        (okno.outer_position(), okno.outer_size(), okno.available_monitors())
    else {
        // Keď sa to nedá zistiť, okno radšej nechaj tam, kde je, než ho
        // zbytočne preskakovať na stred.
        return true;
    };
    let stred_x = pozicia.x + velkost.width as i32 / 2;
    let stred_y = pozicia.y + velkost.height as i32 / 2;
    monitory.iter().any(|m| {
        let mp = m.position();
        let ms = m.size();
        stred_x >= mp.x
            && stred_x < mp.x + ms.width as i32
            && stred_y >= mp.y
            && stred_y < mp.y + ms.height as i32
    })
}

/// Appka okno znova POSTAVÍ podľa pôvodnej konfigurácie z `tauri.conf.json`,
/// keď `get_webview_window` vráti `None`. Po pridaní `on_window_event`
/// (`main.rs`, mení zatvorenie na skrytie) by sa to nemalo stať nikdy pri
/// bežnom používaní — je to záchranná sieť pre prípad, že by okno zaniklo
/// inak, nech appka aj vtedy nič nemlčí.
pub fn znova_postav_okno<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
) -> Result<WebviewWindow<R>, String> {
    let cfg = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == label)
        .cloned()
        .ok_or_else(|| format!("okno '{label}' nie je v tauri.conf.json"))?;
    tauri::WebviewWindowBuilder::from_config(app, &cfg)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())
}

/// Ukáže a zaostrí dané okno (nastavenia/hlavné) — spoločné pre tray menu aj
/// pre príkaz volaný z JS (tlačidlo „Otvoriť nastavenia" v záložke
/// Nastavenia hlavného okna).
///
/// TLI 23: Tauri v2 pri zatvorení (X) okno PREDVOLENE ZNIČÍ — druhý klik na
/// „Nastavenia" v tray menu potom narazil na `get_webview_window` vracajúce
/// `None` a appka ticho neurobila nič, hoci sa navonok tvárila v poriadku.
/// Hlavná oprava je `on_window_event(CloseRequested)` v `main.rs`
/// (zatvorenie sa mení na skrytie); táto funkcia je druhá vrstva: keby okno
/// napriek tomu chýbalo, postaví ho znova namiesto ticheho no-opu, a keď je
/// mimo viditeľnej obrazovky, vráti ho na stred.
///
/// Vracia zoznam `(kontext, chyba)` pre KAŽDÝ krok, ktorý zlyhal — appka ich
/// v `main.rs` zapíše do `okna.log` (bod 2 zadania „žiadne ticho zahodené
/// chyby"), test z `tests/e2e_okna.rs` ich vie overiť priamo bez súborov.
pub fn ukaz_a_zaostri<R: Runtime>(app: &AppHandle<R>, label: &str) -> Vec<(String, String)> {
    let mut chyby = Vec::new();

    let okno = match app.get_webview_window(label) {
        Some(okno) => okno,
        None => match znova_postav_okno(app, label) {
            Ok(okno) => okno,
            Err(e) => {
                chyby.push((format!("znova postaviť okno '{label}'"), e));
                return chyby;
            }
        },
    };

    if let Err(e) = okno.show() {
        chyby.push((format!("show('{label}')"), e.to_string()));
    }
    if let Err(e) = okno.unminimize() {
        chyby.push((format!("unminimize('{label}')"), e.to_string()));
    }
    if !je_na_viditelnom_monitore(&okno) {
        if let Err(e) = okno.center() {
            chyby.push((
                format!("center('{label}') po zistení pozície mimo obrazovky"),
                e.to_string(),
            ));
        }
    }
    if let Err(e) = okno.set_focus() {
        chyby.push((format!("set_focus('{label}')"), e.to_string()));
    }

    chyby
}
