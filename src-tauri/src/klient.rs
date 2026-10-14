use tli_overlay_core::aktualizacia;
use tli_overlay_core::api;
use tli_overlay_core::api::Analyza;
use tli_overlay_core::nastavenia::Nastavenia;

#[derive(Debug)]
pub struct Vysledok {
    pub analyza: Analyza,
    pub trvanie_ms: u128,
}

/// Odosle predpripravenu JPEG snimku na analyzu na server.
/// DOVOD: Tato operacia umoznuje klientskej aplikacii ziskat spravne zhodnotenie pre svoj krok
/// v Torchlight: Infinite (odporucanie postupu pre herne mechaniky na serveri namiesto narocneho rátania lokálne).
pub fn posli_analyzu(n: &Nastavenia, jpeg: Vec<u8>) -> Result<Vysledok, String> {
    let zaciatok = std::time::Instant::now();

    let odpoved = ureq::post(&n.adresa_analyza())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", "image/jpeg")
        .timeout(std::time::Duration::from_secs(60))
        .send_bytes(&jpeg);

    match odpoved {
        Ok(res) => {
            let telo = res.into_string().unwrap_or_default();
            let analyza: Analyza = serde_json::from_str(&telo)
                .map_err(|_| "Server odpovedal niečím, čomu nerozumiem.".to_string())?;

            Ok(Vysledok {
                analyza,
                trvanie_ms: zaciatok.elapsed().as_millis(),
            })
        }
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        // Zvyšok sú chyby prenosu (DNS, TLS, timeout). Zámerne catch-all, nie
        // `Transport(t)` — `ureq::Error` môže pribudnúť variant a appka sa nemá
        // prestať kompilovať pri `cargo update`.
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}

/// Preveri funkcne pripojenie z aplikacie k API sluzbe pre spravnu odozvu.
/// DOVOD: Hrac si vdaka testu pripojenia dokaze overit platnost svojich registracnych udajov v overlay bez
/// nutnosti prepinat z a do hry.
pub fn test_pripojenia(n: &Nastavenia) -> Result<String, String> {
    let odpoved = ureq::get(&n.adresa_zdravie())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .timeout(std::time::Duration::from_secs(10))
        .call();

    match odpoved {
        Ok(res) => {
            let telo = res.into_string().unwrap_or_default();
            let json: serde_json::Value = serde_json::from_str(&telo).unwrap_or_default();

            let sluzba = json["sluzba"].as_str().unwrap_or("?");
            let limit = json["limit_za_minutu"].as_i64().map(|v| v.to_string()).unwrap_or_else(|| "?".to_string());

            Ok(format!("Spojenie funguje. Server: {}, limit {} požiadaviek za minútu.", sluzba, limit))
        }
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        // Zvyšok sú chyby prenosu (DNS, TLS, timeout). Zámerne catch-all, nie
        // `Transport(t)` — `ureq::Error` môže pribudnúť variant a appka sa nemá
        // prestať kompilovať pri `cargo update`.
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}

/// Ziska informaciu z overeneho repozitara o prichadzajucej softverovej aktualizacii z webu.
/// DOVOD: Je zivotne dolezite, aby hrac pouzival pre hru nalezitu verziu analytika (aktualny herny build zisti iba aktualizovany klient).
pub fn stiahni_manifest() -> Result<String, String> {
    // Zarazka proti tomu, aby sa konstanta raz prepisala na cudzi endpoint
    if !aktualizacia::je_povolena_adresa(aktualizacia::MANIFEST_URL) {
        return Err("Nepovolená adresa manifestu pre aktualizácie.".to_string());
    }

    let odpoved = ureq::get(aktualizacia::MANIFEST_URL)
        .timeout(std::time::Duration::from_secs(15))
        .call();

    match odpoved {
        Ok(res) => Ok(res.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        // Zvyšok sú chyby prenosu (DNS, TLS, timeout). Zámerne catch-all, nie
        // `Transport(t)` — `ureq::Error` môže pribudnúť variant a appka sa nemá
        // prestať kompilovať pri `cargo update`.
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn test_adresa_manifestu_je_v_poriadku() {
        assert!(aktualizacia::je_povolena_adresa(aktualizacia::MANIFEST_URL));
    }
}
