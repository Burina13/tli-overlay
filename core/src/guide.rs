use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Kapitola {
    pub slug: String,
    pub nazov: String,
    pub html: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Guide {
    pub verzia: String,
    pub zdroj: String,
    pub tahak: Option<Kapitola>,
    pub kapitoly: Vec<Kapitola>,
    pub ikony: BTreeMap<String, String>,
    /// Unixový čas poslednej úspešnej synchronizácie (0 = ešte nikdy).
    pub stiahnuty: u64,
}

/// Rozparsuje JSON odpoveď zo servera. Validuje základné predpoklady,
/// aby sme neuložili nefunkčný obsah, a aktualizuje čas stiahnutia.
pub fn z_odpovede(telo: &str, teraz: u64) -> Result<Guide, String> {
    let mut guide: Guide = serde_json::from_str(telo).map_err(|_| {
        "Server poslal sprievodcu v tvare, ktorému appka nerozumie.".to_string()
    })?;

    if guide.verzia.trim().is_empty() || guide.kapitoly.is_empty() {
        return Err("Server poslal prázdneho sprievodcu.".to_string());
    }

    guide.stiahnuty = teraz;
    Ok(guide)
}

/// Načíta stav z lokálnej cache (súboru). Pri akejkoľvek chybe vráti prázdneho
/// sprievodcu, aby poškodený cache súbor nespôsobil pád appky (napr. pri zlom update).
pub fn z_cache(text: &str) -> Guide {
    serde_json::from_str(text).unwrap_or_default()
}

/// Skonvertuje sprievodcu na JSON bez formátovania (minimalizácia veľkosti,
/// súbor na disku bude mať MB a čitateľnosť nie je priorita).
pub fn do_cache(g: &Guide) -> String {
    serde_json::to_string(g).unwrap_or_else(|_| "{}".to_string())
}

/// Očistí a pripraví HTML pred tým, než ho pošleme do webview.
/// Nemáme k dispozícii regex, preto využívame manuálne prechádzanie znakov.
pub fn priprav_html(html: &str, ikony: &BTreeMap<String, String>) -> String {
    let bez_skriptov = odstran_blok(html, "script");
    let bez_iframov = odstran_blok(&bez_skriptov, "iframe");
    let s_ikonami = nahrad_ikony(&bez_iframov, ikony);
    pridaj_data_externy(&s_ikonami)
}

/// Pomocná funkcia: Máme prázdnu verziu, čo znamená, že ešte nemáme nič?
pub fn treba_stiahnut(g: &Guide) -> bool {
    g.verzia.is_empty()
}

/// Pomocná funkcia na vytvorenie GET parametra pre overenie aktualizácie.
/// Posielame aktuálnu verziu, aby server mohol odpovedať 304 Not Modified.
pub fn parameter_verzie(g: &Guide) -> String {
    if g.verzia.is_empty() {
        "".to_string()
    } else {
        format!("?ak_novsie_ako={}", g.verzia)
    }
}

/// Formátovanie veku stiahnutia pre používateľské rozhranie.
pub fn vek_text(sekundy: u64) -> String {
    if sekundy < 60 {
        "pred chvíľou".to_string()
    } else if sekundy < 3600 {
        format!("pred {} min", sekundy / 60)
    } else if sekundy < 48 * 3600 {
        format!("pred {} h", sekundy / 3600)
    } else {
        format!("pred {} dňami", sekundy / 86400)
    }
}

// ============================================================================
// Pomocné privátne funkcie pre úpravu HTML (bez závislosti na Regex)
// ============================================================================

/// Vyhľadá ASCII reťazec ignorujúc veľkosť písmen (bezpečné pre utf8, lebo hľadáme len ASCII tagy).
fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() {
        return Some(0);
    }
    h.windows(n.len()).position(|w| w.eq_ignore_ascii_case(n))
}

/// Bezpečnostná poistka: Odstráni bloky <tag ...>...</tag> (ignoruje case).
fn odstran_blok(html: &str, tag: &str) -> String {
    let mut vysledok = String::with_capacity(html.len());
    let mut i = 0;
    let start_tag = format!("<{}", tag);
    let end_tag = format!("</{}>", tag);

    while i < html.len() {
        if let Some(rel_start) = find_ignore_ascii_case(&html[i..], &start_tag) {
            let start_idx = i + rel_start;
            vysledok.push_str(&html[i..start_idx]);

            // Nájdeme uzatvárací tag, inak zmažeme zvyšok.
            if let Some(rel_end) = find_ignore_ascii_case(&html[start_idx..], &end_tag) {
                i = start_idx + rel_end + end_tag.len();
            } else {
                i = html.len();
            }
        } else {
            vysledok.push_str(&html[i..]);
            break;
        }
    }
    vysledok
}

