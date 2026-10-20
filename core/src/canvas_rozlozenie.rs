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

// ── Podobrazovka s kartami (TLI 71, `2026-10-20-1635-mcp.md`) ───────────────
//
// „Select the Brushstroke direction" aj „Select Brushstroke Effect" majú
// ROVNAKÉ rozloženie: nadpis, pod ním zvislý stĺpec kariet a vpravo dole
// tlačidlo „Select". Zmerané z Richardových snímok 1920×1080 z 20.10.2026
// (`2026-10-20-163311`, `-163430`, `-145835`, `-134614`) — na všetkých
// štyroch sedia tie isté hodnoty na jednotky pixelov.
//
//   nadpis            y ≈ 425–445
//   karta 0           x 1040–1600, y 448–547
//   karta 1           x 1040–1600, y 560–660   (krok 112 px)
//   tlačidlo Select   x 1393–1627, y 813–870
//
// ⚠️ Rovnaký LIMIT ako pri mriežke vyššie: zmerané na JEDNOM rozlíšení.
// Zrkadlí `canvas_rozlozenie.py` na serveri — obe strany musia dávať tie isté
// čísla, inak by appka kreslila rámik inam, než server myslí.

const KARTA_ORIGIN_X: f64 = 0.5417;
const KARTA_ORIGIN_Y: f64 = 0.4148;
const KARTA_W: f64 = 0.2917;
const KARTA_H: f64 = 0.0926;
const KARTA_ROW_PITCH: f64 = 0.1037;

/// Obálka celej podobrazovky s kartami — nadpis aj všetky tri karty.
///
/// ⚠️ **Appka toto NEOCRuje.** Číta nadpis a KAŽDÚ KARTU zvlášť
/// (`nadpis_kariet_box01`, `karta_box01`), lebo hra text karty láme na dva
/// riadky a zo spoločného OCR by sa nedalo určiť, kde jedna karta končí.
/// Tento box je spoločná obálka na kontrolu (že všetko leží v okne) a pre
/// prípadné rýchle odmietnutie oblasti. Výrez MUSÍ obsahovať aj poslednú
/// kartu — do opravy z 20.10.2026 končil na 0,63 a tretia karta (do 0,7148)
/// z neho vytŕčala, takže komentár sľuboval viac, než box pokrýval.
const OCR_KARTY_X: f64 = 0.53;
const OCR_KARTY_Y: f64 = 0.385;
const OCR_KARTY_W: f64 = 0.32;
const OCR_KARTY_H: f64 = 0.335;

/// Nadpis podobrazovky: y ≈ 420–446 px pri 1080 (zmerané na tých istých
/// štyroch snímkach ako karty).
const NADPIS_Y: f64 = 0.3889;
const NADPIS_H: f64 = 0.0241;

const TLACIDLO_SELECT_X: f64 = 0.7255;
const TLACIDLO_SELECT_Y: f64 = 0.7528;
const TLACIDLO_SELECT_W: f64 = 0.1219;
const TLACIDLO_SELECT_H: f64 = 0.0528;

/// Box01 obdĺžnik karty podľa poradia zhora (0-indexované).
pub fn karta_box01(index: i64) -> Pole {
    let i = index.max(0);
    Pole {
        index: i as u32,
        x: KARTA_ORIGIN_X,
        y: KARTA_ORIGIN_Y + (i as f64) * KARTA_ROW_PITCH,
        w: KARTA_W,
        h: KARTA_H,
    }
}

/// `pocet` kariet zhora nadol.
pub fn karty_box01(pocet: i64) -> Vec<Pole> {
    (0..pocet.max(0)).map(karta_box01).collect()
}

/// Box01 tlačidla „Select", ktorým sa vybraná karta potvrdzuje.
pub fn tlacidlo_select_box01() -> Pole {
    Pole { index: 0, x: TLACIDLO_SELECT_X, y: TLACIDLO_SELECT_Y,
           w: TLACIDLO_SELECT_W, h: TLACIDLO_SELECT_H }
}

/// Box01 obálky celej podobrazovky (nadpis + všetky karty) — pozri komentár
/// pri `OCR_KARTY_*`, appka ju samu neOCRuje.
pub fn ocr_oblast_kariet_box01() -> Pole {
    Pole { index: 0, x: OCR_KARTY_X, y: OCR_KARTY_Y, w: OCR_KARTY_W, h: OCR_KARTY_H }
}

/// Box01 samotného NADPISU podobrazovky („Select the Brushstroke direction"
/// / „Select Brushstroke Effect"). OCRuje sa zvlášť od kariet, aby sa text
/// nadpisu nezliepal s textom prvej karty — podľa nadpisu sa rozhoduje, či
/// vôbec ide o podobrazovku s výberom.
pub fn nadpis_kariet_box01() -> Pole {
    Pole { index: 0, x: KARTA_ORIGIN_X, y: NADPIS_Y, w: KARTA_W, h: NADPIS_H }
}

