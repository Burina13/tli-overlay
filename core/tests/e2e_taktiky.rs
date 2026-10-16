//! E2E (TLI 13b): celá cesta od uzavretých behov cez lokálnu DB až po
//! zoskupenie na taktiky a ich zoradenie podľa FE/h.
//!
//! Vzorka je tá istá `vzorky/session-3-mapy.log` ako v `e2e_tracker.rs` —
//! tri mapy, dve rôzne kombinácie chart+kompasy, dve rôzne zóny. Bez
//! modifikátorov mapy a bez build snapshotu (vzorka ich neobsahuje) sa
//! `podpis_taktiky` zúži na chart+kompasy+mapa, čo z troch behov urobí
//! PRESNE DVE taktiky: „OAZA" (2 behy) a „LAVA" (1 beh) — rovnako, ako sa
//! appka naživo zoskupí nad reálnym logom.
//!
//! Čísla sú prevzaté z `e2e_tracker.rs` (ten istý beh naparsovaný tou istou
//! cestou musí dať to isté číslo) — keď sa test rozbije, pokazila sa buď
//! vzorka, alebo logika zoskupenia/štatistík, nie že si niekto vymyslel iné
//! očakávanie.

use tli_overlay_core::tracker::agregacie::ocen_beh;
use tli_overlay_core::tracker::ceny::{Cena, Cennik, Nastavenie};
use tli_overlay_core::tracker::db::{self, OceneneHodnoty};
use tli_overlay_core::tracker::stav::{Beh, HracSnimka, Sledovac};
use tli_overlay_core::tracker::taktiky;

fn blizko(a: f64, b: f64) {
    assert!((a - b).abs() < 0.001, "{a} != {b}");
}

/// Naplní in-memory DB presne tak, ako to pri živom čítaní robí
/// `src-tauri/src/tracker.rs::uloz_beh_do_db` (duplikuje len malý výpočet
/// `fe_za_hodinu` — ten je v `src-tauri`, kam core testy nedočiahnu).
fn naplnena_db() -> rusqlite::Connection {
    let text = include_str!("vzorky/session-3-mapy.log");
    let mut sledovac = Sledovac::novy();
    let mut behy: Vec<Beh> = Vec::new();
    for riadok in text.lines() {
        if let Some(beh) = sledovac.riadok(riadok) {
            behy.push(beh);
        }
    }
    if let Some(beh) = sledovac.ukonci() {
        behy.push(beh);
    }

    let mut cennik = Cennik::novy();
    for nalez in sledovac.odober_ceny() {
        cennik.vloz(Cena::vlastna(
            nalez.polozka,
            nalez.p10,
            nalez.median,
            nalez.pocet_ponuk,
            1_792_000_000,
        ));
    }

    let mut conn = db::otvor_v_pamati().unwrap();
    let nastavenie = Nastavenie { dan: false };
    for beh in behy.iter().filter(|b| !b.je_hub) {
        let v = ocen_beh(beh, &cennik, &nastavenie).expect("nehubový beh sa ocení vždy");
        let fe_za_hodinu =
            if v.trvanie_ms > 0 { Some(v.cisty_zisk / (v.trvanie_ms as f64 / 3_600_000.0)) } else { None };
        let ocenenie = OceneneHodnoty {
            hodnota_dropov_fe: Some(v.hodnota_dropov),
            hodnota_vstupov_fe: Some(v.hodnota_vstupov),
            cisty_zisk_fe: Some(v.cisty_zisk),
            fe_za_hodinu,
        };
        db::uloz_beh(&mut conn, beh, &HracSnimka::default(), &ocenenie).unwrap();
    }
    conn
}

#[test]
fn tri_behy_sa_zoskupia_do_dvoch_taktik() {
    let conn = naplnena_db();
    let riadky = db::data_pre_taktiky(&conn, &db::FilterTaktik::default()).unwrap();
    assert_eq!(riadky.len(), 3, "tri nehubové behy zo vzorky");

    let modifikatory = db::modifikator_podpisy(&conn).unwrap();
    let priradenia = db::nacitaj_priradenia_taktik(&conn).unwrap();
    let vstupy = taktiky::priprav_vstupy(&riadky, &modifikatory);
    let analyza = taktiky::priprav_analyzu(taktiky::zoskup(&vstupy, &priradenia));

    assert_eq!(analyza.taktiky.len(), 2, "dve rôzne mapy/vstupy musia dať dve taktiky");
}

