use tli_overlay_core::aktualizacia;
use tli_overlay_core::api;
use tli_overlay_core::api::{Analyza, AnalyzaCanvas, AnalyzaCeny, MinimapaOdpoved, MinimapaStavOdpoved};
use tli_overlay_core::nastavenia::Nastavenia;

/// TLI 29: keď server odpovie 2xx, ale telo sa nedá rozobrať na očakávaný
/// tvar (napr. pole nekompatibilného typu — presne to sa stalo 18.10.2026,
/// `cez_scratch: null` proti neoptionálnemu `bool`), predtým appka ukázala
/// len „Server odpovedal niečím, čomu nerozumiem." bez stopy, kde hľadať.
/// Celé telo (do 200 znakov, nech log neopuchne pri veľkej odpovedi) ide do
/// denníka, hráč v toaste vidí len HTTP kód a odkaz na log.
///
/// `#[cfg(not(test))]`: `tests/e2e_mock.rs` pridáva tento súbor do VLASTNEJ
/// testovacej binárky cez `#[path]` bez zvyšku appky (`denik`/`stav` tam
/// nie sú a ani nemajú byť — mock test nemá prístup k priečinku appky na
/// disku). `cfg(test)` je v tejto binárke pravdivé, takže sa namiesto
/// reálneho zápisu použije no-op a `crate::denik` sa vôbec nemusí nájsť.
#[cfg(not(test))]
fn zapis_do_denika(kontext: &str, text: &str) {
    crate::denik::zapis(kontext, text);
}
#[cfg(test)]
fn zapis_do_denika(_kontext: &str, _text: &str) {}

fn loguj_neparsovatelnu_odpoved(kontext: &str, stav: u16, telo: &str) -> String {
    let ukazka: String = telo.chars().take(200).collect();
    zapis_do_denika(
        "analyza",
        &format!("{kontext}: HTTP {stav} sa nedalo rozobrať, telo (prvých 200 znakov): {ukazka}"),
    );
    api::sprava_neparsovatelnej_odpovede(stav)
}

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

