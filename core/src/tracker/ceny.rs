use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Odkiaľ cena prišla. Poradie variantov je zároveň poradie dôveryhodnosti.
/// Variant definovaný vyššie má menšiu číselnú hodnotu (napr. VlastnyTrh = 0),
/// takže pri porovnávaní znamená menšia hodnota vyššiu dôveryhodnosť.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum Zdroj {
    /// Hráč si položku sám vyhľadal na trhu — najpravdivejšie, čo vieme mať.
    #[default]
    VlastnyTrh,
    /// TLI 22: cena prečítaná z obrazovky `AuctionHouseShowItemGoodV2` cez
    /// Gemini vision (keď `XchgSearchPrice` SS14 do logu nepíše). Vidí sa len
    /// 3–5 najnižších viditeľných ponúk, nie celý trh — o niečo menej
    /// dôveryhodné než `VlastnyTrh`, stále vlastné pozorovanie hráča.
    Obrazovka,
    /// Prišlo od partie cez tli.tvojtatko.sk.
    Partia,
    /// Ručne zadané hráčom v nastaveniach.
    Rucne,
}

impl Zdroj {
    /// Dôveryhodnosť zdroja ako číslo 0–1. Používa sa spolu s váhou veku
    /// (`vaha_veku`) pri rozhodovaní, ktorá cena v cenníku vyhrá.
    pub fn doveryhodnost(self) -> f64 {
        match self {
            Zdroj::VlastnyTrh => 1.0,
            Zdroj::Obrazovka => 0.9,
            Zdroj::Partia => 0.8,
            Zdroj::Rucne => 0.6,
        }
    }

    /// Krátke označenie pre UI aj pre prenos na server.
    pub fn kod(self) -> &'static str {
        match self {
            Zdroj::VlastnyTrh => "vlastny_trh",
            Zdroj::Obrazovka => "obrazovka",
            Zdroj::Partia => "partia",
            Zdroj::Rucne => "rucne",
        }
    }

    pub fn z_kodu(kod: &str) -> Zdroj {
        match kod {
            "obrazovka" => Zdroj::Obrazovka,
            "partia" => Zdroj::Partia,
            "rucne" => Zdroj::Rucne,
            _ => Zdroj::VlastnyTrh,
        }
    }
}

/// Reprezentuje jednu záznamovú cenu pre položku na trhu.
/// Udržiavame kedy a odkiaľ prišla, aby sme vedeli staršie ceny
/// nahradiť novšími a menej presné presnejšími.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cena {
    pub polozka: u32,
    /// Referenčná cena, ktorou sa oceňuje. Pri vlastnom vyhľadaní je to 10.
    /// percentil ponúk, pri cene od partie medián cez hráčov.
    pub fe: f64,
    pub zdroj: Zdroj,
    /// Unixový čas v sekundách, kedy cena vznikla.
    pub kedy: u64,
    /// Medián ponúk z toho istého vyhľadania (TLI 20). Nepoužíva sa na
    /// ocenenie — je to kontrolné číslo: keď je medián mnohonásobne vyšší než
    /// p10, trh je riedky a cene sa dá veriť menej.
    pub median: Option<f64>,
    /// Koľko ponúk bolo v tom vyhľadaní. Tri ponuky nie sú trh.
    pub pocet_ponuk: u32,
    /// Koľko NEZÁVISLÝCH zdrojov (hráčov) sa na cene zhodlo. Pri vlastnom
    /// vyhľadaní 1, pri cene od partie počet prispievateľov.
    pub zdrojov: u32,
}

impl Cena {
    /// Cena z vlastného vyhľadania na in-game Exchange.
    pub fn vlastna(polozka: u32, p10: f64, median: f64, pocet_ponuk: u32, kedy: u64) -> Self {
        Self {
            polozka,
            fe: p10,
            zdroj: Zdroj::VlastnyTrh,
            kedy,
            median: Some(median),
            pocet_ponuk,
            zdrojov: 1,
        }
    }

