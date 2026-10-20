//! Rozhodovanie zberu snímok minimapy (TLI 62, krok 62/2 — jadro appky pre
//! zber ROI/ticker/fronta, zadanie `inbox/2026-10-20-0440-mcp.md`,
//! kontrakt `docs/fakty/hry/torchlight-infinite/minimapa-api-kontrakt.md`).
//!
//! Vzor rozdelenia je `zber_fronta` (TLI 60, Frozen Canvas): sem patrí len
//! to, čo sa dá overiť `cargo test` na Linuxe bez obrazovky, siete a okna
//! hry — teda ROI, kedy fotiť, denný strop, fronta na disku a vyhodnotenie
//! odpovede servera „už netreba". Samotné fotenie, HTTP a čítanie
//! `UE_game.log` sú až v `src-tauri/` (krok 62/3).
//!
//! **Vlastný `DennyStav`, nezdieľaný s Canvasom** — keby appka počítala oba
//! zbery do jedného stropu, jeden by druhému jedol rozpočet a hráč by
//! nevedel prečo.
//!
//! **ToS (zadanie `inbox/done/2026-10-20-0155-mcp.md`, pravidlo 23):** zbiera
//! sa len to, čo hráč VIDÍ (snímka okna + `UE_game.log`), nič sa neodkrýva,
//! žiadny pohyb ani klik za hráča. Tento modul rozhoduje len KEDY a ČI sa
//! odfotí — čo sa fotí, rieši krok 62/3 výrezom okna.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ── 1. ROI minimapy (box 0..1 voči oknu hry, rovnaký tvar ako `canvas_rozlozenie`) ──

/// Výrez minimapy ako zlomok rozmerov okna hry (`Box01`) — rovnaká
/// matematika ako `canvas_rozlozenie`/`rozlozenie`, nech sa dá priamo
/// prepočítať na pixely cez `suradnice::box01_na_css`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RoiMinimapy {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl RoiMinimapy {
    /// Orezanie do platného rámca okna — ručne upravené nastavenia nesmú
    /// appke poslať zápornú alebo presahujúcu hodnotu na server.
    pub fn normalizuj(&mut self) {
        self.x = self.x.clamp(0.0, 1.0);
        self.y = self.y.clamp(0.0, 1.0);
        self.w = self.w.clamp(0.0, 1.0 - self.x);
        self.h = self.h.clamp(0.0, 1.0 - self.y);
    }

    /// Tvar parametra `roi` z kontraktu — `x,y,w,h` v 0..1, štyri desatinné
    /// miesta stačia na pixel presne aj pri 4K oknách.
    pub fn ako_parameter(&self) -> String {
        format!("{:.4},{:.4},{:.4},{:.4}", self.x, self.y, self.w, self.h)
    }
}

/// **Pracovná hypotéza, NIE zmeraná hodnota** — žiadna snímka minimapy
/// zatiaľ k dispozícii nie je (krok 62/1 server len prijíma, appka ešte
/// nefotí). Odhad: ľavý horný štvorec, bežné umiestnenie malej minimapy
/// v ARPG. Opraví sa v `Nastavenia::minimapa_roi`, keď prídu prvé snímky
/// (krok 62/4 ich vie overiť spätne nad archívom cez parameter `roi`).
pub const ROI_ROH_PREDVOLENE: RoiMinimapy = RoiMinimapy { x: 0.0, y: 0.0, w: 0.16, h: 0.16 };

/// **Pracovná hypotéza, NIE zmeraná hodnota** — Richard používa režim
/// „stred" (mapa stojí, hýbe sa jeho poloha), kde je minimapa veľký
/// stredový obdĺžnik, vyšší než širší (bežný tvar rozbaleného F-mapy
/// pohľadu). Rovnako prepísateľné bez nového buildu.
pub const ROI_STRED_PREDVOLENE: RoiMinimapy = RoiMinimapy { x: 0.25, y: 0.125, w: 0.5, h: 0.75 };

/// Ktorý režim minimapy appka práve sleduje — zodpovedá parametru `rezim`
/// z kontraktu (`"roh"` / `"stred"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RezimMinimapy {
    Roh,
    Stred,
}

