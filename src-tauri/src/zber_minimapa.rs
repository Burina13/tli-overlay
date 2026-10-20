//! Zber snímok minimapy nad bežiacou hrou (TLI 62/3, zadanie
//! `inbox/2026-10-20-0600-agent.md`) — prepája `core::minimapa_fronta`
//! (ROI, ticker, denný strop, fronta na disku, `dost_dat`) so skutočným
//! oknom hry, diskom a sieťou. Kontrakt: `minimapa-api-kontrakt.md`.
//!
//! Vzor je `zber_canvas.rs` (TLI 60): vlastné vlákno, vlastný stav, vlastná
//! HTTP cesta — žiadne zdieľanie s Canvasom okrem všeobecných pomôcok
//! (`obraz`, `klient`, `okno`, `snimka`); `appka_prekaza`/
//! `okno_hry_v_popredi` sú zámerne vlastné kópie, nie `use` z `zber_canvas`.
//!
//! **Zdroj „beh mapy prebieha"**: appka nemá druhý sledovač `UE_game.log` —
//! tracker (`crate::tracker::Tracker`) túto informáciu už parsuje pre farm
//! tracker (TLI 11c). Fotí sa len keď je aktuálny beh NEHUBOVÝ a má už
//! pridelené `level_uid` (appka vie oboje až po `LevelMgr@` riadku, ktorý
//! prichádza TESNE PO `SceneLevelMgr@ OpenMainWorld END!`).
//!
//! **Sidecar `.meta` súbor vedľa každej snímky.** Fronta Canvasu nepotrebuje
//! metadáta (vždy ide na ten istý endpoint), ale minimapa potrebuje poslať
//! `mapa`/`level_uid`/`rezim`/`okno`/`roi` PRESNE také, aké platili v čase
//! ODFOTENIA — nie aktuálne nastavenia appky (hráč mohol medzi odfotením
//! a odoslaním prepnúť režim alebo appku reštartovať). `core::minimapa_fronta`
//! (krok 62/2, už otestovaný a vydaný) nesie v mene súboru len rezim a čas,
//! preto si tento súbor drží zvyšok VEDĽA v `<meno>.jpg.meta` (JSON), nie v
//! mene — rozširovať už otestovaný formát mena by znamenalo meniť kontrakt
//! modulu, ktorý krok 62/2 uzavrel.
//!
//! **ToS** (`minimapa-api-kontrakt.md` § „Richard rozhodol"): len to, čo
//! appka zachytí z OKNA hry (výrez ROI) a z logu (zóna/`level_uid`) — nič sa
//! neodkrýva, žiadny pohyb ani klik za hráča.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tli_overlay_core::minimapa_fronta as fronta;
use tli_overlay_core::nastavenia::Nastavenia;

use crate::denik;
use crate::klient;
use crate::stav::Stav;
use crate::tracker::Tracker;

#[cfg(windows)]
use crate::okno;
#[cfg(windows)]
use crate::snimka::{self, Snimka};
#[cfg(windows)]
use tli_overlay_core::obraz;
#[cfg(windows)]
use tli_overlay_core::suradnice::Pole;

/// Ako často sa appka pozerá na stav trackera/nastavení. Ticker samotný
/// (`fronta::ODSTUP_TIKU_MS`, 2 s) riadi, kedy sa reálne odfotí — tento krok
/// je len jemnosť pollingu, nech appka nezaostane za behom mapy o viac.
const KROK_POLLINGU_MS: u64 = 500;

const ODOSIELAC_NECINNY_S: u64 = 10;

/// Dlhšia strana zmenšenej snímky — minimapa je už len výrez okna, netreba
/// viac než Canvas zber (TLI 60).
#[cfg(windows)]
const DLZKA_ZBERU: u32 = 1280;
#[cfg(windows)]
const KVALITA_ZBERU: u8 = 75;

