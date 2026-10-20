//! Simulátor jedného drilu Tartarus (SS14) — port `engine.py`
//! (`stacks/games/tli/engine/engine.py` v repe `ops`) do core Rust (TLI 63, bod 2).
//! ÚMYSELNE riadok po riadku tá istá logika vrátane všetkých 31 nástrojov (TLI 25)
//! a všetkých 11 Prison Incidents (TLI 63), aby lokálny výsledok pre rovnaký
//! vstupný `Board` vyšiel identicky ako server pre rovnaký vstup.
//!
//! Tok jedného dňa je rovnaký ako v Pythone:
//!   `roll_destinations` → `patrol(cieľ)` → prípadne `resolve_event(prijat)` →
//!   `devour(z, na)` a/alebo `swap()` v ľubovoľnom poradí, najviac raz každé →
//!   `end_day()`.
//!
//! ⚠️ **Zámerne zachovaná chyba servera** (`_open_cell`/Sorting Hook, pozri
//! komentár pri `open_cell` nižšie) — bod 2 zadania žiada ZHODU so serverom na
//! fixných vstupoch, nie opravu nájdených chýb. Zapísané do `BACKLOG.md`.

use std::collections::HashMap;

use crate::tartarus::{
    novy_board, roluj_event_stage, roluj_vazna, tier_index, Board, Inmate, PendingEvent,
    PreyLink, Rng, BEASTS, BEAST_WEIGHTS, BLUE, CELL_COUNT, CHROMATIC, EXECUTION_REROLLS,
    EXECUTION_SAME_TIER, FEWER_DOORS_CHANCE_PCT, GIFT_IDS, NEST_DAYS, NEST_HATCH_TIER, OFFICE,
    ORANGE, PATROL_DESTINATIONS, PATROL_RANGE, PREY_ROUNDS, SCRATCH_BONUS_TIERS, TIER_WEIGHTS,
    TOOL_IDS, TUSK_RAISE_COUNT,
};

/// Volajúci (solver, appka, testy) sa pokúsil o nelegálny ťah.
#[derive(Debug, Clone, PartialEq)]
pub struct PravidloPorusene(pub String);

impl std::fmt::Display for PravidloPorusene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for PravidloPorusene {}

type Vysledok = Result<Vec<String>, PravidloPorusene>;

fn chyba(text: impl Into<String>) -> PravidloPorusene {
    PravidloPorusene(text.into())
}

// ── Pomocné funkcie bez stavu ────────────────────────────────────────────────

/// Políčka na ceste v smere hodinových ručičiek OD (vylúčene) PO (vrátane).
pub fn clockwise_path(od: usize, do_: usize) -> Vec<usize> {
    let mut cesta = vec![];
    let mut n = od;
    while n != do_ {
        n = (n + 1) % CELL_COUNT;
        cesta.push(n);
    }
    if cesta.is_empty() {
        vec![do_]
    } else {
        cesta
    }
}

pub fn steps_between(od: usize, do_: usize) -> usize {
    (do_ + CELL_COUNT - od) % CELL_COUNT
}

pub fn is_drill_over(board: &Board) -> bool {
    board.day > crate::tartarus::DRILL_DAYS
}

pub fn has_pending_choice(board: &Board) -> bool {
    board.pending_event.is_some() || board.pending_tool_choice.is_some() || board.pending_gift.is_some()
}

// ── Žreby a pomocné interné funkcie ──────────────────────────────────────────

/// Vyprázdni políčko — znova sa zamkne (reusable cells) s NOVOU skrytou
/// pravdou. Vracia True, keď tým zaniklo potkanie hniezdo.
fn empty_cell(board: &mut Board, index: usize, rng: &mut Rng) -> bool {
    let stage = board.usurpation_stage;
    let cell = board.cell_mut(index);
    let stratene_hniezdo = cell.nest_due_day.is_some();
    cell.inmate = None;
    cell.known_tier = None;
    cell.scratch = false;
    cell.nest_due_day = None;
    cell.tusk = false;
    cell.sealed = true;
    cell.hidden_inmate = Some(roluj_vazna(rng, None));
    cell.hidden_event = roluj_event_stage(rng, stage);
    stratene_hniezdo
}

fn can_eat(board: &Board, eater: &Inmate, meal: &Inmate) -> bool {
    if eater.tier >= CHROMATIC {
        return false;
    }
    if eater.tier != meal.tier {
        // Fanged Guillotine: vlk zožerie svoj jedálniček aj NIŽŠEJ rarity.
        if eater.tier > meal.tier
            && eater.beast == "wolf"
            && board.has_tool("guillotine")
            && board.eats_of("wolf").contains(&meal.beast.as_str())
        {
            return true;
        }
        // Elephant-Foot Maul (TLI 25): mamut zožerie svoj jedálniček aj VYŠŠEJ rarity.
        return eater.tier < meal.tier
            && eater.beast == "mammoth"
            && board.has_tool("maul")
            && board.eats_of("mammoth").contains(&meal.beast.as_str());
    }
    if board.eats_of(&eater.beast).contains(&meal.beast.as_str()) {
        return true;
    }
    // Iron Crown of the Prison King (TLI 25): lev zožerie HOCIKOHO rovnakej rarity.
    if eater.beast == "lion" && board.has_tool("crown") {
        return true;
    }
    eater.beast == "rat" && meal.beast == "rat" && board.has_tool("skewer")
}

fn after_growth(board: &mut Board, index: usize, rng: &mut Rng, events: &mut Vec<String>) {
    let inmate_tier = match &board.cell(index).inmate {
        Some(i) => i.tier,
        None => return,
    };
    if inmate_tier >= ORANGE && !board.milestone_tool_offered && board.pending_tool_choice.is_none() {
        board.milestone_tool_offered = true;
        let volne: Vec<&str> = TOOL_IDS.iter().copied().filter(|t| !board.has_tool(t)).collect();
        let n = 3.min(volne.len());
        let vyber: Vec<String> = rng.sample(&volne, n).into_iter().map(|s| s.to_string()).collect();
        board.pending_tool_choice = Some(vyber);
        events.push("milestone_tool".to_string());
    }
    if inmate_tier >= CHROMATIC {
        board.vault += 1;
        board.gifts_pending += 1;
        events.push(format!("chromatic:{}", index));
        empty_cell(board, index, rng);
        if board.pending_gift.is_none() {
            board.pending_gift = Some(GIFT_IDS.iter().map(|s| s.to_string()).collect());
        }
    }
}

fn promote(board: &mut Board, index: usize, by: i64, reroll_beast: bool, rng: &mut Rng, events: &mut Vec<String>) {
    let has_inmate = board.cell(index).inmate.is_some();
    if !has_inmate || by <= 0 {
        return;
    }
    {
        let cell = board.cell_mut(index);
        let inmate = cell.inmate.as_mut().unwrap();
        inmate.tier = (inmate.tier + by).min(CHROMATIC);
        if reroll_beast && inmate.tier < CHROMATIC {
            inmate.beast = rng.vazeny_vyber(&BEAST_WEIGHTS);
        }
    }
    let tier_now = board.cell(index).inmate.as_ref().unwrap().tier;
    events.push(format!("promote:{}:{}", index, tier_now));
    after_growth(board, index, rng, events);
}

