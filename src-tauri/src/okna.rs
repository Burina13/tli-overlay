//! Práca s oknami appky (TLI 23 — hotfix „okno sa po prvom zatvorení už
//! znova neotvorí"). Vytiahnuté z `main.rs` do vlastného súboru, aby bola
//! logika na jednom mieste a aby sa dala aspoň čiastočne (`bod_v_obdlzniku`)
//! otestovať bez behu celej appky.
//!
//! **Prečo tu nie je plný E2E test cez `tauri::test::mock_builder()`:**
//! skúšalo sa to (`tests/e2e_okna.rs`, zmazaný v tomto commite) — na Linuxe
//! (CT 111) test prešiel, ale na Windows CI appka job padol s
//! `STATUS_ENTRYPOINT_NOT_FOUND` hneď pri štarte testovacej binárky (bez
//! ohľadu na obsah testu — padalo to už na prázdnom `mock_builder().build(…)`).
//! Príčina je niekde v tom, ako `tauri`/`wry` linkuje natívne Windows
//! závislosti pre `MockRuntime` binárku oproti bežnej appke/`e2e_mock.rs`
//! (ktorý `tauri::test` nepoužíva a beží bez problému) — presné odladenie by
//! vyžadovalo Windows stroj, ktorý CT 111 nemá, a `[FABLE]`/`[GEMINI]`
//! markery sem (zásah) nepatria. **Čaká to na Richarda/ďalší beh s prístupom
//! k Windowsu** — zapísané v `BACKLOG.md`. Oprava samotná (nižšie) je
//! overená priamym porovnaním s Tauri v2 dokumentáciou k
//! `on_window_event`/`CloseRequested`/`prevent_close` (štandardný, bežne
//! používaný vzor na „zavri = skry") a ručným code review proti presne
//! popísanému bugu — nie automatizovaným testom nad živým oknom.
//!
//! Funkcie sú generické cez `R: tauri::Runtime`, nie pevne na `tauri::Wry`
//! (predvolený runtime appky) — pripravené na to, keby sa raz
//! `tauri::test` predsa len dalo rozbehnúť.

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

/// Je bod `(x, y)` vo vnútri obdĺžnika so začiatkom `(rx, ry)` a rozmermi
/// `(rw, rh)`? Čistá geometria bez Tauri — jediná časť tohto súboru, ktorá
/// sa dá otestovať bez živého okna (pozri `#[cfg(test)] mod testy` nižšie
/// a vysvetlenie v hlavičke súboru, prečo zvyšok testovaný nie je).
fn bod_v_obdlzniku(x: i32, y: i32, rx: i32, ry: i32, rw: u32, rh: u32) -> bool {
    x >= rx && x < rx + rw as i32 && y >= ry && y < ry + rh as i32
}

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
        bod_v_obdlzniku(stred_x, stred_y, mp.x, mp.y, ms.width, ms.height)
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
/// chyby").
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

#[cfg(test)]
mod testy {
    use super::bod_v_obdlzniku;

    #[test]
    fn stred_vo_vnutri_monitora() {
        assert!(bod_v_obdlzniku(960, 540, 0, 0, 1920, 1080));
    }

    #[test]
    fn lavy_horny_roh_patri_dnu_pravy_dolny_uz_nie() {
        // Polootvorený interval — konzistentné s tým, ako Windows/monitory
        // súradnice bežne počítajú (šírka/výška je exkluzívna hranica).
        assert!(bod_v_obdlzniku(0, 0, 0, 0, 1920, 1080));
        assert!(!bod_v_obdlzniku(1920, 1080, 0, 0, 1920, 1080));
    }

    #[test]
    fn bod_vlavo_od_monitora_je_mimo() {
        assert!(!bod_v_obdlzniku(-100, 540, 0, 0, 1920, 1080));
    }

    #[test]
    fn bod_na_druhom_monitore_vpravo_je_v_nom_vo_vnutri() {
        // Druhý monitor začína presne tam, kde prvý končí (bežné rozloženie
        // "vedľa seba") — bod 2000 patrí doňho, nie prvému.
        assert!(!bod_v_obdlzniku(2000, 540, 0, 0, 1920, 1080));
        assert!(bod_v_obdlzniku(2000, 540, 1920, 0, 1920, 1080));
    }

    #[test]
    fn prazdny_zoznam_monitorov_nenajde_ziadnu_zhodu() {
        // Presne tento prípad appka dostane v `tauri::test::MockRuntime`
        // (`available_monitors()` tam vždy vráti prázdny zoznam) — žiaden
        // bod sa nezhoduje so žiadnym obdĺžnikom z prázdneho zoznamu.
        let monitory: Vec<(i32, i32, u32, u32)> = vec![];
        assert!(!monitory
            .iter()
            .any(|&(rx, ry, rw, rh)| bod_v_obdlzniku(960, 540, rx, ry, rw, rh)));
    }
}
