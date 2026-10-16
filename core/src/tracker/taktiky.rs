//! Analýza podľa taktiky (TLI 13b).
//!
//! Taktika = podpis setupu behu: štartovací chart a kompasy, mapa a mód,
//! modifikátory mapy a build (hrdina, talenty, pact konfigurácia). Appka
//! podpisy automaticky zoskupí; hráč môže skupinu pomenovať alebo viac
//! podpisov zlúčiť do jednej (perzistencia v `db::taktiky`/`taktiky_priradenie`).
//!
//! Zámerne BEZ `rusqlite` — vstup (`VstupTaktiky`) je plochý a dá sa postaviť
//! z testovacích dát bez databázy, rovnako ako `agregacie.rs`. Volajúci
//! (`src-tauri`) spojí `db::data_pre_taktiky` + `db::modifikator_podpisy` do
//! `VstupTaktiky` cez `priprav_vstupy` a zavolá `zoskup`.
//!
//! Artistry/Snowpaper tier a sezónne mechaniky (Tartarus, Followers) zo
//! zadania TLI 13b do podpisu VEDOME nevstupujú — dátový model (`db.rs`) ich
//! zatiaľ neukladá (Tartarus parser čaká na logy po štarte sezóny, pozri
//! `BACKLOG.md` „TLI 12 bod 2"). Keď pribudnú, podpis sa rozšíri.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::db::RiadokPreTaktiku;
use super::stav::cena_percentil;

/// Pod koľkými OCENENÝMI behmi sa štatistika taktiky považuje za „málo dát"
/// (zobrazí sa to v UI namiesto tvárenia sa, že čísla sú spoľahlivé) a taktika
/// sa nenavrhne v `odporuc()`.
pub const MALO_DAT_PRAH: usize = 5;

/// Jeden beh pripravený na zoskupenie — plochý, bez závislosti na tom, odkiaľ
/// dáta prišli (DB alebo testovacie dáta).
#[derive(Debug, Clone, PartialEq)]
pub struct VstupTaktiky {
    pub beh_id: i64,
    pub podpis_vstupov: String,
    pub zona: String,
    pub level_typ: Option<i64>,
    /// Zotriedený a spojený podpis modifikátorov mapy (`db::modifikator_podpisy`), prázdny reťazec = žiadne.
    pub modifikator_podpis: String,
    /// `hero_id|pact_konfiguracia|zotriedene,genius,career,ids`.
    pub build_podpis: String,
    pub cisty_zisk_fe: Option<f64>,
    pub hodnota_vstupov_fe: Option<f64>,
    pub fe_za_hodinu: Option<f64>,
    pub trvanie_ms: Option<i64>,
}

/// Kanonický podpis buildu: hrdina, pact konfigurácia a zotriedené ID
/// talentových stromov (genius career) spojené čiarkou — poradie v logu nehrá
/// rolu, rovnaký build musí dať rovnaký reťazec vždy.
pub fn build_podpis(hero_id: Option<&str>, pact_konfiguracia: Option<&str>, genius_career_json: Option<&str>) -> String {
    let mut genius: Vec<String> = genius_career_json
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default();
    genius.sort();
    format!("{}|{}|{}", hero_id.unwrap_or(""), pact_konfiguracia.unwrap_or(""), genius.join(","))
}

/// Spojí surové riadky z DB (`db::data_pre_taktiky`) s podpismi modifikátorov
/// (`db::modifikator_podpisy`) do vstupov pre `zoskup`.
pub fn priprav_vstupy(riadky: &[RiadokPreTaktiku], modifikatory: &BTreeMap<i64, String>) -> Vec<VstupTaktiky> {
    riadky
        .iter()
        .map(|r| VstupTaktiky {
            beh_id: r.beh_id,
            podpis_vstupov: r.podpis_vstupov.clone(),
            zona: r.zona.clone(),
            level_typ: r.level_typ,
            modifikator_podpis: modifikatory.get(&r.beh_id).cloned().unwrap_or_default(),
            build_podpis: build_podpis(r.hero_id.as_deref(), r.pact_konfiguracia.as_deref(), r.genius_career_ids.as_deref()),
            cisty_zisk_fe: r.cisty_zisk_fe,
            hodnota_vstupov_fe: r.hodnota_vstupov_fe,
            fe_za_hodinu: r.fe_za_hodinu,
            trvanie_ms: r.trvanie_ms,
        })
        .collect()
}

