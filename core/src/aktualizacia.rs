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
    /// TLI 52, časť 2/2: adresa NSIS `-setup.exe` (perUser, tichá inštalácia bez UAC).
    /// `#[serde(default)]`, lebo starý `latest-tli.json` toto pole nemá a appka ho
    /// musí vedieť prečítať aj tak — a opačne, appka postavená z tohto kódu musí
    /// vedieť spracovať manifest, v ktorom toto pole ešte chýba (migrácia nie je
    /// overená na Richardovom PC, viď CHANGELOG, preto sa pole zatiaľ do živého
    /// `latest-tli.json` nedopĺňa).
    #[serde(rename = "installerUrl", default)]
    pub installer_url: Option<String>,
    #[serde(rename = "installerSha256", default)]
    pub installer_sha256: Option<String>,
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

    if let Some(url) = &manifest.installer_url {
        if !je_povolena_adresa(url) {
            return Err("Ponúkaná inštalácia pochádza z nedôveryhodnej adresy.".to_string());
        }
    }

    if je_novsia(aktualna_verzia, &manifest.version) {
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

/// Overí, že stiahnuté bajty inštalátora majú presne ten SHA256 odtlačok, ktorý
/// sľúbil manifest — bez tohto by si appka naschvál spustila čokoľvek, čo jej
/// niekto podsunie na už overenej adrese (kompromitovaný release asset, MITM
/// pri zlyhaní TLS). Porovnanie je bez ohľadu na veľkosť písmen, lebo hex
/// reťazce v manifeste aj v `SHA256SUMS.txt` z CI bývajú malými písmenami,
/// ale nie je dôvod sa na to spoliehať.
pub fn over_sha256(data: &[u8], ocakavany: &str) -> bool {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let vysledok = hasher.finalize();
    let hex: String = vysledok.iter().map(|b| format!("{b:02x}")).collect();
    hex.eq_ignore_ascii_case(ocakavany.trim())
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

    // TLI 52, časť 2/2 — od tohto bodu nadol.

    #[test]
    fn stary_manifest_bez_installer_poli_sa_precita() {
        // Presne tvar dnešného živého `latest-tli.json` — appka postavená z
        // TOHTO kódu ho musí vedieť spracovať bez pádu, kým sa tichý update
        // nepustí do prevádzky.
        let json = r#"{
            "version": "9.9.9",
            "notes": "",
            "msiUrl": "https://github.com/Burina13/tli-overlay/releases/download/tli-v9.9.9/x.msi",
            "msiSha256": "abc"
        }"#;
        let manifest = skontroluj_manifest(json, "1.0.0").unwrap().unwrap();
        assert!(manifest.installer_url.is_none());
        assert!(manifest.installer_sha256.is_none());
    }

    #[test]
    fn novy_manifest_s_installer_polami_sa_precita() {
        let json = r#"{
            "version": "9.9.9",
            "notes": "",
            "msiUrl": "https://github.com/Burina13/tli-overlay/releases/download/tli-v9.9.9/x.msi",
            "msiSha256": "abc",
            "installerUrl": "https://github.com/Burina13/tli-overlay/releases/download/tli-v9.9.9/x-setup.exe",
            "installerSha256": "def"
        }"#;
        let manifest = skontroluj_manifest(json, "1.0.0").unwrap().unwrap();
        assert_eq!(manifest.installer_url.as_deref(), Some("https://github.com/Burina13/tli-overlay/releases/download/tli-v9.9.9/x-setup.exe"));
        assert_eq!(manifest.installer_sha256.as_deref(), Some("def"));
    }

    #[test]
    fn manifest_s_cudzim_installerom_je_chyba() {
        // Rovnaká zarážka ako pri `msiUrl` (incident A2Tools) — platí aj na
        // druhé pole, inak by ňou niekto mohol obísť kontrolu adresy.
        let json = r#"{
            "version": "9.9.9",
            "notes": "",
            "msiUrl": "https://github.com/Burina13/tli-overlay/releases/download/tli-v9.9.9/x.msi",
            "msiSha256": "abc",
            "installerUrl": "https://evil.sk/virus-setup.exe",
            "installerSha256": "def"
        }"#;
        let vysledok = skontroluj_manifest(json, "1.0.0");
        assert!(vysledok.is_err());
    }

    #[test]
    fn sha256_sa_zhoduje() {
        // Nezávisle spočítaný odtlačok (`python3 -c "import hashlib;
        // print(hashlib.sha256(b'tli').hexdigest())"`).
        let spravny = "dc872fb7937503df9c0d16fe4bcd31661150ba08d70ab81c4fa0d391a6ca490d";
        let data = b"tli";
        assert!(over_sha256(data, spravny));
        assert!(over_sha256(data, &spravny.to_uppercase()));
        assert!(!over_sha256(data, "0000000000000000000000000000000000000000000000000000000000000"));
        assert!(!over_sha256(b"ina data", spravny));
    }
}