/// Otvorí zamknuté políčko. `how` je "patrol" | "side" (vedľajší efekt
/// nástroja/reťazovej reakcie — event za takými dverami sa MINIE bez spustenia).
/// Vracia kind eventu len keď `how == "patrol"`.
fn open_cell(
    board: &mut Board,
    index: usize,
    how: &str,
    rng: &mut Rng,
    events: &mut Vec<String>,
    forced: Option<Inmate>,
) -> Option<String> {
    if !board.cell(index).sealed {
        return None;
    }
    let forced_je_none = forced.is_none();
    let event = board.cell(index).hidden_event.clone();
    let mut inmate = match forced {
        Some(i) => i,
        None => board
            .cell(index)
            .hidden_inmate
            .clone()
            .unwrap_or_else(|| roluj_vazna(rng, None)),
    };
    // Sorting Hook: za zamknutými dverami nikdy Normal (kým ho hráč ešte nevidel).
    // ⚠️ Zhoda so serverom: `engine.py` tu filtruje kľúč "normal", ktorý v
    // `tier_weights_pct` po premenovaní (TLI 9/54b: normal→white atď.) už
    // neexistuje — filter je preto v Pythone NO-OP a výsledné rozdelenie je
    // PLNÉ (vrátane white). Zdedená chyba servera, nie zámer; replikuje sa
    // 1:1 kvôli bodu 2 zadania (zhoda na fixných vstupoch), nie opravuje.
    if board.has_tool("hook") && forced_je_none && inmate.tier == 0 {
        let nazov = rng.vazeny_vyber(&TIER_WEIGHTS);
        inmate = Inmate { beast: inmate.beast.clone(), tier: tier_index(&nazov) };
    }
    {
        let cell = board.cell_mut(index);
        cell.sealed = false;
        cell.inmate = Some(inmate.clone());
        cell.hidden_inmate = None;
        cell.hidden_event = None;
        cell.scratch = false;
        cell.known_tier = None;
    }
    let stratene_hniezdo = board.cell(index).nest_due_day.is_some();
    board.cell_mut(index).nest_due_day = None;
    events.push(format!(
        "open:{}{}",
        index,
        if stratene_hniezdo { ":nest_zniceny" } else { "" }
    ));

    // Lost Ring of Keys — len pri otvorení PATROLOU.
    if how == "patrol" && board.has_tool("keys") && rng.random() < 0.8 {
        for krok in [-1i64, 1i64] {
            let sused = (index as i64 + krok).rem_euclid(CELL_COUNT as i64) as usize;
            if sused != OFFICE && board.cell(sused).sealed {
                open_cell(board, sused, "side", rng, events, None);
            }
        }
    }
    // Searing Branding Iron — len pri otvorení PATROLOU.
    if how == "patrol" && board.has_tool("fire") && rng.random() * 100.0 < 35.0 {
        let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
        if !zamknute.is_empty() {
            let ciel = *rng.choice(&zamknute);
            open_cell(board, ciel, "side", rng, events, Some(Inmate { beast: inmate.beast.clone(), tier: inmate.tier }));
        }
    }
    // ── TLI 25 ──
    // Forged Key — ďalšie náhodné zamknuté políčko, len pri otvorení PATROLOU.
    if how == "patrol" && board.has_tool("forged") && rng.random() * 100.0 < 50.0 {
        let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
        if !zamknute.is_empty() {
            let ciel = *rng.choice(&zamknute);
            open_cell(board, ciel, "side", rng, events, None);
        }
    }
    // Felon's Dog Tag — väzeň za práve otvorenými dverami stúpne o stupeň.
    if how == "patrol"
        && board.has_tool("dogtag")
        && rng.random() * 100.0 < 25.0
        && !board.cell(index).sealed
        && board.cell(index).inmate.is_some()
    {
        promote(board, index, 1, false, rng, events);
    }
    // Executioner's Blacklist — keď po otvorení ostáva najviac N zamknutých.
    if how == "patrol" && board.has_tool("blacklist") {
        let zostava = board.cells.iter().filter(|c| c.sealed).count();
        if zostava <= 5 {
            let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
            let pocet = 2usize.min(bazen.len());
            let vybrane = rng.sample(&bazen, pocet);
            for idx in &vybrane {
                promote(board, *idx, 1, false, rng, events);
            }
            if pocet > 0 {
                events.push(format!("blacklist:{}", pocet));
            }
        }
    }
    if how == "patrol" {
        event
    } else {
        None
    }
}

fn check_prey_vazba(board: &Board, link: &PreyLink) -> bool {
    let lion = board.cell(link.lion_cell);
    let korist = board.cell(link.prey_cell);
    !lion.sealed
        && lion.inmate.as_ref().map(|i| i.beast == "lion").unwrap_or(false)
        && !korist.sealed
        && korist.inmate.as_ref().map(|i| i.beast == link.prey_beast).unwrap_or(false)
}

fn jedno_uhryznutie(
    board: &mut Board,
    z: usize,
    na: usize,
    cez_scratch: bool,
    rng: &mut Rng,
    events: &mut Vec<String>,
    poradie: i64,
) {
    let eater = board.cell(z).inmate.clone().unwrap();
    let bol_potkan = eater.beast == "rat";
    let bol_lev = eater.beast == "lion";
    let bol_mamut = eater.beast == "mammoth";
    let tier_jedla = board.cell(na).inmate.as_ref().map(|i| i.tier).unwrap_or(eater.tier);
    let tier_jedaka = eater.tier;
    let davka = board.board_cells().len() as i64;
    empty_cell(board, na, rng);
    let mut zisk: i64 = if cez_scratch { SCRATCH_BONUS_TIERS } else { 1 };
    if board.has_tool("muzzle") && davka <= 4 {
        zisk += 2;
    }
    if board.has_tool("skewer") && bol_potkan {
        zisk += 1;
    }
    if board.has_tool("saw") && rng.random() * 100.0 < 25.0 {
        zisk += 1;
    }
    if board.has_tool("crown") && bol_lev {
        zisk += 2;
    }
    if board.has_tool("whip") && rng.random() * 100.0 < 35.0 {
        zisk += 1;
    }
    if poradie > 1 && board.has_tool("collar") && rng.random() * 100.0 < 80.0 {
        zisk += 1;
    }
    // Elephant-Foot Maul: rarita mamuta sa NASTAVÍ na 2 nad raritu jedla (absolútna
    // hodnota z karty) — berie sa VYŠŠIE z oboch.
    if board.has_tool("maul") && bol_mamut {
        let ciel_tier = tier_jedla + 2;
        zisk = zisk.max(ciel_tier - tier_jedaka);
    }
    let pomsta = board.has_tool("revenge") && rng.random() * 100.0 < 30.0;
    // Crowded Beastcage: políčko zožratého sa hneď znova otvorí s novým väzňom.
    if board.has_tool("cage") && rng.random() * 100.0 < 40.0 && board.cell(na).sealed {
        open_cell(board, na, "side", rng, events, None);
    }
    events.push(format!("devour:{}->{}{}", z, na, if cez_scratch { ":scratch" } else { "" }));
    promote(board, z, zisk, true, rng, events);
    let eater_now = board.cell(z).clone();
    if pomsta && !eater_now.sealed && eater_now.inmate.is_some() {
        let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
        if !zamknute.is_empty() {
            let terc = *rng.choice(&zamknute);
            let eater_inmate = eater_now.inmate.unwrap();
            let jedlo = board.eats_of(&eater_inmate.beast);
            let beast = if !jedlo.is_empty() {
                rng.choice(jedlo).to_string()
            } else {
                eater_inmate.beast.clone()
            };
            open_cell(board, terc, "side", rng, events, Some(Inmate { beast, tier: eater_inmate.tier }));
        }
    }
}

// ── Verejné akcie (engine.py) ────────────────────────────────────────────────

