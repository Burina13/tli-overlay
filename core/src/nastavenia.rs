use crate::minimapa_fronta::{RezimMinimapy, RoiSada};
use crate::minimapa_vzhlad::VzhladTrasy;
use crate::rozlozenie::Rozlozenie;
use serde::{Deserialize, Serialize};

/// Poloha a veľkosť okna na ploche vo fyzických pixeloch.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PolohaOkna {
    pub x: i32,
    pub y: i32,
    pub sirka: u32,
    pub vyska: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Nastavenia {
    pub url_servera: String,
    pub kluc: String,
    pub hotkey: String,
    /// Druhá skratka pre Frozen Canvas (TLI 4b) — iná obrazovka hry, iný
    /// modul API (`?modul=canvas`), preto samostatný hotkey namiesto
    /// prepínača v nastaveniach (hráč je uprostred hrania, nemá čas
    /// prekliknúť si režim).
    pub hotkey_canvas: String,
    pub autoskrytie_s: u64,
    /// TLI 58 (`2026-10-19-2134-mcp.md`) — ako zhasnúť panel rady po analýze.
    /// `true` (predvolené) = až po PRVOM kliku myšou kdekoľvek; `false` =
    /// po `autoskrytie_s` sekundách, ako to bolo do 19.10.2026. Richard
    /// 19.10.2026: „po stlačení solvera nech mi svieti overlay, kým nespravím
    /// klik myškou, až potom zhasne, namiesto času." Pri režime kliku je
    /// `autoskrytie_s` nepoužité — poistkou je pevných
    /// `SKRYTIE_KLIKOM_POISTKA_S` sekúnd (`main.rs`), nie nastavenie hráča:
    /// nula v `autoskrytie_s` znamená „nikdy sám", a to by pri zlyhanej
    /// detekcii kliku nechalo overlay svietiť navždy.
    pub skryt_radu_klikom: bool,
    pub proces_hry: String,
    pub zvuk: bool,
    /// Zber dát (TLI 4c, `2026-10-15-0028-mcp.md`): keď zapnuté, hotkey pre
    /// Frozen Canvas (`hotkey_canvas`) NEukazuje radu solvera, len odfotí,
    /// pošle na server (ten uloží VŽDY, bez ohľadu na istotu — nezmenené
    /// správanie servera z TLI 4b) a ukáže počítadlo „uložené #N". Žiadny
    /// tretí hotkey — zámerne, nech sa Richard uprostred hrania nemusí učiť
    /// novú klávesu, len si raz zaškrtne prepínač v Nastaveniach.
    pub zber_dat_zapnute: bool,
    /// Vzhľad hlavného okna: `svetla` alebo `tmava`. **Predvolená je svetlá** —
    /// Richard 15.10.2026: „nemá to byť celé tmavé". Prepínač je v hlavičke okna.
    pub tema: String,
    /// Farm tracker (TLI 11/11b): cesta k hernému logu `UE_game.log`. Prázdna
    /// hodnota znamená „nájdi ju sám" — appka skúsi bežné inštalačné cesty.
    pub cesta_logu: String,
    /// Či sa má log vôbec čítať. Vypnuté = tracker nerobí nič a nič nečíta.
    pub tracker_zapnuty: bool,
    /// Odpočítať z hodnoty dropov 12,5 % daň z predaja na trhu. Na samotné FE
    /// sa neuplatňuje nikdy (daň sa platí z predaja, nie z meny).
    pub tracker_dan: bool,
    /// Posielať výsledky máp partii na `tli.tvojtatko.sk` (TLI 11, krok 5).
    /// **Predvolene VYPNUTÉ** — je to zdieľanie vlastných čísel s partiou a to
    /// si musí hráč zapnúť sám, nie sa to na neho nasťahovať s aktualizáciou.
    pub tracker_sync: bool,
    /// Malý panel nad hrou s FE tejto mapy, časom a FE/h (TLI 11, krok 5).
    /// Predvolene vypnutý — overlay nad hrou je rušivý a nie každý ho chce.
    pub mini_overlay: bool,
    /// Pripomienka „Enable Log" na začiatku každej session (TLI 11d). Hra si
    /// toto nastavenie nepamätá medzi spusteniami — **predvolene ZAPNUTÉ**,
    /// na rozdiel od `tracker_sync`/`mini_overlay`: bez pripomienky by sa
    /// nevytrackovaná session zistila až spätne, keď je neskoro niečo robiť.
    pub pripominat_enable_log: bool,
    /// Zdieľanie CIEN s partiou (TLI 20). **Predvolene ZAPNUTÉ** — na rozdiel
    /// od `tracker_sync` (ktorý zdieľa tvoje výsledky a zárobky, čo je osobné)
    /// tu ide o cenu veci na verejnom in-game trhu. Nie je to tvoje číslo,
    /// a bez zdieľania by hráč, ktorý si nič nevyhľadal, videl behy za 0 FE —
    /// presne to je problém, kvôli ktorému TLI 20 vzniklo (zadanie bod 3).
    pub ceny_sync: bool,
    /// Denný upload surových logov a DB exportu na server (TLI 14). **Predvolene
    /// ZAPNUTÉ** — na rozdiel od `tracker_sync`/`mini_overlay` ide o náhradu
    /// ručného kopírovania logu na zdieľaný disk (zadanie `2026-10-16-1208-mcp.md`),
    /// nie o nové zdieľanie niečoho, čo hráč doteraz pre seba držal: Richard
    /// logy aj tak posiela, len ručne. Posiela sa len VLASTNÝ log a DB export,
    /// nikdy cudzie dáta.
    pub upload_logov: bool,
    /// Uložená poloha/veľkosť/priehľadnosť okien nad hrou (TLI 21) — panel
    /// F8/F9, mini-panel trackera, toast pripomienky. Prázdne pre každý panel
    /// a monitor, kým ho hráč prvýkrát nepresunie v režime „Upraviť
    /// rozloženie“ (Ctrl+F8).
    pub rozlozenie: Rozlozenie,
    /// Poloha a veľkosť HLAVNÉHO okna appky (záložky), vo fyzických pixeloch
    /// plochy (TLI 25, `2026-10-17-1242-mcp.md` bod 5). Do TLI 25 sa okno pri
    /// každom otvorení vycentrovalo a hráč si ho musel zakaždým odtiahnuť
    /// nabok. `None` = ešte ním nikto nepohol, okno sa vycentruje ako doteraz.
    ///
    /// Nie je to súčasť `Rozlozenie`: tam sú panely NAD HROU, ukladané ako
    /// zlomok okna hry; toto je obyčajné okno na ploche a zlomok okna hry
    /// preň nedáva zmysel (hra ani nemusí bežať).
    pub okno_hlavne: Option<PolohaOkna>,
    /// Ceny z obrazovky trhu (TLI 22, `2026-10-16-1705-mcp.md`): keď appka v
    /// logu uvidí otvorenie `AuctionHouseShowItemGoodV2` (SS14 do logu ceny
    /// nepíše, len lifecycle stránky), sama odfotí a pošle na `?modul=ceny`.
    /// **Predvolene ZAPNUTÉ** — rovnaký dôvod ako `ceny_sync`: je to verejná
    /// cena na trhu, nie osobný údaj, a cieľom zadania je „ceny bez ručnej
    /// práce". Žiadny nový povinný hotkey (`hotkey_trh` nižšie je len
    /// nepovinný ručný fallback) — zámerne, rovnaká filozofia ako
    /// `zber_dat_zapnute` vyššie.
    pub ceny_obrazovka_zapnute: bool,
    /// Skratka pre RUČNÉ vyvolanie analýzy trhu (zadanie bod 2, „alebo na
    /// hotkey") — napr. keď automatická detekcia z logu zlyhá (iný jazyk
    /// klienta hry, zmena textu stránky).
    ///
    /// **Predvolená hodnota je od TLI 39 (`2026-10-18-1613-mcp.md`) „F10",
    /// NIE prázdny reťazec.** Pôvodne prázdna predvoľala appku do stavu, kde
    /// hráč nevidel ŽIADNU klávesu pre tento modul nikde (ani v Nastaveniach,
    /// ani v toaste pri štarte) — presne to sa 18.10.2026 podpísalo pod
    /// nejasný incident „F8 nefunguje": Tartarus hotkey bol omylom prepísaný
    /// na holú klávesu a hráč nemal sa o čom uistiť. `normalizuj()` preto
    /// prázdny reťazec vždy premení na „F10" — pole teda už nikdy neostane
    /// bez vlastnej hodnoty, aj keď appka dostane starý súbor nastavení alebo
    /// hráč pole v UI vyprázdni. Konflikt s `hotkey`/`hotkey_canvas` (rovnaká
    /// klávesa pre dva moduly) rieši appka osobitne pri registrácii
    /// (`src-tauri/src/main.rs::konfliktne_moduly`), nie tu — táto štruktúra
    /// o ostatných dvoch poliach nič nevie.
    pub hotkey_trh: String,
    /// Automatické učenie Frozen Canvasu na pozadí (TLI 60,
    /// `2026-10-19-2135-mcp-2.md`). Keď je zapnuté a appka na obrazovke
    /// rozpozná otvorený Frozen Canvas, po KAŽDOM kliku myšou odfotí okno
    /// hry a na pozadí snímku pošle na server na učenie (`?modul=canvas&zber=1`).
    /// Richardovi sa pritom NIČ neukazuje a na nič sa nečaká — je to zber
    /// podkladov pre nočné učenie solvera, nie rada.
    ///
    /// **Predvolene ZAPNUTÉ** — priamo zo zadania Richarda 19.10.2026:
    /// „kľudne nech si všetko, čo spravím, fotí a na pozadí pošle agentovi
    /// na učenie". To ho odlišuje od `zber_dat_zapnute` vyššie, ktorý je
    /// RUČNÝ (klávesa pre Canvas), predvolene vypnutý a navyše mení, čo appka
    /// ukáže v paneli rady. Sú to dve nezávislé veci a nesmú sa zliať.
    pub ucenie_canvas_zapnute: bool,
    /// CURRENT INVENTORY (TLI 47, `2026-10-19-0744-mcp.md` bod 2) — zoznam
    /// `ConfigBaseId`, ktoré hráč tlačidlom „Hide" schoval zo zoznamu na
    /// predaj (napr. veci, čo si chce nechať, nie predať). Appka nič
    /// NEMAŽE ani neposiela — je to len filter vo vlastnom zobrazení.
    /// Vzhľad trasy nad minimapou (TLI 62/2b, doplnok 1 k zadaniu
    /// `2026-10-20-0158-mcp.md` bod 4): opacita čiary aj celej vrstvy
    /// (0–100 %), hrúbka a farba čiary. Richard 20.10.2026: „nech sa tam dá
    /// nastaviť opacita."
    ///
    /// Je to **samostatné pole, nie súčasť `rozlozenie`** — `Rozlozenie::
    /// priehladnost` platí pre všetky panely nad hrou naraz a má dolnú medzu
    /// 0,3, kým tu je nula platná hodnota (dôvod je pri `VzhladTrasy`).
    /// Čiaru samotnú kreslí až krok 62/8; tieto hodnoty vznikajú skôr, aby
    /// ich nebolo treba dopĺňať do hotového kresliaceho kódu.
    pub vzhlad_trasy: VzhladTrasy,
    pub batoh_skryte_polozky: Vec<u32>,
    /// ROI minimapy pre oba režimy (TLI 62/2, `minimapa_fronta::RoiSada`) —
    /// „štyri čísla na režim", prepísateľné bez nového buildu. Predvolené
    /// hodnoty sú zatiaľ PRACOVNÁ HYPOTÉZA (žiadna snímka minimapy ešte
    /// nebola zozbieraná) a opravia sa tu, keď prídu prvé reálne dáta.
    pub minimapa_roi: RoiSada,
    /// Zber snímok minimapy na stavbu knižnice trás (TLI 62/3, zadanie
    /// `inbox/2026-10-20-0600-agent.md`). **Predvolene VYPNUTÉ** — rovnaký
    /// precedens ako `zber_dat_zapnute`: appka nesmie fotiť nikoho, kto si to
    /// sám nezapol (ToS, `minimapa-api-kontrakt.md` § „Richard rozhodol").
    /// Na rozdiel od `ucenie_canvas_zapnute` (ktoré je predvolene ZAPNUTÉ,
    /// lebo Richard to tak výslovne chcel pre Canvas) tu zadanie žiadnu takú
    /// vetu nemá — zber minimapy je ručné rozhodnutie hráča.
    pub minimapa_zber_zapnute: bool,
    /// Ktorý režim minimapy appka sleduje. Appka to NEVIE zistiť sama (nie je
    /// to niečo, čo appka môže čítať z obrazovky bez OCR navyše) — hráč si ho
    /// vyberie v Nastaveniach. Richard používa „stred" — predvolené.
    pub minimapa_rezim: RezimMinimapy,
}

