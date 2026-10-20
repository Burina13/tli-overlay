//! Lokálne rozpoznanie ŽLTÉHO RÁMU, ktorým hra sama označuje platné cieľové
//! bunky Frozen Canvasu (TLI 69, úloha `2026-10-20-1332-mcp.md`).
//!
//! **Prečo to vzniklo.** Do 20.10.2026 appka vedela poradiť len pri smere
//! `adjacent` a pri každom inom (`vertical`, `horizontal`, `diagonal`) napísala
//! „žiadny krok — smer nemá overené pravidlo". Richard na snímke ukázal, že
//! hra platné bunky **sama orámuje**: pri cieli „Fill a Vertical Cell…" so
//! zdrojom v r6c3 boli žlto orámované presne r1c3, r2c3, r4c3 a r5c3 (r3c3 je
//! ľad). Dosah teda netreba odvodzovať z názvu smeru — stačí prečítať rám,
//! a potom je pravidlo pre VŠETKY smery to isté. Text smeru slúži už len na
//! kontrolu.
//!
//! **Prečo lokálne a nie na serveri.** Appka má snímku v plnom rozlíšení,
//! serveru chodí zmenšený JPEG; rám je pritom čiara hrubá ~4 px. Navyše je to
//! jeden prechod cez pixely bez OCR a bez vision modelu, takže sa to dá
//! spraviť pri každej analýze zadarmo. Výsledok (zoznam buniek) posiela appka
//! serveru v parametri `ramy`, kde ho solver použije ako dosah.
//!
//! ## Ako to funguje
//!
//! Dve oddelené vrstvy, nech sa dá každá testovať zvlášť:
//!
//! 1. [`Maska`] — per-pixel test „je tento bod jantárovo žltý?" ([`je_zlty`]).
//! 2. [`oramovane_bunky_z_masky`] — čistá geometria nad pevnou mriežkou 6×6
//!    (`canvas_rozlozenie`): pre každú bunku sa v okolí jej štyroch hraníc
//!    hľadá čiara rámu.
//!
//! ## Dve veci, ktoré robia rozdiel medzi „funguje" a „nefunguje"
//!
//! **(a) Rám suseda vyzerá ako vlastný.** Bunky mriežky na seba doliehajú
//! (rozostup 100,03 px, šírka 99,8 px) a rám sa kreslí tesne VNÚTRI bunky,
//! takže čiara suseda leží pár pixelov za našou hranicou — v okne hľadania.
//! Preto sa z kandidátov s porovnateľným pokrytím berie ten **bližšie k vnútru**
//! bunky (`DNU_*`), nie ten s najvyšším pokrytím.
//!
//! **(b) Zdrojová bunka (so značkou „!") orámovaná NIE JE, ale pri smere
//! `adjacent` ju obkolesujú orámovaní susedia zo všetkých štyroch strán** —
//! takže všetky štyri jej „hrany" sú žlté a podľa pokrytia by vyšla ako
//! orámovaná. Rozlíši ju až ROZMER nájdeného obdĺžnika: vlastný rám je
//! o niečo MENŠÍ než bunka (namerané 91–99 px), zlepenina zo štyroch susedov
//! je väčšia (105–107 px). Preto `sirka <= cw + TOLERANCIA_ROZMERU`.
//!
//! ## Kalibrácia — žiadne odhadnuté prahy
//!
//! 75 unikátnych reálnych snímok Frozen Canvasu z `/srv/games/tli/analyzy`
//! (CT 114, Richardovo hranie 19.–20.10.2026), vyhodnotených touto istou
//! logikou:
//!
//! | | výsledok |
//! |---|---|
//! | snímok, kde hra rám kreslila | 30 |
//! | z toho zhoda tvaru rámu s menom smeru z textu hry | **27/27** (adjacent 10×, horizontal 10×, vertical 4×, diagonal 3×; zvyšné 3 nemali rozpoznaný text) |
//! | snímok bez rámu | 45 — **všetky** z podobrazoviek „Select the Brushstroke direction" (37) a „Draw" (8), kde ho hra nekreslí |
//! | falošne kladné | 0 |
//!
//! Namerané rozpätia (2 700 vyhodnotených buniek):
//!
//! | veličina | orámované (109) | neorámované (2 591) |
//! |---|---|---|
//! | `pokrytie` (min zo 4 hrán) | 0,161–1,0 (medián 0,25) | 0–0,196 |
//! | šírka/výška nájdeného rámu | 91–99 px | 105–107 px (zlepeniny zo susedov) |
//!
//! ⚠️ **Samotné `pokrytie` nestačí** — rozpätia sa prekrývajú (0,161 vs 0,196).
//! Oddeľuje ich až rozmer; práve tých 5 buniek s pokrytím 0,196 sú zdrojové
//! bunky z bodu (b). Keď raz niekto bude prahy ladiť, toto je to miesto, kde
//! sa dá všetko pokaziť naraz.
//!
//! Rám je na bunke **aj keď je už obsadená** — po položení paintu na nej ostáva
//! (pár snímok `2026-10-20-151222` „0/3" → `-151242` „2/3"). „Kam sa dá položiť"
//! je preto `oramovana && dostupna && paint == None`, nie len `oramovana`;
//! rieši to solver (`canvas_solver.rs`), nie tento modul.