// ── Koniec behu: „All paint has been added" (TLI 72) ────────────────────────
//
// Keď hráč položí posledný paint, hra cez plátno vypíše banner „All paint has
// been added" a vpravo dole ponúkne „Start Over" a „Complete". Počítadlo
// „20/20" vedľa ruky je jediný lokálne čitateľný údaj o tom, koľko paintov do
// konca behu zostáva — solver z neho vie povedať, že cieľ sa už nestihne
// dokončiť a oplatí sa skôr prepísať. Zmerané na 1920×1080, rovnaký LIMIT
// ako pri mriežke a kartách vyššie. Zrkadlí `canvas_rozlozenie.py` na serveri.
//
//   banner            x ≈ 1017–1600, y ≈ 272–312
//   počítadlo         x ≈ 1536–1642, y ≈ 783–826
//   tlačidlo Start Over x ≈ 1187–1388, y ≈ 810–891
//   tlačidlo Complete leží presne tam, kde inak „Select"

const BANNER_X: f64 = 0.5297;
const BANNER_Y: f64 = 0.2519;
const BANNER_W: f64 = 0.3036;
const BANNER_H: f64 = 0.0370;

const POCITADLO_X: f64 = 0.8000;
const POCITADLO_Y: f64 = 0.7250;
const POCITADLO_W: f64 = 0.0550;
const POCITADLO_H: f64 = 0.0400;

const TLACIDLO_START_OVER_X: f64 = 0.6180;
const TLACIDLO_START_OVER_Y: f64 = 0.7500;
const TLACIDLO_START_OVER_W: f64 = 0.1050;
const TLACIDLO_START_OVER_H: f64 = 0.0750;

/// Box01 bannera „All paint has been added" — OCRuje sa zvlášť, aby sa jeho
/// text nezlial s textom cieľa, ktorý hra pod ním ešte nechá.
pub fn banner_box01() -> Pole {
    Pole { index: 0, x: BANNER_X, y: BANNER_Y, w: BANNER_W, h: BANNER_H }
}

/// Box01 počítadla „položené/celkom" (`canvas_ocr::parsuj_pocitadlo`).
pub fn pocitadlo_box01() -> Pole {
    Pole { index: 0, x: POCITADLO_X, y: POCITADLO_Y, w: POCITADLO_W, h: POCITADLO_H }
}

/// Box01 tlačidla „Start Over" na konci behu.
pub fn tlacidlo_start_over_box01() -> Pole {
    Pole { index: 0, x: TLACIDLO_START_OVER_X, y: TLACIDLO_START_OVER_Y,
           w: TLACIDLO_START_OVER_W, h: TLACIDLO_START_OVER_H }
}

/// Box01 tlačidla „Complete". Hra ho kreslí na TO ISTÉ miesto ako „Select"
/// z podobrazovky s kartami — preto ZÁMERNE tie isté konštanty, nie kópia
/// čísel, ktorá by sa pri ďalšom premeraní mohla rozísť.
pub fn tlacidlo_complete_box01() -> Pole {
    Pole { index: 0, x: TLACIDLO_SELECT_X, y: TLACIDLO_SELECT_Y,
           w: TLACIDLO_SELECT_W, h: TLACIDLO_SELECT_H }
}

#[cfg(test)]
mod testy_koniec_behu {
    use super::*;

    fn prekryvaju_sa(a: &Pole, b: &Pole) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    #[test]
    fn vsetky_boxy_konca_behu_lezia_v_obrazku() {
        for p in [banner_box01(), pocitadlo_box01(), tlacidlo_start_over_box01(),
                  tlacidlo_complete_box01()] {
            assert!(p.x >= 0.0 && p.x + p.w <= 1.0, "{p:?}");
            assert!(p.y >= 0.0 && p.y + p.h <= 1.0, "{p:?}");
            assert!(p.w > 0.0 && p.h > 0.0, "{p:?}");
        }
    }

    /// Hodnoty musia sedieť s `canvas_rozlozenie.py` na serveri — inak by
    /// appka čítala počítadlo alebo banner z iného miesta, než server myslí.
    #[test]
    fn hodnoty_zodpovedaju_serveru() {
        let b = banner_box01();
        assert_eq!((b.x, b.y, b.w, b.h), (0.5297, 0.2519, 0.3036, 0.0370));
        let p = pocitadlo_box01();
        assert_eq!((p.x, p.y, p.w, p.h), (0.8000, 0.7250, 0.0550, 0.0400));
        let s = tlacidlo_start_over_box01();
        assert_eq!((s.x, s.y, s.w, s.h), (0.6180, 0.7500, 0.1050, 0.0750));
    }

    /// „Complete" leží presne na mieste „Select" — keby sa niekto jedno
    /// premeral a druhé nie, tento test na to upozorní.
    #[test]
    fn complete_je_presne_na_mieste_select() {
        assert_eq!(tlacidlo_complete_box01(), tlacidlo_select_box01());
    }

