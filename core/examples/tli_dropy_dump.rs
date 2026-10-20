//! Overovací nástroj (TLI 68) — prečíta surový `UE_game.log` presne tou istou
//! cestou ako appka (`StavChvosta::priprav_riadky` + `Sledovac::riadok`) a
//! vypíše ZOZNAM DROPOV ZA KAŽDÚ MAPU ako JSON, aby sa dal porovnať 1:1
//! s referenčným parserom TITrack nad TÝM ISTÝM súborom.
//! Nie je súčasť appky — žiadny release ho nepoužíva.
//!
//! Použitie: `tli_dropy_dump <cesta-k-logu> [--restart <HH:MM:SS>]`
//!
//! `--restart` simuluje REŠTART APPKY uprostred logu: od prvého riadku s
//! daným (alebo vyšším) časom sa pokračuje s ČERSTVÝM `Sledovac`om, presne
//! ako keď sa appka reštartuje a `StavChvosta` jej drží pozíciu v logu, ale
//! stav batohu si zo session neprenáša. Presne tak vznikol bug TLI 68.
//!
//! Výstup: `[{"zona":…,"je_hub":…,"zaciatok":…,"koniec":…,
//!            "dropy":{"<id>":<počet>},"vstupy":{…}}, …]`

use tli_overlay_core::tracker::chvost::StavChvosta;
use tli_overlay_core::tracker::stav::{Beh, Sledovac};

fn cas_txt(c: &Option<tli_overlay_core::tracker::log::Cas>) -> String {
    c.as_ref().map(|c| c.text()).unwrap_or_default()
}

fn vypis(behy: &[Beh]) {
    let mut von = String::from("[\n");
    for (i, beh) in behy.iter().enumerate() {
        let dropy: Vec<String> = beh
            .dropy
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(id, n)| format!("\"{id}\":{n}"))
            .collect();
        let vstupy: Vec<String> = beh
            .vstupy
            .iter()
            .map(|(id, n)| format!("\"{id}\":{n}"))
            .collect();
        von.push_str(&format!(
            "  {{\"zona\":{:?},\"je_hub\":{},\"zaciatok\":{:?},\"koniec\":{:?},\"dropy\":{{{}}},\"vstupy\":{{{}}}}}{}\n",
            beh.zona,
            beh.je_hub,
            cas_txt(&beh.zaciatok),
            cas_txt(&beh.koniec),
            dropy.join(","),
            vstupy.join(","),
            if i + 1 < behy.len() { "," } else { "" }
        ));
    }
    von.push(']');
    println!("{von}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cesta = args.get(1).cloned().expect("cesta k logu ako argument");
    let restart_o: Option<String> = args
        .iter()
        .position(|a| a == "--restart")
        .and_then(|i| args.get(i + 1).cloned());
    let bajty = std::fs::read(&cesta).unwrap_or_else(|e| panic!("čítanie {cesta}: {e}"));
    eprintln!("načítaný {cesta}: {} bajtov", bajty.len());

    let mut chvost = StavChvosta::novy();
    let riadky = chvost.priprav_riadky(&bajty);
    eprintln!("riadkov: {}", riadky.len());

    let mut sledovac = Sledovac::novy();
    let mut behy: Vec<Beh> = Vec::new();
    let mut restart_hotovy = restart_o.is_none();
    for riadok in &riadky {
        if !restart_hotovy {
            if let Some(cas) = restart_o.as_deref() {
                // Riadky majú tvar `[2026.10.20-09.54.00:123][...]`.
                if riadok.len() > 22 && riadok.starts_with('[') {
                    let hhmmss = riadok[12..20].replace('.', ":");
                    if hhmmss.as_str() >= cas {
                        eprintln!("--restart: čerstvý Sledovac od {hhmmss}");
                        sledovac = Sledovac::novy();
                        restart_hotovy = true;
                    }
                }
            }
        }
        if let Some(beh) = sledovac.riadok(riadok) {
            behy.push(beh);
        }
    }
    if let Some(beh) = sledovac.ukonci() {
        behy.push(beh);
    }
    eprintln!("behov: {}", behy.len());
    vypis(&behy);
}