impl RezimMinimapy {
    /// Hodnota pre query parameter `rezim` v `POST /api/minimapa`.
    pub fn ako_parameter(&self) -> &'static str {
        match self {
            RezimMinimapy::Roh => "roh",
            RezimMinimapy::Stred => "stred",
        }
    }

    /// Spätné parsovanie (napr. z uloženej hodnoty v nastaveniach alebo od
    /// sledovača logu). Neznáma hodnota padá na `None` — volajúci si musí
    /// zvoliť vlastnú predvolenú hodnotu, aby sa omylom nepoužil nesprávny
    /// režim ticho.
    pub fn z_textu(text: &str) -> Option<Self> {
        match text.trim().to_lowercase().as_str() {
            "roh" => Some(RezimMinimapy::Roh),
            "stred" => Some(RezimMinimapy::Stred),
            _ => None,
        }
    }
}

/// Dvojica ROI (jeden na režim), to, čo sa reálne ukladá do
/// `Nastavenia::minimapa_roi` — „štyri čísla na režim", prepísateľné bez
/// nového buildu.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoiSada {
    pub roh: RoiMinimapy,
    pub stred: RoiMinimapy,
}

impl Default for RoiSada {
    fn default() -> Self {
        Self { roh: ROI_ROH_PREDVOLENE, stred: ROI_STRED_PREDVOLENE }
    }
}

impl RoiSada {
    /// Volá sa pri každom načítaní/uložení nastavení, rovnako ako
    /// `VzhladTrasy::normalizuj` — ručne upravený súbor nesmie appku
    /// dostať do stavu s ROI mimo okna.
    pub fn normalizuj(&mut self) {
        self.roh.normalizuj();
        self.stred.normalizuj();
    }

    /// ROI pre daný režim — jedno miesto, kde si zberač pýta, čo má vyrezať.
    pub fn pre_rezim(&self, rezim: RezimMinimapy) -> RoiMinimapy {
        match rezim {
            RezimMinimapy::Roh => self.roh,
            RezimMinimapy::Stred => self.stred,
        }
    }
}

// ── 2. Ticker á 2 s ──

/// Najkratší odstup medzi dvoma snímkami minimapy. Zo zadania: „ticker
/// á 2 s". Na rozdiel od Canvasu (`zber_fronta`, dva odlišné odstupy podľa
/// toho, či je Canvas otvorený) tu je len jeden stav — buď beh mapy
/// prebieha, alebo nie — takže stačí jedna konštanta.
pub const ODSTUP_TIKU_MS: u64 = 2000;

/// Má appka práve teraz odfotiť minimapu? Fotí sa LEN keď beh mapy prebieha
/// (`beh_prebieha` — vstupom je to, čo appke dáva sledovač `UE_game.log`,
/// nie samotný ticker), nikdy v hube/lobby, kde by sa minimapa fotila
/// zbytočne alebo by mohla ukazovať niečo iné.
///
/// Prvé volanie (appka sa práve spustila do behu) sa pozná podľa
/// `od_poslednej_ms == u64::MAX` — rovnaká konvencia ako
/// `zber_fronta::ma_sa_kontrolovat`.
pub fn ma_sa_fotit(beh_prebieha: bool, od_poslednej_ms: u64) -> bool {
    beh_prebieha && od_poslednej_ms >= ODSTUP_TIKU_MS
}

// ── 3. Denný strop a fronta na disku ──

/// Najviac snímok minimapy za deň na appku — zrkadlí `max_za_den: 2000`
/// z `tli-web.yml` (krok 62/1). Appka si to drží aj sama, aby po dosiahnutí
/// stropu neplytvala HTTP požiadavkami, ktoré server aj tak odmietne
/// (`prijate: false`, `dost_dat`/denný strop).
pub const MAX_ZA_DEN: u32 = 2000;

/// Koľko nedoručených snímok minimapy smie naraz ležať v priečinku fronty.
/// Vlastný strop, NEZDIEĽANÝ s Canvasom (`zber_fronta::MAX_CAKAJUCICH`) —
/// zdieľaný by znamenal, že plný front jedného zberu zastaví aj druhý.
/// Rovnaké číslo ako `MAX_ZA_DEN`: pri výpadku servera na celý deň sa nič
/// nezahadzuje (pravidlo „Nič nemaž"), len sa prestane pridávať.
pub const MAX_CAKAJUCICH: usize = 2000;

/// Deň zo zberu, pre ten istý počet snímok za deň ako Canvas. Nie je dôvod
/// mať inú definíciu dňa pre dva zbery na tom istom počítači.
pub fn den_z_casu(unix_s: u64) -> u64 {
    crate::zber_fronta::den_z_casu(unix_s)
}