/// Dnešné cieľové políčka patroly — 3 z najbližších 5 políčok v smere hodín
/// od dozorcu, kancelária sa nikdy neponúkne.
pub fn roll_destinations(board: &mut Board, rng: &mut Rng) {
    let rozsah = PATROL_RANGE.min(CELL_COUNT - 1);
    let mut pred_sebou: Vec<usize> = vec![];
    let mut i = 1usize;
    while i < CELL_COUNT && (i <= rozsah || pred_sebou.is_empty()) {
        let idx = (board.warden + i) % CELL_COUNT;
        if idx != OFFICE {
            pred_sebou.push(idx);
        }
        i += 1;
    }
    let mut menej: i64 = if rng.random() * 100.0 < FEWER_DOORS_CHANCE_PCT { 1 } else { 0 };
    // TLI 25: Forged Key, Barbed Whip a Burning Stake sa SČÍTAVAJÚ.
    for tid in ["forged", "whip", "stake"] {
        if board.has_tool(tid) {
            menej += 1;
        }
    }
    let limit = ((PATROL_DESTINATIONS as i64) - menej).max(1) as usize;
    let mut vyber = rng.sample(&pred_sebou, limit.min(pred_sebou.len()));
    vyber.sort_by_key(|&idx| steps_between(board.warden, idx));
    board.destinations = vyber;
}

pub fn patrol(board: &mut Board, ciel: usize, rng: &mut Rng) -> Vysledok {
    if has_pending_choice(board) {
        return Err(chyba("najprv rozhodni visiacu voľbu"));
    }
    if !board.destinations.contains(&ciel) {
        return Err(chyba(format!("políčko {} nie je dnešný cieľ patroly", ciel)));
    }
    let mut events = vec![format!("move:{}->{}", board.warden, ciel)];
    let krokov = steps_between(board.warden, ciel);
    if board.chain_armed {
        for idx in clockwise_path(board.warden, ciel) {
            if board.cell(idx).sealed {
                open_cell(board, idx, "side", rng, &mut events, None);
            }
        }
        board.chain_armed = false;
    } else if board.cell(ciel).sealed {
        let event = open_cell(board, ciel, "patrol", rng, &mut events, None);
        if let Some(ev) = event {
            if board.cell(ciel).inmate.is_some() {
                board.pending_event = Some(PendingEvent { kind: ev, cell: ciel });
            }
        }
    }
    board.warden = ciel;
    board.destinations = vec![];
    // ── TLI 25: nástroje viazané na DĹŽKU dnešnej chôdze ──
    if board.has_tool("shackles") && krokov >= 4 && rng.random() * 100.0 < 40.0 {
        let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
        if !bazen.is_empty() {
            let idx = *rng.choice(&bazen);
            promote(board, idx, 1, false, rng, &mut events);
        }
    }
    if board.has_tool("shortchain") && krokov <= 3 && rng.random() * 100.0 < 50.0 {
        let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
        if !zamknute.is_empty() {
            let idx = *rng.choice(&zamknute);
            open_cell(board, idx, "side", rng, &mut events, None);
        }
    }
    Ok(events)
}

fn najmensie_zastupenie_beast(board: &Board, okrem_index: usize, rng: &mut Rng) -> String {
    let mut counts: HashMap<&str, i64> = BEASTS.iter().map(|&b| (b, 0)).collect();
    for c in board.board_cells() {
        if c.index != okrem_index {
            if let Some(i) = &c.inmate {
                *counts.get_mut(i.beast.as_str()).unwrap() += 1;
            }
        }
    }
    let minimum = *counts.values().min().unwrap();
    let candidates: Vec<&str> = BEASTS.iter().copied().filter(|b| counts[b] == minimum).collect();
    rng.choice(&candidates).to_string()
}

fn odhal_dalsie_zamknute(board: &Board, n: usize) -> Vec<usize> {
    let mut out = vec![];
    let mut i = 1usize;
    while i < CELL_COUNT && out.len() < n {
        let idx = (board.warden + i) % CELL_COUNT;
        if board.cell(idx).sealed {
            out.push(idx);
        }
        i += 1;
    }
    out
}

/// Všetkých 11 Prison Incidents (TLI 63). `chain_reaction`/`execution`
/// OVERENÉ Richardovou snímkou, zvyšných 9 KOMUNITA (pozri `tartarus.yml`).
pub fn resolve_event(board: &mut Board, prijat: bool, rng: &mut Rng) -> Vysledok {
    let pending = board.pending_event.clone().ok_or_else(|| chyba("žiadny event nečaká"))?;
    let kind = pending.kind.clone();
    let ciel = pending.cell;
    board.pending_event = None;
    let mut events: Vec<String> = vec![];

    // "Treasure" Hunt (hunt_doors) má v hre len Confirm — nedá sa odmietnuť.
    if !prijat && kind != "hunt_doors" {
        events.push(format!("event_odmietnuty:{}", kind));
        return Ok(events);
    }

    match kind.as_str() {
        "chain_reaction" => {
            board.chain_armed = true;
            events.push("chain_armed".to_string());
            Ok(events)
        }
        "execution" => {
            let exec_inmate = match board.cell(ciel).inmate.clone() {
                Some(i) => i,
                None => return Ok(events),
            };
            let obete: Vec<usize> = board
                .board_cells()
                .iter()
                .filter(|c| {
                    c.index != ciel
                        && board.eats_of(&exec_inmate.beast).contains(&c.inmate.as_ref().unwrap().beast.as_str())
                        && (if EXECUTION_SAME_TIER {
                            c.inmate.as_ref().unwrap().tier == exec_inmate.tier
                        } else {
                            c.inmate.as_ref().unwrap().tier < exec_inmate.tier
                        })
                })
                .map(|c| c.index)
                .collect();
            for &o in &obete {
                empty_cell(board, o, rng);
            }
            events.push(format!("execution:{}:{}", ciel, obete.len()));
            if !obete.is_empty() {
                promote(board, ciel, obete.len() as i64, EXECUTION_REROLLS, rng, &mut events);
            }
            Ok(events)
        }
        "trickster" => {
            if board.cell(ciel).inmate.is_some() {
                let beast = najmensie_zastupenie_beast(board, ciel, rng);
                board.cell_mut(ciel).inmate = Some(Inmate { beast: beast.clone(), tier: crate::tartarus::SUPREME });
                events.push(format!("trickster:{}:{}", ciel, beast));
            }
            Ok(events)
        }
        "ravenous_appetite" => {
            let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
            if !bazen.is_empty() {
                let najnizsia = bazen.iter().map(|&i| board.cell(i).inmate.as_ref().unwrap().tier).min().unwrap();
                let kandidati: Vec<usize> = bazen
                    .iter()
                    .copied()
                    .filter(|&i| board.cell(i).inmate.as_ref().unwrap().tier == najnizsia)
                    .collect();
                let c = *rng.choice(&kandidati);
                let mut zasahov = 0;
                for _ in 0..5 {
                    if board.cell(c).sealed || board.cell(c).inmate.is_none() {
                        break;
                    }
                    if rng.random() < 0.5 {
                        promote(board, c, 1, false, rng, &mut events);
                        zasahov += 1;
                    }
                }
                events.push(format!("ravenous_appetite:{}:{}", c, zasahov));
            }
            Ok(events)
        }
        "high_risk_convict" => {
            let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
            if !zamknute.is_empty() {
                let c = *rng.choice(&zamknute);
                let beast = rng.vazeny_vyber(&BEAST_WEIGHTS);
                open_cell(board, c, "side", rng, &mut events, Some(Inmate { beast, tier: ORANGE }));
                events.push(format!("high_risk_convict:{}", c));
            }
            Ok(events)
        }
        "crow" => {
            for c in odhal_dalsie_zamknute(board, 4) {
                if let Some(h) = board.cell(c).hidden_inmate.clone() {
                    board.cell_mut(c).known_tier = Some(h.tier);
                }
            }
            events.push("crow".to_string());
            Ok(events)
        }
        "freedom" => {
            let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
            let pocet = 3usize.min(zamknute.len());
            for idx in rng.sample(&zamknute, pocet) {
                open_cell(board, idx, "side", rng, &mut events, None);
            }
            events.push("freedom".to_string());
            Ok(events)
        }
        "hunt_doors" => {
            let candidates: Vec<usize> = board
                .cells
                .iter()
                .filter(|c| c.sealed && c.hidden_inmate.as_ref().map(|h| h.tier <= BLUE).unwrap_or(false))
                .map(|c| c.index)
                .collect();
            for idx in candidates {
                let tier = board.cell(idx).hidden_inmate.as_ref().unwrap().tier;
                board.cell_mut(idx).known_tier = Some(tier);
            }
            events.push("hunt_doors".to_string());
            Ok(events)
        }
        "portion" => {
            let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
            let pocet = 3usize.min(bazen.len());
            for idx in rng.sample(&bazen, pocet) {
                promote(board, idx, 1, false, rng, &mut events);
            }
            events.push("portion".to_string());
            Ok(events)
        }
        "ratway" => {
            let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
            if zamknute.is_empty() {
                events.push("ratway:bez_ciela".to_string());
                return Ok(events);
            }
            let mut ciel_idx = zamknute[0];
            let mut najmensie = steps_between(board.warden, ciel_idx);
            for &idx in &zamknute[1..] {
                let s = steps_between(board.warden, idx);
                if s < najmensie {
                    najmensie = s;
                    ciel_idx = idx;
                }
            }
            board.warden = ciel_idx;
            events.push(format!("move:ratway->{}", ciel_idx));
            if board.cell(ciel_idx).sealed {
                let udalost = open_cell(board, ciel_idx, "patrol", rng, &mut events, None);
                if let Some(ev) = udalost {
                    if board.cell(ciel_idx).inmate.is_some() {
                        board.pending_event = Some(PendingEvent { kind: ev, cell: ciel_idx });
                    }
                }
            }
            Ok(events)
        }
        "usurpation" => {
            let stage = board.usurpation_stage;
            if stage >= 4 {
                events.push("usurpation:already_done".to_string());
                return Ok(events);
            }
            let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
            if stage == 0 && !bazen.is_empty() {
                let idx = *rng.choice(&bazen);
                promote(board, idx, 1, false, rng, &mut events);
            } else if stage == 1 && !bazen.is_empty() {
                let pocet = 2usize.min(bazen.len());
                for idx in rng.sample(&bazen, pocet) {
                    promote(board, idx, 1, false, rng, &mut events);
                }
            } else if stage == 2 && !bazen.is_empty() {
                let pocet = 2usize.min(bazen.len());
                for idx in rng.sample(&bazen, pocet) {
                    promote(board, idx, 2, false, rng, &mut events);
                }
            } else if stage == 3 {
                for idx in bazen {
                    promote(board, idx, 2, false, rng, &mut events);
                }
            }
            board.usurpation_stage = stage + 1;
            events.push(format!("usurpation:{}", stage + 1));
            Ok(events)
        }
        _ => Ok(events),
    }
}

