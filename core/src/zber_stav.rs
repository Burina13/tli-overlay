//! Čo sa PRÁVE dá zbierať a ako na tom zber stojí (TLI 70,
//! `2026-10-20-1334-mcp.md`, body 2, 3 a 5).
//!
//! Vzniklo z poruchy: od TLI 60 appka po každom kliku rozhodovala, či je
//! otvorený Frozen Canvas, VÝHRADNE z pixelov (`canvas_detekcia`). Richard
//! 20.10.2026 odohral celý deň Frozen Canvasu s Ctrl+W, server dostal 91
//! canvas analýz — a priečinok `canvas-zber` ostal PRÁZDNY (0 súborov).
//! Z appky sa pritom nedalo zistiť prečo: každá zo šiestich zamietacích
//! podmienok `pokus_o_zber` vracala `false` **ticho**, takže v denníku nebol
//! ani jeden riadok, z ktorého by sa dalo povedať, ktorá z nich to bola.
//!
//! Tento modul preto rieši dve veci naraz:
//!
//! 1. **Druhý, nezávislý zdroj pravdy o tom, čo je na obrazovke.** Richard
//!    v zadaní: „Canvas je otvorený, keď posledná Ctrl+W analýza (alebo
//!    server) vrátila obrazovku canvas, a to platí do zmeny obrazovky alebo
//!    5 min bez kliku." Je to silnejší dôkaz než pomer farieb — server
//!    obrazovku naozaj PREČÍTAL vision modelom. Lokálna detekcia ostáva ako
//!    prvá, lacná cesta; serverové potvrdenie je záchranná sieť pod ňou.
//! 2. **Dôvod zamietnutia sa počíta a dá sa ukázať** (bod 5 zadania) — nech
//!    sa pri ďalšej takej poruche nemusí hádať.
//!
//! Tu je zámerne len logika bez obrazovky, disku a siete (rovnaké rozdelenie
//! ako `zber_fronta`): všetko nižšie sa dá overiť `cargo test` na Linuxe.

use std::collections::BTreeMap;

/// Ako dlho po poslednej POTVRDENEJ analýze sa ešte verí, že je tá istá
/// obrazovka otvorená. Priamo zo zadania („platí do zmeny obrazovky alebo
/// 5 min bez kliku").
pub const PLATNOST_POTVRDENIA_S: u64 = 300;

/// Čo sa na tejto obrazovke dá zbierať na učenie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoZbierat {
    /// Frozen Canvas — `/api/analyza?modul=canvas&zber=1` (TLI 60).
    Canvas,
    /// Doska Tartaru — `/api/analyza?zber=1` (TLI 70, bod 3; dovtedy sa
    /// doska na pozadí nezbierala vôbec).
    Tartarus,
}

impl CoZbierat {
    /// Meno do denníka, do mena súboru vo fronte a do prehľadu pre hráča.
    pub fn kluc(&self) -> &'static str {
        match self {
            CoZbierat::Canvas => "canvas",
            CoZbierat::Tartarus => "tartarus",
        }
    }
}

/// Ktorú obrazovku server naposledy POTVRDIL a ako dávno (v sekundách).
///
/// `co` je presne to, čo server vrátil: `"canvas"` pre `modul=canvas`,
/// inak hodnota poľa `obrazovka` (`doska`, `vyber_nastroja`, `vyber_pokladu`,
/// `objective`, `poradca`, `ine`). Nič sa tu nemapuje „podobne znejúce"
/// dokopy — obrazovka, ktorú tento modul nepozná, jednoducho nič nezbiera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Potvrdenie {
    pub co: String,
    pub pred_s: u64,
}

impl Potvrdenie {
    pub fn je_platne(&self) -> bool {
        self.pred_s < PLATNOST_POTVRDENIA_S
    }
}

/// Rozhodnutie „zbierať, a ak áno, čo" pre JEDEN klik.
///
/// `je_canvas_lokalne` je výsledok `canvas_detekcia` nad práve odfotenou
/// snímkou (lacná cesta, nepotrebuje server). `potvrdene` je posledná
/// odpoveď servera. Lokálna detekcia má prednosť — je to dôkaz o TEJTO
/// snímke, kým potvrdenie je dôkaz o obrazovke spred niekoľkých sekúnd.
pub fn co_zbierat(je_canvas_lokalne: bool, potvrdene: Option<&Potvrdenie>) -> Option<CoZbierat> {
    if je_canvas_lokalne {
        return Some(CoZbierat::Canvas);
    }
    let p = potvrdene?;
    if !p.je_platne() {
        return None;
    }
    match p.co.as_str() {
        "canvas" => Some(CoZbierat::Canvas),
        "doska" => Some(CoZbierat::Tartarus),
        // `vyber_nastroja`, `vyber_pokladu`, `objective`, `poradca` sú
        // jednorazové obrazovky bez ťahov — po kliku na nich sa niet čo
        // učiť. `ine` je priznanie servera, že obrazovku nepozná.
        _ => None,
    }
}

/// Počítadlá jedného druhu zberu — čo odišlo, čo čaká, čo naposledy zlyhalo.
/// Len v pamäti; `caka` sa dopĺňa z disku pri čítaní prehľadu.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct StavJednehoZberu {
    pub odoslane: u32,
    pub caka: u32,
    pub posledna_chyba: Option<String>,
}