/// Koľko snímok minimapy už dnes odišlo — vlastná inštancia, samostatný
/// súbor na disku vedľa Canvasovho (appka ich nesmie zlúčiť).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DennyStav {
    pub den: u64,
    pub pocet: u32,
}

impl DennyStav {
    /// Posunie stav na daný deň a vráti, či sa ešte smie zbierať.
    pub fn smie_zbierat(&mut self, unix_s: u64) -> bool {
        self.posun_na_den(unix_s);
        self.pocet < MAX_ZA_DEN
    }

    /// Zaráta jednu odfotenú snímku — volá sa, až keď snímka naozaj vznikla.
    pub fn zapocitaj(&mut self, unix_s: u64) {
        self.posun_na_den(unix_s);
        self.pocet = self.pocet.saturating_add(1);
    }

    fn posun_na_den(&mut self, unix_s: u64) {
        let dnes = den_z_casu(unix_s);
        if self.den != dnes {
            self.den = dnes;
            self.pocet = 0;
        }
    }

    /// Koľko ešte dnes ostáva — do denníka a do hlásenia appky.
    pub fn zostava(&self) -> u32 {
        MAX_ZA_DEN.saturating_sub(self.pocet)
    }
}

/// Smie do fronty na disku pribudnúť ďalšia snímka?
pub fn smie_pribudnut(cakajucich: usize) -> bool {
    cakajucich < MAX_CAKAJUCICH
}

/// Ako dlho počkať pred ďalším pokusom o odoslanie fronty — rovnaká krivka
/// ako `zber_fronta::odstup_po_zlyhani` (vlastná kópia, lebo zadanie
/// vymenúva túto funkciu ako súčasť tohto modulu, nech je `minimapa_fronta`
/// sebestačný a testovateľný bez závislosti na Canvase).
pub fn odstup_po_zlyhani(pocet_zlyhani: u32) -> u64 {
    const ZACIATOK_S: u64 = 5;
    const STROP_S: u64 = 300;
    if pocet_zlyhani == 0 {
        return 0;
    }
    let exponent = (pocet_zlyhani - 1).min(16);
    ZACIATOK_S.saturating_mul(1u64 << exponent).min(STROP_S)
}

/// Meno súboru jednej snímky vo fronte. Prefix `minimapa-` (nie `canvas-`),
/// aby `je_subor_fronty` nikdy neprehliadol ani neprevzal cudzí súbor.
/// Nesie aj režim — pri obnove appky po páde appka z mena pozná, čo bolo vo
/// fronte, bez nutnosti čítať `meta.jsonl`.
pub fn nazov_snimky(rezim: RezimMinimapy, unix_s: u64, poradie: u64) -> String {
    format!("minimapa-{}-{unix_s:010}-{:04}.jpg", rezim.ako_parameter(), poradie % 10_000)
}

/// Je to súbor fronty minimapy?
pub fn je_subor_fronty(nazov: &str) -> bool {
    nazov.starts_with("minimapa-") && nazov.ends_with(".jpg")
}

// ── 5. Kód zóny (krok 62/3) ──

/// „Kód zóny" z kontraktu (`minimapa-api-kontrakt.md`, parameter `mapa`) —
/// posledný segment `InMainLevelPath`, napr. `SD_DuiLongJuQiang211`. Tracker
/// (`Beh::zona`) nesie CELÚ cestu (`/Game/Art/Maps/01SD/XZ_Foo200/XZ_Foo200`),
/// appka pred odoslaním posiela len tento posledný kus — **nie** `LevelId`
/// (62/0: jedna zóna má 6–8 `LevelId` nad tou istou geometriou).
pub fn kod_zony(cesta: &str) -> String {
    cesta.rsplit('/').next().unwrap_or(cesta).to_string()
}

// ── 4. `dost_dat` — kedy sa appka prestane pýtať/fotiť danú mapu ──

/// Ako dlho platí odpoveď `GET /api/minimapa/stav` pre jednu mapu, kým sa
/// appka spýta znova. Zadanie: „s časom platnosti (neptať sa á 2 s)" — 5
/// minút je dosť rýchle na to, aby appka zbadala čerstvo dosiahnutý strop
/// ešte v tom istom hernom sedení, a dosť zriedkavé, aby to nebolo ďalšie
/// HTTP volanie popri tikovaní minimapy.
pub const PLATNOST_STAVU_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq)]
struct StavMapyZoServera {
    dost_dat: bool,
    aktualizovane_ms: u64,
}

