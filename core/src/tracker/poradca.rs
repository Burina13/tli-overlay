//! Samoučiaci „poradca farmenia“ (TLI 13c, časť 1/N) — bayesovský odhad hodnoty
//! taktiky s rozpadom váhy starších behov, odhad príspevku jednotlivých
//! prvkov (hrebeňová regresia) a Thompson sampling bandit na odporúčanie
//! medzi „hraj overené“ a „vyskúšaj menej isté“.
//!
//! Stavia na `tracker::taktiky` (zoskupenie behov podľa podpisu CELÉHO
//! setupu) — berie TIE ISTÉ vstupy (`VstupTaktiky`), ale namiesto klasických
//! priemerov/mediánov počíta bayesovský posterior, ktorý starším behom dáva
//! menšiu váhu (patch alebo zmena cien model časom prebijú, nie pokazia
//! navždy) a šíre intervaly tam, kde appka má málo dát.
//!
//! **Čo v tejto časti ZÁMERNE chýba** (pozri `BACKLOG.md` „TLI 13c —“):
//! skutočné príznaky kompasov/modifikátorov/mechaník z `podpis_vstupov`
//! (`prispevky()` nižšie je hotový regresný stroj, len zatiaľ bez napojenia
//! na reálne stĺpce — tie si appka ešte nevie vybrať, viď TLI 13b poznámka
//! o Artistry/Snowpaper tier a sezónnych mechanikách v podpise), trend cien
//! z `ceny.rs`, panel „Poradca“ v UI a denný súhrn na Discord — na týchto
//! základoch stavajú TLI 13c 2/N a TLI 14.
//!
//! Zámerne bez `chrono`: `zaciatok_cas` je uložený ako `RRRR-MM-DD HH:MM:SS`
//! (`log::Cas::text()`), čo stačí na civilný prevod na dni od epochy —
//! rovnaký duch ako `log::Cas::ms_od_zaciatku_roka()` o kúsok vyššie v strome,
//! a rovnaký dôvod: jedna vec, kvôli ktorej by appka ťahala celú kalendárovú
//! knižnicu.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::taktiky::{podpis_taktiky, VstupTaktiky};

// ─────────────────────────────────────────────────────────────────────────
// Čas bez chrono — civilný prevod dátumu na dni od epochy (Howard Hinnant,
// `days_from_civil`), funguje pre gregoriánsky kalendár vrátane priestupných
// rokov a nezávisí od časovej zóny (appka aj appka aj DB čas ukladajú ako
// "naivný" lokálny čas, takže ani prevod naň siahať nemá).
// ─────────────────────────────────────────────────────────────────────────

fn dni_od_epochy(rok: i64, mesiac: i64, den: i64) -> i64 {
    let y = if mesiac <= 2 { rok - 1 } else { rok };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (mesiac + 9) % 12;
    let doy = (153 * mp + 2) / 5 + den - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Rozparsuje `RRRR-MM-DD HH:MM:SS` na (zlomkové) dni od 1970-01-01. `None`
/// pri čomkoľvek inom tvare — chýbajúci čas je lepší než vymyslený, volajúci
/// (`odhadni_taktiky`) taký beh z odhadu jednoducho vynechá.
pub fn parsuj_cas_na_dni(text: &str) -> Option<f64> {
    let (datum, cas) = text.split_once(' ')?;
    let mut d = datum.split('-');
    let rok: i64 = d.next()?.parse().ok()?;
    let mesiac: i64 = d.next()?.parse().ok()?;
    let den: i64 = d.next()?.parse().ok()?;
    if d.next().is_some() {
        return None;
    }
    let mut c = cas.split(':');
    let hodina: i64 = c.next()?.parse().ok()?;
    let minuta: i64 = c.next()?.parse().ok()?;
    let sekunda: i64 = c.next()?.parse().ok()?;
    if c.next().is_some() {
        return None;
    }
    let dni = dni_od_epochy(rok, mesiac, den);
    let zlomok = (hodina * 3600 + minuta * 60 + sekunda) as f64 / 86_400.0;
    Some(dni as f64 + zlomok)
}

// ─────────────────────────────────────────────────────────────────────────
// 1) Bayesovský odhad s rozpadom váhy (zadanie TLI 13c, bod 1)
// ─────────────────────────────────────────────────────────────────────────

/// Jedno pozorovanie FE/h s vekom v dňoch voči „teraz“ — plochý vstup,
/// nezávislý od DB aj od appky, aby sa dal postaviť zo simulovaných dát.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pozorovanie {
    pub fe_za_hodinu: f64,
    pub vek_dni: f64,
}

