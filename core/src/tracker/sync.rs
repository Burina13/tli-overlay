use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};

// Importujeme už existujúcu štruktúru podľa zadania.
use super::agregacie::VysledokBehu;

/// Vypočíta stabilný podpis pre daný beh.
/// 
/// PREČO používame FNV-1a a nie napr. SHA-256: 
/// Do tohto crate (tli-overlay-core) nechceme kvôli jednoduchej deduplikácii sťahovať 
/// ťažkú kryptografickú závislosť. Prípadná kolízia hashov nie je bezpečnostný problém,
/// znamená len nanajvýš to, že sa jedna odohratá mapa neodošle na server. Pre tento účel
/// je 64-bitový FNV-1a dostatočne rýchly a spoľahlivý.
pub fn kluc_behu(v: &VysledokBehu) -> String {
    let text_na_hash = format!("{}|{}|{}|{}", v.zaciatok, v.zona, v.podpis_kombinacie, v.trvanie_ms);
    
    // FNV-1a 64-bit parametre
    let mut hash: u64 = 0xcbf29ce484222325;
    let prime: u64 = 0x100000001b3;

    for bajt in text_na_hash.bytes() {
        hash ^= bajt as u64;
        hash = hash.wrapping_mul(prime);
    }

    // Výstup ako 16 znakov dlhý hexadecimálny reťazec
    format!("{:016x}", hash)
}

/// Dátová štruktúra reprezentujúca jednu odohratú mapu pripravenú na odoslanie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BehNaOdoslanie {
    pub kluc: String,
    pub zaciatok: String,
    pub zona: String,
    pub podpis_vstupov: String,
    pub podpis_kombinacie: String,
    pub trvanie_ms: i64,
    pub hodnota_dropov: f64,
    pub hodnota_vstupov: f64,
    pub cisty_zisk: f64,
}

/// Prevedie lokálny `VysledokBehu` na formu pre server, obohatenú o vypočítaný kľúč.
pub fn na_odoslanie(v: &VysledokBehu) -> BehNaOdoslanie {
    BehNaOdoslanie {
        kluc: kluc_behu(v),
        zaciatok: v.zaciatok.clone(),
        zona: v.zona.clone(),
        podpis_vstupov: v.podpis_vstupov.clone(),
        podpis_kombinacie: v.podpis_kombinacie.clone(),
        trvanie_ms: v.trvanie_ms,
        hodnota_dropov: v.hodnota_dropov,
        hodnota_vstupov: v.hodnota_vstupov,
        cisty_zisk: v.cisty_zisk,
    }
}

/// Zo zoznamu všetkých výsledkov vyberie tie, ktoré sa majú odoslať na server.
pub fn vyber_neposlane(
    vysledky: &[VysledokBehu],
    uz_poslane: &BTreeSet<String>,
    max: usize,
) -> Vec<BehNaOdoslanie> {
    vysledky
        .iter()
        .filter(|v| {
            // Preskočíme behy, ktorým chýba začiatok.
            // DÔVOD: Bez času začiatku z logu nie je podpis mapy stabilný.
            // Server by si takúto mapu uložil, ale po reštarte aplikácie by sa čas
            // mohol načítať inak, čím by sa zmenil hash a server by dostal duplikát.
            if v.zaciatok.is_empty() {
                return false;
            }
            // Zbytočné (alebo chybné) behy s nulovým či záporným časom ignorujeme.
            if v.trvanie_ms <= 0 {
                return false;
            }
            true
        })
        .map(na_odoslanie)
        .filter(|beh| !uz_poslane.contains(&beh.kluc))
        .take(max) // Obmedzíme na maximálny počet (spracováva sa od najstarších, ako prišli)
        .collect()
}

/// Pomocná štruktúra na vytvorenie JSON požiadavky.
#[derive(Serialize)]
struct TeloPoziadavky<'a> {
    klient: &'a str,
    behy: &'a [BehNaOdoslanie],
}

