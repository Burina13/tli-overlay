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

#[cfg(test)]
mod testy {
    use super::*;

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
}
