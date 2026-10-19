//! Solver pre Frozen Canvas — `navrhni_kroky()` vráti sekvenciu krokov ČO→KAM
//! (slot v ruke → bunka plátna) pre splnenie aktuálneho miešacieho cieľa.
//!
//! Port `canvas_solver.py` (`stacks/games/tli/engine/canvas_solver.py` v repe `ops`)
//! do core Rust (TLI 59, bod 2) — ÚMYSELNE riadok po riadku tá istá logika, aby
//! lokálny výsledok pre rovnaký `CanvasStav` vyšiel identicky ako server. Implementovaný
//! je LEN `smer: "adjacent"`, presne ako v Pythone — ostatné smery solver vedome
//! odmieta hádať, kým pre ne nie je overené pravidlo.

use crate::canvas::{Bunka, CanvasStav, Paint};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Krok {
    pub poradie: i64,
    pub zdroj_slot: i64,
    pub ciel_riadok: i64,
    pub ciel_stlpec: i64,
    pub dovod: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VysledokSolvera {
    pub kroky: Vec<Krok>,
    pub istota: f64,
    pub skontroluj: Vec<String>,
}

impl VysledokSolvera {
    fn skontroluj(dovod: impl Into<String>) -> Self {
        Self { kroky: vec![], istota: 0.0, skontroluj: vec![dovod.into()] }
    }
}

pub fn navrhni_kroky(stav: &CanvasStav) -> VysledokSolvera {
    let ciel = match &stav.ciel {
        None => {
            return VysledokSolvera::skontroluj(
                "plátno nemá rozpoznaný miešací cieľ (ciel == null)",
            )
        }
        Some(c) => c,
    };

    if ciel.smer != "adjacent" {
        return VysledokSolvera::skontroluj(format!(
            "smer '{}' nemá overené pravidlo miešania — solver preto nič nenavrhuje, \
             nech sa nehrá podľa vymyslenej mechaniky (čaká sa na dáta z 16.10.2026)",
            ciel.smer
        ));
    }

    let (kluc_riadok, kluc_stlpec) = match (ciel.kluc_riadok, ciel.kluc_stlpec) {
        (Some(r), Some(c)) => (r, c),
        _ => {
            return VysledokSolvera::skontroluj(
                "cieľ smeru 'adjacent' nemá kluc_riadok/kluc_stlpec \
                 (bunku s výkričníkom, okolo ktorej sa počíta)",
            )
        }
    };

    let potrebne = ciel.pocet_pozadovany - ciel.pocet_aktualny;
    if potrebne <= 0 {
        return VysledokSolvera {
            kroky: vec![],
            istota: 1.0,
            skontroluj: vec!["cieľ je už splnený (pocet_aktualny >= pocet_pozadovany)".to_string()],
        };
    }

    let mut volne: Vec<&Bunka> = stav.volne_susedne_bunky(kluc_riadok, kluc_stlpec);
    volne.sort_by_key(|b| (b.riadok, b.stlpec));

    let mut monster_sloty: Vec<(i64, &Paint)> = stav
        .ruka
        .iter()
        .filter_map(|s| s.paint.as_ref().filter(|p| p.kategoria == "monster").map(|p| (s.index, p)))
        .collect();
    // Najnižší level prv — nech zostane rezerva pod stropom (rovnaké ako Python).
    monster_sloty.sort_by_key(|(index, paint)| (paint.level, *index));

    let kluc_bunka = stav.bunka(kluc_riadok, kluc_stlpec);
    let kluc_popis = match kluc_bunka.and_then(|b| b.paint.as_ref()) {
        Some(p) => format!("{} Lv{}", p.typ, p.level),
        None => "kľúčový paint".to_string(),
    };

    let pocet = (potrebne as usize).min(volne.len()).min(monster_sloty.len());
    let mut skontroluj: Vec<String> = vec![];
    if (pocet as i64) < potrebne {
        let chyba = if (volne.len() as i64) < potrebne {
            "voľných susedných políčok"
        } else {
            "Monster Paintov v ruke"
        };
        skontroluj.push(format!(
            "cieľ žiada {} umiestnení, ale k dispozícii je len {} ({}) — vrátená sekvencia cieľ nedokončí celý",
            potrebne, pocet, chyba
        ));
    }

    let mut kroky: Vec<Krok> = vec![];
    for i in 0..pocet {
        let (slot_index, paint) = monster_sloty[i];
        let bunka = volne[i];
        let po_umiestneni = (paint.level + 1).min(stav.max_level);
        if paint.level >= stav.max_level {
            skontroluj.push(format!(
                "slot {} ({} Lv{}) je už na max_level {} — Color Mixing Effect naň podľa pozorovania \
                 nemá kam zvýšiť level, umiestnenie samo cieľ 'Fill' aj tak splní",
                slot_index, paint.typ, paint.level, stav.max_level
            ));
        }
        let dovod = format!(
            "Vezmi {} Lv{} zo slotu {} v ruke a polož ho na bunku (riadok {}, stĺpec {}) vedľa {} \
             (r{}c{}) — splní krok {}/{} cieľa '{}' a umiestnený paint vyskočí na Lv{} ({}).",
            paint.typ,
            paint.level,
            slot_index,
            bunka.riadok,
            bunka.stlpec,
            kluc_popis,
            kluc_riadok,
            kluc_stlpec,
            ciel.pocet_aktualny + i as i64 + 1,
            ciel.pocet_pozadovany,
            ciel.popis,
            po_umiestneni,
            ciel.efekt_popis,
        );
        kroky.push(Krok {
            poradie: i as i64 + 1,
            zdroj_slot: slot_index,
            ciel_riadok: bunka.riadok,
            ciel_stlpec: bunka.stlpec,
            dovod,
        });
    }

    let istota = if pocet as i64 == potrebne {
        0.9
    } else if pocet > 0 {
        0.5
    } else {
        0.0
    };

    VysledokSolvera { kroky, istota, skontroluj }
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::canvas::{Bunka, CielMiesania, Paint, SlotRuky};
    use std::collections::HashSet;

    // Voľné bunky podľa reálneho screenshotu (riadok, stĺpec, od 1) — rovnaké
    // fixture ako `testy-canvas-solver.py` v `ops` (`2026-10-15-0025-mcp.md`), aby
    // sa dal výsledok priamo porovnať s Python solverom na tom istom vstupe.
    const VOLNE_ZO_SCREENSHOTU: [(i64, i64); 10] =
        [(2, 5), (3, 2), (3, 3), (3, 4), (3, 5), (4, 2), (4, 4), (4, 5), (5, 2), (5, 3)];
    const KLUC_BUNKA: (i64, i64) = (4, 3); // Dwarf Lv3, štartový/fixný paint

    fn board_zo_screenshotu() -> CanvasStav {
        let volne: HashSet<(i64, i64)> = VOLNE_ZO_SCREENSHOTU.iter().copied().collect();
        let mut bunky = vec![];
        for r in 1..=6 {
            for c in 1..=6 {
                if (r, c) == KLUC_BUNKA {
                    bunky.push(Bunka {
                        riadok: r,
                        stlpec: c,
                        dostupna: true,
                        paint: Some(Paint::monster("dwarf", 3)),
                        oznacena_vykricnikom: true,
                    });
                } else if volne.contains(&(r, c)) {
                    bunky.push(Bunka { riadok: r, stlpec: c, dostupna: true, paint: None, oznacena_vykricnikom: false });
                } else {
                    bunky.push(Bunka { riadok: r, stlpec: c, dostupna: false, paint: None, oznacena_vykricnikom: false });
                }
            }
        }

        let ruka = vec![
            SlotRuky { index: 0, paint: Some(Paint::monster("dwarf", 1)) },
            SlotRuky { index: 1, paint: Some(Paint::monster("dwarf", 1)) },
            SlotRuky { index: 2, paint: Some(Paint::monster("dwarf", 1)) },
            SlotRuky { index: 3, paint: Some(Paint::monster("ghost", 1)) },
            SlotRuky { index: 4, paint: Some(Paint::monster("ghost", 1)) },
            SlotRuky { index: 5, paint: Some(Paint::monster("test_subject", 1)) },
            SlotRuky { index: 6, paint: Some(Paint::monster("eumbra", 1)) },
            SlotRuky { index: 7, paint: Some(Paint::monster("einherjar", 1)) },
            SlotRuky { index: 8, paint: Some(Paint::monster("einherjar", 1)) },
            SlotRuky { index: 9, paint: Some(Paint::monster("agito", 1)) },
        ];

        let ciel = CielMiesania {
            smer: "adjacent".to_string(),
            popis: "Fill a Adjacent Cell with a Monster Paint".to_string(),
            efekt_popis: "Level up the Adjacent Monster Paint by 1 level(s)".to_string(),
            pocet_pozadovany: 3,
            pocet_aktualny: 0,
            kluc_riadok: Some(KLUC_BUNKA.0),
            kluc_stlpec: Some(KLUC_BUNKA.1),
        };

        CanvasStav { riadky: 6, stlpce: 6, max_level: 7, bunky, ruka, ciel: Some(ciel) }
    }

    fn susedia_r4c3() -> HashSet<(i64, i64)> {
        [(3, 3), (4, 2), (4, 4), (5, 3)].into_iter().collect()
    }

    #[test]
    fn sekvencia_splna_ciel_0_z_3() {
        let stav = board_zo_screenshotu();
        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.len(), 3, "{:?}", v);
        assert!(v.skontroluj.is_empty(), "{:?}", v.skontroluj);
        assert!(v.istota > 0.8);
    }

    #[test]
    fn ciele_su_volne_susedne_bunky_dwarfa() {
        let stav = board_zo_screenshotu();
        let v = navrhni_kroky(&stav);
        let ciele: HashSet<(i64, i64)> = v.kroky.iter().map(|k| (k.ciel_riadok, k.ciel_stlpec)).collect();
        assert_eq!(ciele.len(), 3);
        assert!(ciele.is_subset(&susedia_r4c3()));
    }

    #[test]
    fn zdroje_su_rozne_monster_painty_z_ruky() {
        let stav = board_zo_screenshotu();
        let v = navrhni_kroky(&stav);
        let zdroje: Vec<i64> = v.kroky.iter().map(|k| k.zdroj_slot).collect();
        let unikatne: HashSet<i64> = zdroje.iter().copied().collect();
        assert_eq!(unikatne.len(), zdroje.len());
        for idx in &zdroje {
            let slot = stav.ruka.iter().find(|s| s.index == *idx).unwrap();
            assert_eq!(slot.paint.as_ref().unwrap().kategoria, "monster");
        }
    }

    #[test]
    fn poradie_je_1_2_3() {
        let stav = board_zo_screenshotu();
        let v = navrhni_kroky(&stav);
        let poradia: Vec<i64> = v.kroky.iter().map(|k| k.poradie).collect();
        assert_eq!(poradia, vec![1, 2, 3]);
    }

    #[test]
    fn dovod_spomina_cielovu_bunku_aj_efekt() {
        let stav = board_zo_screenshotu();
        let v = navrhni_kroky(&stav);
        for k in &v.kroky {
            assert!(k.dovod.contains("Lv"));
            assert!(k.dovod.contains("Color Mixing Effect") || k.dovod.contains("Level up"));
        }
    }

    #[test]
    fn cely_splneny_ciel_neziada_dalsie_kroky() {
        let mut stav = board_zo_screenshotu();
        stav.ciel.as_mut().unwrap().pocet_aktualny = 3;
        let v = navrhni_kroky(&stav);
        assert!(v.kroky.is_empty());
        assert_eq!(v.istota, 1.0);
    }

    #[test]
    fn nepodporovany_smer_sa_vedome_odmietne() {
        let mut stav = board_zo_screenshotu();
        stav.ciel.as_mut().unwrap().smer = "horizontal".to_string();
        let v = navrhni_kroky(&stav);
        assert!(v.kroky.is_empty());
        assert!(v.skontroluj.iter().any(|s| s.contains("horizontal")));
    }

    #[test]
    fn nedostatok_volnych_susednych_buniek_vrati_ciastocnu_sekvenciu() {
        let mut stav = board_zo_screenshotu();
        for (r, c) in [(3, 3), (4, 2), (4, 4)] {
            stav.bunky.iter_mut().find(|b| b.riadok == r && b.stlpec == c).unwrap().paint =
                Some(Paint::monster("gang", 1));
        }
        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.len(), 1);
        assert!(v.skontroluj.iter().any(|s| s.contains("voľných susedných políčok")));
    }

    #[test]
    fn nedostatok_monster_paintov_v_ruke_vrati_ciastocnu_sekvenciu() {
        let mut stav = board_zo_screenshotu();
        for slot in stav.ruka.iter_mut() {
            if slot.index > 0 {
                slot.paint = Some(Paint { kategoria: "gameplay".to_string(), typ: "neznamy".to_string(), level: 1 });
            }
        }
        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.len(), 1);
        assert!(v.skontroluj.iter().any(|s| s.contains("Monster Paintov v ruke")));
    }

    #[test]
    fn level_na_strope_sa_vyznaci_ale_krok_sa_nevynecha() {
        let mut stav = board_zo_screenshotu();
        for slot in stav.ruka.iter_mut() {
            if slot.index < 3 {
                slot.paint = Some(Paint::monster("dwarf", 7));
            } else {
                slot.paint = Some(Paint { kategoria: "gameplay".to_string(), typ: "neznamy".to_string(), level: 1 });
            }
        }
        stav.max_level = 7;
        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.len(), 3);
        assert!(v.skontroluj.iter().any(|s| s.contains("max_level")));
    }

    #[test]
    fn chybajuci_ciel_sa_hlasi_zrozumitelne() {
        let mut stav = board_zo_screenshotu();
        stav.ciel = None;
        let v = navrhni_kroky(&stav);
        assert!(v.kroky.is_empty());
        assert!(v.skontroluj[0].contains("ciel"));
    }
}