/// Vygeneruje JSON telo pre HTTP POST požiadavku.
pub fn telo_poziadavky(behy: &[BehNaOdoslanie], klient_verzia: &str) -> String {
    let poziadavka = TeloPoziadavky {
        klient: klient_verzia,
        behy,
    };
    
    // Ak by náhodou zlyhala serializácia (čo by pri týchto dátach nemala), 
    // nevrátime chybu, ale aspoň bezpečný prázdny JSON.
    serde_json::to_string(&poziadavka).unwrap_or_else(|_| "{}".to_string())
}

/// Odpoveď prijatá zo synchronizačného servera.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OdpovedSync {
    pub ok: bool,
    pub prijatych: u64,
    pub preskocenych: u64,
}

/// Rozparsuje odpoveď zo servera a zhodnotí, či bola úspešná.
pub fn z_odpovede(telo: &str) -> Result<OdpovedSync, String> {
    let odpoved: OdpovedSync = serde_json::from_str(telo).map_err(|_| {
        "Server odpovedal na synchronizáciu niečím, čomu appka nerozumie.".to_string()
    })?;

    if !odpoved.ok {
        return Err("Server synchronizáciu odmietol.".to_string());
    }

    Ok(odpoved)
}

/// Vygeneruje textové zhrnutie synchronizácie pre používateľské rozhranie.
pub fn zhrnutie(o: &OdpovedSync) -> String {
    if o.prijatych == 0 && o.preskocenych == 0 {
        return "Server už mal všetko.".to_string();
    }

    let slovo_novych = match o.prijatych {
        1 => "nový",
        2..=4 => "nové",
        _ => "nových",
    };

    format!(
        "Odoslané: {} {}, {} už server mal.",
        o.prijatych, slovo_novych, o.preskocenych
    )
}

#[cfg(test)]
mod testy {
    use super::*;

    /// Vytvorí štandardnú vzorku pre testovanie
    fn vzorka_behu() -> VysledokBehu {
        VysledokBehu {
            id: 1,
            zona: "Cemetery".to_string(),
            podpis_vstupov: "vstup123".to_string(),
            podpis_kombinacie: "kombinacia456".to_string(),
            zaciatok: "2023-10-25 15:30:00".to_string(),
            trvanie_ms: 120500,
            hodnota_dropov: 150.5,
            hodnota_vstupov: 20.0,
            cisty_zisk: 130.5,
            bez_ceny: vec![],
            najstarsia_cena: None,
        }
    }

    #[test]
    fn test_kluc_behu_je_stabilny() {
        let beh1 = vzorka_behu();
        let beh2 = vzorka_behu();
        assert_eq!(kluc_behu(&beh1), kluc_behu(&beh2));
        assert_eq!(kluc_behu(&beh1).len(), 16);
    }

    #[test]
    fn test_kluc_behu_sa_lisi_pri_zmene_pola() {
        let zaklad = vzorka_behu();

        let mut iny_zaciatok = zaklad.clone();
        iny_zaciatok.zaciatok = "2023-10-25 15:30:01".to_string();
        assert_ne!(kluc_behu(&zaklad), kluc_behu(&iny_zaciatok));

        let mut ina_zona = zaklad.clone();
        ina_zona.zona = "Dunes".to_string();
        assert_ne!(kluc_behu(&zaklad), kluc_behu(&ina_zona));

        let mut iny_podpis = zaklad.clone();
        iny_podpis.podpis_kombinacie = "ina_kombinacia".to_string();
        assert_ne!(kluc_behu(&zaklad), kluc_behu(&iny_podpis));

        let mut iny_cas = zaklad.clone();
        iny_cas.trvanie_ms = 120501;
        assert_ne!(kluc_behu(&zaklad), kluc_behu(&iny_cas));
    }

    #[test]
    fn test_vyber_neposlane_ignoruje_prazdny_zaciatok() {
        let mut beh = vzorka_behu();
        beh.zaciatok = "".to_string(); // prázdny
        
        let uz_poslane = BTreeSet::new();
        let vysledok = vyber_neposlane(&[beh], &uz_poslane, 10);
        assert!(vysledok.is_empty());
    }

