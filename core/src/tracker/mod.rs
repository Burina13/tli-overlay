//! Farm tracker — čítanie herného logu `UE_game.log` a výpočet čistého zisku z máp.
//!
//! Celý modul je zámerne BEZ Windows API a bez práce so súbormi: dostáva text
//! a vracia dáta, takže `cargo test` nad ním beží aj na Linuxe v CI. Čítanie
//! súboru (tail, rotácia, kódovanie) rieši `src-tauri`.
//!
//! Čo presne sa z logu dá vyčítať a prečo, je v
//! `docs/fakty/hry/torchlight-infinite/log-format.md` v ops repe.

pub mod log;
pub mod agregacie;
pub mod ceny;
pub mod stav;
