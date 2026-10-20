//! Cieľová funkcia Frozen Canvasu — PODĽA ČOHO sa meria, či je plátno lepšie
//! (TLI 72, `2026-10-20-1636-mcp.md`).
//!
//! Port `canvas_hodnotenie.py` (`stacks/games/tli/engine/canvas_hodnotenie.py`
//! v repe `ops`), presne tých štyroch funkcií, ktoré potrebuje solver
//! (`canvas_solver.rs`) — rovnaký dôvod ako pri celom porte: lokálny a
//! serverový výsledok musia na rovnaký `CanvasStav` vyjsť identicky.
//!
//! **Prečo samostatný modul.** Do TLI 72 bola „cieľová funkcia" len implicitne
//! v triedení slotov solvera (najvyšší výsledný level prv) a solver tak tíško
//! maximalizoval SÚČET levelov. Hra ale odmeňuje painty na Max Paint Level
//! (dúhové) — dva dúhové sú cennejšie než štyri o level nižšie, hoci súčet je
//! menší. Pravidlo je zapísané v `canvas.yml` → `cielova_funkcia` (repo `ops`)
//! a tu je jeho jediné miesto v jadre, aby sa kritérium nerozpadlo po
//! viacerých súboroch s inými skratkami.

use crate::canvas::CanvasStav;
use serde::{Deserialize, Serialize};

/// „Level up the Vertical Monster Paint by 2 level(s)" → 2; inak 1.
///
/// Server to robí regexom `by\s+(\d+)\s*level` (bez ohľadu na veľkosť
/// písmen); jadro `regex` nemá, takže sa to tu napodobňuje ručne — vrátane
/// toho, že sa berie PRVÝ úplný zápas a keď je jeho číslo mimo 1..=10, vráti
/// sa 1 bez hľadania ďalej (`re.search` tiež ďalej nehľadá). Do TLI 72 Rust
/// verzia v takom prípade pokračovala a na texte „by 99 level … by 2 level"
/// by vrátila 2, kým server 1. Hodnota 1 ako záloha je overená pre
/// `adjacent` z dvoch párov pred/po (15.10.2026).
pub fn pocet_levelov_efektu(efekt_popis: &str) -> i64 {
    let z: Vec<char> = efekt_popis.to_lowercase().chars().collect();
    let level: Vec<char> = "level".chars().collect();
    let mut i = 0usize;
    while i + 1 < z.len() {
        if z[i] != 'b' || z[i + 1] != 'y' {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        let zac_medzier = j;
        while j < z.len() && z[j].is_whitespace() {
            j += 1;
        }
        // `\s+` — aspoň jedna medzera medzi „by" a číslom
        if j == zac_medzier {
            i += 1;
            continue;
        }
        let zac_cisla = j;
        while j < z.len() && z[j].is_ascii_digit() {
            j += 1;
        }
        if j == zac_cisla {
            i += 1;
            continue;
        }
        let kon_cisla = j;
        while j < z.len() && z[j].is_whitespace() {
            j += 1;
        }
        if z.len() < j + level.len() || z[j..j + level.len()] != level[..] {
            i += 1;
            continue;
        }
        // Prvý úplný zápas rozhoduje. Číslo, ktoré sa do i64 nezmestí, je
        // rovnako „mimo rozsahu" ako 99 — Python ho prečíta a zahodí tiež.
        let cislo: Option<i64> = z[zac_cisla..kon_cisla].iter().collect::<String>().parse().ok();
        return match cislo {
            Some(n) if (1..=10).contains(&n) => n,
            _ => 1,
        };
    }
    1
}

/// Na aký level paint skočí: `level + N`, no nikdy nad strop hry. Jediné
/// miesto, kde sa strop uplatňuje — solver ho kedysi počítal na štyroch
/// miestach inline a každé sa dalo pokaziť zvlášť.
pub fn vysledny_level(level: i64, pocet_levelov: i64, max_level: i64) -> i64 {
    (level + pocet_levelov).min(max_level)
}

/// Skóre plátna podľa `canvas.yml` → `cielova_funkcia`. Serializuje sa
/// rovnako ako slovník na serveri (`skore_pred` vo výsledku solvera).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Skore {
    /// HLAVNÉ kritérium: počet paintov na `max_level` (dúhových).
    pub na_max: i64,
    /// Sekundárne: súčet levelov — rozhoduje LEN pri rovnakom `na_max`.
    pub sucet: i64,
    pub obsadenych: i64,
    pub dostupnych: i64,
    pub max_level: i64,
}