use crate::canvas_rozlozenie::{PLATNO_RIADKY, PLATNO_STLPCE};
use crate::canvas_rozlozenie::platno_bunka_box01;

/// O koľko px (pri bunke 100 px) sa okolo vypočítanej hranice bunky hľadá
/// čiara rámu. Mriežka je odmeraná ako priemer zo 14 rozpoznaní, takže na
/// konkrétnej snímke môže sedieť o pár pixelov vedľa.
pub const TOLERANCIA_HLADANIA: i64 = 7;
/// Koľko px pri rohoch bunky sa z hrany vynechá — roh patrí dvom hranám naraz
/// a kazil by meranie oboch.
pub const ROH: i64 = 22;
/// Najmenšie pokrytie (podiel riadkov/stĺpcov hrany, na ktorých čiara je),
/// ktoré musí mať KAŽDÁ zo štyroch hrán. Namerané: orámované ≥ 0,161.
pub const PRAH_POKRYTIA: f64 = 0.10;
/// Kandidát na čiaru sa uzná, keď má aspoň tento podiel pokrytia najlepšieho
/// kandidáta tej istej hrany — a z uznaných sa berie ten bližšie k vnútru.
pub const PODIEL_KANDIDATA: f64 = 0.6;
/// O koľko px smie byť nájdený obdĺžnik väčší než samotná bunka, kým sa ešte
/// berie ako jej vlastný rám. Namerané: vlastný ≤ 99, zlepenina ≥ 105.
pub const TOLERANCIA_ROZMERU: i64 = 2;
/// Najväčší podiel žltých bodov VNÚTRI bunky (stredné dve štvrtiny), pri
/// ktorom sa to ešte berie ako rám. Rám je tenký prstenec — keď je žltá aj
/// výplň, nie je to rám, ale veľká jantárová plocha (panel hry, animácia).
/// Namerané: orámované bunky ≤ 0,184; prah je zámerne hlboko nad tým.
pub const PRAH_VNUTRA: f64 = 0.50;

/// Je bod jantárovo žltý, ako hra kreslí rám? Namerané priamo z rámov na
/// reálnych snímkach: typicky okolo `(247, 185, 62)`, s prechodom do svetlejšej
/// `(255, 245, 72)` — rám má gradient, preto je test voľný v jase a prísny
/// v tom, čo ho robí žltým (červená aj zelená výrazne nad modrou).
pub fn je_zlty(r: u8, g: u8, b: u8) -> bool {
    let mn = r.min(g) as i32;
    mn >= 120 && (mn - b as i32) >= 45 && r >= g
}

