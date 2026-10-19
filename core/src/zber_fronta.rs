//! Rozhodovanie fronty automatického zberu snímok Frozen Canvasu
//! (TLI 60, `2026-10-19-2135-mcp-2.md`, body 1 a 2) — kedy sa vôbec oplatí
//! odfotiť obrazovku, koľko snímok smie za deň odísť, ako dlho čakať po
//! neúspešnom odoslaní a kedy prestať pridávať do fronty na disku.
//!
//! Samotné fotenie, zápis na disk a HTTP sú v `src-tauri/src/zber_canvas.rs` —
//! sem patrí len to, čo sa dá overiť `cargo test` na Linuxe bez obrazovky,
//! disku a siete (rovnaké rozdelenie ako `denik` alebo `aktualizacia`).

use serde::{Deserialize, Serialize};

/// Najviac snímok za deň — priamo zo zadania („Limit 500 snímok na deň“).
pub const MAX_ZA_DEN: u32 = 500;

/// Koľko nedoručených snímok smie naraz ležať v priečinku fronty. Po
/// dosiahnutí sa NIČ NEMAŽE (zadanie: „Nič nemaž“) — len sa prestane
/// pridávať, kým sa fronta neodošle. Pri ~150 kB na snímku je to strop
/// okolo 150 MB na disku hráča, čo je strop, ktorý si človek ešte nevšimne,
/// a zároveň 2 dni plného zberu, keby bol server dlho dole.
pub const MAX_CAKAJUCICH: usize = 1000;

/// Najkratší odstup medzi dvoma snímkami, KEĎ je Canvas otvorený. Hráč pri
/// miešaní farieb klikne aj niekoľkokrát za sekundu a každý klik meniť na
/// JPEG + HTTP by bola zbytočná práca; 250 ms stačí na to, aby sa zachytil
/// stav po každom zmysluplnom ťahu.
pub const ODSTUP_V_CANVASE_MS: u64 = 250;

/// Najkratší odstup medzi dvoma KONTROLAMI, keď posledná skončila „toto nie
/// je Canvas“. Pri bežnom hraní sa kliká nepretržite a odfotiť celú plochu
/// pri každom kliku len preto, aby sa zistilo, že Canvas otvorený nie je, by
/// hráča stálo výkon v boji. Preto sa mimo Canvasu kontroluje nanajvýš raz
/// za dve sekundy — otvorenie Canvasu sa tak zachytí najneskôr o 2 s neskôr,
/// čo pri obrazovke, na ktorej sa sedí minúty, nič nestojí.
pub const ODSTUP_MIMO_CANVASU_MS: u64 = 2000;

/// Ako dlho po kliku sa čaká, než sa fotí — zo zadania („odfotí okno hry
/// 300 ms po kliku“). Dovtedy hra stihne dokresliť výsledok ťahu.
pub const ONESKORENIE_PO_KLIKU_MS: u64 = 300;

/// Koľko snímok už dnes odišlo. Ukladá sa vedľa fronty, aby limit prežil
/// reštart appky (inak by stačilo appku vypnúť a zapnúť a limit by sa
/// vynuloval).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DennyStav {
    /// Poradové číslo dňa od 1. 1. 1970 (`unix / 86400`). Zámerne UTC, nie
    /// miestny čas: appka nemá knižnicu časových pásiem a prepočet „ručne“
    /// by bol ďalší zdroj chýb. Pre hráča v SR to znamená, že sa počítadlo
    /// nuluje o 01:00/02:00 v noci, teda v praxi medzi hernými sedeniami.
    pub den: u64,
    pub pocet: u32,
}

/// Číslo dňa z unixového času.
pub fn den_z_casu(unix_s: u64) -> u64 {
    unix_s / 86_400
}

impl DennyStav {
    /// Posunie stav na daný deň (keď sa deň zmenil, počítadlo sa vynuluje)
    /// a vráti, či sa ešte smie zbierať.
    pub fn smie_zbierat(&mut self, unix_s: u64) -> bool {
        let dnes = den_z_casu(unix_s);
        if self.den != dnes {
            self.den = dnes;
            self.pocet = 0;
        }
        self.pocet < MAX_ZA_DEN
    }

