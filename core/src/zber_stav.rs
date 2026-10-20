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

/// Ako ďaleko smie byť pomer farieb od toho, aký bol pri POTVRDENÍ, aby to
/// ešte platilo za „tá istá obrazovka".
///
/// Namerané na reálnych snímkach z `/srv/games/tli/analyzy` (CT 114,
/// 20.10.2026) — trojica `(ladovy, mriezka_modra, mriezka_tmava)`:
///
/// | obrazovka | podpis |
/// |---|---|
/// | Frozen Canvas (plátno) | ~(0,50; 0,93; 0,18) |
/// | doska Tartaru | ~(0,002; 0,011; 0,90) |
/// | bežné hranie / mapa | ~(0,02; 0,04; 0,04) |
///
/// Rozptyl TEJ ISTEJ obrazovky medzi ťahmi je nanajvýš ~0,18 (plátno,
/// `mriezka_modra` 0,79–0,97), kým najmenší rozdiel MEDZI obrazovkami je
/// 0,48. Prah 0,25 teda leží v medzere a ani jednu stranu netlačí.
pub const TOLERANCIA_PODPISU: f32 = 0.25;

/// Hrubý „ako obrazovka vyzerá" — tie isté tri čísla, ktoré počíta
/// `canvas_detekcia::crty`. Je tu zámerne ako vlastný typ a nie ako závislosť
/// na `canvas_detekcia`: tento modul rozhoduje, `canvas_detekcia` meria.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Podpis {
    pub ladovy: f32,
    pub mriezka_modra: f32,
    pub mriezka_tmava: f32,
}

impl Podpis {
    /// Je to (v rámci tolerancie) tá istá obrazovka?
    pub fn je_to_ta_ista(&self, iny: &Podpis) -> bool {
        (self.ladovy - iny.ladovy).abs() <= TOLERANCIA_PODPISU
            && (self.mriezka_modra - iny.mriezka_modra).abs() <= TOLERANCIA_PODPISU
            && (self.mriezka_tmava - iny.mriezka_tmava).abs() <= TOLERANCIA_PODPISU
    }
}