/// Bitová maska „žltých" bodov snímky. Bit na pozícii `y * sirka + x`.
/// 1920×1080 zaberie 259 kB — rádovo menej než samotná snímka (6,2 MB RGB),
/// a geometria nad ňou pracuje bez ďalšieho prechodu cez farby.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Maska {
    pub sirka: u32,
    pub vyska: u32,
    bity: Vec<u64>,
}

impl Maska {
    pub fn nova(sirka: u32, vyska: u32) -> Self {
        let pocet = (sirka as usize) * (vyska as usize);
        Self { sirka, vyska, bity: vec![0u64; pocet.div_ceil(64)] }
    }

    /// Maska z celej snímky (RGB, 3 bajty/pixel, zhora nadol). Pri poškodenom
    /// alebo prikrátkom poli vráti prázdnu masku — rovnako ako
    /// `canvas_detekcia::crty`, teda „nič som nenašiel", nie panika.
    pub fn z_rgb(rgb: &[u8], sirka: u32, vyska: u32) -> Self {
        let mut m = Self::nova(sirka, vyska);
        let pocet = (sirka as usize) * (vyska as usize);
        if sirka == 0 || vyska == 0 || rgb.len() < pocet * 3 {
            return Self::nova(0, 0);
        }
        for i in 0..pocet {
            if je_zlty(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]) {
                m.bity[i / 64] |= 1u64 << (i % 64);
            }
        }
        m
    }

    #[inline]
    pub fn ma(&self, x: i64, y: i64) -> bool {
        if x < 0 || y < 0 || x >= self.sirka as i64 || y >= self.vyska as i64 {
            return false;
        }
        let i = (y as usize) * (self.sirka as usize) + (x as usize);
        self.bity[i / 64] & (1u64 << (i % 64)) != 0
    }

    pub fn nastav(&mut self, x: i64, y: i64) {
        if x < 0 || y < 0 || x >= self.sirka as i64 || y >= self.vyska as i64 {
            return;
        }
        let i = (y as usize) * (self.sirka as usize) + (x as usize);
        self.bity[i / 64] |= 1u64 << (i % 64);
    }

    pub fn pocet_bodov(&self) -> u32 {
        self.bity.iter().map(|w| w.count_ones()).sum()
    }
}

/// Čo sa o ráme jednej bunky nameralo. Vracia sa celé (nielen `bool`), aby sa
/// to dalo zapísať do denníka appky — keď detekcia niekedy u hráča zlyhá, z
/// logu je vidno, ktorá z podmienok netrafila, bez posielania obrázka.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NalezRamu {
    pub riadok: i64,
    pub stlpec: i64,
    /// Najslabšia zo štyroch hrán (0..1).
    pub pokrytie: f64,
    /// Šírka nájdeného obdĺžnika v px a šírka samotnej bunky.
    pub sirka: i64,
    pub vyska: i64,
    pub sirka_bunky: i64,
    pub vyska_bunky: i64,
    /// Podiel žltých bodov vnútri bunky (stredné dve štvrtiny).
    pub vnutro: f64,
}

impl NalezRamu {
    /// Je to skutočne rám TEJTO bunky? Obe podmienky musia platiť naraz —
    /// prečo, je v hlavičke modulu, bod (b).
    pub fn je_ramovana(&self) -> bool {
        self.pokrytie >= PRAH_POKRYTIA
            && self.sirka <= self.sirka_bunky + TOLERANCIA_ROZMERU
            && self.vyska <= self.vyska_bunky + TOLERANCIA_ROZMERU
            && self.vnutro <= PRAH_VNUTRA
    }
}

