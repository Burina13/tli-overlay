//! Jadro overlay klienta „Tvoj Tatko TLI Overlay“.
//!
//! Je úmyselne oddelené od Tauri aj od Windows API: prepočet súradníc, nastavenia,
//! parsovanie odpovedí servera a zarážka auto-updatu sa tak dajú testovať `cargo test`
//! kdekoľvek — aj na Linuxe v CI, kde sa appka ako taká ani zostaviť nedá. Všetko, čo
//! potrebuje obrazovku alebo okno hry, žije v `src-tauri/`.

pub mod api;
pub mod aktualizacia;
pub mod canvas;
pub mod canvas_detekcia;
pub mod canvas_obsah;
pub mod canvas_ocr;
pub mod canvas_rozlozenie;
pub mod canvas_sablony;
pub mod canvas_solver;
pub mod denik;
pub mod minimapa_vzhlad;
pub mod nastavenia;
pub mod obraz;
pub mod rozlozenie;
pub mod suradnice;
pub mod tartarus;
pub mod tartarus_engine;
pub mod tartarus_solver;
pub mod zber_fronta;
pub mod zmensenie;

pub mod tracker;

pub mod guide;
