//! E2E: celá cesta od surového `UE_game.log` po rozdelenie zisku podľa kombinácie
//! „chart + kompasy + mapa“.
//!
//! Vzorka `vzorky/session-3-mapy.log` je vymyslená session v PRESNOM tvare, aký
//! píše hra (formát je popísaný v `docs/fakty/hry/torchlight-infinite/log-format.md`
//! v ops repe). Zámerne obsahuje aj veci, ktoré parser nesmie zraziť: nákup na trhu
//! uprostred mapy (nesmie sa počítať ako drop), položku bez známej ceny, riadky,
//! ktorým parser nerozumie, a prázdny riadok.
//!
//! Čísla v testoch sú dopočítané ručne zo vzorky — keď sa test rozbije, pokazila sa
//! buď vzorka, alebo logika; v oboch prípadoch to treba pozrieť, nie prepísať číslo.

use std::collections::BTreeMap;
use tli_overlay_core::tracker::agregacie::{ocen_beh, prehlad};
use tli_overlay_core::tracker::ceny::{Cena, Cennik, Nastavenie};
use tli_overlay_core::tracker::stav::{Beh, Sledovac};

/// Prejde vzorku a vráti hotové behy aj cenník naplnený z trhových vyhľadávaní v logu.
fn prejdi_vzorku() -> (Vec<Beh>, Cennik, u64) {
    let text = include_str!("vzorky/session-3-mapy.log");
    let mut sledovac = Sledovac::novy();
    let mut behy = Vec::new();
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
    (behy, cennik, sledovac.neznamych_riadkov())
}

fn mapy(behy: &[Beh]) -> Vec<&Beh> {
    behy.iter().filter(|b| !b.je_hub).collect()
}

fn blizko(a: f64, b: f64) {
    assert!((a - b).abs() < 0.001, "{a} != {b}");
}

#[test]
fn ceny_sa_nacitali_z_trhovych_vyhladavani() {
    let (_, cennik, _) = prejdi_vzorku();
    // Referenčná cena je 10. percentil, teda pri piatich ponukách tá najnižšia.
    blizko(cennik.cena(400021).unwrap().fe, 2.10);
    blizko(cennik.cena(500101).unwrap().fe, 0.50);
    blizko(cennik.cena(500102).unwrap().fe, 1.00);
    blizko(cennik.cena(5080).unwrap().fe, 0.10);
    blizko(cennik.cena(140005).unwrap().fe, 0.02);
    assert_eq!(cennik.pocet(), 5);
}

#[test]
fn session_ma_tri_mapy_so_spravnym_trvanim() {
    let (behy, _, _) = prejdi_vzorku();
    let m = mapy(&behy);
    assert_eq!(m.len(), 3, "očakávané tri mapy, nájdené {}", m.len());
    assert_eq!(m[0].trvanie_ms(), Some(300_000));
    assert_eq!(m[1].trvanie_ms(), Some(240_000));
    assert_eq!(m[2].trvanie_ms(), Some(180_000));
}

#[test]
fn startovaci_chart_a_kompasy_sa_rozpoznali() {
    let (behy, _, _) = prejdi_vzorku();
    let m = mapy(&behy);

    let ocakavane1: BTreeMap<u32, i64> = [(400021, 1), (500101, 2)].into_iter().collect();
    let ocakavane2: BTreeMap<u32, i64> = [(400021, 1), (500102, 1)].into_iter().collect();
    assert_eq!(m[0].vstupy, ocakavane1);
    assert_eq!(m[1].vstupy, ocakavane2);
    assert_eq!(m[2].vstupy, ocakavane1);

    // Prvá a tretia mapa majú rovnaký podpis — musia padnúť do jednej skupiny.
    assert_eq!(m[0].podpis_vstupov(), "400021x1+500101x2");
    assert_eq!(m[0].podpis_vstupov(), m[2].podpis_vstupov());
    assert_ne!(m[0].podpis_vstupov(), m[1].podpis_vstupov());
}

#[test]
fn nakup_na_trhu_sa_nepocita_ako_drop() {
    let (behy, _, _) = prejdi_vzorku();
    let m = mapy(&behy);
    // V prvej mape hráč kúpil 10 kusov kompasu 500101 — do dropov nesmie.
    assert!(!m[0].dropy.contains_key(&500101), "nákup na trhu sa započítal ako drop");
    assert_eq!(m[0].dropy.get(&5080), Some(&120));
    assert_eq!(m[0].dropy.get(&140005), Some(&300));
    assert_eq!(m[0].dropy.get(&100300), Some(&450));
}