    /// Zaráta jednu odfotenú snímku. Volá sa až vtedy, keď snímka naozaj
    /// vznikla — nie pri každej kontrole obrazovky.
    pub fn zapocitaj(&mut self, unix_s: u64) {
        let dnes = den_z_casu(unix_s);
        if self.den != dnes {
            self.den = dnes;
            self.pocet = 0;
        }
        self.pocet = self.pocet.saturating_add(1);
    }

    /// Koľko ešte dnes ostáva — len do denníka a do hlásenia.
    pub fn zostava(&self) -> u32 {
        MAX_ZA_DEN.saturating_sub(self.pocet)
    }
}

/// Má sa po kliku vôbec siahnuť na obrazovku? `bol_canvas` je výsledok
/// POSLEDNEJ kontroly, `od_poslednej_ms` čas od nej.
///
/// Prvé volanie (ešte sa nekontrolovalo nič) sa pozná podľa
/// `od_poslednej_ms == u64::MAX` — vtedy sa kontroluje vždy.
pub fn ma_sa_kontrolovat(bol_canvas: bool, od_poslednej_ms: u64) -> bool {
    let prah = if bol_canvas { ODSTUP_V_CANVASE_MS } else { ODSTUP_MIMO_CANVASU_MS };
    od_poslednej_ms >= prah
}

/// Koľko sekúnd počkať pred ďalším pokusom o odoslanie fronty. Prvý pokus
/// po zlyhaní o 5 s, potom zdvojnásobovanie po 5 minút — server býva dole
/// buď na chvíľu (reštart služby), alebo nadlho (výpadok internetu u hráča)
/// a obe krajnosti majú vyjsť rozumne.
pub fn odstup_po_zlyhani(pocet_zlyhani: u32) -> u64 {
    const ZACIATOK_S: u64 = 5;
    const STROP_S: u64 = 300;
    if pocet_zlyhani == 0 {
        return 0;
    }
    let exponent = (pocet_zlyhani - 1).min(16);
    ZACIATOK_S.saturating_mul(1u64 << exponent).min(STROP_S)
}

/// Smie do fronty na disku pribudnúť ďalšia snímka?
pub fn smie_pribudnut(cakajucich: usize) -> bool {
    cakajucich < MAX_CAKAJUCICH
}

/// Meno súboru jednej snímky vo fronte. Obsahuje čas (kvôli poradiu
/// odosielania, ktoré je abecedné) aj poradové číslo v rámci sekundy —
/// v Canvase môžu za jednu sekundu vzniknúť aj štyri snímky a rovnaké meno
/// by jednu z nich prepísalo.
pub fn nazov_snimky(unix_s: u64, poradie: u64) -> String {
    format!("canvas-{unix_s:010}-{:04}.jpg", poradie % 10_000)
}