impl Default for Nastavenia {
    fn default() -> Self {
        Self {
            url_servera: "https://tli.tvojtatko.sk".to_string(),
            kluc: "".to_string(),
            hotkey: "F8".to_string(),
            hotkey_canvas: "F9".to_string(),
            autoskrytie_s: 20,
            skryt_radu_klikom: true,
            proces_hry: "Torchlight Infinite, Torchlight".to_string(),
            zvuk: false,
            zber_dat_zapnute: false,
            ucenie_canvas_zapnute: true,
            tema: "svetla".to_string(),
            cesta_logu: String::new(),
            tracker_zapnuty: true,
            tracker_dan: false,
            tracker_sync: false,
            mini_overlay: false,
            pripominat_enable_log: true,
            ceny_sync: true,
            upload_logov: true,
            rozlozenie: Rozlozenie::default(),
            okno_hlavne: None,
            ceny_obrazovka_zapnute: true,
            hotkey_trh: "F10".to_string(),
            vzhlad_trasy: VzhladTrasy::default(),
            batoh_skryte_polozky: Vec::new(),
            minimapa_roi: RoiSada::default(),
            minimapa_zber_zapnute: false,
            minimapa_rezim: RezimMinimapy::Stred,
        }
    }
}

impl Nastavenia {
    /// Normalizácia zabraňuje chybám pri ukladaní a spájaní URL adries, 
    /// a garantuje platné rozsahy pre bezpečnú prevádzku klienta.
    pub fn normalizuj(&mut self) {
        let upravena_url = self.url_servera.trim().trim_end_matches('/').to_string();
        if upravena_url.is_empty() {
            self.url_servera = "https://tli.tvojtatko.sk".to_string();
        } else {
            self.url_servera = upravena_url;
        }

        self.kluc = self.kluc.trim().to_string();
        self.hotkey = self.hotkey.to_uppercase();
        self.hotkey_canvas = self.hotkey_canvas.to_uppercase();
        self.hotkey_trh = self.hotkey_trh.trim().to_uppercase();
        // TLI 39, bod 2: prázdny hotkey_trh nesmie appku nechať bez vlastnej
        // klávesy pre tento modul — padá sa na predvolenú „F10", nie na
        // tichú nezaregistrovanú hodnotu (pozri komentár pri poli vyššie).
        if self.hotkey_trh.is_empty() {
            self.hotkey_trh = "F10".to_string();
        }

        if self.autoskrytie_s > 600 {
            self.autoskrytie_s = 600;
        }

        // Neznáma hodnota témy nesmie nechať okno bez farieb — padá sa na svetlú.
        let t = self.tema.trim().to_lowercase();
        self.tema = if t == "tmava" { t } else { "svetla".to_string() };
        self.cesta_logu = self.cesta_logu.trim().to_string();

        // TLI 62/2b — ručne upravený alebo starý `nastavenia.json` nesmie
        // nechať trasu neviditeľnú ani s rozbitou farbou.
        self.vzhlad_trasy.normalizuj();

        // TLI 62/2 — ROI minimapy nesmie po ručnej úprave presahovať okno hry.
        self.minimapa_roi.normalizuj();
    }