/// Ktorú obrazovku server naposledy POTVRDIL a ako dávno (v sekundách).
///
/// `co` je presne to, čo server vrátil: `"canvas"` pre `modul=canvas`,
/// inak hodnota poľa `obrazovka` (`doska`, `vyber_nastroja`, `vyber_pokladu`,
/// `objective`, `poradca`, `ine`). Nič sa tu nemapuje „podobne znejúce"
/// dokopy — obrazovka, ktorú tento modul nepozná, jednoducho nič nezbiera.
#[derive(Debug, Clone, PartialEq)]
pub struct Potvrdenie {
    pub co: String,
    pub pred_s: u64,
    /// Ako obrazovka v tej chvíli VYZERALA. Bez nej sa „platí do zmeny
    /// obrazovky" (zadanie) nedá splniť — čas sám o sebe nevie, že hráč
    /// medzitým Canvas zavrel a odišiel bojovať do mapy.
    pub podpis: Option<Podpis>,
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
pub fn co_zbierat(
    je_canvas_lokalne: bool,
    potvrdene: Option<&Potvrdenie>,
    teraz: Option<Podpis>,
) -> Option<CoZbierat> {
    if je_canvas_lokalne {
        return Some(CoZbierat::Canvas);
    }
    let p = potvrdene?;
    if !p.je_platne() {
        return None;
    }
    // „Platí do ZMENY OBRAZOVKY alebo 5 min bez kliku" (zadanie). Čas rieši
    // `je_platne`, zmenu obrazovky toto: keď hráč Canvas zavrie a odíde
    // bojovať, pomer farieb sa zmení rádovo a potvrdenie prestane platiť.
    // Bez uloženého podpisu sa na potvrdenie NESPOLIEHAME vôbec — radšej
    // nezbierať než zbierať snímky boja a míňať na ne vision volania.
    match (p.podpis, teraz) {
        (Some(vtedy), Some(dnes)) if vtedy.je_to_ta_ista(&dnes) => {}
        _ => return None,
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
    /// Naozaj POSLEDNÁ chyba zo všetkých troch front, nech je ktorákoľvek.
    /// Je to vlastné pole a nie „prvá neprázdna v poradí Canvas → Tartarus →
    /// Minimapa", lebo tým by stará chyba Canvasu zakryla čerstvú chybu
    /// minimapy a hráč by videl nepravdu.
    pub posledna_chyba: Option<String>,
    /// Hotová veta pre UI (`veta()`), nech ju JS neskladá druhýkrát a nech
    /// to, čo hráč naozaj vidí, chráni test tu v jadre.
    pub veta: String,
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

    pub fn zapis_chybu(&mut self, co: CoZbierat, chyba: &str) {
        self.pre(co).posledna_chyba = Some(chyba.to_string());
        self.posledna_chyba = Some(chyba.to_string());
    }

    pub fn zapis_chybu_minimapy(&mut self, chyba: &str) {
        self.minimapa.posledna_chyba = Some(chyba.to_string());
        self.posledna_chyba = Some(chyba.to_string());
    }

    /// Úspešné odoslanie — zmaže chybu TEJTO fronty, ale nie celkovú
    /// poslednú chybu (tá patrí tej fronte, ktorá naozaj zlyhala naposledy).
    pub fn zapis_uspech(&mut self, co: CoZbierat) {
        let j = self.pre(co);
        j.odoslane += 1;
        j.posledna_chyba = None;
    }

    pub fn zapis_uspech_minimapy(&mut self) {
        self.minimapa.odoslane += 1;
        self.minimapa.posledna_chyba = None;
    }

    /// Jednoriadkový prehľad presne v tvare, aký Richard pýta v bode 5:
    /// „Učenie: Canvas X odoslaných / Y čaká, Tartarus …, Minimapa …,
    /// posledná chyba …".
    pub fn veta(&self) -> String {
        let kus = |m: &str, s: &StavJednehoZberu| {
            format!("{m} {} odoslaných / {} čaká", s.odoslane, s.caka)
        };
        format!(
            "Učenie: {}, {}, {}, posledná chyba: {}",
            kus("Canvas", &self.canvas),
            kus("Tartarus", &self.tartarus),
            kus("Minimapa", &self.minimapa),
            self.posledna_chyba.as_deref().unwrap_or("žiadna"),
        )
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    /// Podpisy namerané na reálnych snímkach (pozri `TOLERANCIA_PODPISU`).
    const PLATNO: Podpis = Podpis { ladovy: 0.50, mriezka_modra: 0.93, mriezka_tmava: 0.18 };
    const DOSKA: Podpis = Podpis { ladovy: 0.002, mriezka_modra: 0.011, mriezka_tmava: 0.90 };
    const MAPA: Podpis = Podpis { ladovy: 0.019, mriezka_modra: 0.043, mriezka_tmava: 0.044 };

    fn p(co: &str, pred_s: u64, podpis: Option<Podpis>) -> Potvrdenie {
        Potvrdenie { co: co.to_string(), pred_s, podpis }
    }

    #[test]
    fn lokalna_detekcia_ma_prednost_a_staci_sama() {
        assert_eq!(co_zbierat(true, None, None), Some(CoZbierat::Canvas));
        // Aj keď server naposledy potvrdil niečo iné — pixely sú dôkaz
        // o PRÁVE odfotenej snímke, potvrdenie je staršie.
        assert_eq!(
            co_zbierat(true, Some(&p("doska", 1, Some(DOSKA))), Some(PLATNO)),
            Some(CoZbierat::Canvas)
        );
    }

    #[test]
    fn potvrdeny_canvas_zoberie_zber_aj_ked_lokalna_detekcia_mlci() {
        // Presne tá porucha z 20.10.2026: lokálna detekcia z nejakého dôvodu
        // nepadne, ale server pred chvíľou canvas PREČÍTAL a obrazovka
        // vyzerá rovnako ako vtedy.
        assert_eq!(
            co_zbierat(false, Some(&p("canvas", 10, Some(PLATNO))), Some(PLATNO)),
            Some(CoZbierat::Canvas)
        );
    }

    #[test]
    fn potvrdena_doska_zbiera_tartarus() {
        assert_eq!(
            co_zbierat(false, Some(&p("doska", 10, Some(DOSKA))), Some(DOSKA)),
            Some(CoZbierat::Tartarus)
        );
    }

    #[test]
    fn zmena_obrazovky_zrusi_potvrdenie_skor_nez_vyprsi_cas() {
        // Hráč stlačil Ctrl+W nad Canvasom, zavrel ho a odišiel bojovať.
        // Bez tejto kontroly by appka ďalších 5 minút fotila boj ako Canvas
        // a na každú snímku minula jedno vision volanie na serveri.
        assert_eq!(co_zbierat(false, Some(&p("canvas", 10, Some(PLATNO))), Some(MAPA)), None);
        // To isté pre dosku Tartaru.
        assert_eq!(co_zbierat(false, Some(&p("doska", 10, Some(DOSKA))), Some(MAPA)), None);
    }

    #[test]
    fn ta_ista_obrazovka_medzi_tahmi_sa_este_zmesti_do_tolerancie() {
        // Plátno sa medzi ťahmi mení (pribúdajú zafarbené bunky) — namerané
        // rozpätie `mriezka_modra` 0,79–0,97, `ladovy` 0,46–0,56.
        let neskor = Podpis { ladovy: 0.46, mriezka_modra: 0.79, mriezka_tmava: 0.21 };
        assert!(PLATNO.je_to_ta_ista(&neskor));
        assert!(!PLATNO.je_to_ta_ista(&MAPA));
        assert!(!DOSKA.je_to_ta_ista(&MAPA));
    }

    #[test]
    fn bez_ulozeneho_podpisu_sa_na_potvrdenie_nespoliehame() {
        // Staršia/neúplná cesta (analýza bez snímky) — radšej nezbierať.
        assert_eq!(co_zbierat(false, Some(&p("canvas", 1, None)), Some(PLATNO)), None);
        assert_eq!(co_zbierat(false, Some(&p("canvas", 1, Some(PLATNO))), None), None);
    }

    #[test]
    fn potvrdenie_vyprsi_po_piatich_minutach() {
        assert_eq!(
            co_zbierat(false, Some(&p("canvas", 299, Some(PLATNO))), Some(PLATNO)),
            Some(CoZbierat::Canvas)
        );
        assert_eq!(co_zbierat(false, Some(&p("canvas", 300, Some(PLATNO))), Some(PLATNO)), None);
        assert_eq!(co_zbierat(false, Some(&p("canvas", 100_000, Some(PLATNO))), Some(PLATNO)), None);
    }

    #[test]
    fn obrazovky_bez_tahov_nic_nezbieraju() {
        for o in ["vyber_nastroja", "vyber_pokladu", "objective", "poradca", "ine", "trh", ""] {
            assert_eq!(co_zbierat(false, Some(&p(o, 1, Some(PLATNO))), Some(PLATNO)), None, "{o}");
        }
    }

    #[test]
    fn bez_potvrdenia_a_bez_pixelov_sa_nezbiera_nic() {
        assert_eq!(co_zbierat(false, None, Some(MAPA)), None);
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
        s.posledna_chyba = Some("Server má poruchu".to_string());
        assert!(s.veta().ends_with("posledná chyba: Server má poruchu"));
    }

    #[test]
    fn posledna_chyba_je_naozaj_POSLEDNA_bez_ohladu_na_frontu() {
        // Predtým sa brala prvá neprázdna v poradí Canvas → Tartarus →
        // Minimapa, takže stará chyba Canvasu zakryla čerstvú chybu minimapy.
        let mut s = StavZberu::default();
        s.zapis_chybu(CoZbierat::Canvas, "stará");
        s.zapis_chybu_minimapy("čerstvá");
        assert_eq!(s.posledna_chyba.as_deref(), Some("čerstvá"));
        assert_eq!(s.canvas.posledna_chyba.as_deref(), Some("stará"));
        s.zapis_uspech(CoZbierat::Canvas);
        assert_eq!(s.canvas.posledna_chyba, None);
        // Úspech jednej fronty NEMAŽE celkovú poslednú chybu inej.
        assert_eq!(s.posledna_chyba.as_deref(), Some("čerstvá"));
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
