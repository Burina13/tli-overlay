//! Vzhľad trasy nad minimapou (TLI 62, krok 62/2b).
//!
//! Zadanie: doplnok 1 k TLI 62 (`inbox/done/2026-10-20-0158-mcp.md`, bod 4).
//! Richard 20.10.2026: „nech sa tam dá nastaviť opacita“ — konkrétne posuvník
//! priehľadnosti **čiary** aj **celej vrstvy minimapy** (0–100 %), k tomu
//! hrúbka a farba čiary, a rýchla zmena **kolieskom myši nad úchytom
//! overlayu**. Všetko sa ukladá do nastavení.
//!
//! Prečo vlastný modul a nie `Rozlozenie::priehladnost`: tá je spoločná pre
//! VŠETKY panely nad hrou (F8 doska, mini-panel, toast) a má dolnú medzu
//! `0,3`, aby sa panel s textom nedal stiahnuť do nečitateľna. Trasa nad
//! minimapou je iný prípad — je to tenká čiara v hre, pri ktorej Richard
//! výslovne chce **až po 0 %** (teda „dočasne to nechcem vidieť, ale
//! nevypínam to"). Zliatie oboch by znamenalo, že stíšenie trasy stiahne aj
//! panel s radou solvera, čo nikto nechcel.
//!
//! Modul je úmyselne bez obrazovky: `cargo test` ho preverí aj na Linuxe.
//! Kreslenie samotnej čiary je až krok 62/8.

use serde::{Deserialize, Serialize};

/// Predvolená farba trasy — teplá žltá. Minimapa TLI je modro-sivá a herné
/// ikony sú prevažne červené/zelené, takže žltá je na nej vidieť a zároveň sa
/// nepletie s ikonou nepriateľa.
pub const FARBA_PREDVOLENA: &str = "#FFD24A";

/// Rozsah hrúbky čiary v CSS pixeloch. Pod 1 px sa čiara na škálovanom
/// displeji stráca, nad 12 px prekryje samotnú minimapu.
pub const HRUBKA_MIN: u8 = 1;
pub const HRUBKA_MAX: u8 = 12;

/// O koľko percent posunie jeden „klik" kolieska myši nad úchytom overlayu.
/// Päť percent je kompromis: 20 klikov cez celý rozsah je ešte rýchle, ale
/// dá sa ním trafiť aj konkrétna hodnota.
pub const KROK_KOLIESKOM: i32 = 5;

/// Vzhľad trasy a vrstvy minimapy. Percentá, nie `f64` 0–1: posuvník v UI je
/// v percentách, hráč hodnotu vidí v percentách a celé číslo sa v JSON
/// nastavení nerozsype na `0.30000000000000004`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VzhladTrasy {
    /// Priehľadnosť samotnej ČIARY trasy, 0–100 %.
    pub opacita_ciary: u8,
    /// Priehľadnosť CELEJ vrstvy, ktorú appka kreslí nad minimapu (čiara,
    /// značky cieľov, šípka na okraji). Násobí sa s `opacita_ciary` —
    /// vrstva je „hlavný vypínač hlasitosti", čiara doladenie.
    pub opacita_vrstvy: u8,
    /// Hrúbka čiary v CSS pixeloch, `HRUBKA_MIN`..=`HRUBKA_MAX`.
    pub hrubka_px: u8,
    /// Farba čiary ako `#RRGGBB`. Neplatná hodnota padá na `FARBA_PREDVOLENA`
    /// — appka nesmie skončiť s neviditeľnou alebo rozbitou čiarou preto, že
    /// niekto ručne upravil `nastavenia.json`.
    pub farba: String,
}

impl Default for VzhladTrasy {
    fn default() -> Self {
        Self {
            // 85 %, nie 100 %: trasa má byť vidieť, ale hráč musí cez ňu
            // stále rozoznať, čo je pod ňou na minimape.
            opacita_ciary: 85,
            opacita_vrstvy: 100,
            hrubka_px: 3,
            farba: FARBA_PREDVOLENA.to_string(),
        }
    }
}

