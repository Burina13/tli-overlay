use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::ceny::{ocen, Cennik, Nastavenie};
use super::stav::Beh;

/// Výsledok jedného behu po ocenení.
/// Oddeľuje čisté herné dáta od finančného vyjadrenia, aby sa UI a ďalšie agregácie
/// mohli spoliehať na hotové, vypočítané čísla a nemuseli riešiť cenníky.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VysledokBehu {
    pub id: u64,
    pub zona: String,
    pub podpis_vstupov: String,
    pub podpis_kombinacie: String,
    pub trvanie_ms: i64,
    /// Hodnota všetkých dropov v FE.
    pub hodnota_dropov: f64,
    /// Hodnota spotrebovaných vstupov v FE.
    pub hodnota_vstupov: f64,
    /// `hodnota_dropov - hodnota_vstupov`. Môže byť záporná — a vtedy to tak treba ukázať.
    pub cisty_zisk: f64,
    /// Položky bez známej ceny (z dropov aj zo vstupov) — v UI sa zobrazia ako „bez ceny".
    pub bez_ceny: Vec<(u32, i64)>,
    /// Najstaršia použitá cena (unixový čas) — na zobrazenie veku dát.
    pub najstarsia_cena: Option<u64>,
}

/// Ocení jeden beh. Hubové behy (`je_hub`) sa NEOCEŇUJÚ — vracia `None`,
/// pretože pobyt v hube nie je herná mapa a skresľoval by štatistiky zárobku.
pub fn ocen_beh(beh: &Beh, cennik: &Cennik, nastavenie: &Nastavenie) -> Option<VysledokBehu> {
    if beh.je_hub {
        return None;
    }

    let ocenene_dropy = ocen(&beh.dropy, cennik, nastavenie);
    let ocenene_vstupy = ocen(&beh.vstupy, cennik, nastavenie);

    // Položky bez ceny musíme zlúčiť (sčítame ich počty), aby hráč dostal
    // jeden ucelený zoznam všetkého, čomu chýba cenovka. BTreeMap zaistí determinizmus.
    let mut bez_ceny_map = BTreeMap::new();
    for (polozka, pocet) in ocenene_dropy.bez_ceny.into_iter().chain(ocenene_vstupy.bez_ceny) {
        *bez_ceny_map.entry(polozka).or_insert(0) += pocet;
    }
    let bez_ceny = bez_ceny_map.into_iter().collect();

    // Vyberáme absolútne najstarší timestamp (najnižšie číslo), aby používateľ
    // videl, kedy bola aktualizovaná najstaršia použitá položka.
    let najstarsia_cena = match (ocenene_dropy.najstarsia_cena, ocenene_vstupy.najstarsia_cena) {
        (Some(d), Some(v)) => Some(d.min(v)),
        (Some(d), None) => Some(d),
        (None, Some(v)) => Some(v),
        (None, None) => None,
    };

    Some(VysledokBehu {
        id: beh.id,
        zona: beh.zona.clone(),
        podpis_vstupov: beh.podpis_vstupov(),
        podpis_kombinacie: beh.podpis_kombinacie(),
        trvanie_ms: beh.trvanie_ms().unwrap_or(0),
        hodnota_dropov: ocenene_dropy.fe,
        hodnota_vstupov: ocenene_vstupy.fe,
        cisty_zisk: ocenene_dropy.fe - ocenene_vstupy.fe,
        bez_ceny,
        najstarsia_cena,
    })
}

/// Súhrn skupiny behov.
/// Obsahuje celkové trvanie, sumárne hodnoty a poskytuje metódy pre prepočet
/// na hodinu/mapu, čo je základ pre zobrazenie v UI.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Suhrn {
    pub pocet_map: u64,
    pub cas_ms: i64,
    pub hodnota_dropov: f64,
    pub hodnota_vstupov: f64,
    pub cisty_zisk: f64,
}

impl Suhrn {
    /// Čistý zisk za hodinu. Pri nulovom čase vráti 0.0, aby sme nepadli na delení nulou
    /// a nedostali nečitateľné NaN alebo Infinity v používateľskom rozhraní.
    pub fn zisk_za_hodinu(&self) -> f64 {
        if self.cas_ms == 0 {
            0.0
        } else {
            self.cisty_zisk / (self.cas_ms as f64 / 3_600_000.0)
        }
    }

