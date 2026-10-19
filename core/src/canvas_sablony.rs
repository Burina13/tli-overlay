//! Template matching ikon Monster Paint (TLI 59b2b, bod 2 zo zadania
//! `2026-10-20-0032-agent.md`) — rozhodne, KTORÝ z 13 `MONSTER_PAINT_TYPY`
//! (`canvas_model.py` v `ops`) je na položenom painte v jednej bunke, nad
//! výrezom z `canvas_rozlozenie` + `obraz::orez_box01` (rovnaká geometria ako
//! `canvas_obsah`, volá sa až keď tá vráti `ObsahBunky::Namalovana`).
//!
//! **Algoritmus je zámerne najjednoduchší funkčný** (zadanie to výslovne
//! žiada: „priemerná farba + hrubý histogram, nie plná perceptuálna hash
//! kniha navyše") — kvantizovaný 3D RGB histogram (6 binov na kanál, 216
//! binov spolu), porovnávaný cez prienik (súčet `min` po binoch, 0..1).
//! Šablóny sú vopred spočítané OFFLINE z reálnych ikon
//! `/srv/games/tli/ikony/canvas/*.webp` skriptom
//! `scripts/games/tli-ikony-canvas-sablony-generuj.py` (repo `ops`) do
//! `canvas_sablony_data.rs` — appka preto nepotrebuje ŽIADNU závislosť na
//! dekódovaní obrázkov (žiadna `image`/`webp` krata v `Cargo.toml`), len
//! porovnáva dve polia čísel.
//!
//! **Prečo histogram a nie priemerná farba:** priemerná farba samotná
//! (vyskúšané s rôznym orezaním okraja bunky, 0–30 %) dala na kalibračnej
//! sade len 7,5–12,5 % presnosti — prakticky náhoda pri 13 typoch (1/13 ≈
//! 7,7 %). Kvantizovaný histogram CELEJ bunky (bez orezania) dal **37,5 %
//! top-1 / 50,0 % top-3** — najlepšia z vyskúšaných kombinácií (4/5/6/7/8
//! binov, rôzne orezanie okraja aj vylúčenie pravého dolného rohu s „LvN"
//! odznakom, aj separovateľný R+G+B histogram namiesto spoločného 3D).
//!
//! **Prečo len 37,5 %, nie viac:** zmerané na 40 reálnych bunkách s monster
//! paintom (13 screenshotov, `/srv/games/tli/analyzy`, CT 114). Bunka v hre
//! má okolo ikony farebný PRSTENEC, ktorého farba sa MENÍ medzi inštanciami
//! (overené vizuálne — ten istý typ raz s modrým, inokedy s iným prstencom)
//! a nie je súčasťou oficiálnej ikony (tá má priehľadné pozadie) — histogram
//! celej bunky preto nesie aj tento cudzí signál, ktorý maskuje farby
//! samotnej príšery. Presnejšie maskovanie prstenca/odznaku je mimo rozsahu
//! tejto časti — zapísané v `BACKLOG.md`.
//!
//! **Preto táto funkcia VŽDY vracia aj skóre istoty** (prienik histogramov,
//! 0..1, VYŠŠIE = podobnejšie), nie len najlepšiu zhodu bez miery istoty —
//! volajúci (plánovaná TLI 59b2c, zapojenie do appky) má podľa tohto skóre
//! vedieť padnúť na server namiesto slepého dôverovania nízko istej lokálnej
//! zhode, rovnako ako `istota_rozpoznania` z `api::AnalyzaCanvas`.

use crate::canvas_rozlozenie::platno_bunka_box01;
use crate::obraz::orez_box01;

mod canvas_sablony_data;
pub use canvas_sablony_data::{POCET_BINOV, SABLONY, VELKOST_HISTOGRAMU};

/// Kvantizovaný 3D RGB histogram CELÉHO výrezu (bez orezania okraja — pozri
/// doc-komentár modulu, prečo orezanie na kalibračnej sade nepomohlo),
/// normalizovaný na súčet 1. Rovnaký binovací vzorec ako generátor šablón
/// (`tli-ikony-canvas-sablony-generuj.py`) — zmena jedného bez druhého by
/// porovnanie potichu pokazila, preto `POCET_BINOV` žije v generovanom
/// súbore a používa sa TU, nie ako samostatná konštanta.
pub fn histogram(rgb: &[u8], sirka: u32, vyska: u32) -> Option<[f32; VELKOST_HISTOGRAMU]> {
    let pocet_pixelov = (sirka as usize) * (vyska as usize);
    if sirka == 0 || vyska == 0 || rgb.len() < pocet_pixelov * 3 {
        return None;
    }
    let biny = POCET_BINOV as u32;
    let mut h = [0u32; VELKOST_HISTOGRAMU];
    for i in 0..pocet_pixelov {
        let (r, g, b) = (rgb[i * 3] as u32, rgb[i * 3 + 1] as u32, rgb[i * 3 + 2] as u32);
        let ri = (r * biny / 256).min(biny - 1);
        let gi = (g * biny / 256).min(biny - 1);
        let bi = (b * biny / 256).min(biny - 1);
        h[((ri * biny + gi) * biny + bi) as usize] += 1;
    }
    let mut vysledok = [0f32; VELKOST_HISTOGRAMU];
    for (i, pocet) in h.iter().enumerate() {
        vysledok[i] = *pocet as f32 / pocet_pixelov as f32;
    }
    Some(vysledok)
}