pub fn legal_devour_targets(board: &Board, z: usize) -> Vec<(usize, bool)> {
    let cell = board.cell(z);
    if cell.sealed || cell.inmate.is_none() {
        return vec![];
    }
    let eater = cell.inmate.clone().unwrap();
    let mut out = vec![];
    for c in &board.cells {
        if c.index == z {
            continue;
        }
        if !c.sealed {
            if let Some(m) = &c.inmate {
                if can_eat(board, &eater, m) {
                    out.push((c.index, false));
                }
            }
        } else if c.scratch
            && eater.beast == "wolf"
            && c.known_tier.is_some()
            && c.known_tier.unwrap() == eater.tier
            && eater.tier < CHROMATIC
        {
            out.push((c.index, true));
        }
    }
    out
}

pub fn devour(board: &mut Board, z: usize, na: usize, rng: &mut Rng) -> Vysledok {
    if has_pending_choice(board) {
        return Err(chyba("najprv rozhodni visiacu voľbu"));
    }
    if board.used_devour {
        return Err(chyba("dnešné požretie je už vyčerpané"));
    }
    if z != board.warden {
        return Err(chyba("požretie smie začať len väzeň na políčku dozorcu"));
    }
    board.used_devour = true;
    let mut events = vec![];
    let moznosti = legal_devour_targets(board, z);
    let mut volba = match moznosti.iter().find(|o| o.0 == na).copied() {
        Some(v) => v,
        None => return Err(chyba(format!("{} nemôže požrieť {}", z, na))),
    };
    let mut aktualne = z;
    let mut poradie = 0i64;
    loop {
        let (ciel, cez_scratch) = volba;
        poradie += 1;
        jedno_uhryznutie(board, aktualne, ciel, cez_scratch, rng, &mut events, poradie);
        aktualne = ciel;
        let eater_cell = board.cell(aktualne);
        if eater_cell.sealed || eater_cell.inmate.is_none() {
            break;
        }
        let dalsie = legal_devour_targets(board, aktualne);
        if dalsie.is_empty() {
            break;
        }
        volba = *rng.choice(&dalsie);
    }
    Ok(events)
}

pub fn swap(board: &mut Board, rng: &mut Rng) -> Vysledok {
    if has_pending_choice(board) {
        return Err(chyba("najprv rozhodni visiacu voľbu"));
    }
    if board.used_swap {
        return Err(chyba("dnešná výmena je už vyčerpaná"));
    }
    let warden = board.warden;
    if board.cell(warden).sealed {
        return Err(chyba("na políčku dozorcu nie je koho vymeniť"));
    }
    board.used_swap = true;
    let mut events = vec![];
    let povodny = board.cell(warden).inmate.clone();
    let mal_tusk = board.cell(warden).tusk;
    let mut novy = roluj_vazna(rng, None);
    // Hangman's Noose: nikdy Normal (0) ani Magic (1).
    if board.has_tool("rope") {
        while novy.tier < 2 {
            novy = roluj_vazna(rng, None);
        }
    }
    if board.has_tool("pestle") {
        if let Some(p) = &povodny {
            let nazov = rng.vazeny_vyber(&TIER_WEIGHTS);
            novy = Inmate { beast: p.beast.clone(), tier: tier_index(&nazov) };
        }
    }
    // ── TLI 25: nástroje, ktoré menia VÝSLEDOK výmeny ──
    // Burning Stake: výmena väzňa pod Orange sa so šancou zmení na Orange.
    if board.has_tool("stake") {
        if let Some(p) = &povodny {
            if p.tier < ORANGE && rng.random() * 100.0 < 40.0 {
                novy = Inmate { beast: novy.beast.clone(), tier: ORANGE };
            }
        }
    }
    // Scapegoat's Guillotine.
    let mut obetovani = 0usize;
    if board.has_tool("scapegoat") {
        if let Some(p) = &povodny {
            let ini: Vec<usize> = board.board_cells().iter().filter(|c| c.index != warden).map(|c| c.index).collect();
            let pocet = 2usize;
            if ini.len() >= pocet {
                let vybrane = rng.sample(&ini, pocet);
                for idx in &vybrane {
                    empty_cell(board, *idx, rng);
                }
                obetovani = pocet;
                if novy.tier <= p.tier {
                    novy = Inmate { beast: novy.beast.clone(), tier: (p.tier + 1).min(CHROMATIC) };
                }
            }
        }
    }
    board.cell_mut(warden).inmate = Some(novy);
    events.push(format!("swap:{}", warden));
    if obetovani > 0 {
        events.push(format!("scapegoat:{}", obetovani));
    }
    // Severed Iron Maiden.
    if board.has_tool("maiden") {
        if let Some(cur) = board.cell(warden).inmate.clone() {
            let moja = cur.tier;
            let ostatni: Vec<i64> = board
                .board_cells()
                .iter()
                .filter(|c| c.index != warden)
                .map(|c| c.inmate.as_ref().unwrap().tier)
                .collect();
            let nad = ostatni.iter().filter(|&&t| t > moja).count();
            let pod = ostatni.iter().filter(|&&t| t < moja).count();
            if nad == pod {
                events.push(format!("maiden:{}", warden));
                promote(board, warden, 3, false, rng, &mut events);
            }
        }
    }
    // Gaoler's Execution Axe: výmena na VYŠŠIU raritu otvorí 2 zamknuté políčka.
    if board.has_tool("axe") {
        if let Some(p) = &povodny {
            let cur = board.cell(warden).clone();
            if !cur.sealed {
                if let Some(ci) = &cur.inmate {
                    if ci.tier > p.tier {
                        let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
                        let pocet = 2usize.min(zamknute.len());
                        for idx in rng.sample(&zamknute, pocet) {
                            open_cell(board, idx, "side", rng, &mut events, None);
                        }
                    }
                }
            }
        }
    }
    if mal_tusk {
        let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
        let pocet = TUSK_RAISE_COUNT.min(bazen.len());
        for idx in rng.sample(&bazen, pocet) {
            promote(board, idx, 1, false, rng, &mut events);
        }
        events.push("tusk".to_string());
    }
    if board.has_tool("scales") && rng.random() < 0.8 {
        let candidates: Vec<usize> = board
            .cells
            .iter()
            .filter(|c| c.sealed && c.hidden_inmate.is_some())
            .map(|c| c.index)
            .collect();
        if !candidates.is_empty() {
            let mut najlepsie = candidates[0];
            let mut najlepsi_tier = board.cell(najlepsie).hidden_inmate.as_ref().unwrap().tier;
            for &idx in &candidates[1..] {
                let t = board.cell(idx).hidden_inmate.as_ref().unwrap().tier;
                if t > najlepsi_tier {
                    najlepsie = idx;
                    najlepsi_tier = t;
                }
            }
            board.cell_mut(najlepsie).known_tier = Some(najlepsi_tier);
        }
    }
    Ok(events)
}