/// Runtime stav „načo sa tejto mape ešte oplatí fotiť" — kombinuje
/// (a) periodický `GET /api/minimapa/stav` (predpočítané, platí niekoľko
/// minút) a (b) priamu odpoveď `POST /api/minimapa` s `prijate: false`,
/// ktorá sa pamätá len DO KONCA AKTUÁLNEHO BEHU (iný `level_uid` = iný
/// pokus, mohlo sa medzitým zmeniť aj to, čo server považuje za dosť dát).
/// Zámerne sa neukladá na disk — appka ho stavia nanovo pri každom štarte.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DostDatStav {
    zo_servera: HashMap<String, StavMapyZoServera>,
    odmietnute_v_tomto_behu: HashSet<String>,
}

impl DostDatStav {
    /// Má appka zavolať `GET /api/minimapa/stav` znova pre túto mapu, alebo
    /// je uložený výsledok ešte čerstvý?
    pub fn ma_sa_opytat_servera(&self, mapa: &str, teraz_ms: u64) -> bool {
        match self.zo_servera.get(mapa) {
            None => true,
            Some(s) => teraz_ms.saturating_sub(s.aktualizovane_ms) >= PLATNOST_STAVU_MS,
        }
    }

    /// Zapíše čerstvú odpoveď `GET /api/minimapa/stav` pre jednu mapu.
    pub fn uloz_stav_servera(&mut self, mapa: &str, dost_dat: bool, teraz_ms: u64) {
        self.zo_servera.insert(mapa.to_string(), StavMapyZoServera { dost_dat, aktualizovane_ms: teraz_ms });
    }

    /// `POST /api/minimapa` vrátil `prijate: false` — appka si to pre túto
    /// mapu zapamätá, nech sa do konca behu nepýta znova zbytočne.
    pub fn zapamataj_odmietnutie(&mut self, mapa: &str) {
        self.odmietnute_v_tomto_behu.insert(mapa.to_string());
    }

    /// Nový beh (appka rozpoznala iný `level_uid`) — odmietnutia z
    /// predošlého behu už neplatia, len uložený stav zo servera ostáva.
    pub fn novy_beh(&mut self) {
        self.odmietnute_v_tomto_behu.clear();
    }

    /// Má appka vôbec skúšať fotiť túto mapu?
    pub fn ma_sa_fotit_mapu(&self, mapa: &str) -> bool {
        if self.odmietnute_v_tomto_behu.contains(mapa) {
            return false;
        }
        !matches!(self.zo_servera.get(mapa), Some(s) if s.dost_dat)
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    // ── ROI ──

    #[test]
    fn predvolene_roi_su_v_platnom_ramci() {
        for roi in [ROI_ROH_PREDVOLENE, ROI_STRED_PREDVOLENE] {
            assert!(roi.x >= 0.0 && roi.x <= 1.0);
            assert!(roi.y >= 0.0 && roi.y <= 1.0);
            assert!(roi.x + roi.w <= 1.0 + 1e-9);
            assert!(roi.y + roi.h <= 1.0 + 1e-9);
            assert!(roi.w > 0.0 && roi.h > 0.0, "ROI nesmie mat nulovu plochu");
        }
    }

    #[test]
    fn roi_normalizuj_oreze_zaporne_a_presahujuce_hodnoty() {
        let mut r = RoiMinimapy { x: -0.2, y: -0.1, w: 2.0, h: 2.0 };
        r.normalizuj();
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.w, 1.0);
        assert_eq!(r.h, 1.0);
    }

    #[test]
    fn roi_normalizuj_zmensi_sirku_aby_nepresiahla_okno() {
        let mut r = RoiMinimapy { x: 0.8, y: 0.9, w: 0.5, h: 0.5 };
        r.normalizuj();
        assert!((r.x + r.w - 1.0).abs() < 1e-9);
        assert!((r.y + r.h - 1.0).abs() < 1e-9);
    }

    #[test]
    fn roi_ako_parameter_ma_tvar_z_kontraktu() {
        let r = RoiMinimapy { x: 0.25, y: 0.125, w: 0.5, h: 0.75 };
        assert_eq!(r.ako_parameter(), "0.2500,0.1250,0.5000,0.7500");
    }

