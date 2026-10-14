pub const MAX_DLZKA: u32 = 1920;

/// Zmenšovanie predchádza plytvaniu sieťového pásma a zbytočnému preťažovaniu servera.
/// Server dokáže spoľahlivo rozpoznať dosku už pri rozlíšení 1920 pixelov a zároveň má 
/// prísny limit 4 MB na prichádzajúci obrázok, ktorý by inak 4K monitory prekonali.
pub fn nove_rozmery(sirka: u32, vyska: u32, max_dlzka: u32) -> (u32, u32) {
    if sirka == 0 || vyska == 0 {
        return (1, 1);
    }

    if sirka <= max_dlzka && vyska <= max_dlzka {
        return (sirka, vyska);
    }

    let n_sirka: u32;
    let n_vyska: u32;

    if sirka > vyska {
        n_sirka = max_dlzka;
        n_vyska = ((vyska as u64 * max_dlzka as u64) / sirka as u64) as u32;
    } else {
        n_vyska = max_dlzka;
        n_sirka = ((sirka as u64 * max_dlzka as u64) / vyska as u64) as u32;
    }

    (n_sirka.max(1), n_vyska.max(1))
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn zmensenie_z_4k() {
        assert_eq!(nove_rozmery(3840, 2160, 1920), (1920, 1080));
    }

    #[test]
    fn zmensenie_z_1440p() {
        assert_eq!(nove_rozmery(2560, 1440, 1920), (1920, 1080));
    }

    #[test]
    fn podlimitne_ostavaju() {
        assert_eq!(nove_rozmery(1600, 900, 1920), (1600, 900));
        assert_eq!(nove_rozmery(1920, 1080, 1920), (1920, 1080));
    }

    #[test]
    fn udrzi_aspekt_aj_v_extremoch() {
        assert_eq!(nove_rozmery(5000, 100, 1920), (1920, 38));
    }

    #[test]
    fn nulove_rozmery_nepadnu() {
        assert_eq!(nove_rozmery(0, 0, 1920), (1, 1));
    }
}