    /// Cena prečítaná z obrazovky trhu (TLI 22) — `najnizsia` je najlacnejšia
    /// z 3–5 viditeľných ponúk. Bez mediánu: pri len niekoľkých viditeľných
    /// ponukách by medián nič navyše nepovedal oproti p10/minimu.
    pub fn z_obrazovky(polozka: u32, najnizsia: f64, pocet_ponuk: u32, kedy: u64) -> Self {
        Self {
            polozka,
            fe: najnizsia,
            zdroj: Zdroj::Obrazovka,
            kedy,
            median: None,
            pocet_ponuk,
            zdrojov: 1,
        }
    }
}

/// Dokedy je cena „čerstvá" — do 24 hodín má plnú váhu.
pub const CERSTVA_DO_S: u64 = 24 * 3600;

/// Za koľko ďalších sekúnd po uplynutí čerstvosti klesne váha na minimum.
const DOBEHOVE_OKNO_S: u64 = 7 * 24 * 3600;

/// Najnižšia váha, na ktorú môže cena klesnúť. Zámerne NIE nula — stará cena
/// je stále lepší odhad než žiadna; nula by znamenala, že beh s dropmi má
/// zrazu hodnotu 0 FE, a presne to bol pôvodný problém TLI 20.
pub const VAHA_MINIMUM: f64 = 0.25;