/// Nastavenie bayesovského odhadu. Prior je zámerne veľmi slabý (appka si
/// netrúfa predpokladať, čo je „dobré“ FE/h skôr, než niečo zmeria) —
/// ovplyvní odhad len pri málo dátach, pri desiatkach behov ho už prehlušia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NastaveniePriora {
    pub prior_priemer: f64,
    pub prior_smerodajna: f64,
    /// Typický rozptyl JEDNÉHO behu tej istej taktiky (šum medzi behmi) —
    /// nie neistota priemeru, tú appka počíta sama z počtu a váh pozorovaní.
    pub pozorovacia_smerodajna: f64,
    /// Za koľko dní klesne váha behu na polovicu. Nižšie číslo = appka sa
    /// rýchlejšie „zabudne“ na staré patche a ceny.
    pub polcas_rozpadu_dni: f64,
}

impl Default for NastaveniePriora {
    fn default() -> Self {
        Self {
            prior_priemer: 0.0,
            prior_smerodajna: 500.0,
            pozorovacia_smerodajna: 150.0,
            polcas_rozpadu_dni: 30.0,
        }
    }
}

/// Pod akým EFEKTÍVNYM (váženým) počtom behov sa odhad považuje za „málo
/// dát“ — rovnaký duch ako `taktiky::MALO_DAT_PRAH`, len v spojitej miere
/// (rozpad váhy spraví z 5 starých behov menej než 5 „efektívnych“).
pub const MALO_DAT_EFEKTIVNY_PRAH: f64 = 3.0;

/// Bayesovský posterior FE/h jednej taktiky/prvku — stred, neistota a 80%
/// vierohodný interval (±1,2816 σ pri normálnom rozdelení).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BayesOdhad {
    pub efektivny_pocet_behov: f64,
    pub posterior_priemer: f64,
    pub posterior_smerodajna: f64,
    pub interval_80_nizsie: f64,
    pub interval_80_vyssie: f64,
    pub malo_dat: bool,
}

const Z_80: f64 = 1.2816;

/// Jadro TLI 13c bodu 1: bayesovská kombinácia priora s pozorovaniami,
/// vážená rozpadom podľa veku. Konjugovaný normálny model — presnosti
/// (1/rozptyl) sa jednoducho sčítavajú, preto je to vysvetliteľné (nie čierna
/// skrinka): viac/novších dát = vyššia presnosť = odhad bližšie k nameranému
/// priemeru a užší interval; málo/staré dáta = odhad ostáva blízko priora.
pub fn odhadni(pozorovania: &[Pozorovanie], prior: &NastaveniePriora) -> BayesOdhad {
    let vahy: Vec<f64> = pozorovania
        .iter()
        .map(|p| 0.5_f64.powf(p.vek_dni.max(0.0) / prior.polcas_rozpadu_dni))
        .collect();
    let efektivny_pocet: f64 = vahy.iter().sum();

    let prior_presnost = 1.0 / prior.prior_smerodajna.powi(2);
    let pozorovacia_presnost = if efektivny_pocet > 0.0 {
        efektivny_pocet / prior.pozorovacia_smerodajna.powi(2)
    } else {
        0.0
    };
    let posterior_presnost = prior_presnost + pozorovacia_presnost;

    let posterior_priemer = if efektivny_pocet > 0.0 {
        let vazeny_sucet: f64 = pozorovania.iter().zip(&vahy).map(|(p, w)| p.fe_za_hodinu * w).sum();
        let priemer_pozorovani = vazeny_sucet / efektivny_pocet;
        (prior_presnost * prior.prior_priemer + pozorovacia_presnost * priemer_pozorovani) / posterior_presnost
    } else {
        prior.prior_priemer
    };

    let posterior_smerodajna = (1.0 / posterior_presnost).sqrt();

    BayesOdhad {
        efektivny_pocet_behov: efektivny_pocet,
        posterior_priemer,
        posterior_smerodajna,
        interval_80_nizsie: posterior_priemer - Z_80 * posterior_smerodajna,
        interval_80_vyssie: posterior_priemer + Z_80 * posterior_smerodajna,
        malo_dat: efektivny_pocet < MALO_DAT_EFEKTIVNY_PRAH,
    }
}

