//! Uložené rozloženie okien nad hrou (TLI 21, `2026-10-16-1528-mcp.md` bod 1).
//!
//! Richard chce vedieť posunúť a zmeniť veľkosť všetkému, čo appka kreslí nad
//! hrou: hlavný panel (F8 Tartarus / F9 Canvas — zdieľajú jeden DOM prvok),
//! mini-panel trackera aj toast pripomienky. Poloha a veľkosť sa ukladajú ako
//! ZLOMOK (0–1) rozmerov okna hry, nie v pixeloch — vďaka tomu sedí rozloženie
//! aj po zmene rozlíšenia hry, a `suradnice::box01_na_css`/`box01_na_css` s
//! `mierka = 1.0` ho vie priamo prepočítať na fyzické pixely okna (natívne
//! Tauri okná mini/pripomienka) alebo s reálnou mierkou monitora na CSS
//! pixely (DOM panel vnútri okna `overlay`) — JEDNA matematika pre oba prípady,
//! tá istá, čo už pokrývajú testy v `suradnice.rs` pre DPI 100/125/150 %.
//!
//! Kľúčom je dvojica (panel, monitor) — `monitor_signatura` z `suradnice.rs` —
//! lebo Richard môže hrať na viacerých monitoroch s rôznym rozlíšením a chce
//! si rozloženie pamätať pre každý zvlášť.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identifikátory panelov, ktoré appka vie posúvať. Reťazec, nie enum —
/// ukladá sa priamo ako kľúč mapy a Artistry (zatiaľ bez overlay UI, TLI 8/4d)
/// pribudne bez zmeny formátu, len novou hodnotou.
pub const PANEL_HLAVNY: &str = "panel";
pub const PANEL_MINI: &str = "mini";
pub const PANEL_PRIPOMIENKA: &str = "pripomienka";