/// Váha ceny podľa veku: 1,0 do 24 h, potom lineárny pokles na `VAHA_MINIMUM`
/// počas nasledujúcich 7 dní, ďalej už konštantne `VAHA_MINIMUM`.
pub fn vaha_veku(vek_s: u64) -> f64 {
    if vek_s <= CERSTVA_DO_S {
        return 1.0;
    }
    let po = vek_s - CERSTVA_DO_S;
    if po >= DOBEHOVE_OKNO_S {
        return VAHA_MINIMUM;
    }
    let podiel = po as f64 / DOBEHOVE_OKNO_S as f64;
    1.0 - podiel * (1.0 - VAHA_MINIMUM)
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

    /// Vloží cenu. Staršia cena sa nikdy neprepíše ešte staršou; inak
    /// rozhoduje **dôveryhodnosť zdroja vynásobená váhou veku** k času novej
    /// ceny. Vracia `true`, keď sa cena naozaj zmenila.
    ///
    /// TLI 20: do tejto verzie platilo tvrdé „menej dôveryhodný zdroj
    /// neprepíše nikdy", takže **týždeň stará cena z vlastného vyhľadania
    /// navždy zablokovala desať minút starý medián od partie** — a cieľom
    /// celého TLI 20 je, aby ceny neboli zastarané. Preto sa teraz do
    /// porovnania započítava aj vek: čerstvá cena od partie starú vlastnú
    /// prebije, čerstvú vlastnú neprebije.
    pub fn vloz(&mut self, cena: Cena) -> bool {
        let teraz = cena.kedy;
        self.vloz_v_case(cena, teraz)
    }

    /// To isté ako `vloz`, len s výslovne zadaným „teraz" — používa sa, keď sa
    /// do cenníka sype dávka cien s rôznymi časmi a porovnávať sa má voči
    /// jednému okamihu.
    pub fn vloz_v_case(&mut self, cena: Cena, teraz: u64) -> bool {
        if let Some(stara) = self.mapa.get(&cena.polozka) {
            // Staršiu cenu nikdy nenahrádzame ešte staršou.
            if cena.kedy < stara.kedy {
                return false;
            }
            // Zhodný záznam netreba prepisovať.
            if cena == *stara {
                return false;
            }
            let sila_novej =
                cena.zdroj.doveryhodnost() * vaha_veku(teraz.saturating_sub(cena.kedy));
            let sila_starej =
                stara.zdroj.doveryhodnost() * vaha_veku(teraz.saturating_sub(stara.kedy));
            if sila_novej < sila_starej {
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
    /// Položky ocenené cenou staršou než 24 h (TLI 20). Hodnota sa NEZNIŽUJE —
    /// stará cena je stále najlepší odhad, aký máme — ale UI pri takom súčte
    /// ukáže, že je neistý, a Tracker to použije na výzvu „vyhľadaj na Exchange".
    pub zastarane: Vec<(u32, i64)>,
}

/// Ocení mapu `polozka -> pocet`. Neznáma položka nesmie spadnúť ani vynulovať výsledok —
/// zapíše sa do `bez_ceny`. Položka `POLOZKA_FE` má vždy hodnotu 1 FE za kus a daň sa na ňu
/// neuplatňuje, aj keď v cenníku nie je.
pub fn ocen(
    mnozstva: &BTreeMap<u32, i64>,
    cennik: &Cennik,
    nastavenie: &Nastavenie,
) -> Ocenenie {
    ocen_v_case(mnozstva, cennik, nastavenie, None)
}

/// To isté ako `ocen`, ale s výslovne zadaným „teraz" (unixový čas), aby sa
/// dalo zistiť, ktoré ceny sú staršie než 24 h. Keď je `teraz` `None`,
/// zastaranosť sa nevyhodnocuje (`Ocenenie::zastarane` ostane prázdne) —
/// `core` zámerne nesiaha na systémové hodiny, čas dodáva volajúci.
pub fn ocen_v_case(
    mnozstva: &BTreeMap<u32, i64>,
    cennik: &Cennik,
    nastavenie: &Nastavenie,
    teraz: Option<u64>,
) -> Ocenenie {
    let mut celkove_fe = 0.0;
    let mut bez_ceny = Vec::new();
    let mut zastarane = Vec::new();
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

            if let Some(t) = teraz {
                if t.saturating_sub(cena.kedy) > ZASTARANA_PO_S {
                    zastarane.push((polozka, pocet));
                }
            }

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
        zastarane,
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
            ..Default::default()
        }
    }

    #[test]
    fn zdroj_obrazovka_je_menej_doveryhodny_nez_vlastny_trh_ale_viac_nez_partia() {
        assert!(Zdroj::Obrazovka.doveryhodnost() < Zdroj::VlastnyTrh.doveryhodnost());
        assert!(Zdroj::Obrazovka.doveryhodnost() > Zdroj::Partia.doveryhodnost());
        assert_eq!(Zdroj::z_kodu("obrazovka"), Zdroj::Obrazovka);
        assert_eq!(Zdroj::Obrazovka.kod(), "obrazovka");
    }

    #[test]
    fn cena_z_obrazovky_ma_pocet_ponuk_a_bez_medianu() {
        let c = Cena::z_obrazovky(1001, 12.0, 3, 1000);
        assert_eq!(c.fe, 12.0);
        assert_eq!(c.zdroj, Zdroj::Obrazovka);
        assert_eq!(c.pocet_ponuk, 3);
        assert_eq!(c.median, None);
        assert_eq!(c.zdrojov, 1);
    }

    #[test]
    fn cerstva_cena_z_obrazovky_prebije_tyzden_stary_vlastny_trh() {
        // Rovnaký dôvod ako TLI 20 (vloz dokumentácia vyššie): stará, aj keď
        // dôveryhodnejšia, cena nesmie navždy zablokovať čerstvú.
        let mut cennik = Cennik::novy();
        let tyzden_s = 7 * 24 * 3600;
        cennik.vloz_v_case(vytvor_cenu(1001, 50.0, Zdroj::VlastnyTrh, 0), tyzden_s);
        let prebila = cennik.vloz_v_case(
            Cena::z_obrazovky(1001, 40.0, 3, tyzden_s),
            tyzden_s,
        );
        assert!(prebila);
        assert_eq!(cennik.cena(1001).unwrap().fe, 40.0);
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


    // ── TLI 20 ────────────────────────────────────────────────────────────

    #[test]
    fn test_vaha_veku() {
        assert_eq!(vaha_veku(0), 1.0);
        assert_eq!(vaha_veku(CERSTVA_DO_S), 1.0, "presne 24 h je ešte plná váha");
        // Deň po uplynutí čerstvosti je váha niekde medzi minimom a jednotkou.
        let v = vaha_veku(CERSTVA_DO_S + 86400);
        assert!(v < 1.0 && v > VAHA_MINIMUM, "{v}");
        // Veľmi stará cena neklesne na nulu — stará cena je lepšia než žiadna.
        assert_eq!(vaha_veku(365 * 86400), VAHA_MINIMUM);
    }

    #[test]
    fn test_cerstva_partia_prebije_staru_vlastnu() {
        // Presne to, kvôli čomu TLI 20 vzniklo: týždeň stará vlastná cena
        // nesmie navždy blokovať čerstvý medián od partie.
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(10, 50.0, Zdroj::VlastnyTrh, 0));
        let teraz = 10 * 86400;
        let z_partie = vytvor_cenu(10, 80.0, Zdroj::Partia, teraz - 600);
        assert!(cennik.vloz_v_case(z_partie, teraz));
        assert_eq!(cennik.cena(10).unwrap().fe, 80.0);
        assert_eq!(cennik.cena(10).unwrap().zdroj, Zdroj::Partia);
    }

    #[test]
    fn test_cerstva_vlastna_prebije_cerstvu_partiu() {
        let mut cennik = Cennik::novy();
        let teraz = 10 * 86400;
        cennik.vloz_v_case(vytvor_cenu(10, 80.0, Zdroj::Partia, teraz - 600), teraz);
        assert!(cennik.vloz_v_case(vytvor_cenu(10, 50.0, Zdroj::VlastnyTrh, teraz), teraz));
        assert_eq!(cennik.cena(10).unwrap().zdroj, Zdroj::VlastnyTrh);
    }

    #[test]
    fn test_ocen_oznaci_zastarane_ale_hodnotu_nezrazi() {
        let mut cennik = Cennik::novy();
        cennik.vloz(vytvor_cenu(1, 10.0, Zdroj::VlastnyTrh, 0));
        cennik.vloz(vytvor_cenu(2, 20.0, Zdroj::VlastnyTrh, 0));

        let mut mnozstva = BTreeMap::new();
        mnozstva.insert(1, 2);
        mnozstva.insert(2, 1);

        let teraz = 3 * 86400; // obe ceny sú 3 dni staré
        let v = ocen_v_case(&mnozstva, &cennik, &Nastavenie::default(), Some(teraz));
        assert_eq!(v.fe, 40.0, "zastaranosť hodnotu NEZNIŽUJE");
        assert_eq!(v.zastarane.len(), 2);

        // To isté hneď po zápise cien — nič zastarané.
        let v2 = ocen_v_case(&mnozstva, &cennik, &Nastavenie::default(), Some(100));
        assert!(v2.zastarane.is_empty());
    }

    #[test]
    fn test_cena_vlastna_nesie_median_a_pocet() {
        let c = Cena::vlastna(400021, 1.5, 2.0, 7, 1000);
        assert_eq!(c.fe, 1.5);
        assert_eq!(c.median, Some(2.0));
        assert_eq!(c.pocet_ponuk, 7);
        assert_eq!(c.zdrojov, 1);
        assert_eq!(c.zdroj, Zdroj::VlastnyTrh);
    }

    #[test]
    fn test_kod_zdroja_tam_a_spat() {
        for z in [Zdroj::VlastnyTrh, Zdroj::Partia, Zdroj::Rucne] {
            assert_eq!(Zdroj::z_kodu(z.kod()), z);
        }
        assert_eq!(Zdroj::z_kodu("niečo neznáme"), Zdroj::VlastnyTrh);
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