//! Lokálne rozpoznanie „je PRÁVE otvorený Frozen Canvas?“ z pomeru farieb
//! (TLI 60, `2026-10-19-2135-mcp-2.md`, bod 1 — „rozpozná to lokálne podľa
//! mriežky z TLI 59, **prípadne podľa pomeru farieb**“).
//!
//! Prečo pomer farieb a nie mriežka: TLI 59 (lokálne rozpoznanie mriežky 6×6,
//! OCR, template matching) v čase písania tohto modulu ešte nebola hotová —
//! hotový bol len solver (TLI 59a). Zber snímok na učenie ju ale nepotrebuje:
//! stačí vedieť, ČI je obrazovka Canvasu otvorená, nie čo presne na nej je.
//! Pomer farieb je navyše o rádovo lacnejší (jeden prechod vzorkovanou
//! mriežkou bodov, žiadne OCR, žiadne ikony v balíku appky), takže sa dá
//! spustiť pri každom kliku myšou bez toho, aby to hráč na výkone pocítil.
//! Keď TLI 59 dodá skutočné rozpoznanie mriežky, toto sa dá nahradiť — rozhranie
//! (`je_otvoreny_canvas`) ostáva.
//!
//! **Prahy nie sú odhad.** Sú odmerané na 309 reálnych snímkach z
//! `/srv/games/tli/analyzy` (CT 114, stav 19.10.2026): 17 snímok Frozen Canvasu
//! a 292 snímok všetkého ostatného (hranie, Tartarus, trh, odmeny). Pozor,
//! 12 z tých 17 prišlo ako `modul=canvas`, zvyšných 5 ako `tartarus-poradca` —
//! Richard mal Canvas otvorený a stlačil F8; ručne sa overilo, že sú to naozaj
//! snímky Canvasu, a do merania sa počítajú ako kladné.
//!
//! Namerané rozpätia (vzorkovanie `VZORKA_SIRKA`×`VZORKA_VYSKA`):
//!
//! | veličina | Canvas (17) | ostatné (292) |
//! |---|---|---|
//! | `ladovy` | min **0,102** | pri `mriezka_modra ≥ 0,65` max **0,021** |
//! | `mriezka_modra` | min **0,797** | pri `ladovy ≥ 0,07` max **0,123** |
//!
//! Prahy sú preto nastavené hlboko do medzery (0,07 a 0,65) a obe podmienky
//! musia platiť NARAZ: samotné „je to modrasté“ by chytilo ľadovú mapu,
//! samotná „modrá mriežka“ zasa obrazovku odmien s modrým pozadím. Spolu dali
//! na tej sade 17/17 správne kladných a 0/292 falošne kladných.

/// Koľko bodov sa z obrázka odoberie na šírku. Nejde o zmenšovanie obrázka
/// (to by si pýtalo `image` a tým aj systémové závislosti v jadre) — len sa
/// vyberie pravidelná mriežka bodov. 192×108 = 20 736 vzoriek je dosť na
/// stabilné pomery a zároveň to trvá zlomok milisekundy.
pub const VZORKA_SIRKA: u32 = 192;
/// Koľko bodov sa z obrázka odoberie na výšku.
pub const VZORKA_VYSKA: u32 = 108;

/// Výrez, v ktorom na obrazovke Canvasu leží mriežka 6×6 (zlomok okna hry).
/// Odčítané z reálnych snímok 1920×1080 — mriežka tam zaberá zhruba
/// x 340–1000 px, y 230–870 px. Je to zámerne ŠIROKÝ výrez: nehľadá sa v ňom
/// presná poloha buniek (to je úloha TLI 59), len to, či je celá tá oblasť
/// chladno modrá, ako ju kreslí hra.
const MRIEZKA_X0: f32 = 0.18;
const MRIEZKA_X1: f32 = 0.52;
const MRIEZKA_Y0: f32 = 0.22;
const MRIEZKA_Y1: f32 = 0.80;

/// Najmenší podiel „ľadových“ bodov v celom okne, pri ktorom sa ešte uvažuje
/// o Canvase. Namerané: Canvas ≥ 0,102; ostatné snímky s modrou mriežkou ≤ 0,021.
pub const PRAH_LADOVY: f32 = 0.07;
/// Najmenší podiel modrastých bodov vo výreze mriežky. Namerané: Canvas ≥ 0,797;
/// ostatné snímky s ľadovým nádychom ≤ 0,123.
pub const PRAH_MRIEZKA_MODRA: f32 = 0.65;

/// Namerané pomery farieb jednej snímky. Vracajú sa celé (nielen `bool`), aby
/// sa dali zapísať do denníka appky — keď detekcia niekedy zlyhá u hráča,
/// z logu sa dá zistiť, KTORÁ z dvoch podmienok netrafila, bez posielania
/// obrázka kamkoľvek.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crty {
    /// Podiel bodov celého okna, ktoré sú „ľadové“ — stredne jasná, málo
    /// sýta modrá, akou hra kreslí pozadie Frozen Canvasu.
    pub ladovy: f32,
    /// Podiel bodov vo výreze mriežky, ktoré majú modrú zložku navrchu.
    pub mriezka_modra: f32,
    /// Podiel veľmi tmavých bodov vo výreze mriežky (prázdne bunky–víry).
    /// Nevstupuje do rozhodnutia — je to len údaj do denníka, lebo pri
    /// zaplnenej doske klesá a pomáha pochopiť, čo appka videla.
    pub mriezka_tmava: f32,
}