/// Je to platný `#RRGGBB`? (Skratka `#RGB` sa zámerne neprijíma — UI posiela
/// vždy plný tvar z `<input type="color">` a prijímať dva tvary by znamenalo
/// dve cesty, ktoré by sa dali rozísť.)
fn je_platna_farba(text: &str) -> bool {
    let t = text.trim();
    t.len() == 7
        && t.starts_with('#')
        && t[1..].chars().all(|z| z.is_ascii_hexdigit())
}

impl VzhladTrasy {
    /// Orezanie do platných medzí. Volá sa pri každom načítaní aj uložení
    /// nastavení — ručne upravený alebo starý súbor nesmie appku dostať do
    /// stavu, z ktorého sa hráč bez editora nedostane.
    pub fn normalizuj(&mut self) {
        self.opacita_ciary = self.opacita_ciary.min(100);
        self.opacita_vrstvy = self.opacita_vrstvy.min(100);
        self.hrubka_px = self.hrubka_px.clamp(HRUBKA_MIN, HRUBKA_MAX);
        let f = self.farba.trim().to_uppercase();
        self.farba = if je_platna_farba(&f) { f } else { FARBA_PREDVOLENA.to_string() };
    }

    /// Výsledná priehľadnosť čiary 0–1 pre kreslenie: vrstva × čiara.
    /// Jedno miesto, kde sa tie dve percentá skladajú — aby sa 62/8 nemuselo
    /// rozhodovať, či ich násobiť, alebo brať menšiu z nich.
    pub fn vysledna_opacita(&self) -> f64 {
        (self.opacita_ciary.min(100) as f64 / 100.0) * (self.opacita_vrstvy.min(100) as f64 / 100.0)
    }

    /// Farba pripravená na kreslenie, aj s výslednou priehľadnosťou.
    /// `rgba(…)`, nie `#RRGGBBAA` — Canvas 2D aj CSS berú oboje, ale `rgba`
    /// je čitateľné v logu, keď sa bude zisťovať, prečo nie je čiaru vidieť.
    pub fn farba_rgba(&self) -> String {
        let f = if je_platna_farba(&self.farba) { self.farba.trim() } else { FARBA_PREDVOLENA };
        let zlozka = |od: usize| u8::from_str_radix(&f[od..od + 2], 16).unwrap_or(0);
        format!(
            "rgba({}, {}, {}, {:.3})",
            zlozka(1),
            zlozka(3),
            zlozka(5),
            self.vysledna_opacita()
        )
    }