/// Priečinok fronty vedľa nastavení appky — VLASTNÝ, nezdieľaný s Canvasom
/// (`zber_canvas::cesta_fronty_pre` má iné priečinky `canvas-zber`/`tartarus-zber`).
pub fn cesta_fronty() -> PathBuf {
    let mut c = crate::stav::cesta_suboru();
    c.pop(); // preč "nastavenia.json"
    c.push("minimapa-zber");
    c
}

fn cesta_denneho_stavu() -> PathBuf {
    cesta_fronty().join("stav.json")
}

fn cesta_meta(cesta_snimky: &Path) -> PathBuf {
    PathBuf::from(format!("{}.meta", cesta_snimky.display()))
}

/// Čo appka potrebuje poslať s konkrétnou snímkou — presne tak, ako to
/// platilo v čase ODFOTENIA (pozri hlavičku modulu).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct MetaSnimky {
    mapa: String,
    level_uid: u64,
    rezim: String,
    okno: Option<(u32, u32)>,
    roi: String,
}

fn nacitaj_denny_stav() -> fronta::DennyStav {
    std::fs::read_to_string(cesta_denneho_stavu())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn uloz_denny_stav(s: &fronta::DennyStav) {
    // Best-effort, rovnako ako Canvas: keď sa strop nepodarí zapísať, appka
    // kvôli tomu nesmie prestať fungovať.
    if let Ok(json) = serde_json::to_string(s) {
        let _ = std::fs::write(cesta_denneho_stavu(), json);
    }
}

/// Zoradený zoznam čakajúcich snímok (bez `.meta` sidecarov — tie filtruje
/// `fronta::je_subor_fronty`, ktorá berie len `minimapa-*.jpg`).
fn cakajuce() -> Vec<PathBuf> {
    let Ok(polozky) = std::fs::read_dir(cesta_fronty()) else {
        return Vec::new();
    };
    let mut subory: Vec<PathBuf> = polozky
        .flatten()
        .map(|p| p.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(fronta::je_subor_fronty)
                .unwrap_or(false)
        })
        .collect();
    subory.sort();
    subory
}

/// Koľko snímok minimapy práve čaká vo fronte — pre riadok „Učenie: …"
/// v Nastaveniach (TLI 70, bod 5).
pub fn pocet_cakajucich() -> u32 {
    cakajuce().len() as u32
}

fn nastavenia_appky(app: &AppHandle) -> Nastavenia {
    app.state::<Stav>().nastavenia.lock().unwrap().clone()
}

/// Zóna (kód) + `level_uid` aktuálneho behu mapy z trackera. `None`, kým
/// appka ešte nevidela žiaden beh, kým beh je HUB, alebo kým beh ešte nemá
/// pridelené `level_uid`.
fn aktualny_beh_mapy(app: &AppHandle) -> Option<(String, u64)> {
    let tracker = app.state::<Tracker>();
    let stav = tracker.0.lock().unwrap();
    let beh = stav.aktualny_beh()?;
    if beh.je_hub {
        return None;
    }
    let level_uid = beh.level_uid?;
    Some((fronta::kod_zony(&beh.zona), level_uid))
}

/// Svieti práve nad hrou vlastný panel appky, alebo beží ručná analýza?
/// Vtedy sa nefotí — rovnaký dôvod ako v `zber_canvas::appka_prekaza`
/// (vlastná kópia, hlavička modulu vysvetľuje prečo).
#[cfg(windows)]
fn appka_prekaza(app: &AppHandle) -> bool {
    let stav = app.state::<Stav>();
    let panel_svieti = stav.zobrazene_od.lock().unwrap().is_some();
    let analyza_bezi = *stav.beh.lock().unwrap();
    panel_svieti || analyza_bezi
}

/// Okno hry — a len vtedy, keď je naozaj v popredí (rovnaký dôvod ako
/// `zber_canvas::okno_hry_v_popredi`: toto beží bez vedomia hráča, nesmie
/// sa odfotiť nič, čo nie je hra).
#[cfg(windows)]
fn okno_hry_v_popredi(proces: &str) -> Option<okno::NajdeneOkno> {
    let hra = okno::najdi_hru(proces)?;
    let vpredu = okno::popredie()?;
    if vpredu.hwnd == hra.hwnd {
        Some(hra)
    } else {
        None
    }
}