/// Nájde v rozsahu `od..=do_` pozíciu čiary rámu. `dnu` je `+1`, keď vnútro
/// bunky leží smerom k vyšším súradniciam (ľavá/horná hrana), `-1` pre pravú
/// a dolnú. Vracia `(pozicia, pokrytie)`.
fn najdi_ciaru(
    m: &Maska,
    fix0: i64,
    fix1: i64,
    od: i64,
    do_: i64,
    zvisla: bool,
    dnu: i64,
) -> (i64, f64) {
    let dlzka = fix1 - fix0;
    if dlzka <= 0 {
        return (-1, 0.0);
    }
    let mut najlepsie = 0.0f64;
    let mut pokrytia: Vec<(i64, f64)> = Vec::with_capacity((do_ - od + 1).max(0) as usize);
    for p in od..=do_ {
        let mut zasiahnute = 0i64;
        for q in fix0..fix1 {
            let ma = if zvisla { m.ma(p, q) } else { m.ma(q, p) };
            if ma {
                zasiahnute += 1;
            }
        }
        let pok = zasiahnute as f64 / dlzka as f64;
        if pok > najlepsie {
            najlepsie = pok;
        }
        pokrytia.push((p, pok));
    }
    if najlepsie <= 0.0 {
        return (-1, 0.0);
    }
    let mut vybrane: Option<(i64, f64)> = None;
    for (p, pok) in pokrytia {
        if pok < PODIEL_KANDIDATA * najlepsie {
            continue;
        }
        // Spomedzi uznaných kandidátov ten najbližšie k vnútru bunky.
        match vybrane {
            Some((vp, _)) if vp * dnu >= p * dnu => {}
            _ => vybrane = Some((p, pok)),
        }
    }
    vybrane.unwrap_or((-1, 0.0))
}

/// Zmeria rám jednej bunky (`riadok`/`stlpec` 1-indexované). `None`, keď by
/// okno hľadania ležalo mimo snímky.
pub fn ram_bunky(m: &Maska, riadok: i64, stlpec: i64) -> Option<NalezRamu> {
    if m.sirka == 0 || m.vyska == 0 {
        return None;
    }
    let (w, h) = (m.sirka as f64, m.vyska as f64);
    let pole = platno_bunka_box01(riadok, stlpec);
    let x0 = (pole.x * w).round() as i64;
    let y0 = (pole.y * h).round() as i64;
    let cw = (pole.w * w).round() as i64;
    let ch = (pole.h * h).round() as i64;
    if cw < 20 || ch < 20 {
        return None;
    }
    let x1 = x0 + cw;
    let y1 = y0 + ch;
    // Prahy sú odmerané na bunke ~100 px — prepočítaj ich na skutočnú veľkosť,
    // nech to platí aj na inom rozlíšení monitora.
    let k = cw as f64 / 100.0;
    let t = ((TOLERANCIA_HLADANIA as f64 * k).round() as i64).max(2);
    let roh = ((ROH as f64 * k).round() as i64).max(4);
    if x0 - t < 0 || y0 - t < 0 || x1 + t >= m.sirka as i64 || y1 + t >= m.vyska as i64 {
        return None;
    }
    if y1 - roh <= y0 + roh || x1 - roh <= x0 + roh {
        return None;
    }
    let (lx, lp) = najdi_ciaru(m, y0 + roh, y1 - roh, x0 - t, x0 + t, true, 1);
    let (px, pp) = najdi_ciaru(m, y0 + roh, y1 - roh, x1 - t, x1 + t, true, -1);
    let (ty, tp) = najdi_ciaru(m, x0 + roh, x1 - roh, y0 - t, y0 + t, false, 1);
    let (by, bp) = najdi_ciaru(m, x0 + roh, x1 - roh, y1 - t, y1 + t, false, -1);
    // Výplň bunky — stredné dve štvrtiny, teda bez rámu samotného.
    let vx = cw / 4;
    let vy = ch / 4;
    let mut zlte_vnutri = 0i64;
    let mut bodov = 0i64;
    for y in (y0 + vy)..(y1 - vy) {
        for x in (x0 + vx)..(x1 - vx) {
            bodov += 1;
            if m.ma(x, y) {
                zlte_vnutri += 1;
            }
        }
    }
    let vnutro = if bodov > 0 { zlte_vnutri as f64 / bodov as f64 } else { 0.0 };
    Some(NalezRamu {
        riadok,
        stlpec,
        pokrytie: lp.min(pp).min(tp).min(bp),
        sirka: px - lx,
        vyska: by - ty,
        sirka_bunky: cw,
        vyska_bunky: ch,
        vnutro,
    })
}

