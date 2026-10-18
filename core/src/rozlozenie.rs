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

/// Hlavný panel mal do TLI 25 JEDEN kľúč pre všetky moduly, takže sa F8 doska,
/// F9 Canvas aj Artistry museli deliť o to isté miesto nad hrou. Richard chcel
/// (úloha `2026-10-17-1242-mcp.md` bod 5) posúvať každý zvlášť — preto má
/// odteraz každý modul vlastný kľúč a `panel` ostáva ako SPOLOČNÁ ZÁLOHA:
/// kto si rozloženie uložil pred TLI 25, nájde ho po aktualizácii tam, kde ho
/// nechal, kým si niektorý modul neposunie samostatne.
pub const PANEL_TARTARUS: &str = "panel:tartarus";
pub const PANEL_NASTROJ: &str = "panel:nastroj";
pub const PANEL_CANVAS: &str = "panel:canvas";
pub const PANEL_ARTISTRY: &str = "panel:artistry";
/// TLI 35 — výber „Warden's Treasure" má vlastnú uloženú polohu rovnako ako
/// výber nástroja (TLI 25 bod 5): iná obrazovka, iný typický počet kartičiek
/// (4, nie 3), hráč ju chce vedieť posunúť nezávisle od ostatných.
pub const PANEL_POKLAD: &str = "panel:poklad";
/// TLI 36 — univerzálny poradca výberu. Vlastný kľúč z toho istého dôvodu
/// ako `PANEL_POKLAD`: je to iná obrazovka (modál, lišta tlačidiel, zoznam
/// cieľov) a hráč si ju chce posunúť nezávisle od dosky a kartičiek.
pub const PANEL_PORADCA: &str = "panel:poradca";

/// Moduly v poradí, v akom ich ponúka prepínač v režime úpravy
/// (`kľúč`, `čo to je pre hráča`).
pub const MODULOVE_PANELY: [(&str, &str); 6] = [
    (PANEL_TARTARUS, "F8 — doska"),
    (PANEL_NASTROJ, "F8 — výber nástroja"),
    (PANEL_POKLAD, "F8 — výber pokladu"),
    (PANEL_PORADCA, "F8 — poradca (neznáma obrazovka)"),
    (PANEL_CANVAS, "F9 — Frozen Canvas"),
    (PANEL_ARTISTRY, "Artistry"),
];

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
    /// TLI 26, bod 2: `monitor_sig` odteraz obsahuje aj meno monitora od OS.
    /// Keď appka pod NOVÝM kľúčom nič nenájde, skúsi STARÝ tvar (čisté
    /// `rozlíšenie@mierka`, bez mena) — presne to, pod čím appka ukladala
    /// pozície pred touto zmenou. Vďaka tomu hráč, ktorý si panel posunul už
    /// pred 0.18.3, po aktualizácii nenájde predvolenú polohu namiesto svojej
    /// vlastnej. Prvý ďalší posun v edit móde sa uloží už pod nový kľúč.
    pub fn poloha(&self, panel: &str, monitor_sig: &str) -> Option<Obdlznik01> {
        if let Some(v) = self.polohy.get(&kluc(panel, monitor_sig)) {
            return Some(*v);
        }
        let stary_sig = crate::suradnice::monitor_signatura_bez_mena(monitor_sig)?;
        self.polohy.get(&kluc(panel, &stary_sig)).copied()
    }

    /// Poloha panela modulu s pádom na spoločný `panel` (TLI 25). Používa sa
    /// všade, kde sa panel SKUTOČNE kreslí — `poloha()` ostáva presná (bez
    /// pádu), lebo režim úpravy musí vedieť rozoznať „nič uložené" od
    /// „zdedené zo spoločného".
    pub fn poloha_modulu(&self, panel: &str, monitor_sig: &str) -> Option<Obdlznik01> {
        self.poloha(panel, monitor_sig)
            .or_else(|| self.poloha(PANEL_HLAVNY, monitor_sig))
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
        for (panel, _) in MODULOVE_PANELY {
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
        Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0, nazov: None }
    }
    fn monitor_1440_125() -> Monitor {
        Monitor { x: 1920, y: 0, sirka: 2560, vyska: 1440, mierka: 1.25, nazov: None }
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

    /// TLI 26, bod 2 — migrácia: hráč si posunul panel PRED touto zmenou,
    /// keď kľúč monitora ešte neobsahoval meno od OS. Appka musí jeho starú
    /// pozíciu nájsť aj po aktualizácii na nový (s menom) kľúč, nie mu ukázať
    /// predvolenú polohu, akoby nič neuložil — „nič nemaž, starý formát
    /// zmigruj" zo zadania.
    #[test]
    fn stara_poloha_bez_mena_monitora_sa_najde_aj_pod_novym_klucom() {
        let mut r = Rozlozenie::default();
        let stary_sig = "2560x1440@100"; // presne to, čo appka ukladala do 0.18.2
        let obd = Obdlznik01 { x: 0.6, y: 0.05, w: 0.15, h: 0.1 };
        r.uloz_polohu(PANEL_MINI, stary_sig, obd);

        let novy_sig = format!(r"\\.\DISPLAY2#{stary_sig}");
        assert_eq!(r.poloha(PANEL_MINI, &novy_sig), Some(obd));
    }

    /// Len čo appka pod NOVÝM kľúčom niečo skutočne uloží, ten má prednosť
    /// pred starým — inak by sa DISPLAY1 a DISPLAY2 aj po migrácii stále
    /// delili o jednu (starú) hodnotu.
    #[test]
    fn nova_poloha_pod_menom_monitora_prebije_stary_zdieliany_zaznam() {
        let mut r = Rozlozenie::default();
        let stary_sig = "2560x1440@100";
        r.uloz_polohu(PANEL_MINI, stary_sig, Obdlznik01 { x: 0.6, y: 0.05, w: 0.15, h: 0.1 });

        let sig_display2 = format!(r"\\.\DISPLAY2#{stary_sig}");
        let vlastna = Obdlznik01 { x: 0.1, y: 0.1, w: 0.2, h: 0.1 };
        r.uloz_polohu(PANEL_MINI, &sig_display2, vlastna);

        let sig_display1 = format!(r"\\.\DISPLAY1#{stary_sig}");
        assert_eq!(r.poloha(PANEL_MINI, &sig_display2), Some(vlastna));
        // DISPLAY1 nikdy neuložil vlastnú — stále dostane starú zdieľanú.
        assert_eq!(
            r.poloha(PANEL_MINI, &sig_display1),
            Some(Obdlznik01 { x: 0.6, y: 0.05, w: 0.15, h: 0.1 })
        );
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
            Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0, nazov: None },
            Monitor { x: 1920, y: 0, sirka: 2560, vyska: 1440, mierka: 1.5, nazov: None },
        ];
        let m_a = monitor_okna(&OknoFyz { x: 0, y: 0, sirka: 1920, vyska: 1080 }, &monitory).unwrap();
        let m_b = monitor_okna(&OknoFyz { x: 2000, y: 0, sirka: 1920, vyska: 1080 }, &monitory).unwrap();
        assert_ne!(m_a.mierka, m_b.mierka);
        let css_a = box01_na_css(obd.x, obd.y, obd.w, obd.h, &okno, m_a.mierka);
        let css_b = box01_na_css(obd.x, obd.y, obd.w, obd.h, &okno, m_b.mierka);
        assert_ne!(css_a.w, css_b.w);
    }
}