/// Jedna taktika s bayesovským odhadom — náprotivok `taktiky::TaktikaSkupina`,
/// len namiesto klasickej štatistiky nesie posterior s neistotou.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaktikaOdhad {
    pub kluc: String,
    pub nazov: String,
    pub odhad: BayesOdhad,
}

/// Zoskupí vstupy ROVNAKO ako `taktiky::zoskup` (ten istý podpis aj
/// priradenia pomenovania/zlúčenia z DB), ale namiesto klasickej štatistiky
/// počíta bayesovský posterior z pozorovaní odvodených z `zaciatok_cas`
/// a `fe_za_hodinu`. Behy bez platného času alebo bez ocenenia
/// (`fe_za_hodinu: None`) do odhadu nevstupujú.
pub fn odhadni_taktiky(
    vstupy: &[VstupTaktiky],
    priradenia: &BTreeMap<String, (i64, String)>,
    teraz_dni: f64,
    prior: &NastaveniePriora,
) -> Vec<TaktikaOdhad> {
    let mut skupiny: BTreeMap<String, (String, Vec<Pozorovanie>)> = BTreeMap::new();

    for vstup in vstupy {
        let Some(fe_h) = vstup.fe_za_hodinu else { continue };
        let Some(cas_text) = &vstup.zaciatok_cas else { continue };
        let Some(cas_dni) = parsuj_cas_na_dni(cas_text) else { continue };

        let podpis = podpis_taktiky(vstup);
        let (kluc, nazov) = match priradenia.get(&podpis) {
            Some((id, nazov)) => (format!("t{id}"), nazov.clone()),
            None => (podpis.clone(), podpis.clone()),
        };
        let zaznam = skupiny.entry(kluc).or_insert_with(|| (nazov.clone(), Vec::new()));
        zaznam.0 = nazov;
        zaznam.1.push(Pozorovanie { fe_za_hodinu: fe_h, vek_dni: (teraz_dni - cas_dni).max(0.0) });
    }

    let mut vysledok: Vec<TaktikaOdhad> = skupiny
        .into_iter()
        .map(|(kluc, (nazov, pozorovania))| TaktikaOdhad { kluc, nazov, odhad: odhadni(&pozorovania, prior) })
        .collect();

    vysledok.sort_by(|a, b| {
        b.odhad
            .posterior_priemer
            .partial_cmp(&a.odhad.posterior_priemer)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.nazov.cmp(&b.nazov))
    });
    vysledok
}

// ─────────────────────────────────────────────────────────────────────────
// 2) Príspevok jednotlivých prvkov — hrebeňová (L2) lineárna regresia
//    (zadanie TLI 13c, bod 2)
// ─────────────────────────────────────────────────────────────────────────

/// Jeden riadok vstupu pre regresiu príspevkov — hodnoty príznakov
/// (zvyčajne 0/1 = kompas/modifikátor/mechanika prítomná, ale pokojne aj
/// škála, napr. počet rares) a výsledné FE/h toho behu. Mená príznakov sú
/// spoločné pre všetky riadky (`prispevky(..., nazvy_priznakov, ...)`), aby
/// výstup vedel povedať „kompas X: +12 FE/h“, nie len číslo stĺpca.
#[derive(Debug, Clone, PartialEq)]
pub struct RiadokRegresie {
    pub priznaky: Vec<f64>,
    pub fe_za_hodinu: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrispevokPrvku {
    pub nazov: String,
    pub prispevok_fe_za_hodinu: f64,
}

/// Hrebeňová regresia `(XᵀX + λI)⁻¹ Xᵀy` — rieši sa Gaussovou elimináciou
/// s čiastočným pivotovaním. Pri desiatkach príznakov (kompasy, modifikátory,
/// mechaniky) je vlastná implementácia jednoduchšia a bez novej závislosti
/// (`nalgebra`/`ndarray` by pre jednu maticovú inverziu priniesli celý strom
/// balíkov) — λ > 0 navyše drží sústavu riešiteľnú aj pri korelujúcich
/// príznakoch (napr. dva kompasy, ktoré sa takmer vždy berú spolu).
pub fn prispevky(riadky: &[RiadokRegresie], nazvy_priznakov: &[String], lambda: f64) -> Vec<PrispevokPrvku> {
    let p = nazvy_priznakov.len();
    if riadky.is_empty() || p == 0 {
        return nazvy_priznakov
            .iter()
            .map(|n| PrispevokPrvku { nazov: n.clone(), prispevok_fe_za_hodinu: 0.0 })
            .collect();
    }

    let mut xtx = vec![vec![0.0_f64; p]; p];
    let mut xty = vec![0.0_f64; p];
    for r in riadky {
        for i in 0..p {
            xty[i] += r.priznaky[i] * r.fe_za_hodinu;
            for j in 0..p {
                xtx[i][j] += r.priznaky[i] * r.priznaky[j];
            }
        }
    }
    for (i, row) in xtx.iter_mut().enumerate() {
        row[i] += lambda;
    }

    let riesenie = over_sustavu(xtx, xty);

    nazvy_priznakov
        .iter()
        .zip(riesenie)
        .map(|(n, v)| PrispevokPrvku { nazov: n.clone(), prispevok_fe_za_hodinu: v })
        .collect()
}

/// Gaussova eliminácia s čiastočným pivotovaním pre `A x = b`. Pri
/// singulárnej sústave (λ = 0 a lineárne závislé príznaky) vráti nuly namiesto
/// panického delenia nulou — poistka, bežne sem λ > 0 nepustí.
fn over_sustavu(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| a[i][col].abs().partial_cmp(&a[j][col].abs()).unwrap_or(Ordering::Equal))
            .unwrap();
        if a[pivot][col].abs() < 1e-12 {
            return vec![0.0; n];
        }
        a.swap(col, pivot);
        b.swap(col, pivot);

