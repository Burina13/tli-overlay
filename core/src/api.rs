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
}
