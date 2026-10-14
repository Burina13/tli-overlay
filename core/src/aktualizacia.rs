use serde::{Deserialize, Serialize};

pub const MANIFEST_URL: &str = "https://raw.githubusercontent.com/Burina13/tli-overlay/main/latest-tli.json";

pub const POVOLENE_HOSTITELE: [&str; 3] = [
    "raw.githubusercontent.com",
    "github.com",
    "objects.githubusercontent.com"
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub notes: String,
    #[serde(rename = "msiUrl")]
    pub msi_url: String,
    #[serde(rename = "msiSha256")]
    pub msi_sha256: String,
}

/// Striktne preveruje, či inštalačný súbor pochádza výhradne z nášho oficiálneho repozitára,
/// aby žiadna odpoveď podvrhnutá útočníkom (alebo DNS otravou) nevyvolala inštaláciu 
/// nebezpečného softvéru na počítači hráča.
pub fn je_povolena_adresa(url: &str) -> bool {
    if let Some(zbytok) = url.strip_prefix("https://") {
        if let Some(lomitko) = zbytok.find('/') {
            let host = &zbytok[..lomitko];
            let cesta = &zbytok[lomitko..];
            return POVOLENE_HOSTITELE.contains(&host) && cesta.contains("/Burina13/tli-overlay/");
        }
    }
    false
}

/// Extrahuje všetky číselné časti z verzie pre spoľahlivé sémantické porovnanie.
/// Umožňuje nám to ignorovať tagy ako `-rc1` a sústrediť sa čisto na číslovanie releasov.
pub fn verzia_na_cisla(v: &str) -> Vec<u64> {
    let cisla: Vec<u64> = v
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();

    if cisla.is_empty() {
        vec![0]
    } else {
        cisla
    }
}

/// Vyhodnocuje, či má ponúkaná verzia vyššie číslovanie ako to, čo už beží.
/// Dĺžkové rozdiely ošetruje dopĺňaním núl (1.0 vs 1.0.1).
pub fn je_novsia(aktualna: &str, ponukana: &str) -> bool {
    let a = verzia_na_cisla(aktualna);
    let p = verzia_na_cisla(ponukana);
    
    let max_len = a.len().max(p.len());
    
    for i in 0..max_len {
        let cast_a = a.get(i).copied().unwrap_or(0);
        let cast_p = p.get(i).copied().unwrap_or(0);
        
        if cast_p > cast_a {
            return true;
        } else if cast_p < cast_a {
            return false;
        }
    }
    
    false
}

/// Hlavný bod vstupnej kontroly stiahnutého manifestu aktualizácií.
/// Odmietne poškodené formáty a zablokuje cudzie adresy bezpečným chybovým výstupom.
pub fn skontroluj_manifest(json: &str, aktualna_verzia: &str) -> Result<Option<Manifest>, String> {
    let manifest: Manifest = serde_json::from_str(json)
        .map_err(|_| "Nepodarilo sa spracovať odpoveď aktualizačného servera.".to_string())?;

    if !je_povolena_adresa(&manifest.msi_url) {
        return Err("Ponúkaná inštalácia pochádza z nedôveryhodnej adresy.".to_string());
    }

    if je_novsia(aktualna_verzia, &manifest.version) {
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn manifest_url_je_nas() {
        // Jediný zdroj aktualizácií. Keby sa táto konštanta raz zmenila na cudziu
        // adresu, padne tento test — presne to je incident A2Tools z 13.10.2026.
        assert!(je_povolena_adresa(MANIFEST_URL));
        assert!(MANIFEST_URL.contains("Burina13/tli-overlay"));
        assert!(MANIFEST_URL.ends_with("/latest-tli.json"));
        assert!(MANIFEST_URL.starts_with("https://raw.githubusercontent.com/"));
    }

    #[test]
    fn cudzie_adresy_sa_odmietnu() {
        assert!(!je_povolena_adresa("http://raw.githubusercontent.com/Burina13/tli-overlay/msi"));
        assert!(!je_povolena_adresa("https://raw.githubusercontent.com/Burina13/A2Tools-DPS-Meter/msi"));
        assert!(!je_povolena_adresa("https://tli.example.com/latest-tli.json"));
        assert!(!je_povolena_adresa("https://raw.githubusercontent.com.zly.sk/Burina13/tli-overlay/"));
        assert!(!je_povolena_adresa("https://evil.sk/?x=raw.githubusercontent.com/Burina13/tli-overlay/"));
    }

    #[test]
    fn novsia_verzia() {
        assert!(je_novsia("1.0.0", "1.0.1"));
        assert!(je_novsia("1.0.1", "1.1.0"));
        assert!(je_novsia("1.1.0", "2.0.0"));
        assert!(!je_novsia("2.0.0", "2.0.0"));
        assert!(!je_novsia("1.0.1", "1.0.0"));
        
        // Ekvivalent
        assert!(!je_novsia("0.1.0", "0.1"));
        assert!(!je_novsia("0.1", "0.1.0"));
    }

    #[test]
    fn manifest_s_cudzim_msi_je_chyba() {
        let json = r#"{
            "version": "9.9.9",
            "notes": "",
            "msiUrl": "https://evil.sk/virus.msi",
            "msiSha256": ""
        }"#;
        
        let vysledok = skontroluj_manifest(json, "1.0.0");
        assert!(vysledok.is_err());
        assert_eq!(vysledok.unwrap_err(), "Ponúkaná inštalácia pochádza z nedôveryhodnej adresy.");
    }

    #[test]
    fn manifest_s_rovnakou_verziou_nic_neponukne() {
        let json = r#"{
            "version": "1.0.0",
            "notes": "",
            "msiUrl": "https://github.com/Burina13/tli-overlay/test.msi",
            "msiSha256": ""
        }"#;
        
        let vysledok = skontroluj_manifest(json, "1.0.0").unwrap();
        assert!(vysledok.is_none());
    }
}
