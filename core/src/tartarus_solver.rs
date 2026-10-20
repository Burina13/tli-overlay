//! Solver pre Tartarus — port `solver.py` (`stacks/games/tli/engine/solver.py`
//! v repe `ops`) do core Rust (TLI 63, bod 2). Monte Carlo odhad: pre dané
//! rozhodnutie vráti zoradené možnosti s priemernou hodnotou z rollout-ov,
//! istotou a krátkym slovenským zdôvodnením.
//!
//! Rovnaký princíp ako Python (vlastná heuristika, nie kopírovanie cudzieho
//! kódu): požieraj vždy keď sa dá (okrem prerušenia platnej Prey značky),
//! vymieňaj nízku raritu, prijímaj Chain reaction, prijímaj Execution len keď
//! odstráni aspoň jedného väzňa, z nástrojov/darov ber podľa pevného poradia.
//!
//! **RNG nie je bit-zhodný s Pythonom** (`tartarus.rs::Rng`, vlastný
//! SplitMix64) — „zhoda" sa overuje pravidlovo na fixných vstupoch
//! (`tartarus_engine.rs` testy), nie cez zdieľaný seed ani cez zhodu týchto
//! Monte Carlo čísel s Pythonom do poslednej desatiny.

use crate::tartarus::{Board, Cell, Rng, BLUE, ORANGE, PURPLE};
use crate::tartarus_engine::{
    choose_gift, choose_tool, devour, end_day, is_drill_over, legal_devour_targets, patrol,
    resolve_event, swap, PravidloPorusene,
};

const HORIZONT_DNI: i64 = 3;
const CAS_LIMIT_S: f64 = 1.5;
const ISTOTA_PRAH: f64 = 0.55;

