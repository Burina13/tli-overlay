//! Solver pre Frozen Canvas — `navrhni_kroky()` vráti sekvenciu krokov ČO→KAM
//! (slot v ruke → bunka plátna) pre splnenie aktuálneho miešacieho cieľa.
//!
//! Port `canvas_solver.py` (`stacks/games/tli/engine/canvas_solver.py` v repe `ops`)
//! do core Rust (TLI 59, bod 2) — ÚMYSELNE riadok po riadku tá istá logika, aby
//! lokálny výsledok pre rovnaký `CanvasStav` vyšiel identicky ako server.
//!
//! **TLI 69 (20.10.2026) — jedno pravidlo pre všetky smery.** Do tohto dňa tu
//! bolo implementované LEN `smer: "adjacent"` a pri každom inom solver vrátil
//! „žiadny krok" s vetou „smer '<x>' nemá overené pravidlo miešania". Richard to
//! nahlásil na smere `vertical` a ukázal, čo sme prehliadali: **hra platné
//! cieľové bunky sama žlto orámuje** (`canvas_ramy.rs`). Dosah sa teda nehádá z
//! mena smeru, berie sa z rámu — a potom je pravidlo pre `adjacent`,
//! `vertical`, `horizontal`, `diagonal` aj čokoľvek ďalšie to isté.
//!
//! Poradie zdrojov pravdy (`CanvasStav::cielove_bunky`):
//!
//! 1. **Žltý rám hry** (`Bunka::oramovana`) — platí pre akýkoľvek smer.
//! 2. **Geometria podľa názvu smeru** (`canvas::bunky_v_smere`) — záloha pre
//!    snímky, kde rám nie je (hra ho nekreslí v podobrazovkách „Select the
//!    Brushstroke direction" a „Draw").
//!
//! Čo sa NEZMENILO: EFEKT (o koľko levelov vyskočí položený paint) je potvrdený
//! z párov pred/po len pre `adjacent`. Pre ostatné smery sa rada UKÁŽE,
//! označená ako neoverená — zadanie TLI 69 bod 4 to žiada výslovne („ukáž radu
//! s označením 'neoverené pre vertical', ale UKÁŽ ju. Nie 'žiadny krok'").
//!
//! **TLI 72 (20.10.2026) — cieľová funkcia.** Hlavné kritérium je počet
//! paintov na Max Paint Level (dúhových), súčet levelov je až druhý
//! (`canvas_hodnotenie.rs`, `canvas.yml` → `cielova_funkcia`). Výber slotov sa
//! tým nemení — radenie podľa výsledného levelu už maximalizuje aj počet
//! dúhových — mení sa to, čo rada HOVORÍ: koľko dúhových sekvencia vyrobí,
//! a keď žiadny, či sa cieľ vôbec oplatí dokončiť.

use crate::canvas::{kategoria_z_popisu, Bunka, CanvasStav, Paint, ZdrojDosahu,
                    SMERY_S_OVERENYM_EFEKTOM};
use crate::canvas_hodnotenie::{pocet_levelov_efektu, prinos_na_max, skore_platna, vysledny_level,
                               Skore};
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
    /// Odkiaľ sa vzal dosah — `"ram"` | `"smer"` | `""` (nerátalo sa).
    #[serde(default)]
    pub zdroj_dosahu: String,
    /// TLI 72: koľko paintov na Max Paint Level táto sekvencia vyrobí.
    #[serde(default)]
    pub prinos_na_max: i64,
    /// TLI 72: skóre celého plátna PRED sekvenciou. Nesú ho aj early-return
    /// vetvy — aj keď nie je čo radiť, appka ukáže, kde plátno stojí.
    #[serde(default)]
    pub skore_pred: Skore,
}

impl VysledokSolvera {
    fn skontroluj(dovod: impl Into<String>, skore_pred: Skore) -> Self {
        Self {
            kroky: vec![],
            istota: 0.0,
            skontroluj: vec![dovod.into()],
            zdroj_dosahu: String::new(),
            prinos_na_max: 0,
            skore_pred,
        }
    }
}

/// Množné číslo kategórie do vety („Monster Painty" / „Gameplay Painty").
/// Zrkadlí `_nazov_kategorie_mn` na serveri.
fn nazov_kategorie_mn(kategoria: &str) -> &'static str {
    if kategoria == "gameplay" { "Gameplay Painty" } else { "Monster Painty" }
}

/// Jedna veta „prečo práve tento paint" — TLI 69b, doplnok k TLI 69.
/// Zrkadlí `_dovod_vyberu` na serveri; reťazce sú znak po znaku rovnaké.
///
/// Richard 20.10.2026 nad snímkou v0.24.0: „PREČO CHCE ZOBRAŤ LEVEL 1 A NIE TIE
/// LVL 2?" Rada bez zdôvodnenia výberu sa nedá skontrolovať — hráč vidí len
/// slot a bunku, a keď mu sedliacky rozum hovorí niečo iné, nemá ako zistiť,
/// či sa mýli solver alebo on. Preto ku každému kroku pribúda, aký výsledný
/// level ten paint dá a prečo nie je lepší iný. Platí to v OBOCH smeroch: keď
/// sa vyberie NIŽŠÍ level, musí byť napísané prečo.
///
/// `sloty` je už zoradený zoznam kandidátov (najlepší výsledok prvý), `i` je
/// index práve radeného kroku a `pocet` dĺžka celej sekvencie — z nej sa pozná,
/// ktoré painty v ruke naozaj OSTANÚ a ktoré sekvencia minie.
fn dovod_vyberu(
    sloty: &[(i64, &Paint)],
    i: usize,
    pocet: usize,
    pocet_levelov: i64,
    max_level: i64,
    kategoria: &str,
) -> String {
    let (_, paint) = sloty[i];
    let kat_mn = nazov_kategorie_mn(kategoria);
    // V Pythone slovník index → výsledok; indexy slotov sú unikátne, takže
    // výsledok k slotu sa dá rátať priamo z jeho paintu.
    let vysledok_z = |p: &Paint| vysledny_level(p.level, pocet_levelov, max_level);
    let vysledok = vysledok_z(paint);
    let najlepsi = sloty.iter().map(|(_, p)| vysledok_z(p)).max().unwrap_or(vysledok);
    let sip = format!("Lv{} → Lv{}", paint.level, vysledok);
    // TLI 72: krok, ktorý vyrobí dúhový paint, to musí povedať — hráč inak
    // nevie, že práve tento krok je ten cenný, a mohol by ho obetovať za
    // „vyšší súčet" inde.
    let na_max = if vysledok >= max_level {
        format!(
            " Tento krok vyrobí paint na Max Paint Level Lv{max_level} (dúhový) — to je hlavný \
             cieľ, počet dúhových je viac než súčet levelov."
        )
    } else {
        String::new()
    };

    if vysledok < najlepsi {
        return format!(
            "Prečo tento: {sip} — painty s vyšším výsledkom (až Lv{najlepsi}) \
             si vzali predošlé kroky tejto sekvencie, toto je najvyšší zvyšný.{na_max}"
        );
    }

    let najvyssi_nizsi = sloty.iter().map(|(_, p)| vysledok_z(p)).filter(|v| *v < vysledok).max();
    let mut preco = match najvyssi_nizsi {
        Some(nizsi) => format!(
            "Prečo tento: {sip} je najvyšší výsledok, aký sa z ruky dá dosiahnuť — \
             slabšie {kat_mn} v ruke by skončili najviac na Lv{nizsi}.{na_max}"
        ),
        None => format!(
            "Prečo tento: {sip}; vyšší výsledok z tejto ruky nejde, \
             všetky {kat_mn} v nej dajú to isté.{na_max}"
        ),
    };

    // Strop sa spomína LEN vtedy, keď silnejší paint naozaj ostane v ruke —
    // teda je až za koncom tejto sekvencie (`sloty[pocet..]`). Keby ho sekvencia
    // aj tak minula, veta „silnejší ostáva v ruke" by klamala.
    let vyssie_rovnake = sloty
        .get(pocet..)
        .unwrap_or(&[])
        .iter()
        .filter(|(_, p)| p.level > paint.level && vysledok_z(p) == vysledok)
        .map(|(_, p)| p.level)
        .max();
    if let Some(vyssi) = vyssie_rovnake {
        preco.push_str(&format!(
            " Lv{vyssi} by na strope Lv{max_level} skončil rovnako, \
             preto ide prv nižší a silnejší ostáva v ruke."
        ));
    }
    preco
}

/// Slovenské množné číslo pre „bunka" — rovnaká konvencia ako `mn_paint`
/// v `canvas_hodnotenie.rs`. Zrkadlí `_mn_buniek()` v `canvas_solver.py`.
fn mn_buniek(pocet: usize) -> &'static str {
    match pocet {
        1 => "bunku",
        2..=4 => "bunky",
        _ => "buniek",
    }
}