/// Najmenšia prípustná šírka/výška — bez nej by sa dal panel pretiahnutím
/// zmenšiť na nulu a zmiznúť bez spôsobu, ako ho znova nájsť (okrem
/// „Obnoviť predvolené“).
pub const MIN_ROZMER: f64 = 0.03;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Obdlznik01 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Obdlznik01 {
    /// Orezanie do rozumných medzí — panel nesmie odísť úplne mimo okno hry
    /// (stratil by sa) ani sa zmenšiť pod `MIN_ROZMER`.
    pub fn normalizuj(&self) -> Self {
        let w = self.w.clamp(MIN_ROZMER, 1.0);
        let h = self.h.clamp(MIN_ROZMER, 1.0);
        let x = self.x.clamp(0.0, 1.0 - w);
        let y = self.y.clamp(0.0, 1.0 - h);
        Obdlznik01 { x, y, w, h }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rozlozenie {
    /// Kľúč `"<panel>@<monitor_signatura>"` → uložený obdĺžnik. Chýbajúci
    /// kľúč znamená „použi vstavanú predvolenú polohu“ (tú appka pozná z CSS/
    /// konštánt, nie je tu duplikovaná).
    pub polohy: BTreeMap<String, Obdlznik01>,
    /// Priehľadnosť panelov (0.3–1.0), spoločná pre všetky tri — Richard
    /// chcel jeden posuvník, nie tri (zadanie bod 1: „priehľadnosť
    /// nastaviteľná posuvníkom“, jednotné číslo).
    pub priehladnost: f64,
}

impl Default for Rozlozenie {
    fn default() -> Self {
        Self { polohy: BTreeMap::new(), priehladnost: 1.0 }
    }
}

fn kluc(panel: &str, monitor_sig: &str) -> String {
    format!("{panel}@{monitor_sig}")
}

impl Rozlozenie {
    pub fn poloha(&self, panel: &str, monitor_sig: &str) -> Option<Obdlznik01> {
        self.polohy.get(&kluc(panel, monitor_sig)).copied()
    }

    pub fn uloz_polohu(&mut self, panel: &str, monitor_sig: &str, obd: Obdlznik01) {
        self.polohy.insert(kluc(panel, monitor_sig), obd.normalizuj());
    }

    /// Vymaže uloženú polohu JEDNÉHO panela na danom monitore — vráti sa
    /// vstavaná predvolená.
    pub fn obnov_predvolene(&mut self, panel: &str, monitor_sig: &str) {
        self.polohy.remove(&kluc(panel, monitor_sig));
    }

    /// Vymaže uložené polohy VŠETKÝCH troch panelov na danom monitore —
    /// tlačidlo „Obnoviť predvolené“ v Nastaveniach rieši naraz všetko, čo je
    /// práve v režime úpravy vidieť, nie len jeden panel.
    pub fn obnov_predvolene_vsetky(&mut self, monitor_sig: &str) {
        for panel in [PANEL_HLAVNY, PANEL_MINI, PANEL_PRIPOMIENKA] {
            self.obnov_predvolene(panel, monitor_sig);
        }
    }

    pub fn nastav_priehladnost(&mut self, hodnota: f64) {
        self.priehladnost = hodnota.clamp(0.3, 1.0);
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::suradnice::{monitor_signatura, Monitor};

    fn monitor_1080() -> Monitor {
        Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0 }
    }
    fn monitor_1440_125() -> Monitor {
        Monitor { x: 1920, y: 0, sirka: 2560, vyska: 1440, mierka: 1.25 }
    }

    #[test]
    fn predvolene_je_prazdne_s_plnou_priehladnostou() {
        let r = Rozlozenie::default();
        assert!(r.polohy.is_empty());
        assert_eq!(r.priehladnost, 1.0);
    }

    #[test]
    fn neznamy_panel_vrati_none() {
        let r = Rozlozenie::default();
        let sig = monitor_signatura(&monitor_1080());
        assert!(r.poloha(PANEL_HLAVNY, &sig).is_none());
    }

    #[test]
    fn ulozenie_a_citanie_polohy() {
        let mut r = Rozlozenie::default();
        let sig = monitor_signatura(&monitor_1080());
        let obd = Obdlznik01 { x: 0.1, y: 0.2, w: 0.3, h: 0.1 };
        r.uloz_polohu(PANEL_HLAVNY, &sig, obd);
        assert_eq!(r.poloha(PANEL_HLAVNY, &sig), Some(obd));
    }

    #[test]
    fn rozne_panely_na_tom_istom_monitore_sa_neprepisu() {
        let mut r = Rozlozenie::default();
        let sig = monitor_signatura(&monitor_1080());
        r.uloz_polohu(PANEL_HLAVNY, &sig, Obdlznik01 { x: 0.0, y: 0.0, w: 0.2, h: 0.1 });
        r.uloz_polohu(PANEL_MINI, &sig, Obdlznik01 { x: 0.8, y: 0.8, w: 0.1, h: 0.05 });
        assert_eq!(r.poloha(PANEL_HLAVNY, &sig).unwrap().x, 0.0);
        assert_eq!(r.poloha(PANEL_MINI, &sig).unwrap().x, 0.8);
    }

    #[test]
    fn rovnaky_panel_na_dvoch_monitoroch_sa_neprepise() {
        // Presne Richardova požiadavka: „ukladajú sa na okno a monitor“.
        let mut r = Rozlozenie::default();
        let sig_a = monitor_signatura(&monitor_1080());
        let sig_b = monitor_signatura(&monitor_1440_125());
        r.uloz_polohu(PANEL_HLAVNY, &sig_a, Obdlznik01 { x: 0.1, y: 0.1, w: 0.2, h: 0.1 });
        r.uloz_polohu(PANEL_HLAVNY, &sig_b, Obdlznik01 { x: 0.5, y: 0.5, w: 0.2, h: 0.1 });
        assert_eq!(r.poloha(PANEL_HLAVNY, &sig_a).unwrap().x, 0.1);
        assert_eq!(r.poloha(PANEL_HLAVNY, &sig_b).unwrap().x, 0.5);
    }

    #[test]
    fn obnov_predvolene_zmaze_len_jeden_panel() {
        let mut r = Rozlozenie::default();
        let sig = monitor_signatura(&monitor_1080());
        r.uloz_polohu(PANEL_HLAVNY, &sig, Obdlznik01 { x: 0.1, y: 0.1, w: 0.2, h: 0.1 });
        r.uloz_polohu(PANEL_MINI, &sig, Obdlznik01 { x: 0.5, y: 0.5, w: 0.1, h: 0.05 });
        r.obnov_predvolene(PANEL_HLAVNY, &sig);
        assert!(r.poloha(PANEL_HLAVNY, &sig).is_none());
        assert!(r.poloha(PANEL_MINI, &sig).is_some());
    }

    #[test]
    fn obnov_predvolene_vsetky_zmaze_vsetky_tri_na_danom_monitore_ale_nie_inde() {
        let mut r = Rozlozenie::default();
        let sig_a = monitor_signatura(&monitor_1080());
        let sig_b = monitor_signatura(&monitor_1440_125());
        for panel in [PANEL_HLAVNY, PANEL_MINI, PANEL_PRIPOMIENKA] {
            r.uloz_polohu(panel, &sig_a, Obdlznik01 { x: 0.1, y: 0.1, w: 0.2, h: 0.1 });
            r.uloz_polohu(panel, &sig_b, Obdlznik01 { x: 0.3, y: 0.3, w: 0.2, h: 0.1 });
        }
        r.obnov_predvolene_vsetky(&sig_a);
        for panel in [PANEL_HLAVNY, PANEL_MINI, PANEL_PRIPOMIENKA] {
            assert!(r.poloha(panel, &sig_a).is_none(), "{panel} malo zmiznúť na monitore A");
            assert!(r.poloha(panel, &sig_b).is_some(), "{panel} malo ZOSTAŤ na monitore B");
        }
    }

    #[test]
    fn normalizuj_oreze_prilis_maly_rozmer() {
        let obd = Obdlznik01 { x: 0.5, y: 0.5, w: 0.0, h: -1.0 };
        let n = obd.normalizuj();
        assert_eq!(n.w, MIN_ROZMER);
        assert_eq!(n.h, MIN_ROZMER);
    }

    #[test]
    fn normalizuj_nedovoli_odist_za_pravy_okraj() {
        // Panel 50 % široký posunutý na x=0.9 by siahal do 1.4 — musí sa
        // posunúť späť tak, aby celý ostal v okne hry.
        let obd = Obdlznik01 { x: 0.9, y: 0.0, w: 0.5, h: 0.2 };
        let n = obd.normalizuj();
        assert!(n.x + n.w <= 1.0 + 1e-9);
        assert_eq!(n.w, 0.5);
    }

    #[test]
    fn normalizuj_je_identita_na_platnej_hodnote() {
        let obd = Obdlznik01 { x: 0.2, y: 0.3, w: 0.25, h: 0.15 };
        assert_eq!(obd.normalizuj(), obd);
    }

    #[test]
    fn nastav_priehladnost_oreze_rozsah() {
        let mut r = Rozlozenie::default();
        r.nastav_priehladnost(0.05);
        assert_eq!(r.priehladnost, 0.3);
        r.nastav_priehladnost(5.0);
        assert_eq!(r.priehladnost, 1.0);
        r.nastav_priehladnost(0.6);
        assert_eq!(r.priehladnost, 0.6);
    }

    #[test]
    fn round_trip_jsonu() {
        let mut r = Rozlozenie::default();
        let sig = monitor_signatura(&monitor_1080());
        r.uloz_polohu(PANEL_HLAVNY, &sig, Obdlznik01 { x: 0.1, y: 0.2, w: 0.3, h: 0.1 });
        r.nastav_priehladnost(0.75);
        let json = serde_json::to_string(&r).unwrap();
        let r2: Rozlozenie = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    #[test]
    fn stary_json_bez_rozlozenia_sa_nacita_s_predvolenym() {
        // `#[serde(default)]` na úrovni Rozlozenie aj na úrovni jednotlivých
        // polí — staré uložené nastavenia (pred TLI 21) toto pole vôbec
        // nemajú.
        let r: Rozlozenie = serde_json::from_str("{}").unwrap();
        assert_eq!(r, Rozlozenie::default());
    }

    #[test]
    fn prepocet_na_fyzicke_pixely_cez_box01_na_css_sedi_pre_dpi_100_125_150() {
        // Toto je presne matematika, ktorú TLI 21 používa na umiestnenie
        // natívnych okien mini/pripomienka (mierka=1.0 → fyzické pixely okna
        // hry) aj panela vo webview `overlay` (reálna mierka monitora → CSS
        // pixely). Test dokumentuje, že jedna uložená `Obdlznik01` dáva
        // zmysluplné a DPI-nezávislé umiestnenie pri všetkých troch bežných
        // mierkach a pri viacerých monitoroch naraz.
        use crate::suradnice::{box01_na_css, monitor_okna, OknoFyz};

        let okno = OknoFyz { x: 100, y: 50, sirka: 1920, vyska: 1080 };
        let obd = Obdlznik01 { x: 0.5, y: 0.25, w: 0.2, h: 0.1 };

        // Fyzické pixely okna hry (natívne okno mini/pripomienka) — mierka=1.0.
        let fyz = box01_na_css(obd.x, obd.y, obd.w, obd.h, &okno, 1.0);
        assert_eq!(fyz.x, 960.0);
        assert_eq!(fyz.y, 270.0);
        assert_eq!(fyz.w, 384.0);
        assert_eq!(fyz.h, 108.0);

        // CSS pixely vo webview pri DPI 125 % a 150 % — menšie CSS rozmery pri
        // vyššej mierke, presne ako pri existujúcich poliach zo solvera.
        let css_125 = box01_na_css(obd.x, obd.y, obd.w, obd.h, &okno, 1.25);
        let css_150 = box01_na_css(obd.x, obd.y, obd.w, obd.h, &okno, 1.5);
        assert!(css_125.w < fyz.w && css_150.w < css_125.w);
        assert!(css_125.h < fyz.h && css_150.h < css_125.h);

        // Viac monitorov: tá istá uložená `Obdlznik01` sa na monitore so 150 %
        // prepočíta inak než na monitore so 100 % — presne preto sa ukladá
        // kľúčom (panel, monitor), nie globálne.
        let monitory = vec![
            Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0 },
            Monitor { x: 1920, y: 0, sirka: 2560, vyska: 1440, mierka: 1.5 },
        ];
        let m_a = monitor_okna(&OknoFyz { x: 0, y: 0, sirka: 1920, vyska: 1080 }, &monitory).unwrap();
        let m_b = monitor_okna(&OknoFyz { x: 2000, y: 0, sirka: 1920, vyska: 1080 }, &monitory).unwrap();
        assert_ne!(m_a.mierka, m_b.mierka);
        let css_a = box01_na_css(obd.x, obd.y, obd.w, obd.h, &okno, m_a.mierka);
        let css_b = box01_na_css(obd.x, obd.y, obd.w, obd.h, &okno, m_b.mierka);
        assert_ne!(css_a.w, css_b.w);
    }
}