    /// Krok kolieskom myši nad úchytom overlayu (zadanie: „rýchlo cez koliesko
    /// myši"). `smer` je +1 hore / −1 dole; mení sa **opacita vrstvy**, teda
    /// to, čo hráč vníma ako „stíš mi to celé", nie jedna zložka.
    ///
    /// Vracia novú hodnotu, aby ju volajúci vedel hneď ukázať v bubline bez
    /// druhého čítania nastavení.
    pub fn koliesko(&mut self, smer: i32) -> u8 {
        let nova = (self.opacita_vrstvy as i32 + smer.signum() * KROK_KOLIESKOM).clamp(0, 100);
        self.opacita_vrstvy = nova as u8;
        nova as u8
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn predvolene_hodnoty_su_viditelna_ciara() {
        let v = VzhladTrasy::default();
        assert_eq!(v.opacita_ciary, 85);
        assert_eq!(v.opacita_vrstvy, 100);
        assert_eq!(v.hrubka_px, 3);
        assert_eq!(v.farba, FARBA_PREDVOLENA);
        assert!(v.vysledna_opacita() > 0.5, "predvolene musi byt trasu vidiet");
    }

    #[test]
    fn normalizuj_oreze_opacitu_aj_hrubku() {
        let mut v = VzhladTrasy { opacita_ciary: 200, opacita_vrstvy: 101, hrubka_px: 0, farba: "x".into() };
        v.normalizuj();
        assert_eq!(v.opacita_ciary, 100);
        assert_eq!(v.opacita_vrstvy, 100);
        assert_eq!(v.hrubka_px, HRUBKA_MIN);
        assert_eq!(v.farba, FARBA_PREDVOLENA, "neplatna farba padne na predvolenu");

        let mut v = VzhladTrasy { hrubka_px: 99, ..Default::default() };
        v.normalizuj();
        assert_eq!(v.hrubka_px, HRUBKA_MAX);
    }

    /// Nula je PLATNÁ hodnota — „dočasne to nechcem vidieť, ale nevypínam to".
    /// Keby ju normalizácia dvíhala na nejaké minimum (ako `Rozlozenie`
    /// dvíha na 0,3), zadanie „posuvník 0–100 %" by sa nedalo splniť.
    #[test]
    fn nula_prezije_normalizaciu() {
        let mut v = VzhladTrasy { opacita_ciary: 0, opacita_vrstvy: 0, ..Default::default() };
        v.normalizuj();
        assert_eq!(v.opacita_ciary, 0);
        assert_eq!(v.opacita_vrstvy, 0);
        assert_eq!(v.vysledna_opacita(), 0.0);
    }

    #[test]
    fn farba_sa_normalizuje_na_velke_pismena_a_prijme_sa_mala() {
        let mut v = VzhladTrasy { farba: "  #ff0080 ".into(), ..Default::default() };
        v.normalizuj();
        assert_eq!(v.farba, "#FF0080");
    }

    #[test]
    fn vysledna_opacita_je_sucin_oboch() {
        let v = VzhladTrasy { opacita_ciary: 50, opacita_vrstvy: 50, ..Default::default() };
        assert!((v.vysledna_opacita() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn farba_rgba_rozlozi_zlozky_a_doplni_alfu() {
        let v = VzhladTrasy { farba: "#FFD24A".into(), opacita_ciary: 100, opacita_vrstvy: 100, hrubka_px: 3 };
        assert_eq!(v.farba_rgba(), "rgba(255, 210, 74, 1.000)");

        let v = VzhladTrasy { farba: "#000000".into(), opacita_ciary: 50, opacita_vrstvy: 100, hrubka_px: 3 };
        assert_eq!(v.farba_rgba(), "rgba(0, 0, 0, 0.500)");
    }

    /// Neplatná farba nesmie zhodiť kreslenie ani vrátiť prázdny reťazec —
    /// `farba_rgba` musí dať použiteľnú hodnotu aj bez predošlej normalizácie.
    #[test]
    fn farba_rgba_prezije_rozbitu_hodnotu() {
        let v = VzhladTrasy { farba: "nezmysel".into(), ..Default::default() };
        assert!(v.farba_rgba().starts_with("rgba(255, 210, 74,"));
    }

    #[test]
    fn koliesko_hybe_opacitou_vrstvy_po_krokoch_a_zastavi_na_medziach() {
        let mut v = VzhladTrasy::default();
        assert_eq!(v.koliesko(-1), 95);
        assert_eq!(v.koliesko(-1), 90);
        assert_eq!(v.koliesko(1), 95);
        for _ in 0..50 {
            v.koliesko(-1);
        }
        assert_eq!(v.opacita_vrstvy, 0, "dole sa zastavi na nule, nepretecie");
        for _ in 0..50 {
            v.koliesko(1);
        }
        assert_eq!(v.opacita_vrstvy, 100, "hore sa zastavi na stovke");
    }

    /// Koliesko sa nesmie dotknúť ostatných polí — hráč točí kolieskom, nie
    /// aby si prefarbil trasu.
    #[test]
    fn koliesko_nemeni_farbu_hrubku_ani_opacitu_ciary() {
        let mut v = VzhladTrasy { opacita_ciary: 40, hrubka_px: 7, farba: "#112233".into(), opacita_vrstvy: 60 };
        v.koliesko(-1);
        assert_eq!(v.opacita_ciary, 40);
        assert_eq!(v.hrubka_px, 7);
        assert_eq!(v.farba, "#112233");
        assert_eq!(v.opacita_vrstvy, 55);
    }

    /// Hodnota kolieska väčšia než 1 (niektoré myši posielajú násobky) sa
    /// berie len ako SMER — inak by jedno „švihnutie" skočilo z 100 na 0.
    #[test]
    fn koliesko_berie_len_smer_nie_velkost() {
        let mut v = VzhladTrasy::default();
        assert_eq!(v.koliesko(-120), 95);
        assert_eq!(v.koliesko(0), 95, "nulovy smer nic nemeni");
    }
}
