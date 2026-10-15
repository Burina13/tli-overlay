use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Odkiaľ cena prišla. Poradie variantov je zároveň poradie dôveryhodnosti.
/// Variant definovaný vyššie má menšiu číselnú hodnotu (napr. VlastnyTrh = 0),
/// takže pri porovnávaní znamená menšia hodnota vyššiu dôveryhodnosť.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Zdroj {
    /// Hráč si položku sám vyhľadal na trhu — najpravdivejšie, čo vieme mať.
    VlastnyTrh,
    /// Prišlo od partie cez tli.tvojtatko.sk.
    Partia,
    /// Ručne zadané hráčom v nastaveniach.
    Rucne,
}

/// Reprezentuje jednu záznamovú cenu pre položku na trhu.
/// Udržiavame kedy a odkiaľ prišla, aby sme vedeli staršie ceny
/// nahradiť novšími a menej presné presnejšími.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cena {
    pub polozka: u32,
    pub fe: f64,
    pub zdroj: Zdroj,
    /// Unixový čas v sekundách, kedy cena vznikla.
    pub kedy: u64,
}

/// Ako starú cenu ešte považujeme za použiteľnú, kým ju označíme ako zastaranú.
/// Hodnota je 24 hodín (v sekundách).
pub const ZASTARANA_PO_S: u64 = 24 * 3600;

/// Zoznam cien pre všetky známe položky.
/// Interná štruktúra zaručuje, že pri exporte budú ceny zoradené podľa ID položky.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cennik {
    mapa: BTreeMap<u32, Cena>,
}

impl Cennik {
    /// Vytvorí nový, prázdny cenník.
    pub fn novy() -> Self {
        Self {
            mapa: BTreeMap::new(),
        }
    }

    /// Vloží cenu. Staršiu cenu NEPREPÍŠE novšou z menej dôveryhodného zdroja,
    /// ale vždy prepíše, keď je nová cena z rovnako alebo viac dôveryhodného zdroja
    /// a je novšia. Vracia `true`, keď sa cena naozaj zmenila.
    pub fn vloz(&mut self, cena: Cena) -> bool {
        if let Some(stara) = self.mapa.get(&cena.polozka) {
            // Ak je nová cena z horšieho (menej dôveryhodného) zdroja, odmietneme ju.
            // (V enum Zdroj znamená vyššia hodnota menšiu dôveryhodnosť)
            if cena.zdroj > stara.zdroj {
                return false;
            }
            // Ak je z rovnakého alebo lepšieho zdroja, ale je staršia, odmietneme ju.
            if cena.kedy < stara.kedy {
                return false;
            }
            // Ak sa čas aj zdroj zhodujú a aj samotná cena je rovnaká, nevykonávame zmenu.
            if cena == *stara {
                return false;
            }
        }
        self.mapa.insert(cena.polozka, cena);
        true
    }

    /// Vráti referenciu na cenu položky, ak ju cenník obsahuje.
    pub fn cena(&self, polozka: u32) -> Option<&Cena> {
        self.mapa.get(&polozka)
    }

    /// Vek ceny v sekundách voči zadanému času `teraz`.
    /// Ak cenu nepoznáme, vracia `None`.
    pub fn vek_s(&self, polozka: u32, teraz: u64) -> Option<u64> {
        self.cena(polozka).map(|c| teraz.saturating_sub(c.kedy))
    }

    /// Zistí, či je cena pre danú položku zastaraná (staršia ako ZASTARANA_PO_S).
    /// Ak cenu nemáme, vracia false (cena neexistuje, nedá sa určiť ako zastaraná).
    pub fn je_zastarana(&self, polozka: u32, teraz: u64) -> bool {
        self.vek_s(polozka, teraz)
            .map_or(false, |vek| vek > ZASTARANA_PO_S)
    }

    /// Vráti celkový počet položiek, pre ktoré poznáme cenu.
    pub fn pocet(&self) -> usize {
        self.mapa.len()
    }

    /// Vráti všetky ceny zoradené podľa ID položky (zabezpečené použitím BTreeMap).
    pub fn vsetky(&self) -> Vec<&Cena> {
        self.mapa.values().collect()
    }
}

/// Nastavenie pre spôsob oceňovania množstva položiek.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Nastavenie {
    /// Odpočítať daň z predaja (12,5 %) — na samotné FE sa NIKDY neuplatní.
    pub dan: bool,
}

impl Default for Nastavenie {
    fn default() -> Self {
        Self { dan: false }
    }
}

/// Výsledok ocenenia jedného zoznamu položiek.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ocenenie {
    /// Súčet hodnoty v FE.
    pub fe: f64,
    /// Položky, pre ktoré cena nie je známa: `(polozka, pocet)`. Zobrazia sa ako „bez ceny".
    pub bez_ceny: Vec<(u32, i64)>,
    /// Najstaršia použitá cena (unixový čas) — z nej sa v UI kreslí vek celého súčtu.
    pub najstarsia_cena: Option<u64>,
}