    /// Bezpečné zostrojenie adresy zamedzuje chybám v smerovaní požadaviek
    /// (napr. dvojité lomky po zlom zápise v konfiguračnom súbore).
    pub fn adresa(&self, cesta: &str) -> String {
        format!("{}/{}", self.url_servera, cesta.trim_start_matches('/'))
    }

    /// Skratka pre priamy prístup k endpointu, ktorý vracia výpočet ťahov na doske.
    pub fn adresa_analyza(&self) -> String {
        self.adresa("/api/analyza")
    }

    /// Frozen Canvas (TLI 4b) — ten istý endpoint, iný modul.
    pub fn adresa_analyza_canvas(&self) -> String {
        self.adresa("/api/analyza?modul=canvas")
    }

    /// Automatický zber na učenie (TLI 60) — ten istý endpoint ako
    /// `adresa_analyza_canvas`, navyše `zber=1`. Server si tým označí riadok
    /// ako „prišlo to samo z pozadia", nie z hráčovho stlačenia klávesy
    /// (`stacks/games/tli-web/app.py`, `vysledok["zber"]`).
    pub fn adresa_analyza_canvas_zber(&self) -> String {
        self.adresa("/api/analyza?modul=canvas&zber=1")
    }

    /// Ceny z obrazovky trhu (TLI 22) — ten istý endpoint, iný modul.
    pub fn adresa_analyza_ceny(&self) -> String {
        self.adresa("/api/analyza?modul=ceny")
    }