    /// Hrubá hodnota dropov za hodinu (bez odpočtu vstupov).
    pub fn dropy_za_hodinu(&self) -> f64 {
        if self.cas_ms == 0 {
            0.0
        } else {
            self.hodnota_dropov / (self.cas_ms as f64 / 3_600_000.0)
        }
    }

    /// Koľko máp stíha hráč ubehnúť za hodinu pri aktuálnom tempe.
    pub fn map_za_hodinu(&self) -> f64 {
        if self.cas_ms == 0 {
            0.0
        } else {
            self.pocet_map as f64 / (self.cas_ms as f64 / 3_600_000.0)
        }
    }

    /// Priemerný čistý zisk z jednej mapy.
    pub fn priemer_na_mapu(&self) -> f64 {
        if self.pocet_map == 0 {
            0.0
        } else {
            self.cisty_zisk / self.pocet_map as f64
        }
    }

    /// Priemerné trvanie jednej mapy v milisekundách.
    pub fn priemerne_trvanie_ms(&self) -> i64 {
        if self.pocet_map == 0 {
            0
        } else {
            self.cas_ms / self.pocet_map as i64
        }
    }
}

/// Jedna kategória (mapa, vstupy alebo ich kombinácia) s vypočítaným súhrnom,
/// pripravená na zobrazenie v tabuľke.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skupina {
    pub kluc: String,
    pub suhrn: Suhrn,
}

/// Celkový pohľad, ktorý spája globálne štatistiky a rozdelenia (group-by) podľa
/// rôznych kľúčov. Všetko je predpočítané kvôli priamemu použitiu v UI.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Prehlad {
    pub celkom: Suhrn,
    /// Rozdelenie podľa `podpis_kombinacie` (chart + kompasy + mapa), zoradené
    /// podľa `zisk_za_hodinu` ZOSTUPNE — prvá skupina je tá, ktorá sa najviac oplatí.
    pub podla_kombinacie: Vec<Skupina>,
    /// Rozdelenie len podľa `podpis_vstupov` (chart + kompasy, naprieč mapami).
    pub podla_vstupov: Vec<Skupina>,
    /// Rozdelenie len podľa mapy (`zona`).
    pub podla_mapy: Vec<Skupina>,
    /// Súčet dropov cez všetky behy: položka -> počet kusov (na „najčastejšie dropy").
    pub dropy_spolu: BTreeMap<u32, i64>,
    /// Položky, ktoré sa objavili, ale cenu nemajú: položka -> počet kusov.
    pub bez_ceny_spolu: BTreeMap<u32, i64>,
}

