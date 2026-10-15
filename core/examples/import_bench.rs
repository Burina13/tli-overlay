use std::time::Instant;
use tli_overlay_core::tracker::{db, import, log};

fn main() {
    let cesta = std::env::args().nth(1).expect("cesta k logu");
    let bajty = std::fs::read(&cesta).expect("čítanie súboru");
    let text = log::dekoduj(&bajty);
    println!("riadkov (približne): {}", text.lines().count());

    let mut conn = db::otvor_v_pamati().expect("otvorenie DB");
    let zaciatok = Instant::now();
    let vysledok = import::importuj_text(&mut conn, &text).expect("import");
    let trvanie = zaciatok.elapsed();

    println!("trvanie importu: {:?}", trvanie);
    println!("{:#?}", vysledok);

    let riadky = db::vyhladaj_behy(&conn, &db::FilterBehov::default()).expect("vyhladanie");
    println!("pocet behov v DB: {}", riadky.len());
    for r in &riadky {
        println!("  beh {} zona={} monstier={} rares/boss={}", r.id, r.zona, r.pocet_monster, r.pocet_rares_a_bossov);
    }
}