pub fn navrhni_kroky(stav: &CanvasStav) -> VysledokSolvera {
    let skore_pred = skore_platna(stav);
    let ciel = match &stav.ciel {
        None => {
            return VysledokSolvera::skontroluj(
                "plátno nemá rozpoznaný miešací cieľ (ciel == null)",
                skore_pred,
            )
        }
        Some(c) => c,
    };

    let potrebne = ciel.pocet_pozadovany - ciel.pocet_aktualny;
    if potrebne <= 0 {
        return VysledokSolvera {
            kroky: vec![],
            istota: 1.0,
            skontroluj: vec!["cieľ je už splnený (pocet_aktualny >= pocet_pozadovany)".to_string()],
            zdroj_dosahu: String::new(),
            prinos_na_max: 0,
            skore_pred,
        };
    }

    let (volne, zdroj_dosahu) = stav.cielove_bunky(ciel);
    let mut skontroluj: Vec<String> = vec![];
    if volne.is_empty() {
        if zdroj_dosahu == ZdrojDosahu::Ram {
            // TLI 73: hláška musí povedať PREČO nie sú voľné. Richard 20.10.
            // narazil presne na toto — všetky tri orámované bunky boli biele
            // dlaždice so štetcom (teda voľné), appka ich čítala ako obsadené
            // a z hlášky sa to nedalo odlíšiť od naozaj plného plátna.
            // Kategórie sú VZÁJOMNE VÝLUČNÉ — nedostupná a zároveň zamrznutá
            // bunka sa inak započíta dvakrát (nález z review diffu 20.10.2026).
            let oramovane = stav.oramovane_bunky();
            let nedostupne = oramovane.iter().filter(|b| !b.dostupna).count();
            let zamrznute =
                oramovane.iter().filter(|b| b.dostupna && b.je_zamrznuta()).count();
            let obsadene = oramovane
                .iter()
                .filter(|b| b.dostupna && !b.je_zamrznuta() && b.paint_na_bunke().is_some())
                .count();
            // Tvar „nálepka: počet" je zámerný — vyhne sa slovenskému
            // skloňovaniu podľa čísla a číta sa ako súpis. Musí sedieť
            // ZNAK PO ZNAKU s `canvas_solver.py`.
            let mut dovody: Vec<String> = vec![];
            if obsadene > 0 {
                dovody.push(format!("obsadených: {}", obsadene));
            }
            if zamrznute > 0 {
                dovody.push(format!("zamrznutých ľadom: {}", zamrznute));
            }
            if nedostupne > 0 {
                dovody.push(format!("nedostupných (vír): {}", nedostupne));
            }
            let preco = if dovody.is_empty() {
                "dôvod sa z rozpoznaného plátna nedá určiť".to_string()
            } else {
                dovody.join(", ")
            };
            skontroluj.push(format!(
                "hra orámovala {} {}, ale ani jedna z nich nie je voľná ({}) — na plátne \
                 nie je kam položiť ďalší paint",
                oramovane.len(),
                mn_buniek(oramovane.len()),
                preco
            ));
        } else if ciel.kluc_riadok.is_none() || ciel.kluc_stlpec.is_none() {
            skontroluj.push(format!(
                "appka na snímke nenašla žltý rám hry a cieľ smeru '{}' nemá \
                 kluc_riadok/kluc_stlpec (bunku s výkričníkom), takže dosah nie je z čoho odvodiť",
                ciel.smer
            ));
        } else {
            skontroluj.push(format!(
                "v dosahu smeru '{}' od bunky r{}c{} nie je žiadna voľná bunka",
                ciel.smer,
                ciel.kluc_riadok.unwrap_or(0),
                ciel.kluc_stlpec.unwrap_or(0)
            ));
        }
        return VysledokSolvera {
            kroky: vec![],
            istota: 0.0,
            skontroluj,
            zdroj_dosahu: zdroj_dosahu.nazov().to_string(),
            prinos_na_max: 0,
            skore_pred,
        };
    }

    let kategoria = kategoria_z_popisu(&ciel.popis);
    let pocet_levelov = pocet_levelov_efektu(&ciel.efekt_popis);

    // Painty z ruky zoradené tak, aby sekvencia maximalizovala VÝSLEDNÉ levely
    // (`min(level + N, max_level)`). Pri zhode ide prv NIŽŠÍ vstupný level —
    // z boostu sa tak premrhá menej. Do TLI 69 sa radilo opačne (najnižší prv),
    // čo cieľ splnilo, ale na doske nechávalo najslabšie možné painty.
    let mut sloty: Vec<(i64, &Paint)> = stav
        .ruka
        .iter()
        .filter_map(|s| s.paint.as_ref().filter(|p| p.kategoria == kategoria).map(|p| (s.index, p)))
        .collect();
    sloty.sort_by_key(|(index, paint)| {
        (-vysledny_level(paint.level, pocet_levelov, stav.max_level), paint.level, *index)
    });

    let kluc_bunka = match (ciel.kluc_riadok, ciel.kluc_stlpec) {
        (Some(r), Some(c)) => stav.bunka(r, c),
        _ => None,
    };
    let kluc_popis = match kluc_bunka.and_then(|b| b.paint.as_ref()) {
        Some(p) => format!("{} Lv{}", p.typ, p.level),
        None => "kľúčový paint".to_string(),
    };
    let nazov_kategorie =
        if kategoria == "gameplay" { "Gameplay Paintov" } else { "Monster Paintov" };

    let pocet = (potrebne as usize).min(volne.len()).min(sloty.len());

    // TLI 72: čo sekvencia prinesie z pohľadu HLAVNÉHO kritéria. Keď žiadny
    // dúhový paint a beh sa navyše blíži ku koncu, cieľ sa oplatí skôr
    // prepísať ([Reimagine]) — splnený cieľ, ktorý nič dúhové nevyrobí, len
    // minie posledné painty behu.
    let prinos = prinos_na_max(stav, kategoria, pocet_levelov, pocet as i64);
    let zostava = stav.zostava_paintov();
    let veta_prinos = if pocet > 0 {
        if prinos > 0 {
            format!(
                " Hlavné kritérium: z tejto sekvencie vypadne {} {} na Max Paint Level \
                 Lv{} (dúhový).",
                prinos,
                if prinos == 1 { "paint" } else { "paintov" },
                stav.max_level
            )
        } else {
            let najvyssi = sloty
                .iter()
                .map(|(_, p)| vysledny_level(p.level, pocet_levelov, stav.max_level))
                .max()
                .unwrap_or(0);
            let mut veta = format!(
                " Hlavné kritérium: z tejto sekvencie nevypadne ani jeden paint na Max Paint \
                 Level Lv{} — najvyšší výsledok z ruky je Lv{}.",
                stav.max_level, najvyssi
            );
            if let Some(z) = zostava.filter(|z| *z > 0) {
                veta.push_str(&format!(
                    " Do konca behu zostáva {z} paintov, takže cieľ sa oplatí skôr prepísať \
                     ([Reimagine]) než dokončiť."
                ));
            }
            veta
        }
    } else {
        String::new()
    };
    if let Some(z) = zostava {
        if pocet > 0 && z < pocet as i64 {
            skontroluj.push(format!(
                "počítadlo hry hovorí, že do konca behu zostáva {z} paintov, ale sekvencia ich \
                 chce položiť {pocet} — toľko krokov sa už nemusí zmestiť"
            ));
        }
    }
    if (pocet as i64) < potrebne {
        let chyba = if (volne.len() as i64) < potrebne {
            "voľných cieľových políčok".to_string()
        } else {
            format!("{nazov_kategorie} v ruke")
        };
        skontroluj.push(format!(
            "cieľ žiada {} umiestnení, ale k dispozícii je len {} ({}) — vrátená sekvencia cieľ nedokončí celý",
            potrebne, pocet, chyba
        ));
    }

    let mut kroky: Vec<Krok> = vec![];
    for i in 0..pocet {
        let (slot_index, paint) = sloty[i];
        let bunka: &Bunka = volne[i];
        let po_umiestneni = vysledny_level(paint.level, pocet_levelov, stav.max_level);
        if paint.level >= stav.max_level {
            skontroluj.push(format!(
                "slot {} ({} Lv{}) je už na max_level {} — Color Mixing Effect naň podľa pozorovania \
                 nemá kam zvýšiť level, umiestnenie samo cieľ 'Fill' aj tak splní",
                slot_index, paint.typ, paint.level, stav.max_level
            ));
        }
        let kde = if zdroj_dosahu == ZdrojDosahu::Ram {
            format!(
                "na bunku (riadok {}, stĺpec {}), ktorú hra žlto orámovala",
                bunka.riadok, bunka.stlpec
            )
        } else {
            format!(
                "na bunku (riadok {}, stĺpec {}) v smere '{}' od {} (r{}c{})",
                bunka.riadok,
                bunka.stlpec,
                ciel.smer,
                kluc_popis,
                ciel.kluc_riadok.unwrap_or(0),
                ciel.kluc_stlpec.unwrap_or(0)
            )
        };
        let mut dovod = format!(
            "Vezmi {} Lv{} zo slotu {} v ruke a polož ho {} — splní krok {}/{} cieľa '{}' \
             a umiestnený paint vyskočí na Lv{} ({}). {}",
            paint.typ,
            paint.level,
            slot_index,
            kde,
            ciel.pocet_aktualny + i as i64 + 1,
            ciel.pocet_pozadovany,
            ciel.popis,
            po_umiestneni,
            ciel.efekt_popis,
            dovod_vyberu(&sloty, i, pocet, pocet_levelov, stav.max_level, &kategoria),
        );
        // Pri prvom kroku raz povedz, že TYP monštra na splnenie cieľa nerozhoduje —
        // presne na tom sa Richard 20.10.2026 zasekol („prečo nie rovnaký typ ako
        // zdroj?"). Hovorí to až vtedy, keď sa typy naozaj líšia, nech to pri
        // zhodnom type zbytočne nezavadzia. Konkrétny dôkaz (dwarf → ratlin+pest,
        // 0/3 → 2/3) sem ZÁMERNE nepatrí: na Richardovej vlastnej snímke je v
        // kľúčovej bunke agito, takže veta o „kľúčovom dwarfovi" by hovorila
        // o monštre, ktoré na doske nie je — a zmätočné zdôvodnenie je horšie
        // než žiadne. Dôkaz je v `canvas.yml` → `typ_paintu.zdroj` (repo `ops`).
        if i == 0 {
            if let Some(kluc_paint) = kluc_bunka.and_then(|b| b.paint.as_ref()) {
                if kluc_paint.typ != paint.typ {
                    dovod.push_str(&format!(
                        " Na type nezáleží — cieľ aj efekt počítajú len kategóriu \
                         ({} Paint), nie zhodu typu so zdrojom (overené z páru pred/po \
                         19.10.2026, smer adjacent).",
                        if kategoria == "gameplay" { "Gameplay" } else { "Monster" }
                    ));
                }
            }
            // Až ZA vetou o type — rovnaké poradie ako na serveri, inak by sa
            // reťazce rozišli.
            dovod.push_str(&veta_prinos);
        }
        kroky.push(Krok {
            poradie: i as i64 + 1,
            zdroj_slot: slot_index,
            ciel_riadok: bunka.riadok,
            ciel_stlpec: bunka.stlpec,
            dovod,
        });
    }

    // Istota: dosah zo žltého rámu je pozorovanie z obrazovky, dosah odvodený
    // z nepotvrdeného mena smeru je dohad — preto nižšie číslo. Neoverený EFEKT
    // istotu znižuje tiež, ale radu NEPOTLÁČA (zadanie TLI 69, bod 4).
    let smer_maly = ciel.smer.trim().to_ascii_lowercase();
    let efekt_overeny = SMERY_S_OVERENYM_EFEKTOM.contains(&smer_maly.as_str());
    // `adjacent` má dosah overený aj BEZ rámu (zo screenshotov 15.10.2026), takže
    // mu istota nemá klesať len preto, že hráč stlačil F9 v podobrazovke, kde hra
    // rám nekreslí — a to je 45 zo 75 reálnych snímok. Klesá len tam, kde je
    // dosah naozaj odvodený z nepotvrdeného mena smeru.
    let mut istota = if pocet as i64 == potrebne {
        if zdroj_dosahu == ZdrojDosahu::Ram || efekt_overeny {
            0.9
        } else {
            0.7
        }
    } else if pocet > 0 {
        0.5
    } else {
        0.0
    };
    if !efekt_overeny {
        istota = (istota * 0.8 * 10_000.0_f64).round() / 10_000.0;
        skontroluj.push(if zdroj_dosahu == ZdrojDosahu::Ram {
            format!(
                "dosah je istý (hra bunky sama orámovala), ale EFEKT pre smer '{}' zatiaľ nie je \
                 overený z párov pred/po — o koľko levelov paint naozaj vyskočí, berie appka \
                 z textu hry ('{}')",
                ciel.smer, ciel.efekt_popis
            )
        } else {
            format!(
                "pre smer '{}' nie je overené ani umiestnenie (appka na snímke nenašla žltý rám \
                 hry a dosah odvodila z názvu smeru), ani efekt — rada je návrh, nie istota",
                ciel.smer
            )
        });
    }

    VysledokSolvera {
        kroky,
        istota,
        skontroluj,
        zdroj_dosahu: zdroj_dosahu.nazov().to_string(),
        prinos_na_max: prinos,
        skore_pred,
    }
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
    // Na tom istom screenshote hra ŽLTO ORÁMOVALA práve tieto štyri bunky
    // (TLI 69) — fixture to teraz nesie, lebo to na snímke reálne bolo.
    const ORAMOVANE_ZO_SCREENSHOTU: [(i64, i64); 4] = [(3, 3), (4, 2), (4, 4), (5, 3)];

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
                        oramovana: false,
                        zamrznuta: false,
                    });
                } else if volne.contains(&(r, c)) {
                    bunky.push(Bunka {
                        riadok: r,
                        stlpec: c,
                        dostupna: true,
                        paint: None,
                        oznacena_vykricnikom: false,
                        oramovana: ORAMOVANE_ZO_SCREENSHOTU.contains(&(r, c)),
                        zamrznuta: false,
                    });
                } else {
                    bunky.push(Bunka {
                        riadok: r,
                        stlpec: c,
                        dostupna: false,
                        paint: None,
                        oznacena_vykricnikom: false,
                        oramovana: false,
                        zamrznuta: false,
                    });
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

        CanvasStav { riadky: 6, stlpce: 6, max_level: 7, bunky, ruka, ciel: Some(ciel), polozene: None, celkom: None }
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

    /// TLI 69: do 20.10.2026 tu solver vracal prázdno s vetou „smer nemá overené
    /// pravidlo". Richard to nahlásil ako chybu — dosah berie hra z vlastného
    /// žltého rámu, takže rada sa UKÁŽE pre akýkoľvek smer, len označená.
    #[test]
    fn iny_smer_uz_radu_vracia_ale_oznacenu_ako_neoverenu() {
        let mut stav = board_zo_screenshotu();
        stav.ciel.as_mut().unwrap().smer = "horizontal".to_string();
        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.len(), 3, "{v:?}");
        assert_eq!(v.zdroj_dosahu, "ram");
        assert!(v.skontroluj.iter().any(|s| s.contains("horizontal")), "{:?}", v.skontroluj);
        assert!(v.istota < 0.9, "{}", v.istota);
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
        assert!(v.skontroluj.iter().any(|s| s.contains("voľných cieľových políčok")));
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
    // ── TLI 69: dosah zo žltého rámu hry, všetky smery ───────────────────────
    //
    // Fixture je Richardova snímka z 20.10.2026 ~12:2x (analýza
    // `2026-10-20-133046-4fa90dea` na CT 114): cieľ „Fill a Vertical Cell with a
    // Monster Paint(0/3)", zdrojové monštrum Lv3 so značkou „!" v r6c3, hra žlto
    // orámovala r1c3, r2c3, r4c3 a r5c3 — riadok 3 je ľad (blokovaný). Testy
    // sú ZÁMERNE 1:1 s `testy-canvas-solver.py` (15.–27.) v repe `ops`, nech sa
    // dá lokálny a serverový solver priamo porovnať na tom istom vstupe.

    const ZDROJ_VERTICAL: (i64, i64) = (6, 3);
    const ORAMOVANE_VERTICAL: [(i64, i64); 4] = [(1, 3), (2, 3), (4, 3), (5, 3)];

    fn board_vertical(oramovat: bool, kategoria: &str) -> CanvasStav {
        let mut bunky = vec![];
        for r in 1..=6 {
            for c in 1..=6 {
                if (r, c) == ZDROJ_VERTICAL {
                    bunky.push(Bunka {
                        riadok: r,
                        stlpec: c,
                        dostupna: true,
                        paint: Some(Paint::monster("dwarf", 3)),
                        oznacena_vykricnikom: true,
                        oramovana: false,
                        zamrznuta: false,
                    });
                } else if ORAMOVANE_VERTICAL.contains(&(r, c)) {
                    bunky.push(Bunka {
                        riadok: r,
                        stlpec: c,
                        dostupna: true,
                        paint: None,
                        oznacena_vykricnikom: false,
                        oramovana: oramovat,
                        zamrznuta: false,
                    });
                } else {
                    // r3c3 je ľad, zvyšok plátna mimo aktívnej plochy
                    bunky.push(Bunka {
                        riadok: r,
                        stlpec: c,
                        dostupna: false,
                        paint: None,
                        oznacena_vykricnikom: false,
                        oramovana: false,
                        zamrznuta: false,
                    });
                }
            }
        }
        let ruka: Vec<SlotRuky> = [1i64, 2, 5, 7, 3]
            .iter()
            .enumerate()
            .map(|(i, lvl)| SlotRuky {
                index: i as i64,
                paint: Some(Paint {
                    kategoria: kategoria.to_string(),
                    typ: "ghost".to_string(),
                    level: *lvl,
                }),
            })
            .collect();
        let nazov = if kategoria == "gameplay" { "Gameplay" } else { "Monster" };
        let ciel = CielMiesania {
            smer: "vertical".to_string(),
            popis: format!("Fill a Vertical Cell with a {nazov} Paint(0/3)"),
            efekt_popis: format!("Level up the Vertical {nazov} Paint by 1 level(s)"),
            pocet_pozadovany: 3,
            pocet_aktualny: 0,
            kluc_riadok: Some(ZDROJ_VERTICAL.0),
            kluc_stlpec: Some(ZDROJ_VERTICAL.1),
        };
        CanvasStav { riadky: 6, stlpce: 6, max_level: 7, bunky, ruka, ciel: Some(ciel), polozene: None, celkom: None }
    }

    #[test]
    fn vertical_zo_snimky_vrati_tri_kroky_do_oramovanych_buniek() {
        let v = navrhni_kroky(&board_vertical(true, "monster"));
        assert_eq!(v.kroky.len(), 3, "{v:?}");
        assert_eq!(v.zdroj_dosahu, "ram");
        let ciele: Vec<(i64, i64)> = v.kroky.iter().map(|k| (k.ciel_riadok, k.ciel_stlpec)).collect();
        for c in &ciele {
            assert!(ORAMOVANE_VERTICAL.contains(c), "{c:?}");
        }
        let mut zoradene = ciele.clone();
        zoradene.sort();
        assert_eq!(ciele, zoradene, "poradie cieľov má byť deterministické");
    }

    #[test]
    fn vertical_rada_je_oznacena_ako_neoverena_ale_existuje() {
        let v = navrhni_kroky(&board_vertical(true, "monster"));
        assert!(!v.kroky.is_empty(), "zadanie TLI 69 bod 4: radu UKÁŽ, nie „žiadny krok“");
        assert!(v.skontroluj.iter().any(|s| s.contains("vertical")), "{:?}", v.skontroluj);
        assert!(v.istota > 0.0 && v.istota < 0.9, "{}", v.istota);
    }

    /// Text smeru slúži už len na kontrolu — keď hra orámuje stĺpec a text
    /// tvrdí „adjacent", platí rám. (r5c3 je jediná ortogonálna susedka r6c3,
    /// takže podľa mena smeru by krok bol jeden; s rámom sú tri.)
    #[test]
    fn ram_prebije_nazov_smeru() {
        let mut stav = board_vertical(true, "monster");
        {
            let c = stav.ciel.as_mut().unwrap();
            c.smer = "adjacent".to_string();
            c.popis = "Fill a Adjacent Cell with a Monster Paint(0/3)".to_string();
        }
        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.len(), 3, "{v:?}");
        for k in &v.kroky {
            assert!(ORAMOVANE_VERTICAL.contains(&(k.ciel_riadok, k.ciel_stlpec)));
        }
    }

    #[test]
    fn obsadena_oramovana_bunka_nie_je_ciel() {
        let mut stav = board_vertical(true, "monster");
        stav.bunky.iter_mut().find(|b| b.riadok == 1 && b.stlpec == 3).unwrap().paint =
            Some(Paint::monster("gang", 2));
        let v = navrhni_kroky(&stav);
        let mut ciele: Vec<(i64, i64)> =
            v.kroky.iter().map(|k| (k.ciel_riadok, k.ciel_stlpec)).collect();
        ciele.sort();
        assert_eq!(ciele, vec![(2, 3), (4, 3), (5, 3)]);
        // rám na obsadenej bunke ostáva
        assert!(stav.bunka(1, 3).unwrap().oramovana);
    }

    #[test]
    fn bez_ramu_sa_dosah_odvodi_zo_smeru_a_istota_klesne() {
        let s_ram = navrhni_kroky(&board_vertical(true, "monster"));
        let s_bez = navrhni_kroky(&board_vertical(false, "monster"));
        assert_eq!(s_bez.zdroj_dosahu, "smer");
        assert_eq!(s_bez.kroky.len(), 3, "{s_bez:?}");
        let ciele_ram: Vec<(i64, i64)> =
            s_ram.kroky.iter().map(|k| (k.ciel_riadok, k.ciel_stlpec)).collect();
        let ciele_bez: Vec<(i64, i64)> =
            s_bez.kroky.iter().map(|k| (k.ciel_riadok, k.ciel_stlpec)).collect();
        assert_eq!(ciele_ram, ciele_bez);
        assert!(s_bez.istota < s_ram.istota);
    }

    #[test]
    fn gameplay_ciel_berie_gameplay_painty() {
        let stav = board_vertical(true, "gameplay");
        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.len(), 3, "{v:?}");
        for k in &v.kroky {
            let slot = stav.ruka.iter().find(|s| s.index == k.zdroj_slot).unwrap();
            assert_eq!(slot.paint.as_ref().unwrap().kategoria, "gameplay");
        }
    }

    #[test]
    fn vyber_paintov_maximalizuje_vysledne_levely() {
        let stav = board_vertical(true, "monster");
        let v = navrhni_kroky(&stav);
        let levely: Vec<i64> = v
            .kroky
            .iter()
            .map(|k| {
                stav.ruka.iter().find(|s| s.index == k.zdroj_slot).unwrap().paint.as_ref().unwrap().level
            })
            .collect();
        assert_eq!(levely, vec![7, 5, 3], "{levely:?}");
    }

    #[test]
    fn pocet_levelov_sa_cita_z_textu_efektu() {
        assert_eq!(pocet_levelov_efektu("Level up the Vertical Monster Paint by 2 level(s)"), 2);
        assert_eq!(pocet_levelov_efektu("Level up the Adjacent Monster Paint by 1 level(s)"), 1);
        assert_eq!(pocet_levelov_efektu(""), 1);
        assert_eq!(pocet_levelov_efektu("nezmysel bez cisla"), 1);
        assert_eq!(pocet_levelov_efektu("by 99 level(s)"), 1); // mimo rozsahu 1..10
        // "by 3" bez slova level sa nepočíta; platné "by 2 level" o kus ďalej áno
        assert_eq!(pocet_levelov_efektu("increased by 3 and leveled up by 2 level(s)"), 2);
    }

    #[test]
    fn bunky_v_smere_pre_vsetky_smery() {
        use crate::canvas::bunky_v_smere;
        let zoradene = |mut v: Vec<(i64, i64)>| {
            v.sort();
            v
        };
        assert_eq!(zoradene(bunky_v_smere("adjacent", 4, 3, 6, 6)),
                   vec![(3, 3), (4, 2), (4, 4), (5, 3)]);
        assert_eq!(zoradene(bunky_v_smere("vertical", 6, 3, 6, 6)),
                   vec![(1, 3), (2, 3), (3, 3), (4, 3), (5, 3)]);
        assert_eq!(zoradene(bunky_v_smere("horizontal", 3, 4, 6, 6)),
                   vec![(3, 1), (3, 2), (3, 3), (3, 5), (3, 6)]);
        assert_eq!(zoradene(bunky_v_smere("diagonal", 2, 2, 6, 6)),
                   vec![(1, 1), (1, 3), (3, 1), (3, 3), (4, 4), (5, 5), (6, 6)]);
        assert!(bunky_v_smere("neznamy", 3, 3, 6, 6).is_empty());
        for smer in ["adjacent", "vertical", "horizontal", "diagonal", "cross"] {
            assert!(!bunky_v_smere(smer, 3, 3, 6, 6).contains(&(3, 3)), "{smer}");
        }
    }

    #[test]
    fn kategoria_z_popisu_rozlisi_gameplay_a_monster() {
        use crate::canvas::kategoria_z_popisu;
        assert_eq!(kategoria_z_popisu("Fill a Vertical Cell with a Gameplay Paint(0/3)"), "gameplay");
        assert_eq!(kategoria_z_popisu("Fill a Adjacent Cell with a Monster Paint"), "monster");
        assert_eq!(kategoria_z_popisu(""), "monster");
    }

    /// `adjacent` má dosah overený aj bez rámu (screenshoty 15.10.2026), takže
    /// istota nemá klesať len preto, že hra na danej podobrazovke rám nekreslí.
    #[test]
    fn adjacent_bez_ramu_si_drzi_plnu_istotu() {
        let mut stav = board_zo_screenshotu();
        for b in stav.bunky.iter_mut() {
            b.oramovana = false;
        }
        let v = navrhni_kroky(&stav);
        assert_eq!(v.zdroj_dosahu, "smer");
        assert_eq!(v.kroky.len(), 3, "{v:?}");
        assert_eq!(v.istota, 0.9, "{}", v.istota);
    }

    #[test]
    fn oramovane_ale_ziadna_volna_bunka_to_povie_zrozumitelne() {
        let mut stav = board_vertical(true, "monster");
        for (r, c) in ORAMOVANE_VERTICAL {
            stav.bunky.iter_mut().find(|b| b.riadok == r && b.stlpec == c).unwrap().paint =
                Some(Paint::monster("gang", 1));
        }
        let v = navrhni_kroky(&stav);
        assert!(v.kroky.is_empty());
        assert!(v.skontroluj.iter().any(|s| s.contains("orámovala")), "{:?}", v.skontroluj);
    }

    // ── TLI 69b: „PREČO práve tento paint" ───────────────────────────────────
    //
    // Zrkadlí `test_28`–`test_34` v `testy-canvas-solver.py` (repo `ops`).
    // Fixture je REÁLNA Richardova snímka z 20.10.2026 13:44 („PREČO CHCE
    // ZOBRAŤ LEVEL 1 A NIE TIE LVL 2?").

    const NEDOSTUPNE_134419: [(i64, i64); 10] =
        [(1, 5), (1, 6), (2, 1), (3, 2), (4, 6), (5, 1), (5, 6), (6, 1), (6, 2), (6, 6)];
    const ZDROJ_134419: (i64, i64) = (2, 5);
    const LAD_134419: [(i64, i64); 4] = [(1, 3), (3, 3), (4, 3), (5, 3)];
    const ORAMOVANE_134419: [(i64, i64); 3] = [(2, 4), (2, 6), (3, 5)];

    fn board_richard_134419() -> CanvasStav {
        let mut bunky = vec![];
        for r in 1..=6 {
            for c in 1..=6 {
                let paint = if (r, c) == ZDROJ_134419 {
                    Some(Paint::monster("agito", 3))
                } else if LAD_134419.contains(&(r, c)) {
                    Some(Paint {
                        kategoria: "gameplay".to_string(),
                        typ: "ice_block".to_string(),
                        level: 1,
                    })
                } else {
                    None
                };
                bunky.push(Bunka {
                    riadok: r,
                    stlpec: c,
                    dostupna: !NEDOSTUPNE_134419.contains(&(r, c)),
                    paint,
                    oznacena_vykricnikom: (r, c) == ZDROJ_134419,
                    oramovana: ORAMOVANE_134419.contains(&(r, c)),
                    zamrznuta: false,
                });
            }
        }
        let ruka_def: [(&str, &str, i64); 15] = [
            ("monster", "test_subject", 2),
            ("monster", "dwarf", 2),
            ("monster", "agito", 2),
            ("monster", "pest", 1),
            ("monster", "dragonbone", 1),
            ("monster", "aember", 1),
            ("monster", "einherjar", 1),
            ("monster", "gang", 1),
            ("monster", "ichi", 1),
            ("monster", "sand_monster", 1),
            ("monster", "ghost", 1),
            ("gameplay", "hourglass", 1),
            ("gameplay", "hourglass", 1),
            ("gameplay", "chest", 1),
            ("gameplay", "statue", 1),
        ];
        let ruka = ruka_def
            .iter()
            .enumerate()
            .map(|(i, (kat, typ, lvl))| SlotRuky {
                index: i as i64,
                paint: Some(Paint { kategoria: kat.to_string(), typ: typ.to_string(), level: *lvl }),
            })
            .collect();
        let ciel = CielMiesania {
            smer: "adjacent".to_string(),
            popis: "Fill a Adjacent Cell with a Monster Paint".to_string(),
            efekt_popis: "Level up the Adjacent Monster Paint by 1 level(s)".to_string(),
            pocet_pozadovany: 3,
            pocet_aktualny: 0,
            kluc_riadok: Some(ZDROJ_134419.0),
            kluc_stlpec: Some(ZDROJ_134419.1),
        };
        CanvasStav { riadky: 6, stlpce: 6, max_level: 7, bunky, ruka, ciel: Some(ciel), polozene: None, celkom: None }
    }

    fn sloty_a_ciele(v: &VysledokSolvera) -> (Vec<i64>, HashSet<(i64, i64)>) {
        (
            v.kroky.iter().map(|k| k.zdroj_slot).collect(),
            v.kroky.iter().map(|k| (k.ciel_riadok, k.ciel_stlpec)).collect(),
        )
    }

    fn s_rukou(mut stav: CanvasStav, levely: &[i64]) -> CanvasStav {
        stav.ruka = levely
            .iter()
            .enumerate()
            .map(|(i, lvl)| SlotRuky { index: i as i64, paint: Some(Paint::monster("ghost", *lvl)) })
            .collect();
        stav
    }

    /// test_28: berú sa Lv2 painty (sloty 0,1,2), nie Lv1 (3,4,5).
    #[test]
    fn richard_134419_berie_lv2_sloty_do_oramovanych_buniek() {
        let v = navrhni_kroky(&board_richard_134419());
        assert_eq!(v.zdroj_dosahu, "ram");
        let (sloty, ciele) = sloty_a_ciele(&v);
        assert_eq!(sloty, vec![0, 1, 2], "{v:?}");
        assert_eq!(ciele, ORAMOVANE_134419.iter().copied().collect::<HashSet<_>>());
    }

    /// test_29: bez rámu (dosah zo smeru `adjacent`) vyjde to isté.
    #[test]
    fn richard_134419_bez_ramu_vyjde_rovnako() {
        let mut stav = board_richard_134419();
        for b in stav.bunky.iter_mut() {
            b.oramovana = false;
        }
        let v = navrhni_kroky(&stav);
        assert_eq!(v.zdroj_dosahu, "smer");
        let (sloty, ciele) = sloty_a_ciele(&v);
        assert_eq!(sloty, vec![0, 1, 2], "{v:?}");
        assert_eq!(ciele, ORAMOVANE_134419.iter().copied().collect::<HashSet<_>>());
    }

    /// test_30: každý krok vysvetlí, prečo Lv2 a nie Lv1.
    #[test]
    fn richard_134419_dovod_vysvetli_vyber_lv2() {
        let v = navrhni_kroky(&board_richard_134419());
        assert_eq!(v.kroky.len(), 3);
        for k in &v.kroky {
            assert!(k.dovod.contains("Prečo tento: Lv2 → Lv3"), "{}", k.dovod);
            assert!(k.dovod.contains("najvyšší výsledok"), "{}", k.dovod);
            assert!(k.dovod.contains("najviac na Lv2"), "{}", k.dovod);
        }
    }

    /// Kontrola reťazca znak po znaku (port musí sedieť so serverom presne).
    #[test]
    fn richard_134419_prvy_dovod_presne_ako_server() {
        let v = navrhni_kroky(&board_richard_134419());
        assert_eq!(
            v.kroky[0].dovod,
            "Vezmi test_subject Lv2 zo slotu 0 v ruke a polož ho na bunku (riadok 2, stĺpec 4), \
             ktorú hra žlto orámovala — splní krok 1/3 cieľa 'Fill a Adjacent Cell with a Monster \
             Paint' a umiestnený paint vyskočí na Lv3 (Level up the Adjacent Monster Paint by 1 \
             level(s)). Prečo tento: Lv2 → Lv3 je najvyšší výsledok, aký sa z ruky dá dosiahnuť — \
             slabšie Monster Painty v ruke by skončili najviac na Lv2. Na type nezáleží — cieľ aj \
             efekt počítajú len kategóriu (Monster Paint), nie zhodu typu so zdrojom (overené \
             z páru pred/po 19.10.2026, smer adjacent). Hlavné kritérium: z tejto sekvencie \
             nevypadne ani jeden paint na Max Paint Level Lv7 — najvyšší výsledok z ruky je Lv3."
        );
    }

    /// test_31: veta o type monštra je LEN v prvom kroku.
    #[test]
    fn veta_o_type_je_len_v_prvom_kroku() {
        let v = navrhni_kroky(&board_richard_134419());
        assert_eq!(v.kroky.len(), 3);
        assert!(v.kroky[0].dovod.contains("Na type nezáleží"), "{}", v.kroky[0].dovod);
        for k in &v.kroky[1..] {
            assert!(!k.dovod.contains("Na type nezáleží"), "{}", k.dovod);
        }
    }

    /// test_32: keď sa vezme NIŽŠÍ level, musí byť napísané prečo.
    #[test]
    fn nizsi_vyber_vysvetli_ze_vyssie_vzali_predosle_kroky() {
        let v = navrhni_kroky(&board_vertical(true, "monster"));
        assert_eq!(v.kroky.len(), 3);
        assert!(v.kroky[0].dovod.contains("najvyšší výsledok"), "{}", v.kroky[0].dovod);
        for k in &v.kroky[1..] {
            assert!(k.dovod.contains("si vzali predošlé kroky"), "{}", k.dovod);
            assert!(k.dovod.contains("najvyšší zvyšný"), "{}", k.dovod);
        }
    }

    /// test_33: strop sa spomína len keď silnejší paint naozaj ostane v ruke.
    #[test]
    fn strop_sa_spomina_len_ked_silnejsi_ostava_v_ruke() {
        let v = navrhni_kroky(&s_rukou(board_vertical(true, "monster"), &[6, 7, 7, 7]));
        assert_eq!(v.kroky.len(), 3);
        assert!(v.kroky[0].dovod.contains("silnejší ostáva v ruke"), "{}", v.kroky[0].dovod);

        let v = navrhni_kroky(&s_rukou(board_vertical(true, "monster"), &[6, 7, 7]));
        assert_eq!(v.kroky.len(), 3);
        for k in &v.kroky {
            assert!(!k.dovod.contains("silnejší ostáva v ruke"), "{}", k.dovod);
        }
    }

    /// test_34: pri ruke samých Lv1 nie je z čoho vyberať.
    ///
    /// TLI 73: tento test v Ruste CHÝBAL — pri TLI 69b sa jeho doc komentár
    /// aj `#[test]` prilepili na nasledujúcu funkciu (duplicitný atribút,
    /// `cargo test` to hlásil ako warning) a samotné telo sa nenapísalo.
    #[test]
    fn rovnaka_ruka_dokola_povie_ze_vyssie_nejde() {
        let stav = s_rukou(board_vertical(true, "monster"), &[1, 1, 1, 1, 1]);
        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.len(), 3);
        for k in &v.kroky {
            assert!(k.dovod.contains("vyšší výsledok z tejto ruky nejde"), "{}", k.dovod);
        }
    }

    /// test_31b: pri zhodnom type sa veta o type nepíše.
    #[test]
    fn pri_zhodnom_type_sa_veta_o_type_nepise() {
        // Nález z review 20.10.2026: veta nesmie citovať konkrétne monštrum,
        // ktoré na doske nie je (pôvodne hovorila o „kľúčovom dwarfovi", hoci
        // Richardov zdroj je agito), a pri zhodnom type nemá byť vôbec.
        let mut stav = board_richard_134419();
        stav.ruka[0] = SlotRuky { index: 0, paint: Some(Paint {
            kategoria: "monster".to_string(), typ: "agito".to_string(), level: 2 }) };
        let v = navrhni_kroky(&stav);
        assert!(!v.kroky[0].dovod.contains("Na type nezáleží"), "{}", v.kroky[0].dovod);
    }

    #[test]
    fn veta_o_type_necituje_konkretne_monstrum() {
        let v = navrhni_kroky(&board_richard_134419());
        assert!(v.kroky[0].dovod.contains("nie zhodu typu so zdrojom"), "{}", v.kroky[0].dovod);
        assert!(!v.kroky[0].dovod.contains("dwarf"), "{}", v.kroky[0].dovod);
    }

    #[test]
    fn gameplay_ciel_pouziva_gameplay_nazvy() {
        let v = navrhni_kroky(&board_vertical(true, "gameplay"));
        assert!(v.kroky[0].dovod.contains("Gameplay Painty"), "{}", v.kroky[0].dovod);
        assert!(v.kroky[0].dovod.contains("(Gameplay Paint)"), "{}", v.kroky[0].dovod);
    }

    #[test]
    fn rovnaka_ruka_povie_ze_vyssi_vysledok_nejde() {
        let v = navrhni_kroky(&s_rukou(board_vertical(true, "monster"), &[1, 1, 1, 1, 1]));
        assert_eq!(v.kroky.len(), 3);
        for k in &v.kroky {
            assert!(k.dovod.contains("vyšší výsledok z tejto ruky nejde"), "{}", k.dovod);
        }
    }

    // ── TLI 72: cieľová funkcia — dúhové painty sú hlavné kritérium ──────────
    //
    // Zrkadlí testy TLI 72 v `testy-canvas-solver.py` (repo `ops`); vety sa
    // kontrolujú ZNAK PO ZNAKU, lebo obe strany musia hráčovi povedať to isté.

    fn s_pocitadlom(mut stav: CanvasStav, polozene: i64, celkom: i64) -> CanvasStav {
        stav.polozene = Some(polozene);
        stav.celkom = Some(celkom);
        stav
    }

    /// Ruka [1, 2, 5, 7, 3] s +1: jediný dúhový je Lv7 → Lv7. Prvý krok musí
    /// mať VŠETKY vety v poradí servera: výber → dúhový krok → typ → prínos.
    #[test]
    fn prvy_dovod_s_prinosom_presne_ako_server() {
        let v = navrhni_kroky(&board_vertical(true, "monster"));
        assert_eq!(v.prinos_na_max, 1);
        assert_eq!(
            v.kroky[0].dovod,
            "Vezmi ghost Lv7 zo slotu 3 v ruke a polož ho na bunku (riadok 1, stĺpec 3), \
             ktorú hra žlto orámovala — splní krok 1/3 cieľa 'Fill a Vertical Cell with a Monster \
             Paint(0/3)' a umiestnený paint vyskočí na Lv7 (Level up the Vertical Monster Paint by 1 \
             level(s)). Prečo tento: Lv7 → Lv7 je najvyšší výsledok, aký sa z ruky dá dosiahnuť — \
             slabšie Monster Painty v ruke by skončili najviac na Lv6. Tento krok vyrobí paint na \
             Max Paint Level Lv7 (dúhový) — to je hlavný cieľ, počet dúhových je viac než súčet \
             levelov. Na type nezáleží — cieľ aj efekt počítajú len kategóriu (Monster Paint), nie \
             zhodu typu so zdrojom (overené z páru pred/po 19.10.2026, smer adjacent). Hlavné \
             kritérium: z tejto sekvencie vypadne 1 paint na Max Paint Level Lv7 (dúhový)."
        );
        // Lv5 → Lv6 dúhový nie je — veta o dúhovom kroku tam byť nesmie
        assert!(!v.kroky[1].dovod.contains("Tento krok vyrobí"), "{}", v.kroky[1].dovod);
        // veta o prínose patrí len k prvému kroku
        for k in &v.kroky[1..] {
            assert!(!k.dovod.contains("Hlavné kritérium"), "{}", k.dovod);
        }
    }

    #[test]
    fn prinos_viac_paintov_ma_mnozne_cislo() {
        let v = navrhni_kroky(&s_rukou(board_vertical(true, "monster"), &[6, 6, 7, 1]));
        assert_eq!(v.prinos_na_max, 3);
        assert!(
            v.kroky[0].dovod.ends_with(
                " Hlavné kritérium: z tejto sekvencie vypadne 3 paintov na Max Paint Level Lv7 (dúhový)."
            ),
            "{}",
            v.kroky[0].dovod
        );
    }

    /// Veta o dúhovom kroku sa pripája aj k vetve „vyšší výsledok nejde" a
    /// veta o strope ide až ZA ňou — poradie musí sedieť so serverom.
    #[test]
    fn duhovy_krok_v_rovnakej_ruke_a_pred_vetou_o_strope() {
        let v = navrhni_kroky(&s_rukou(board_vertical(true, "monster"), &[6, 7, 7, 7]));
        assert!(
            v.kroky[0].dovod.contains(
                "Prečo tento: Lv6 → Lv7; vyšší výsledok z tejto ruky nejde, všetky Monster Painty \
                 v nej dajú to isté. Tento krok vyrobí paint na Max Paint Level Lv7 (dúhový) — to je \
                 hlavný cieľ, počet dúhových je viac než súčet levelov. Lv7 by na strope Lv7 skončil \
                 rovnako, preto ide prv nižší a silnejší ostáva v ruke."
            ),
            "{}",
            v.kroky[0].dovod
        );
    }

    /// Vetva „najvyšší zvyšný" nesie `na_max` len kvôli zhode so serverom —
    /// reálne je vždy prázdny: výsledok je tam pod najlepším, a ten je
    /// najviac strop, takže dúhový byť nemôže. Test stráži, že sa tam veta
    /// o dúhovom kroku omylom neobjaví.
    #[test]
    fn vetva_najvyssi_zvysny_duhovu_vetu_nema() {
        let v = navrhni_kroky(&board_vertical(true, "monster"));
        for k in &v.kroky[1..] {
            assert!(k.dovod.contains("najvyšší zvyšný."), "{}", k.dovod);
            assert!(!k.dovod.contains("dúhový"), "{}", k.dovod);
        }
    }

    /// Richardova ruka 13:44 dá najviac Lv3 pri strope Lv7 — rada musí
    /// povedať, že dúhový z toho nebude, a keď beh ešte pokračuje, že sa
    /// cieľ oplatí prepísať.
    #[test]
    fn bez_duhoveho_a_s_pocitadlom_odporuci_reimagine() {
        let v = navrhni_kroky(&s_pocitadlom(board_richard_134419(), 15, 20));
        assert_eq!(v.prinos_na_max, 0);
        assert!(
            v.kroky[0].dovod.ends_with(
                " Hlavné kritérium: z tejto sekvencie nevypadne ani jeden paint na Max Paint Level \
                 Lv7 — najvyšší výsledok z ruky je Lv3. Do konca behu zostáva 5 paintov, takže cieľ \
                 sa oplatí skôr prepísať ([Reimagine]) než dokončiť."
            ),
            "{}",
            v.kroky[0].dovod
        );
        assert!(!v.skontroluj.iter().any(|s| s.contains("počítadlo hry")), "{:?}", v.skontroluj);
    }

    /// Bez prečítaného počítadla alebo pri nulovom zvyšku sa o Reimagine
    /// nehovorí — nevieme, či beh ešte niečo dovolí.
    #[test]
    fn bez_pocitadla_alebo_na_konci_behu_sa_reimagine_nespomina() {
        for stav in [board_richard_134419(), s_pocitadlom(board_richard_134419(), 20, 20)] {
            let v = navrhni_kroky(&stav);
            assert!(v.kroky[0].dovod.ends_with("najvyšší výsledok z ruky je Lv3."), "{}", v.kroky[0].dovod);
            assert!(!v.kroky[0].dovod.contains("Reimagine"), "{}", v.kroky[0].dovod);
        }
    }

    #[test]
    fn malo_paintov_do_konca_behu_ide_do_skontroluj() {
        let v = navrhni_kroky(&s_pocitadlom(board_richard_134419(), 18, 20));
        assert_eq!(v.kroky.len(), 3);
        assert!(
            v.skontroluj.contains(
                &"počítadlo hry hovorí, že do konca behu zostáva 2 paintov, ale sekvencia ich chce \
                  položiť 3 — toľko krokov sa už nemusí zmestiť"
                    .to_string()
            ),
            "{:?}",
            v.skontroluj
        );
        // zostáva presne toľko, koľko treba → žiadne varovanie
        let v = navrhni_kroky(&s_pocitadlom(board_richard_134419(), 17, 20));
        assert!(!v.skontroluj.iter().any(|s| s.contains("počítadlo hry")), "{:?}", v.skontroluj);
    }

    /// Early-return vetvy nesú skóre celého plátna — appka ho ukazuje aj
    /// vtedy, keď nie je čo radiť. Fixture 13:44: 22 bunky, na ktoré sa dá
    /// maľovať, a jeden položený paint (agito Lv3).
    ///
    /// TLI 73: do opravy klasifikácie buniek tu stálo 26 dostupných a 5
    /// obsadených — štyri z tých „paintov" sú modré ĽADOVÉ KOCKY, ktoré
    /// vision hlásil ako `g:ice_block1`. Maľovať sa na ne nedá a do skóre
    /// nepatria (`canvas.yml` → `stavy_buniek`).
    #[test]
    fn early_return_vetvy_nesu_skore_pred() {
        let ocakavane = Skore { na_max: 0, sucet: 3, obsadenych: 1, dostupnych: 22, max_level: 7 };

        let mut bez_ciela = board_richard_134419();
        bez_ciela.ciel = None;
        let mut splneny = board_richard_134419();
        splneny.ciel.as_mut().unwrap().pocet_aktualny = 3;
        let mut plne = board_richard_134419();
        for (r, c) in ORAMOVANE_134419 {
            plne.bunky.iter_mut().find(|b| b.riadok == r && b.stlpec == c).unwrap().paint =
                Some(Paint::monster("gang", 7));
        }
        let plne_skore = Skore { na_max: 3, sucet: 24, obsadenych: 4, ..ocakavane };

        for (stav, skore) in [(bez_ciela, ocakavane), (splneny, ocakavane), (plne, plne_skore)] {
            let v = navrhni_kroky(&stav);
            assert!(v.kroky.is_empty(), "{v:?}");
            assert_eq!(v.skore_pred, skore);
            assert_eq!(v.prinos_na_max, 0);
        }
        // a plná vetva ho nesie tiež — skóre je PRED sekvenciou
        assert_eq!(navrhni_kroky(&board_richard_134419()).skore_pred, ocakavane);
    }

    // ── TLI 73: biela bunka so štetcom je VOĽNÁ ─────────────────────────────
    //
    // Zrkadlí `test_38`–`test_44` v `testy-canvas-solver.py` (repo `ops`).
    // Fixture je REÁLNA Richardova snímka z 20.10.2026 16:39 (analýza
    // `2026-10-20-163919-46410386` na CT 114) PRESNE TAK, ako ju vtedy
    // prečítal vision model — vrátane chyby: každá voľná bunka má nálepku
    // `g:paint_brush1` a každá ľadová `g:blue_crystal1`. Testuje sa tým
    // poistka `canvas::stav_z_paintu`, nie opravený prompt.

    const NEDOSTUPNE_163919: [(i64, i64); 13] = [
        (1, 5), (1, 6), (2, 1), (3, 1), (3, 2), (4, 5), (4, 6),
        (5, 4), (5, 5), (5, 6), (6, 4), (6, 5), (6, 6),
    ];
    const LAD_163919: [(i64, i64); 4] = [(1, 1), (4, 1), (5, 1), (5, 3)];
    const ZDROJ_163919: (i64, i64) = (2, 5);
    const ORAMOVANE_163919: [(i64, i64); 3] = [(2, 4), (2, 6), (3, 5)];

    fn board_richard_163919() -> CanvasStav {
        let mut bunky = vec![];
        for r in 1..=6 {
            for c in 1..=6 {
                let dostupna = !NEDOSTUPNE_163919.contains(&(r, c));
                let paint = if (r, c) == ZDROJ_163919 {
                    Some(Paint::monster("ratlin", 3))
                } else if LAD_163919.contains(&(r, c)) {
                    Some(Paint {
                        kategoria: "gameplay".to_string(),
                        typ: "blue_crystal".to_string(),
                        level: 1,
                    })
                } else if dostupna {
                    // chybná nálepka vtedajšieho vision modelu — v skutočnosti
                    // je to prázdne plátno (biely štetec)
                    Some(Paint {
                        kategoria: "gameplay".to_string(),
                        typ: "paint_brush".to_string(),
                        level: 1,
                    })
                } else {
                    None
                };
                bunky.push(Bunka {
                    riadok: r,
                    stlpec: c,
                    dostupna,
                    paint,
                    oznacena_vykricnikom: (r, c) == ZDROJ_163919,
                    oramovana: ORAMOVANE_163919.contains(&(r, c)),
                    zamrznuta: false,
                });
            }
        }
        // sloty 0–4 sú Lv2, zvyšok Lv1 — presne ako na snímke
        let ruka_def: [(&str, i64); 15] = [
            ("ratlin", 2), ("ratlin", 2), ("agito", 2), ("agito", 2), ("dragonbone", 2),
            ("dwarf", 1), ("dwarf", 1), ("pest", 1), ("aember", 1), ("test_subject", 1),
            ("gang", 1), ("ichi", 1), ("sand_monster", 1), ("eumbra", 1), ("ghost", 1),
        ];
        let ruka = ruka_def
            .iter()
            .enumerate()
            .map(|(i, (typ, lvl))| SlotRuky {
                index: i as i64,
                paint: Some(Paint::monster(typ, *lvl)),
            })
            .collect();
        CanvasStav {
            riadky: 6,
            stlpce: 6,
            max_level: 7,
            bunky,
            ruka,
            ciel: Some(CielMiesania {
                smer: "adjacent".to_string(),
                popis: "Fill a Adjacent Cell with a Monster Paint".to_string(),
                efekt_popis: "Level up the Adjacent Monster Paint by 1 level(s)".to_string(),
                pocet_pozadovany: 3,
                pocet_aktualny: 0,
                kluc_riadok: Some(ZDROJ_163919.0),
                kluc_stlpec: Some(ZDROJ_163919.1),
            }),
            polozene: None,
            celkom: None,
        }
    }

    /// test_38 + test_39: štetec je voľná bunka, ľadová kocka je zamrznutá.
    #[test]
    fn stetec_je_volna_bunka_a_lad_je_zamrznuty() {
        let stav = board_richard_163919();
        for (r, c) in ORAMOVANE_163919 {
            let b = stav.bunka(r, c).unwrap();
            assert!(b.volna(), "r{}c{} nie je voľná", r, c);
            assert!(b.paint_na_bunke().is_none(), "r{}c{}", r, c);
        }
        for (r, c) in LAD_163919 {
            let b = stav.bunka(r, c).unwrap();
            assert!(b.je_zamrznuta(), "r{}c{}", r, c);
            assert!(!b.volna(), "r{}c{}", r, c);
            assert!(b.paint_na_bunke().is_none(), "r{}c{}", r, c);
        }
    }

    /// test_40 + test_41: rada položí tri najvyššie Lv2 painty zo slotov 0–2.
    #[test]
    fn rada_poklada_najvyssie_lv2_painty() {
        let stav = board_richard_163919();
        let (volne, zdroj) = stav.cielove_bunky(stav.ciel.as_ref().unwrap());
        assert_eq!(zdroj, ZdrojDosahu::Ram);
        let mut suradnice: Vec<(i64, i64)> = volne.iter().map(|b| (b.riadok, b.stlpec)).collect();
        suradnice.sort();
        let mut ocakavane = ORAMOVANE_163919.to_vec();
        ocakavane.sort();
        assert_eq!(suradnice, ocakavane);

        let v = navrhni_kroky(&stav);
        assert_eq!(v.kroky.iter().map(|k| k.zdroj_slot).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert!(v.skontroluj.is_empty(), "{:?}", v.skontroluj);
    }

    /// test_43 + test_44: hláška o plnom plátne povie DÔVOD.
    #[test]
    fn hlaska_o_plnom_platne_povie_preco() {
        let mut stav = board_richard_163919();
        for (r, c) in ORAMOVANE_163919 {
            stav.bunky.iter_mut().find(|b| b.riadok == r && b.stlpec == c).unwrap().paint =
                Some(Paint::monster("ratlin", 1));
        }
        let v = navrhni_kroky(&stav);
        assert!(v.kroky.is_empty());
        // Text musí sedieť ZNAK PO ZNAKU so serverom (`canvas_solver.py`).
        assert_eq!(
            v.skontroluj,
            vec!["hra orámovala 3 bunky, ale ani jedna z nich nie je voľná (obsadených: 3) — \
                  na plátne nie je kam položiť ďalší paint"
                .to_string()]
        );

        let mut stav = board_richard_163919();
        for (r, c) in ORAMOVANE_163919 {
            let b = stav.bunky.iter_mut().find(|b| b.riadok == r && b.stlpec == c).unwrap();
            b.paint = None;
            b.zamrznuta = true;
        }
        let v = navrhni_kroky(&stav);
        assert!(v.kroky.is_empty());
        assert_eq!(
            v.skontroluj,
            vec!["hra orámovala 3 bunky, ale ani jedna z nich nie je voľná (zamrznutých ľadom: \
                  3) — na plátne nie je kam položiť ďalší paint"
                .to_string()]
        );
    }

    /// test_44b: jedna bunka sa nesmie započítať do dvoch kategórií naraz.
    #[test]
    fn hlaska_nepocita_jednu_bunku_dvakrat() {
        let mut stav = board_richard_163919();
        for (r, c) in ORAMOVANE_163919 {
            let b = stav.bunky.iter_mut().find(|b| b.riadok == r && b.stlpec == c).unwrap();
            b.dostupna = false;
            b.zamrznuta = true;
        }
        let v = navrhni_kroky(&stav);
        assert_eq!(
            v.skontroluj,
            vec!["hra orámovala 3 bunky, ale ani jedna z nich nie je voľná (nedostupných \
                  (vír): 3) — na plátne nie je kam položiť ďalší paint"
                .to_string()]
        );
    }

    /// Poistka musí zabrať aj na SKUTOČNE deserializovanej odpovedi servera,
    /// nielen na bunke poskladanej v teste (nález z review diffu 20.10.2026).
    #[test]
    fn poistka_zabera_aj_na_deserializovanej_odpovedi() {
        let json = r#"{
            "riadky": 6, "stlpce": 6, "max_level": 7,
            "bunky": [
              {"riadok": 1, "stlpec": 1, "dostupna": true,
               "paint": {"kategoria": "gameplay", "typ": "paint_brush", "level": 1}},
              {"riadok": 1, "stlpec": 2, "dostupna": true,
               "paint": {"kategoria": "gameplay", "typ": "ice_block", "level": 1}},
              {"riadok": 1, "stlpec": 3, "dostupna": true, "paint": null, "zamrznuta": true},
              {"riadok": 1, "stlpec": 4, "dostupna": true,
               "paint": {"kategoria": "monster", "typ": "ratlin", "level": 3}}
            ],
            "ruka": [], "ciel": null
        }"#;
        let stav: CanvasStav = serde_json::from_str(json).unwrap();
        assert!(stav.bunka(1, 1).unwrap().volna(), "štetec má byť voľná bunka");
        assert!(stav.bunka(1, 2).unwrap().je_zamrznuta(), "ľad má byť zamrznutá bunka");
        assert!(stav.bunka(1, 3).unwrap().je_zamrznuta(), "pole zamrznuta sa má čítať");
        assert!(stav.bunka(1, 4).unwrap().paint_na_bunke().is_some());
        assert!(!stav.bunka(1, 4).unwrap().volna());
    }

    /// Pole `zamrznuta` má prednosť pred nálepkou paintu — inak by sa skóre
    /// rozišlo so serverom (nález z review diffu 20.10.2026).
    #[test]
    fn zamrznuta_prebije_aj_skutocny_paint() {
        let b = Bunka {
            riadok: 1,
            stlpec: 1,
            dostupna: true,
            paint: Some(Paint::monster("ratlin", 3)),
            oznacena_vykricnikom: false,
            oramovana: false,
            zamrznuta: true,
        };
        assert!(b.je_zamrznuta());
        assert!(!b.volna());
        assert!(b.paint_na_bunke().is_none());
    }

    /// test_42: zoznamy názvov rozlíšia štetec, ľad a skutočný paint.
    #[test]
    fn stav_z_paintu_rozlisi_stetec_lad_a_paint() {
        use crate::canvas::{stav_z_paintu, StavObsahu};
        for typ in ["brush", "paint_brush", "brushstroke", "arrow", "white_arrow", "highlight"] {
            assert_eq!(stav_z_paintu("gameplay", typ, 1), StavObsahu::Prazdna, "{typ}");
            // nad Lv1 to štetec nie je — paint so štítkom „Lv3" sa nesmie stratiť
            assert_eq!(stav_z_paintu("gameplay", typ, 3), StavObsahu::Paint, "{typ}");
        }
        for typ in ["ice", "ice_block", "ice-block", "ice_cube", "cube", "blue_cube"] {
            assert_eq!(stav_z_paintu("gameplay", typ, 1), StavObsahu::Zamrznuta, "{typ}");
            assert_eq!(stav_z_paintu("gameplay", typ, 3), StavObsahu::Zamrznuta, "{typ}");
        }
        assert_eq!(stav_z_paintu("gameplay", "blue_crystal", 1), StavObsahu::Zamrznuta);
        assert_eq!(stav_z_paintu("gameplay", "blue_crystal", 3), StavObsahu::Paint);
        for typ in ["horizontal_paint", "vertical_paint", "ice_brush", "red_crystal", "chest"] {
            assert_ne!(stav_z_paintu("gameplay", typ, 1), StavObsahu::Prazdna, "{typ}");
        }
        assert_eq!(stav_z_paintu("monster", "ratlin", 3), StavObsahu::Paint);
    }

    /// Server posiela `prinos_na_max` a `skore_pred` v JSON; stará odpoveď
    /// bez nich sa musí dať načítať ďalej.
    #[test]
    fn vysledok_serde_s_novymi_polami_aj_bez_nich() {
        let v = navrhni_kroky(&board_vertical(true, "monster"));
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(j["prinos_na_max"], 1);
        assert_eq!(j["skore_pred"]["na_max"], 0);
        assert_eq!(j["skore_pred"]["dostupnych"], 5);
        let stary: VysledokSolvera = serde_json::from_str(
            r#"{"kroky": [], "istota": 0.0, "skontroluj": [], "zdroj_dosahu": ""}"#,
        )
        .unwrap();
        assert_eq!(stary.prinos_na_max, 0);
        assert_eq!(stary.skore_pred, Skore::default());
    }
}