/// Zostaví ucelený prehľad agregovaním hotových výsledkov z jednotlivých behov.
pub fn prehlad(vysledky: &[VysledokBehu], behy: &[Beh]) -> Prehlad {
    let mut celkom = Suhrn::default();
    let mut mapa_kombinacie: BTreeMap<String, Suhrn> = BTreeMap::new();
    let mut mapa_vstupov: BTreeMap<String, Suhrn> = BTreeMap::new();
    let mut mapa_mapy: BTreeMap<String, Suhrn> = BTreeMap::new();
    
    let mut bez_ceny_spolu: BTreeMap<u32, i64> = BTreeMap::new();
    let mut dropy_spolu: BTreeMap<u32, i64> = BTreeMap::new();

    // Rýchly lookup tabuľka behov podľa ich ID pre sčítavanie surových dropov.
    let behy_map: BTreeMap<u64, &Beh> = behy.iter().map(|b| (b.id, b)).collect();

    for vysledok in vysledky {
        let aktualizuj_suhrn = |suhrn: &mut Suhrn| {
            suhrn.pocet_map += 1;
            suhrn.cas_ms += vysledok.trvanie_ms;
            suhrn.hodnota_dropov += vysledok.hodnota_dropov;
            suhrn.hodnota_vstupov += vysledok.hodnota_vstupov;
            suhrn.cisty_zisk += vysledok.cisty_zisk;
        };

        aktualizuj_suhrn(&mut celkom);
        aktualizuj_suhrn(mapa_kombinacie.entry(vysledok.podpis_kombinacie.clone()).or_default());
        aktualizuj_suhrn(mapa_vstupov.entry(vysledok.podpis_vstupov.clone()).or_default());
        aktualizuj_suhrn(mapa_mapy.entry(vysledok.zona.clone()).or_default());

        for &(polozka, pocet) in &vysledok.bez_ceny {
            *bez_ceny_spolu.entry(polozka).or_insert(0) += pocet;
        }

        if let Some(beh) = behy_map.get(&vysledok.id) {
            for (&polozka, &pocet) in &beh.dropy {
                *dropy_spolu.entry(polozka).or_insert(0) += pocet;
            }
        }
    }

    // Pretvorí zoskupenú mapu na zoradený vektor kategórií podľa zisku za hodinu
    let pretvor = |mapa: BTreeMap<String, Suhrn>| -> Vec<Skupina> {
        let mut pole: Vec<Skupina> = mapa
            .into_iter()
            .map(|(kluc, suhrn)| Skupina { kluc, suhrn })
            .collect();

        // Zoradí primárne podľa zisku zostupne, sekundárne abecedne kvôli determinizmu.
        pole.sort_by(|a, b| {
            b.suhrn
                .zisk_za_hodinu()
                .partial_cmp(&a.suhrn.zisk_za_hodinu())
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.kluc.cmp(&b.kluc))
        });
        pole
    };

    Prehlad {
        celkom,
        podla_kombinacie: pretvor(mapa_kombinacie),
        podla_vstupov: pretvor(mapa_vstupov),
        podla_mapy: pretvor(mapa_mapy),
        dropy_spolu,
        bez_ceny_spolu,
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    fn dummy_beh(id: u64, je_hub: bool) -> Beh {
        Beh {
            id,
            zona: "Z".to_string(),
            je_hub,
            level_uid: None,
            level_typ: None,
            level_id: None,
            zaciatok: None,
            koniec: None,
            dropy: BTreeMap::new(),
            vstupy: BTreeMap::new(),
            vsetky_zmeny: vec![],
        }
    }

    fn dummy_vysledok(
        id: u64,
        kombinacia: &str,
        vstupy: &str,
        zisk: f64,
        cas_ms: i64,
        mapa: &str,
    ) -> VysledokBehu {
        VysledokBehu {
            id,
            zona: mapa.to_string(),
            podpis_vstupov: vstupy.to_string(),
            podpis_kombinacie: kombinacia.to_string(),
            trvanie_ms: cas_ms,
            hodnota_dropov: zisk + 10.0,
            hodnota_vstupov: 10.0,
            cisty_zisk: zisk,
            bez_ceny: vec![],
            najstarsia_cena: None,
        }
    }

    #[test]
    fn test_hubovy_beh_sa_neocenuje() {
        let beh = dummy_beh(1, true);
        let cennik = Cennik::novy();
        let nastavenie = Nastavenie { dan: false };
        assert!(ocen_beh(&beh, &cennik, &nastavenie).is_none());
    }

    #[test]
    fn test_suhrn_zisk_za_hodinu() {
        let suhrn = Suhrn {
            cas_ms: 3_600_000,
            cisty_zisk: 150.5,
            ..Default::default()
        };
        assert_eq!(suhrn.zisk_za_hodinu(), 150.5);

        let suhrn_pol_hodiny = Suhrn {
            cas_ms: 1_800_000,
            cisty_zisk: 100.0,
            ..Default::default()
        };
        assert_eq!(suhrn_pol_hodiny.zisk_za_hodinu(), 200.0);
    }

    #[test]
    fn test_suhrn_zisk_za_hodinu_nulovy_cas() {
        let suhrn = Suhrn {
            cas_ms: 0,
            cisty_zisk: 100.0,
            ..Default::default()
        };
        assert_eq!(suhrn.zisk_za_hodinu(), 0.0);
    }

    #[test]
    fn test_suhrn_zaporny_zisk() {
        let suhrn = Suhrn {
            cas_ms: 3_600_000,
            cisty_zisk: -50.0, // hrame do minusu
            ..Default::default()
        };
        assert_eq!(suhrn.zisk_za_hodinu(), -50.0);
    }

    #[test]
    fn test_suhrn_dropy_za_hodinu() {
        let suhrn = Suhrn {
            cas_ms: 3_600_000,
            hodnota_dropov: 500.0,
            ..Default::default()
        };
        assert_eq!(suhrn.dropy_za_hodinu(), 500.0);
    }

    #[test]
    fn test_suhrn_map_za_hodinu() {
        let suhrn = Suhrn {
            cas_ms: 1_800_000, // pol hodiny
            pocet_map: 5,
            ..Default::default()
        };
        assert_eq!(suhrn.map_za_hodinu(), 10.0);
    }

    #[test]
    fn test_suhrn_map_za_hodinu_nula() {
        let suhrn = Suhrn {
            cas_ms: 0,
            pocet_map: 5,
            ..Default::default()
        };
        assert_eq!(suhrn.map_za_hodinu(), 0.0);
    }

    #[test]
    fn test_suhrn_priemer_na_mapu() {
        let suhrn = Suhrn {
            pocet_map: 4,
            cisty_zisk: 100.0,
            ..Default::default()
        };
        assert_eq!(suhrn.priemer_na_mapu(), 25.0);
    }

    #[test]
    fn test_suhrn_priemer_na_mapu_nula() {
        let suhrn = Suhrn {
            pocet_map: 0,
            cisty_zisk: 100.0,
            ..Default::default()
        };
        assert_eq!(suhrn.priemer_na_mapu(), 0.0);
    }

    #[test]
    fn test_suhrn_priemerne_trvanie_ms() {
        let suhrn = Suhrn {
            pocet_map: 2,
            cas_ms: 10_000,
            ..Default::default()
        };
        assert_eq!(suhrn.priemerne_trvanie_ms(), 5_000);
    }

    #[test]
    fn test_prehlad_radenie_podla_zisku() {
        let slaba = dummy_vysledok(1, "Slaba", "Vstupy", 50.0, 3_600_000, "Z");
        let dobra = dummy_vysledok(2, "Dobra", "Vstupy", 50.0, 1_800_000, "Z"); // 2x rychlejsia, takze vacsi zisk/hod
        
        let p = prehlad(&[slaba, dobra], &[]);
        // Dobra ma zisk 100/hod, Slaba 50/hod
        assert_eq!(p.podla_kombinacie.len(), 2);
        assert_eq!(p.podla_kombinacie[0].kluc, "Dobra");
        assert_eq!(p.podla_kombinacie[1].kluc, "Slaba");
    }

    #[test]
    fn test_prehlad_radenie_determinizmus() {
        let rovnaka_b = dummy_vysledok(1, "B Kombinacia", "X", 100.0, 3_600_000, "Z");
        let rovnaka_a = dummy_vysledok(2, "A Kombinacia", "X", 100.0, 3_600_000, "Z");
        
        let p = prehlad(&[rovnaka_b, rovnaka_a], &[]);
        // Obe maju zisk 100/hod. Zoradit sa musia abecedne.
        assert_eq!(p.podla_kombinacie[0].kluc, "A Kombinacia");
        assert_eq!(p.podla_kombinacie[1].kluc, "B Kombinacia");
    }

    #[test]
    fn test_prehlad_dropy_a_bez_ceny() {
        let mut v1 = dummy_vysledok(1, "K", "V", 10.0, 1000, "Z");
        v1.bez_ceny = vec![(10, 5), (20, 1)];
        let mut v2 = dummy_vysledok(2, "K", "V", 10.0, 1000, "Z");
        v2.bez_ceny = vec![(10, 3)];

        let mut b1 = dummy_beh(1, false);
        b1.dropy.insert(100, 2);
        let mut b2 = dummy_beh(2, false);
        b2.dropy.insert(100, 8);
        b2.dropy.insert(200, 1);

        let p = prehlad(&[v1, v2], &[b1, b2]);

        assert_eq!(p.bez_ceny_spolu.get(&10), Some(&8));
        assert_eq!(p.bez_ceny_spolu.get(&20), Some(&1));
        assert_eq!(p.dropy_spolu.get(&100), Some(&10));
        assert_eq!(p.dropy_spolu.get(&200), Some(&1));
    }

    #[test]
    fn test_prehlad_vstupy_zlucovanie() {
        let m1 = dummy_vysledok(1, "K1", "RovnakeVstupy", 50.0, 1000, "Mapa1");
        let m2 = dummy_vysledok(2, "K2", "RovnakeVstupy", 50.0, 1000, "Mapa2");
        
        let p = prehlad(&[m1, m2], &[]);
        
        // Rozne kombinacie (kvoli mapam)
        assert_eq!(p.podla_kombinacie.len(), 2);
        // Ale vstupy su rovnake, mali by sa zlucit
        assert_eq!(p.podla_vstupov.len(), 1);
        assert_eq!(p.podla_vstupov[0].kluc, "RovnakeVstupy");
        assert_eq!(p.podla_vstupov[0].suhrn.cisty_zisk, 100.0);
    }
}