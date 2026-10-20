//! Lokálny dátový model Tartarus (SS14) — čistý Rust ekvivalent `model.py`
//! (`stacks/games/tli/engine/model.py` v repe `ops`), aby `tartarus_engine.rs` a
//! `tartarus_solver.rs` vracali presne tie isté UDALOSTI/STAV ako serverový
//! Python engine pre rovnaký vstup (TLI 63, bod 2).
//!
//! **Config ako natvrdo prenesené hodnoty, nie embedded YAML** — rovnaký princíp
//! ako `canvas.rs` (ten `canvas.yml` tiež nenačítava). Všetky čísla tu zodpovedajú
//! `stacks/games/tli/engine/tartarus.yml` v repe `ops` k 20.10.2026; keby sa config
//! tam zmenil, treba zmeniť aj tu (žiadne zdieľané načítanie, pozor pri budúcich
//! úpravách `tartarus.yml`).
//!
//! **Žiadny `rand` crate** (core zatiaľ žiadny generátor nepoužíval) — vlastný
//! SplitMix64 `Rng` nižšie. Python `random.Random` (Mersenne Twister) a tento Rng
//! NIE SÚ bit-zhodné a ani to nie je cieľom: TLI 63/2 bod 2 hovorí, že „zhoda“ sa
//! overuje na FIXNÝCH vstupoch (ručne zostavené dosky), nie cez zdieľaný seed.

use std::collections::HashMap;

// ── Konštanty (model.py) ────────────────────────────────────────────────────

pub const BEASTS: [&str; 4] = ["mammoth", "lion", "wolf", "rat"];
pub const TIERS: [&str; 6] = ["white", "blue", "purple", "orange", "supreme", "chromatic"];
pub const CHROMATIC: i64 = 5;
pub const SUPREME: i64 = 4;
pub const ORANGE: i64 = 3;
pub const PURPLE: i64 = 2;
pub const BLUE: i64 = 1;
pub const OFFICE: usize = 0;
pub const CELL_COUNT: usize = 16;

/// 31 nástrojov SS14 (TLI 25) — rovnaké poradie ako `model.py:TOOL_IDS`.
pub const TOOL_IDS: [&str; 31] = [
    "revenge", "muzzle", "saw", "keys", "rope", "fire", "ironbox",
    "skewer", "hook", "pestle", "scales", "tongs", "guillotine",
    "axe", "shackles", "collar", "forged", "dogtag", "cage",
    "blacklist", "shortchain", "maiden", "whip", "ravenous",
    "scapegoat", "stake", "coin", "crown", "maul", "claws", "trap",
];

pub const GIFT_IDS: [&str; 4] = ["ring", "horn", "wine", "brand"];

/// 11 Prison Incidents (TLI 63) — rovnaké poradie ako `model.py:EVENT_KINDS`
/// a `tartarus.yml:events.kinds`.
pub const EVENT_KINDS: [&str; 11] = [
    "chain_reaction", "execution", "trickster", "ravenous_appetite",
    "high_risk_convict", "crow", "freedom", "hunt_doors", "portion",
    "ratway", "usurpation",
];

// ── `tartarus.yml` hodnoty (PRAX. HODNOTA aj OVERENÉ, pozri zdrojový YAML) ──

pub const DRILL_DAYS: i64 = 20;
pub const PATROL_DESTINATIONS: usize = 3;
pub const PATROL_RANGE: usize = 5;
pub const FEWER_DOORS_CHANCE_PCT: f64 = 0.0;

pub const EVENTS_CHANCE_PCT: f64 = 18.0;
pub const EXECUTION_SAME_TIER: bool = false;
/// `tartarus.yml` nemá kľúč `execution_rerolls` vôbec — `cfg.get(..., False)`
/// v Pythone je preto vždy `False`.
pub const EXECUTION_REROLLS: bool = false;

pub const TUSK_RAISE_COUNT: usize = 4;
pub const PREY_ROUNDS: i64 = 3;
pub const SCRATCH_BONUS_TIERS: i64 = 3;
pub const NEST_DAYS: i64 = 2;
/// `nest_hatch_tier: purple` → index 2 = `PURPLE`.
pub const NEST_HATCH_TIER: i64 = PURPLE;

pub const TIER_WEIGHTS: [(&str, f64); 6] = [
    ("white", 52.0), ("blue", 27.0), ("purple", 15.0),
    ("orange", 6.0), ("supreme", 0.0), ("chromatic", 0.0),
];
pub const BEAST_WEIGHTS: [(&str, f64); 4] = [
    ("mammoth", 25.0), ("lion", 25.0), ("wolf", 25.0), ("rat", 25.0),
];

pub fn tier_index(nazov: &str) -> i64 {
    TIERS.iter().position(|&t| t == nazov).expect("neznámy tier") as i64
}

