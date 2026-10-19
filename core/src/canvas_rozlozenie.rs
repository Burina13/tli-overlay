//! Pevné normalizované (Box01) rozloženie Frozen Canvasu — TLI 59b, prvá
//! polovica kroku 1 zo zadania TLI 59 („Mriežka 6×6 na pevnom mieste
//! obrazovky — prepočítaj podľa rozlíšenia a DPI, reuse `suradnice.rs`").
//!
//! **Hodnoty NIE SÚ odhadnuté.** Sú to priemery zo 14 reálnych rozpoznaní
//! modulu `canvas` (Gemini vision, polia `polia_platno`/`polia_ruky`) z
//! `/srv/games/tli/analyzy` na CT 114 — 12 malo platno, 6 z nich aj ruku
//! (Richardova hracia session 19.10.2026, ~20:40 a ~21:3x). Smerodajná
//! odchýlka počiatku mriežky bola len ~0,0006–0,0007 (Box01, teda na 1920 px
//! šírky okna rádovo ±1 px) — Frozen Canvas UI teda sedí na PEVNOM mieste
//! vzhľadom na okno hry, presne ako zadanie predpokladalo, a dá sa odvodiť
//! vzorcom (počiatok + pitch), nie 36+11 magickými číslami zvlášť.
//!
//! **⚠️ LIMIT, ktorý treba vedieť pred nasadením:** zmerané na JEDNOM
//! rozlíšení/okne (Richardovo, 19.10.2026). Box01 je normalizované na okno
//! hry (`suradnice.rs`), takže by to malo platiť aj pri iných rozlíšeniach a
//! DPI, AK hra škáluje Frozen Canvas UI proporcionálne k oknu (bežný prípad
//! u modálnych dialógov) — to ale zatiaľ NIE JE overené na inom rozlíšení.
//! Kým appka nezbiera snímky z iného rozlíšenia (alebo kým to Richard
//! nepotvrdí), berie sa to ako pracovná hypotéza, nie istota — presne preto
//! toto zostáva LEN geometria (kam sa pozrieť), nie samotné rozpoznanie
//! (čo tam je) — to rieši až OCR/template matching na tejto geometrii.
//!
//! Poradie polí v `platno_box01()` je zhodné s `polia_platno` zo servera aj
//! s row-major poradím `CanvasStav::bunky` (`canvas.rs`): riadok/stĺpec
//! 1-indexované, `index = (riadok-1)*6 + (stlpec-1)`.

use crate::suradnice::Pole;

pub const PLATNO_RIADKY: i64 = 6;
pub const PLATNO_STLPCE: i64 = 6;

const PLATNO_ORIGIN_X: f64 = 0.1857;
const PLATNO_ORIGIN_Y: f64 = 0.2517;
const PLATNO_CELL_W: f64 = 0.0520;
const PLATNO_CELL_H: f64 = 0.0926;
const PLATNO_COL_PITCH: f64 = 0.0521;
const PLATNO_ROW_PITCH: f64 = 0.0926;

/// Ruka má v nameraných snímkach najviac 11 viditeľných slotov, 5 na riadok
/// (posledný riadok neúplný) — `ruka_box01` berie skutočný počet z appky.
const RUKA_SLOTOV_V_RIADKU: i64 = 5;
const RUKA_ORIGIN_X: f64 = 0.5362;
const RUKA_ORIGIN_Y: f64 = 0.4140;
const RUKA_CELL_W: f64 = 0.0540;
const RUKA_CELL_H: f64 = 0.0958;
const RUKA_COL_PITCH: f64 = 0.0597;
const RUKA_ROW_PITCH: f64 = 0.1017;

/// Box01 obdĺžnik bunky platna pre `riadok`/`stlpec` (1-indexované, rovnaká
/// konvencia ako `Bunka` v `canvas.rs`). Mimo rozsahu 1..=6 vráti geometriu
/// ďalej podľa vzorca (nekontroluje hranice) — volajúci v `src-tauri/` má
/// vlastný zdroj pravdy o tom, že platno má 6×6 polí (`CanvasStav.riadky/
/// stlpce`), tu sa to druhýkrát nevaliduje.
pub fn platno_bunka_box01(riadok: i64, stlpec: i64) -> Pole {
    let r = (riadok - 1) as f64;
    let s = (stlpec - 1) as f64;
    Pole {
        index: ((riadok - 1) * PLATNO_STLPCE + (stlpec - 1)) as u32,
        x: PLATNO_ORIGIN_X + s * PLATNO_COL_PITCH,
        y: PLATNO_ORIGIN_Y + r * PLATNO_ROW_PITCH,
        w: PLATNO_CELL_W,
        h: PLATNO_CELL_H,
    }
}

