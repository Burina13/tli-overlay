//! Rozpoznanie obsahu JEDNEJ bunky platna Frozen Canvas z farby (TLI 59b2b,
//! bod 1 zo zadania `2026-10-20-0032-agent.md`) — rovnaký princíp ako
//! `canvas_detekcia.rs` (pomer jasu/sýtosti farby, žiadne OCR ani dekódovanie
//! obrázkov), ale aplikovaný PER BUNKA namiesto na celú obrazovku.
//!
//! **Tri vizuálne stavy, nie štyri.** Zadanie vymenúva „prázdna/vír",
//! „biela (voľná a dostupná)", „monštrum" a „zamrznutá/nedostupná" — vo
//! všetkých 468 reálnych bunkách z 13 screenshotov použitých na kalibráciu
//! (nižšie) sa ale ukázal len JEDEN vizuálne odlišný vzor pre „bez obsahu,
//! nedá sa sem hrať" (tmavý vír) — druhý, vizuálne ODLIŠNÝ „zamrznutý" stav sa
//! v dostupných dátach nenašiel (buď neexistuje, alebo sa ešte nenazbieral).
//! `Nedostupna` preto pokrýva oba pomenovania zo zadania; ak sa raz ukáže
//! vizuálne odlišná „zamrznutá" bunka, pridá sa tretia vetva.
//!
//! **Čo `dostupna` v `Bunka` (server/Gemini) NIE JE:** pri kalibrácii sa
//! ukázalo, že ~3,4 % prázdnych buniek, ktoré vyzerajú úplne rovnako ako
//! dostupné (jasný štetec na svetlom pozadí), má v reálnych dátach
//! `dostupna: false` — to nie je šum merania, je to INÁ otázka, než na akú
//! táto funkcia vie odpovedať z pixelov jednej bunky: „je táto bunka práve
//! teraz podľa pravidiel susedstva legálna na položenie" je vlastnosť CELÉHO
//! plátna (rieši ju `CanvasStav::volne_susedne_bunky` v `canvas.rs`), nie
//! farba jedného výrezu. Táto funkcia preto odpovedá na užšiu, ale z jedného
//! obrázka skutočne zodpovedateľnú otázku: „je táto bunka vizuálne prázdna
//! časť AKTÍVNEJ (odomknutej) plochy, uzamknutá/nevyužiteľná plocha, alebo má
//! na sebe položený paint?".
//!
//! **Kalibrácia (žiadne odhadnuté prahy):** 468 bunky z 13 reálnych snímok
//! Frozen Canvasu (`/srv/games/tli/analyzy`, CT 114, Richardova session
//! 19.–20.10.2026), zoskupené podľa poľa `dostupna`/`paint.kategoria` zo
//! servera. Hľadaním najlepšej dvojice prahov (jas, sýtosť) nad celým touto
//! sadou vyšlo:
//!
//! | kategória | n | presnosť kaskády nižšie |
//! |---|---|---|
//! | `Nedostupna` (tmavý vír) | 319 | 307/319 (96,2 %) |
//! | `Prazdna` (svetlý štetec) | 109 | 90/109 (82,6 %) |
//! | `Namalovana` (farebný kruh + príšera) | 40 | 32/40 (80,0 %) |
//! | **spolu** | **468** | **429/468 (91,7 %)** |
//!
//! Chyby idú prevažne medzi `Nedostupna`↔`Prazdna` (16+11 z 27 chýb) — presne
//! tá hranica, kde „vizuálne prázdna" a „logicky dostupná" nie sú to isté,
//! viď vyššie. Medzi `Prazdna` a `Namalovana` je menej zámen (8+3), lebo
//! farebný kruh s príšerou má výrazne vyššiu sýtosť než hladké pozadie.
//!
//! **Gameplay paint (napr. `"brush"`) sa NEDÁ odlíšiť od prázdnej bunky touto
//! metódou** — jediná nameraná vzorka (`kategoria: "gameplay", typ: "brush"`)
//! mala identický vzhľad ako prázdna dostupná bunka (ten istý štetec-ikonka).
//! Táto funkcia preto také bunky vráti ako `Prazdna`, nie `Namalovana` —
//! zámerne, lebo „vyzerá to ako nič" je vernejší popis než hádaný `Namalovana`
//! s nezmyselným typom. Rozlíšenie gameplay vs. skutočne prázdna (ak to vôbec
//! bude niekedy treba) je mimo rozsahu tejto časti.