    /// Príjem výsledkov máp pre rebríček partie (TLI 11, krok 5).
    pub fn adresa_tracker_sync(&self) -> String {
        self.adresa("/api/tracker/sync")
    }

    /// Zdieľané ceny partie (TLI 20) — ten istý endpoint pre upload (POST)
    /// aj sťahovanie (GET).
    pub fn adresa_ceny(&self) -> String {
        self.adresa("/api/ceny")
    }

    /// Skratka pre kontrolu dostupnosti a stavu serverového API.
    pub fn adresa_zdravie(&self) -> String {
        self.adresa("/api/zdravie")
    }

    /// Denný upload surových logov a DB exportu (TLI 14) — ten istý endpoint
    /// pre oba typy (`typ=log`/`typ=db_export`), rozlíšené v tele requestu.
    pub fn adresa_tracker_upload(&self) -> String {
        self.adresa("/api/tracker/upload")
    }

    /// Zber snímok minimapy (TLI 62/3) — vlastný endpoint, vlastný
    /// rate-limit kôš na serveri (`limity.minimapa_za_minutu`), nie
    /// `/api/analyza*` (ten volá vision model, toto len uloží na disk).
    pub fn adresa_minimapa(&self) -> String {
        self.adresa("/api/minimapa")
    }

    /// Stav nazbieraných dát za mapu (TLI 62/3) — appka sa pýta raz za čas
    /// (`DostDatStav::ma_sa_opytat_servera`), nie pri každej snímke.
    pub fn adresa_minimapa_stav(&self) -> String {
        self.adresa("/api/minimapa/stav")
    }

    /// Poskytuje varovanie používateľovi ešte predtým, ako sa aplikácia pokúsi spojiť,
    /// čím šetríme sieť a poskytujeme zrozumiteľnú odozvu na chýbajúce úvodné kroky.
    pub fn chyba_konfiguracie(&self) -> Option<String> {
        if self.kluc.is_empty() {
            return Some("Nemáš vyplnený osobný kľúč. Získaš ho na tli.tvojtatko.sk/kluc alebo príkazom /tli kluc v Discorde.".to_string());
        }
        if !self.url_servera.starts_with("https://") {
            return Some("Adresa servera musí začínať https://".to_string());
        }
        None
    }

    /// Zaisťuje, že chybný alebo prázdny lokálny súbor nezhodí celú aplikáciu,
    /// ale prepne sa do predvoleného, funkčného stavu, aby klient prežil.
    pub fn z_jsonu(text: &str) -> Self {
        let mut n: Nastavenia = serde_json::from_str(text).unwrap_or_default();
        n.normalizuj();
        n
    }

