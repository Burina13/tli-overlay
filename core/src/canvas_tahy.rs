//! Fáza „Select the Brushstroke direction" / „Select Brushstroke Effect" —
//! ROZPOZNANIE podobrazovky a kariet z OCR textu (TLI 71,
//! `2026-10-20-1635-mcp.md`).
//!
//! **Čo tu je a čo zámerne NIE.** Appka si fázu aj texty kariet prečíta SAMA
//! (Windows OCR nad pevným výrezom `canvas_rozlozenie::ocr_oblast_kariet_box01`)
//! a pošle ich serveru parametrami `faza`/`karty` — rovnaký vzor a rovnaký
//! dôvod ako `ramy` pri TLI 69: appka má snímku v PLNOM rozlíšení, serveru ide
//! zmenšený JPEG, v ktorom je text karty na hrane čitateľnosti.
//!
//! Samotné BODOVANIE kariet (ktorú vybrať) tu **nie je** a je to vedomé
//! rozhodnutie, nie nedorobok: odpoveď závisí od stavu dosky — koľko je
//! voľných cieľových buniek a koľko vhodných paintov v ruke — a ten appka
//! lokálne nepozná (TLI 59b2c, 20.10.2026: lokálne rozpoznanie celého plátna
//! malo 7,7 % presnosť a vedome sa nezapojilo). Bodovanie preto žije len na
//! serveri (`stacks/games/tli/engine/canvas_tahy.py`), kde je doska z vision
//! modelu. Keby sa sem skopírovalo, bol by to druhý zdroj pravdy bez dát,
//! ktorými by sa dal použiť.
//!
//! Pravidlá a katalóg efektov: `stacks/games/tli/engine/canvas.yml` →
//! `tahy_stetcom` (repo `ops`).

use serde::{Deserialize, Serialize};

/// Nadpis, ktorým hra tú-ktorú podobrazovku označuje. Porovnáva sa
/// NORMALIZOVANE (malé písmená, len písmená/číslice, jednoduché medzery) —
/// OCR mieša veľkosť písmen aj počet medzier.
const NADPIS_VYBER_TAHU: &str = "select the brushstroke direction";
const NADPIS_VYBER_EFEKTU: &str = "select brushstroke effect";

/// Banner, ktorým hra oznamuje koniec behu (TLI 72) — po ňom sa už nekreslí,
/// ostáva len „Start Over" / „Complete". Porovnáva sa normalizovane ako
/// nadpisy vyššie. Zrkadlí `BANNER_VSETKO_POLOZENE` v `canvas_tahy.py`.
pub const BANNER_VSETKO_POLOZENE: &str = "all paint has been added";

/// Tri druhy ťahu štetcom. Poradie je zhodné s `DRUHY` v `canvas_tahy.py`.
pub const DRUHY: [&str; 3] = ["reimagine", "spread", "layer"];

/// Najviac kariet, ktoré sa z jednej obrazovky berú. Na všetkých štyroch
/// reálnych snímkach z 20.10.2026 boli DVE; strop je vyšší, aby tretia ponuka
/// appku nezhodila. Musí sedieť s `MAX_KARIET` v `canvas_tahy.py`.
///
/// **Prečo práve 3 a nie viac:** nad tlačidlom „Select" sa viac kariet
/// nezmestí — `canvas_rozlozenie::karta_box01(3)` by už ležal NA tlačidle
/// (y 0,726–0,819 vs. tlačidlo 0,753–0,806) a appka by z neho prečítala
/// slovo „Select" ako štvrtú, neexistujúcu kartu. Stráži to test
/// `ziadny_box_karty_neprekryva_tlacidlo` v `canvas_rozlozenie.rs`.
/// Nález z review diffu cez API kredit (20.10.2026).
pub const MAX_KARIET: usize = 3;

/// Ktorá podobrazovka Frozen Canvasu je otvorená.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Faza {
    /// Bežné plátno — umiestňujú sa painty (všetko, čo platilo pred TLI 71).
    Kreslenie,
    /// „Select the Brushstroke direction" — vyberá sa DRUH ťahu.
    VyberTahu,
    /// „Select Brushstroke Effect" — vyberá sa konkrétny efekt.
    VyberEfektu,
    /// „All paint has been added" — koniec behu (TLI 72). Karty tu nie sú,
    /// takže to NIE JE výber; radiť umiestnenie paintu už tiež nemá zmysel.
    VsetkoPolozene,
}

/// Všetky fázy v poradí `FAZY` z `canvas_tahy.py`.
pub const FAZY: [Faza; 4] = [Faza::Kreslenie, Faza::VyberTahu, Faza::VyberEfektu, Faza::VsetkoPolozene];