/// Prienik dvoch normalizovaných histogramov (súčet `min` po binoch) — 1,0 pri
/// identických histogramoch, 0,0 keď nemajú spoločný bin. Symetrická miera,
/// zámerne lacnejšia než napr. chí-kvadrát vzdialenosť.
pub fn prekryv(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x.min(*y)).sum()
}

/// Nad už vyrezaným RGB bufferom jednej bunky nájde najbližšiu šablónu z
/// `MONSTER_PAINT_TYPY` a vráti `(typ, skóre_istoty)`. `None`, len keď je
/// vstup poškodený/príliš malý — medzi 13 šablónami sa vždy niečo nájde,
/// nízke skóre signalizuje slabú zhodu, nie chýbajúci výsledok.
pub fn rozpoznaj_ikonu_z_vyrezu(rgb: &[u8], sirka: u32, vyska: u32) -> Option<(String, f32)> {
    let h = histogram(rgb, sirka, vyska)?;
    SABLONY
        .iter()
        .map(|(typ, sablona)| (*typ, prekryv(&h, sablona)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(typ, skore)| (typ.to_string(), skore))
}

/// Vyreže bunku `riadok`/`stlpec` (1-indexované) z celej snímky podľa pevnej
/// geometrie `canvas_rozlozenie` a rozpozná ikonu na nej — skratka pre
/// volajúceho, ktorý má celú snímku, nie už hotový výrez jednej bunky.
pub fn rozpoznaj_ikonu(rgb: &[u8], sirka: u32, vyska: u32, riadok: i64, stlpec: i64) -> Option<(String, f32)> {
    let oblast = platno_bunka_box01(riadok, stlpec);
    let (vyrez, w, h) = orez_box01(rgb, sirka, vyska, oblast)?;
    rozpoznaj_ikonu_z_vyrezu(&vyrez, w, h)
}

#[cfg(test)]
mod testy {
    use super::*;

    fn vzorka(nazov: &str) -> Vec<u8> {
        let cesta = format!("{}/testy-data/{}.rgb", env!("CARGO_MANIFEST_DIR"), nazov);
        std::fs::read(&cesta).unwrap_or_else(|e| panic!("chýba vzorka {cesta}: {e}"))
    }

    const BUNKA_SIRKA: u32 = 100;
    const BUNKA_VYSKA: u32 = 100;

    #[test]
    fn sablony_su_normalizovane_na_sucet_jedna() {
        for (typ, sablona) in SABLONY.iter() {
            let sucet: f32 = sablona.iter().sum();
            assert!((sucet - 1.0).abs() < 1e-3, "{typ}: súčet histogramu {sucet}");
        }
    }

    #[test]
    fn je_trinast_sablon_presne_monster_paint_typy() {
        assert_eq!(SABLONY.len(), 13);
    }

    #[test]
    fn prekryv_identickych_histogramov_je_jeden() {
        let (_, sablona) = &SABLONY[0];
        assert!((prekryv(sablona, sablona) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn prekryv_cisto_disjunktnych_histogramov_je_nula() {
        let mut a = [0f32; VELKOST_HISTOGRAMU];
        let mut b = [0f32; VELKOST_HISTOGRAMU];
        a[0] = 1.0;
        b[1] = 1.0;
        assert_eq!(prekryv(&a, &b), 0.0);
    }

    #[test]
    fn histogram_prilis_malej_snimky_vracia_none() {
        assert_eq!(histogram(&[0u8; 10], 100, 100), None);
    }

    /// Fixtures sú skutočné výrezy 100×100 bunky s monster paintom, pre
    /// ktoré overená kalibrácia (viď doc-komentár modulu) potvrdila SPRÁVNU
    /// top-1 zhodu — nie náhodný výber zo 40 nameraných vzoriek, presne tie
    /// prípady, kde algoritmus reálne funguje. Pri 37,5 % celkovej presnosti
    /// NIE JE možné, aby tu fixture „náhodou vyšla" bez toho, aby sa
    /// histogram skutočne zhodoval — to by bola náhoda 1:13 opakovaná štyrikrát.
    #[test]
    fn rozpoznane_typy_na_potvrdenych_realnych_vzorkach() {
        let pripady = [
            ("canvas-bunka-monster-dwarf", "dwarf"),
            ("canvas-bunka-monster-pest", "pest"),
            ("canvas-bunka-monster-eumbra", "eumbra"),
            ("canvas-bunka-monster-ratlin", "ratlin"),
        ];
        for (nazov, ocakavany_typ) in pripady {
            let rgb = vzorka(nazov);
            let (typ, skore) = rozpoznaj_ikonu_z_vyrezu(&rgb, BUNKA_SIRKA, BUNKA_VYSKA).unwrap();
            assert_eq!(typ, ocakavany_typ, "{nazov}: skóre {skore}");
            assert!(skore > 0.0, "{nazov}: nulové skóre pri správnom type?");
        }
    }

    #[test]
    fn rozpoznaj_ikonu_nad_malou_snimkou_vracia_none() {
        let rgb = vec![0u8; 4 * 4 * 3];
        assert_eq!(rozpoznaj_ikonu(&rgb, 4, 4, 1, 1), None);
    }
}