/// Celý stav zberu, tak ako ho uvidí hráč v Nastaveniach (bod 5 zadania).
///
/// `dovody` je to, čo pri tejto poruche najviac chýbalo: koľkokrát sa klik
/// NEODFOTIL a prečo. Kľúč je krátky strojový dôvod (`mimo-obrazoviek`,
/// `hra-nie-je-v-popredi`, `panel-svieti`, …), hodnota je počet.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StavZberu {
    pub canvas: StavJednehoZberu,
    pub tartarus: StavJednehoZberu,
    pub minimapa: StavJednehoZberu,
    pub dovody: BTreeMap<String, u32>,
    /// Posledná obrazovka, ktorú server potvrdil, a pred koľkými sekundami.
    pub potvrdena_obrazovka: Option<String>,
    pub potvrdena_pred_s: Option<u64>,
}

impl StavZberu {
    pub fn zapocitaj_dovod(&mut self, dovod: &str) {
        *self.dovody.entry(dovod.to_string()).or_insert(0) += 1;
    }

    pub fn pre(&mut self, co: CoZbierat) -> &mut StavJednehoZberu {
        match co {
            CoZbierat::Canvas => &mut self.canvas,
            CoZbierat::Tartarus => &mut self.tartarus,
        }
    }

    /// Jednoriadkový prehľad presne v tvare, aký Richard pýta v bode 5:
    /// „Učenie: Canvas X odoslaných / Y čaká, Tartarus …, Minimapa …,
    /// posledná chyba …". Skladá sa tu (nie v JS), aby sa dal otestovať.
    pub fn veta(&self) -> String {
        let kus = |m: &str, s: &StavJednehoZberu| {
            format!("{m} {} odoslaných / {} čaká", s.odoslane, s.caka)
        };
        let mut v = format!(
            "Učenie: {}, {}, {}",
            kus("Canvas", &self.canvas),
            kus("Tartarus", &self.tartarus),
            kus("Minimapa", &self.minimapa),
        );
        let chyba = self
            .canvas
            .posledna_chyba
            .as_ref()
            .or(self.tartarus.posledna_chyba.as_ref())
            .or(self.minimapa.posledna_chyba.as_ref());
        match chyba {
            Some(c) => v.push_str(&format!(", posledná chyba: {c}")),
            None => v.push_str(", posledná chyba: žiadna"),
        }
        v
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    fn p(co: &str, pred_s: u64) -> Potvrdenie {
        Potvrdenie { co: co.to_string(), pred_s }
    }

    #[test]
    fn lokalna_detekcia_ma_prednost_a_staci_sama() {
        assert_eq!(co_zbierat(true, None), Some(CoZbierat::Canvas));
        // Aj keď server naposledy potvrdil niečo iné — pixely sú dôkaz
        // o PRÁVE odfotenej snímke, potvrdenie je staršie.
        assert_eq!(co_zbierat(true, Some(&p("doska", 1))), Some(CoZbierat::Canvas));
    }

    #[test]
    fn potvrdeny_canvas_zoberie_zber_aj_ked_pixely_nesedia() {
        // Toto je presne tá porucha z 20.10.2026: lokálna detekcia z nejakého
        // dôvodu nepadne, ale server pred chvíľou canvas PREČÍTAL.
        assert_eq!(co_zbierat(false, Some(&p("canvas", 10))), Some(CoZbierat::Canvas));
    }

    #[test]
    fn potvrdena_doska_zbiera_tartarus() {
        assert_eq!(co_zbierat(false, Some(&p("doska", 10))), Some(CoZbierat::Tartarus));
    }

    #[test]
    fn potvrdenie_vyprsi_po_piatich_minutach() {
        assert_eq!(co_zbierat(false, Some(&p("canvas", 299))), Some(CoZbierat::Canvas));
        assert_eq!(co_zbierat(false, Some(&p("canvas", 300))), None);
        assert_eq!(co_zbierat(false, Some(&p("canvas", 100_000))), None);
    }

    #[test]
    fn obrazovky_bez_tahov_nic_nezbieraju() {
        for o in ["vyber_nastroja", "vyber_pokladu", "objective", "poradca", "ine", ""] {
            assert_eq!(co_zbierat(false, Some(&p(o, 1))), None, "{o}");
        }
    }

    #[test]
    fn bez_potvrdenia_a_bez_pixelov_sa_nezbiera_nic() {
        assert_eq!(co_zbierat(false, None), None);
    }

    #[test]
    fn veta_ma_tvar_zo_zadania() {
        let mut s = StavZberu::default();
        s.canvas.odoslane = 12;
        s.canvas.caka = 3;
        s.tartarus.odoslane = 1;
        s.minimapa.caka = 1488;
        assert_eq!(
            s.veta(),
            "Učenie: Canvas 12 odoslaných / 3 čaká, Tartarus 1 odoslaných / 0 čaká, \
             Minimapa 0 odoslaných / 1488 čaká, posledná chyba: žiadna"
        );
        s.minimapa.posledna_chyba = Some("Server má poruchu".to_string());
        assert!(s.veta().ends_with("posledná chyba: Server má poruchu"));
    }

    #[test]
    fn dovody_sa_scitavaju() {
        let mut s = StavZberu::default();
        s.zapocitaj_dovod("hra-nie-je-v-popredi");
        s.zapocitaj_dovod("hra-nie-je-v-popredi");
        s.zapocitaj_dovod("mimo-obrazoviek");
        assert_eq!(s.dovody.get("hra-nie-je-v-popredi"), Some(&2));
        assert_eq!(s.dovody.get("mimo-obrazoviek"), Some(&1));
    }

    #[test]
    fn kluc_sa_pouziva_v_mene_suboru_aj_v_denniku() {
        assert_eq!(CoZbierat::Canvas.kluc(), "canvas");
        assert_eq!(CoZbierat::Tartarus.kluc(), "tartarus");
    }
}
