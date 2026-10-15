use std::path::PathBuf;
use std::sync::Mutex;
use serde::Serialize;

use tli_overlay_core::guide::{
    do_cache, priprav_html, treba_stiahnut, vek_text, z_cache, Guide
};

/// Drží načítaného sprievodcu v pamäti počas behu aplikácie.
pub struct StavGuide(pub Mutex<Guide>);

impl Default for StavGuide {
    fn default() -> Self {
        // Vytvoríme prázdneho sprievodcu pri štarte aplikácie
        StavGuide(Mutex::new(Guide::default()))
    }
}

/// Vráti cestu k súboru `guide.json`, ktorý je uložený v rovnakom priečinku
/// ako `nastavenia.json`.
pub fn cesta_cache() -> PathBuf {
    crate::stav::cesta_suboru().with_file_name("guide.json")
}

/// Prečíta súbor z disku. Ak neexistuje alebo je pokazený, vráti prázdny Guide.
pub fn nacitaj() -> Guide {
    let cesta = cesta_cache();
    let obsah = std::fs::read_to_string(cesta).unwrap_or_default();
    z_cache(&obsah)
}

/// Uloží sprievodcu do súboru `guide.json`. Ak chýbajú priečinky, vytvorí ich.
pub fn uloz(g: &Guide) -> Result<(), String> {
    let cesta = cesta_cache();
    
    // Uistíme sa, že nadradený priečinok existuje
    if let Some(rodic) = cesta.parent() {
        std::fs::create_dir_all(rodic)
            .map_err(|e| format!("Nepodarilo sa vytvoriť priečinok pre cache sprievodcu: {}", e))?;
    }

    let json_text = do_cache(g);
    std::fs::write(&cesta, json_text)
        .map_err(|e| format!("Nepodarilo sa zapísať dáta sprievodcu: {}", e))
}

#[derive(Serialize)]
pub struct KapitolaProUi {
    pub slug: String,
    pub nazov: String,
    pub html: String,
}

#[derive(Serialize)]
pub struct GuideProUi {
    pub verzia: String,
    pub zdroj: String,
    pub stiahnuty: u64,
    pub vek: String,
    pub je_prazdny: bool,
    pub tahak: Option<KapitolaProUi>,
    pub kapitoly: Vec<KapitolaProUi>,
}

/// Pripraví kópiu sprievodcu určenú pre frontend. Namiesto odkazov na ikony rovno
/// vloží `data:` URI do HTML, aby frontend nemusel riešiť vlastné sťahovanie obrázkov.
///
/// Mapa ikon sa posiela ako referencia, aby sme zabránili zbytočnému klonovaniu
/// všetkých ikon do pamäte pred mapovaním kapitol.
pub fn pre_ui(g: &Guide, teraz: u64) -> GuideProUi {
    let vek = if g.stiahnuty == 0 {
        "nikdy".to_string()
    } else {
        vek_text(teraz.saturating_sub(g.stiahnuty))
    };

    let tahak = g.tahak.as_ref().map(|k| KapitolaProUi {
        slug: k.slug.clone(),
        nazov: k.nazov.clone(),
        html: priprav_html(&k.html, &g.ikony),
    });

    let kapitoly = g.kapitoly.iter().map(|k| KapitolaProUi {
        slug: k.slug.clone(),
        nazov: k.nazov.clone(),
        html: priprav_html(&k.html, &g.ikony),
    }).collect();

    GuideProUi {
        verzia: g.verzia.clone(),
        zdroj: g.zdroj.clone(),
        stiahnuty: g.stiahnuty,
        vek,
        je_prazdny: treba_stiahnut(g),
        tahak,
        kapitoly,
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use tli_overlay_core::guide::Kapitola;

    #[test]
    fn test_cesta_cache() {
        let nastavenia_cesta = crate::stav::cesta_suboru();
        let cache_cesta = cesta_cache();
        
        assert_eq!(cache_cesta.file_name().unwrap().to_str().unwrap(), "guide.json");
        assert_eq!(cache_cesta.parent(), nastavenia_cesta.parent());
    }

    #[test]
    fn test_pre_ui_prazdny() {
        let prazdny = Guide::default();
        
        let ui = pre_ui(&prazdny, 1000);
        assert!(ui.je_prazdny);
        assert_eq!(ui.vek, "nikdy");
    }

    #[test]
    fn test_pre_ui_ikony() {
        let mut ikony = std::collections::BTreeMap::new();
        ikony.insert("mec".to_string(), "data:image/png;base64,abc".to_string());
        
        let g = Guide {
            verzia: "v1".to_string(),
            zdroj: "test".to_string(),
            tahak: None,
            kapitoly: vec![Kapitola {
                slug: "uvod".to_string(),
                nazov: "Úvod".to_string(),
                html: "Tvoj zbraň: <img src=\"/ikony/mec\" />".to_string(),
            }],
            ikony,
            stiahnuty: 100,
        };
        
        let ui = pre_ui(&g, 150);
        assert_eq!(ui.kapitoly.len(), 1);
        assert_eq!(
            ui.kapitoly[0].html,
            "Tvoj zbraň: <img src=\"data:image/png;base64,abc\" />"
        );
    }

    #[test]
    fn test_cache_do_a_z_cez_pre_ui() {
        let povodny = Guide {
            verzia: "v2".to_string(),
            zdroj: "test_zdroj".to_string(),
            tahak: None,
            kapitoly: vec![],
            ikony: std::collections::BTreeMap::new(),
            stiahnuty: 500,
        };
        
        // Namiesto zápisu na disk použijeme core funkcie do_cache a z_cache
        let serializovany = do_cache(&povodny);
        let deserializovany = z_cache(&serializovany);
        
        let ui = pre_ui(&deserializovany, 600);
        assert_eq!(ui.verzia, "v2");
        assert_eq!(ui.zdroj, "test_zdroj");
        assert_eq!(ui.stiahnuty, 500);
        assert_eq!(ui.je_prazdny, false); // Stiahnuté pred 100 sekundami, nevyžaduje stiahnutie
    }
}
