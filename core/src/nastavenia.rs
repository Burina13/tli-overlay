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
    fn round_trip_jsonu() {
        let mut n = Nastavenia::default();
        n.kluc = "test1234".to_string();
        let json = n.do_jsonu();
        let n2 = Nastavenia::z_jsonu(&json);
        assert_eq!(n, n2);
    }
}
