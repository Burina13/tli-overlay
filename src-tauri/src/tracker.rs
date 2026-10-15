//! Farm tracker — čítanie herného logu a vydávanie hotových čísel do UI.
//!
//! Všetka logika (parsovanie riadkov, delty inventára, hranice behov, ceny,
//! agregácie) je v `tli_overlay_core::tracker` a testuje sa na Linuxe. Tu ostáva
//! len to, čo sa bez skutočného počítača nedá: nájsť súbor, čítať ho od poslednej
//! pozície a ustrážiť, že z toho nebude zožratý procesor ani zamknuté UI.
//!
//! **Appka do hry nijako nezasahuje — iba číta textový súbor, ktorý si hra sama
//! píše na disk** (a len keď si hráč v hre zapne „Enable Log"). Žiadne čítanie
//! pamäte procesu, žiadna injekcia, žiadne pakety — to je pravidlo projektu, nie
//! odporúčanie.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tli_overlay_core::tracker::agregacie::{ocen_beh, prehlad, Prehlad, VysledokBehu};
use tli_overlay_core::tracker::ceny::{vek_text, Cena, Cennik, Nastavenie, Zdroj};
use tli_overlay_core::tracker::stav::{Beh, Sledovac};

/// Koľko najviac bajtov prečítame naraz. Pri prvom spustení je log kľudne
/// stomegový a natiahnuť ho celý do pamäte by appku na pár sekúnd zmrazilo.
const DAVKA_B: u64 = 2 * 1024 * 1024;

/// Bežné miesta, kde hra log píše. Zoznam je zámerne krátky a doplniteľný ručne
/// v nastaveniach — hádať sa dá donekonečna, ale hráč to vie na jeden pokus.
const ZNAME_CESTY: &[&str] = &[
    r"C:\Program Files (x86)\Steam\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
    r"C:\Program Files\Steam\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
    r"D:\SteamLibrary\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
    r"E:\SteamLibrary\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
];

/// Stav trackera medzi dvoma čítaniami. Celé to drží jeden `Mutex` — čítanie
/// súboru aj prepočet sú rádovo milisekundy a dve vlákna sem nechodia.
pub struct StavTrackera {
    sledovac: Sledovac,
    cennik: Cennik,
    behy: Vec<Beh>,
    pozicia: u64,
    cesta: Option<PathBuf>,
    /// Zvyšok posledného, ešte nedokončeného riadku z predošlej dávky.
    zvysok: String,
    pub chyba: Option<String>,
}

impl Default for StavTrackera {
    fn default() -> Self {
        Self {
            sledovac: Sledovac::novy(),
            cennik: Cennik::novy(),
            behy: Vec::new(),
            pozicia: 0,
            cesta: None,
            zvysok: String::new(),
            chyba: None,
        }
    }
}