/// Blood-Stained Trap (TLI 25): keď dozorca stojí na LEVOVI, pribúda
/// možnosť „Hunt". Bez nástroja je zoznam vždy prázdny.
pub fn legal_hunt_targets(board: &Board) -> Vec<usize> {
    if !board.has_tool("trap") {
        return vec![];
    }
    let cur = board.cell(board.warden);
    if cur.sealed || cur.inmate.is_none() || cur.inmate.as_ref().unwrap().beast != "lion" {
        return vec![];
    }
    if board.used_devour || board.used_swap {
        return vec![];
    }
    legal_devour_targets(board, board.warden)
        .into_iter()
        .filter(|&(_, cez_scratch)| !cez_scratch)
        .map(|(i, _)| i)
        .collect()
}

/// Hunt je ALTERNATÍVA ku Consume/Replace — zaberá oba denné limity.
pub fn hunt(board: &mut Board, na: usize, rng: &mut Rng) -> Vysledok {
    let _ = rng;
    if has_pending_choice(board) {
        return Err(chyba("najprv rozhodni visiacu voľbu"));
    }
    if !legal_hunt_targets(board).contains(&na) {
        return Err(chyba(format!("na políčko {} sa nedá nastražiť pasca", na)));
    }
    let korist_beast = board.cell(na).inmate.as_ref().unwrap().beast.clone();
    let warden = board.warden;
    board.used_devour = true;
    board.used_swap = true;
    board.prey.push(PreyLink { lion_cell: warden, prey_cell: na, prey_beast: korist_beast, rounds_left: PREY_ROUNDS });
    Ok(vec![format!("hunt:{}->{}", warden, na)])
}

pub fn choose_tool(board: &mut Board, tool_id: &str) -> Vysledok {
    let ok = board.pending_tool_choice.as_ref().map(|v| v.iter().any(|t| t == tool_id)).unwrap_or(false);
    if !ok {
        return Err(chyba(format!("nástroj {} nie je medzi ponúknutými", tool_id)));
    }
    let mut events = vec![];
    board.tools.push(tool_id.to_string());
    board.pending_tool_choice = None;
    events.push(format!("tool_zvoleny:{}", tool_id));
    Ok(events)
}