/// Vyrezanie ROI minimapy zo zachytenej celej snímky okna.
#[cfg(windows)]
fn vystrihni_roi(snimok: &Snimka, roi: fronta::RoiMinimapy) -> Option<Snimka> {
    let pole = Pole { index: 0, x: roi.x, y: roi.y, w: roi.w, h: roi.h };
    let (rgb, sirka, vyska) = obraz::orez_box01(&snimok.rgb, snimok.sirka, snimok.vyska, pole)?;
    Some(Snimka { sirka, vyska, rgb })
}

/// Jeden pokus o zber v behu mapy. Vracia `true`, keď snímka naozaj pribudla
/// do fronty (aj so sidecar `.meta`) — volajúci podľa toho vie, či sa má
/// zarátať do denného stropu.
#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn pokus_o_zber(app: &AppHandle, n: &Nastavenia, mapa: &str, level_uid: u64, cas: u64) -> bool {
    if appka_prekaza(app) {
        return false;
    }
    let Some(hra) = okno_hry_v_popredi(&n.proces_hry) else {
        return false;
    };

    let snimok = match snimka::zachyt(&hra.obdlznik) {
        Ok(s) => s,
        Err(_) => return false, // ticho, rovnako ako zber_canvas
    };
    if snimka::je_cierna(&snimok) {
        return false; // exkluzívny fullscreen
    }

    let roi = n.minimapa_roi.pre_rezim(n.minimapa_rezim);
    let Some(vyrez) = vystrihni_roi(&snimok, roi) else {
        return false;
    };

    let cakajucich = cakajuce().len();
    if !fronta::smie_pribudnut(cakajucich) {
        denik::zapis(
            "minimapa",
            &format!("fronta je plná ({cakajucich} snímok) — nič sa nemaže, len sa nepridáva"),
        );
        return false;
    }

    let Ok(jpeg) = snimka::do_jpegu(&vyrez, DLZKA_ZBERU, KVALITA_ZBERU) else {
        return false;
    };

    let priecinok = cesta_fronty();
    if std::fs::create_dir_all(&priecinok).is_err() {
        return false;
    }

    let mut poradie = 0u64;
    let cesta = loop {
        let c = priecinok.join(fronta::nazov_snimky(n.minimapa_rezim, cas, poradie));
        if !c.exists() {
            break c;
        }
        poradie += 1;
        if poradie >= 10_000 {
            return false;
        }
    };

    // Najprv `.tmp`, potom premenovanie — odosielač tak nikdy nenarazí na
    // polovičný súbor (`je_subor_fronty` `.tmp` neberie).
    let docasna = cesta.with_extension("jpg.tmp");
    if std::fs::write(&docasna, &jpeg).is_err() || std::fs::rename(&docasna, &cesta).is_err() {
        let _ = std::fs::remove_file(&docasna);
        return false;
    }

    let meta = MetaSnimky {
        mapa: mapa.to_string(),
        level_uid,
        rezim: n.minimapa_rezim.ako_parameter().to_string(),
        okno: Some((hra.obdlznik.sirka, hra.obdlznik.vyska)),
        roi: roi.ako_parameter(),
    };
    if let Ok(json) = serde_json::to_string(&meta) {
        // Best-effort rovnako ako denný stav — keď sidecar nejde zapísať,
        // odosielač snímku bez metadát zahodí (pozri `odosielac`), nestratí
        // sa ticho nikde inde.
        let _ = std::fs::write(cesta_meta(&cesta), json);
    }

    denik::zapis(
        "minimapa",
        &format!("minimapa {mapa} odfotená ({} B) — čaká {}", jpeg.len(), cakajucich + 1),
    );
    true
}

/// Mimo Windowsu sa obrazovka ani okno hry nedajú čítať — appka tam nebeží,
/// toto je len preto, aby `cargo check` prešiel nad celým súborom.
#[cfg(not(windows))]
fn pokus_o_zber(_app: &AppHandle, _n: &Nastavenia, _mapa: &str, _level_uid: u64, _cas: u64) -> bool {
    false
}

