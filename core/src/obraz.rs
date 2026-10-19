/// Funkcia zistuje, ci je obrazok takmer uplne cierny (viac nez 99.5 % bajtov je pod danym prahom).
/// DOVOD: Ked hra bezi v rezime "exclusive fullscreen", zachytavanie obrazovky cez BitBlt
/// vrati uplne ciernu plochu. Zistenim tejto anomalia mozeme hracovi poradit, aby hru prepol
/// do rezimu "borderless window", namiesto toho, aby sme na server posielali cierny stvorec a cakali chybu 422.
pub fn je_takmer_cierny(rgb: &[u8], prah: u8) -> bool {
    let pocet_bajtov = rgb.len();
    if pocet_bajtov == 0 {
        return true;
    }

    let pocet_svetlych = rgb.iter().filter(|&&b| b > prah).count();
    let podiel_svetlych = pocet_svetlych as f64 / pocet_bajtov as f64;

    podiel_svetlych < 0.005
}

/// Skonvertuje surove data vo formate BGRA na suvisle pole RGB pixelov.
/// DOVOD: Windows GDI API (GetDIBits) standardne vracia obrazove data s prehodenymi farebnymi kanalmi (BGRA)
/// a casto aj so spodnym riadkom na zaciatku pola (zdola nahor). Tato funkcia ich znormalizuje pre standardne
/// kniznice na spracovanie obrazu.
pub fn bgra_na_rgb(bgra: &[u8], sirka: u32, vyska: u32, prevratit: bool) -> Vec<u8> {
    let sirka_usize = sirka as usize;
    let vyska_usize = vyska as usize;
    let pocet_pixelov = sirka_usize * vyska_usize;
    let mut rgb = vec![0u8; pocet_pixelov * 3];

    let riadok_bgra_bajty = sirka_usize * 4;
    let riadok_rgb_bajty = sirka_usize * 3;

    for y in 0..vyska_usize {
        let zdrojovy_y = if prevratit { vyska_usize - 1 - y } else { y };

        let offset_bgra = zdrojovy_y * riadok_bgra_bajty;
        let offset_rgb = y * riadok_rgb_bajty;

        for x in 0..sirka_usize {
            let i_bgra = offset_bgra + (x * 4);
            let i_rgb = offset_rgb + (x * 3);

            // GDI zarucuje, ze pole ma aspon pozadovanu velkost,
            // podmienky su tu pre bezpecnost behu.
            if i_bgra + 2 < bgra.len() && i_rgb + 2 < rgb.len() {
                rgb[i_rgb] = bgra[i_bgra + 2];     // R (z B)
                rgb[i_rgb + 1] = bgra[i_bgra + 1]; // G (z G)
                rgb[i_rgb + 2] = bgra[i_bgra];     // B (z B)
            }
        }
    }

    rgb
}

/// Podiel pixelov, ktoré sa medzi dvoma RGB snímkami ROVNAKÉHO rozmeru líšia
/// o viac než `prah` na aspoň jednom kanáli. Vracia `None`, keď sa snímky
/// nedajú porovnať (iná dĺžka — napr. hráč medzitým zmenil veľkosť okna).
/// DOVOD (TLI 40, `2026-10-18-1622-mcp.md`): appka nevie priamo zistiť, či
/// WebView2 v overlay okne SKUTOČNE odprezentoval snímku na obrazovku — DOM sa
/// vie „vykresliť" interne (appka to aj logovala) a napriek tomu sa nič
/// neukáže na skutočnej obrazovke. Odpoveď sa preto nehľadá v okne samom, ale
/// v tom, čo je NAOZAJ vidno: odfotí sa ten istý obdĺžnik predtým (bez
/// overlayu) a potom (keď by mal byť nad hrou vidieť panel s radou) a snímky
/// sa porovnajú — ak sa takmer nič nezmenilo, panel sa neprezentoval.
pub fn percento_odlisnych_pixelov(pred: &[u8], po: &[u8], prah: u8) -> Option<f64> {
    if pred.is_empty() || pred.len() != po.len() {
        return None;
    }
    let pocet_odlisnych = pred
        .iter()
        .zip(po.iter())
        .filter(|&(a, b)| a.abs_diff(*b) > prah)
        .count();
    Some(pocet_odlisnych as f64 / pred.len() as f64)
}