        let diag = a[col][col];
        for j in col..n {
            a[col][j] /= diag;
        }
        b[col] /= diag;

        for row in 0..n {
            if row == col {
                continue;
            }
            let faktor = a[row][col];
            if faktor != 0.0 {
                for j in col..n {
                    a[row][j] -= faktor * a[col][j];
                }
                b[row] -= faktor * b[col];
            }
        }
    }
    b
}

// ─────────────────────────────────────────────────────────────────────────
// 3) Prieskum vs. využitie — Thompson sampling bandit (zadanie TLI 13c, bod 3)
// ─────────────────────────────────────────────────────────────────────────

/// Minimálny PRNG (SplitMix64 + Box-Muller) — appka nepotrebuje kryptografickú
/// kvalitu, len reprodukovateľné vzorky z normálneho rozdelenia pre Thompson
/// sampling. Žiadna nová závislosť (`rand`/`rand_distr`) pre jednu vec, ktorú
/// appka potrebuje.
pub struct Prng(u64);

impl Prng {
    pub fn nova(seed: u64) -> Self {
        Self(seed)
    }

    fn dalsie_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Rovnomerné na (0,1) — vylučuje presnú 0, aby `ln()` nižšie nedal `-inf`.
    fn dalsie_f64(&mut self) -> f64 {
        ((self.dalsie_u64() >> 11) as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0)
    }

