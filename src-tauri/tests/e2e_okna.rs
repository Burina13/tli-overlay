//! TLI 23 — regresný test „okno sa po prvom zatvorení už znova neotvorí".
//!
//! Hlásil Richard 16.10.2026: klik na „Nastavenia" okno otvorí, X ho zavrie,
//! druhý klik už nič neotvorí. Príčina (`main.rs`, pred opravou): appka
//! nemala `on_window_event(CloseRequested)`, takže Tauri v2 okno pri X
//! PREDVOLENE ZNIČILO, a `ukaz_a_zaostri` pri `get_webview_window` == `None`
//! ticho nerobila nič — appka sa tvárila v poriadku, len druhý klik naozaj
//! neotváral nič.
//!
//! **Prečo tento test necvičí skutočný klik na X:** Tauri `MockRuntime`
//! (`tauri::test::mock_builder`) nemá na to podporu — overené priamo v zdroji
//! balíka `tauri-2.12.3`:
//! - `impl WindowDispatch<T> for MockWindowDispatcher { fn on_window_event(&self, f: F)
//!   -> WindowEventId { self.context.next_window_event_id() } }` — handler `f` sa
//!   nikam neuloží, len sa vráti nové ID. Nijaký zaregistrovaný
//!   `Builder::on_window_event`/`Window::on_window_event` sa teda cez mock
//!   runtime NIKDY nezavolá.
//! - `Runtime::run_iteration` je pre `MockRuntime` prázdne telo (`{}`) —
//!   čakajúce správy (`Message::CloseWindow`/`DestroyWindow`) spracúva len
//!   `run()`, ktorý sa nedá v teste bezpečne spustiť (je to nekonečná
//!   zaneprázdnená slučka, ktorá sa inak ukončí len skutočným ukončením appky).
//!
//! Test preto cieli priamo na pozorovateľný príznak TEJ ISTEJ triedy chyby —
//! appka musí vedieť okno OTVORIŤ aj vtedy, keď `get_webview_window` vráti
//! `None` (presne to vráti aj po reálnom zničení oknom manažérom OS — z
//! pohľadu `main.rs` niet medzi týmito dvoma stavmi žiadny rozdiel). Testuje
//! sa pre VŠETKY okná appky (zadanie, bod 1), v 3 nezávislých cykloch
//! (zadanie, bod 3) — každý cyklus so svojou čerstvou mock appkou, keďže mock
//! runtime nevie okno naozaj zničiť a nechať appku bežať ďalej (viď vyššie).
//!
//! `cargo fix`/`clippy` toto nerieši — je to medzera v testovacej
//! infraštruktúre Tauri v2, nie v appke. Logika sa preto testuje priamo cez
//! `okna::ukaz_a_zaostri` (`src/okna.rs`), ktorú volá aj `main.rs`.

#[path = "../src/okna.rs"]
mod okna;

use tauri::Manager;

/// Presne tých 5 okien, ktoré appka deklaruje v `tauri.conf.json`
/// (`main.rs`: `OVERLAY`, `NASTAVENIA`, `MINI`, `HLAVNE`, `PRIPOMIENKA`) —
/// zámerne vypísané tu znova namiesto importu konštánt z `main.rs` (ten sa
/// do testu nedá zahrnúť, má `fn main()` a `#![cfg_attr(..., windows_subsystem)]`
/// platné len na koreni crate-u). Keď pribudne ďalšie okno, pridaj ho sem.
const VSETKY_OKNA: [&str; 5] = ["overlay", "nastavenia", "mini", "hlavne", "pripomienka"];

fn mock_app() -> tauri::App<tauri::test::MockRuntime> {
    tauri::test::mock_builder()
        .build(tauri::generate_context!())
        .expect("mock appka sa nepostavila")
}

/// Jadro testu: nad ČERSTVOU appkou (okno teda neexistuje, presne ako po
/// tom, čo ho Tauri zničilo) musí `okna::ukaz_a_zaostri`:
/// 1. okno znova postaviť a ukázať bez jedinej ticho zahodenej chyby,
/// 2. nechať appku v stave, že `get_webview_window` ho odteraz nájde.
///
/// Na nefixnutom kóde (starý `main.rs::ukaz_a_zaostri`, ktorý pri `None` len
/// `if let Some(okno) = ... { ... }` a inak nič) by krok 2 zlyhal —
/// `get_webview_window` by ostalo `None` aj po volaní. To je presne TLI 23.
fn over_otvorenie_chybajuceho_okna(label: &str) {
    let app = mock_app();

    assert!(
        app.get_webview_window(label).is_none(),
        "'{label}': čerstvá appka by ešte nemala mať toto okno postavené"
    );

    let chyby = okna::ukaz_a_zaostri(app.handle(), label);
    assert!(
        chyby.is_empty(),
        "'{label}': ukaz_a_zaostri nahlásila chyby namiesto ich ticheho zahodenia: {chyby:?}"
    );

    assert!(
        app.get_webview_window(label).is_some(),
        "'{label}': okno sa malo znova postaviť a ukázať, ale `get_webview_window` \
         vrátilo None — presne bug TLI 23 (druhý klik na tray menu neotvoril nič)"
    );
}

/// Zadanie TLI 23, bod 3: „3 cykly, pre každé okno appky". Každý cyklus je
/// samostatná appka (dôvod je v hlavičke súboru) — opakovanie overuje, že
/// oprava nie je závislá od poradia/počtu predošlých volaní a že v nej nie
/// je skryté zdieľané stavové „funguje len prvýkrát".
#[test]
fn tli23_okno_sa_da_otvorit_aj_ked_ho_tauri_zrusilo() {
    for okno in VSETKY_OKNA {
        for cyklus in 1..=3 {
            over_otvorenie_chybajuceho_okna(okno);
            let _ = cyklus; // len na čitateľnosť zámeru v prípade zlyhania assertu vyššie
        }
    }
}

/// `ukaz_a_zaostri` nesmie zlyhať/panikáriť na neznámej značke okna — appka
/// v budúcnosti pribudne o ďalšie okno a zabudnutý záznam v `tauri.conf.json`
/// sa nesmie prejaviť pádom, len nahlásenou chybou (bod 2 zadania).
#[test]
fn neznama_znacka_okna_vrati_chybu_nie_paniku() {
    let app = mock_app();
    let chyby = okna::ukaz_a_zaostri(app.handle(), "neexistujuce-okno-xyz");
    assert_eq!(chyby.len(), 1, "{chyby:?}");
    assert!(chyby[0].1.contains("neexistujuce-okno-xyz"), "{chyby:?}");
}