#[cfg(test)]
mod testy_moduly {
    use super::*;
    use crate::suradnice::{monitor_signatura, Monitor};

    fn sig() -> String {
        monitor_signatura(&Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0, nazov: None })
    }

    /// Kto si rozloženie uložil pred TLI 25 (kľúč `panel`), musí ho po
    /// aktualizácii nájsť tam, kde ho nechal — vo VŠETKÝCH moduloch.
    #[test]
    fn modul_bez_vlastnej_polohy_zdedi_spolocnu() {
        let mut r = Rozlozenie::default();
        let obd = Obdlznik01 { x: 0.5, y: 0.1, w: 0.2, h: 0.2 };
        r.uloz_polohu(PANEL_HLAVNY, &sig(), obd);
        for (panel, _) in MODULOVE_PANELY {
            assert_eq!(r.poloha_modulu(panel, &sig()), Some(obd));
            // `poloha()` naopak musí priznať, že vlastnú uloženú nemá.
            assert_eq!(r.poloha(panel, &sig()), None);
        }
    }

    #[test]
    fn vlastna_poloha_modulu_prebije_spolocnu_a_ostatne_nechaju_tak() {
        let mut r = Rozlozenie::default();
        let spolocna = Obdlznik01 { x: 0.5, y: 0.1, w: 0.2, h: 0.2 };
        let vlastna = Obdlznik01 { x: 0.05, y: 0.6, w: 0.3, h: 0.25 };
        r.uloz_polohu(PANEL_HLAVNY, &sig(), spolocna);
        r.uloz_polohu(PANEL_CANVAS, &sig(), vlastna);
        assert_eq!(r.poloha_modulu(PANEL_CANVAS, &sig()), Some(vlastna));
        assert_eq!(r.poloha_modulu(PANEL_TARTARUS, &sig()), Some(spolocna));
    }

    #[test]
    fn obnova_predvolenych_zmaze_aj_modulove_kluce() {
        let mut r = Rozlozenie::default();
        let obd = Obdlznik01 { x: 0.5, y: 0.1, w: 0.2, h: 0.2 };
        r.uloz_polohu(PANEL_HLAVNY, &sig(), obd);
        for (panel, _) in MODULOVE_PANELY {
            r.uloz_polohu(panel, &sig(), obd);
        }
        r.obnov_predvolene_vsetky(&sig());
        assert!(r.polohy.is_empty());
    }
}