/// Vyreže z celistvého RGB poľa (3 bajty/pixel, zhora nadol) obdĺžnik zadaný
/// v normalizovaných Box01 súradniciach (0..1 vzhľadom na celý obrázok) —
/// rovnaký tvar, aký vracia `canvas_rozlozenie::platno_bunka_box01`/
/// `ruka_slot_box01`. Používa sa pred OCR (TLI 59b časť 2, bod 1): OCR nad
/// malým výrezom panelu je rýchlejšie a presnejšie než nad celou snímkou.
///
/// Vracia `None`, keď je vstupné pole menšie než `sirka`×`vyska`×3 (poškodená
/// snímka) alebo keď by výsledný výrez mal nulovú šírku/výšku (napr. `oblast`
/// leží celá mimo obrázka).
pub fn orez_box01(rgb: &[u8], sirka: u32, vyska: u32, oblast: crate::suradnice::Pole) -> Option<(Vec<u8>, u32, u32)> {
    if sirka == 0 || vyska == 0 || rgb.len() < (sirka as usize * vyska as usize * 3) {
        return None;
    }

    // Zámerne sa nezaokrúhľuje smerom von (napr. `ceil` na pravý okraj) — pri
    // zápornom alebo prebytočnom `oblast` by to vecne znamenalo žiadosť o
    // pixel mimo obrázka, čo `min`/`max` nižšie aj tak orezáva na platný rozsah.
    let x0 = ((oblast.x * sirka as f64) as i64).clamp(0, sirka as i64) as u32;
    let y0 = ((oblast.y * vyska as f64) as i64).clamp(0, vyska as i64) as u32;
    let x1 = (((oblast.x + oblast.w) * sirka as f64) as i64).clamp(0, sirka as i64) as u32;
    let y1 = (((oblast.y + oblast.h) * vyska as f64) as i64).clamp(0, vyska as i64) as u32;

    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    let nova_sirka = x1 - x0;
    let nova_vyska = y1 - y0;
    let mut vyrez = vec![0u8; (nova_sirka * nova_vyska * 3) as usize];

    let riadok_zdroj_bajty = sirka as usize * 3;
    let riadok_ciel_bajty = nova_sirka as usize * 3;

    for riadok in 0..nova_vyska as usize {
        let zdroj_offset = (y0 as usize + riadok) * riadok_zdroj_bajty + x0 as usize * 3;
        let ciel_offset = riadok * riadok_ciel_bajty;
        vyrez[ciel_offset..ciel_offset + riadok_ciel_bajty]
            .copy_from_slice(&rgb[zdroj_offset..zdroj_offset + riadok_ciel_bajty]);
    }

    Some((vyrez, nova_sirka, nova_vyska))
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn test_percento_odlisnych_pixelov_identicke_su_nulove() {
        let data = vec![10, 20, 30, 40, 50, 60];
        assert_eq!(percento_odlisnych_pixelov(&data, &data, 8), Some(0.0));
    }

    #[test]
    fn test_percento_odlisnych_pixelov_rozdielna_dlzka_vracia_none() {
        assert_eq!(percento_odlisnych_pixelov(&[1, 2, 3], &[1, 2], 8), None);
    }

    #[test]
    fn test_percento_odlisnych_pixelov_prazdne_vracia_none() {
        assert_eq!(percento_odlisnych_pixelov(&[], &[], 8), None);
    }

    #[test]
    fn test_percento_odlisnych_pixelov_pod_prahom_sa_nepocita() {
        // Rozdiel presne 8 nie je VIAC než prah 8 — nesmie sa počítať (šum
        // snímania obrazovky, nie skutočná zmena).
        assert_eq!(percento_odlisnych_pixelov(&[100], &[108], 8), Some(0.0));
    }

    #[test]
    fn test_percento_odlisnych_pixelov_spocita_presny_podiel() {
        // 10 bajtov, 2 sa zmenili o viac než prah → 0.2.
        let pred = vec![0u8; 10];
        let mut po = vec![0u8; 10];
        po[0] = 255;
        po[5] = 200;
        assert_eq!(percento_odlisnych_pixelov(&pred, &po, 8), Some(0.2));
    }

    #[test]
    fn test_je_takmer_cierny_uplne_cierna() {
        let data = vec![0; 1000];
        assert!(je_takmer_cierny(&data, 8));
    }

    #[test]
    fn test_je_takmer_cierny_biela() {
        let data = vec![255; 1000];
        assert!(!je_takmer_cierny(&data, 8));
    }

    #[test]
    fn test_je_takmer_cierny_jeden_svetly_pixel() {
        let mut data = vec![0; 1000];
        data[0] = 255; // Menej nez 0.5% z 1000 je 5, co je v poriadku
        assert!(je_takmer_cierny(&data, 8));
    }

    #[test]
    fn test_bgra_na_rgb_bez_prevratenia() {
        // 2x2 obrazok
        // B, G, R, A
        let bgra = vec![
            255, 0, 0, 255,   0, 255, 0, 255, // riadok 0
            0, 0, 255, 255,   255, 255, 255, 255, // riadok 1
        ];

        let rgb = bgra_na_rgb(&bgra, 2, 2, false);

        assert_eq!(rgb, vec![
            0, 0, 255,   0, 255, 0, // riadok 0 (R, G, B)
            255, 0, 0,   255, 255, 255, // riadok 1 (R, G, B)
        ]);
    }

    #[test]
    fn test_bgra_na_rgb_s_prevratenim() {
        let bgra = vec![
            255, 0, 0, 255,   0, 255, 0, 255, // riadok 0 (v skutocnosti spodny, ked prevratit=true)
            0, 0, 255, 255,   255, 255, 255, 255, // riadok 1 (v skutocnosti vrchny)
        ];

        let rgb = bgra_na_rgb(&bgra, 2, 2, true);

        assert_eq!(rgb, vec![
            255, 0, 0,   255, 255, 255, // povodny riadok 1 presunuty hore
            0, 0, 255,   0, 255, 0, // povodny riadok 0 presunuty dole
        ]);
    }

    fn pole(x: f64, y: f64, w: f64, h: f64) -> crate::suradnice::Pole {
        crate::suradnice::Pole { index: 0, x, y, w, h }
    }

    #[test]
    fn test_orez_box01_presny_pravy_dolny_kvadrant() {
        // 4x4 obrázok, 3 bajty/pixel, hodnota bajtu = (riadok*4 + stĺpec)
        let mut rgb = vec![0u8; 4 * 4 * 3];
        for riadok in 0..4u32 {
            for stlpec in 0..4u32 {
                let i = ((riadok * 4 + stlpec) * 3) as usize;
                let hodnota = (riadok * 4 + stlpec) as u8;
                rgb[i] = hodnota;
                rgb[i + 1] = hodnota;
                rgb[i + 2] = hodnota;
            }
        }

        let (vyrez, sirka, vyska) = orez_box01(&rgb, 4, 4, pole(0.5, 0.5, 0.5, 0.5)).unwrap();
        assert_eq!((sirka, vyska), (2, 2));
        // Pravý dolný kvadrant 4x4 obrázka sú pixely (2,2),(3,2),(2,3),(3,3) = hodnoty 10,11,14,15
        assert_eq!(vyrez, vec![10, 10, 10, 11, 11, 11, 14, 14, 14, 15, 15, 15]);
    }

    #[test]
    fn test_orez_box01_cela_snimka() {
        let rgb: Vec<u8> = (0..(3 * 3 * 3)).map(|i| i as u8).collect();
        let (vyrez, sirka, vyska) = orez_box01(&rgb, 3, 3, pole(0.0, 0.0, 1.0, 1.0)).unwrap();
        assert_eq!((sirka, vyska), (3, 3));
        assert_eq!(vyrez, rgb);
    }

    #[test]
    fn test_orez_box01_mimo_obrazka_vracia_none() {
        let rgb = vec![0u8; 4 * 4 * 3];
        assert_eq!(orez_box01(&rgb, 4, 4, pole(1.5, 1.5, 0.2, 0.2)), None);
        assert_eq!(orez_box01(&rgb, 4, 4, pole(0.5, 0.5, 0.0, 0.0)), None);
    }

    #[test]
    fn test_orez_box01_prilis_mala_vstupna_snimka_vracia_none() {
        let rgb = vec![0u8; 10]; // menej než 4*4*3
        assert_eq!(orez_box01(&rgb, 4, 4, pole(0.0, 0.0, 1.0, 1.0)), None);
    }
}