    /// Dve tlačidlá konca behu sú vedľa seba; keby sa prekrývali, klik/rámik
    /// na „Start Over" by mohol trafiť „Complete" a beh ukončiť.
    #[test]
    fn start_over_a_complete_sa_neprekryvaju() {
        let s = tlacidlo_start_over_box01();
        let c = tlacidlo_complete_box01();
        assert!(!prekryvaju_sa(&s, &c), "{s:?} {c:?}");
        assert!(s.x + s.w <= c.x, "Start Over má byť vľavo od Complete");
    }

    #[test]
    fn banner_je_nad_kartami_aj_nad_tlacidlami() {
        let b = banner_box01();
        assert!(b.y + b.h <= nadpis_kariet_box01().y, "{b:?}");
        assert!(b.y + b.h <= tlacidlo_start_over_box01().y);
    }
}

#[cfg(test)]
mod testy_karty {
    use super::*;

    #[test]
    fn karty_su_pod_sebou_a_neprekryvaju_sa() {
        let a = karta_box01(0);
        let b = karta_box01(1);
        assert!(b.y > a.y + a.h - 1e-9, "{a:?} {b:?}");
        assert_eq!(a.x, b.x);
        assert_eq!(a.w, b.w);
    }

    #[test]
    fn karty_a_tlacidlo_lezia_v_obrazku() {
        for p in [karta_box01(0), karta_box01(2), tlacidlo_select_box01(),
                  ocr_oblast_kariet_box01()] {
            assert!(p.x >= 0.0 && p.x + p.w <= 1.0, "{p:?}");
            assert!(p.y >= 0.0 && p.y + p.h <= 1.0, "{p:?}");
        }
    }

    #[test]
    fn ocr_vyrez_obsahuje_vsetky_karty_aj_nadpis() {
        let o = ocr_oblast_kariet_box01();
        let n = nadpis_kariet_box01();
        assert!(o.y <= n.y && n.y + n.h <= o.y + o.h + 1e-9, "{o:?} {n:?}");
        for i in 0..crate::canvas_tahy::MAX_KARIET as i64 {
            let k = karta_box01(i);
            assert!(o.x <= k.x && k.x + k.w <= o.x + o.w + 1e-9, "{o:?} {k:?}");
            assert!(o.y <= k.y && k.y + k.h <= o.y + o.h + 1e-9, "{o:?} {k:?}");
        }
    }

    /// Nález z review diffu cez API kredit (20.10.2026): štvrtý box karty
    /// ležal NA tlačidle „Select" a appka by z neho prečítala slovo „Select"
    /// ako neexistujúcu štvrtú kartu — a pri zhode skóre by ju aj odporučila,
    /// teda orámovala by tlačidlo s textom „✓ vyber túto".
    #[test]
    fn ziadny_box_karty_neprekryva_tlacidlo() {
        let t = tlacidlo_select_box01();
        for i in 0..crate::canvas_tahy::MAX_KARIET as i64 {
            let k = karta_box01(i);
            let prekryv_x = k.x < t.x + t.w && t.x < k.x + k.w;
            let prekryv_y = k.y < t.y + t.h && t.y < k.y + k.h;
            assert!(!(prekryv_x && prekryv_y), "karta {i} prekrýva tlačidlo: {k:?} {t:?}");
        }
    }

    #[test]
    fn tlacidlo_je_pod_kartami() {
        let t = tlacidlo_select_box01();
        let k = karta_box01(1);
        assert!(t.y > k.y + k.h);
    }

    /// Hodnoty musia sedieť s `canvas_rozlozenie.py` na serveri — keby sa
    /// rozišli, appka by orámovala inú kartu, než ktorú server odporučil.
    #[test]
    fn hodnoty_zodpovedaju_serveru() {
        let k = karta_box01(1);
        assert!((k.x - 0.5417).abs() < 1e-9);
        assert!((k.y - 0.5185).abs() < 1e-9, "{}", k.y);
        assert!((k.w - 0.2917).abs() < 1e-9);
        assert!((k.h - 0.0926).abs() < 1e-9);
        let t = tlacidlo_select_box01();
        assert!((t.x - 0.7255).abs() < 1e-9);
        assert!((t.y - 0.7528).abs() < 1e-9);
    }

    #[test]
    fn nadpis_je_nad_prvou_kartou_a_neprekryva_ju() {
        let n = nadpis_kariet_box01();
        let k = karta_box01(0);
        assert!(n.y + n.h <= k.y + 1e-9, "{n:?} {k:?}");
        assert_eq!(n.x, k.x);
        let o = ocr_oblast_kariet_box01();
        assert!(o.y <= n.y && n.y + n.h <= o.y + o.h, "{o:?} {n:?}");
    }

    #[test]
    fn karty_box01_vrati_presny_pocet() {
        assert!(karty_box01(0).is_empty());
        assert_eq!(karty_box01(3).len(), 3);
        assert_eq!(karty_box01(3).iter().map(|p| p.index).collect::<Vec<_>>(), vec![0, 1, 2]);
    }
}
