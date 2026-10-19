//! Lokálny dátový model plátna Frozen Canvas — čistý Rust ekvivalent `canvas_model.py`
//! z `tli-web` (`stacks/games/tli/engine/canvas_model.py` v repe `ops`), aby solver
//! (`canvas_solver.rs`) vracal presne tú istú sekvenciu krokov ako server pre rovnaký
//! vstup (TLI 59, bod 2).
//!
//! Zdrojom `CanvasStav` je zatiaľ VÝHRADNE odpoveď servera (pole `platno` v
//! `api::AnalyzaCanvas`, dnes nepoužité `serde_json::Value`) — lokálne rozpoznanie
//! mriežky/ruky/OCR (TLI 59, bod 1) ešte nie je hotové, viď `ROZPRACOVANE` v `STAV.md`.
//! Modul preto zatiaľ nemá volajúceho v `src-tauri/`; je to pripravená, otestovaná
//! infraštruktúra pre neho, rovnako ako bol krok 1 TLI 5 pripravený server endpoint.

use serde::{Deserialize, Serialize};

/// Ortogonálni susedia (hore/dole/vľavo/vpravo) — smer `adjacent` je OVERENÝ zo
/// screenshotu (žlté orámovanie okolo kľúčovej bunky), žiadne diagonály.
pub const SUSEDIA_ORTO: [(i64, i64); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paint {
    pub kategoria: String,
    pub typ: String,
    pub level: i64,
}

impl Paint {
    pub fn monster(typ: &str, level: i64) -> Self {
        Self { kategoria: "monster".to_string(), typ: typ.to_string(), level }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bunka {
    pub riadok: i64,
    pub stlpec: i64,
    pub dostupna: bool,
    #[serde(default)]
    pub paint: Option<Paint>,
    #[serde(default)]
    pub oznacena_vykricnikom: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotRuky {
    pub index: i64,
    #[serde(default)]
    pub paint: Option<Paint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CielMiesania {
    pub smer: String,
    pub popis: String,
    pub efekt_popis: String,
    pub pocet_pozadovany: i64,
    pub pocet_aktualny: i64,
    #[serde(default)]
    pub kluc_riadok: Option<i64>,
    #[serde(default)]
    pub kluc_stlpec: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasStav {
    pub riadky: i64,
    pub stlpce: i64,
    pub max_level: i64,
    #[serde(default)]
    pub bunky: Vec<Bunka>,
    #[serde(default)]
    pub ruka: Vec<SlotRuky>,
    #[serde(default)]
    pub ciel: Option<CielMiesania>,
}

impl CanvasStav {
    pub fn bunka(&self, riadok: i64, stlpec: i64) -> Option<&Bunka> {
        self.bunky.iter().find(|b| b.riadok == riadok && b.stlpec == stlpec)
    }

    /// Dostupné, zatiaľ neobsadené políčka ortogonálne susediace so zadanou
    /// bunkou — presne `CanvasState.volne_susedne_bunky` z `canvas_model.py`.
    pub fn volne_susedne_bunky(&self, riadok: i64, stlpec: i64) -> Vec<&Bunka> {
        SUSEDIA_ORTO
            .iter()
            .filter_map(|(dr, dc)| self.bunka(riadok + dr, stlpec + dc))
            .filter(|b| b.dostupna && b.paint.is_none())
            .collect()
    }
}
