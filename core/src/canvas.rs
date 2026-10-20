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
//!
//! **Výnimka od TLI 69 (20.10.2026): pole `oramovana`** appka plní SAMA z
//! pixelov (`canvas_ramy.rs`) — je to jediná časť stavu plátna, ktorú vie
//! prečítať lokálne a spoľahlivo. Hra ňou označuje platné cieľové bunky pre
//! AKÝKOĽVEK smer, takže dosah sa už neodvodzuje z názvu smeru
//! (`cielove_bunky`). Rám ostáva aj na bunke, ktorá je už obsadená.

use serde::{Deserialize, Serialize};

/// Ortogonálni susedia (hore/dole/vľavo/vpravo) — smer `adjacent` je OVERENÝ zo
/// screenshotu (žlté orámovanie okolo kľúčovej bunky), žiadne diagonály.
pub const SUSEDIA_ORTO: [(i64, i64); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Smery, pre ktoré je OVERENÝ aj EFEKT (o koľko levelov vyskočí položený
/// paint), nielen dosah. Dosah je od TLI 69 istý pre všetky smery — berie sa
/// zo žltého rámu hry. Zrkadlí `SMERY_S_OVERENYM_EFEKTOM` v `canvas_model.py`.
pub const SMERY_S_OVERENYM_EFEKTOM: [&str; 1] = ["adjacent"];

/// Odkiaľ solver vzal dosah (ktoré bunky sú platné cieľové).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZdrojDosahu {
    /// Hra bunky sama žlto orámovala — platí pre ľubovoľný smer.
    Ram,
    /// Rám na snímke nebol, dosah sa odvodil z názvu smeru.
    Smer,
}

impl ZdrojDosahu {
    pub fn nazov(&self) -> &'static str {
        match self {
            ZdrojDosahu::Ram => "ram",
            ZdrojDosahu::Smer => "smer",
        }
    }
}

/// Súradnice, na ktoré podľa NÁZVU smeru dosiahne Color Mixing z bunky
/// (`riadok`, `stlpec`). ZÁLOHA pre snímky bez rozpoznaného rámu — zrkadlí
/// `canvas_model.bunky_v_smere()` na serveri, riadok po riadku.
pub fn bunky_v_smere(smer: &str, riadok: i64, stlpec: i64, riadky: i64, stlpce: i64) -> Vec<(i64, i64)> {
    let s = smer.trim().to_ascii_lowercase();
    let mut out: Vec<(i64, i64)> = vec![];
    match s.as_str() {
        "adjacent" => {
            for (dr, dc) in SUSEDIA_ORTO {
                out.push((riadok + dr, stlpec + dc));
            }
        }
        "vertical" => {
            for r in 1..=riadky {
                if r != riadok {
                    out.push((r, stlpec));
                }
            }
        }
        "horizontal" => {
            for c in 1..=stlpce {
                if c != stlpec {
                    out.push((riadok, c));
                }
            }
        }
        "diagonal" => {
            for k in 1..riadky.max(stlpce) {
                out.push((riadok - k, stlpec - k));
                out.push((riadok - k, stlpec + k));
                out.push((riadok + k, stlpec - k));
                out.push((riadok + k, stlpec + k));
            }
        }
        "cross" => {
            for r in 1..=riadky {
                if r != riadok {
                    out.push((r, stlpec));
                }
            }
            for c in 1..=stlpce {
                if c != stlpec {
                    out.push((riadok, c));
                }
            }
        }
        _ => {}
    }
    out.retain(|(r, c)| (1..=riadky).contains(r) && (1..=stlpce).contains(c));
    out
}

/// „Fill a Vertical Cell with a **Gameplay** Paint(0/3)" → `"gameplay"`.
/// Zrkadlí `canvas_model.kategoria_z_popisu()`.
pub fn kategoria_z_popisu(popis: &str) -> &'static str {
    if popis.to_ascii_lowercase().contains("gameplay") {
        "gameplay"
    } else {
        "monster"
    }
}

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
    /// TLI 69: hra túto bunku žlto orámovala (`canvas_ramy.rs`). Je na nej aj
    /// vtedy, keď je už obsadená — „kam sa dá položiť" je preto
    /// `oramovana && dostupna && paint.is_none()`.
    #[serde(default)]
    pub oramovana: bool,
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

    /// Všetky bunky, ktoré hra orámovala — vrátane už obsadených.
    pub fn oramovane_bunky(&self) -> Vec<&Bunka> {
        self.bunky.iter().filter(|b| b.oramovana).collect()
    }

    /// Kam sa dá položiť paint, a ODKIAĽ to vieme. Zrkadlí
    /// `CanvasState.cielove_bunky()` na serveri. Bunky sú zoradené
    /// riadok/stĺpec, aby bolo poradie krokov deterministické.
    pub fn cielove_bunky(&self, ciel: &CielMiesania) -> (Vec<&Bunka>, ZdrojDosahu) {
        let oramovane = self.oramovane_bunky();
        if !oramovane.is_empty() {
            let mut volne: Vec<&Bunka> =
                oramovane.into_iter().filter(|b| b.dostupna && b.paint.is_none()).collect();
            volne.sort_by_key(|b| (b.riadok, b.stlpec));
            return (volne, ZdrojDosahu::Ram);
        }
        let (kr, ks) = match (ciel.kluc_riadok, ciel.kluc_stlpec) {
            (Some(r), Some(c)) => (r, c),
            _ => return (vec![], ZdrojDosahu::Smer),
        };
        let mut volne: Vec<&Bunka> = bunky_v_smere(&ciel.smer, kr, ks, self.riadky, self.stlpce)
            .into_iter()
            .filter_map(|(r, c)| self.bunka(r, c))
            .filter(|b| b.dostupna && b.paint.is_none())
            .collect();
        volne.sort_by_key(|b| (b.riadok, b.stlpec));
        (volne, ZdrojDosahu::Smer)
    }
}