/// Podpis taktiky = chart+kompasy @ mapa @ mód @ modifikátory @ build.
/// Zámerne jeden spojený reťazec (nie štruktúra) — je to zároveň prirodzený
/// kľúč pre `taktiky_priradenie.podpis` v DB, takže sa nemusí nič ďalej hashovať.
pub fn podpis_taktiky(v: &VstupTaktiky) -> String {
    format!(
        "{}@{}@mod{}@map[{}]@build[{}]",
        v.podpis_vstupov,
        v.zona,
        v.level_typ.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string()),
        v.modifikator_podpis,
        v.build_podpis,
    )
}

fn priemer(hodnoty: &[f64]) -> Option<f64> {
    if hodnoty.is_empty() {
        None
    } else {
        Some(hodnoty.iter().sum::<f64>() / hodnoty.len() as f64)
    }
}

fn median(hodnoty: &[f64]) -> Option<f64> {
    if hodnoty.is_empty() {
        return None;
    }
    let mut zotriedene = hodnoty.to_vec();
    zotriedene.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let n = zotriedene.len();
    if n % 2 == 1 {
        Some(zotriedene[n / 2])
    } else {
        Some((zotriedene[n / 2 - 1] + zotriedene[n / 2]) / 2.0)
    }
}

/// Výberová smerodajná odchýlka (delenie `n-1`) — pri menej než 2 hodnotách
/// nemá zmysel (vrátane prípadu presne 1 hodnoty, kde by vyšla nula a
/// predstierala istotu, ktorú jeden beh nedáva), preto `None`.
fn smerodajna_odchylka(hodnoty: &[f64]) -> Option<f64> {
    if hodnoty.len() < 2 {
        return None;
    }
    let p = priemer(hodnoty)?;
    let rozptyl = hodnoty.iter().map(|x| (x - p).powi(2)).sum::<f64>() / (hodnoty.len() - 1) as f64;
    Some(rozptyl.sqrt())
}

/// Vypočítané metriky jednej taktiky — zámerne všetko `Option`, lebo
/// importované behy bez cenníka majú `cisty_zisk_fe = NULL` (BACKLOG.md,
/// „TLI 13 — spätný import neoceňuje...") a taktika môže pozostávať len z nich.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StatistikaTaktiky {
    pub pocet_behov: u64,
    /// Koľko z `pocet_behov` má známy `cisty_zisk_fe` — zvyšok do štatistík zisku nevstupuje.
    pub pocet_ocenenych: u64,
    pub priemerny_zisk: Option<f64>,
    pub median_zisk: Option<f64>,
    pub min_zisk: Option<f64>,
    pub max_zisk: Option<f64>,
    pub p10_zisk: Option<f64>,
    pub p90_zisk: Option<f64>,
    pub smerodajna_odchylka: Option<f64>,
    pub priemerne_fe_za_hodinu: Option<f64>,
    pub priemerny_naklad: Option<f64>,
    /// `priemerny_zisk / priemerny_naklad`, len keď je náklad kladný.
    pub roi: Option<f64>,
    /// `priemerny_zisk / smerodajna_odchylka` — vyššie je stabilnejšie;
    /// podklad pre rebríček „najlepší pomer zisk/riziko".
    pub pomer_zisk_riziko: Option<f64>,
    pub priemerne_trvanie_ms: Option<i64>,
    pub malo_dat: bool,
}