impl Crty {
    /// Rozhodnutie podľa prahov vyššie. Obe podmienky musia platiť naraz.
    pub fn je_canvas(&self) -> bool {
        self.ladovy >= PRAH_LADOVY && self.mriezka_modra >= PRAH_MRIEZKA_MODRA
    }
}

/// Je bod „ľadový“? Stredne jasná (nie čierna, nie presvietená), zreteľne
/// modrejšia než červená a málo sýta — presne tón, akým hra kreslí pozadie
/// a rám Frozen Canvasu. Hranice sú zámerne široké: Canvas má dve varianty
/// pozadia (bledú „Paint“ a tmavšiu „Draw“) a obe musia prejsť.
fn je_ladovy(r: u8, g: u8, b: u8) -> bool {
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    (90..=215).contains(&b) && b as i32 > r as i32 + 12 && b >= g && (mx as i32 - mn as i32) <= 85
}

/// Má bod modrú zložku navrchu (čo i len mierne)? Oveľa voľnejšie než
/// `je_ladovy` — vo výreze mriežky ide o to, že CELÁ plocha má modrý nádych,
/// vrátane tmavých buniek, ktoré by do „ľadových“ nikdy nespadli.
fn je_modrasty(r: u8, g: u8, b: u8) -> bool {
    b as i32 > r as i32 + 8 && b >= g
}

/// Spočíta pomery farieb nad súvislým RGB poľom (3 bajty na pixel, zhora
/// nadol) s rozmermi `sirka`×`vyska`. Pole smie byť ľubovoľne veľké —
/// vzorkuje sa z neho pevná mriežka `VZORKA_SIRKA`×`VZORKA_VYSKA` bodov,
/// takže čas behu nezávisí od rozlíšenia monitora ani od DPI.
pub fn crty(rgb: &[u8], sirka: u32, vyska: u32) -> Crty {
    let prazdne = Crty { ladovy: 0.0, mriezka_modra: 0.0, mriezka_tmava: 0.0 };
    if sirka == 0 || vyska == 0 || rgb.len() < (sirka as usize * vyska as usize * 3) {
        return prazdne;
    }

    let mut ladovych: u32 = 0;
    let mut mriezka_bodov: u32 = 0;
    let mut mriezka_modrych: u32 = 0;
    let mut mriezka_tmavych: u32 = 0;

    for vy in 0..VZORKA_VYSKA {
        // Stred vzorkovanej bunky, nie jej roh — inak by sa pri malých
        // obrázkoch vzorkoval stále prvý riadok/stĺpec.
        let fy = (vy as f32 + 0.5) / VZORKA_VYSKA as f32;
        let y = ((fy * vyska as f32) as u32).min(vyska - 1);
        for vx in 0..VZORKA_SIRKA {
            let fx = (vx as f32 + 0.5) / VZORKA_SIRKA as f32;
            let x = ((fx * sirka as f32) as u32).min(sirka - 1);

            let i = ((y as usize * sirka as usize) + x as usize) * 3;
            let (r, g, b) = (rgb[i], rgb[i + 1], rgb[i + 2]);

            if je_ladovy(r, g, b) {
                ladovych += 1;
            }
            if (MRIEZKA_X0..MRIEZKA_X1).contains(&fx) && (MRIEZKA_Y0..MRIEZKA_Y1).contains(&fy) {
                mriezka_bodov += 1;
                if je_modrasty(r, g, b) {
                    mriezka_modrych += 1;
                }
                if r.max(g).max(b) < 55 {
                    mriezka_tmavych += 1;
                }
            }
        }
    }

    let vsetkych = (VZORKA_SIRKA * VZORKA_VYSKA) as f32;
    if mriezka_bodov == 0 {
        return prazdne;
    }
    Crty {
        ladovy: ladovych as f32 / vsetkych,
        mriezka_modra: mriezka_modrych as f32 / mriezka_bodov as f32,
        mriezka_tmava: mriezka_tmavych as f32 / mriezka_bodov as f32,
    }
}

/// Skratka pre volajúceho, ktorého nezaujímajú čísla, len odpoveď.
pub fn je_otvoreny_canvas(rgb: &[u8], sirka: u32, vyska: u32) -> bool {
    crty(rgb, sirka, vyska).je_canvas()
}

#[cfg(test)]
mod testy {
    use super::*;

    /// Vzorky sú skutočné snímky z `/srv/games/tli/analyzy` (CT 114), zmenšené
    /// bodovým vzorkovaním na 96×54 a uložené ako surové RGB — bez závislosti
    /// na dekodéri obrázkov, nech `cargo test` v CI nepotrebuje `image`.
    const VZ_SIRKA: u32 = 96;
    const VZ_VYSKA: u32 = 54;