/// Fázy, v ktorých sú na obrazovke karty na výber — `FAZY_S_KARTAMI` na serveri.
pub const FAZY_S_KARTAMI: [Faza; 2] = [Faza::VyberTahu, Faza::VyberEfektu];

impl Faza {
    /// Hodnota, ktorá ide do query parametra `faza` servera.
    pub fn nazov(&self) -> &'static str {
        match self {
            Faza::Kreslenie => "kreslenie",
            Faza::VyberTahu => "vyber_tahu",
            Faza::VyberEfektu => "vyber_efektu",
            Faza::VsetkoPolozene => "vsetko_polozene",
        }
    }

    /// Je na obrazovke výber karty? Vymenúva sa KLADNE (nie „všetko okrem
    /// kreslenia"), aby nová fáza bez kariet — ako `VsetkoPolozene` — nezačala
    /// omylom spúšťať čítanie kariet.
    pub fn je_vyber(&self) -> bool {
        FAZY_S_KARTAMI.contains(self)
    }
}

/// Čo appka na podobrazovke prečítala.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VyberKariet {
    pub faza: Faza,
    /// Texty kariet zhora nadol, tak ako ich prečítalo OCR.
    pub karty: Vec<String>,
}

/// Text na porovnávanie: malé písmená, len písmená/číslice/medzery.
pub fn normalizuj(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut medzera = false;
    for z in text.chars() {
        if z.is_ascii_alphanumeric() {
            if medzera && !out.is_empty() {
                out.push(' ');
            }
            medzera = false;
            out.push(z.to_ascii_lowercase());
        } else {
            medzera = true;
        }
    }
    out
}

/// „[Reimagine] changes the Color Mixing…" → `"reimagine"`.
///
/// Páruje sa NA SLOVO V HRANATÝCH ZÁTVORKÁCH, nie na celú vetu — text karty
/// [Spread] je v hre USEKNUTÝ („[Spread] takes effect on" a nič za tým,
/// overené zväčšením výrezu snímky `2026-10-20-163311`), takže porovnanie
/// celej vety by zlyhalo práve na tej karte, na ktorej Richardovi záležalo.
/// Zátvorky sa nevyžadujú — OCR ich občas zje. Zrkadlí `druh_z_textu`
/// v `canvas_tahy.py`.
pub fn druh_z_textu(text: &str) -> &'static str {
    let n = normalizuj(text);
    for druh in DRUHY {
        if n.split(' ').any(|s| s == druh) {
            return druh;
        }
    }
    ""
}

/// Nadpis podobrazovky → `Faza`. `None`, keď to nie je ani jedna z nich.
pub fn faza_z_nadpisu(nadpis: &str) -> Option<Faza> {
    let n = normalizuj(nadpis);
    if n.is_empty() {
        return None;
    }
    if n.contains(NADPIS_VYBER_TAHU) {
        return Some(Faza::VyberTahu);
    }
    if n.contains(NADPIS_VYBER_EFEKTU) {
        return Some(Faza::VyberEfektu);
    }
    None
}

/// Je v texte (OCR bannera `canvas_rozlozenie::banner_box01`) oznam o konci
/// behu? Zámerne samostatná funkcia, nie vetva `faza_z_nadpisu` — banner leží
/// inde než nadpis kariet a `faza_z_nadpisu` sa pre ne správať nesmie inak.
/// Zrkadlí `je_banner_vsetko_polozene` v `canvas_tahy.py`.
pub fn je_banner_vsetko_polozene(text: &str) -> bool {
    normalizuj(text).contains(BANNER_VSETKO_POLOZENE)
}