pub fn vypocitaj_statistiku(vstupy: &[&VstupTaktiky]) -> StatistikaTaktiky {
    let zisky: Vec<f64> = vstupy.iter().filter_map(|v| v.cisty_zisk_fe).collect();
    let fe_za_hodinu: Vec<f64> = vstupy.iter().filter_map(|v| v.fe_za_hodinu).collect();
    let naklady: Vec<f64> = vstupy.iter().filter_map(|v| v.hodnota_vstupov_fe).collect();
    let trvania: Vec<i64> = vstupy.iter().filter_map(|v| v.trvanie_ms).collect();

    let priemerny_zisk = priemer(&zisky);
    let priemerny_naklad = priemer(&naklady);
    let smerodajna = smerodajna_odchylka(&zisky);

    let roi = match (priemerny_zisk, priemerny_naklad) {
        (Some(z), Some(n)) if n > 0.0 => Some(z / n),
        _ => None,
    };
    let pomer_zisk_riziko = match (priemerny_zisk, smerodajna) {
        (Some(z), Some(s)) if s > 0.0 => Some(z / s),
        _ => None,
    };

    StatistikaTaktiky {
        pocet_behov: vstupy.len() as u64,
        pocet_ocenenych: zisky.len() as u64,
        priemerny_zisk,
        median_zisk: median(&zisky),
        min_zisk: zisky.iter().cloned().fold(None, |acc: Option<f64>, x| Some(acc.map_or(x, |a| a.min(x)))),
        max_zisk: zisky.iter().cloned().fold(None, |acc: Option<f64>, x| Some(acc.map_or(x, |a| a.max(x)))),
        p10_zisk: cena_percentil(&zisky, 10),
        p90_zisk: cena_percentil(&zisky, 90),
        smerodajna_odchylka: smerodajna,
        priemerne_fe_za_hodinu: priemer(&fe_za_hodinu),
        priemerny_naklad,
        roi,
        pomer_zisk_riziko,
        priemerne_trvanie_ms: if trvania.is_empty() { None } else { Some(trvania.iter().sum::<i64>() / trvania.len() as i64) },
        malo_dat: zisky.len() < MALO_DAT_PRAH,
    }
}

/// Jedna zoskupená taktika pripravená na zobrazenie — pomenovaná (hráčom)
/// alebo len surovým podpisom, keď ju ešte nikto nepomenoval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaktikaSkupina {
    /// Stabilný identifikátor pre akcie v UI (premenovanie/zlúčenie): `t<id>`
    /// pre pomenovanú taktiku, inak samotný podpis.
    pub kluc: String,
    pub nazov: String,
    /// Všetky surové podpisy zahrnuté v tejto skupine (viac než 1 pri zlúčení).
    pub podpisy: Vec<String>,
    /// Chart+kompasy a mapa JEDNÉHO zástupcu skupiny — na čitateľné zobrazenie
    /// v UI, keď taktika ešte nemá vlastné pomenovanie (surový `kluc`/`nazov`
    /// je dlhý spojený reťazec, nie niečo, čo sa dá ukázať hráčovi priamo).
    /// Pri zlúčení viacerých rôznych podpisov do jednej pomenovanej taktiky to
    /// je len ukážka prvého člena, nie súhrn celej skupiny.
    pub ukazka_podpis_vstupov: String,
    pub ukazka_zona: String,
    pub statistika: StatistikaTaktiky,
}

/// Zoskupí vstupy do taktík podľa `podpis_taktiky`, s ohľadom na uložené
/// priradenia (pomenovanie/zlúčenie z `db::nacitaj_priradenia_taktik`).
/// Zoradené ZOSTUPNE podľa priemerného FE/h — prvá taktika je „najlepšia
/// taktika za hodinu" (rovnaký vzor ako `agregacie::prehlad`).
pub fn zoskup(vstupy: &[VstupTaktiky], priradenia: &BTreeMap<String, (i64, String)>) -> Vec<TaktikaSkupina> {
    let mut skupiny: BTreeMap<String, (String, Vec<&VstupTaktiky>, BTreeSet<String>)> = BTreeMap::new();

    for vstup in vstupy {
        let podpis = podpis_taktiky(vstup);
        let (kluc, nazov) = match priradenia.get(&podpis) {
            Some((id, nazov)) => (format!("t{id}"), nazov.clone()),
            None => (podpis.clone(), podpis.clone()),
        };
        let zaznam = skupiny.entry(kluc).or_insert_with(|| (nazov.clone(), Vec::new(), BTreeSet::new()));
        zaznam.0 = nazov;
        zaznam.1.push(vstup);
        zaznam.2.insert(podpis);
    }

    let mut vysledok: Vec<TaktikaSkupina> = skupiny
        .into_iter()
        .map(|(kluc, (nazov, vstupy_ref, podpisy))| {
            let zastupca = vstupy_ref.first();
            TaktikaSkupina {
                kluc,
                nazov,
                podpisy: podpisy.into_iter().collect(),
                ukazka_podpis_vstupov: zastupca.map(|v| v.podpis_vstupov.clone()).unwrap_or_default(),
                ukazka_zona: zastupca.map(|v| v.zona.clone()).unwrap_or_default(),
                statistika: vypocitaj_statistiku(&vstupy_ref),
            }
        })
        .collect();

    vysledok.sort_by(|a, b| {
        b.statistika
            .priemerne_fe_za_hodinu
            .partial_cmp(&a.statistika.priemerne_fe_za_hodinu)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.nazov.cmp(&b.nazov))
    });
    vysledok
}