    fn vzorka(nazov: &str) -> Vec<u8> {
        let cesta = format!("{}/testy-data/{}.rgb", env!("CARGO_MANIFEST_DIR"), nazov);
        std::fs::read(&cesta).unwrap_or_else(|e| panic!("chýba vzorka {cesta}: {e}"))
    }

    fn crty_vzorky(nazov: &str) -> Crty {
        crty(&vzorka(nazov), VZ_SIRKA, VZ_VYSKA)
    }

    #[test]
    fn canvas_bledy_sa_rozpozna() {
        let c = crty_vzorky("canvas-svetly");
        assert!(c.je_canvas(), "{c:?}");
        assert!(c.ladovy > 0.4, "{c:?}");
    }

    #[test]
    fn canvas_tmava_varianta_sa_tiez_rozpozna() {
        // Obrazovka „Draw" má o rád tmavšie pozadie — práve na nej sa
        // samotný prah `ladovy` láme najtesnejšie (0,109 pri prahu 0,07),
        // a preto je v sade.
        let c = crty_vzorky("canvas-tmavy");
        assert!(c.je_canvas(), "{c:?}");
        assert!(c.ladovy < 0.2, "tmavá varianta má byť tesne nad prahom: {c:?}");
    }

    #[test]
    fn canvas_odfoteny_cez_f8_sa_rozpozna_rovnako() {
        assert!(crty_vzorky("canvas-f8").je_canvas());
    }

    #[test]
    fn bezne_hranie_nie_je_canvas() {
        let c = crty_vzorky("ine-mapa");
        assert!(!c.je_canvas(), "{c:?}");
    }

    #[test]
    fn modra_obrazovka_bez_ladoveho_naderu_nie_je_canvas() {
        // Najmodrejšia NE-canvas snímka celej sady: `mriezka_modra` 0,84
        // (nad prahom!), ale `ladovy` len 0,02. Presne preto sú podmienky dve.
        let c = crty_vzorky("ine-modra");
        assert!(c.mriezka_modra >= PRAH_MRIEZKA_MODRA, "{c:?}");
        assert!(!c.je_canvas(), "{c:?}");
    }

    #[test]
    fn ladova_obrazovka_bez_modrej_mriezky_nie_je_canvas() {
        // Opačný prípad: obrazovka odmien s farebnou žiarou má `ladovy` 0,118
        // (nad prahom), ale vo výreze mriežky modrá nie je.
        let c = crty_vzorky("ine-ladova");
        assert!(c.ladovy >= PRAH_LADOVY, "{c:?}");
        assert!(!c.je_canvas(), "{c:?}");
    }

    #[test]
    fn vsetkych_sest_vzoriek_sedi_s_namerou_z_pythonu() {
        // Referenčné hodnoty spočítal pri návrhu prahov Python nad TÝMI ISTÝMI
        // bajtmi (`~/praca/tli60-vzorky`). Keď sa raz niekto dotkne vzorkovania
        // alebo prahov, tento test ukáže presne, o koľko sa výsledok pohol.
        let ocakavane: [(&str, f32, f32); 6] = [
            ("canvas-svetly", 0.4946, 0.9404),
            ("canvas-tmavy", 0.1086, 0.8278),
            ("canvas-f8", 0.4979, 0.9266),
            ("ine-mapa", 0.0193, 0.0427),
            ("ine-modra", 0.0199, 0.8402),
            ("ine-ladova", 0.1182, 0.1270),
        ];
        for (nazov, ladovy, modra) in ocakavane {
            let c = crty_vzorky(nazov);
            assert!((c.ladovy - ladovy).abs() < 0.005, "{nazov}: ladovy {c:?}");
            assert!((c.mriezka_modra - modra).abs() < 0.005, "{nazov}: modra {c:?}");
        }
    }

    #[test]
    fn prazdne_a_poskodene_vstupy_nepadnu() {
        assert!(!je_otvoreny_canvas(&[], 0, 0));
        assert!(!je_otvoreny_canvas(&[0u8; 30], 100, 100)); // pole kratšie než rozmery
        assert_eq!(crty(&[], 10, 10), Crty { ladovy: 0.0, mriezka_modra: 0.0, mriezka_tmava: 0.0 });
    }

    #[test]
    fn jednofarebne_plochy_nie_su_canvas() {
        let cierna = vec![0u8; 64 * 36 * 3];
        assert!(!je_otvoreny_canvas(&cierna, 64, 36));
        let biela = vec![255u8; 64 * 36 * 3];
        assert!(!je_otvoreny_canvas(&biela, 64, 36));
    }

    #[test]
    fn obraz_mensi_nez_vzorkovacia_mriezka_sa_zvladne() {
        // 8×8 bodov sa vzorkuje 192×108-krát — každý bod mnohokrát. Nesmie to
        // ani spadnúť, ani čítať mimo poľa.
        let maly = vec![120u8; 8 * 8 * 3];
        let c = crty(&maly, 8, 8);
        assert_eq!(c.ladovy, 0.0);
    }
}