#[derive(Debug)]
pub struct VysledokCeny {
    pub analyza: AnalyzaCeny,
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
            let stav = res.status();
            let telo = res.into_string().unwrap_or_default();
            let analyza: Analyza = serde_json::from_str(&telo)
                .map_err(|_| loguj_neparsovatelnu_odpoved("posli_analyzu", stav, &telo))?;

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
            let stav = res.status();
            let telo = res.into_string().unwrap_or_default();
            let analyza: AnalyzaCanvas = serde_json::from_str(&telo)
                .map_err(|_| loguj_neparsovatelnu_odpoved("posli_analyzu_canvas", stav, &telo))?;

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

/// Automatický zber na učenie (TLI 60) — snímka ide na ten istý endpoint ako
/// Frozen Canvas, len s `zber=1`, a odpoveď servera nás NEZAUJÍMA. Appka
/// hráčovi nič neukazuje, takže netreba parsovať ani solverom navrhnuté kroky;
/// stačí vedieť, či server snímku prijal (aby sa dala vyhodiť z fronty), alebo
/// nie (aby sa skúsila znova).
///
/// Kratší timeout než pri ručnej analýze (60 s) je zámer: na pozadí nikto
/// nečaká, a keď server odpovedá pomaly, je lepšie sa k snímke vrátiť neskôr
/// než držať spojenie a blokovať zvyšok fronty.
pub fn posli_canvas_zber(n: &Nastavenia, jpeg: Vec<u8>) -> Result<(), String> {
    let odpoved = ureq::post(&n.adresa_analyza_canvas_zber())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", "image/jpeg")
        .timeout(std::time::Duration::from_secs(30))
        .send_bytes(&jpeg);

    match odpoved {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}

/// Pošle jeden výrez minimapy na `POST /api/minimapa` (TLI 62/3,
/// `minimapa-api-kontrakt.md`). Vracia vždy rozobraté telo, aj keď server
/// snímku odmietol (`prijate: false`) — to NIE JE chyba, len pokyn „prestaň
/// fotiť". Skutočné `Err` je len HTTP chyba/zlyhanie prenosu.
///
/// Kratší timeout než pri ručnej analýze (60 s), rovnako ako pri
/// `posli_canvas_zber`: appka zbiera na pozadí, nikto nečaká na odpoveď.
#[allow(clippy::too_many_arguments)]
pub fn posli_minimapa_zber(
    n: &Nastavenia,
    jpeg: Vec<u8>,
    rezim: &str,
    mapa: &str,
    level_uid: u64,
    cas_unix_s: u64,
    okno: Option<(u32, u32)>,
    roi: &str,
) -> Result<MinimapaOdpoved, String> {
    let mut req = ureq::post(&n.adresa_minimapa())
        .query("rezim", rezim)
        .query("mapa", mapa)
        .query("level_uid", &level_uid.to_string())
        .query("cas", &cas_unix_s.to_string())
        .query("roi", roi)
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", "image/jpeg")
        .timeout(std::time::Duration::from_secs(30));
    if let Some((sirka, vyska)) = okno {
        req = req.query("okno", &format!("{sirka}x{vyska}"));
    }

    let odpoved = req.send_bytes(&jpeg);

    match odpoved {
        Ok(res) => {
            let stav = res.status();
            let telo = res.into_string().unwrap_or_default();
            serde_json::from_str(&telo)
                .map_err(|_| loguj_neparsovatelnu_odpoved("posli_minimapa_zber", stav, &telo))
        }
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}

/// `GET /api/minimapa/stav` (TLI 62/3) — appka sa pýta raz za čas
/// (`DostDatStav::ma_sa_opytat_servera`), nie pri každej snímke.
pub fn ziskaj_stav_minimapy(n: &Nastavenia) -> Result<MinimapaStavOdpoved, String> {
    let odpoved = ureq::get(&n.adresa_minimapa_stav())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .timeout(std::time::Duration::from_secs(15))
        .call();

    match odpoved {
        Ok(res) => {
            let stav = res.status();
            let telo = res.into_string().unwrap_or_default();
            serde_json::from_str(&telo)
                .map_err(|_| loguj_neparsovatelnu_odpoved("ziskaj_stav_minimapy", stav, &telo))
        }
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
        Err(ina) => Err(api::sprava_siete(&ina.to_string())),
    }
}

/// Rovnaké ako `posli_analyzu`, len pre ceny z obrazovky trhu (TLI 22) — iný
/// endpoint (`?modul=ceny`) a iný tvar odpovede (meno položky + ceny, žiadne
/// súradnice — nič sa nekreslí nad hrou).
pub fn posli_analyzu_ceny(n: &Nastavenia, jpeg: Vec<u8>) -> Result<VysledokCeny, String> {
    let zaciatok = std::time::Instant::now();

    let odpoved = ureq::post(&n.adresa_analyza_ceny())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", "image/jpeg")
        .timeout(std::time::Duration::from_secs(60))
        .send_bytes(&jpeg);

    match odpoved {
        Ok(res) => {
            let stav = res.status();
            let telo = res.into_string().unwrap_or_default();
            let analyza: AnalyzaCeny = serde_json::from_str(&telo)
                .map_err(|_| loguj_neparsovatelnu_odpoved("posli_analyzu_ceny", stav, &telo))?;

            Ok(VysledokCeny {
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

/// Stiahne NSIS `-setup.exe` pre tichú aktualizáciu (TLI 52, časť 2/2) — surové
/// bajty, nie text. Adresu si volajúci overuje VOPRED cez
/// `aktualizacia::je_povolena_adresa` (rovnako ako pri `stiahni_manifest`),
/// SHA256 bajtov sa porovnáva až po stiahnutí (`aktualizacia::over_sha256`),
/// lebo obsah ani veľkosť vopred nepoznáme. Limit 200 MB je len poistka proti
/// nezmyselne veľkej odpovedi — náš inštalátor má rádovo desiatky MB.
pub fn stiahni_installer(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;

    if !aktualizacia::je_povolena_adresa(url) {
        return Err("Nepovolená adresa inštalátora pre aktualizácie.".to_string());
    }

    let odpoved = ureq::get(url)
        .timeout(std::time::Duration::from_secs(180))
        .call();

    match odpoved {
        Ok(res) => {
            let mut bajty = Vec::new();
            res.into_reader()
                .take(200 * 1024 * 1024)
                .read_to_end(&mut bajty)
                .map_err(|e| format!("Nepodarilo sa stiahnuť inštalátor: {e}"))?;
            Ok(bajty)
        }
        Err(ureq::Error::Status(kod, odpoved)) => {
            let telo = odpoved.into_string().unwrap_or_default();
            Err(api::sprava_chyby(kod, &telo))
        }
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

/// Zabalí bajty gzipom — server (`_rozbal_gzip` v `app.py`) iný formát
/// neprijme. Predvolená úroveň kompresie je rýchlostný kompromis: appka
/// toto volá v slučke po `upload::KUSOK_BAJTOV` kusoch na pozadí hry, nemá
/// zmysel míňať čas na maximálnu kompresiu textového logu.
fn gzipuj(data: &[u8]) -> Result<Vec<u8>, String> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(data).map_err(|e| format!("gzip zlyhal: {e}"))?;
    e.finish().map_err(|e| format!("gzip zlyhal: {e}"))
}

/// Ručne poskladané `multipart/form-data` telo — appka nepridáva kvôli
/// jedinému uploadu novú závislosť, formát je jednoduchý a stabilný (RFC 7578).
/// Vracia `(boundary, telo)`; `boundary` ide do hlavičky `Content-Type`.
fn multipart_telo(polia: &[(&str, &str)], meno_suboru: &str, data: &[u8]) -> (String, Vec<u8>) {
    let boundary = format!("----tliupload{}{}", std::process::id(), data.len());
    let mut telo = Vec::with_capacity(data.len() + 512);
    for (meno, hodnota) in polia {
        telo.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{meno}\"\r\n\r\n{hodnota}\r\n"
            )
            .as_bytes(),
        );
    }
    telo.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"data\"; filename=\"{meno_suboru}\"\r\nContent-Type: application/gzip\r\n\r\n"
        )
        .as_bytes(),
    );
    telo.extend_from_slice(data);
    telo.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary, telo)
}

/// Pošle jeden gzip segment surového `UE_game*.log` na `POST /api/tracker/upload`
/// (TLI 14). Telo odpovede sa vracia AJ pri chybovom HTTP stave (400/409/413) —
/// server naň vždy odpovedá zrozumiteľným JSON-om (napr. 409 nesie
/// `ocakavana_pozicia`, podľa ktorej si appka vie sama opraviť ďalší pokus),
/// takže len transportná chyba (DNS, TLS, timeout) je tu skutočné `Err`.
pub fn posli_upload_log(
    n: &Nastavenia,
    subor: &str,
    pozicia_od: u64,
    surove: &[u8],
) -> Result<tli_overlay_core::tracker::upload::OdpovedUploadu, String> {
    let gz = gzipuj(surove)?;
    let (boundary, telo) = multipart_telo(
        &[("typ", "log"), ("subor", subor), ("pozicia_od", &pozicia_od.to_string())],
        "log.gz",
        &gz,
    );

    let odpoved = ureq::post(&n.adresa_tracker_upload())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", &format!("multipart/form-data; boundary={boundary}"))
        .timeout(std::time::Duration::from_secs(120))
        .send_bytes(&telo);

    let telo_odpovede = match odpoved {
        Ok(res) => res.into_string().unwrap_or_default(),
        Err(ureq::Error::Status(_, res)) => res.into_string().unwrap_or_default(),
        Err(ina) => return Err(api::sprava_siete(&ina.to_string())),
    };
    serde_json::from_str(&telo_odpovede)
        .map_err(|_| "Server odpovedal na upload logu niečím, čomu appka nerozumie.".to_string())
}

/// Pošle export lokálnej DB (TLI 14) — vždy ako jeden celý JSON, bez
/// delenia na kusy (export je rádovo desiatky/stovky KB, nie stovky MB ako
/// surový log).
pub fn posli_db_export(
    n: &Nastavenia,
    json_obsah: &[u8],
) -> Result<tli_overlay_core::tracker::upload::OdpovedUploadu, String> {
    let gz = gzipuj(json_obsah)?;
    let (boundary, telo) = multipart_telo(&[("typ", "db_export")], "export.json.gz", &gz);

    let odpoved = ureq::post(&n.adresa_tracker_upload())
        .set("Authorization", &format!("Bearer {}", n.kluc))
        .set("Content-Type", &format!("multipart/form-data; boundary={boundary}"))
        .timeout(std::time::Duration::from_secs(60))
        .send_bytes(&telo);

    let telo_odpovede = match odpoved {
        Ok(res) => res.into_string().unwrap_or_default(),
        Err(ureq::Error::Status(_, res)) => res.into_string().unwrap_or_default(),
        Err(ina) => return Err(api::sprava_siete(&ina.to_string())),
    };
    serde_json::from_str(&telo_odpovede)
        .map_err(|_| "Server odpovedal na DB export niečím, čomu appka nerozumie.".to_string())
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn test_adresa_manifestu_je_v_poriadku() {
        assert!(aktualizacia::je_povolena_adresa(aktualizacia::MANIFEST_URL));
    }

    #[test]
    fn gzipuj_sa_da_rozbalit_spat() {
        use std::io::Read;
        let povodne = b"ahoj svet, toto je testovaci obsah logu\n".repeat(50);
        let gz = gzipuj(&povodne).unwrap();
        assert_ne!(gz, povodne);
        let mut rozbalene = Vec::new();
        flate2::read::GzDecoder::new(&gz[..]).read_to_end(&mut rozbalene).unwrap();
        assert_eq!(rozbalene, povodne);
    }

    #[test]
    fn multipart_telo_obsahuje_vsetky_polia_a_data() {
        let (boundary, telo) = multipart_telo(
            &[("typ", "log"), ("subor", "UE_game.log"), ("pozicia_od", "123")],
            "log.gz",
            b"gzipdata",
        );
        let text = String::from_utf8_lossy(&telo);
        assert!(text.contains(&boundary));
        assert!(text.contains("name=\"typ\""));
        assert!(text.contains("log"));
        assert!(text.contains("name=\"subor\""));
        assert!(text.contains("UE_game.log"));
        assert!(text.contains("name=\"pozicia_od\""));
        assert!(text.contains("123"));
        assert!(text.contains("filename=\"log.gz\""));
        assert!(text.contains("application/gzip"));
        assert!(telo.windows(b"gzipdata".len()).any(|w| w == b"gzipdata"));
        assert!(text.trim_end().ends_with(&format!("--{boundary}--")));
    }
}