use crate::canvas_rozlozenie::platno_bunka_box01;
use crate::obraz::orez_box01;

/// Najväčší priemerný jas (0..255 cez R+G+B/3), pri ktorom sa bunka ešte
/// považuje za potenciálne `Nedostupna` — musí platiť SPOLU so
/// `PRAH_SYTOST_NEDOSTUPNA` (obe podmienky naraz, rovnaký vzor ako
/// `canvas_detekcia::je_canvas`).
pub const PRAH_JAS_NEDOSTUPNA: f64 = 56.0;
/// Najväčšia priemerná sýtosť (max(r,g,b) - min(r,g,b)), pri ktorej sa bunka
/// ešte považuje za `Nedostupna`.
pub const PRAH_SYTOST_NEDOSTUPNA: f64 = 26.0;
/// Najmenšia priemerná sýtosť, od ktorej sa bunka (ak už nie je `Nedostupna`)
/// považuje za `Namalovana` namiesto `Prazdna` — farebný kruh s príšerou má
/// výrazne pestrejšie farby než hladké pozadie prázdnej bunky.
pub const PRAH_SYTOST_NAMALOVANA: f64 = 41.0;

/// Čo je vizuálne v jednej bunke platna — bez typu/levelu paintu (to rieši
/// `canvas_sablony::rozpoznaj_ikonu` + OCR, samostatne, nad rovnakým výrezom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObsahBunky {
    /// Tmavý vír — bunka mimo aktívnej/odomknutej plochy platna.
    Nedostupna,
    /// Svetlé pozadie so štetcom — prázdne miesto v rámci aktívnej plochy.
    Prazdna,
    /// Farebný kruh s postavičkou a úrovňovým odznakom — položený paint.
    Namalovana,
}

/// Priemerný jas a sýtosť farby jedného výrezu (rovnaký štýl ako
/// `canvas_detekcia::Crty`, len bez vzorkovania na pevnú mriežku — bunka má
/// len ~100×100 px, prejsť všetky je lacnejšie než ďalšia vrstva vzorkovania).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrtyBunky {
    pub jas: f64,
    pub sytost: f64,
}

/// Spočíta `CrtyBunky` nad už vyrezaným RGB bufferom bunky (3 bajty/pixel).
pub fn crty_z_vyrezu(rgb: &[u8], sirka: u32, vyska: u32) -> Option<CrtyBunky> {
    let pocet_pixelov = (sirka as usize) * (vyska as usize);
    if sirka == 0 || vyska == 0 || rgb.len() < pocet_pixelov * 3 {
        return None;
    }
    let mut jas_sucet = 0.0f64;
    let mut sytost_sucet = 0.0f64;
    for i in 0..pocet_pixelov {
        let (r, g, b) = (rgb[i * 3] as i32, rgb[i * 3 + 1] as i32, rgb[i * 3 + 2] as i32);
        jas_sucet += (r + g + b) as f64 / 3.0;
        sytost_sucet += (r.max(g).max(b) - r.min(g).min(b)) as f64;
    }
    Some(CrtyBunky { jas: jas_sucet / pocet_pixelov as f64, sytost: sytost_sucet / pocet_pixelov as f64 })
}

/// Rozhodnutie podľa prahov vyššie — kaskáda, nie tri nezávislé podmienky:
/// najprv „vyzerá to ako vír" (jas aj sýtosť nízke), inak podľa sýtosti
/// `Namalovana`/`Prazdna`. Presne tento poradie vzišlo z merania (iné poradie
/// dávalo horšiu celkovú presnosť, hoci jednotlivé prahy sa nemenili).
impl CrtyBunky {
    pub fn obsah(&self) -> ObsahBunky {
        if self.jas < PRAH_JAS_NEDOSTUPNA && self.sytost < PRAH_SYTOST_NEDOSTUPNA {
            ObsahBunky::Nedostupna
        } else if self.sytost >= PRAH_SYTOST_NAMALOVANA {
            ObsahBunky::Namalovana
        } else {
            ObsahBunky::Prazdna
        }
    }
}