/// `model.py:food_chain_mapa` — len `variant` "single" (predvolené, jediné
/// použité v `novy_board`/enginu) a "loop" (alternatívny režim z configu,
/// prenesený pre úplnosť, hoci sa dnes nikde nevolá s `food_chain="loop"`).
pub fn food_chain_mapa(variant: &str, beast: &str) -> &'static [&'static str] {
    match (variant, beast) {
        ("single", "mammoth") => &["lion"],
        ("single", "lion") => &["wolf"],
        ("single", "wolf") => &["rat"],
        ("single", "rat") => &["mammoth"],
        ("loop", "mammoth") => &["lion", "wolf"],
        ("loop", "lion") => &["wolf", "rat"],
        ("loop", "wolf") => &["rat", "mammoth"],
        ("loop", "rat") => &["mammoth", "lion"],
        _ => &[],
    }
}

// ── RNG — vlastný SplitMix64, žiadna externá závislosť ──────────────────────

#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// f64 v [0, 1) — ekvivalent `random.Random.random()` (nie bit-zhodné).
    pub fn random(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.random() * n as f64) as usize
        }
    }

    /// Ekvivalent `random.Random.choice`. Panikne na prázdnom slice — rovnako
    /// ako Python vyhodí `IndexError`; volajúci musí neprázdnosť zaručiť sám.
    pub fn choice<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// `k` RÔZNYCH prvkov bez opakovania — ekvivalent `random.Random.sample`.
    /// Poradie výsledku nie je zmysluplné, rovnako ako v Pythone.
    pub fn sample<T: Clone>(&mut self, items: &[T], k: usize) -> Vec<T> {
        let mut pool: Vec<T> = items.to_vec();
        let k = k.min(pool.len());
        let mut out = Vec::with_capacity(k);
        for _ in 0..k {
            let i = self.below(pool.len());
            out.push(pool.remove(i));
        }
        out
    }

    /// Výber kľúča váženo podľa `vahy` — ekvivalent `model.py:_vazeny_vyber`.
    pub fn vazeny_vyber(&mut self, vahy: &[(&str, f64)]) -> String {
        let celkom: f64 = vahy.iter().map(|(_, v)| v.max(0.0)).sum();
        if celkom <= 0.0 {
            let keys: Vec<&str> = vahy.iter().map(|(k, _)| *k).collect();
            return (*self.choice(&keys)).to_string();
        }
        let mut r = self.random() * celkom;
        for &(k, v) in vahy.iter() {
            let w = v.max(0.0);
            if r < w {
                return k.to_string();
            }
            r -= w;
        }
        vahy.last().unwrap().0.to_string()
    }
}

// ── Dátový model (model.py: Inmate/Cell/PreyLink/Board) ─────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Inmate {
    pub beast: String,
    pub tier: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub index: usize,
    pub sealed: bool,
    pub inmate: Option<Inmate>,
    pub known_tier: Option<i64>,
    pub tusk: bool,
    pub scratch: bool,
    pub nest_due_day: Option<i64>,
    // ── interné, NIE je v board.schema.json (rovnaký komentár ako v Pythone) ──
    pub hidden_inmate: Option<Inmate>,
    pub hidden_event: Option<String>,
}