/// Ocení mapu `polozka -> pocet`. Neznáma položka nesmie spadnúť ani vynulovať výsledok —
/// zapíše sa do `bez_ceny`. Položka `POLOZKA_FE` má vždy hodnotu 1 FE za kus a daň sa na ňu
/// neuplatňuje, aj keď v cenníku nie je.
pub fn ocen(
    mnozstva: &BTreeMap<u32, i64>,
    cennik: &Cennik,
    nastavenie: &Nastavenie,
) -> Ocenenie {
    let mut celkove_fe = 0.0;
    let mut bez_ceny = Vec::new();
    let mut najstarsia_cena: Option<u64> = None;

    for (&polozka, &pocet) in mnozstva {
        // Samotná mena FE sa vždy počíta ako 1:1, bez ohľadu na dane a cenník.
        if polozka == super::stav::POLOZKA_FE {
            celkove_fe += pocet as f64;
            continue;
        }

        if let Some(cena) = cennik.cena(polozka) {
            let mut hodnota_polozky = cena.fe * (pocet as f64);
            if nastavenie.dan {
                // Odpočítame daň 12,5 % z predaja (zostáva 87,5 %)
                hodnota_polozky *= 0.875;
            }
            celkove_fe += hodnota_polozky;

            // Udržujeme najstaršiu použitú cenu, aby sme vedeli zobraziť vek celého ocenenia.
            match najstarsia_cena {
                Some(doterajsi) => {
                    if cena.kedy < doterajsi {
                        najstarsia_cena = Some(cena.kedy);
                    }
                }
                None => {
                    najstarsia_cena = Some(cena.kedy);
                }
            }
        } else {
            // Položka sa nenašla v cenníku
            bez_ceny.push((polozka, pocet));
        }
    }

    Ocenenie {
        fe: celkove_fe,
        bez_ceny,
        najstarsia_cena,
    }
}

