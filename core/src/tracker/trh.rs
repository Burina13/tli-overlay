//! Detekcia otvorenia obrazovky trhu „AuctionHouseShowItemGoodV2" (TLI 22,
//! `2026-10-16-1705-mcp.md`) — doplnok k TLI 20.
//!
//! SS14 do `UE_game.log` ceny NEPÍŠE (`XchgSearchPrice` sa v aktuálnej sezóne
//! nevyskytuje ani raz — overené 16.10.2026 nad Richardovým reálnym logom,
//! `docs/fakty/hry/torchlight-infinite/market-ceny-kontrakt.md`), ale
//! ŽIVOTNÝ CYKLUS stránky s detailom položky na trhu sa loguje VŽDY, rovnako
//! ako prechod medzi mapami v `pripomienka.rs`. Appka preto nemusí čakať na
//! hotkey — keď hráč otvorí detail ponuky, appka to vie z logu samu a sama
//! odfotí a pošle na `?modul=ceny`.
//!
//! Riadok (reálny, z Richardovho logu 14.10.2026, `tli-logy/UE_game-backup-
//! 2026.10.14-22.44.27.log:102209`):
//! `[2026.10.14-22.23.45:283][305]TLLua: Display: [Game] AuctionHouseShowItemGoodV2 Run`
//!
//! Čisto textová zhoda, žiadny stav — appka ho volá pre KAŽDÝ nový riadok
//! logu (rovnaký vzor ako `pripomienka::klasifikuj`). Debounce proti
//! viacnásobnému vyvolaniu v tom istom okamihu (napr. opakovaný riadok pri
//! chvoste súboru) rieši volajúci v `src-tauri` tým istým zámkom `beh`, čo už
//! chráni pred dvojitým stlačením F8/F9.

/// `true`, keď riadok hovorí, že práve NABEHLA stránka detailu položky na
/// trhu (`PageRunState = Run`, nie `OpenFlow`/`Destory`) — vtedy sú ceny už
/// vykreslené a dá sa fotiť.
pub fn je_otvorenie_trhu(riadok: &str) -> bool {
    riadok.contains("AuctionHouseShowItemGoodV2 Run")
}

#[cfg(test)]
mod testy {
    use super::*;

    const RIADOK_RUN: &str =
        "[2026.10.14-22.23.45:283][305]TLLua: Display: [Game] AuctionHouseShowItemGoodV2 Run";
    const RIADOK_DESTORY: &str =
        "[2026.10.14-22.23.48:676][342]TLLua: Display: [Game] AuctionHouseShowItemGoodV2 Destory";
    const RIADOK_OPENVIEW: &str =
        "[2026.10.14-22.23.45:271][305]TLLua: Display: [Game] PageBase@ OpenFlow5! OpenView AuctionHouseShowItemGoodV2";
    const RIADOK_CTRL: &str = "[2026.10.14-22.23.45:283][305]TLLua: Display: [Game] PageStack@ \
CurRunView = 5804_AuctionHouseShowItemGoodV2Ctrl";

    #[test]
    fn rozpozna_realny_riadok_run() {
        assert!(je_otvorenie_trhu(RIADOK_RUN));
    }

    #[test]
    fn nerozpozna_destory_ani_openview() {
        // Destory = hráč stránku ZAVIERA, fotiť už nemá zmysel. OpenView
        // prichádza skôr, než sa dáta vôbec vykreslia — fotenie by chytilo
        // prázdnu/nabiehajúcu obrazovku.
        assert!(!je_otvorenie_trhu(RIADOK_DESTORY));
        assert!(!je_otvorenie_trhu(RIADOK_OPENVIEW));
    }

    #[test]
    fn nerozpozna_pribuzne_ale_ine_auction_house_stranky() {
        // AuctionHouseV2 (zoznam/search) a iné pod-stránky majú vlastné mená —
        // nemajú spúšťať fotenie detailu položky.
        assert!(!je_otvorenie_trhu(
            "TLLua: Display: [Game] AuctionHouseV2 Run"
        ));
        assert!(!je_otvorenie_trhu(
            "TLLua: Display: [Game] AuctionHouseSearchV2 Run"
        ));
    }

    #[test]
    fn nerozpozna_ctrl_stavove_riadky() {
        assert!(!je_otvorenie_trhu(RIADOK_CTRL));
    }

    #[test]
    fn nerozpozna_prazdny_ani_necitatelny_riadok() {
        assert!(!je_otvorenie_trhu(""));
        assert!(!je_otvorenie_trhu("\u{0}\u{0}odpad"));
    }
}