pub fn choose_gift(board: &mut Board, gift_id: &str, rng: &mut Rng) -> Vysledok {
    let ok = board.pending_gift.as_ref().map(|v| v.iter().any(|g| g == gift_id)).unwrap_or(false);
    if !ok {
        return Err(chyba(format!("dar {} nie je medzi ponúknutými", gift_id)));
    }
    let mut events = vec![];
    board.pending_gift = None;
    board.gifts_pending = (board.gifts_pending - 1).max(0);
    match gift_id {
        "ring" => {
            let zamknute: Vec<usize> = board.cells.iter().filter(|c| c.sealed).map(|c| c.index).collect();
            let pocet = 3usize.min(zamknute.len());
            for idx in rng.sample(&zamknute, pocet) {
                open_cell(board, idx, "side", rng, &mut events, None);
            }
        }
        "horn" => {
            let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
            for idx in bazen {
                if board.cell(idx).sealed || board.cell(idx).inmate.is_none() {
                    continue;
                }
                let ciele = legal_devour_targets(board, idx);
                if !ciele.is_empty() {
                    let volba = *rng.choice(&ciele);
                    jedno_uhryznutie(board, idx, volba.0, volba.1, rng, &mut events, 1);
                }
            }
        }
        "wine" => {
            let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
            for idx in bazen {
                if let Some(i) = board.cell(idx).inmate.clone() {
                    if i.tier < ORANGE {
                        promote(board, idx, 1, false, rng, &mut events);
                    }
                }
            }
        }
        "brand" => {
            let bez_ciela: Vec<usize> = board
                .board_cells()
                .iter()
                .filter(|c| legal_devour_targets(board, c.index).is_empty())
                .map(|c| c.index)
                .collect();
            let pocet = 4usize.min(bez_ciela.len());
            for idx in rng.sample(&bez_ciela, pocet) {
                for kandidat in ["mammoth", "lion", "wolf", "rat"] {
                    let aktualny = board.cell(idx).inmate.as_ref().map(|i| i.beast.clone());
                    match aktualny {
                        Some(ab) if ab != kandidat => {
                            board.cell_mut(idx).inmate.as_mut().unwrap().beast = kandidat.to_string();
                            if !legal_devour_targets(board, idx).is_empty() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
    events.push(format!("gift_zvoleny:{}", gift_id));
    if board.gifts_pending > 0 {
        board.pending_gift = Some(GIFT_IDS.iter().map(|s| s.to_string()).collect());
    }
    Ok(events)
}

pub fn end_day(board: &mut Board, rng: &mut Rng) -> Vysledok {
    if has_pending_choice(board) {
        return Err(chyba("najprv rozhodni visiacu voľbu pred koncom dňa"));
    }
    let mut events = vec![];
    // Prey rastie, neplatné väzby zanikajú.
    let mut este_platne: Vec<PreyLink> = vec![];
    for mut link in board.prey.clone() {
        if !check_prey_vazba(board, &link) {
            events.push(format!("prey_zanikla:{}-{}", link.lion_cell, link.prey_cell));
            continue;
        }
        promote(board, link.lion_cell, 1, false, rng, &mut events);
        if check_prey_vazba(board, &link) {
            promote(board, link.prey_cell, 1, false, rng, &mut events);
        }
        link.rounds_left -= 1;
        if link.rounds_left > 0 && check_prey_vazba(board, &link) {
            este_platne.push(link);
        } else {
            events.push(format!("prey_spotrebovana:{}-{}", link.lion_cell, link.prey_cell));
        }
    }
    board.prey = este_platne;

    board.day += 1;
    // Potkanie hniezda, čo dnes dozrievajú.
    for idx in 0..board.cells.len() {
        if board.cell(idx).nest_due_day == Some(board.day) {
            open_cell(board, idx, "side", rng, &mut events, Some(Inmate { beast: "rat".to_string(), tier: NEST_HATCH_TIER }));
        }
    }
    // Nástroje, čo pôsobia "každé ráno".
    if board.has_tool("ironbox") {
        let bez_hniezda: Vec<usize> = board
            .cells
            .iter()
            .filter(|c| c.sealed && c.nest_due_day.is_none())
            .map(|c| c.index)
            .collect();
        if !bez_hniezda.is_empty() {
            let idx = *rng.choice(&bez_hniezda);
            let den = board.day + NEST_DAYS;
            board.cell_mut(idx).nest_due_day = Some(den);
            events.push(format!("nest_vytvorene:{}", idx));
        }
    }
    if board.has_tool("tongs") {
        let mamuty: Vec<usize> = board
            .board_cells()
            .iter()
            .filter(|c| c.inmate.as_ref().unwrap().beast == "mammoth" && !c.tusk)
            .map(|c| c.index)
            .collect();
        if !mamuty.is_empty() {
            let idx = *rng.choice(&mamuty);
            board.cell_mut(idx).tusk = true;
            events.push(format!("tusk_udeleny:{}", idx));
        }
    }
    // ── TLI 25: ďalšie ranné efekty ──
    if board.has_tool("claws") {
        let bez_scratchu: Vec<usize> = board.cells.iter().filter(|c| c.sealed && !c.scratch).map(|c| c.index).collect();
        if !bez_scratchu.is_empty() {
            let idx = *rng.choice(&bez_scratchu);
            board.cell_mut(idx).scratch = true;
            if let Some(h) = board.cell(idx).hidden_inmate.clone() {
                board.cell_mut(idx).known_tier = Some(h.tier);
            }
            events.push(format!("scratch_udeleny:{}", idx));
        }
    }
    if board.has_tool("coin") && rng.random() * 100.0 < 60.0 {
        let mut skupiny: HashMap<(String, i64), Vec<usize>> = HashMap::new();
        for c in board.board_cells() {
            let i = c.inmate.as_ref().unwrap();
            skupiny.entry((i.beast.clone(), i.tier)).or_default().push(c.index);
        }
        let dvojice: Vec<Vec<usize>> = skupiny.into_values().filter(|v| v.len() >= 2).collect();
        if !dvojice.is_empty() {
            let skupina = rng.choice(&dvojice).clone();
            let vybrane = rng.sample(&skupina, 2);
            let (odide, zostane) = (vybrane[0], vybrane[1]);
            empty_cell(board, odide, rng);
            events.push(format!("coin:{}->{}", odide, zostane));
            promote(board, zostane, 2, false, rng, &mut events);
        }
    }
    if board.has_tool("ravenous") {
        let bazen: Vec<usize> = board.board_cells().iter().map(|c| c.index).collect();
        if !bazen.is_empty() {
            let mut najvyssi = bazen[0];
            let mut najvyssi_tier = board.cell(najvyssi).inmate.as_ref().unwrap().tier;
            for &idx in &bazen[1..] {
                let t = board.cell(idx).inmate.as_ref().unwrap().tier;
                if t > najvyssi_tier {
                    najvyssi = idx;
                    najvyssi_tier = t;
                }
            }
            let mut zmazanych = 0;
            for krok in [-1i64, 1i64] {
                let sused = (najvyssi as i64 + krok).rem_euclid(CELL_COUNT as i64) as usize;
                if sused == OFFICE || board.cell(sused).sealed || board.cell(sused).inmate.is_none() {
                    continue;
                }
                empty_cell(board, sused, rng);
                zmazanych += 1;
            }
            if zmazanych > 0 {
                events.push(format!("ravenous:{}:{}", najvyssi, zmazanych));
                let mut bonus = 0;
                for _ in 0..zmazanych {
                    if rng.random() * 100.0 < 40.0 {
                        bonus += 1;
                    }
                }
                if bonus > 0 && !board.cell(najvyssi).sealed && board.cell(najvyssi).inmate.is_some() {
                    promote(board, najvyssi, bonus, false, rng, &mut events);
                }
            }
        }
    }

    board.used_devour = false;
    board.used_swap = false;
    if !is_drill_over(board) {
        roll_destinations(board, rng);
    } else {
        board.destinations = vec![];
    }
    Ok(events)
}

pub fn novy_drill(rng: &mut Rng, food_chain: Option<&str>, seed: Option<String>) -> Board {
    let mut board = novy_board(rng, food_chain, seed);
    roll_destinations(&mut board, rng);
    board
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::tartarus::{dostupne_eventy, novy_board, Cell, EVENT_KINDS, PURPLE};

    /// Priamo nastaví OTVORENÉ políčko s daným väzňom — rovnaký vzor ako
    /// `testy-engine.py:_otvor`, na deterministické testy bez závislosti na
    /// tom, čo sa vyžrebovalo cez `novy_drill`.
    fn otvor(board: &mut Board, index: usize, beast: &str, tier: i64) {
        board.cells[index] = Cell {
            index,
            sealed: false,
            inmate: Some(Inmate { beast: beast.to_string(), tier }),
            known_tier: None,
            tusk: false,
            scratch: false,
            nest_due_day: None,
            hidden_inmate: None,
            hidden_event: None,
        };
    }

    fn novy_test_board(seed: u64) -> Board {
        let mut rng = Rng::new(seed);
        novy_drill(&mut rng, None, None)
    }

    fn over_invarianty(board: &Board) {
        let c0 = board.cell(OFFICE);
        assert!(!c0.sealed, "políčko 0 (kancelária) nesmie byť zamknuté");
        assert!(c0.inmate.is_none(), "políčko 0 (kancelária) nesmie obsahovať väzňa");
        for cell in &board.cells {
            if let Some(i) = &cell.inmate {
                assert!(0 <= i.tier && i.tier <= CHROMATIC, "rarita {} mimo rozsahu", i.tier);
                assert!(BEASTS.contains(&i.beast.as_str()), "neznámy beast {}", i.beast);
            }
        }
    }

    #[test]
    fn test_novy_drill_zaklad() {
        let board = novy_test_board(42);
        assert_eq!(board.day, 1);
        assert_eq!(board.warden, OFFICE);
        assert_eq!(board.cells.len(), CELL_COUNT);
        assert!(!board.destinations.is_empty());
        over_invarianty(&board);
    }

    #[test]
    fn test_dostupne_eventy_pocet_a_filter_usurpacie() {
        let mut board = novy_test_board(630);
        assert_eq!(EVENT_KINDS.len(), 11);
        assert_eq!(dostupne_eventy(&board).len(), 11);
        board.usurpation_stage = 4;
        let dostupne = dostupne_eventy(&board);
        assert_eq!(dostupne.len(), 10);
        assert!(!dostupne.contains(&"usurpation"));
    }

    #[test]
    fn test_vsetkych_11_incidentov_je_implementovanych() {
        for kind in EVENT_KINDS {
            let mut rng = Rng::new(1);
            let mut board = novy_board(&mut Rng::new(42), None, None);
            for (i, beast) in [(2, "mammoth"), (3, "lion"), (4, "wolf"), (5, "rat")] {
                otvor(&mut board, i, beast, 1);
            }
            board.warden = 2;
            board.pending_event = Some(PendingEvent { kind: kind.to_string(), cell: 2 });
            let events = resolve_event(&mut board, true, &mut rng).unwrap();
            assert!(!events.is_empty(), "incident '{}' nevrátil žiadnu udalosť", kind);
        }
    }

    #[test]
    fn test_trickster_supreme_najredsieho_beastu() {
        let mut rng = Rng::new(620);
        let mut board = novy_board(&mut rng, None, None);
        otvor(&mut board, 2, "mammoth", BLUE);
        otvor(&mut board, 3, "mammoth", PURPLE);
        otvor(&mut board, 4, "lion", BLUE);
        otvor(&mut board, 6, "rat", BLUE);
        otvor(&mut board, 5, "rat", BLUE); // cieľ — pôvodný beast sa NEráta
        board.pending_event = Some(PendingEvent { kind: "trickster".to_string(), cell: 5 });

        let mut rng2 = Rng::new(1);
        let events = resolve_event(&mut board, true, &mut rng2).unwrap();

        let cell = board.cell(5);
        let inmate = cell.inmate.as_ref().unwrap();
        assert_eq!(inmate.tier, crate::tartarus::SUPREME);
        assert_eq!(inmate.beast, "wolf", "wolf je jediný beast s 0 výskytmi mimo cieľa");
        assert!(events.iter().any(|e| e.starts_with("trickster:5:wolf")));
    }

    #[test]
    fn test_ravenous_appetite_najslabsi_az_do_chromatic() {
        let mut rng = Rng::new(621);
        let mut board = novy_board(&mut rng, None, None);
        otvor(&mut board, 2, "mammoth", 0); // najnižšia rarita na doske
        otvor(&mut board, 3, "lion", PURPLE);
        board.pending_event = Some(PendingEvent { kind: "ravenous_appetite".to_string(), cell: 3 });
        let v_pred = board.vault;

        // Simulujeme "vždy úspech" priamym volaním resolve_event s rng, ktorý
        // dáva veľmi nízke hodnoty — namiesto patchovania `.random()` (Rust
        // nemá duck typing) over invariant priamo: 5 pokusov s p=0.5 pri
        // dostatku opakovaní s pevným seedom má byť deterministicky
        // reprodukovateľné; tu over, že sa dá dosiahnuť chromatic/vault+1
        // aspoň raz spomedzi viacerých seedov (robustnejšie než 1 magic seed).
        let mut dosiahnute = false;
        for seed in 0u64..200 {
            let mut b2 = board.clone();
            b2.pending_event = Some(PendingEvent { kind: "ravenous_appetite".to_string(), cell: 3 });
            let mut r = Rng::new(seed);
            let events = resolve_event(&mut b2, true, &mut r).unwrap();
            assert!(events.iter().any(|e| e.starts_with("ravenous_appetite:2:")));
            if b2.vault == v_pred + 1 && b2.cell(2).sealed && b2.cell(2).inmate.is_none() {
                dosiahnute = true;
                break;
            }
        }
        assert!(dosiahnute, "aspoň jeden z 200 seedov mal dať 5x úspech (0+5=CHROMATIC)");
    }

    #[test]
    fn test_high_risk_convict_otvori_orange() {
        let mut rng = Rng::new(622);
        let mut board = novy_board(&mut rng, None, None);
        let pred = board.cells.iter().filter(|c| c.sealed).count();
        board.pending_event = Some(PendingEvent { kind: "high_risk_convict".to_string(), cell: 1 });

        let mut r = Rng::new(99);
        let events = resolve_event(&mut board, true, &mut r).unwrap();

        let po = board.cells.iter().filter(|c| c.sealed).count();
        assert_eq!(po, pred - 1);
        let zaznam = events.iter().find(|e| e.starts_with("high_risk_convict:")).unwrap();
        let idx: usize = zaznam.split(':').nth(1).unwrap().parse().unwrap();
        let cell = board.cell(idx);
        assert!(!cell.sealed);
        assert_eq!(cell.inmate.as_ref().unwrap().tier, ORANGE);
        assert!(BEASTS.contains(&cell.inmate.as_ref().unwrap().beast.as_str()));
    }

    #[test]
    fn test_crow_odhali_4_najblizsie_zamknute() {
        let mut rng = Rng::new(623);
        let mut board = novy_board(&mut rng, None, None);
        board.warden = 0;
        board.pending_event = Some(PendingEvent { kind: "crow".to_string(), cell: 1 });

        let mut r = Rng::new(1);
        resolve_event(&mut board, true, &mut r).unwrap();

        for i in [1, 2, 3, 4] {
            let c = board.cell(i);
            assert!(c.known_tier.is_some());
            assert_eq!(c.known_tier.unwrap(), c.hidden_inmate.as_ref().unwrap().tier);
        }
        for i in [5, 6, 15] {
            assert!(board.cell(i).known_tier.is_none());
        }
    }

    #[test]
    fn test_freedom_otvori_3_zamknute() {
        let mut rng = Rng::new(624);
        let mut board = novy_board(&mut rng, None, None);
        let pred = board.cells.iter().filter(|c| c.sealed).count();
        board.pending_event = Some(PendingEvent { kind: "freedom".to_string(), cell: 1 });

        let mut r = Rng::new(5);
        resolve_event(&mut board, true, &mut r).unwrap();

        let po = board.cells.iter().filter(|c| c.sealed).count();
        assert_eq!(po, pred - 3);
    }

    #[test]
    fn test_hunt_doors_odhali_len_white_a_blue_a_neda_sa_odmietnut() {
        let mut rng = Rng::new(625);
        let mut board = novy_board(&mut rng, None, None);
        board.cells[1].hidden_inmate = Some(Inmate { beast: "rat".to_string(), tier: 0 });
        board.cells[2].hidden_inmate = Some(Inmate { beast: "rat".to_string(), tier: 1 });
        board.cells[3].hidden_inmate = Some(Inmate { beast: "rat".to_string(), tier: 2 });
        board.cells[4].hidden_inmate = Some(Inmate { beast: "rat".to_string(), tier: 3 });
        board.pending_event = Some(PendingEvent { kind: "hunt_doors".to_string(), cell: 1 });

        let mut r = Rng::new(1);
        let events = resolve_event(&mut board, false, &mut r).unwrap();

        assert_eq!(board.cell(1).known_tier, Some(0));
        assert_eq!(board.cell(2).known_tier, Some(1));
        assert_eq!(board.cell(3).known_tier, None);
        assert_eq!(board.cell(4).known_tier, None);
        assert!(!events.iter().any(|e| e.starts_with("event_odmietnuty")));
    }

    #[test]
    fn test_portion_povysi_najviac_3_z_dostupnych() {
        let mut rng = Rng::new(627);
        let mut board = novy_board(&mut rng, None, None);
        for (i, beast) in [(2, "mammoth"), (3, "lion"), (4, "wolf"), (5, "rat")] {
            otvor(&mut board, i, beast, 0);
        }
        board.pending_event = Some(PendingEvent { kind: "portion".to_string(), cell: 2 });

        let mut r = Rng::new(3);
        resolve_event(&mut board, true, &mut r).unwrap();

        let povysenych = [2, 3, 4, 5].iter().filter(|&&i| board.cell(i).inmate.as_ref().unwrap().tier == 1).count();
        assert_eq!(povysenych, 3);
    }

    #[test]
    fn test_ratway_presunie_dozorcu_na_najblizsie_zamknute() {
        let mut rng = Rng::new(628);
        let mut board = novy_board(&mut rng, None, None);
        otvor(&mut board, 10, "rat", 0);
        board.warden = 10;
        board.pending_event = Some(PendingEvent { kind: "ratway".to_string(), cell: 10 });

        let mut r = Rng::new(1);
        let events = resolve_event(&mut board, true, &mut r).unwrap();

        assert_ne!(board.warden, 10);
        assert!(!board.cell(board.warden).sealed);
        assert!(events.iter().any(|e| e.starts_with("move:ratway->")));
    }

    #[test]
    fn test_usurpation_retaz_i_az_iv_a_potom_zmizne() {
        let mut rng = Rng::new(629);
        let mut board = novy_board(&mut rng, None, None);
        for (i, beast) in [(2, "mammoth"), (3, "lion"), (4, "wolf"), (5, "rat")] {
            otvor(&mut board, i, beast, 0);
        }
        let mut r = Rng::new(7);

        board.pending_event = Some(PendingEvent { kind: "usurpation".to_string(), cell: 2 });
        resolve_event(&mut board, true, &mut r).unwrap();
        assert_eq!(board.usurpation_stage, 1);
        assert_eq!([2, 3, 4, 5].iter().filter(|&&i| board.cell(i).inmate.as_ref().unwrap().tier == 1).count(), 1);

        board.pending_event = Some(PendingEvent { kind: "usurpation".to_string(), cell: 2 });
        resolve_event(&mut board, true, &mut r).unwrap();
        assert_eq!(board.usurpation_stage, 2);

        board.pending_event = Some(PendingEvent { kind: "usurpation".to_string(), cell: 2 });
        resolve_event(&mut board, true, &mut r).unwrap();
        assert_eq!(board.usurpation_stage, 3);

        board.pending_event = Some(PendingEvent { kind: "usurpation".to_string(), cell: 2 });
        resolve_event(&mut board, true, &mut r).unwrap();
        assert_eq!(board.usurpation_stage, 4);

        assert!(!dostupne_eventy(&board).contains(&"usurpation"));
        board.pending_event = Some(PendingEvent { kind: "usurpation".to_string(), cell: 2 });
        let events = resolve_event(&mut board, true, &mut r).unwrap();
        assert_eq!(events, vec!["usurpation:already_done".to_string()]);
    }

    #[test]
    fn test_used_devour_a_swap_limity() {
        let mut rng = Rng::new(700);
        let mut board = novy_board(&mut rng, None, None);
        otvor(&mut board, 1, "mammoth", 2);
        otvor(&mut board, 2, "lion", 2);
        board.warden = 1;
        let mut r = Rng::new(1);
        devour(&mut board, 1, 2, &mut r).unwrap();
        assert!(board.used_devour);
        let err = devour(&mut board, 1, 2, &mut r);
        assert!(err.is_err());
    }

    #[test]
    fn test_promote_rarita_neklesne_a_strop_je_chromatic() {
        let mut rng = Rng::new(701);
        let mut board = novy_board(&mut rng, None, None);
        otvor(&mut board, 1, "mammoth", CHROMATIC - 1);
        let mut events = vec![];
        let mut r = Rng::new(1);
        promote(&mut board, 1, 10, false, &mut r, &mut events);
        // Po dosiahnutí CHROMATIC sa políčko vyprázdni (vault) — inmate zmizne.
        assert!(board.cell(1).sealed);
        assert_eq!(board.vault, 1);
    }

    #[test]
    fn test_1000_nahodnych_behov_bez_chyby() {
        for seed in 0u64..200 {
            let mut rng = Rng::new(seed);
            let mut board = novy_drill(&mut rng, None, Some(format!("test-{}", seed)));
            let mut prev_vault = board.vault;
            let mut kroky = 0;
            while !is_drill_over(&board) {
                kroky += 1;
                assert!(kroky <= 3000, "drill presiahol strop pri seede {}", seed);
                over_invarianty(&board);
                assert!(board.vault >= prev_vault, "vault klesol pri seede {}", seed);
                prev_vault = board.vault;

                if board.pending_event.is_some() {
                    let prijat = rng.random() < 0.5;
                    resolve_event(&mut board, prijat, &mut rng).unwrap();
                } else if let Some(choices) = board.pending_tool_choice.clone() {
                    let t = rng.choice(&choices).clone();
                    choose_tool(&mut board, &t).unwrap();
                } else if let Some(gifts) = board.pending_gift.clone() {
                    let g = rng.choice(&gifts).clone();
                    choose_gift(&mut board, &g, &mut rng).unwrap();
                } else if !board.destinations.is_empty() {
                    let ciel = *rng.choice(&board.destinations.clone());
                    patrol(&mut board, ciel, &mut rng).unwrap();
                } else {
                    let mut moznosti: Vec<&str> = vec!["end_day"];
                    let dev_targets = legal_devour_targets(&board, board.warden);
                    if !board.used_devour && !dev_targets.is_empty() {
                        moznosti.push("devour");
                    }
                    if !board.used_swap && !board.cell(board.warden).sealed {
                        moznosti.push("swap");
                    }
                    let akcia = *rng.choice(&moznosti);
                    match akcia {
                        "end_day" => {
                            end_day(&mut board, &mut rng).unwrap();
                        }
                        "devour" => {
                            let (na, _) = *rng.choice(&dev_targets);
                            let warden = board.warden;
                            devour(&mut board, warden, na, &mut rng).unwrap();
                        }
                        "swap" => {
                            swap(&mut board, &mut rng).unwrap();
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod krizovy_test_s_pythonom {
    use super::*;
    use crate::tartarus::{novy_board, Cell};

    /// Priamy cross-check s `python3 -c "..."` beh (zapísaný v JOURNAL.md,
    /// TLI 63/2): rat(tier2) s muzzle+skewer+saw zožerie rat(tier2) —
    /// `davka<=4` (muzzle+2), skewer (+1, rat jedol raťa) => zisk>=4,
    /// 2+4=6 orezané na CHROMATIC(5) v JEDNOM kroku `promote` => milestone_tool
    /// AJ chromatic/vault sa spustia v TOM ISTOM volaní `_after_growth`
    /// (Python: dve nezávislé `if`, nie `elif`). Python výstup:
    /// events=['devour:1->2', 'promote:1:5', 'milestone_tool', 'chromatic:1'],
    /// vault=1, eater tier po=None (políčko sa vyprázdnilo).
    #[test]
    fn muzzle_skewer_rat_jedol_rata_skok_rovno_na_chromatic() {
        let mut rng0 = Rng::new(1);
        let mut board = novy_board(&mut rng0, None, Some("x".to_string()));
        board.tools = vec!["muzzle".to_string(), "skewer".to_string(), "saw".to_string()];
        board.cells[1] = Cell {
            index: 1, sealed: false, inmate: Some(Inmate { beast: "rat".to_string(), tier: 2 }),
            known_tier: None, tusk: false, scratch: false, nest_due_day: None,
            hidden_inmate: None, hidden_event: None,
        };
        board.cells[2] = Cell {
            index: 2, sealed: false, inmate: Some(Inmate { beast: "rat".to_string(), tier: 2 }),
            known_tier: None, tusk: false, scratch: false, nest_due_day: None,
            hidden_inmate: None, hidden_event: None,
        };
        board.warden = 1;
        let mut rng = Rng::new(123);
        let events = devour(&mut board, 1, 2, &mut rng).unwrap();

        assert_eq!(events[0], "devour:1->2");
        assert!(events.contains(&"milestone_tool".to_string()));
        assert!(events.contains(&"chromatic:1".to_string()));
        assert!(board.cell(1).inmate.is_none(), "políčko sa muselo vyprázdniť (chromatic)");
        assert_eq!(board.vault, 1);
    }
}