/// Nahradí relačné cesty k ikonám priamo za base64 URI, aby webview
/// nehlásilo 404 chyby pri pokuse načítať ich z lokálnej zložky.
fn nahrad_ikony(html: &str, ikony: &BTreeMap<String, String>) -> String {
    let mut vysledok = String::with_capacity(html.len() + 1024);
    let prefix = "src=\"/ikony/";
    let mut i = 0;

    while let Some(rel_start) = html[i..].find(prefix) {
        let start_idx = i + rel_start;
        vysledok.push_str(&html[i..start_idx]);

        let hlada_od = start_idx + prefix.len();
        if let Some(rel_end) = html[hlada_od..].find('"') {
            let end_idx = hlada_od + rel_end;
            let rel_cesta = &html[hlada_od..end_idx];

            if let Some(data_uri) = ikony.get(rel_cesta) {
                vysledok.push_str("src=\"");
                vysledok.push_str(data_uri);
                vysledok.push('"');
            } else {
                vysledok.push_str("src=\"\""); // Chýba v mape, zabránime 404
            }
            i = end_idx + 1; // preskočíme záverečnú úvodzovku
        } else {
            // Neuzatvorená úvodzovka (poškodené HTML)
            vysledok.push_str(prefix);
            i = hlada_od;
        }
    }
    vysledok.push_str(&html[i..]);
    vysledok
}

/// Pre lepšiu integráciu v Rust klientovi pridáva data-externy atribút ku klasickým linkom.
fn pridaj_data_externy(html: &str) -> String {
    let mut vysledok = String::with_capacity(html.len() + 256);
    let mut i = 0;

    while i < html.len() {
        if let Some(rel_start) = find_ignore_ascii_case(&html[i..], "<a ") {
            let start_idx = i + rel_start;
            vysledok.push_str(&html[i..start_idx]);

            if let Some(rel_end) = html[start_idx..].find('>') {
                let end_idx = start_idx + rel_end;
                let a_tag = &html[start_idx..=end_idx]; // celý <a> tag vrátane >

                let has_http = find_ignore_ascii_case(a_tag, "href=\"http").is_some();
                let has_externy = find_ignore_ascii_case(a_tag, "data-externy=\"1\"").is_some();

                if has_http && !has_externy {
                    vysledok.push_str(&html[start_idx..end_idx]);
                    vysledok.push_str(" data-externy=\"1\">");
                } else {
                    vysledok.push_str(a_tag);
                }
                i = end_idx + 1;
            } else {
                vysledok.push_str(&html[i..]);
                break;
            }
        } else {
            vysledok.push_str(&html[i..]);
            break;
        }
    }
    vysledok
}

// ============================================================================
// Testy (pokrývajú všetky požiadavky zadania vrátane hraníc)
// ============================================================================
#[cfg(test)]
mod testy {
    use super::*;