/// Nájde log: najprv ručne zadaná cesta, potom známe miesta.
pub fn najdi_log(rucna: &str) -> Option<PathBuf> {
    let rucna = rucna.trim();
    if !rucna.is_empty() {
        let p = PathBuf::from(rucna);
        return if p.is_file() { Some(p) } else { None };
    }
    ZNAME_CESTY
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

/// Prečíta nové bajty a pretlačí ich cez `Sledovac`.
///
/// Rotáciu logu (hra pri štarte súbor skráti alebo premenuje) spoznáme podľa
/// toho, že súbor je menší než naša pozícia — vtedy začíname odznova. Pri PRVOM
/// čítaní sa zámerne neskáče na koniec: hráč appku často spustí až po pár mapách
/// a história z dnešného logu je presne to, čo chce vidieť.
pub fn precitaj(stav: &mut StavTrackera, cesta: &Path) -> Result<usize, String> {
    let meta = std::fs::metadata(cesta).map_err(|e| format!("log sa nedá otvoriť: {e}"))?;
    let velkost = meta.len();

    if stav.cesta.as_deref() != Some(cesta) {
        // Iný súbor než naposledy — začíname načisto.
        *stav = StavTrackera { cesta: Some(cesta.to_path_buf()), ..Default::default() };
    }
    if velkost < stav.pozicia {
        stav.pozicia = 0;
        stav.zvysok.clear();
    }
    if velkost == stav.pozicia {
        return Ok(0);
    }

    let mut subor = std::fs::File::open(cesta).map_err(|e| format!("log sa nedá otvoriť: {e}"))?;
    subor
        .seek(SeekFrom::Start(stav.pozicia))
        .map_err(|e| format!("v logu sa nedá posunúť: {e}"))?;

    let kolko = (velkost - stav.pozicia).min(DAVKA_B) as usize;
    let mut bajty = vec![0u8; kolko];
    let precitane = subor
        .read(&mut bajty)
        .map_err(|e| format!("log sa nedá prečítať: {e}"))?;
    bajty.truncate(precitane);
    stav.pozicia += precitane as u64;

    // `dekoduj` rieši aj UTF-16 (Unreal naň prepne, keď sa do logu dostane
    // znak mimo ANSI — napríklad emoji v mene postavy).
    let text = tli_overlay_core::tracker::log::dekoduj(&bajty);
    let mut data = std::mem::take(&mut stav.zvysok);
    data.push_str(&text);

    // Posledný riadok môže byť odseknutý uprostred — odložíme ho na ďalšiu dávku.
    let konci_riadkom = data.ends_with('\n');
    let mut riadky: Vec<&str> = data.split('\n').collect();
    if !konci_riadkom {
        stav.zvysok = riadky.pop().unwrap_or("").to_string();
    }

    let mut novych = 0;
    for riadok in riadky {
        if let Some(beh) = stav.sledovac.riadok(riadok.trim_end_matches('\r')) {
            stav.behy.push(beh);
            novych += 1;
        }
    }

    let teraz = unixovy_cas();
    for (polozka, fe, _) in stav.sledovac.odober_ceny() {
        stav.cennik.vloz(Cena { polozka, fe, zdroj: Zdroj::VlastnyTrh, kedy: teraz });
    }

    Ok(novych)
}

/// Unixový čas v sekundách. Keby hodiny ukazovali pred rok 1970, radšej nula
/// než panika — vek ceny je kozmetika, appka na ňom nesmie spadnúť.
pub fn unixovy_cas() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Živý pohľad na práve bežiacu mapu — to, čo ukazuje mini-overlay.
#[derive(Debug, Clone, Serialize)]
pub struct Zivo {
    pub zona: String,
    pub trvanie_ms: i64,
    pub cisty_zisk: f64,
    pub podpis_vstupov: String,
}

/// Všetko, čo záložka Tracker potrebuje na jedno vykreslenie.
#[derive(Debug, Clone, Serialize)]
pub struct PrehladProUi {
    pub zapnuty: bool,
    pub cesta_logu: Option<String>,
    pub chyba: Option<String>,
    pub neznamych_riadkov: u64,
    pub zivo: Option<Zivo>,
    pub prehlad: Prehlad,
    pub behy: Vec<VysledokBehu>,
    /// Vek každej známej ceny v ľudskej podobe (`"400021" -> "pred 4 min"`).
    pub vek_cien: BTreeMap<String, String>,
    pub pocet_cien: usize,
}

/// Zostaví prehľad z aktuálneho stavu. Nič nečíta — to robí `precitaj`.
pub fn prehlad_pre_ui(stav: &StavTrackera, zapnuty: bool, dan: bool) -> PrehladProUi {
    let nastavenie = Nastavenie { dan };
    let vysledky: Vec<VysledokBehu> = stav
        .behy
        .iter()
        .filter_map(|b| ocen_beh(b, &stav.cennik, &nastavenie))
        .collect();
    let suhrn = prehlad(&vysledky, &stav.behy);

    let teraz = unixovy_cas();
    let mut vek_cien = BTreeMap::new();
    for cena in stav.cennik.vsetky() {
        let vek = teraz.saturating_sub(cena.kedy);
        vek_cien.insert(cena.polozka.to_string(), vek_text(vek));
    }

    let zivo = stav.sledovac.aktualny().and_then(|b| {
        if b.je_hub {
            return None;
        }
        let v = ocen_beh(b, &stav.cennik, &nastavenie)?;
        Some(Zivo {
            zona: b.zona.clone(),
            trvanie_ms: v.trvanie_ms,
            cisty_zisk: v.cisty_zisk,
            podpis_vstupov: v.podpis_vstupov,
        })
    });

    PrehladProUi {
        zapnuty,
        cesta_logu: stav.cesta.as_ref().map(|p| p.display().to_string()),
        chyba: stav.chyba.clone(),
        neznamych_riadkov: stav.sledovac.neznamych_riadkov(),
        zivo,
        prehlad: suhrn,
        behy: vysledky,
        vek_cien,
        pocet_cien: stav.cennik.pocet(),
    }
}

/// CSV s jedným riadkom na mapu — presne to, čo sa dá otvoriť v tabuľkovom
/// procesore a ďalej si s tým robiť, čo kto chce.
pub fn csv(vysledky: &[VysledokBehu]) -> String {
    let mut out = String::from(
        "id;mapa;chart_a_kompasy;trvanie_s;hodnota_dropov_fe;hodnota_vstupov_fe;cisty_zisk_fe\n",
    );
    for v in vysledky {
        out.push_str(&format!(
            "{};{};{};{};{:.2};{:.2};{:.2}\n",
            v.id,
            v.zona.replace(';', ","),
            v.podpis_vstupov,
            v.trvanie_ms / 1000,
            v.hodnota_dropov,
            v.hodnota_vstupov,
            v.cisty_zisk
        ));
    }
    out
}

/// Obal nad stavom, aby sa dal držať v `tauri::State`.
pub struct Tracker(pub Mutex<StavTrackera>);

impl Default for Tracker {
    fn default() -> Self {
        Tracker(Mutex::new(StavTrackera::default()))
    }
}