impl Skore {
    /// Lexikografické porovnanie (`na_max`, `sucet`). Zámerne nie vážený
    /// súčet: žiadna váha by nezaručila, že ľubovoľne veľký súčet levelov
    /// nikdy neprebije jeden dúhový paint navyše.
    pub fn lepsie_nez(&self, iny: &Skore) -> bool {
        (self.na_max, self.sucet) > (iny.na_max, iny.sucet)
    }
}

/// Skóre celého plátna. Rátajú sa len DOSTUPNÉ bunky — nedostupné na doske
/// fyzicky nie sú a hra ich do výsledku nepočíta.
pub fn skore_platna(stav: &CanvasStav) -> Skore {
    let mut s = Skore { max_level: stav.max_level, ..Skore::default() };
    for b in stav.bunky.iter().filter(|b| b.dostupna) {
        s.dostupnych += 1;
        if let Some(p) = &b.paint {
            s.obsadenych += 1;
            s.sucet += p.level;
            if p.level >= stav.max_level {
                s.na_max += 1;
            }
        }
    }
    s
}

/// Koľko paintov na `max_level` pribudne, keď sa `pocet_umiestneni`
/// najlepších paintov danej kategórie z ruky položí s efektom +N.
///
/// „Najlepšie" = najvyšší VÝSLEDNÝ level, rovnako ako ich vyberá solver —
/// inak by veta o prínose hovorila o iných paintoch, než aké rada naozaj
/// použije.
pub fn prinos_na_max(stav: &CanvasStav, kategoria: &str, pocet_levelov: i64, pocet_umiestneni: i64) -> i64 {
    if pocet_umiestneni <= 0 {
        return 0;
    }
    let mut levely: Vec<i64> = stav
        .ruka
        .iter()
        .filter_map(|s| s.paint.as_ref())
        .filter(|p| p.kategoria == kategoria)
        .map(|p| p.level)
        .collect();
    // stabilné triedenie ako Pythonovo `sorted`
    levely.sort_by_key(|lv| -vysledny_level(*lv, pocet_levelov, stav.max_level));
    levely
        .iter()
        .take(pocet_umiestneni as usize)
        .filter(|lv| vysledny_level(**lv, pocet_levelov, stav.max_level) >= stav.max_level)
        .count() as i64
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::canvas::{Bunka, Paint, SlotRuky};

    fn bunka(riadok: i64, stlpec: i64, dostupna: bool, paint: Option<Paint>) -> Bunka {
        Bunka { riadok, stlpec, dostupna, paint, oznacena_vykricnikom: false, oramovana: false }
    }

    fn stav(bunky: Vec<Bunka>, ruka: Vec<Paint>, max_level: i64) -> CanvasStav {
        CanvasStav {
            riadky: 6,
            stlpce: 6,
            max_level,
            bunky,
            ruka: ruka
                .into_iter()
                .enumerate()
                .map(|(i, p)| SlotRuky { index: i as i64, paint: Some(p) })
                .collect(),
            ciel: None,
            polozene: None,
            celkom: None,
        }
    }

    fn gameplay(level: i64) -> Paint {
        Paint { kategoria: "gameplay".to_string(), typ: "chest".to_string(), level }
    }

    #[test]
    fn pocet_levelov_sa_cita_z_textu_efektu() {
        assert_eq!(pocet_levelov_efektu("Level up the Vertical Monster Paint by 2 level(s)"), 2);
        assert_eq!(pocet_levelov_efektu("Level up the Adjacent Monster Paint by 1 level(s)"), 1);
        assert_eq!(pocet_levelov_efektu("LEVEL UP BY 3 LEVEL(S)"), 3);
        assert_eq!(pocet_levelov_efektu("by 10 level"), 10);
        assert_eq!(pocet_levelov_efektu("by 4level"), 4);
        assert_eq!(pocet_levelov_efektu(""), 1);
        assert_eq!(pocet_levelov_efektu("nezmysel bez cisla"), 1);
        assert_eq!(pocet_levelov_efektu("by 0 level(s)"), 1);
        assert_eq!(pocet_levelov_efektu("by 99 level(s)"), 1);
        assert_eq!(pocet_levelov_efektu("by 99999999999999999999999 level(s)"), 1);
    }

    /// Ručná náhrada regexu `by\s+(\d+)\s*level` musí mať tie isté okraje ako
    /// `re.search` na serveri, inak by sa lokálna a serverová rada rozišli
    /// práve na texte, ktorý OCR trochu pokazí.
    #[test]
    fn pocet_levelov_sa_sprava_ako_regex_servera() {
        // „by 3" bez slova level sa nepočíta; platné „by 2 level" ďalej áno
        assert_eq!(pocet_levelov_efektu("increased by 3 and leveled up by 2 level(s)"), 2);
        // medzi „by" a číslom musí byť aspoň jedna medzera, smie byť aj zalomenie
        assert_eq!(pocet_levelov_efektu("by2 level"), 1);
        assert_eq!(pocet_levelov_efektu("Paint by\n2\nlevel(s)"), 2);
        // prvý úplný zápas rozhoduje aj vtedy, keď je mimo rozsahu
        assert_eq!(pocet_levelov_efektu("by 99 level, then by 2 level"), 1);
    }

    #[test]
    fn vysledny_level_nepretiahne_strop() {
        assert_eq!(vysledny_level(3, 1, 7), 4);
        assert_eq!(vysledny_level(6, 2, 7), 7);
        assert_eq!(vysledny_level(7, 1, 7), 7);
    }

    /// Jadro TLI 72: jeden dúhový paint navyše prebije AKÝKOĽVEK súčet.
    #[test]
    fn na_max_prebije_sucet() {
        let viac_duhovych = Skore { na_max: 2, sucet: 14, ..Skore::default() };
        let vacsi_sucet = Skore { na_max: 1, sucet: 40, ..Skore::default() };
        assert!(viac_duhovych.lepsie_nez(&vacsi_sucet));
        assert!(!vacsi_sucet.lepsie_nez(&viac_duhovych));
    }

    #[test]
    fn pri_rovnakom_na_max_rozhodne_sucet_a_zhoda_nie_je_lepsia() {
        let a = Skore { na_max: 1, sucet: 10, ..Skore::default() };
        let b = Skore { na_max: 1, sucet: 9, ..Skore::default() };
        assert!(a.lepsie_nez(&b));
        assert!(!b.lepsie_nez(&a));
        assert!(!a.lepsie_nez(&a));
    }

    #[test]
    fn skore_platna_rata_len_dostupne_bunky() {
        let s = stav(
            vec![
                bunka(1, 1, true, Some(Paint::monster("dwarf", 7))),
                bunka(1, 2, true, Some(Paint::monster("ghost", 3))),
                bunka(1, 3, true, None),
                // nedostupná bunka sa nepočíta, ani keď na nej niečo je
                bunka(1, 4, false, Some(Paint::monster("ghost", 7))),
            ],
            vec![],
            7,
        );
        assert_eq!(
            skore_platna(&s),
            Skore { na_max: 1, sucet: 10, obsadenych: 2, dostupnych: 3, max_level: 7 }
        );
    }

    /// Server posiela `skore_pred` ako slovník s presne týmito kľúčmi.
    #[test]
    fn skore_sa_serializuje_ako_na_serveri() {
        let s = Skore { na_max: 1, sucet: 10, obsadenych: 2, dostupnych: 3, max_level: 7 };
        assert_eq!(
            serde_json::to_value(s).unwrap(),
            serde_json::json!({"na_max": 1, "sucet": 10, "obsadenych": 2, "dostupnych": 3, "max_level": 7})
        );
    }

    #[test]
    fn prinos_na_max_berie_najlepsie_painty_kategorie() {
        let s = stav(
            vec![],
            vec![Paint::monster("a", 2), Paint::monster("b", 6), gameplay(7), Paint::monster("c", 7),
                 Paint::monster("d", 5)],
            7,
        );
        // +1: Lv6 → 7 a Lv7 → 7 sú dúhové, Lv5 → 6 nie; gameplay sa nepočíta
        assert_eq!(prinos_na_max(&s, "monster", 1, 1), 1);
        assert_eq!(prinos_na_max(&s, "monster", 1, 2), 2);
        assert_eq!(prinos_na_max(&s, "monster", 1, 10), 2);
        // +2: pribudne aj Lv5 → 7
        assert_eq!(prinos_na_max(&s, "monster", 2, 4), 3);
        assert_eq!(prinos_na_max(&s, "gameplay", 1, 3), 1);
    }

    #[test]
    fn prinos_na_max_bez_umiestneni_je_nula() {
        let s = stav(vec![], vec![Paint::monster("a", 7)], 7);
        assert_eq!(prinos_na_max(&s, "monster", 1, 0), 0);
        assert_eq!(prinos_na_max(&s, "monster", 1, -1), 0);
        assert_eq!(prinos_na_max(&stav(vec![], vec![], 7), "monster", 1, 3), 0);
    }
}
