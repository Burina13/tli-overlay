//! Pripomienka „Enable Log" (TLI 11d) — doplnok k TLI 11c.
//!
//! Hra si nastavenie Settings → Other Settings → „Enable Log" NEPAMÄTÁ medzi
//! spusteniami: keď ho hráč zabudne zapnúť, appka síce nájde `UE_game.log`
//! (TLI 11c), ale do neho nepribudne ani jeden z riadkov, ktoré tracker vie
//! spracovať — a celá session sa potichu nenatrackuje.
//!
//! Dva typy riadkov v logu majú ÚPLNE inú podmienku vzniku (zistené z
//! existujúcej kontroly `obsahuje_ocakavane_znacky` v `src-tauri/src/tracker.rs`,
//! TLI 11c): `BagMgr`/`ItemChange` (inventár, vstupy mapy) sa zapíšu LEN keď je
//! „Enable Log" zapnuté, kým prechod medzi mapami (`SceneLevelMgr@`) si engine
//! loguje VŽDY, nezávisle od tohto nastavenia. Vďaka tomu appka vie oboje:
//! či logovanie beží (prvý typ) AJ kedy hráč vstúpil do prvej mapy (druhý typ)
//! — aj v momente, keď prvý typ celú session chýba.
//!
//! Celý modul je čistá logika nad injektovaným unixovým časom (žiadne reálne
//! čakanie) — testuje sa okamžite, bez skutočných 60 sekúnd.

use super::log::{parsuj_riadok, Udalost};
use super::stav::je_hub;

/// Koľko sekúnd od začiatku session má hráč čas zapnúť logovanie, kým appka
/// ukáže upozornenie (požiadavka TLI 11d, bod 2 zadania).
pub const LEHOTA_S: u64 = 60;

/// Klasifikácia jedného surového riadku logu pre potreby tejto pripomienky.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalRiadku {
    /// Riadok, ktorý sa v logu objaví LEN keď je v hre zapnuté „Enable Log"
    /// (zmena batohu alebo kontext `ItemChange@`, vrátane `Spv3Open`).
    EnableLogZnacka,
    /// Vstup do novej, NEHUBOVEJ mapy (`SceneLevelMgr@ OpenMainWorld`) — táto
    /// značka sa v logu objavuje nezávisle od „Enable Log".
    VstupDoMapy,
    /// Nič z vyššie uvedeného (vrátane neznámych riadkov).
    Ine,
}

/// Rozhodne, čo daný surový riadok logu znamená pre stav pripomienky.
/// Znovupoužíva `parsuj_riadok`/`je_hub` — rovnaký zdroj pravdy ako zvyšok
/// trackera, žiadna druhá kópia rozpoznávacej logiky.
pub fn klasifikuj(riadok: &str) -> SignalRiadku {
    match parsuj_riadok(riadok) {
        Some((_, Udalost::Batoh(_)))
        | Some((_, Udalost::BatohOdstranenie { .. }))
        | Some((_, Udalost::Kontext { .. })) => SignalRiadku::EnableLogZnacka,
        Some((_, Udalost::Zona { cesta })) if !je_hub(&cesta) => SignalRiadku::VstupDoMapy,
        _ => SignalRiadku::Ine,
    }
}

/// Udalosť, ktorú má appka ukázať hráčovi (toast, Windows notifikácia, červený
/// stav v záložke Tracker).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdalostPripomienky {
    /// Lehota vypršala a žiadna `EnableLogZnacka` neprišla — prvé upozornenie.
    ZobrazUpozornenie,
    /// Hráč vošiel do prvej mapy tejto session a stále neprišla ani jedna
    /// `EnableLogZnacka` — upozornenie sa opakuje (bod 4 zadania).
    ZopakujUpozornenie,
    /// Prišla prvá `EnableLogZnacka` PO predošlom upozornení — logovanie sa
    /// práve rozbehlo, appka má krátko ukázať „beží OK".
    LogovanieBeziOk,
}

/// Stavový automat jednej session. Žije v appke vedľa `StavTrackera` a
/// `nova_session`/`spracuj_riadok`/`tik` volá volajúci (v `src-tauri`) podľa
/// toho, čo sa práve deje — rotácia logu, nový riadok, pravidelný tik vlákna
/// na pozadí.
#[derive(Debug, Default)]
pub struct SledovacPripomienky {
    /// Nastavenie „Pripomínať Enable Log" (predvolene zapnuté) — keď je
    /// vypnuté, celý automat mlčí a nič neprodukuje.
    zapnute: bool,
    session_od: Option<u64>,
    enable_log_videny: bool,
    prve_upozornenie_poslane: bool,
    druhe_upozornenie_poslane: bool,
    /// „Červený" stav pre UI — aktívny, kým logovanie evidentne nebeží.
    aktivne: bool,
    udalosti: Vec<UdalostPripomienky>,
}

impl SledovacPripomienky {
    pub fn novy(zapnute: bool) -> Self {
        Self { zapnute, ..Self::default() }
    }

