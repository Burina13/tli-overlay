//! Jadro overlay klienta „Tvoj Tatko TLI Overlay“.
//!
//! Je úmyselne oddelené od Tauri aj od Windows API: prepočet súradníc, nastavenia,
//! parsovanie odpovedí servera a zarážka auto-updatu sa tak dajú testovať `cargo test`
//! kdekoľvek — aj na Linuxe v CI, kde sa appka ako taká ani zostaviť nedá. Všetko, čo
//! potrebuje obrazovku alebo okno hry, žije v `src-tauri/`.

pub mod api;
pub mod aktualizacia;
pub mod nastavenia;
pub mod obraz;
pub mod suradnice;
pub mod zmensenie;

pub mod tracker;

pub mod guide;
