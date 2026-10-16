//! Jednorazový debug nástroj (TLI 17 príprava): vypíše pre každý beh zo zadaného
//! logu zónu, level_typ a podpis vstupov — na overenie kampaň/mapa klasifikácie
//! proti živému Richardovmu logu. Nejde do appky, len `cargo run --example`.
use tli_overlay_core::tracker::{log, stav};

fn main() {
    let cesta = std::env::args().nth(1).expect("cesta k logu");
    let bajty = std::fs::read(&cesta).expect("čítanie logu");
    let text = log::dekoduj(&bajty);

    let mut sledovac = stav::Sledovac::novy();
    for riadok in text.lines() {
        if let Some(beh) = sledovac.riadok(riadok) {
            vypis(&beh);
        }
    }
    if let Some(beh) = sledovac.ukonci() {
        vypis(&beh);
    }
}

fn vypis(beh: &stav::Beh) {
    println!(
        "zona={} je_hub={} typ={:?} podpis={}",
        beh.zona,
        stav::je_hub(&beh.zona),
        beh.level_typ,
        beh.podpis_vstupov()
    );
}