/// Vlákno sledovača: ticker á 2 s počas behu mapy → prípadná snímka do
/// fronty. Na rozdiel od Canvasu (klik myšou) tu vstupom nie je udalosť, ale
/// stav trackera — preto vlastný polling krok, nie čakanie na hranu signálu.
fn sledovac(app: AppHandle, dost_dat: Arc<Mutex<fronta::DostDatStav>>) {
    let mut posledne_foto: Option<Instant> = None;
    let mut posledny_level_uid: Option<u64> = None;
    let mut denny = nacitaj_denny_stav();

    loop {
        std::thread::sleep(Duration::from_millis(KROK_POLLINGU_MS));

        let n = nastavenia_appky(&app);
        if !n.minimapa_zber_zapnute {
            posledne_foto = None;
            continue;
        }

        let Some((mapa, level_uid)) = aktualny_beh_mapy(&app) else {
            posledne_foto = None;
            continue;
        };

        if posledny_level_uid != Some(level_uid) {
            // Nový beh (iný vstup do mapy) — odmietnutia z predošlého behu
            // už neplatia (`DostDatStav::novy_beh`) a prvý tik v novom behu
            // sa odfotí hneď, nie až po `ODSTUP_TIKU_MS`.
            posledny_level_uid = Some(level_uid);
            dost_dat.lock().unwrap().novy_beh();
            posledne_foto = None;
        }

        let od_poslednej_ms = posledne_foto.map(|t| t.elapsed().as_millis() as u64).unwrap_or(u64::MAX);
        if !fronta::ma_sa_fotit(true, od_poslednej_ms) {
            continue;
        }

        let cas = crate::tracker::unixovy_cas();
        if !denny.smie_zbierat(cas) {
            // Strop je denný — ďalšia zmysluplná kontrola je až zajtra, ale
            // appka sem príde znova pri ďalšom tiku; `smie_zbierat` si
            // prechod cez polnoc ustráži sám.
            continue;
        }

        // Stav zo servera pre TÚTO mapu — appka sa pýta len raz za
        // `PLATNOST_STAVU_MS`, nie pri každom tiku.
        let teraz_ms = cas.saturating_mul(1000);
        let ma_sa_opytat = dost_dat.lock().unwrap().ma_sa_opytat_servera(&mapa, teraz_ms);
        if ma_sa_opytat {
            match klient::ziskaj_stav_minimapy(&n) {
                Ok(stav) => {
                    let dost = stav.mapy.get(&mapa).map(|m| m.dost_dat).unwrap_or(false);
                    dost_dat.lock().unwrap().uloz_stav_servera(&mapa, dost, teraz_ms);
                }
                Err(e) => {
                    denik::zapis("minimapa", &format!("GET /minimapa/stav zlyhalo ({e}), skúsim nabudúce"));
                }
            }
        }

        if !dost_dat.lock().unwrap().ma_sa_fotit_mapu(&mapa) {
            continue;
        }

        posledne_foto = Some(Instant::now());

        if pokus_o_zber(&app, &n, &mapa, level_uid, cas) {
            denny.zapocitaj(cas);
            uloz_denny_stav(&denny);
            if denny.zostava() == 0 {
                denik::zapis("minimapa", "denný strop snímok minimapy vyčerpaný, dnes už nič");
            }
        }
    }
}