/// Všetkých 36 meraní, row-major (rovnaké poradie ako `canvas_rozlozenie::platno_box01`).
pub fn merania_z_masky(m: &Maska) -> Vec<NalezRamu> {
    let mut v = Vec::with_capacity((PLATNO_RIADKY * PLATNO_STLPCE) as usize);
    for r in 1..=PLATNO_RIADKY {
        for c in 1..=PLATNO_STLPCE {
            if let Some(n) = ram_bunky(m, r, c) {
                v.push(n);
            }
        }
    }
    v
}

/// Bunky, ktoré hra orámovala — `(riadok, stlpec)`, 1-indexované, zoradené.
pub fn oramovane_bunky_z_masky(m: &Maska) -> Vec<(i64, i64)> {
    merania_z_masky(m)
        .into_iter()
        .filter(|n| n.je_ramovana())
        .map(|n| (n.riadok, n.stlpec))
        .collect()
}

/// Skratka pre volajúceho v `src-tauri`: z celej snímky rovno zoznam buniek.
pub fn oramovane_bunky(rgb: &[u8], sirka: u32, vyska: u32) -> Vec<(i64, i64)> {
    oramovane_bunky_z_masky(&Maska::z_rgb(rgb, sirka, vyska))
}

/// Zoznam buniek do parametra `ramy` requestu na server: `"1-3.2-3.4-3.5-3"`.
/// Prázdny zoznam dá prázdny reťazec — volajúci ho vtedy do URL vôbec nedáva.
pub fn ramy_do_parametra(bunky: &[(i64, i64)]) -> String {
    bunky
        .iter()
        .map(|(r, c)| format!("{r}-{c}"))
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod testy {
    use super::*;

    /// Fixtures sú ŽLTÉ MASKY skutočných snímok 1920×1080 z
    /// `/srv/games/tli/analyzy` (CT 114), uložené ako RLE po riadkoch —
    /// `sirka vyska` na prvom riadku, potom `y x0-x1 x2-x3 …` len pre riadky,
    /// kde nejaký žltý bod je. Celá snímka ako RGB by mala 6,2 MB; maska v RLE
    /// má 1,5–23 kB a reprodukuje presne to, čo `Maska::z_rgb` z tej snímky
    /// spraví, takže geometrická vrstva sa testuje na REÁLNYCH dátach.
    fn maska_zo_suboru(nazov: &str) -> Maska {
        let cesta = format!("{}/testy-data/{}.rle", env!("CARGO_MANIFEST_DIR"), nazov);
        let obsah =
            std::fs::read_to_string(&cesta).unwrap_or_else(|e| panic!("chýba vzorka {cesta}: {e}"));
        let mut riadky = obsah.lines();
        let hlavicka: Vec<u32> =
            riadky.next().unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
        let mut m = Maska::nova(hlavicka[0], hlavicka[1]);
        for r in riadky {
            let mut casti = r.split_whitespace();
            let y: i64 = match casti.next() {
                Some(s) => s.parse().unwrap(),
                None => continue,
            };
            for beh in casti {
                let (a, b) = beh.split_once('-').unwrap();
                let (od, do_): (i64, i64) = (a.parse().unwrap(), b.parse().unwrap());
                for x in od..=do_ {
                    m.nastav(x, y);
                }
            }
        }
        m
    }

    fn ramy(nazov: &str) -> Vec<(i64, i64)> {
        oramovane_bunky_z_masky(&maska_zo_suboru(nazov))
    }

    /// Presne Richardov prípad zo zadania TLI 69: cieľ „Fill a Vertical Cell
    /// with a Monster Paint (0/3)", zdrojové monštrum Lv3 so značkou „!" v
    /// r6c3, riadok 3 je ľad. Očakávané cieľové bunky z textu zadania.
    #[test]
    fn vertical_zo_snimky_richarda() {
        assert_eq!(ramy("canvas-ramy-vertical"), vec![(1, 3), (2, 3), (4, 3), (5, 3)]);
    }

    /// Snímka z 19.10.2026 21:32 — hra orámovala 4 ortogonálnych susedov
    /// zdrojovej bunky r4c3.
    #[test]
    fn adjacent_zo_snimky_19_10() {
        assert_eq!(ramy("canvas-ramy-adjacent"), vec![(3, 3), (4, 2), (4, 4), (5, 3)]);
    }

    /// ⚠️ Kľúčový test bodu (b) z hlavičky: zdrojová bunka r4c3 má všetky štyri
    /// „hrany" žlté (sú to rámy jej štyroch susedov), a predsa orámovaná NIE JE.
    /// Rozlíši ju iba rozmer — keby sa `TOLERANCIA_ROZMERU` uvoľnila, appka by
    /// hráčovi radila položiť paint na už obsadenú zdrojovú bunku.
    #[test]
    fn zdrojova_bunka_obklopena_ramami_sa_za_ramovanu_nepovazuje() {
        let m = maska_zo_suboru("canvas-ramy-adjacent");
        let n = ram_bunky(&m, 4, 3).expect("bunka je v snímke");
        assert!(n.pokrytie >= PRAH_POKRYTIA, "{n:?}");
        assert!(n.sirka > n.sirka_bunky + TOLERANCIA_ROZMERU, "{n:?}");
        assert!(!n.je_ramovana(), "{n:?}");
    }

    /// Horizontálny cieľ — a zároveň dôkaz, že rám je aj na bunke, ktorá je
    /// UŽ OBSADENÁ: r3c3 nesie monštrum Lv3 a hra ju napriek tomu orámovala
    /// (r3c4 je zdroj so značkou „!", r3c1 a r3c6 sú mimo aktívnej plochy).
    #[test]
    fn horizontal_ramuje_aj_uz_obsadenu_bunku() {
        assert_eq!(ramy("canvas-ramy-horizontal"), vec![(3, 2), (3, 3), (3, 5)]);
    }

    #[test]
    fn diagonal_zo_snimky_20_10() {
        assert_eq!(ramy("canvas-ramy-diagonal"), vec![(3, 5), (4, 4), (5, 3)]);
    }

    /// Podobrazovka „Select the Brushstroke direction" — hra tam rám nekreslí.
    /// Z 75 reálnych snímok bolo takých 45; appka vtedy nesmie vymyslieť rám,
    /// ktorý na obrazovke nie je (solver má na to zálohu podľa názvu smeru).
    #[test]
    fn podobrazovka_bez_ramu_nevrati_nic() {
        assert!(ramy("canvas-ramy-bez").is_empty());
    }

    #[test]
    fn jantarove_body_z_ramov_prejdu_a_pozadie_nie() {
        // Skutočné farby odobrané z rámov na snímke 2026-10-20-133046.
        for (r, g, b) in [
            (247u8, 226u8, 117u8),
            (248, 240, 142),
            (245, 145, 95),
            (255, 245, 72),
            (254, 224, 110),
            (164, 139, 73),
            (247, 185, 62),
        ] {
            assert!(je_zlty(r, g, b), "{r},{g},{b} má byť žltý");
        }
        // Skutočné farby pozadia Frozen Canvasu z tej istej snímky.
        for (r, g, b) in [
            (22u8, 52u8, 80u8),
            (83, 110, 127),
            (51, 70, 85),
            (61, 97, 119),
            (254, 255, 255), // biela prázdna bunka
            (184, 192, 213), // ľadová modrá
        ] {
            assert!(!je_zlty(r, g, b), "{r},{g},{b} NEMÁ byť žltý");
        }
    }

    #[test]
    fn maska_z_rgb_pozna_svoje_body() {
        // 4×2 obrázok: prvý bod jantárový, zvyšok tmavý.
        let mut rgb = vec![0u8; 4 * 2 * 3];
        rgb[0] = 247;
        rgb[1] = 185;
        rgb[2] = 62;
        let m = Maska::z_rgb(&rgb, 4, 2);
        assert!(m.ma(0, 0));
        assert!(!m.ma(1, 0));
        assert_eq!(m.pocet_bodov(), 1);
    }

    #[test]
    fn prazdne_a_poskodene_vstupy_nepadnu() {
        assert!(oramovane_bunky(&[], 0, 0).is_empty());
        assert!(oramovane_bunky(&[0u8; 30], 100, 100).is_empty()); // pole kratšie než rozmery
        let maly = vec![200u8; 8 * 8 * 3];
        assert!(oramovane_bunky(&maly, 8, 8).is_empty()); // bunka by mala < 20 px
        assert_eq!(Maska::z_rgb(&[], 10, 10), Maska::nova(0, 0));
        assert!(ram_bunky(&Maska::nova(0, 0), 1, 1).is_none());
    }

    #[test]
    fn jednofarebna_plocha_nie_je_ram() {
        // Celá snímka „žltá" — každá hrana má pokrytie 1,0 a obdĺžnik vyjde aj
        // menší než bunka, takže ju zastaví až `PRAH_VNUTRA`: rám je tenký
        // prstenec, nie vyplnený štvorec.
        let zlta = vec![255u8, 240, 60].repeat(1920 * 1080);
        assert!(oramovane_bunky(&zlta, 1920, 1080).is_empty());
        let cierna = vec![0u8; 1920 * 1080 * 3];
        assert!(oramovane_bunky(&cierna, 1920, 1080).is_empty());
    }

    /// Orámovaná bunka má žltý PRSTENEC, nie žltú výplň — na reálnych snímkach
    /// mala vnútro najviac 0,184, prah je 0,50.
    #[test]
    fn oramovane_bunky_maju_vnutro_hlboko_pod_prahom() {
        for nazov in ["canvas-ramy-vertical", "canvas-ramy-adjacent", "canvas-ramy-horizontal"] {
            let m = maska_zo_suboru(nazov);
            for n in merania_z_masky(&m).into_iter().filter(|n| n.je_ramovana()) {
                assert!(n.vnutro <= 0.25, "{nazov} r{}c{}: {n:?}", n.riadok, n.stlpec);
            }
        }
    }

    #[test]
    fn ramy_do_parametra_ma_tvar_ktory_server_parsuje() {
        assert_eq!(ramy_do_parametra(&[(1, 3), (2, 3), (4, 3), (5, 3)]), "1-3.2-3.4-3.5-3");
        assert_eq!(ramy_do_parametra(&[]), "");
    }

    /// Mriežka je odmeraná ako priemer, takže na konkrétnej snímke môže sedieť
    /// o pár px vedľa — detekcia to musí zniesť. Posunutím masky o ±3 px sa
    /// výsledok nesmie zmeniť.
    #[test]
    fn posun_mriezky_o_par_pixelov_vysledok_nezmeni() {
        let p = maska_zo_suboru("canvas-ramy-vertical");
        let ocakavane = vec![(1, 3), (2, 3), (4, 3), (5, 3)];
        for posun in [-3i64, -1, 1, 3] {
            let mut m = Maska::nova(p.sirka, p.vyska);
            for y in 0..p.vyska as i64 {
                for x in 0..p.sirka as i64 {
                    if p.ma(x, y) {
                        m.nastav(x + posun, y + posun);
                    }
                }
            }
            assert_eq!(oramovane_bunky_z_masky(&m), ocakavane, "posun {posun}");
        }
    }
}
