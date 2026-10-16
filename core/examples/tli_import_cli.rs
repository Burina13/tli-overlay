//! CLI na spätný import historických logov do PERZISTENTNEJ SQLite DB —
//! TLI 15 (server spracúva Richardove staré logy mimo appky). Používa
//! VÝHRADNE existujúci, otestovaný parser (`tracker::import`/`db`/`log`) —
//! žiadna druhá implementácia pravidiel formátu logu.
//!
//! Použitie: `tli_import_cli <cesta-k-db.sqlite> <log1> [log2 ...]`

use tli_overlay_core::tracker::{db, import};

fn main() {
    let mut argy = std::env::args().skip(1);
    let cesta_db = argy.next().expect("prvý argument: cesta k výstupnej DB");
    let cesty_logov: Vec<String> = argy.collect();
    if cesty_logov.is_empty() {
        eprintln!("aspoň jeden log treba zadať ako ďalší argument");
        std::process::exit(1);
    }

    let mut obsahy = Vec::new();
    for cesta in &cesty_logov {
        let bajty = std::fs::read(cesta).unwrap_or_else(|e| panic!("čítanie {cesta}: {e}"));
        eprintln!("načítaný {cesta}: {} bajtov", bajty.len());
        obsahy.push(bajty);
    }

    let mut conn = db::otvor(std::path::Path::new(&cesta_db)).expect("otvorenie DB");
    let vysledok = import::importuj_subory(&mut conn, &obsahy).expect("import");
    println!("{:#?}", vysledok);
}