    #[test]
    fn test_vyber_neposlane_ignoruje_nulove_trvanie() {
        let mut beh1 = vzorka_behu();
        beh1.trvanie_ms = 0;
        
        let mut beh2 = vzorka_behu();
        beh2.trvanie_ms = -500;

        let uz_poslane = BTreeSet::new();
        let vysledok = vyber_neposlane(&[beh1, beh2], &uz_poslane, 10);
        assert!(vysledok.is_empty());
    }

    #[test]
    fn test_vyber_neposlane_ignoruje_uz_poslane() {
        let beh = vzorka_behu();
        let kluc = kluc_behu(&beh);
        
        let mut uz_poslane = BTreeSet::new();
        uz_poslane.insert(kluc);

        let vysledok = vyber_neposlane(&[beh], &uz_poslane, 10);
        assert!(vysledok.is_empty());
    }

    #[test]
    fn test_vyber_neposlane_dodrzuje_max() {
        let mut behy = vec![];
        for i in 0..5 {
            let mut b = vzorka_behu();
            b.trvanie_ms += i; // aby bol kľúč iný
            behy.push(b);
        }

        let uz_poslane = BTreeSet::new();
        let vysledok = vyber_neposlane(&behy, &uz_poslane, 3);
        assert_eq!(vysledok.len(), 3);
    }

    #[test]
    fn test_telo_poziadavky_je_platny_json() {
        let b = na_odoslanie(&vzorka_behu());
        let json = telo_poziadavky(&[b], "v1.2.3");
        
        // V jednoduchosti skontrolujeme výskyt kľúčových slov v reťazci
        assert!(json.contains(r#""klient":"v1.2.3""#));
        assert!(json.contains(r#""behy":[{"#));
        assert!(json.contains(r#""zona":"Cemetery""#));
        
        // Overíme syntaktickú správnosť deserializáciou
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_z_odpovede_uspech() {
        let json = r#"{"ok": true, "prijatych": 5, "preskocenych": 2}"#;
        let odpoved = z_odpovede(json).unwrap();
        assert_eq!(odpoved.prijatych, 5);
        assert_eq!(odpoved.preskocenych, 2);
    }

    #[test]
    fn test_z_odpovede_zamietnutie() {
        let json = r#"{"ok": false, "prijatych": 0, "preskocenych": 0}"#;
        let err = z_odpovede(json).unwrap_err();
        assert_eq!(err, "Server synchronizáciu odmietol.");
    }

    #[test]
    fn test_z_odpovede_zle_telo() {
        let smeti = "toto urcite nie je json";
        let err = z_odpovede(smeti).unwrap_err();
        assert_eq!(err, "Server odpovedal na synchronizáciu niečím, čomu appka nerozumie.");
    }

    #[test]
    fn test_zhrnutie_rozne_stavy() {
        let o_nic = OdpovedSync { ok: true, prijatych: 0, preskocenych: 0 };
        assert_eq!(zhrnutie(&o_nic), "Server už mal všetko.");

        let o_jeden = OdpovedSync { ok: true, prijatych: 1, preskocenych: 3 };
        assert_eq!(zhrnutie(&o_jeden), "Odoslané: 1 nový, 3 už server mal.");

        let o_tri = OdpovedSync { ok: true, prijatych: 3, preskocenych: 0 };
        assert_eq!(zhrnutie(&o_tri), "Odoslané: 3 nové, 0 už server mal.");

        let o_pat = OdpovedSync { ok: true, prijatych: 5, preskocenych: 1 };
        assert_eq!(zhrnutie(&o_pat), "Odoslané: 5 nových, 1 už server mal.");
        
        let o_nula_prijata_nieco_preskocene = OdpovedSync { ok: true, prijatych: 0, preskocenych: 5 };
        assert_eq!(zhrnutie(&o_nula_prijata_nieco_preskocene), "Odoslané: 0 nových, 5 už server mal.");
    }
}