    const DOBRY_JSON: &str = r#"{
        "ok": true,
        "verzia": "v1.0",
        "zdroj": "https://tlicompendium.com",
        "tahak": {"slug": "t1", "nazov": "T1", "html": "<p>A</p>"},
        "kapitoly": [{"slug": "k1", "nazov": "K1", "html": "<p>B</p>"}],
        "ikony": {"test.webp": "data:image"}
    }"#;

    #[test]
    fn test_parsovanie_plna_odpoved() {
        let g = z_odpovede(DOBRY_JSON, 123456).expect("Správny JSON musí prejsť");
        assert_eq!(g.verzia, "v1.0");
        assert_eq!(g.stiahnuty, 123456);
        assert_eq!(g.kapitoly.len(), 1);
        assert!(g.tahak.is_some());
        assert_eq!(g.ikony.get("test.webp").unwrap(), "data:image");
    }

    #[test]
    fn test_tahak_null() {
        let json = r#"{"verzia": "v1.0", "zdroj": "...", "tahak": null, "kapitoly": [{"slug":"a", "nazov":"a", "html":"a"}]}"#;
        let g = z_odpovede(json, 0).unwrap();
        assert!(g.tahak.is_none());
    }

    #[test]
    fn test_chybajuce_ikony() {
        let json = r#"{"verzia": "v1", "zdroj": "", "kapitoly": [{"slug":"a", "nazov":"a", "html":"a"}]}"#;
        let g = z_odpovede(json, 0).unwrap();
        assert!(g.ikony.is_empty()); // Default generuje prázdnu mapu
    }

    #[test]
    fn test_pokazeny_json() {
        let chyba = z_odpovede("{ zly syntax", 0).unwrap_err();
        assert_eq!(chyba, "Server poslal sprievodcu v tvare, ktorému appka nerozumie.");
    }

    #[test]
    fn test_prazdne_kapitoly() {
        let json = r#"{"verzia": "v1", "zdroj": "", "kapitoly": []}"#;
        let chyba = z_odpovede(json, 0).unwrap_err();
        assert_eq!(chyba, "Server poslal prázdneho sprievodcu.");
    }

    #[test]
    fn test_prazdna_verzia() {
        let json = r#"{"verzia": "", "zdroj": "", "kapitoly": [{"slug":"a", "nazov":"a", "html":"a"}]}"#;
        let chyba = z_odpovede(json, 0).unwrap_err();
        assert_eq!(chyba, "Server poslal prázdneho sprievodcu.");
    }

    #[test]
    fn test_cache_roundtrip() {
        let g1 = z_odpovede(DOBRY_JSON, 999).unwrap();
        let ulozeny_text = do_cache(&g1);
        let g2 = z_cache(&ulozeny_text);
        assert_eq!(g1.verzia, g2.verzia);
        assert_eq!(g1.stiahnuty, g2.stiahnuty);
    }

    #[test]
    fn test_poskodena_cache() {
        let g = z_cache("totalny blud {{{");
        assert!(g.verzia.is_empty()); // fallback na Default
        assert!(g.kapitoly.is_empty());
    }

    #[test]
    fn test_nahradenie_ikony() {
        let html = r#"<img src="/ikony/skill.webp" />"#;
        let mut ikony = BTreeMap::new();
        ikony.insert("skill.webp".to_string(), "data:image/webp;base64,123".to_string());
        
        let vysledok = priprav_html(html, &ikony);
        assert_eq!(vysledok, r#"<img src="data:image/webp;base64,123" />"#);
    }

    #[test]
    fn test_chybajuca_ikona_src_prazdne() {
        let html = r#"<img src="/ikony/neznama.webp" />"#;
        let ikony = BTreeMap::new();
        
        let vysledok = priprav_html(html, &ikony);
        assert_eq!(vysledok, r#"<img src="" />"#); // Žiadna 404
    }

    #[test]
    fn test_odstranenie_script_a_iframe() {
        let html = "Start <script>alert(1);</SCRIPT> Stred <IFRAME src='x'></IFRame> Koniec";
        let vysledok = priprav_html(html, &BTreeMap::new());
        assert_eq!(vysledok, "Start  Stred  Koniec");
    }

    #[test]
    fn test_data_externy_sa_nepride_dvakrat() {
        let html = r#"<a href="https://example.com">1</a> <a href="http://a.com" data-externy="1">2</a>"#;
        let vysledok = priprav_html(html, &BTreeMap::new());
        // Prvý nemá externý atribút, pridá sa mu. Druhý ho už má, nepridá sa druhýkrát.
        assert_eq!(vysledok, r#"<a href="https://example.com" data-externy="1">1</a> <a href="http://a.com" data-externy="1">2</a>"#);
    }

    #[test]
    fn test_parameter_verzie() {
        let prazdny = Guide::default();
        assert_eq!(parameter_verzie(&prazdny), "");

        let mut naplneny = Guide::default();
        naplneny.verzia = "xyz123".to_string();
        assert_eq!(parameter_verzie(&naplneny), "?ak_novsie_ako=xyz123");
    }

    #[test]
    fn test_vek_text_hranice() {
        assert_eq!(vek_text(10), "pred chvíľou");
        assert_eq!(vek_text(59), "pred chvíľou");
        assert_eq!(vek_text(60), "pred 1 min");
        assert_eq!(vek_text(3599), "pred 59 min");
        assert_eq!(vek_text(3600), "pred 1 h");
        assert_eq!(vek_text(172799), "pred 47 h");
        assert_eq!(vek_text(172800), "pred 2 dňami"); // 48 hodin a viac sú dni
        assert_eq!(vek_text(864000), "pred 10 dňami");
    }

    // ── Kontrola Clauda nad vygenerovaným kódom: prípady, ktoré sa v appke
    // reálne stanú a testy od generátora ich nepokrývali ─────────────────────

    #[test]
    fn viac_ikon_v_jednom_kuse_a_slovencina_okolo() {
        // Byte-indexy vo vyhľadávaní nesmú rozseknúť viacbajtový znak — kapitoly
        // sú po slovensky a `ť`/`ž` sú presne tie znaky, na ktorých to padá.
        let html = r#"<p>Ťahák: <img src="/ikony/a.webp"> a žiadna <img src="/ikony/b.webp"></p>"#;
        let mut ikony = BTreeMap::new();
        ikony.insert("a.webp".to_string(), "data:A".to_string());
        ikony.insert("b.webp".to_string(), "data:B".to_string());
        let v = priprav_html(html, &ikony);
        assert_eq!(
            v,
            r#"<p>Ťahák: <img src="data:A"> a žiadna <img src="data:B"></p>"#
        );
    }

    #[test]
    fn neuzavrety_script_zje_zvysok_a_neskonci_v_nekonecnej_slucke() {
        let v = priprav_html("Pred <script>zly kod bez konca", &BTreeMap::new());
        assert_eq!(v, "Pred ");
    }

    #[test]
    fn odkaz_bez_http_nedostane_data_externy() {
        // Vnútorné kotvy v kapitole (`#nadpis`) sa otvárať cez Rust nemajú.
        let html = r##"<a href="#dalej">ďalej</a>"##;
        assert_eq!(priprav_html(html, &BTreeMap::new()), html);
    }

    #[test]
    fn treba_stiahnut_len_ked_este_nic_nie_je() {
        assert!(treba_stiahnut(&Guide::default()));
        let g = Guide { verzia: "abc".to_string(), ..Default::default() };
        assert!(!treba_stiahnut(&g));
    }

    #[test]
    fn cache_prezije_cely_guide_vratane_ikon() {
        let g1 = z_odpovede(DOBRY_JSON, 42).unwrap();
        let g2 = z_cache(&do_cache(&g1));
        assert_eq!(g1, g2);
    }
}