    /// Ukladá súbor v čitateľnom tvare, aby si ho technicky zdatnejší hráči mohli
    /// upraviť aj cez jednoduchý textový editor.
    pub fn do_jsonu(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn predvolene_hodnoty() {
        let n = Nastavenia::default();
        assert_eq!(n.url_servera, "https://tli.tvojtatko.sk");
        assert_eq!(n.kluc, "");
        assert_eq!(n.hotkey, "F8");
        assert_eq!(n.hotkey_canvas, "F9");
        assert_eq!(n.autoskrytie_s, 20);
        // TLI 58: predvolený režim je „skry až po kliku myšou", nie po čase.
        assert!(n.skryt_radu_klikom);
    }

    /// Staré `nastavenia.json` (spred TLI 58) pole `skryt_radu_klikom`
    /// nemajú — `#[serde(default)]` nad štruktúrou musí dosadiť `true`,
    /// nie `false` z `Default::default()` pre `bool`. Keby dosadil `false`,
    /// každému existujúcemu hráčovi by aktualizácia potichu nechala staré
    /// správanie a nová funkcia by sa u nikoho nezapla.
    #[test]
    fn stary_subor_bez_pola_dostane_rezim_kliku() {
        let n: Nastavenia = serde_json::from_str(r#"{"hotkey":"F8"}"#).unwrap();
        assert!(n.skryt_radu_klikom);
        assert_eq!(n.autoskrytie_s, 20);
    }

    #[test]
    fn adresa_analyza_canvas_nesie_modul() {
        let n = Nastavenia::default();
        assert_eq!(n.adresa_analyza_canvas(),
                  "https://tli.tvojtatko.sk/api/analyza?modul=canvas");
    }

    #[test]
    fn hotkey_canvas_sa_tiez_velkymi_pismenami() {
        let mut n = Nastavenia { hotkey_canvas: "f9".to_string(), ..Default::default() };
        n.normalizuj();
        assert_eq!(n.hotkey_canvas, "F9");
    }

    #[test]
    fn normalizacia_url_a_kluca() {
        let mut n = Nastavenia {
            url_servera: "  https://iny.server.sk/  ".to_string(),
            kluc: "  abc  ".to_string(),
            ..Default::default()
        };
        n.normalizuj();
        assert_eq!(n.url_servera, "https://iny.server.sk");
        assert_eq!(n.kluc, "abc");
    }

    #[test]
    fn adresa_nedava_dvojite_lomitka() {
        let n = Nastavenia::default();
        assert_eq!(n.adresa_analyza(), "https://tli.tvojtatko.sk/api/analyza");
    }

    #[test]
    fn chyby_konfiguracie_su_spravne() {
        let mut n = Nastavenia::default();
        assert!(n.chyba_konfiguracie().unwrap().contains("Nemáš vyplnený osobný kľúč"));
        
        n.kluc = "klucik".to_string();
        n.url_servera = "http://zlade.sk".to_string();
        assert!(n.chyba_konfiguracie().unwrap().contains("musí začínať https://"));
        
        n.url_servera = "https://tli.tvojtatko.sk".to_string();
        assert!(n.chyba_konfiguracie().is_none());
    }

    #[test]
    fn poskodeny_json_vrati_predvolene() {
        let n = Nastavenia::z_jsonu("{ zle : [");
        assert_eq!(n.url_servera, "https://tli.tvojtatko.sk");
    }

    #[test]
    fn synchronizacia_a_mini_overlay_su_predvolene_vypnute() {
        // Zdieľanie vlastných čísel s partiou aj panel nad hrou sú veci, ktoré
        // si hráč zapína sám — nesmú sa zapnúť samy pri aktualizácii.
        let n = Nastavenia::default();
        assert!(!n.tracker_sync);
        assert!(!n.mini_overlay);
        assert_eq!(n.adresa_tracker_sync(), "https://tli.tvojtatko.sk/api/tracker/sync");
        // TLI 20: zdieľanie CIEN je predvolene ZAPNUTÉ (na rozdiel od
        // zdieľania vlastných výsledkov) — cena veci na verejnom trhu nie je
        // osobný údaj a bez neho má hráč behy za 0 FE.
        assert!(n.ceny_sync);
        assert_eq!(n.adresa_ceny(), "https://tli.tvojtatko.sk/api/ceny");
    }

    #[test]
    fn upload_logov_je_predvolene_zapnute() {
        // TLI 14: náhrada ručného kopírovania logu na zdieľaný disk — na
        // rozdiel od `tracker_sync`/`mini_overlay` sa tu nič nové nezdieľa
        // s partiou, len posiela vlastný log tam, kam ho Richard aj tak
        // kopíruje ručne.
        let n = Nastavenia::default();
        assert!(n.upload_logov);
        assert_eq!(n.adresa_tracker_upload(), "https://tli.tvojtatko.sk/api/tracker/upload");
    }

    #[test]
    fn stare_nastavenia_bez_novych_poli_sa_nacitaju() {
        let n = Nastavenia::z_jsonu(r#"{"kluc":"abc","tracker_zapnuty":true}"#);
        assert!(!n.tracker_sync);
        assert!(!n.mini_overlay);
        assert_eq!(n.kluc, "abc");
    }

    #[test]
    fn zber_dat_zapnute_je_predvolene_vypnute() {
        let n = Nastavenia::default();
        assert!(!n.zber_dat_zapnute);
    }

    #[test]
    fn ucenie_canvas_je_predvolene_zapnute_a_je_to_ine_pole_nez_zber_dat() {
        // TLI 60: automatický zber na pozadí je predvolene ZAPNUTÝ (zadanie
        // Richarda), RUČNÝ zber klávesou ostáva predvolene vypnutý. Keby sa
        // tie dve polia niekedy zliali, prestane platiť jedno alebo druhé.
        let n = Nastavenia::default();
        assert!(n.ucenie_canvas_zapnute);
        assert!(!n.zber_dat_zapnute);
    }

    #[test]
    fn stary_json_bez_ucenia_canvas_sa_nacita_so_zapnutym_predvolenim() {
        // Hráč, ktorý appku používa od TLI 50, nemá toto pole v súbore —
        // `#[serde(default)]` mu ho musí doplniť ako zapnuté, inak by sa
        // zber u nikoho okrem čerstvých inštalácií nerozbehol.
        let n = Nastavenia::z_jsonu(r#"{"kluc":"abc","zber_dat_zapnute":true}"#);
        assert!(n.ucenie_canvas_zapnute);
        assert!(n.zber_dat_zapnute);
    }

    #[test]
    fn ucenie_canvas_sa_da_vypnut_a_prezije_round_trip() {
        let mut n = Nastavenia::default();
        n.ucenie_canvas_zapnute = false;
        let n2 = Nastavenia::z_jsonu(&n.do_jsonu());
        assert!(!n2.ucenie_canvas_zapnute);
    }

    #[test]
    fn adresa_zberu_nesie_modul_aj_priznak_zber() {
        let n = Nastavenia::default();
        assert_eq!(n.adresa_analyza_canvas_zber(),
                   "https://tli.tvojtatko.sk/api/analyza?modul=canvas&zber=1");
    }

    #[test]
    fn pripominat_enable_log_je_predvolene_zapnute() {
        // Na rozdiel od tracker_sync/mini_overlay — bez pripomienky by sa
        // vypnuté logovanie zistilo až spätne (TLI 11d).
        let n = Nastavenia::default();
        assert!(n.pripominat_enable_log);
    }

    #[test]
    fn stary_json_bez_pripomienky_sa_nacita_so_zapnutym_predvolenim() {
        let n = Nastavenia::z_jsonu(r#"{"kluc":"abc","hotkey":"F8"}"#);
        assert!(n.pripominat_enable_log);
        assert_eq!(n.kluc, "abc");
    }

    #[test]
    fn pripominat_enable_log_sa_da_vypnut_a_prezije_round_trip() {
        let mut n = Nastavenia::default();
        n.pripominat_enable_log = false;
        let json = n.do_jsonu();
        let n2 = Nastavenia::z_jsonu(&json);
        assert!(!n2.pripominat_enable_log);
    }

    #[test]
    fn stary_json_bez_zber_dat_pola_sa_nacita_s_vypnutym_predvolenim() {
        // Nastavenia z verzie pred TLI 4c (0.2.0) toto pole nemajú vôbec —
        // `#[serde(default)]` nesmie pri ich načítaní zlyhať.
        let n = Nastavenia::z_jsonu(r#"{"kluc":"abc","hotkey":"F8"}"#);
        assert!(!n.zber_dat_zapnute);
        assert_eq!(n.kluc, "abc");
    }

    #[test]
    fn round_trip_jsonu() {
        let mut n = Nastavenia::default();
        n.kluc = "test1234".to_string();
        let json = n.do_jsonu();
        let n2 = Nastavenia::z_jsonu(&json);
        assert_eq!(n, n2);
    }

    #[test]
    fn stare_nastavenia_bez_rozlozenia_sa_nacitaju_s_predvolenym() {
        // TLI 21: appka pred verziou s posúvaním okien pole `rozlozenie`
        // vôbec nemá — `#[serde(default)]` nesmie pri jej načítaní zlyhať.
        let n = Nastavenia::z_jsonu(r#"{"kluc":"abc","hotkey":"F8"}"#);
        assert_eq!(n.rozlozenie, crate::rozlozenie::Rozlozenie::default());
        assert_eq!(n.kluc, "abc");
    }

    #[test]
    fn ceny_obrazovka_je_predvolene_zapnuta_a_hotkey_trh_je_f10() {
        // TLI 22: verejná cena na trhu, rovnaký dôvod ako ceny_sync — zapnuté
        // predvolene. TLI 39: hotkey_trh má odteraz vlastnú predvolenú
        // hodnotu (predtým prázdny reťazec — pozri `hotkey_trh_prazdny_sa_normalizuje_na_f10`).
        let n = Nastavenia::default();
        assert!(n.ceny_obrazovka_zapnute);
        assert_eq!(n.hotkey_trh, "F10");
        assert_eq!(n.adresa_analyza_ceny(), "https://tli.tvojtatko.sk/api/analyza?modul=ceny");
    }

    #[test]
    fn stare_nastavenia_bez_ceny_obrazovka_sa_nacitaju_so_zapnutym_predvolenim() {
        // appka pred TLI 22 toto pole nemá vôbec — `#[serde(default)]` nesmie
        // pri jej načítaní zlyhať a predvolené má byť ZAPNUTÉ, hotkey_trh
        // sa doplní na predvolenú „F10" (TLI 39).
        let n = Nastavenia::z_jsonu(r#"{"kluc":"abc","hotkey":"F8"}"#);
        assert!(n.ceny_obrazovka_zapnute);
        assert_eq!(n.hotkey_trh, "F10");
    }

    #[test]
    fn hotkey_trh_sa_normalizuje_velkymi_pismenami_ked_je_nastaveny() {
        let mut n = Nastavenia { hotkey_trh: " f11 ".to_string(), ..Default::default() };
        n.normalizuj();
        assert_eq!(n.hotkey_trh, "F11");
    }

    /// TLI 39, bod 2 (`2026-10-18-1613-mcp.md`): pôvodne prázdny hotkey_trh
    /// nechával appku bez vlastnej klávesy pre tento modul — teraz sa vždy
    /// doplní na predvolenú „F10", či už pole v súbore chýba úplne, alebo je
    /// explicitne uložené ako prázdny reťazec (napr. hráč ho v Nastaveniach
    /// vymaže a uloží).
    #[test]
    fn hotkey_trh_prazdny_sa_normalizuje_na_f10() {
        let mut n = Nastavenia { hotkey_trh: "   ".to_string(), ..Default::default() };
        n.normalizuj();
        assert_eq!(n.hotkey_trh, "F10");

        let n2 = Nastavenia::z_jsonu(r#"{"kluc":"abc","hotkey_trh":""}"#);
        assert_eq!(n2.hotkey_trh, "F10");
    }

    #[test]
    fn rozlozenie_prezije_round_trip() {
        use crate::rozlozenie::{Obdlznik01, PANEL_MINI};
        let mut n = Nastavenia::default();
        n.rozlozenie.uloz_polohu(PANEL_MINI, "1920x1080@100", Obdlznik01 { x: 0.1, y: 0.1, w: 0.2, h: 0.1 });
        n.rozlozenie.nastav_priehladnost(0.5);
        let json = n.do_jsonu();
        let n2 = Nastavenia::z_jsonu(&json);
        assert_eq!(n.rozlozenie, n2.rozlozenie);
    }

    /// TLI 25 — hlavné okno si má pamätať, kde ho hráč nechal.
    #[test]
    fn poloha_hlavneho_okna_prezije_ulozenie_a_nacitanie() {
        let mut n = Nastavenia::default();
        assert_eq!(n.okno_hlavne, None, "kým ním nikto nepohol, okno sa centruje");
        n.okno_hlavne = Some(PolohaOkna { x: -1200, y: 340, sirka: 1100, vyska: 720 });
        let n2 = Nastavenia::z_jsonu(&n.do_jsonu());
        assert_eq!(n2.okno_hlavne, n.okno_hlavne);
    }

    /// Nastavenia uložené staršou verziou appky pole `okno_hlavne` nemajú —
    /// načítanie nesmie spadnúť ani vyrobiť nezmyselnú nulovú polohu.
    #[test]
    fn stary_subor_nastaveni_bez_pola_okno_hlavne_sa_nacita() {
        let n = Nastavenia::z_jsonu("{\"url_servera\": \"https://tli.tvojtatko.sk\"}");
        assert_eq!(n.okno_hlavne, None);
        assert_eq!(n.url_servera, "https://tli.tvojtatko.sk");
    }
}

#[cfg(test)]
mod testy_vzhlad_trasy {
    use super::*;

    /// Staré `nastavenia.json` (spred TLI 62) pole `vzhlad_trasy` nemajú —
    /// `#[serde(default)]` nad štruktúrou musí dosadiť predvolený vzhľad,
    /// nie `0` zo zeru pre `u8`. Keby dosadil nuly, trasa by bola u každého
    /// existujúceho hráča po aktualizácii neviditeľná a vyzeralo by to ako
    /// „overlay nefunguje", nie ako nastavenie.
    #[test]
    fn stare_nastavenia_bez_pola_dostanu_viditelny_vzhlad() {
        let n = Nastavenia::z_jsonu(r#"{"url_servera":"https://tli.tvojtatko.sk","kluc":"abc"}"#);
        assert_eq!(n.vzhlad_trasy, crate::minimapa_vzhlad::VzhladTrasy::default());
        assert!(n.vzhlad_trasy.vysledna_opacita() > 0.5);
    }

    #[test]
    fn normalizacia_nastaveni_oreze_aj_vzhlad_trasy() {
        let mut n = Nastavenia::default();
        n.vzhlad_trasy.opacita_ciary = 250;
        n.vzhlad_trasy.hrubka_px = 0;
        n.vzhlad_trasy.farba = "zelena".to_string();
        n.normalizuj();
        assert_eq!(n.vzhlad_trasy.opacita_ciary, 100);
        assert_eq!(n.vzhlad_trasy.hrubka_px, crate::minimapa_vzhlad::HRUBKA_MIN);
        assert_eq!(n.vzhlad_trasy.farba, crate::minimapa_vzhlad::FARBA_PREDVOLENA);
    }

    /// Vlastné hodnoty musia prežiť uloženie a načítanie — vrátane nuly,
    /// ktorú normalizácia nesmie „opraviť" na predvolených 85/100.
    #[test]
    fn vlastny_vzhlad_prezije_kolo_cez_json() {
        let mut n = Nastavenia::default();
        n.vzhlad_trasy.opacita_ciary = 0;
        n.vzhlad_trasy.opacita_vrstvy = 40;
        n.vzhlad_trasy.hrubka_px = 6;
        n.vzhlad_trasy.farba = "#00FF99".to_string();
        let znova = Nastavenia::z_jsonu(&n.do_jsonu());
        assert_eq!(znova.vzhlad_trasy, n.vzhlad_trasy);
    }

    /// `Rozlozenie::priehladnost` (spoločná pre panely) a vzhľad trasy sú
    /// dve nezávislé veci — stíšenie trasy nesmie stiahnuť panel s radou
    /// solvera a naopak. Keby sa raz zliali, tento test spadne.
    #[test]
    fn opacita_trasy_a_priehladnost_panelov_su_nezavisle() {
        let mut n = Nastavenia::default();
        n.rozlozenie.nastav_priehladnost(0.3);
        n.vzhlad_trasy.opacita_vrstvy = 100;
        n.normalizuj();
        assert_eq!(n.rozlozenie.priehladnost, 0.3);
        assert_eq!(n.vzhlad_trasy.opacita_vrstvy, 100);
    }
}

#[cfg(test)]
mod testy_minimapa_roi {
    use super::*;

    /// Staré `nastavenia.json` (spred TLI 62/2) pole `minimapa_roi` nemajú —
    /// `#[serde(default)]` musí dosadiť pracovnú hypotézu z
    /// `minimapa_fronta`, nie nulový (a teda nepoužiteľný) výrez.
    #[test]
    fn stare_nastavenia_bez_pola_dostanu_odhadnute_roi() {
        let n = Nastavenia::z_jsonu(r#"{"url_servera":"https://tli.tvojtatko.sk"}"#);
        assert_eq!(n.minimapa_roi, crate::minimapa_fronta::RoiSada::default());
    }

    #[test]
    fn normalizacia_nastaveni_oreze_aj_minimapa_roi() {
        let mut n = Nastavenia::default();
        n.minimapa_roi.roh = crate::minimapa_fronta::RoiMinimapy { x: 0.9, y: 0.9, w: 0.9, h: 0.9 };
        n.normalizuj();
        assert!((n.minimapa_roi.roh.x + n.minimapa_roi.roh.w - 1.0).abs() < 1e-9);
        assert!((n.minimapa_roi.roh.y + n.minimapa_roi.roh.h - 1.0).abs() < 1e-9);
    }

    /// Richardom opravená hodnota (keď prídu prvé snímky, krok 62/4) musí
    /// prežiť uloženie a opätovné načítanie.
    #[test]
    fn vlastne_roi_prezije_kolo_cez_json() {
        let mut n = Nastavenia::default();
        n.minimapa_roi.stred = crate::minimapa_fronta::RoiMinimapy { x: 0.3, y: 0.2, w: 0.4, h: 0.5 };
        let znova = Nastavenia::z_jsonu(&n.do_jsonu());
        assert_eq!(znova.minimapa_roi.stred, n.minimapa_roi.stred);
    }
}

#[cfg(test)]
mod testy_minimapa_zber {
    use super::*;
    use crate::minimapa_fronta::RezimMinimapy;

    /// TLI 62/3 — appka nesmie fotiť nikoho, kto si to sám nezapol.
    #[test]
    fn minimapa_zber_je_predvolene_vypnuty_a_rezim_stred() {
        let n = Nastavenia::default();
        assert!(!n.minimapa_zber_zapnute);
        assert_eq!(n.minimapa_rezim, RezimMinimapy::Stred);
    }

    /// Staré `nastavenia.json` (spred TLI 62/3) tieto polia nemajú —
    /// `#[serde(default)]` musí dosadiť rovnaké predvoľby ako čerstvý
    /// Default, nie appku mlčky zapnúť alebo nechať bez režimu.
    #[test]
    fn stare_nastavenia_bez_poli_dostanu_predvolene_hodnoty() {
        let n = Nastavenia::z_jsonu(r#"{"url_servera":"https://tli.tvojtatko.sk"}"#);
        assert!(!n.minimapa_zber_zapnute);
        assert_eq!(n.minimapa_rezim, RezimMinimapy::Stred);
    }

    #[test]
    fn vlastne_hodnoty_prezije_kolo_cez_json() {
        let mut n = Nastavenia::default();
        n.minimapa_zber_zapnute = true;
        n.minimapa_rezim = RezimMinimapy::Roh;
        let znova = Nastavenia::z_jsonu(&n.do_jsonu());
        assert!(znova.minimapa_zber_zapnute);
        assert_eq!(znova.minimapa_rezim, RezimMinimapy::Roh);
    }

    #[test]
    fn adresy_minimapy_su_spravne() {
        let n = Nastavenia { url_servera: "https://tli.tvojtatko.sk".to_string(), ..Default::default() };
        assert_eq!(n.adresa_minimapa(), "https://tli.tvojtatko.sk/api/minimapa");
        assert_eq!(n.adresa_minimapa_stav(), "https://tli.tvojtatko.sk/api/minimapa/stav");
    }
}