/// Deterministické odporúčanie „ktorú taktiku hrať teraz" — PRVÁ taktika
/// (zoznam je už zoradený podľa FE/h) s dosť dátami na to, aby sa jej dalo
/// veriť. Žiadne AI, len štatistika nad tým, čo appka reálne nazbierala.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Odporucanie {
    pub text: String,
    pub taktika_kluc: Option<String>,
}

pub fn odporuc(skupiny: &[TaktikaSkupina]) -> Odporucanie {
    match skupiny.iter().find(|s| !s.statistika.malo_dat) {
        Some(s) => Odporucanie {
            text: format!(
                "Hraj „{}\" — priemerne {:.1} FE/h z {} ocenených behov (medián zisku {:.1} FE{}).",
                s.nazov,
                s.statistika.priemerne_fe_za_hodinu.unwrap_or(0.0),
                s.statistika.pocet_ocenenych,
                s.statistika.median_zisk.unwrap_or(0.0),
                match s.statistika.roi {
                    Some(r) => format!(", ROI {r:.2}×"),
                    None => String::new(),
                },
            ),
            taktika_kluc: Some(s.kluc.clone()),
        },
        None => Odporucanie {
            text: format!(
                "Zatiaľ málo dát na odporúčanie — žiadna taktika nemá aspoň {MALO_DAT_PRAH} ocenených behov."
            ),
            taktika_kluc: None,
        },
    }
}

/// Taktiky s pozitívnym, ale štatisticky ešte nedôveryhodným počtom behov —
/// „oplatí sa otestovať viac", odvodené výhradne z reálnych dát (appka nemá
/// katalóg všetkých chartov/kompasov/máp, takže nevie navrhnúť kombinácie,
/// ktoré hráč ešte vôbec neskúsil — pozri BACKLOG.md).
pub fn navrhy_na_otestovanie(skupiny: &[TaktikaSkupina]) -> Vec<String> {
    skupiny
        .iter()
        .filter(|s| s.statistika.malo_dat && s.statistika.pocet_ocenenych > 0)
        .take(5)
        .map(|s| {
            format!(
                "{} — zatiaľ len {} ocenený beh (z {} potrebných), oplatí sa otestovať viac",
                s.nazov, s.statistika.pocet_ocenenych, MALO_DAT_PRAH
            )
        })
        .collect()
}

/// Export zoznamu taktík do CSV — jeden riadok na taktiku.
pub fn export_csv(skupiny: &[TaktikaSkupina]) -> String {
    let mut out = String::from(
        "nazov;pocet_behov;pocet_ocenenych;priemerny_zisk;median_zisk;min_zisk;max_zisk;p10_zisk;p90_zisk;\
         smerodajna_odchylka;priemerne_fe_za_hodinu;priemerny_naklad;roi;pomer_zisk_riziko\n",
    );
    let f = |h: Option<f64>| h.map(|v| format!("{v:.2}")).unwrap_or_default();
    for s in skupiny {
        let st = &s.statistika;
        out.push_str(&format!(
            "{};{};{};{};{};{};{};{};{};{};{};{};{};{}\n",
            s.nazov.replace(';', ","),
            st.pocet_behov,
            st.pocet_ocenenych,
            f(st.priemerny_zisk),
            f(st.median_zisk),
            f(st.min_zisk),
            f(st.max_zisk),
            f(st.p10_zisk),
            f(st.p90_zisk),
            f(st.smerodajna_odchylka),
            f(st.priemerne_fe_za_hodinu),
            f(st.priemerny_naklad),
            f(st.roi),
            f(st.pomer_zisk_riziko),
        ));
    }
    out
}

