//! Dve opakované pripomienky NAD HROU (TLI 48 bod 1): „Log sa neposiela na
//! server" a „Sortni inventár" — doplnok k `pripomienka.rs` (Enable Log),
//! ktoré už toast nad hrou malo od TLI 11d.
//!
//! Na rozdiel od `SledovacPripomienky` (presný stavový automat na JEDEN
//! konkrétny prípad s vlastnou históriou udalostí session) ide tu o
//! jednoduchšiu, OPAKOVANÚ pripomienku nad stavom, ktorý appka už aj tak
//! počíta pre pätičku dashboardu (`posledna_chyba_uploadu`,
//! `posledny_upload_pred_s`, prázdny `batoh`) — kým podmienka trvá, volajúci
//! (`src-tauri`) toast v pravidelných odstupoch obnovuje; keď prestane
//! obnovovať, toast sám zhasne vlastným časovačom (`ukaz_pripomienku_toast`).
//! Žiadny explicitný „zrušiť" príkaz netreba: najneskôr do
//! `OPAKOVAT_PO_S + poslana_dlzka_s` od vyriešenia stavu zmizne sám.

/// Ako často appka toast obnoví, kým podmienka trvá. Appka tikne á ~1 s,
/// takže 25 s nie je vidieť ako blikanie, ale stav sa ukáže znova dávno pred
/// tým, než by prvý toast sám zhasol.
pub const OPAKOVAT_PO_S: u64 = 25;

/// Jeden nezávislý sledovač jednej podmienky (upload/sort) — appka drží dve
/// samostatné inštancie, nech zmena jednej nereštartuje časovanie druhej.
#[derive(Debug, Default, Clone, Copy)]
pub struct SledovacVarovania {
    naposledy_ukazane: Option<u64>,
}

impl SledovacVarovania {
    pub fn novy() -> Self {
        Self::default()
    }

    /// `aktivne` = podmienka platí PRÁVE TERAZ (vyhodnotil volajúci).
    /// Vráti `true`, keď má volajúci TERAZ (znova) zavolať
    /// `ukaz_pripomienku_toast` — buď prvý raz, alebo preto, že uplynul
    /// interval obnovy. `false` podmienka vynuluje vnútorný stav, aby sa pri
    /// ďalšom výskyte ukázala hneď, nie až po zvyšku starého intervalu.
    pub fn vyhodnot(&mut self, teraz: u64, aktivne: bool) -> bool {
        if !aktivne {
            self.naposledy_ukazane = None;
            return false;
        }
        let ma_sa_ukazat = self
            .naposledy_ukazane
            .map_or(true, |t| teraz.saturating_sub(t) >= OPAKOVAT_PO_S);
        if ma_sa_ukazat {
            self.naposledy_ukazane = Some(teraz);
        }
        ma_sa_ukazat
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn prvy_vyskyt_sa_ukaze_hned() {
        let mut s = SledovacVarovania::novy();
        assert!(s.vyhodnot(1000, true));
    }

    #[test]
    fn opakovany_tik_pred_uplynutim_intervalu_nic_neukaze() {
        let mut s = SledovacVarovania::novy();
        assert!(s.vyhodnot(1000, true));
        assert!(!s.vyhodnot(1005, true));
        assert!(!s.vyhodnot(1000 + OPAKOVAT_PO_S - 1, true));
    }

    #[test]
    fn po_uplynuti_intervalu_sa_obnovi() {
        let mut s = SledovacVarovania::novy();
        assert!(s.vyhodnot(1000, true));
        assert!(s.vyhodnot(1000 + OPAKOVAT_PO_S, true));
    }

    #[test]
    fn neaktivna_podmienka_nic_neukaze() {
        let mut s = SledovacVarovania::novy();
        assert!(!s.vyhodnot(1000, false));
    }

    #[test]
    fn vyriesenie_a_novy_vyskyt_sa_ukaze_hned_nie_az_po_zvysku_povodneho_intervalu() {
        let mut s = SledovacVarovania::novy();
        assert!(s.vyhodnot(1000, true));
        assert!(!s.vyhodnot(1010, false), "vyriešené — nič sa neukazuje");
        // Nový výskyt len 2 s po vyriešení (teda dávno pred OPAKOVAT_PO_S od
        // 1000) sa má ukázať hneď, nie čakať na zvyšok pôvodného intervalu.
        assert!(s.vyhodnot(1012, true));
    }
}
