use tli_overlay_core::aktualizacia;
use tli_overlay_core::api;
use tli_overlay_core::api::{Analyza, AnalyzaCanvas};
use tli_overlay_core::nastavenia::Nastavenia;

#[derive(Debug)]
pub struct Vysledok {
    pub analyza: Analyza,
    pub trvanie_ms: u128,
}

#[derive(Debug)]
pub struct VysledokCanvas {
    pub analyza: AnalyzaCanvas,
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

/// Rovnaké ako `posli_analyzu`, len pre Frozen Canvas (TLI 4b) — iný endpoint
/// (`?modul=canvas`) a iný tvar odpovede (`kroky[]` namiesto `odporucanie`).
pub fn posli_analyzu_canvas(n: &Nastavenia, jpeg: Vec<u8>) -> Result<VysledokCanvas, String> {
    let zaciatok = std::time::Instant::now();

    let odpoved = ureq::post(&n.adresa_analyza_canvas())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", "image/jpeg")
        .timeout(std::time::Duration::from_secs(60))
        .send_bytes(&jpeg);

    match odpoved {
        Ok(res) => {
            let telo = res.into_string().unwrap_or_default();
            let analyza: AnalyzaCanvas = serde_json::from_str(&telo)
                .map_err(|_| "Server odpovedal niečím, čomu nerozumiem.".to_string())?;

            Ok(VysledokCanvas {
                analyza,
                trvanie_ms: zaciatok.elapsed().as_millis(),
            })
        }
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
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

/// Stiahne celý sprievodca Erika 1 zo servera (TLI 11, krok 4).
pub fn stiahni_guide(
    n: &Nastavenia,
    parameter_verzie: &str,
) -> Result<Option<String>, String> {
    // Vnútorný import pre Read, aby sme neovplyvňovali zvyšok súboru
    use std::io::Read;

    let url = format!("{}{}", n.adresa("/api/guide"), parameter_verzie);
    let odpoved = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .timeout(std::time::Duration::from_secs(120))
        .call();

    match odpoved {
        Ok(res) => {
            // Server vráti 204 No Content ak máme najaktuálnejšiu verziu podľa parametra ?ak_novsie_ako=
            if res.status() == 204 {
                return Ok(None);
            }

            // NEPOUŽÍVAME res.into_string() !
            // Knižnica ureq má pri `into_string()` pevný vnútorný limit na veľkosť odpovede (10 MB).
            // Náš sprievodca obsahuje vložené ikony a telo môže tento limit presiahnuť.
            // Preto čítame telo priamo z readera a nastavujeme rozumný limit (16 MB).
            let mut telo = String::new();
            res.into_reader()
                .take(16 * 1024 * 1024)
                .read_to_string(&mut telo)
                .map_err(|e| format!("Nepodarilo sa načítať sprievodcu (odpoveď je možno príliš veľká): {}", e))?;

            Ok(Some(telo))
        }
        Err(ureq::Error::Status(kod, odpoved)) => {
            // Chybové odpovede sú malé (iba JSON chyby), takže here do_string() nevadí
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}

/// Pošle dávku odfarmených máp na partiový server (TLI 11, krok 5).
/// Volá sa LEN keď má hráč `tracker_sync` zapnutý — kontroluje to volajúci.
pub fn posli_tracker_sync(n: &Nastavenia, telo: &str) -> Result<String, String> {
    let odpoved = ureq::post(&n.adresa_tracker_sync())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(30))
        .send_string(telo);

    match odpoved {
        Ok(res) => Ok(res.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}


/// Pošle partii ceny, ktoré si hráč sám vyhľadal na in-game Exchange (TLI 20).
/// Volá sa LEN keď má hráč `ceny_sync` zapnuté — kontroluje to volajúci.
pub fn posli_ceny(n: &Nastavenia, telo: &str) -> Result<String, String> {
    let odpoved = ureq::post(&n.adresa_ceny())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(30))
        .send_string(telo);

    match odpoved {
        Ok(res) => Ok(res.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}

/// Stiahne agregát cien partie. `od` je serverový čas poslednej úspešnej
/// dávky (0 = chcem celý cenník).
pub fn stiahni_ceny(n: &Nastavenia, od: i64) -> Result<String, String> {
    let url = format!("{}?od={}", n.adresa_ceny(), od.max(0));
    let odpoved = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .timeout(std::time::Duration::from_secs(30))
        .call();

    match odpoved {
        Ok(res) => Ok(res.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
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