impl Cell {
    fn office() -> Self {
        Cell {
            index: OFFICE,
            sealed: false,
            inmate: None,
            known_tier: None,
            tusk: false,
            scratch: false,
            nest_due_day: None,
            hidden_inmate: None,
            hidden_event: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreyLink {
    pub lion_cell: usize,
    pub prey_cell: usize,
    pub prey_beast: String,
    pub rounds_left: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingEvent {
    pub kind: String,
    pub cell: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Board {
    pub day: i64,
    pub food_chain: String,
    pub warden: usize,
    pub cells: Vec<Cell>,
    pub destinations: Vec<usize>,
    pub tools: Vec<String>,
    pub used_devour: bool,
    pub used_swap: bool,
    pub prey: Vec<PreyLink>,
    pub pending_event: Option<PendingEvent>,
    pub chain_armed: bool,
    pub vault: i64,
    pub pending_gift: Option<Vec<String>>,
    pub pending_tool_choice: Option<Vec<String>>,
    pub milestone_tool_offered: bool,
    pub gifts_pending: i64,
    pub seed: Option<String>,
    pub usurpation_stage: i64,
}

impl Board {
    pub fn cell(&self, index: usize) -> &Cell {
        &self.cells[index]
    }

    pub fn cell_mut(&mut self, index: usize) -> &mut Cell {
        &mut self.cells[index]
    }

    pub fn has_tool(&self, tool_id: &str) -> bool {
        self.tools.iter().any(|t| t == tool_id)
    }

    pub fn eats_of(&self, beast: &str) -> &'static [&'static str] {
        food_chain_mapa(&self.food_chain, beast)
    }

    /// Políčka s PRÁVE ZNÁMYM (otvoreným) väzňom.
    pub fn board_cells(&self) -> Vec<&Cell> {
        self.cells.iter().filter(|c| !c.sealed && c.inmate.is_some()).collect()
    }

    /// Ekvivalent `Board.from_schema_dict` (model.py), aplikovaný priamo na
    /// vlastný typ namiesto JSON — TLI 63/2 bod 4 hovorí, že appka zatiaľ
    /// NEMÁ odkiaľ brať VEREJNÝ (neúplný) stav dosky (žiadne lokálne
    /// rozpoznanie obrazovky), takže `board.schema.json` (de)serializácia je
    /// zámerne mimo rozsahu tohto kroku. Pre Monte Carlo (`tartarus_solver.rs`)
    /// to ale netreba: stačí vedieť pre KAŽDÝ rollout znova vyžrebovať skrytú
    /// pravdu zamknutých políčok nad tým istým vstupným `Board`, presne ako to
    /// `from_schema_dict` robí nad tým istým vstupným JSON dictom pri každom
    /// volaní. `known_tier` (Scratch/Crow/Hunt Doors markery) sa rešpektuje.
    pub fn fog_reroll(&self, rng: &mut Rng) -> Board {
        let mut b = self.clone();
        let stage = b.usurpation_stage;
        for c in b.cells.iter_mut() {
            if c.sealed {
                c.hidden_inmate = Some(roluj_vazna(rng, c.known_tier));
                c.hidden_event = roluj_event_stage(rng, stage);
            }
        }
        b
    }
}

// ── Žreby (model.py) ─────────────────────────────────────────────────────────

/// Nový väzeň podľa `tartarus.yml:new_inmate`. `known_tier` vynúti raritu
/// (Scratch marker ju už prezrádza) a žrebuje sa len beast.
pub fn roluj_vazna(rng: &mut Rng, known_tier: Option<i64>) -> Inmate {
    let beast = rng.vazeny_vyber(&BEAST_WEIGHTS);
    if let Some(t) = known_tier {
        return Inmate { beast, tier: t };
    }
    let tier_nazov = rng.vazeny_vyber(&TIER_WEIGHTS);
    Inmate { beast, tier: tier_index(&tier_nazov) }
}

/// Kódy udalostí, ktoré sa PRÁVE TERAZ môžu vyžrebovať do `hidden_event`.
/// `usurpation` zmizne, keď reťaz I→IV už dobehla.
pub fn dostupne_eventy_stage(usurpation_stage: i64) -> Vec<&'static str> {
    let mut kinds: Vec<&'static str> = EVENT_KINDS.to_vec();
    if usurpation_stage >= 4 {
        kinds.retain(|&k| k != "usurpation");
    }
    if kinds.is_empty() {
        vec!["execution"]
    } else {
        kinds
    }
}

pub fn dostupne_eventy(board: &Board) -> Vec<&'static str> {
    dostupne_eventy_stage(board.usurpation_stage)
}

pub fn roluj_event_stage(rng: &mut Rng, usurpation_stage: i64) -> Option<String> {
    if rng.random() * 100.0 < EVENTS_CHANCE_PCT {
        Some(rng.choice(&dostupne_eventy_stage(usurpation_stage)).to_string())
    } else {
        None
    }
}

pub fn roluj_event(rng: &mut Rng, board: &Board) -> Option<String> {
    roluj_event_stage(rng, board.usurpation_stage)
}

/// Čerstvý drill: deň 1, dozorca v kancelárii, 15 väzenských políčok
/// zamknutých so ZADANOU skrytou pravdou (žrebuje sa hneď, nie lenivo).
pub fn novy_board(rng: &mut Rng, food_chain: Option<&str>, seed: Option<String>) -> Board {
    let mut b = Board {
        day: 1,
        food_chain: food_chain.unwrap_or("single").to_string(),
        warden: OFFICE,
        cells: vec![],
        destinations: vec![],
        tools: vec![],
        used_devour: false,
        used_swap: false,
        prey: vec![],
        pending_event: None,
        chain_armed: false,
        vault: 0,
        pending_gift: None,
        pending_tool_choice: None,
        milestone_tool_offered: false,
        gifts_pending: 0,
        seed,
        usurpation_stage: 0,
    };
    let mut cells = vec![Cell::office()];
    for i in 1..CELL_COUNT {
        let hidden = roluj_vazna(rng, None);
        let udalost = roluj_event(rng, &b);
        cells.push(Cell {
            index: i,
            sealed: true,
            inmate: None,
            known_tier: None,
            tusk: false,
            scratch: false,
            nest_due_day: None,
            hidden_inmate: Some(hidden),
            hidden_event: udalost,
        });
    }
    b.cells = cells;
    b
}

/// Pomocná funkcia pre `HashMap` kľúčovanie v `tartarus_engine.rs::end_day`
/// (Kin's Coin of Fate) — v Pythone je to obyčajný `dict[(beast, tier), ...]`.
pub type SkupinaKluc = (String, i64);
pub fn nova_skupina_mapa() -> HashMap<SkupinaKluc, Vec<usize>> {
    HashMap::new()
}