/// Všetkých 36 políčok platna, row-major (rovnaké poradie ako `polia_platno`
/// zo servera aj `CanvasStav::bunky`).
pub fn platno_box01() -> Vec<Pole> {
    let mut v = Vec::with_capacity((PLATNO_RIADKY * PLATNO_STLPCE) as usize);
    for r in 1..=PLATNO_RIADKY {
        for s in 1..=PLATNO_STLPCE {
            v.push(platno_bunka_box01(r, s));
        }
    }
    v
}

/// Box01 slotu ruky podľa indexu (0-indexované, row-major, `SlotRuky::index`
/// z `canvas.rs` má rovnaké číslovanie).
pub fn ruka_slot_box01(index: i64) -> Pole {
    let r = (index.div_euclid(RUKA_SLOTOV_V_RIADKU)) as f64;
    let s = (index.rem_euclid(RUKA_SLOTOV_V_RIADKU)) as f64;
    Pole {
        index: index.max(0) as u32,
        x: RUKA_ORIGIN_X + s * RUKA_COL_PITCH,
        y: RUKA_ORIGIN_Y + r * RUKA_ROW_PITCH,
        w: RUKA_CELL_W,
        h: RUKA_CELL_H,
    }
}

/// `pocet` slotov ruky, tak ako ich appka vidí reálne obsadené/dostupné
/// (server pri `modul=canvas` posiela rovnako premenlivý počet — 10 alebo 11
/// v nameraných dátach, nikdy pevných 36 ako pri platne).
pub fn ruka_box01(pocet: i64) -> Vec<Pole> {
    (0..pocet.max(0)).map(ruka_slot_box01).collect()
}

#[cfg(test)]
mod testy {
    use super::*;

    fn takmer(a: f64, b: f64, tolerancia: f64) -> bool {
        (a - b).abs() < tolerancia
    }

    #[test]
    fn platno_box01_ma_36_policok_v_spravnom_poradi() {
        let v = platno_box01();
        assert_eq!(v.len(), 36);
        assert_eq!(v[0].index, 0);
        assert_eq!(v[35].index, 35);
        // row-major: index 6 je prvá bunka DRUHÉHO riadku (stĺpec sa vráti na 1).
        assert_eq!(v[6].index, 6);
        assert!(takmer(v[6].x, PLATNO_ORIGIN_X, 1e-9));
    }

    /// Priemer z meraní, 19.10.2026 (`/srv/games/tli/analyzy`, 12 vzoriek):
    /// bunka r1c1 mala x v rozsahu 0,185–0,187, y v rozsahu 0,250–0,252.
    #[test]
    fn platno_bunka_r1c1_zodpoveda_zmeranym_datam() {
        let p = platno_bunka_box01(1, 1);
        assert!(takmer(p.x, 0.186, 0.0015), "x={}", p.x);
        assert!(takmer(p.y, 0.251, 0.0015), "y={}", p.y);
    }

    /// Posledná bunka (riadok 6, stĺpec 6) — zmerané x≈0,445–0,446, y≈0,710–0,717.
    #[test]
    fn platno_bunka_r6c6_zodpoveda_zmeranym_datam() {
        let p = platno_bunka_box01(6, 6);
        assert!(takmer(p.x, 0.4455, 0.0015), "x={}", p.x);
        assert!(takmer(p.y, 0.7135, 0.004), "y={}", p.y);
    }

    /// Pitch musí byť aspoň taký ako rozmer bunky — inak by sa susedné bunky
    /// prekrývali a vzorka farby/ikony by sa trafila do zlého políčka.
    #[test]
    fn platno_bunky_sa_neprekryvaju() {
        assert!(PLATNO_COL_PITCH >= PLATNO_CELL_W - 1e-9);
        assert!(PLATNO_ROW_PITCH >= PLATNO_CELL_H - 1e-9);
    }

    #[test]
    fn ruka_box01_vrati_pozadovany_pocet_v_row_major_poradi() {
        let v = ruka_box01(7);
        assert_eq!(v.len(), 7);
        assert_eq!(v[5].index, 5);
        // 6. slot (index 5) je prvý v DRUHOM riadku ruky.
        assert!(takmer(v[5].y, RUKA_ORIGIN_Y + RUKA_ROW_PITCH, 1e-9));
        assert!(takmer(v[5].x, RUKA_ORIGIN_X, 1e-9));
    }

    #[test]
    fn ruka_box01_s_nulou_je_prazdny() {
        assert!(ruka_box01(0).is_empty());
    }

    /// Priemer z meraní (6 vzoriek s rukou): slot 0 x≈0,536, y≈0,413–0,415.
    #[test]
    fn ruka_slot0_zodpoveda_zmeranym_datam() {
        let p = ruka_slot_box01(0);
        assert!(takmer(p.x, 0.5365, 0.0015), "x={}", p.x);
        assert!(takmer(p.y, 0.414, 0.0015), "y={}", p.y);
    }

    #[test]
    fn ruka_bunky_sa_neprekryvaju() {
        assert!(RUKA_COL_PITCH >= RUKA_CELL_W - 1e-9);
        assert!(RUKA_ROW_PITCH >= RUKA_CELL_H - 1e-9);
    }
}