/// Vek v ľudskej podobe po slovensky.
/// Hranice: < 60 s = „teraz"; < 60 min = minúty; < 48 h = hodiny; inak dni.
pub fn vek_text(sekundy: u64) -> String {
    if sekundy < 60 {
        "teraz".to_string()
    } else if sekundy < 3600 {
        format!("pred {} min", sekundy / 60)
    } else if sekundy < 48 * 3600 {
        format!("pred {} h", sekundy / 3600)
    } else {
        let dni = sekundy / 86400;
        if dni == 1 {
            // Zriedkavý prípad kvôli < 48h hranici, ale prítomný pre istotu a správnosť
            "pred 1 dňom".to_string()
        } else {
            format!("pred {} dňami", dni)
        }
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use std::collections::BTreeMap;

    // Pridáme vlastný mock pre POLOZKA_FE do `super::stav` v rámci testov,
    // aby sa dal modul reálne preložiť, pokiaľ je nezávisle testovaný.
    mod super_mock {
        pub mod stav {
            pub const POLOZKA_FE: u32 = 100300;
        }
    }
    use super_mock::stav::POLOZKA_FE;

    fn vytvor_cenu(polozka: u32, fe: f64, zdroj: Zdroj, kedy: u64) -> Cena {
        Cena {
            polozka,
            fe,
            zdroj,
            kedy,
        }
    }

    #[test]
    fn test_vlozenie_a_precitanie() {
        let mut cennik = Cennik::novy();
        let cena = vytvor_cenu(10, 50.0, Zdroj::Rucne, 1000);
        assert!(cennik.vloz(cena.clone()));
        assert_eq!(cennik.cena(10), Some(&cena));
        assert_eq!(cennik.pocet(), 1);
    }

    #[test]
    fn test_neprepisanie_novsej_starou() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(10, 50.0, Zdroj::Partia, 2000));
        let starsia = vytvor_cenu(10, 40.0, Zdroj::Partia, 1000);
        assert!(!cennik.vloz(starsia));
        assert_eq!(cennik.cena(10).unwrap().fe, 50.0);
    }

    #[test]
    fn test_prepis_novsou_z_rovnakeho() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(10, 50.0, Zdroj::Partia, 1000));
        let novsia = vytvor_cenu(10, 60.0, Zdroj::Partia, 2000);
        assert!(cennik.vloz(novsia));
        assert_eq!(cennik.cena(10).unwrap().fe, 60.0);
    }

    #[test]
    fn test_neprepisanie_menej_doveryhodnou() {
        let mut cennik = Cennik::novy();
        // VlastnyTrh je najlepší
        cennik.vloz(vytvor_cenu(10, 50.0, Zdroj::VlastnyTrh, 1000));
        // Partia je horší zdroj (menej dôveryhodný), takže ani novšia cena ho neprepíše
        let menej_doveryhodna = vytvor_cenu(10, 60.0, Zdroj::Partia, 2000);
        assert!(!cennik.vloz(menej_doveryhodna));
        assert_eq!(cennik.cena(10).unwrap().fe, 50.0);
    }

    #[test]
    fn test_prednost_vlastny_trh_pred_partia() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(10, 50.0, Zdroj::Partia, 1000));
        // Lepší zdroj (VlastnyTrh) prepíše Partiu, ak je cena aspoň tak nová
        let lepsia = vytvor_cenu(10, 60.0, Zdroj::VlastnyTrh, 1500);
        assert!(cennik.vloz(lepsia));
        assert_eq!(cennik.cena(10).unwrap().zdroj, Zdroj::VlastnyTrh);
        assert_eq!(cennik.cena(10).unwrap().fe, 60.0);
    }

    #[test]
    fn test_vek_s() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(10, 50.0, Zdroj::Partia, 1000));
        assert_eq!(cennik.vek_s(10, 1050), Some(50));
        assert_eq!(cennik.vek_s(99, 1050), None);
    }

    #[test]
    fn test_je_zastarana_na_hranici() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(10, 50.0, Zdroj::Partia, 1000));
        // Tesne na hranici
        assert!(!cennik.je_zastarana(10, 1000 + ZASTARANA_PO_S));
        // O sekundu neskôr
        assert!(cennik.je_zastarana(10, 1000 + ZASTARANA_PO_S + 1));
    }

    #[test]
    fn test_ocen_plne_zname() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(1, 10.0, Zdroj::Partia, 1000));
        cennik.vloz(vytvor_cenu(2, 20.0, Zdroj::Partia, 1000));

        let mut mnozstva = BTreeMap::new();
        mnozstva.insert(1, 2); // 2 * 10 = 20
        mnozstva.insert(2, 3); // 3 * 20 = 60

        let nastavenie = Nastavenie { dan: false };
        let vysledok = ocen(&mnozstva, &cennik, &nastavenie);
        
        assert_eq!(vysledok.fe, 80.0);
        assert!(vysledok.bez_ceny.is_empty());
    }

    #[test]
    fn test_ocen_nezname() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(1, 10.0, Zdroj::Partia, 1000));

        let mut mnozstva = BTreeMap::new();
        mnozstva.insert(1, 2); // 20
        mnozstva.insert(99, 5); // Bez ceny

        let nastavenie = Nastavenie { dan: false };
        let vysledok = ocen(&mnozstva, &cennik, &nastavenie);
        
        assert_eq!(vysledok.fe, 20.0);
        assert_eq!(vysledok.bez_ceny.len(), 1);
        assert_eq!(vysledok.bez_ceny[0], (99, 5));
    }

    #[test]
    fn test_ocen_s_fe() {
        let cennik = Cennik::novy(); // Prázdny cenník
        let mut mnozstva = BTreeMap::new();
        mnozstva.insert(POLOZKA_FE, 150); // 150 FE

        let nastavenie = Nastavenie { dan: true }; // Daň zapnutá, ale FE ju ignoruje
        let vysledok = ocen(&mnozstva, &cennik, &nastavenie);
        
        assert_eq!(vysledok.fe, 150.0);
        assert!(vysledok.bez_ceny.is_empty());
        assert_eq!(vysledok.najstarsia_cena, None); // FE nemá vek ceny
    }

    #[test]
    fn test_dan_iba_na_beznu() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(1, 100.0, Zdroj::Partia, 1000)); // Bežná položka za 100 FE

        let mut mnozstva = BTreeMap::new();
        mnozstva.insert(1, 1); // 1 ks * 100 FE
        mnozstva.insert(POLOZKA_FE, 50); // 50 FE

        let nastavenie = Nastavenie { dan: true };
        let vysledok = ocen(&mnozstva, &cennik, &nastavenie);
        
        // 100 - 12.5% = 87.5 FE. FE zostáva 50.0. Spolu 137.5 FE.
        assert_eq!(vysledok.fe, 137.5);
    }

    #[test]
    fn test_najstarsia_cena() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(1, 10.0, Zdroj::Partia, 2000));
        cennik.vloz(vytvor_cenu(2, 20.0, Zdroj::Partia, 1500)); // Toto je najstaršia použitá cena
        cennik.vloz(vytvor_cenu(3, 30.0, Zdroj::Partia, 1000)); // Ešte staršia, ale nefiguruje v ocenení

        let mut mnozstva = BTreeMap::new();
        mnozstva.insert(1, 1);
        mnozstva.insert(2, 1);

        let vysledok = ocen(&mnozstva, &cennik, &Nastavenie::default());
        assert_eq!(vysledok.najstarsia_cena, Some(1500));
    }

    #[test]
    fn test_vek_text() {
        // Teraz (< 60s)
        assert_eq!(vek_text(59), "teraz");
        assert_eq!(vek_text(0), "teraz");
        
        // Minúty (< 60m / 3600s)
        assert_eq!(vek_text(60), "pred 1 min");
        assert_eq!(vek_text(120), "pred 2 min");
        assert_eq!(vek_text(300), "pred 5 min");
        
        // Hodiny (< 48h / 172800s)
        assert_eq!(vek_text(3600), "pred 1 h");
        assert_eq!(vek_text(7200), "pred 2 h");
        assert_eq!(vek_text(86400), "pred 24 h"); // Pod 48h je stále hodiny!
        assert_eq!(vek_text(172799), "pred 47 h");
        
        // Dni (>= 48h)
        assert_eq!(vek_text(172800), "pred 2 dňami");
        assert_eq!(vek_text(432000), "pred 5 dňami");
    }
}