/// Vyreže bunku `riadok`/`stlpec` (1-indexované) z celej snímky (RGB, 3
/// bajty/pixel) podľa pevnej geometrie `canvas_rozlozenie` a rozhodne, čo je
/// v nej vizuálne vidieť. `None`, keď je snímka poškodená alebo by výrez
/// ležal mimo nej (rovnaké správanie ako `obraz::orez_box01`).
pub fn rozpoznaj_obsah_bunky(rgb: &[u8], sirka: u32, vyska: u32, riadok: i64, stlpec: i64) -> Option<ObsahBunky> {
    let oblast = platno_bunka_box01(riadok, stlpec);
    let (vyrez, w, h) = orez_box01(rgb, sirka, vyska, oblast)?;
    let crty = crty_z_vyrezu(&vyrez, w, h)?;
    Some(crty.obsah())
}

#[cfg(test)]
mod testy {
    use super::*;

    fn vzorka(nazov: &str) -> Vec<u8> {
        let cesta = format!("{}/testy-data/{}.rgb", env!("CARGO_MANIFEST_DIR"), nazov);
        std::fs::read(&cesta).unwrap_or_else(|e| panic!("chýba vzorka {cesta}: {e}"))
    }

    /// Fixtures sú skutočné výrezy 100×100 (bunka na 1920×1080 snímke) z
    /// `/srv/games/tli/analyzy`, nie downscalované — na rozdiel od
    /// `canvas_detekcia`, tu ide o rozlíšenie samotnej bunky, nie celej
    /// obrazovky.
    const BUNKA_SIRKA: u32 = 100;
    const BUNKA_VYSKA: u32 = 100;

    fn crty_vzorky(nazov: &str) -> CrtyBunky {
        crty_z_vyrezu(&vzorka(nazov), BUNKA_SIRKA, BUNKA_VYSKA).unwrap()
    }

    #[test]
    fn nedostupna_tmavy_vir_sa_rozpozna() {
        let c = crty_vzorky("canvas-bunka-nedostupna");
        assert_eq!(c.obsah(), ObsahBunky::Nedostupna, "{c:?}");
    }

    #[test]
    fn prazdna_svetly_stetec_sa_rozpozna() {
        let c = crty_vzorky("canvas-bunka-prazdna");
        assert_eq!(c.obsah(), ObsahBunky::Prazdna, "{c:?}");
    }

    #[test]
    fn namalovane_bunky_s_roznymi_typmi_sa_rozpoznaju() {
        // "canvas-bunka-monster-pest" sa tu ZÁMERNE nepoužíva: jej sýtosť
        // (28,4) je pod `PRAH_SYTOST_NAMALOVANA` (41,0) — presne jeden z
        // tých ~20 % prípadov z kalibrácie (8/40), kde farebný kruh okolo
        // slabo sýtej príšery (biela/ružová) sám osebe nestačí na odlíšenie
        // od prázdnej bunky. Šablóna matching (`canvas_sablony`) ju napriek
        // tomu správne rozpozná ako "pest" — tie dva moduly sa volajú
        // postupne, nie namiesto seba, a tento rozpor je zapísaný práve tu.
        for nazov in ["canvas-bunka-monster-dwarf", "canvas-bunka-monster-eumbra", "canvas-bunka-monster-ratlin"] {
            let c = crty_vzorky(nazov);
            assert_eq!(c.obsah(), ObsahBunky::Namalovana, "{nazov}: {c:?}");
        }
    }

    #[test]
    fn prilis_mala_snimka_vracia_none() {
        assert_eq!(crty_z_vyrezu(&[0u8; 10], 100, 100), None);
    }

    #[test]
    fn rozpoznaj_obsah_bunky_nad_celou_snimkou_mimo_platna_vracia_none() {
        // 4×4 "snímka" je príliš malá na to, aby sa do nej zmestil výrez
        // bunky v Box01 geometrii platna — `orez_box01` vráti `None`.
        let rgb = vec![0u8; 4 * 4 * 3];
        assert_eq!(rozpoznaj_obsah_bunky(&rgb, 4, 4, 1, 1), None);
    }
}