/// Je to súbor našej fronty? Používa sa pri počítaní čakajúcich aj pri
/// výbere ďalšieho na odoslanie — nech sa appka nepokúsi poslať niečo,
/// čo si do priečinka odložil niekto iný.
pub fn je_subor_fronty(nazov: &str) -> bool {
    nazov.starts_with("canvas-") && nazov.ends_with(".jpg")
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn denny_limit_zastavi_na_pat_stovkach() {
        let mut s = DennyStav::default();
        let cas = 1_760_000_000;
        for _ in 0..MAX_ZA_DEN {
            assert!(s.smie_zbierat(cas));
            s.zapocitaj(cas);
        }
        assert_eq!(s.pocet, MAX_ZA_DEN);
        assert!(!s.smie_zbierat(cas));
        assert_eq!(s.zostava(), 0);
    }

    #[test]
    fn novy_den_vynuluje_pocitadlo() {
        let mut s = DennyStav::default();
        let cas = 1_760_000_000;
        for _ in 0..MAX_ZA_DEN {
            s.zapocitaj(cas);
        }
        assert!(!s.smie_zbierat(cas));
        // o deň neskôr
        assert!(s.smie_zbierat(cas + 86_400));
        assert_eq!(s.pocet, 0);
    }

    #[test]
    fn zapocitanie_v_novy_den_tiez_vynuluje() {
        // Keď sa deň zmení MEDZI `smie_zbierat` a `zapocitaj` (polnoc práve
        // padla), počítadlo sa nesmie priratať k včerajšku.
        let mut s = DennyStav { den: den_z_casu(1_760_000_000), pocet: 499 };
        s.zapocitaj(1_760_000_000 + 86_400);
        assert_eq!(s.pocet, 1);
    }

    #[test]
    fn prazdny_stav_zacina_od_nuly() {
        let mut s = DennyStav::default();
        assert!(s.smie_zbierat(1_760_000_000));
        assert_eq!(s.zostava(), MAX_ZA_DEN);
    }

    #[test]
    fn mimo_canvasu_sa_kontroluje_zriedka() {
        assert!(!ma_sa_kontrolovat(false, 100));
        assert!(!ma_sa_kontrolovat(false, ODSTUP_MIMO_CANVASU_MS - 1));
        assert!(ma_sa_kontrolovat(false, ODSTUP_MIMO_CANVASU_MS));
    }

    #[test]
    fn v_canvase_sa_kontroluje_takmer_kazdy_klik() {
        assert!(!ma_sa_kontrolovat(true, 100));
        assert!(ma_sa_kontrolovat(true, ODSTUP_V_CANVASE_MS));
        // to isté oneskorenie mimo Canvasu ešte nestačí
        assert!(!ma_sa_kontrolovat(false, ODSTUP_V_CANVASE_MS));
    }

    #[test]
    fn prve_volanie_sa_kontroluje_vzdy() {
        assert!(ma_sa_kontrolovat(false, u64::MAX));
        assert!(ma_sa_kontrolovat(true, u64::MAX));
    }

    #[test]
    fn odstup_rastie_a_zastavi_sa_na_piatich_minutach() {
        assert_eq!(odstup_po_zlyhani(0), 0);
        assert_eq!(odstup_po_zlyhani(1), 5);
        assert_eq!(odstup_po_zlyhani(2), 10);
        assert_eq!(odstup_po_zlyhani(3), 20);
        assert_eq!(odstup_po_zlyhani(4), 40);
        assert_eq!(odstup_po_zlyhani(10), 300);
        assert_eq!(odstup_po_zlyhani(u32::MAX), 300);
    }

    #[test]
    fn plna_fronta_uz_nepriberie_ale_nic_nemaze() {
        assert!(smie_pribudnut(0));
        assert!(smie_pribudnut(MAX_CAKAJUCICH - 1));
        assert!(!smie_pribudnut(MAX_CAKAJUCICH));
        assert!(!smie_pribudnut(MAX_CAKAJUCICH + 50));
    }

    #[test]
    fn mena_suborov_su_zoradene_podla_casu() {
        let a = nazov_snimky(1_760_000_000, 0);
        let b = nazov_snimky(1_760_000_000, 1);
        let c = nazov_snimky(1_760_000_001, 0);
        assert!(a < b && b < c, "{a} {b} {c}");
        assert_eq!(a, "canvas-1760000000-0000.jpg");
    }

    #[test]
    fn poradie_sa_zabali_a_neprerastie_do_mena() {
        assert_eq!(nazov_snimky(1, 10_000), "canvas-0000000001-0000.jpg");
    }

    #[test]
    fn cudzie_subory_sa_do_fronty_nepocitaju() {
        assert!(je_subor_fronty("canvas-1760000000-0000.jpg"));
        assert!(!je_subor_fronty("canvas-1760000000-0000.jpg.tmp"));
        assert!(!je_subor_fronty("poznamka.txt"));
        assert!(!je_subor_fronty("screenshot.jpg"));
    }

    #[test]
    fn denny_stav_prezije_stary_json_bez_poli() {
        let s: DennyStav = serde_json::from_str("{}").unwrap();
        assert_eq!(s, DennyStav::default());
    }
}