/// Zloží hodnotu query parametra `karty` — texty oddelené `|`, zhora nadol.
/// Prázdne karty sa vynechajú, znak `|` sa z textu odstráni (inak by rozbil
/// delenie na serveri) a zoznam sa oreže na `MAX_KARIET`.
pub fn karty_do_parametra(karty: &[String]) -> String {
    karty
        .iter()
        .map(|k| k.replace('|', " ").split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|k| !k.is_empty())
        .take(MAX_KARIET)
        .collect::<Vec<_>>()
        .join("|")
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn druh_sa_pozna_zo_vsetkych_troch_kariet() {
        assert_eq!(
            druh_z_textu("[Reimagine] changes the Color Mixing Objective and Effect"),
            "reimagine"
        );
        assert_eq!(druh_z_textu("[Spread] takes effect on"), "spread");
        assert_eq!(
            druh_z_textu("[Layer] adds additional Color Mixing Effects"),
            "layer"
        );
    }

    #[test]
    fn spread_sa_pozna_aj_bez_zatvoriek_a_z_useknuteho_textu() {
        assert_eq!(druh_z_textu("Spread takes effect on"), "spread");
        assert_eq!(druh_z_textu("  SPREAD  "), "spread");
    }

    #[test]
    fn cudzi_text_nema_druh() {
        assert_eq!(druh_z_textu("Current Max Paint Level7"), "");
        assert_eq!(druh_z_textu(""), "");
        // „spreading" NIE JE „spread" — páruje sa celé slovo, nie podreťazec
        assert_eq!(druh_z_textu("spreading the paint"), "");
    }

    #[test]
    fn nadpis_urci_fazu() {
        assert_eq!(
            faza_z_nadpisu("Select the Brushstroke direction"),
            Some(Faza::VyberTahu)
        );
        assert_eq!(
            faza_z_nadpisu("  SELECT   THE  BRUSHSTROKE   DIRECTION "),
            Some(Faza::VyberTahu)
        );
        assert_eq!(
            faza_z_nadpisu("Select Brushstroke Effect"),
            Some(Faza::VyberEfektu)
        );
        assert_eq!(faza_z_nadpisu("Current Max Paint Level7"), None);
        assert_eq!(faza_z_nadpisu(""), None);
    }

    #[test]
    fn nazvy_faz_zodpovedaju_serveru() {
        assert_eq!(Faza::Kreslenie.nazov(), "kreslenie");
        assert_eq!(Faza::VyberTahu.nazov(), "vyber_tahu");
        assert_eq!(Faza::VyberEfektu.nazov(), "vyber_efektu");
        assert!(!Faza::Kreslenie.je_vyber());
        assert!(Faza::VyberTahu.je_vyber() && Faza::VyberEfektu.je_vyber());
        assert_eq!(Faza::VsetkoPolozene.nazov(), "vsetko_polozene");
    }

    /// Koniec behu nemá karty — keby `je_vyber()` vrátil `true`, appka by
    /// OCRovala prázdne boxy kariet a posielala serveru nezmyselné `karty`.
    #[test]
    fn vsetko_polozene_nie_je_vyber() {
        assert!(!Faza::VsetkoPolozene.je_vyber());
        let s_kartami: Vec<Faza> = FAZY.iter().copied().filter(|f| f.je_vyber()).collect();
        assert_eq!(s_kartami, FAZY_S_KARTAMI.to_vec());
    }

    #[test]
    fn fazy_su_v_poradi_servera() {
        let nazvy: Vec<&str> = FAZY.iter().map(|f| f.nazov()).collect();
        assert_eq!(nazvy, vec!["kreslenie", "vyber_tahu", "vyber_efektu", "vsetko_polozene"]);
    }

    #[test]
    fn banner_konca_behu_sa_pozna_aj_z_hrubeho_ocr() {
        assert!(je_banner_vsetko_polozene("All paint has been added"));
        assert!(je_banner_vsetko_polozene("  ALL PAINT  has been added! "));
        assert!(je_banner_vsetko_polozene("x All\npaint has-been added."));
        assert!(!je_banner_vsetko_polozene("All paint has been"));
        assert!(!je_banner_vsetko_polozene(""));
        assert!(!je_banner_vsetko_polozene("Select the Brushstroke direction"));
    }

    /// Zadanie TLI 72: `faza_z_nadpisu` sa pre doterajšie nadpisy nemení a
    /// banner konca behu ňou NEPREJDE — rozpoznáva sa vlastnou funkciou.
    #[test]
    fn faza_z_nadpisu_banner_nepozna() {
        assert_eq!(faza_z_nadpisu("All paint has been added"), None);
        assert_eq!(faza_z_nadpisu("Select Brushstroke Effect"), Some(Faza::VyberEfektu));
    }

    #[test]
    fn parameter_karty_spoji_texty_rurkou() {
        let k = vec![
            "[Reimagine] changes the Color Mixing Objective and Effect".to_string(),
            "[Spread] takes effect on".to_string(),
        ];
        assert_eq!(
            karty_do_parametra(&k),
            "[Reimagine] changes the Color Mixing Objective and Effect|[Spread] takes effect on"
        );
    }

    #[test]
    fn parameter_karty_vyhodi_rurku_z_textu_a_prazdne_karty() {
        let k = vec![
            "a|b".to_string(),
            "   ".to_string(),
            "c".to_string(),
        ];
        assert_eq!(karty_do_parametra(&k), "a b|c");
    }

    #[test]
    fn parameter_karty_oreze_na_strop() {
        let k: Vec<String> = (0..10).map(|i| format!("karta{i}")).collect();
        assert_eq!(karty_do_parametra(&k).split('|').count(), MAX_KARIET);
    }

    #[test]
    fn normalizacia_zrazi_medzery_aj_interpunkciu() {
        assert_eq!(normalizuj("[Spread]  takes   effect on!"), "spread takes effect on");
        assert_eq!(normalizuj(""), "");
        assert_eq!(normalizuj("---"), "");
    }
}
