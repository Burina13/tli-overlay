//! Jednorazový overovací nástroj (TLI 49 bod 3) — prečíta surový
//! `UE_game.log` presne tou istou cestou ako appka (`Chvost::priprav_riadky`
//! + `Sledovac::riadok`) a vypíše stav batohu po stránkach ako JSON, aby sa
//! dal porovnať s referenčným parserom TITrack nad TÝM ISTÝM súborom.
//! Nie je súčasť appky — žiadny release ho nepoužíva.
//!
//! Použitie: `tli_over_titrack <cesta-k-logu>`

use std::collections::BTreeMap;
use tli_overlay_core::tracker::chvost::StavChvosta;
use tli_overlay_core::tracker::stav::Sledovac;

fn main() {
    let cesta = std::env::args().nth(1).expect("cesta k logu ako argument");
    let bajty = std::fs::read(&cesta).unwrap_or_else(|e| panic!("čítanie {cesta}: {e}"));
    eprintln!("načítaný {cesta}: {} bajtov", bajty.len());

    let mut chvost = StavChvosta::novy();
    let riadky = chvost.priprav_riadky(&bajty);
    eprintln!("riadkov: {}", riadky.len());

    let mut sledovac = Sledovac::novy();
    let mut uzavretych_behov = 0;
    for riadok in &riadky {
        if sledovac.riadok(riadok).is_some() {
            uzavretych_behov += 1;
        }
    }
    eprintln!("uzavretých behov: {uzavretych_behov}");

    let mut stranky: BTreeMap<u32, BTreeMap<u32, i64>> = BTreeMap::new();
    for stranka in [100u32, 101, 102, 103] {
        stranky.insert(stranka, sledovac.batoh_stranka(stranka));
    }

    println!("{{");
    for (i, (stranka, mnozstva)) in stranky.iter().enumerate() {
        println!(
            "  \"{stranka}\": {{{}}}{}",
            mnozstva
                .iter()
                .map(|(id, n)| format!("\"{id}\":{n}"))
                .collect::<Vec<_>>()
                .join(","),
            if i + 1 < stranky.len() { "," } else { "" }
        );
    }
    println!("}}");
}