    /// Nastavenie sa dá zmeniť za behu appky (hráč prepne checkbox) — automat
    /// sa tým nereštartuje, len prestane/začne produkovať udalosti.
    pub fn nastav_zapnute(&mut self, zapnute: bool) {
        self.zapnute = zapnute;
    }

    /// Aktuálna hodnota nastavenia — slúži volajúcemu v `src-tauri`, ktorý
    /// potrebuje ju preniesť cez vnútorný reset `StavTrackera` pri zmene
    /// cesty k logu (`precitaj`).
    pub fn zapnute(&self) -> bool {
        self.zapnute
    }

    /// Má sa práve teraz ukazovať červený stav v záložke Tracker?
    pub fn aktivne(&self) -> bool {
        self.zapnute && self.aktivne
    }

    /// Nová session (appka zistila rotáciu `UE_game.log` — hra sa reštartovala).
    /// Zruší celú predošlú históriu a začne odznova.
    pub fn nova_session(&mut self, teraz: u64) {
        let zapnute = self.zapnute;
        *self = Self { zapnute, session_od: Some(teraz), ..Self::default() };
    }

    /// Spracovanie jedného surového riadku logu (klasifikáciu robí volajúci
    /// cez `klasifikuj`, aby sa táto funkcia dala volať aj priamo v testoch
    /// bez textu riadku).
    pub fn spracuj_riadok(&mut self, teraz: u64, signal: SignalRiadku) {
        if !self.zapnute {
            return;
        }
        if self.session_od.is_none() {
            // appka začala čítať log, ktorý už obsahuje riadky, bez toho, aby
            // sme videli rotáciu (napr. appka sa spustila až počas hrania) —
            // aj to je, z pohľadu tejto session, jej začiatok.
            self.session_od = Some(teraz);
        }
        match signal {
            SignalRiadku::EnableLogZnacka => {
                let bolo_zle = self.aktivne;
                self.enable_log_videny = true;
                self.aktivne = false;
                if bolo_zle {
                    self.udalosti.push(UdalostPripomienky::LogovanieBeziOk);
                }
            }
            SignalRiadku::VstupDoMapy => {
                if !self.enable_log_videny
                    && self.prve_upozornenie_poslane
                    && !self.druhe_upozornenie_poslane
                {
                    self.druhe_upozornenie_poslane = true;
                    self.aktivne = true;
                    self.udalosti.push(UdalostPripomienky::ZopakujUpozornenie);
                }
            }
            SignalRiadku::Ine => {}
        }
    }

    /// Volané pravidelne (vlákno na pozadí, á 1 s) nezávisle od toho, či
    /// prišiel nový riadok — rieši prípad, keď appka beží ďalej a v logu sa
    /// nestane ÚPLNE NIČ (ani značka, ani vstup do mapy).
    pub fn tik(&mut self, teraz: u64) {
        if !self.zapnute {
            return;
        }
        let Some(od) = self.session_od else { return };
        if self.enable_log_videny || self.prve_upozornenie_poslane {
            return;
        }
        if teraz.saturating_sub(od) < LEHOTA_S {
            return;
        }
        self.prve_upozornenie_poslane = true;
        self.aktivne = true;
        self.udalosti.push(UdalostPripomienky::ZobrazUpozornenie);
    }

    /// Vyberie a vyprázdni frontu udalostí od posledného volania — rovnaký
    /// vzor ako `Sledovac::odober_ceny`.
    pub fn odober_udalosti(&mut self) -> Vec<UdalostPripomienky> {
        std::mem::take(&mut self.udalosti)
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn klasifikuj_bagmgr_je_enable_log_znacka() {
        let r = "BagMgr@:InitBagData PageId = 1 SlotId=1 ConfigBaseId=1 Num=1";
        assert_eq!(klasifikuj(r), SignalRiadku::EnableLogZnacka);
    }

    #[test]
    fn klasifikuj_item_change_spv3open_je_enable_log_znacka() {
        let r = "ItemChange@ ProtoName=Spv3Open start";
        assert_eq!(klasifikuj(r), SignalRiadku::EnableLogZnacka);
    }

    #[test]
    fn klasifikuj_odstranenie_je_tiez_enable_log_znacka() {
        let r = "BagMgr@:RemoveBagItem PageId=1 SlotId = 2";
        assert_eq!(klasifikuj(r), SignalRiadku::EnableLogZnacka);
    }

    #[test]
    fn klasifikuj_vstup_do_nehubovej_mapy() {
        let r = "SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Maps/Zone1";
        assert_eq!(klasifikuj(r), SignalRiadku::VstupDoMapy);
    }

    #[test]
    fn klasifikuj_vstup_do_hubu_nie_je_vstup_do_mapy() {
        let r = "SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Maps/Town_Hideout";
        assert_eq!(klasifikuj(r), SignalRiadku::Ine);
    }

    #[test]
    fn klasifikuj_neznamy_riadok_je_ine() {
        assert_eq!(klasifikuj("hocico, co appka nepozna"), SignalRiadku::Ine);
    }

    #[test]
    fn vypnuta_pripomienka_nic_neprodukuje() {
        let mut s = SledovacPripomienky::novy(false);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S + 1);
        s.spracuj_riadok(1000, SignalRiadku::VstupDoMapy);
        assert!(s.odober_udalosti().is_empty());
        assert!(!s.aktivne());
    }