const PREFERENCIA_NASTROJOV: [&str; 13] = [
    "skewer", "revenge", "rope", "saw", "guillotine", "hook", "tongs", "keys", "muzzle", "fire",
    "scales", "pestle", "ironbox",
];
const PREFERENCIA_DAROV: [&str; 4] = ["wine", "ring", "brand", "horn"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Detail {
    pub ciel: Option<usize>,
    pub cez_scratch: Option<bool>,
    pub nastroj: Option<String>,
    pub dar: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Moznost {
    pub typ: String,
    pub detail: Detail,
    pub hodnota: f64,
    pub istota: f64,
    pub dovod: String,
}

#[derive(Debug, Clone)]
pub struct VysledokSolvera {
    pub moznosti: Vec<Moznost>,
    pub odporucana: Option<usize>,
    pub istota_odporucania: f64,
    pub cas_s: f64,
}

/// TVRDÉ PRAVIDLO (TLI 54): Replace (výmena) sa smie odporučiť len pri
/// White/Blue, prípadne Purple BEZ aktuálnej šance na Consume. Orange a
/// vyššie sa NIKDY neodporúča — toto je tvrdé pravidlo PRED Monte Carlom,
/// nie len jeho výsledok.
fn vymena_dovodna(board: &Board, index: usize) -> bool {
    let tier = match &board.cell(index).inmate {
        Some(i) => i.tier,
        None => return false,
    };
    if tier <= BLUE {
        return true;
    }
    if tier == PURPLE {
        return legal_devour_targets(board, index).is_empty();
    }
    false
}

/// Jeden rozumný krok podľa jednoduchej politiky (pre dohrávanie v rollout-e
/// PO vyhodnocovanej voľbe). Vracia Err len pri skutočnej chybe (nemalo by
/// sa stať, keďže funkcia si legálnosť overuje sama pred každým volaním).
fn politika_jeden_krok(board: &mut Board, rng: &mut Rng) -> Result<Vec<String>, PravidloPorusene> {
    if let Some(pe) = board.pending_event.clone() {
        if pe.kind == "execution" {
            let ciel = pe.cell;
            let exec_inmate = board.cell(ciel).inmate.clone();
            let ma_obet = match &exec_inmate {
                Some(ei) => board.board_cells().iter().any(|c| {
                    c.index != ciel
                        && board.eats_of(&ei.beast).contains(&c.inmate.as_ref().unwrap().beast.as_str())
                        && c.inmate.as_ref().unwrap().tier < ei.tier
                }),
                None => false,
            };
            return resolve_event(board, ma_obet, rng);
        }
        // TLI 63: zvyšných 9 incidentov sa pri rýchlej dohrávacej politike
        // prijímajú vždy — samostatné rozhodovanie rieši Monte Carlo vyššie.
        return resolve_event(board, true, rng);
    }
    if let Some(choices) = board.pending_tool_choice.clone() {
        for t in PREFERENCIA_NASTROJOV {
            if choices.iter().any(|c| c == t) {
                return choose_tool(board, t);
            }
        }
        let t = rng.choice(&choices).clone();
        return choose_tool(board, &t);
    }
    if let Some(gifts) = board.pending_gift.clone() {
        for g in PREFERENCIA_DAROV {
            if gifts.iter().any(|x| x == g) {
                return choose_gift(board, g, rng);
            }
        }
        let g = rng.choice(&gifts).clone();
        return choose_gift(board, &g, rng);
    }
    if !board.destinations.is_empty() {
        let zamknute: Vec<usize> = board.destinations.iter().copied().filter(|&d| board.cell(d).sealed).collect();
        let ciel = if !zamknute.is_empty() {
            *rng.choice(&zamknute)
        } else {
            *rng.choice(&board.destinations.clone())
        };
        return patrol(board, ciel, rng);
    }
    if !board.used_devour {
        let moznosti = legal_devour_targets(board, board.warden);
        if !moznosti.is_empty() {
            let warden = board.warden;
            let prey = board.prey.clone();
            let bez_prey: Vec<(usize, bool)> = moznosti
                .iter()
                .copied()
                .filter(|o| !prey.iter().any(|l| l.lion_cell == warden || l.prey_cell == o.0))
                .collect();
            let volba = if !bez_prey.is_empty() { *rng.choice(&bez_prey) } else { *rng.choice(&moznosti) };
            return devour(board, warden, volba.0, rng);
        }
    }
    if !board.used_swap {
        let warden = board.warden;
        let cur_tusk = board.cell(warden).tusk;
        if vymena_dovodna(board, warden) && !cur_tusk {
            return swap(board, rng);
        }
    }
    end_day(board, rng)
}

/// `povodny_den` MUSÍ byť deň PRED vyhodnocovanou akciou (TLI 34) — `end_day`
/// deň posunie o 1 ešte PRED týmto volaním.
fn dohraj(board: &mut Board, dni: i64, rng: &mut Rng, povodny_den: i64) -> f64 {
    let mut kroky = 0;
    while board.day < povodny_den + dni && !is_drill_over(board) && kroky < 400 {
        let _ = politika_jeden_krok(board, rng);
        kroky += 1;
    }
    skore(board)
}

fn skore(board: &Board) -> f64 {
    board.vault as f64 * 100.0 + board.board_cells().iter().map(|c| c.inmate.as_ref().unwrap().tier as f64).sum::<f64>()
}

/// Legálne voľby v AKTUÁLNOM rozhodovacom bode (bez simulácie).
fn moznosti_pre_stav(board: &Board) -> Vec<(String, Detail)> {
    if let Some(pe) = &board.pending_event {
        // "Treasure" Hunt (hunt_doors) má v hre len Confirm.
        if pe.kind == "hunt_doors" {
            return vec![("event_prijat".to_string(), Detail::default())];
        }
        return vec![
            ("event_prijat".to_string(), Detail::default()),
            ("event_odmietnut".to_string(), Detail::default()),
        ];
    }
    if let Some(choices) = &board.pending_tool_choice {
        return choices
            .iter()
            .map(|t| ("zvol_nastroj".to_string(), Detail { nastroj: Some(t.clone()), ..Default::default() }))
            .collect();
    }
    if let Some(gifts) = &board.pending_gift {
        return gifts
            .iter()
            .map(|g| ("zvol_dar".to_string(), Detail { dar: Some(g.clone()), ..Default::default() }))
            .collect();
    }
    if !board.destinations.is_empty() {
        return board
            .destinations
            .iter()
            .map(|&d| ("patrol".to_string(), Detail { ciel: Some(d), ..Default::default() }))
            .collect();
    }
    let mut out = vec![];
    if !board.used_devour {
        for (ciel, cez_scratch) in legal_devour_targets(board, board.warden) {
            out.push(("devour".to_string(), Detail { ciel: Some(ciel), cez_scratch: Some(cez_scratch), ..Default::default() }));
        }
    }
    // TLI 54: swap sa vôbec neponúka, keď ho tvrdé pravidlo zakazuje.
    if !board.used_swap && !board.cell(board.warden).sealed && vymena_dovodna(board, board.warden) {
        out.push(("swap".to_string(), Detail::default()));
    }
    // TLI 34: "End Day" je VŽDY legálna voľba.
    out.push(("koniec_dna".to_string(), Detail::default()));
    out
}

fn aplikuj(board: &mut Board, typ: &str, detail: &Detail, rng: &mut Rng) -> Result<(), PravidloPorusene> {
    match typ {
        "event_prijat" => {
            resolve_event(board, true, rng)?;
        }
        "event_odmietnut" => {
            resolve_event(board, false, rng)?;
        }
        "zvol_nastroj" => {
            choose_tool(board, detail.nastroj.as_ref().unwrap())?;
        }
        "zvol_dar" => {
            choose_gift(board, detail.dar.as_ref().unwrap(), rng)?;
        }
        "patrol" => {
            patrol(board, detail.ciel.unwrap(), rng)?;
        }
        "devour" => {
            let warden = board.warden;
            devour(board, warden, detail.ciel.unwrap(), rng)?;
        }
        "swap" => {
            swap(board, rng)?;
        }
        "koniec_dna" => {
            end_day(board, rng)?;
        }
        _ => {}
    }
    Ok(())
}

fn dovod(typ: &str, detail: &Detail, board: &Board) -> String {
    match typ {
        "devour" => {
            let ciel = detail.ciel.unwrap();
            let meal = board.cell(ciel).inmate.is_some();
            let cez = if detail.cez_scratch.unwrap_or(false) { " cez poškriabané dvere" } else { "" };
            if meal {
                format!("Požretie{} zvyšuje raritu a nestojí nič — jediná výnimka je prerušenie Prey značky.", cez)
            } else {
                "Požretie.".to_string()
            }
        }
        "swap" => {
            let warden = board.warden;
            match &board.cell(warden).inmate {
                Some(cur) if cur.tier <= BLUE => "Nízka rarita na políčku dozorcu — nová výmena je v priemere lepšia.".to_string(),
                _ => "Rarita na políčku dozorcu nemá momentálne šancu na požretie — výmena je lepšia než nechať ju ležať bez úžitku.".to_string(),
            }
        }
        "patrol" => {
            let ciel = detail.ciel.unwrap();
            if board.cell(ciel).sealed {
                "Zamknuté dvere prinášajú nový materiál pre reťaz.".to_string()
            } else {
                "Známy väzeň — ide sa za konkrétnou príležitosťou na požretie/výmenu.".to_string()
            }
        }
        "event_prijat" => {
            let kind = board.pending_event.as_ref().map(|p| p.kind.clone()).unwrap_or_default();
            match kind.as_str() {
                "chain_reaction" => "Reťazová reakcia zaplaví dosku materiálom zadarmo.".to_string(),
                "trickster" => "Za dverami bude rovno Supreme chýbajúceho druhu zvieraťa.".to_string(),
                "ravenous_appetite" => "Najslabší väzeň na doske dostane päť šancí posilnieť zadarmo.".to_string(),
                "high_risk_convict" => "Isté Legendary a vyššie za cudzie dvere, bez rizika.".to_string(),
                "crow" => "Čistá informácia o 4 najbližších dverách, žiadne riziko.".to_string(),
                "freedom" => "Tri zamknuté dvere naraz, bez vedľajších efektov nástrojov.".to_string(),
                "hunt_doors" => "Odhalí, ktoré dvere isto skrývajú Rare a lepšie.".to_string(),
                "portion" => "Až traja väzni na doske dostanú stupeň zadarmo.".to_string(),
                "ratway" => "Bonusová cesta mimo dosahu bežnej patroly.".to_string(),
                "usurpation" => "Reťaz Uzurpácie zdvihne väzňov na doske — čím ďalej, tým silnejšie.".to_string(),
                _ => "Poprava sa oplatí, keď odstráni aspoň jedného slabšieho väzňa.".to_string(),
            }
        }
        "event_odmietnut" => "Poprava by stála viac, než by priniesla.".to_string(),
        "zvol_nastroj" => format!("Nástroj {} je vysoko v poradí preferencie zo strategy guide.", detail.nastroj.as_ref().unwrap()),
        "zvol_dar" => {
            let dar = detail.dar.as_ref().unwrap();
            let bunky: Vec<&Cell> = board.board_cells();
            match dar.as_str() {
                "ring" => {
                    let n = board.cells.iter().filter(|c| c.sealed).count();
                    format!("Otvorí až 3 z {} zamknutých dverí naraz, bez vedľajších efektov.", n)
                }
                "horn" => {
                    let n = bunky.iter().filter(|c| !legal_devour_targets(board, c.index).is_empty()).count();
                    format!("Hneď spustí požretie u {} väzňov, čo práve majú platný cieľ.", n)
                }
                "wine" => {
                    let n = bunky.iter().filter(|c| c.inmate.as_ref().map(|i| i.tier < ORANGE).unwrap_or(false)).count();
                    format!("Zdvihne raritu {} väzňom pod Legendary o stupeň naraz.", n)
                }
                "brand" => {
                    let n = bunky.iter().filter(|c| legal_devour_targets(board, c.index).is_empty()).count();
                    format!("Až 4 z {} väzňov bez cieľa dostanú nový druh, aby mohli žrať.", n)
                }
                _ => "Dar sedí k aktuálnemu zloženiu dosky.".to_string(),
            }
        }
        _ => "Koniec dňa — na políčku dozorcu už nie je čo získať.".to_string(),
    }
}

/// Hlavné API solvera. `board` je AKTUÁLNY (plne známy) stav — na rozdiel od
/// Pythonu (ktorý dostáva neúplný `board.schema.json` dict zo screenshotu),
/// appka zatiaľ nemá odkiaľ brať VEREJNÝ stav (bod 4 zadania TLI 63/2), takže
/// vstup je priamo `&Board`. Monte Carlo neistotu simuluje `Board::fog_reroll`
/// nad TÝMTO vstupným `board` pri KAŽDOM rollout-e (rovnaký princíp ako
/// `Board.from_schema_dict(board_dict, ...)` v Pythone, volané nanovo zakaždým
/// nad tým istým `board_dict`).
pub fn navrhni_tahy(board: &Board, rollouts: usize, seed: u64) -> VysledokSolvera {
    let zaciatok = std::time::Instant::now();
    let mut base_rng = Rng::new(seed);

    let zakladny_board = board.fog_reroll(&mut base_rng);
    let kandidati = moznosti_pre_stav(&zakladny_board);
    if kandidati.is_empty() {
        return VysledokSolvera { moznosti: vec![], odporucana: None, istota_odporucania: 0.0, cas_s: zaciatok.elapsed().as_secs_f64() };
    }

    let mut vysledky: Vec<Moznost> = vec![];
    for (typ, detail) in &kandidati {
        let mut skore_rollouts: Vec<f64> = vec![];
        for _ in 0..rollouts {
            if zaciatok.elapsed().as_secs_f64() > CAS_LIMIT_S {
                break;
            }
            let mut rng = Rng::new(base_rng.next_u64());
            let mut rollout_board = board.fog_reroll(&mut rng);
            let den_pred_tahom = rollout_board.day;
            if aplikuj(&mut rollout_board, typ, detail, &mut rng).is_err() {
                continue;
            }
            let skore = dohraj(&mut rollout_board, HORIZONT_DNI, &mut rng, den_pred_tahom);
            skore_rollouts.push(skore);
        }
        if skore_rollouts.is_empty() {
            continue;
        }
        let priemer = skore_rollouts.iter().sum::<f64>() / skore_rollouts.len() as f64;
        let istota = skore_rollouts.iter().filter(|&&s| s >= priemer).count() as f64 / skore_rollouts.len() as f64;
        vysledky.push(Moznost {
            typ: typ.clone(),
            detail: detail.clone(),
            hodnota: priemer,
            istota,
            dovod: dovod(typ, detail, &zakladny_board),
        });
    }

    vysledky.sort_by(|a, b| b.hodnota.partial_cmp(&a.hodnota).unwrap());
    for m in vysledky.iter_mut().skip(1) {
        if m.istota < ISTOTA_PRAH {
            m.dovod.push_str(" (skontroluj — nízka istota odhadu)");
        }
    }

    let istota_odporucania = if vysledky.len() >= 2 {
        let rozsah = vysledky[0].hodnota.abs().max(0.01);
        let odstup = (vysledky[0].hodnota - vysledky[1].hodnota) / rozsah;
        (0.5 + 1.6 * odstup).max(0.05).min(0.95)
    } else if !vysledky.is_empty() {
        1.0
    } else {
        0.0
    };
    if !vysledky.is_empty() && istota_odporucania < ISTOTA_PRAH {
        vysledky[0].dovod.push_str(" (skontroluj — nízka istota odhadu)");
    }

    VysledokSolvera {
        odporucana: if vysledky.is_empty() { None } else { Some(0) },
        istota_odporucania,
        cas_s: zaciatok.elapsed().as_secs_f64(),
        moznosti: vysledky,
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::tartarus::{Inmate, OFFICE};

    /// Zostaví PLNE ZNÁMY board rovnako ako `testy-solver.py:_cely_board`:
    /// zadané políčka otvorené, zvyšok zamknutý s náhodnou (nepoužitou)
    /// skrytou pravdou — `fog_reroll` ju aj tak prepíše pri každom rollout-e.
    fn cely_board(warden: usize, otvorene: &[(usize, &str, i64)]) -> Board {
        let mut rng = Rng::new(1);
        let mut board = crate::tartarus::novy_board(&mut rng, None, None);
        board.warden = warden;
        for &(i, beast, tier) in otvorene {
            board.cells[i] = Cell {
                index: i,
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
        board
    }

    fn top(v: &VysledokSolvera) -> &Moznost {
        assert!(!v.moznosti.is_empty(), "solver nevrátil žiadnu možnosť");
        &v.moznosti[v.odporucana.unwrap()]
    }

    fn hodnota_pre(v: &VysledokSolvera, typ: &str) -> f64 {
        v.moznosti.iter().find(|m| m.typ == typ).unwrap_or_else(|| panic!("možnosť typ={} sa v odpovedi nenašla", typ)).hodnota
    }

    #[test]
    fn test_devour_zjavne_vyhodny_vyhrava_nad_swapom() {
        let board = cely_board(1, &[(1, "mammoth", 3), (2, "lion", 3)]);
        let v = navrhni_tahy(&board, 150, 1);
        let t = top(&v);
        assert_eq!(t.typ, "devour");
        assert_eq!(t.detail.ciel, Some(2));
    }

    #[test]
    fn test_prijat_chain_reaction_je_lepsie_nez_odmietnut() {
        let mut board = cely_board(2, &[(2, "mammoth", 1)]);
        board.pending_event = Some(crate::tartarus::PendingEvent { kind: "chain_reaction".to_string(), cell: 2 });
        let v = navrhni_tahy(&board, 150, 8);
        let hod_prijat = hodnota_pre(&v, "event_prijat");
        let hod_odmietnut = hodnota_pre(&v, "event_odmietnut");
        assert!(hod_prijat >= hod_odmietnut, "{} vs {}", hod_prijat, hod_odmietnut);
        assert_eq!(top(&v).typ, "event_prijat");
    }

    #[test]
    fn test_prijat_execution_s_obetami_je_lepsie_nez_odmietnut() {
        let mut board = cely_board(2, &[(2, "mammoth", 2), (5, "lion", 1), (9, "lion", 1)]);
        board.pending_event = Some(crate::tartarus::PendingEvent { kind: "execution".to_string(), cell: 2 });
        let v = navrhni_tahy(&board, 150, 9);
        let hod_prijat = hodnota_pre(&v, "event_prijat");
        let hod_odmietnut = hodnota_pre(&v, "event_odmietnut");
        assert!(hod_prijat > hod_odmietnut, "{} vs {}", hod_prijat, hod_odmietnut);
        assert_eq!(top(&v).typ, "event_prijat");
    }

    #[test]
    fn test_koniec_dna_je_vzdy_v_ponuke() {
        let board = cely_board(1, &[(1, "mammoth", 3), (2, "lion", 3)]);
        let v = navrhni_tahy(&board, 50, 11);
        assert!(v.moznosti.iter().any(|m| m.typ == "koniec_dna"));
    }

    #[test]
    fn test_kancelaria_nema_v_boarde_obyvatela() {
        let board = cely_board(1, &[(1, "mammoth", 0)]);
        assert!(board.cell(OFFICE).inmate.is_none());
    }
}
