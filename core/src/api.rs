use serde::{Deserialize, Serialize};
use crate::suradnice::Pole;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Odporucanie {
    pub akcia: String,
    pub nazov: String,
    pub ciel: Option<i64>,
    pub cez_scratch: bool,
    pub nastroj: Option<String>,
    pub dar: Option<String>,
    pub dovod: String,
    pub hodnota: Option<f64>,
}

impl Default for Odporucanie {
    fn default() -> Self {
        Self {
            akcia: "".to_string(),
            nazov: "".to_string(),
            ciel: None,
            cez_scratch: false,
            nastroj: None,
            dar: None,
            dovod: "".to_string(),
            hodnota: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Alternativa {
    pub akcia: String,
    pub nazov: String,
    pub ciel: Option<i64>,
    pub hodnota: Option<f64>,
    pub istota: Option<f64>,
    pub dovod: String,
}

impl Default for Alternativa {
    fn default() -> Self {
        Self {
            akcia: "".to_string(),
            nazov: "".to_string(),
            ciel: None,
            hodnota: None,
            istota: None,
            dovod: "".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Analyza {
    pub ok: bool,
    pub odporucanie: Option<Odporucanie>,
    pub alternativy: Vec<Alternativa>,
    pub istota: Option<f64>,
    pub istota_rozpoznania: Option<f64>,
    pub skontroluj: Vec<String>,
    pub polia: Vec<Pole>,
    pub trvanie_ms: Option<u64>,
}

impl Default for Analyza {
    fn default() -> Self {
        Self {
            ok: false,
            odporucanie: None,
            alternativy: vec![],
            istota: None,
            istota_rozpoznania: None,
            skontroluj: vec![],
            polia: vec![],
            trvanie_ms: None,
        }
    }
}

// ── Frozen Canvas (TLI 4b) — `?modul=canvas`, kontrakt v
// docs/fakty/hry/torchlight-infinite/canvas-api-kontrakt.md. Odpoveď nesie
// `kroky[]` (sekvenciu ČO→KAM), nie jedno `odporucanie` ako Tartarus, lebo
// cieľ typicky žiada viac umiestnení naraz.

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Box01 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KrokZdroj {
    pub typ_slotu: String,
    pub index: i64,
    #[serde(rename = "box")]
    pub box_: Option<Box01>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KrokCiel {
    pub typ_bunky: String,
    pub riadok: i64,
    pub stlpec: i64,
    #[serde(rename = "box")]
    pub box_: Option<Box01>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Krok {
    pub poradie: i64,
    pub zdroj: KrokZdroj,
    pub ciel: KrokCiel,
    pub dovod: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalyzaCanvas {
    pub ok: bool,
    pub modul: String,
    pub kroky: Vec<Krok>,
    pub istota: Option<f64>,
    pub istota_rozpoznania: Option<f64>,
    pub skontroluj: Vec<String>,
    pub trvanie_ms: Option<u64>,
    /// TLI 4c (`2026-10-15-0028-mcp.md`): `true`/`false` = smer aktívneho
    /// cieľa má/nemá aspoň `canvas.yml:prah_overenia_parov` potvrdených párov
    /// pred/po; `null` keď plátno nemá rozpoznaný cieľ (nedá sa posúdiť).
    /// Kontrakt: `canvas-api-kontrakt.md` § `pravidlo_overene`.
    pub pravidlo_overene: Option<bool>,
}

impl Default for AnalyzaCanvas {
    fn default() -> Self {
        Self {
            ok: false,
            modul: "canvas".to_string(),
            kroky: vec![],
            istota: None,
            istota_rozpoznania: None,
            skontroluj: vec![],
            trvanie_ms: None,
            pravidlo_overene: None,
        }
    }
}

/// Krok, ktorého zdroj aj cieľ majú súradnice (iné odpadli, lebo rozpoznávanie
/// pre ne nevrátilo box — rovnaká konvencia ako Tartarus `pouzitelne_polia`).
pub fn pouzitelne_kroky(a: &AnalyzaCanvas) -> Vec<&Krok> {
    a.kroky
        .iter()
        .filter(|k| {
            k.zdroj.box_.map(|b| b.w > 0.0 && b.h > 0.0).unwrap_or(false)
                && k.ciel.box_.map(|b| b.w > 0.0 && b.h > 0.0).unwrap_or(false)
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChybaServera {
    pub ok: bool,
    pub chyba: String,
    pub kod: Option<u16>,
}

impl Default for ChybaServera {
    fn default() -> Self {
        Self {
            ok: false,
            chyba: "".to_string(),
            kod: None,
        }
    }
}

/// Prekladá technické HTTP kódy na ľudskou rečou podané odpovede,
/// aby hráč pochopil, v čom nastal problém a čo má urobiť ďalej.
pub fn sprava_chyby(stav: u16, telo: &str) -> String {
    if let Ok(chyba_srv) = serde_json::from_str::<ChybaServera>(telo) {
        if !chyba_srv.chyba.trim().is_empty() {
            return chyba_srv.chyba;
        }
    }

    match stav {
        400 => "Server screenshotu nerozumel — posiela sa len PNG alebo JPEG do 4 MB. Skús to znova; ak to trvá, nahlás to v #torchlight."
            .to_string(),
        401 => "Server neprijal tvoj kľúč. Skontroluj si ho v nastaveniach — nový si vydáš na tli.tvojtatko.sk/kluc alebo príkazom /tli kluc v Discorde."
            .to_string(),
        403 => "Tvoj účet nemá rolu Torchlight, takže ťa server nepustí. Napíš o ňu Richardovi v Discorde."
            .to_string(),
        404 => "Adresa servera je zlá. V nastaveniach má byť https://tli.tvojtatko.sk."
            .to_string(),
        413 => "Screenshot je pre server priveľký (limit sú 4 MB). Skús to znova — ak sa to opakuje, nahlás to v #torchlight."
            .to_string(),
        422 => "Na obrázku som nenašiel dosku Tartarus. Maj ju celú na obrazovke a skús to znova."
            .to_string(),
        429 => "Priveľa požiadaviek za sebou — server púšťa 30 za minútu. Počkaj chvíľu a stlač hotkey znova."
            .to_string(),
        500..=599 => "Server má poruchu a analýzu teraz nespraví. Skús to o chvíľu znova; ak to trvá, nahlás to v #torchlight."
            .to_string(),
        _ => format!("Neznáma chyba servera (HTTP {stav})."),
    }
}

/// Formátuje chybovú hlášku priamo na vrstve sieťového prenosu (napr. timeouty),
/// aby sme oddelili aplikačné chyby servera od zlyhaní pripojenia.
pub fn sprava_siete(detail: &str) -> String {
    format!(
        "Nepodarilo sa spojiť so serverom. Skontroluj internet a adresu v nastaveniach \
(tlačidlo „Test pripojenia“). Detail: {detail}"
    )
}

/// Filtruje poškodené či zámerne skryté oblasti (vrátené s nulovou veľkosťou),
/// aby sme sa nepokúšali vykresliť v overlayi neviditeľné alebo poškodené štvorce.
pub fn pouzitelne_polia(a: &Analyza) -> Vec<&Pole> {
    a.polia.iter().filter(|p| p.w > 0.0 && p.h > 0.0).collect()
}

/// Identifikuje políčka, ktorými si server nie je celkom istý, 
/// aby sme ich vizuálne odlíšili a dali hráčovi možnosť manuálne si overiť ťah.
pub fn je_neiste(a: &Analyza, index: u32) -> bool {
    let hladane = format!("[{}]", index);
    a.skontroluj.iter().any(|s| s.contains(&hladane))
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn parsovanie_analyzy() {
        let json = r#"{
            "ok": true,
            "odporucanie": {
                "akcia": "Click",
                "nazov": "Zlato",
                "dovod": "Najlepsi tah"
            },
            "polia": [
                { "index": 0, "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2 }
            ]
        }"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        assert!(a.ok);
        assert_eq!(a.odporucanie.unwrap().akcia, "Click");
        assert_eq!(a.polia.len(), 1);
        assert!(a.istota.is_none());
    }

    #[test]
    fn sprava_chyby_berie_text_zo_servera_ak_je() {
        let json = r#"{"ok": false, "chyba": "Tvoj špeciálny problém."}"#;
        assert_eq!(sprava_chyby(401, json), "Tvoj špeciálny problém.");
    }

    #[test]
    fn sprava_chyby_fallback_podla_kodu() {
        let json = "{}";
        assert!(sprava_chyby(401, json).contains("Server neprijal tvoj kľúč"));
        assert!(sprava_chyby(403, json).contains("rolu Torchlight"));
        assert!(sprava_chyby(422, json).contains("dosku Tartarus"));
        assert!(sprava_chyby(429, json).contains("30 za minútu"));
        assert!(sprava_chyby(503, json).contains("poruch"));
        assert_eq!(sprava_chyby(999, json), "Neznáma chyba servera (HTTP 999).");
    }

    #[test]
    fn je_neiste_hlada_index() {
        let a = Analyza {
            skontroluj: vec!["cells[7].inmate.tier".to_string()],
            ..Default::default()
        };
        assert!(je_neiste(&a, 7));
        assert!(!je_neiste(&a, 1));
    }

    #[test]
    fn parsovanie_analyzy_canvas() {
        // Tvar presne podľa docs/fakty/hry/torchlight-infinite/canvas-api-kontrakt.md.
        let json = r#"{
            "ok": true,
            "modul": "canvas",
            "kroky": [
                {
                    "poradie": 1,
                    "zdroj": {"typ_slotu": "ruka", "index": 0,
                              "box": {"x": 0.1, "y": 0.9, "w": 0.08, "h": 0.08}},
                    "ciel": {"typ_bunky": "platno", "riadok": 3, "stlpec": 3,
                             "box": {"x": 0.3, "y": 0.3, "w": 0.08, "h": 0.08}},
                    "dovod": "Vezmi dwarf Lv1 zo slotu 0..."
                },
                {
                    "poradie": 2,
                    "zdroj": {"typ_slotu": "ruka", "index": 1, "box": null},
                    "ciel": {"typ_bunky": "platno", "riadok": 4, "stlpec": 2, "box": null},
                    "dovod": "Chýba súradnica"
                }
            ],
            "istota": 0.9,
            "skontroluj": []
        }"#;
        let a: AnalyzaCanvas = serde_json::from_str(json).unwrap();
        assert!(a.ok);
        assert_eq!(a.modul, "canvas");
        assert_eq!(a.kroky.len(), 2);
        assert_eq!(a.kroky[0].poradie, 1);
        assert_eq!(a.kroky[0].zdroj.index, 0);
        assert_eq!(a.kroky[0].ciel.riadok, 3);
        assert!(a.kroky[0].zdroj.box_.is_some());
        assert!(a.kroky[1].zdroj.box_.is_none());
        assert_eq!(a.istota, Some(0.9));
    }

    #[test]
    fn parsovanie_analyzy_canvas_s_pravidlo_overene() {
        let json = r#"{"ok": true, "modul": "canvas", "kroky": [],
                       "istota": 0.9, "skontroluj": [], "pravidlo_overene": false}"#;
        let a: AnalyzaCanvas = serde_json::from_str(json).unwrap();
        assert_eq!(a.pravidlo_overene, Some(false));
    }

    #[test]
    fn chybajuce_pravidlo_overene_je_none_nie_chyba() {
        // Starší server (pred TLI 4c) toto pole nevracia vôbec — parsovanie
        // nesmie spadnúť, len sa to má brať ako "neznáme".
        let json = r#"{"ok": true, "modul": "canvas", "kroky": [], "istota": 0.9, "skontroluj": []}"#;
        let a: AnalyzaCanvas = serde_json::from_str(json).unwrap();
        assert_eq!(a.pravidlo_overene, None);
    }

    #[test]
    fn pouzitelne_kroky_vynecha_kroky_bez_boxu() {
        let mut a = AnalyzaCanvas { ok: true, ..Default::default() };
        a.kroky.push(Krok {
            poradie: 1,
            zdroj: KrokZdroj { typ_slotu: "ruka".to_string(), index: 0,
                              box_: Some(Box01 { x: 0.1, y: 0.1, w: 0.05, h: 0.05 }) },
            ciel: KrokCiel { typ_bunky: "platno".to_string(), riadok: 1, stlpec: 1,
                             box_: Some(Box01 { x: 0.2, y: 0.2, w: 0.05, h: 0.05 }) },
            dovod: "ok".to_string(),
        });
        a.kroky.push(Krok {
            poradie: 2,
            zdroj: KrokZdroj { typ_slotu: "ruka".to_string(), index: 1, box_: None },
            ciel: KrokCiel { typ_bunky: "platno".to_string(), riadok: 2, stlpec: 1, box_: None },
            dovod: "chyba box".to_string(),
        });
        let pouzitelne = pouzitelne_kroky(&a);
        assert_eq!(pouzitelne.len(), 1);
        assert_eq!(pouzitelne[0].poradie, 1);
    }
}