/// Export zoznamu taktík do JSON (pole objektov).
pub fn export_json(skupiny: &[TaktikaSkupina]) -> String {
    serde_json::to_string_pretty(skupiny).unwrap_or_else(|_| "[]".to_string())
}

/// Celá odpoveď pre záložku Taktiky v jednom volaní — taktiky, odporúčanie
/// a návrhy na otestovanie sú odvodené z TOHO ISTÉHO zoznamu, takže si
/// nikdy nemôžu odporovať (napr. odporučiť taktiku, ktorú tabuľka nezobrazí).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AnalyzaTaktik {
    pub taktiky: Vec<TaktikaSkupina>,
    pub odporucanie: Odporucanie,
    pub navrhy_na_otestovanie: Vec<String>,
}

pub fn priprav_analyzu(skupiny: Vec<TaktikaSkupina>) -> AnalyzaTaktik {
    let odporucanie = odporuc(&skupiny);
    let navrhy_na_otestovanie = navrhy_na_otestovanie(&skupiny);
    AnalyzaTaktik { taktiky: skupiny, odporucanie, navrhy_na_otestovanie }
}

#[cfg(test)]
mod testy {
    use super::*;

    fn vstup(beh_id: i64, podpis: &str, zona: &str, zisk: Option<f64>, naklad: Option<f64>, fe_h: Option<f64>) -> VstupTaktiky {
        VstupTaktiky {
            beh_id,
            podpis_vstupov: podpis.to_string(),
            zona: zona.to_string(),
            level_typ: Some(0),
            modifikator_podpis: String::new(),
            build_podpis: "hrdina|pact|".to_string(),
            cisty_zisk_fe: zisk,
            hodnota_vstupov_fe: naklad,
            fe_za_hodinu: fe_h,
            trvanie_ms: Some(60_000),
        }
    }

