//! Farm tracker — čítanie herného logu `UE_game.log` a výpočet čistého zisku z máp.
//!
//! Celý modul je zámerne BEZ Windows API a bez práce so skutočným súborom:
//! dostáva text/bajty a vracia dáta, takže `cargo test` nad ním beží aj na
//! Linuxe v CI. Otvorenie/seek/read súboru a Windows registry (Steam) rieši
//! `src-tauri` — sem len odovzdá bajty (`chvost`) alebo nájdené cesty
//! (`steam`).
//!
//! Čo presne sa z logu dá vyčítať a prečo, je v
//! `docs/fakty/hry/torchlight-infinite/log-format.md` v ops repe.

pub mod log;
pub mod agregacie;
pub mod ceny;
pub mod ceny_sync;
pub mod chvost;
pub mod db;
pub mod import;
pub mod poradca;
pub mod pripomienka;
pub mod stav;
pub mod steam;
pub mod sync;
pub mod taktiky;
pub mod trh;
pub mod upload;
pub mod varovania;