    #[test]
    fn rezim_ako_parameter_zodpoveda_kontraktu() {
        assert_eq!(RezimMinimapy::Roh.ako_parameter(), "roh");
        assert_eq!(RezimMinimapy::Stred.ako_parameter(), "stred");
    }

    #[test]
    fn rezim_z_textu_je_zhovievavy_na_velkost_pismen_a_medzery() {
        assert_eq!(RezimMinimapy::z_textu(" Stred "), Some(RezimMinimapy::Stred));
        assert_eq!(RezimMinimapy::z_textu("ROH"), Some(RezimMinimapy::Roh));
        assert_eq!(RezimMinimapy::z_textu("nezmysel"), None);
    }

    #[test]
    fn roi_sada_pre_rezim_vyberie_spravnu_dvojicu() {
        let s = RoiSada::default();
        assert_eq!(s.pre_rezim(RezimMinimapy::Roh), ROI_ROH_PREDVOLENE);
        assert_eq!(s.pre_rezim(RezimMinimapy::Stred), ROI_STRED_PREDVOLENE);
    }

    #[test]
    fn roi_sada_normalizuj_opravi_oba_rezimy_nezavisle() {
        let mut s = RoiSada { roh: RoiMinimapy { x: 2.0, y: 0.0, w: 1.0, h: 0.1 }, stred: ROI_STRED_PREDVOLENE };
        s.normalizuj();
        assert_eq!(s.roh.x, 1.0);
        assert_eq!(s.roh.w, 0.0);
        assert_eq!(s.stred, ROI_STRED_PREDVOLENE, "druhy rezim sa nemal dotknut");
    }

    #[test]
    fn roi_sada_prezije_stary_json_bez_poli() {
        let s: RoiSada = serde_json::from_str("{}").unwrap();
        assert_eq!(s, RoiSada::default());
    }

    // ── ticker ──

    #[test]
    fn mimo_behu_mapy_sa_nefoti_nikdy() {
        assert!(!ma_sa_fotit(false, u64::MAX));
        assert!(!ma_sa_fotit(false, ODSTUP_TIKU_MS));
    }

    #[test]
    fn v_behu_sa_foti_az_po_odstupe() {
        assert!(!ma_sa_fotit(true, 0));
        assert!(!ma_sa_fotit(true, ODSTUP_TIKU_MS - 1));
        assert!(ma_sa_fotit(true, ODSTUP_TIKU_MS));
    }

    #[test]
    fn prve_volanie_v_behu_sa_odfoti() {
        assert!(ma_sa_fotit(true, u64::MAX));
    }

    // ── denný strop a fronta ──

    #[test]
    fn denny_limit_zastavi_na_strope() {
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
        assert!(s.smie_zbierat(cas + 86_400));
        assert_eq!(s.pocet, 0);
    }

    #[test]
    fn denny_stav_prezije_stary_json_bez_poli() {
        let s: DennyStav = serde_json::from_str("{}").unwrap();
        assert_eq!(s, DennyStav::default());
    }

    #[test]
    fn denny_stav_minimapy_je_nezavisly_od_canvasu() {
        // Dve rôzne štruktúry, dve rôzne premenné — kompilátor sám zaručí,
        // že appka ich nemôže omylom zliať do jednej inštancie.
        let mut minimapa = DennyStav::default();
        let mut canvas = crate::zber_fronta::DennyStav::default();
        let cas = 1_760_000_000;
        for _ in 0..MAX_ZA_DEN {
            minimapa.zapocitaj(cas);
        }
        assert!(!minimapa.smie_zbierat(cas));
        assert!(canvas.smie_zbierat(cas), "canvas strop sa nesmel vycerpat so zberom minimapy");
    }

    #[test]
    fn plna_fronta_uz_nepriberie_ale_nic_nemaze() {
        assert!(smie_pribudnut(0));
        assert!(smie_pribudnut(MAX_CAKAJUCICH - 1));
        assert!(!smie_pribudnut(MAX_CAKAJUCICH));
    }

    #[test]
    fn odstup_rastie_a_zastavi_sa_na_piatich_minutach() {
        assert_eq!(odstup_po_zlyhani(0), 0);
        assert_eq!(odstup_po_zlyhani(1), 5);
        assert_eq!(odstup_po_zlyhani(2), 10);
        assert_eq!(odstup_po_zlyhani(10), 300);
        assert_eq!(odstup_po_zlyhani(u32::MAX), 300);
    }