    #[test]
    fn build_podpis_triedi_genius_ids_bez_ohladu_na_poradie() {
        let a = build_podpis(Some("1800"), Some("PACT_A"), Some(r#"["g2","g1"]"#));
        let b = build_podpis(Some("1800"), Some("PACT_A"), Some(r#"["g1","g2"]"#));
        assert_eq!(a, b);
        assert_eq!(a, "1800|PACT_A|g1,g2");
    }

    #[test]
    fn build_podpis_chybajuce_polia_su_prazdne_nie_none_text() {
        assert_eq!(build_podpis(None, None, None), "||");
    }

    #[test]
    fn podpis_taktiky_zahrna_vsetky_zlozky() {
        let v = vstup(1, "300x2", "MapaX", Some(10.0), None, None);
        let p = podpis_taktiky(&v);
        assert!(p.contains("300x2"));
        assert!(p.contains("MapaX"));
        assert!(p.contains("mod0"));
        assert!(p.contains("build[hrdina|pact|]"));
    }

    #[test]
    fn rovnaky_setup_da_rovnaky_podpis_aj_pri_inom_beh_id() {
        let a = podpis_taktiky(&vstup(1, "300x2", "MapaX", Some(10.0), None, None));
        let b = podpis_taktiky(&vstup(2, "300x2", "MapaX", Some(99.0), None, None));
        assert_eq!(a, b, "beh_id a zisk nesmú ovplyvniť podpis — len setup");
    }

    #[test]
    fn median_funguje_pre_parny_aj_neparny_pocet() {
        assert_eq!(median(&[1.0, 2.0, 3.0]), Some(2.0));
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn smerodajna_odchylka_pod_dvoma_hodnotami_je_none() {
        assert_eq!(smerodajna_odchylka(&[]), None);
        assert_eq!(smerodajna_odchylka(&[5.0]), None);
        // Výberová odchýlka [2,4,4,4,5,5,7,9] = 2.138...
        let v = smerodajna_odchylka(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();
        assert!((v - 2.1380899).abs() < 0.0001);
    }

    #[test]
    fn statistika_ignoruje_neocenene_behy_v_zisku_ale_pocita_ich_do_poctu() {
        let v1 = vstup(1, "x", "M", Some(10.0), Some(5.0), Some(100.0));
        let v2 = vstup(2, "x", "M", None, None, None); // importovaný beh bez ocenenia
        let st = vypocitaj_statistiku(&[&v1, &v2]);
        assert_eq!(st.pocet_behov, 2);
        assert_eq!(st.pocet_ocenenych, 1);
        assert_eq!(st.priemerny_zisk, Some(10.0));
        assert!(st.malo_dat, "1 ocenený beh je pod prahom {MALO_DAT_PRAH}");
    }

    #[test]
    fn roi_a_pomer_zisk_riziko_su_none_bez_dat() {
        let v = vstup(1, "x", "M", Some(10.0), Some(0.0), None);
        let st = vypocitaj_statistiku(&[&v]);
        assert_eq!(st.roi, None, "nulový náklad nesmie deliť nulou");
        assert_eq!(st.pomer_zisk_riziko, None, "jeden beh nemá smerodajnú odchýlku");
    }

    #[test]
    fn roi_sa_pocita_zo_priemerov() {
        let v1 = vstup(1, "x", "M", Some(20.0), Some(10.0), None);
        let v2 = vstup(2, "x", "M", Some(10.0), Some(10.0), None);
        let st = vypocitaj_statistiku(&[&v1, &v2]);
        assert_eq!(st.priemerny_zisk, Some(15.0));
        assert_eq!(st.priemerny_naklad, Some(10.0));
        assert_eq!(st.roi, Some(1.5));
    }

    #[test]
    fn zoskup_zlucuje_rovnaky_podpis_a_zoradi_podla_fe_za_hodinu() {
        let vstupy = vec![
            vstup(1, "slaba", "M", Some(5.0), None, Some(50.0)),
            vstup(2, "slaba", "M", Some(15.0), None, Some(150.0)),
            vstup(3, "dobra", "M", Some(100.0), None, Some(1000.0)),
        ];
        let skupiny = zoskup(&vstupy, &BTreeMap::new());
        assert_eq!(skupiny.len(), 2);
        assert_eq!(skupiny[0].statistika.pocet_behov, 1, "dobra");
        assert_eq!(skupiny[0].statistika.priemerne_fe_za_hodinu, Some(1000.0));
        assert_eq!(skupiny[1].statistika.pocet_behov, 2, "slaba — dva behy rovnakého podpisu sa zlúčili");
        assert_eq!(skupiny[1].statistika.priemerny_zisk, Some(10.0));
    }

    #[test]
    fn zoskup_naplni_ukazku_podpisu_a_zony_pre_nepomenovanu_taktiku() {
        let vstupy = vec![vstup(1, "400021x2", "XueYuRongLu", Some(10.0), None, None)];
        let skupiny = zoskup(&vstupy, &BTreeMap::new());
        assert_eq!(skupiny[0].ukazka_podpis_vstupov, "400021x2");
        assert_eq!(skupiny[0].ukazka_zona, "XueYuRongLu");
    }

    #[test]
    fn zoskup_pouzije_pomenovanie_a_zluci_dva_rozne_podpisy_do_jednej_taktiky() {
        let vstupy = vec![
            vstup(1, "chartA", "M", Some(10.0), None, Some(100.0)),
            vstup(2, "chartB", "M", Some(20.0), None, Some(200.0)),
        ];
        let podpis_a = podpis_taktiky(&vstupy[0]);
        let podpis_b = podpis_taktiky(&vstupy[1]);
        let mut priradenia = BTreeMap::new();
        priradenia.insert(podpis_a.clone(), (1i64, "Steel Forge farm T7".to_string()));
        priradenia.insert(podpis_b, (1i64, "Steel Forge farm T7".to_string()));

        let skupiny = zoskup(&vstupy, &priradenia);
        assert_eq!(skupiny.len(), 1);
        assert_eq!(skupiny[0].nazov, "Steel Forge farm T7");
        assert_eq!(skupiny[0].kluc, "t1");
        assert_eq!(skupiny[0].podpisy.len(), 2);
        assert_eq!(skupiny[0].statistika.pocet_behov, 2);
    }

    #[test]
    fn odporuc_vyberie_prvu_taktiku_s_dost_datami() {
        let mala_data = vstup(1, "lotéria", "M", Some(1000.0), None, Some(9999.0));
        let mut dobre_data = Vec::new();
        for i in 0..MALO_DAT_PRAH {
            dobre_data.push(vstup(i as i64 + 2, "stabilna", "M", Some(50.0), Some(10.0), Some(300.0)));
        }
        let mut vsetky = vec![mala_data];
        vsetky.extend(dobre_data);

        let skupiny = zoskup(&vsetky, &BTreeMap::new());
        // "lotéria" má vyšší FE/h a je prvá v zozname, ale má málo dát.
        assert!(skupiny[0].nazov.starts_with("lotéria"));
        assert!(skupiny[0].statistika.malo_dat);

        let o = odporuc(&skupiny);
        assert!(o.taktika_kluc.as_deref().unwrap_or("").starts_with("stabilna"));
        assert!(o.text.contains("stabilna"));
    }

    #[test]
    fn odporuc_bez_dost_dat_vrati_zrozumitelnu_spravu_a_ziadnu_taktiku() {
        let v = vstup(1, "x", "M", Some(10.0), None, Some(100.0));
        let skupiny = zoskup(&[v], &BTreeMap::new());
        let o = odporuc(&skupiny);
        assert_eq!(o.taktika_kluc, None);
        assert!(o.text.contains("Zatiaľ málo dát"));
    }

    #[test]
    fn export_csv_a_json_maju_hlavicku_a_su_parsovatelne() {
        let v = vstup(1, "x", "M", Some(10.0), Some(5.0), Some(100.0));
        let skupiny = zoskup(&[v], &BTreeMap::new());

        let csv = export_csv(&skupiny);
        assert!(csv.starts_with("nazov;pocet_behov;"));
        assert!(csv.contains("10.00"));

        let json = export_json(&skupiny);
        let spat: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(spat.len(), 1);
        assert_eq!(spat[0]["statistika"]["priemerny_zisk"], 10.0);
    }

    #[test]
    fn priprav_analyzu_spaja_taktiky_odporucanie_a_navrhy_z_toho_isteho_zoznamu() {
        let s_malo = vstup(1, "malo", "M", Some(10.0), None, Some(50.0));
        let mut dost = Vec::new();
        for i in 0..MALO_DAT_PRAH {
            dost.push(vstup(i as i64 + 2, "dost", "M", Some(20.0), Some(10.0), Some(300.0)));
        }
        let mut vsetky = vec![s_malo];
        vsetky.extend(dost);

        let skupiny = zoskup(&vsetky, &BTreeMap::new());
        let analyza = priprav_analyzu(skupiny.clone());
        assert_eq!(analyza.taktiky, skupiny);
        assert_eq!(analyza.odporucanie, odporuc(&skupiny));
        assert_eq!(analyza.navrhy_na_otestovanie, navrhy_na_otestovanie(&skupiny));
    }

    #[test]
    fn navrhy_na_otestovanie_vyberie_len_taktiky_s_malo_datami_a_aspon_jednym_ocenenym_behom() {
        let bez_ocenenia = vstup(1, "bezceny", "M", None, None, None);
        let s_malo = vstup(2, "malo", "M", Some(10.0), None, Some(100.0));
        let mut dost = Vec::new();
        for i in 0..MALO_DAT_PRAH {
            dost.push(vstup(i as i64 + 3, "dost", "M", Some(10.0), None, Some(100.0)));
        }
        let mut vsetky = vec![bez_ocenenia, s_malo];
        vsetky.extend(dost);

        let skupiny = zoskup(&vsetky, &BTreeMap::new());
        let navrhy = navrhy_na_otestovanie(&skupiny);
        assert_eq!(navrhy.len(), 1);
        assert!(navrhy[0].contains("malo"));
    }
}