#[test]
fn ranking_podla_fe_za_hodinu_je_spravny_aj_ked_slabsia_taktika_ma_viac_behov() {
    let conn = naplnena_db();
    let riadky = db::data_pre_taktiky(&conn, &db::FilterTaktik::default()).unwrap();
    let modifikatory = db::modifikator_podpisy(&conn).unwrap();
    let priradenia = db::nacitaj_priradenia_taktik(&conn).unwrap();
    let vstupy = taktiky::priprav_vstupy(&riadky, &modifikatory);
    let skupiny = taktiky::zoskup(&vstupy, &priradenia);

    // LAVA: jeden beh, zisk 302.9 FE za 240 000 ms => 4543.5 FE/h.
    // OAZA: dva behy, zisky 464.9 (300 000 ms => 5578.8 FE/h) a 100.9
    // (180 000 ms => 2018.0 FE/h), priemer (5578.8 + 2018.0) / 2 = 3798.4 FE/h.
    // Napriek tomu, že OAZA má VIAC behov aj vyšší súhrnný zisk, LAVA má
    // vyšší PRIEMERNÝ FE/h — a presne to musí appka ukázať ako prvé.
    assert_eq!(skupiny[0].ukazka_zona.contains("KD_YuanSuKuangDong"), true, "LAVA je prvá (vyšší priemerný FE/h)");
    blizko(skupiny[0].statistika.priemerne_fe_za_hodinu.unwrap(), 4543.5);
    assert_eq!(skupiny[0].statistika.pocet_behov, 1);

    assert_eq!(skupiny[1].ukazka_zona.contains("YunDuanLvZhou"), true, "OAZA je druhá");
    blizko(skupiny[1].statistika.priemerne_fe_za_hodinu.unwrap(), 3798.4);
    assert_eq!(skupiny[1].statistika.pocet_behov, 2);
    blizko(skupiny[1].statistika.priemerny_zisk.unwrap(), 282.9); // (464.9 + 100.9) / 2
    blizko(skupiny[1].statistika.median_zisk.unwrap(), 282.9);
    blizko(skupiny[1].statistika.min_zisk.unwrap(), 100.9);
    blizko(skupiny[1].statistika.max_zisk.unwrap(), 464.9);

    // Tri reálne behy v celej lige sú ZÁMERNE pod prahom „málo dát" (5) —
    // appka to nemá predstierať inak, ani v tomto najlepšom prípade appky.
    assert!(skupiny[0].statistika.malo_dat);
    assert!(skupiny[1].statistika.malo_dat);
}

#[test]
fn odporucanie_pri_malo_datach_nevybera_ziadnu_taktiku() {
    let conn = naplnena_db();
    let riadky = db::data_pre_taktiky(&conn, &db::FilterTaktik::default()).unwrap();
    let modifikatory = db::modifikator_podpisy(&conn).unwrap();
    let priradenia = db::nacitaj_priradenia_taktik(&conn).unwrap();
    let vstupy = taktiky::priprav_vstupy(&riadky, &modifikatory);
    let analyza = taktiky::priprav_analyzu(taktiky::zoskup(&vstupy, &priradenia));

    assert_eq!(analyza.odporucanie.taktika_kluc, None);
    assert!(analyza.odporucanie.text.contains("Zatiaľ málo dát"));
}

#[test]
fn pomenovanie_a_zlucenie_cez_db_zjednoti_obe_taktiky_na_jednu() {
    let conn = naplnena_db();
    let riadky = db::data_pre_taktiky(&conn, &db::FilterTaktik::default()).unwrap();
    let modifikatory = db::modifikator_podpisy(&conn).unwrap();
    let vstupy = taktiky::priprav_vstupy(&riadky, &modifikatory);
    let skupiny_pred = taktiky::zoskup(&vstupy, &db::nacitaj_priradenia_taktik(&conn).unwrap());
    assert_eq!(skupiny_pred.len(), 2);

    let vsetky_podpisy: Vec<String> = skupiny_pred.iter().flat_map(|s| s.podpisy.clone()).collect();
    db::ulozit_taktiku(&conn, &vsetky_podpisy, "Farm všetkého").unwrap();

    let priradenia_po = db::nacitaj_priradenia_taktik(&conn).unwrap();
    let skupiny_po = taktiky::zoskup(&vstupy, &priradenia_po);
    assert_eq!(skupiny_po.len(), 1, "zlúčenie dvoch taktík má dať jednu");
    assert_eq!(skupiny_po[0].nazov, "Farm všetkého");
    assert_eq!(skupiny_po[0].statistika.pocet_behov, 3);
}