/// Vlákno odosielača: fronta z disku na server, jeden súbor po druhom —
/// spolu s ním aj jeho sidecar `.meta` (mapa/level_uid/rezim/okno/roi).
fn odosielac(app: AppHandle, dost_dat: Arc<Mutex<fronta::DostDatStav>>) {
    let mut zlyhani: u32 = 0;
    loop {
        let n = nastavenia_appky(&app);
        // Vypnutý prepínač zastaví aj odosielanie, rovnako ako pri Canvase —
        // fronta na disku ostane nedotknutá a odíde, keď si to hráč zapne späť.
        let pripravene = if n.minimapa_zber_zapnute && n.chyba_konfiguracie().is_none() {
            cakajuce()
        } else {
            Vec::new()
        };

        let Some(cesta) = pripravene.first().cloned() else {
            std::thread::sleep(Duration::from_secs(ODOSIELAC_NECINNY_S));
            continue;
        };

        let meta_cesta = cesta_meta(&cesta);
        let meta: Option<MetaSnimky> =
            std::fs::read_to_string(&meta_cesta).ok().and_then(|s| serde_json::from_str(&s).ok());
        let Some(meta) = meta else {
            // Sidecar chýba alebo je poškodený — appka nevie, ktorej mape a
            // ktorému behu snímka patrí, takže ju nemá ako správne poslať.
            // Dvojica sa zahodí (nie je čo archivovať — nikdy sa to nedostalo
            // na server), nie ticho preskočí, inak by zablokovala zvyšok
            // fronty navždy (vždy sa berie prvý v poradí).
            denik::zapis("minimapa", &format!("chýba/poškodený meta súbor pre {}, zahadzujem dvojicu", cesta.display()));
            let _ = std::fs::remove_file(&cesta);
            let _ = std::fs::remove_file(&meta_cesta);
            continue;
        };

        let Ok(jpeg) = std::fs::read(&cesta) else {
            // Súbor nám spod rúk zmizol — nič sa nedeje, ide sa ďalej.
            std::thread::sleep(Duration::from_millis(200));
            continue;
        };

        let cas = crate::tracker::unixovy_cas();
        match klient::posli_minimapa_zber(&n, jpeg, &meta.rezim, &meta.mapa, meta.level_uid, cas, meta.okno, &meta.roi) {
            Ok(odpoved) => {
                zlyhani = 0;
                // Server telo spracoval (prijal ALEBO vedome odmietol — obe
                // sú odpoveď 200, pozri kontrakt) — kópia vo fronte na disku
                // hráča už nemá čo robiť.
                let _ = std::fs::remove_file(&cesta);
                let _ = std::fs::remove_file(&meta_cesta);
                if odpoved.dost_dat {
                    // Prestaň fotiť TÚTO mapu hneď, nečakaj na ďalší GET /stav.
                    dost_dat.lock().unwrap().zapamataj_odmietnutie(&meta.mapa);
                }
                {
                    // TLI 70, bod 5 — nech hráč v Nastaveniach vidí, že
                    // sa fronta naozaj posiela.
                    let stav = app.state::<Stav>();
                    stav.stav_zberu.lock().unwrap().zapis_uspech_minimapy();
                }
                denik::zapis(
                    "minimapa",
                    &format!("odoslané ({}), vo fronte ostáva {}", meta.mapa, pripravene.len() - 1),
                );
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(e) => {
                zlyhani = zlyhani.saturating_add(1);
                let odstup = fronta::odstup_po_zlyhani(zlyhani);
                {
                    let stav = app.state::<Stav>();
                    stav.stav_zberu.lock().unwrap().zapis_chybu_minimapy(&e);
                }
                denik::zapis(
                    "minimapa",
                    &format!("odoslanie zlyhalo ({e}) — {} snímok čaká, skúsim o {odstup} s", pripravene.len()),
                );
                std::thread::sleep(Duration::from_secs(odstup));
            }
        }
    }
}

/// Spustí obe vlákna. Volá sa raz pri štarte appky; prepínač v Nastaveniach
/// sa číta pri každom tiku a pri každom kole odosielača, takže zapnutie či
/// vypnutie zberu nepotrebuje reštart appky.
pub fn spusti(app: AppHandle) {
    let dost_dat = Arc::new(Mutex::new(fronta::DostDatStav::default()));
    let a = app.clone();
    let d1 = dost_dat.clone();
    std::thread::spawn(move || sledovac(a, d1));
    std::thread::spawn(move || odosielac(app, dost_dat));
    denik::zapis(
        "minimapa",
        &format!(
            "zber minimapy beží (fronta: {}, čaká {} snímok)",
            cesta_fronty().display(),
            cakajuce().len()
        ),
    );
}