    #[test]
    fn mena_suborov_nesu_rezim_a_su_zoradene_podla_casu() {
        let a = nazov_snimky(RezimMinimapy::Stred, 1_760_000_000, 0);
        let b = nazov_snimky(RezimMinimapy::Stred, 1_760_000_000, 1);
        let c = nazov_snimky(RezimMinimapy::Stred, 1_760_000_001, 0);
        assert!(a < b && b < c, "{a} {b} {c}");
        assert_eq!(a, "minimapa-stred-1760000000-0000.jpg");
        assert_eq!(nazov_snimky(RezimMinimapy::Roh, 1, 0), "minimapa-roh-0000000001-0000.jpg");
    }

    #[test]
    fn cudzie_subory_a_canvas_subory_sa_do_fronty_minimapy_nepocitaju() {
        assert!(je_subor_fronty("minimapa-stred-1760000000-0000.jpg"));
        assert!(!je_subor_fronty("canvas-1760000000-0000.jpg"));
        assert!(!je_subor_fronty("minimapa-stred-1760000000-0000.jpg.tmp"));
        assert!(!je_subor_fronty("poznamka.txt"));
    }

    // ── kód zóny ──

    #[test]
    fn kod_zony_berie_posledny_segment_cesty() {
        assert_eq!(
            kod_zony("/Game/Art/Maps/01SD/XZ_YuJinZhiXiBiNanSuo200/XZ_YuJinZhiXiBiNanSuo200"),
            "XZ_YuJinZhiXiBiNanSuo200"
        );
        assert_eq!(kod_zony("/Game/Art/Maps/Town/Town"), "Town");
    }

    #[test]
    fn kod_zony_bez_lomky_vrati_cely_text() {
        assert_eq!(kod_zony("SamostatnyNazov"), "SamostatnyNazov");
        assert_eq!(kod_zony(""), "");
    }

    // ── dost_dat ──

    #[test]
    fn nova_mapa_sa_najprv_vzdy_opyta_servera() {
        let s = DostDatStav::default();
        assert!(s.ma_sa_opytat_servera("XZ_Mapa", 1_000));
        assert!(s.ma_sa_fotit_mapu("XZ_Mapa"), "bez informacie zo servera sa predpoklada, ze sa este oplati fotit");
    }

    #[test]
    fn cerstvy_stav_servera_sa_este_nepyta_znova() {
        let mut s = DostDatStav::default();
        s.uloz_stav_servera("XZ_Mapa", false, 1_000);
        assert!(!s.ma_sa_opytat_servera("XZ_Mapa", 1_000 + PLATNOST_STAVU_MS - 1));
        assert!(s.ma_sa_opytat_servera("XZ_Mapa", 1_000 + PLATNOST_STAVU_MS));
    }

    #[test]
    fn dost_dat_zo_servera_vypne_fotenie_tej_mapy() {
        let mut s = DostDatStav::default();
        s.uloz_stav_servera("XZ_Mapa", true, 1_000);
        assert!(!s.ma_sa_fotit_mapu("XZ_Mapa"));
        // iná mapa nie je tým ovplyvnená
        assert!(s.ma_sa_fotit_mapu("XZ_Ina"));
    }

    #[test]
    fn odmietnutie_z_postu_plati_len_do_konca_behu() {
        let mut s = DostDatStav::default();
        s.zapamataj_odmietnutie("XZ_Mapa");
        assert!(!s.ma_sa_fotit_mapu("XZ_Mapa"));
        s.novy_beh();
        assert!(s.ma_sa_fotit_mapu("XZ_Mapa"), "novy beh (iny level_uid) dostane novu sancu");
    }

    #[test]
    fn novy_beh_sa_nedotkne_stavu_zo_servera() {
        let mut s = DostDatStav::default();
        s.uloz_stav_servera("XZ_Mapa", true, 1_000);
        s.zapamataj_odmietnutie("XZ_Ina");
        s.novy_beh();
        assert!(!s.ma_sa_fotit_mapu("XZ_Mapa"), "stav zo servera prezije zmenu behu");
        assert!(s.ma_sa_fotit_mapu("XZ_Ina"), "odmietnutie v behu sa po novom behu zrusi");
    }
}