    #[test]
    fn ticho_pred_vyprsanim_lehoty() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S - 1);
        assert!(s.odober_udalosti().is_empty());
        assert!(!s.aktivne());
    }

    #[test]
    fn upozornenie_po_vyprsani_lehoty_bez_jedinej_znacky() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S);
        assert_eq!(s.odober_udalosti(), vec![UdalostPripomienky::ZobrazUpozornenie]);
        assert!(s.aktivne());
    }

    #[test]
    fn opakovany_tik_po_upozorneni_uz_nic_neposle() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S);
        s.odober_udalosti();
        s.tik(1000 + LEHOTA_S + 30);
        assert!(s.odober_udalosti().is_empty());
        // Stále aktívne — čaká sa na riadok (enable-log značku alebo vstup do mapy).
        assert!(s.aktivne());
    }

    #[test]
    fn enable_log_znacka_pred_lehotou_zabrani_upozorneniu() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.spracuj_riadok(1010, SignalRiadku::EnableLogZnacka);
        s.tik(1000 + LEHOTA_S + 1);
        assert!(s.odober_udalosti().is_empty());
        assert!(!s.aktivne());
    }

    #[test]
    fn znacka_po_upozorneni_vyprodukuje_ok_a_vypne_cerveny_stav() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S);
        s.odober_udalosti();
        assert!(s.aktivne());

        s.spracuj_riadok(1000 + LEHOTA_S + 5, SignalRiadku::EnableLogZnacka);
        assert_eq!(s.odober_udalosti(), vec![UdalostPripomienky::LogovanieBeziOk]);
        assert!(!s.aktivne());
    }

    #[test]
    fn znacka_bez_predosleho_upozornenia_neposle_ziadnu_udalost() {
        // Logovanie bolo v poriadku od začiatku — "beží OK" sa netreba chváliť,
        // keď nič nebolo zle.
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.spracuj_riadok(1005, SignalRiadku::EnableLogZnacka);
        assert!(s.odober_udalosti().is_empty());
    }

    #[test]
    fn vstup_do_mapy_bez_predosleho_upozornenia_nic_nerobi() {
        // Hráč ešte nedostal prvé upozornenie (lehota ani nevypršala) — vstup
        // do mapy sám osebe nič nespúšťa.
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.spracuj_riadok(1010, SignalRiadku::VstupDoMapy);
        assert!(s.odober_udalosti().is_empty());
    }

    #[test]
    fn druhe_pripomenutie_pri_vstupe_do_prvej_mapy() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S);
        s.odober_udalosti();

        s.spracuj_riadok(1000 + LEHOTA_S + 10, SignalRiadku::VstupDoMapy);
        assert_eq!(s.odober_udalosti(), vec![UdalostPripomienky::ZopakujUpozornenie]);
        assert!(s.aktivne());
    }

    #[test]
    fn druhe_pripomenutie_sa_neopakuje_pri_dalsich_mapach() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S);
        s.odober_udalosti();
        s.spracuj_riadok(1000 + LEHOTA_S + 10, SignalRiadku::VstupDoMapy);
        s.odober_udalosti();

        s.spracuj_riadok(1000 + LEHOTA_S + 200, SignalRiadku::VstupDoMapy);
        assert!(s.odober_udalosti().is_empty(), "druhé pripomenutie je len jedno, nie pri každej mape");
    }

    #[test]
    fn nova_session_zrusi_predoslu_historiu() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S);
        s.odober_udalosti();
        assert!(s.aktivne());

        s.nova_session(5000);
        assert!(!s.aktivne());
        s.tik(5000 + LEHOTA_S - 1);
        assert!(s.odober_udalosti().is_empty(), "nová session má svoju vlastnú čerstvú lehotu");
    }

    #[test]
    fn prvy_riadok_bez_explicitnej_novej_session_zacina_lehotu() {
        // Appka sa spustila, kým hra už bežala — žiadna rotácia sa nestala,
        // prvý spracovaný riadok preto sám začína počítanie lehoty.
        let mut s = SledovacPripomienky::novy(true);
        s.spracuj_riadok(2000, SignalRiadku::Ine);
        s.tik(2000 + LEHOTA_S);
        assert_eq!(s.odober_udalosti(), vec![UdalostPripomienky::ZobrazUpozornenie]);
    }

    #[test]
    fn prepnutie_na_vypnute_pocas_aktivneho_upozornenia_stiahne_cerveny_stav() {
        let mut s = SledovacPripomienky::novy(true);
        s.nova_session(1000);
        s.tik(1000 + LEHOTA_S);
        s.odober_udalosti();
        assert!(s.aktivne());

        s.nastav_zapnute(false);
        assert!(!s.aktivne(), "vypnuté nastavenie nesmie ukazovať červený stav");
    }
}