    /// Vzorka zo štandardného normálneho rozdelenia (Box-Muller).
    pub fn normalna(&mut self) -> f64 {
        let u1 = self.dalsie_f64();
        let u2 = self.dalsie_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolozkaOdporucania {
    pub kluc: String,
    pub nazov: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OdporucaniePoradcu {
    pub teraz_hraj: Option<PolozkaOdporucania>,
    pub vyskusaj: Vec<PolozkaOdporucania>,
    pub prestan: Vec<PolozkaOdporucania>,
}

fn popis_odhadu(o: &BayesOdhad) -> String {
    format!(
        "{:.0} efekt. behov, odhad {:.0}–{:.0} FE/h",
        o.efektivny_pocet_behov,
        o.interval_80_nizsie.max(0.0),
        o.interval_80_vyssie
    )
}

/// Thompson sampling: z posterioru KAŽDEJ taktiky vytiahne jednu náhodnú
/// vzorku, víťaz žrebu je dnešné „Teraz hraj“. Väčšinu dní vyhrá overená
/// taktika s úzkym intervalom okolo vysokej hodnoty (jej vzorky sú stabilne
/// vysoké); občas vyhrá menej otestovaná taktika so širokým intervalom,
/// ktorej vzorka náhodou vyjde vysoko — to JE vstavaný prieskum, nie chyba,
/// a appka to v texte odporúčania aj povie.
pub fn odporuc(odhady: &[TaktikaOdhad], prng: &mut Prng) -> OdporucaniePoradcu {
    if odhady.is_empty() {
        return OdporucaniePoradcu::default();
    }

    let vzorky: Vec<f64> = odhady
        .iter()
        .map(|o| o.odhad.posterior_priemer + prng.normalna() * o.odhad.posterior_smerodajna)
        .collect();
    let vitaz_i = vzorky
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap();

    let najlepsi_priemer_i = odhady
        .iter()
        .enumerate()
        .max_by(|a, b| {
            a.1.odhad
                .posterior_priemer
                .partial_cmp(&b.1.odhad.posterior_priemer)
                .unwrap_or(Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap();

    let je_prieskum = vitaz_i != najlepsi_priemer_i;
    let vitaz = &odhady[vitaz_i];
    let teraz_hraj = Some(PolozkaOdporucania {
        kluc: vitaz.kluc.clone(),
        nazov: vitaz.nazov.clone(),
        text: if je_prieskum {
            format!(
                "Vyskúšaj „{}“ namiesto zabehnutého — zatiaľ {}, oplatí sa overiť.",
                vitaz.nazov,
                popis_odhadu(&vitaz.odhad)
            )
        } else {
            format!("Hraj „{}“ — {}.", vitaz.nazov, popis_odhadu(&vitaz.odhad))
        },
    });

    // Vyskúšaj: ďalšie taktiky s málo dátami, ktoré dnešný žreb nevyhrali,
    // ale majú slušný horný koniec intervalu — potenciál, nie istota.
    let vyskusaj = odhady
        .iter()
        .enumerate()
        .filter(|(i, o)| *i != vitaz_i && o.odhad.malo_dat && o.odhad.interval_80_vyssie > 0.0)
        .take(3)
        .map(|(_, o)| PolozkaOdporucania {
            kluc: o.kluc.clone(),
            nazov: o.nazov.clone(),
            text: format!("Vyskúšaj „{}“ — zatiaľ {}, oplatí sa overiť.", o.nazov, popis_odhadu(&o.odhad)),
        })
        .collect();

    // Prestaň: taktiky s dosť dátami, ktorých CELÝ 80% interval leží pod
    // dolným koncom intervalu víťaza — nie „horšie dnes“, ale jasne horšie
    // aj s rezervou na neistotu oboch strán.
    let prah = vitaz.odhad.interval_80_nizsie;
    let prestan = odhady
        .iter()
        .filter(|o| o.kluc != vitaz.kluc && !o.odhad.malo_dat && o.odhad.interval_80_vyssie < prah)
        .map(|o| PolozkaOdporucania {
            kluc: o.kluc.clone(),
            nazov: o.nazov.clone(),
            text: format!(
                "Prestaň s „{}“ — odhad {:.0} FE/h, jasne pod „{}“ ({:.0} FE/h).",
                o.nazov, o.odhad.posterior_priemer, vitaz.nazov, vitaz.odhad.posterior_priemer
            ),
        })
        .collect();

    OdporucaniePoradcu { teraz_hraj, vyskusaj, prestan }
}

// ─────────────────────────────────────────────────────────────────────────
// 5) Zmena podmienok — change-point detekcia (zadanie TLI 13c, bod 5)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ZmenaPodmienok {
    pub detegovana: bool,
    pub stary_priemer: f64,
    pub novy_priemer: f64,
    pub text: String,
}

/// Welchov t-test medzi `okno_najnovsich` najnovšími behmi a zvyškom —
/// jednoduchá, vysvetliteľná detekcia zmeny výnosu (patch, pád cien, nový
/// build). `|z| > 2` zodpovedá ~95% istote pri obojstrannom teste. Vracia
/// `detegovana: false` (a prázdny text) pri príliš málo dátach na to, aby mal
/// test zmysel.
pub fn detekuj_zmenu(pozorovania: &[Pozorovanie], okno_najnovsich: usize) -> ZmenaPodmienok {
    if okno_najnovsich < 2 || pozorovania.len() < okno_najnovsich * 2 {
        return ZmenaPodmienok::default();
    }

    let mut zotriedene: Vec<&Pozorovanie> = pozorovania.iter().collect();
    zotriedene.sort_by(|a, b| a.vek_dni.partial_cmp(&b.vek_dni).unwrap_or(Ordering::Equal));

    let nove: Vec<f64> = zotriedene[..okno_najnovsich].iter().map(|p| p.fe_za_hodinu).collect();
    let stare: Vec<f64> = zotriedene[okno_najnovsich..].iter().map(|p| p.fe_za_hodinu).collect();

    let priemer = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let rozptyl = |v: &[f64], m: f64| v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len().max(2) - 1) as f64;

    let m_nove = priemer(&nove);
    let m_stare = priemer(&stare);
    let se = ((rozptyl(&nove, m_nove) / nove.len() as f64) + (rozptyl(&stare, m_stare) / stare.len() as f64)).sqrt();
    // Pri nulovom rozptyle v OBOCH oknách (v praxi nezvyčajné, ale v testoch
    // na simulovaných dátach bežné) by delenie `se` dalo NaN/0 namiesto
    // vysokej istoty — akýkoľvek nenulový rozdiel priemerov je vtedy istý.
    let z = if se > 0.0 {
        (m_nove - m_stare) / se
    } else if m_nove != m_stare {
        f64::INFINITY
    } else {
        0.0
    };
    let detegovana = z.abs() > 2.0;

    ZmenaPodmienok {
        detegovana,
        stary_priemer: m_stare,
        novy_priemer: m_nove,
        text: if !detegovana {
            String::new()
        } else if m_nove > m_stare {
            format!(
                "Zmena výnosu: posledných {okno_najnovsich} behov {m_nove:.0} FE/h vs. predošlé {m_stare:.0} FE/h — zlepšenie, over či je to patch alebo náhoda."
            )
        } else {
            format!(
                "Zmena výnosu: posledných {okno_najnovsich} behov {m_nove:.0} FE/h vs. predošlé {m_stare:.0} FE/h — pokles, over ceny vstupov/výstupov alebo patch."
            )
        },
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn parsuj_cas_na_dni_funguje_a_pocita_priestupny_rok() {
        let d1 = parsuj_cas_na_dni("2026-10-15 20:00:00").unwrap();
        let d2 = parsuj_cas_na_dni("2026-10-16 08:00:00").unwrap();
        assert!((d2 - d1 - 0.5).abs() < 1e-9, "rozdiel má byť presne pol dňa: {d2} - {d1}");

        // 2024 je priestupný rok — 1. marec 2024 je 60 dní od 1. januára 2024.
        let novy_rok = parsuj_cas_na_dni("2024-01-01 00:00:00").unwrap();
        let marec = parsuj_cas_na_dni("2024-03-01 00:00:00").unwrap();
        assert!((marec - novy_rok - 60.0).abs() < 1e-9);
    }

    #[test]
    fn parsuj_cas_na_dni_odmietne_zly_format() {
        assert_eq!(parsuj_cas_na_dni("hocico"), None);
        assert_eq!(parsuj_cas_na_dni("2026-10-15"), None);
        assert_eq!(parsuj_cas_na_dni("2026-10-15 20:00:00 UTC"), None);
    }

    #[test]
    fn odhadni_bez_dat_vrati_prior() {
        let prior = NastaveniePriora { prior_priemer: 123.0, ..Default::default() };
        let o = odhadni(&[], &prior);
        assert_eq!(o.posterior_priemer, 123.0);
        assert_eq!(o.efektivny_pocet_behov, 0.0);
        assert!(o.malo_dat);
    }

    #[test]
    fn odhadni_s_vela_cerstvymi_datami_sa_priblizi_k_nameranemu_priemeru() {
        let prior = NastaveniePriora::default();
        let pozorovania: Vec<Pozorovanie> = (0..50).map(|_| Pozorovanie { fe_za_hodinu: 300.0, vek_dni: 0.0 }).collect();
        let o = odhadni(&pozorovania, &prior);
        assert!((o.posterior_priemer - 300.0).abs() < 5.0, "50 zhodných behov má prebiť slabý prior: {:.1}", o.posterior_priemer);
        assert!(!o.malo_dat);
        assert!(o.posterior_smerodajna < prior.prior_smerodajna, "neistota sa musí zúžiť oproti prioru");
    }

    #[test]
    fn odhadni_stare_behy_vahia_menej_ako_cerstve() {
        let prior = NastaveniePriora::default();
        let cerstve: Vec<Pozorovanie> = (0..20).map(|_| Pozorovanie { fe_za_hodinu: 400.0, vek_dni: 1.0 }).collect();
        let stare: Vec<Pozorovanie> = (0..20).map(|_| Pozorovanie { fe_za_hodinu: 400.0, vek_dni: 180.0 }).collect();

        let o_cerstve = odhadni(&cerstve, &prior);
        let o_stare = odhadni(&stare, &prior);
        assert!(
            o_cerstve.efektivny_pocet_behov > o_stare.efektivny_pocet_behov,
            "20 čerstvých behov musí mať vyššiu efektívnu váhu než 20 polročných: {:.2} vs {:.2}",
            o_cerstve.efektivny_pocet_behov,
            o_stare.efektivny_pocet_behov
        );
    }

    #[test]
    fn odhadni_taktiky_najde_skutocne_lepsiu_taktiku() {
        let priradenia = BTreeMap::new();
        let prior = NastaveniePriora::default();

        let mut vstupy = Vec::new();
        for i in 0..10 {
            vstupy.push(vstup_s_casom(i, "slaba", Some(150.0), &format!("2026-09-{:02} 10:00:00", 1 + i)));
        }
        for i in 0..10 {
            vstupy.push(vstup_s_casom(100 + i, "dobra", Some(320.0), &format!("2026-09-{:02} 10:00:00", 1 + i)));
        }

        let odhady = odhadni_taktiky(&vstupy, &priradenia, parsuj_cas_na_dni("2026-09-15 10:00:00").unwrap(), &prior);
        assert_eq!(odhady.len(), 2);
        assert!(odhady[0].nazov.starts_with("dobra"), "najlepšia taktika má byť prvá: {:?}", odhady[0].nazov);
        assert!(odhady[0].odhad.posterior_priemer > odhady[1].odhad.posterior_priemer);
        assert!(!odhady[0].odhad.malo_dat);
    }

    fn vstup_s_casom(beh_id: i64, podpis: &str, fe_h: Option<f64>, cas: &str) -> VstupTaktiky {
        VstupTaktiky {
            beh_id,
            podpis_vstupov: podpis.to_string(),
            zona: "M".to_string(),
            level_typ: Some(0),
            modifikator_podpis: String::new(),
            build_podpis: "hrdina|pact|".to_string(),
            cisty_zisk_fe: None,
            hodnota_vstupov_fe: None,
            fe_za_hodinu: fe_h,
            trvanie_ms: Some(60_000),
            zaciatok_cas: Some(cas.to_string()),
        }
    }

    #[test]
    fn prispevky_najde_spravne_znamienko_a_pribliznu_velkost() {
        // Simulované dáta: FE/h = 200 + 50*(kompas_a) - 30*(kompas_b), bez šumu —
        // regresia musí vrátiť presne tie koeficienty (plus konštantu).
        let nazvy = vec!["konstanta".to_string(), "kompas_a".to_string(), "kompas_b".to_string()];
        let mut riadky = Vec::new();
        for a in [0.0, 1.0] {
            for b in [0.0, 1.0] {
                for _ in 0..5 {
                    riadky.push(RiadokRegresie {
                        priznaky: vec![1.0, a, b],
                        fe_za_hodinu: 200.0 + 50.0 * a - 30.0 * b,
                    });
                }
            }
        }

        let p = prispevky(&riadky, &nazvy, 0.001);
        let mapa: BTreeMap<_, _> = p.iter().map(|x| (x.nazov.clone(), x.prispevok_fe_za_hodinu)).collect();

        assert!((mapa["konstanta"] - 200.0).abs() < 1.0);
        assert!(mapa["kompas_a"] > 0.0, "kompas_a musí mať KLADNÝ príspevok: {}", mapa["kompas_a"]);
        assert!((mapa["kompas_a"] - 50.0).abs() < 1.0);
        assert!(mapa["kompas_b"] < 0.0, "kompas_b musí mať ZÁPORNÝ príspevok: {}", mapa["kompas_b"]);
        assert!((mapa["kompas_b"] + 30.0).abs() < 1.0);
    }

    #[test]
    fn prispevky_bez_dat_vrati_nuly_pre_vsetky_mena() {
        let nazvy = vec!["x".to_string(), "y".to_string()];
        let p = prispevky(&[], &nazvy, 1.0);
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|x| x.prispevok_fe_za_hodinu == 0.0));
    }

    #[test]
    fn odporuc_na_prazdnom_zozname_nevrati_nic() {
        let mut prng = Prng::nova(1);
        let o = odporuc(&[], &mut prng);
        assert_eq!(o, OdporucaniePoradcu::default());
    }

    #[test]
    fn odporuc_pri_jasnom_rozdiele_a_dost_datami_vyberie_lepsiu_a_oznaci_horsiu_na_prestanie() {
        let dobra = TaktikaOdhad {
            kluc: "dobra".to_string(),
            nazov: "dobra".to_string(),
            odhad: odhadni(
                &(0..20).map(|_| Pozorovanie { fe_za_hodinu: 400.0, vek_dni: 1.0 }).collect::<Vec<_>>(),
                &NastaveniePriora::default(),
            ),
        };
        let slaba = TaktikaOdhad {
            kluc: "slaba".to_string(),
            nazov: "slaba".to_string(),
            odhad: odhadni(
                &(0..20).map(|_| Pozorovanie { fe_za_hodinu: 50.0, vek_dni: 1.0 }).collect::<Vec<_>>(),
                &NastaveniePriora::default(),
            ),
        };

        let mut prng = Prng::nova(7);
        let o = odporuc(&[dobra, slaba], &mut prng);
        assert_eq!(o.teraz_hraj.as_ref().unwrap().kluc, "dobra");
        assert_eq!(o.prestan.len(), 1);
        assert_eq!(o.prestan[0].kluc, "slaba");
    }

    #[test]
    fn bandit_konverguje_k_najlepsej_ale_obcas_preskumi() {
        let prior = NastaveniePriora::default();
        let pravdive_priemery = [100.0_f64, 250.0, 150.0];
        let najlepsi_idx = 1;
        let mut prng = Prng::nova(42);

        let mut skupiny: Vec<Vec<Pozorovanie>> = pravdive_priemery
            .iter()
            .map(|&p| vec![Pozorovanie { fe_za_hodinu: p, vek_dni: 0.0 }])
            .collect();

        let mut pocty_vyhier = vec![0u32; pravdive_priemery.len()];
        const KOL: u32 = 300;

        for _ in 0..KOL {
            let odhady: Vec<TaktikaOdhad> = skupiny
                .iter()
                .enumerate()
                .map(|(i, poz)| TaktikaOdhad {
                    kluc: format!("arm{i}"),
                    nazov: format!("arm{i}"),
                    odhad: odhadni(poz, &prior),
                })
                .collect();

            let odp = odporuc(&odhady, &mut prng);
            let vitaz_kluc = odp.teraz_hraj.unwrap().kluc;
            let idx: usize = vitaz_kluc.trim_start_matches("arm").parse().unwrap();
            pocty_vyhier[idx] += 1;

            let odmena = pravdive_priemery[idx] + prng.normalna() * 20.0;
            skupiny[idx].push(Pozorovanie { fe_za_hodinu: odmena, vek_dni: 0.0 });
        }

        assert!(
            pocty_vyhier[najlepsi_idx] as f64 > KOL as f64 * 0.5,
            "najlepšia ruka musí vyhrávať väčšinu kôl: {pocty_vyhier:?}"
        );
        assert!(
            pocty_vyhier.iter().enumerate().filter(|(i, _)| *i != najlepsi_idx).any(|(_, &c)| c > 0),
            "bandit sa musí aspoň občas preskúmať, nielen využívať: {pocty_vyhier:?}"
        );
    }

    #[test]
    fn detekuj_zmenu_pri_malo_datach_mlci() {
        let poz: Vec<Pozorovanie> = (0..4).map(|i| Pozorovanie { fe_za_hodinu: 200.0, vek_dni: i as f64 }).collect();
        let z = detekuj_zmenu(&poz, 5);
        assert!(!z.detegovana);
        assert_eq!(z.text, "");
    }

    #[test]
    fn detekuj_zmenu_najde_jasny_pokles() {
        let mut poz = Vec::new();
        for i in 0..10 {
            poz.push(Pozorovanie { fe_za_hodinu: 100.0, vek_dni: i as f64 }); // najnovšie (nízky vek)
        }
        for i in 10..20 {
            poz.push(Pozorovanie { fe_za_hodinu: 400.0, vek_dni: i as f64 }); // staršie
        }

        let z = detekuj_zmenu(&poz, 10);
        assert!(z.detegovana);
        assert!(z.novy_priemer < z.stary_priemer);
        assert!(z.text.contains("pokles"));
    }

    #[test]
    fn detekuj_zmenu_bez_rozdielu_nedetekuje_nic() {
        let poz: Vec<Pozorovanie> = (0..20).map(|i| Pozorovanie { fe_za_hodinu: 300.0, vek_dni: i as f64 }).collect();
        let z = detekuj_zmenu(&poz, 10);
        assert!(!z.detegovana);
    }
}
