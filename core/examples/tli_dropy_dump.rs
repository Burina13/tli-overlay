//! Overovací nástroj (TLI 68) — prečíta surový `UE_game.log` presne tou istou
//! cestou ako appka (`StavChvosta::priprav_riadky` + `Sledovac::riadok`) a
//! vypíše ZOZNAM DROPOV ZA KAŽDÚ MAPU ako JSON, aby sa dal porovnať 1:1
//! s referenčným parserom TITrack nad TÝM ISTÝM súborom.
//! Nie je súčasť appky — žiadny release ho nepoužíva.
//!
//! Použitie: `tli_dropy_dump <cesta-k-logu> [--restart <HH:MM:SS>] [--nazhav]`
//!
//! `--nazhav` k tomu simuluje aj opravu TLI 68: reštartovaná appka si
//! základ dropov nažhaví z už spracovanej časti logu (to isté, čo robí
//! `nazhav_zaklad_batohu` v `src-tauri`).
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
    use std::io::Read;

    let args: Vec<String> = std::env::args().collect();
    let cesta = args.get(1).cloned().expect("cesta k logu ako argument");
    let restart_o: Option<String> = args
        .iter()
        .position(|a| a == "--restart")
        .and_then(|i| args.get(i + 1).cloned());
    let nazhavit = args.iter().any(|a| a == "--nazhav");

    // Číta sa po 8 MB dávkach presne ako v appke (`StavChvosta` vie BOM,
    // UTF-16 aj riadok rozdelený na hranici dávky). Richardove rotované logy
    // majú bežne cez 800 MB — načítať taký súbor celý do pamäti znamená, že
    // nástroj zabije OOM killer (CT 111 má 4 GB RAM).
    const DAVKA: usize = 8 * 1024 * 1024;
    let mut subor = std::fs::File::open(&cesta).unwrap_or_else(|e| panic!("čítanie {cesta}: {e}"));
    let velkost = subor.metadata().map(|m| m.len()).unwrap_or(0);
    eprintln!("načítaný {cesta}: {velkost} bajtov");

    let mut chvost = StavChvosta::novy();
    let mut buf = vec![0u8; DAVKA];
    let mut riadkov: usize = 0;

    let mut sledovac = Sledovac::novy();
    let mut behy: Vec<Beh> = Vec::new();
    let mut restart_hotovy = restart_o.is_none();
    // Nažhavovací sledovač vidí VŠETKY riadky od začiatku súboru. Po
    // „reštarte" z neho appka prevezme LEN základ dropov — presne to robí
    // `nazhav_zaklad_batohu` v `src-tauri`, len tam sa tá už spracovaná časť
    // logu číta zo súboru ešte raz.
    let mut nazhavovaci = Sledovac::novy();

    loop {
        let precitane = match subor.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => panic!("čítanie {cesta}: {e}"),
        };
        let riadky = chvost.priprav_riadky(&buf[..precitane]);
        riadkov += riadky.len();
        for riadok in &riadky {
            if nazhavit && !restart_hotovy {
                nazhavovaci.riadok(riadok);
            }
            if !restart_hotovy {
                if let Some(cas) = restart_o.as_deref() {
                    // Riadky majú tvar `[2026.10.20-09.54.00:123][...]`.
                    if riadok.len() > 22 && riadok.starts_with('[') {
                        let hhmmss = riadok[12..20].replace('.', ":");
                        if hhmmss.as_str() >= cas {
                            eprintln!("--restart: čerstvý Sledovac od {hhmmss}");
                            sledovac = Sledovac::novy();
                            if nazhavit {
                                eprintln!("--nazhav: základ dropov prevzatý z už prečítanej časti");
                                sledovac.prevezmi_zaklad_batohu(&nazhavovaci);
                                nazhavovaci = Sledovac::novy();
                            }
                            restart_hotovy = true;
                        }
                    }
                }
            }
            if let Some(beh) = sledovac.riadok(riadok) {
                behy.push(beh);
            }
        }
    }
    eprintln!("riadkov: {riadkov}");
    if let Some(beh) = sledovac.ukonci() {
        behy.push(beh);
    }
    eprintln!("behov: {}", behy.len());
    vypis(&behy);
}
