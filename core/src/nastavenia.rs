use serde::{Deserialize, Serialize};

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
}

impl Default for Nastavenia {
    fn default() -> Self {
        Self {
            url_servera: "https://tli.tvojtatko.sk".to_string(),
            kluc: "".to_string(),
            hotkey: "F8".to_string(),
            hotkey_canvas: "F9".to_string(),
            autoskrytie_s: 20,
            proces_hry: "Torchlight Infinite, Torchlight".to_string(),
            zvuk: false,
            zber_dat_zapnute: false,
            tema: "svetla".to_string(),
            cesta_logu: String::new(),
            tracker_zapnuty: true,
            tracker_dan: false,
            tracker_sync: false,
            mini_overlay: false,
            pripominat_enable_log: true,
            ceny_sync: true,
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

        if self.autoskrytie_s > 600 {
            self.autoskrytie_s = 600;
        }

        // Neznáma hodnota témy nesmie nechať okno bez farieb — padá sa na svetlú.
        let t = self.tema.trim().to_lowercase();
        self.tema = if t == "tmava" { t } else { "svetla".to_string() };
        self.cesta_logu = self.cesta_logu.trim().to_string();
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
}