#[test]
fn cisty_zisk_je_dropy_minus_vstupy() {
    let (behy, cennik, _) = prejdi_vzorku();
    let n = Nastavenie::default();
    let v: Vec<_> = behy.iter().filter_map(|b| ocen_beh(b, &cennik, &n)).collect();
    assert_eq!(v.len(), 3);

    // Mapa 1: dropy 120×0,10 + 300×0,02 + 450 FE = 468,00; vstupy 2,10 + 2×0,50 = 3,10
    blizko(v[0].hodnota_dropov, 468.0);
    blizko(v[0].hodnota_vstupov, 3.10);
    blizko(v[0].cisty_zisk, 464.90);

    // Mapa 2: dropy 60×0,10 + 300 FE = 306,00 (neznáma položka hodnotu nepridá)
    blizko(v[1].hodnota_dropov, 306.0);
    blizko(v[1].hodnota_vstupov, 3.10);
    blizko(v[1].cisty_zisk, 302.90);

    // Mapa 3: dropy 40×0,10 + 100 FE = 104,00
    blizko(v[2].cisty_zisk, 100.90);
}

#[test]
fn neznama_polozka_sa_zapise_a_nepokazi_sucet() {
    let (behy, cennik, _) = prejdi_vzorku();
    let n = Nastavenie::default();
    let v: Vec<_> = behy.iter().filter_map(|b| ocen_beh(b, &cennik, &n)).collect();
    assert!(v[1].bez_ceny.contains(&(999_999, 3)), "neznáma položka sa nezapísala: {:?}", v[1].bez_ceny);
    blizko(v[1].hodnota_dropov, 306.0);
}

#[test]
fn prehlad_pocita_za_hodinu_a_rozdeluje_podla_kombinacie() {
    let (behy, cennik, _) = prejdi_vzorku();
    let n = Nastavenie::default();
    let v: Vec<_> = behy.iter().filter_map(|b| ocen_beh(b, &cennik, &n)).collect();
    let p = prehlad(&v, &behy);

    assert_eq!(p.celkom.pocet_map, 3);
    assert_eq!(p.celkom.cas_ms, 720_000);
    blizko(p.celkom.cisty_zisk, 868.70);
    blizko(p.celkom.zisk_za_hodinu(), 868.70 * 5.0);   // 720 s = 1/5 hodiny
    blizko(p.celkom.map_za_hodinu(), 15.0);
    blizko(p.celkom.priemer_na_mapu(), 868.70 / 3.0);
    assert_eq!(p.celkom.priemerne_trvanie_ms(), 240_000);

    // Dve kombinácie: (beacon + 2× kompas A) na oáze a (beacon + kompas B) v lave.
    assert_eq!(p.podla_kombinacie.len(), 2);
    // Druhá kombinácia zarobí za hodinu viac (302,90 za 240 s), takže je prvá.
    assert!(p.podla_kombinacie[0].kluc.contains("500102"), "zle zoradené: {:?}",
            p.podla_kombinacie.iter().map(|s| &s.kluc).collect::<Vec<_>>());
    blizko(p.podla_kombinacie[0].suhrn.zisk_za_hodinu(), 302.90 * 15.0);
    blizko(p.podla_kombinacie[1].suhrn.zisk_za_hodinu(), 565.80 * 7.5);

    // Podľa vstupov zlúči prvú a tretiu mapu do jednej skupiny.
    assert_eq!(p.podla_vstupov.len(), 2);
    let a = p.podla_vstupov.iter().find(|s| s.kluc == "400021x1+500101x2").unwrap();
    assert_eq!(a.suhrn.pocet_map, 2);

    // Podľa mapy: oáza 2×, lava 1×.
    assert_eq!(p.podla_mapy.len(), 2);

    // Dropy sa posčítali naprieč mapami.
    assert_eq!(p.dropy_spolu.get(&5080), Some(&220));
    assert_eq!(p.bez_ceny_spolu.get(&999_999), Some(&3));
}

#[test]
fn neznamym_riadkom_parser_neumrie() {
    let (behy, _, neznamych) = prejdi_vzorku();
    assert!(!behy.is_empty());
    // Vzorka ich obsahuje zámerne; dôležité je, že sa len spočítali.
    assert!(neznamych > 0, "vzorka mala obsahovať riadky, ktorým parser nerozumie");
}
