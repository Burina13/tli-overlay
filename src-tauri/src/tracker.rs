//! Farm tracker — čítanie herného logu a vydávanie hotových čísel do UI.
//!
//! Všetka logika (parsovanie riadkov, delty inventára, hranice behov, ceny,
//! agregácie, VDF/rotácia/kódovanie bajtov) je v `tli_overlay_core::tracker` a
//! testuje sa na Linuxe. Tu ostáva len to, čo sa bez skutočného počítača
//! nedá: nájsť súbor (register, disk), otvoriť ho, čítať od poslednej
//! pozície a bežať na pozadí nezávisle od toho, či je niektoré okno appky
//! otvorené (TLI 11c — predtým sa čítalo len vtedy, keď UI samo zavolalo
//! `tracker_prehlad`, čo pri appke schovanej v tray nefungovalo spoľahlivo).
//!
//! **Appka do hry nijako nezasahuje — iba číta textový súbor, ktorý si hra sama
//! píše na disk** (a len keď si hráč v hre zapne „Enable Log"). Žiadne čítanie
//! pamäte procesu, žiadna injekcia, žiadne pakety — to je pravidlo projektu, nie
//! odporúčanie.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tli_overlay_core::tracker::agregacie::{ocen_beh, prehlad, Prehlad, VysledokBehu};
use tli_overlay_core::tracker::ceny::{vek_text, Cena, Cennik, Nastavenie, Zdroj};
use tli_overlay_core::tracker::chvost::StavChvosta;
use tli_overlay_core::tracker::pripomienka::{klasifikuj, SledovacPripomienky};
use tli_overlay_core::tracker::stav::{Beh, Sledovac};
use tli_overlay_core::tracker::steam;

use crate::stav::Stav;

/// Koľko najviac bajtov prečítame naraz. Pri prvom spustení je log kľudne
/// stomegový a natiahnuť ho celý do pamäte by appku na pár sekúnd zmrazilo.
const DAVKA_B: u64 = 2 * 1024 * 1024;

/// Bežné miesta, kde hra log píše, keď ju Steam knižnice (registry +
/// `libraryfolders.vdf`) nenašli — non-Steam launcher (XD/Global) alebo
/// nezvyčajná inštalácia. Zoznam je zámerne krátky a doplniteľný ručne
/// v nastaveniach.
///
/// Posledná položka (non-Steam XD launcher) je **neoverené tvrdenie so
/// zdrojom**, nie zmeraná skutočnosť: priamy prístup k takej inštalácii sme
/// 15.10.2026 nemali. Zdroj: advanceduninstaller.com (číta skutočnú
/// `InstallLocation` z registra odinštalovaných strojov) — „Torchlight:
/// Infinite is normally installed in the `C:\Program Files (x86)\Torchlight
/// Infinite` folder". Priečinok s logom (`UE_Game\Torchlight\Saved\Logs`) je
/// odvodený z toho istého tvaru, aký má Steam inštalácia — keby sa to
/// ukázalo inak, oprav podľa reálneho hlásenia hráča.
const ZNAME_CESTY: &[&str] = &[
    r"C:\Program Files (x86)\Steam\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
    r"C:\Program Files\Steam\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
    r"D:\SteamLibrary\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
    r"E:\SteamLibrary\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
    r"C:\Program Files (x86)\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log",
];

/// Stav trackera medzi dvoma čítaniami. Celé to drží jeden `Mutex` — čítanie
/// súboru aj prepočet sú rádovo milisekundy a dve vlákna sem nechodia súčasne
/// (vlákno na pozadí aj príkazy z UI si ho požičiavajú krátko a hneď vracajú).
pub struct StavTrackera {
    sledovac: Sledovac,
    cennik: Cennik,
    behy: Vec<Beh>,
    chvost: StavChvosta,
    cesta: Option<PathBuf>,
    pub chyba: Option<String>,
    /// Pripomienka „Enable Log" (TLI 11d) — žije tu, nie vedľa, lebo rotáciu
    /// aj riadky vidí práve táto funkcia (`precitaj`) ako prvá.
    pub pripomienka: SledovacPripomienky,
}

impl Default for StavTrackera {
    fn default() -> Self {
        Self {
            sledovac: Sledovac::novy(),
            cennik: Cennik::novy(),
            behy: Vec::new(),
            chvost: StavChvosta::novy(),
            cesta: None,
            chyba: None,
            pripomienka: SledovacPripomienky::novy(false),
        }
    }
}

/// Nájde log: najprv ručne zadaná cesta, potom Steam (register +
/// `libraryfolders.vdf`), nakoniec krátky zoznam známych miest.
pub fn najdi_log(rucna: &str) -> Option<PathBuf> {
    let rucna = rucna.trim();
    if !rucna.is_empty() {
        let p = PathBuf::from(rucna);
        return if p.is_file() { Some(p) } else { None };
    }
    if let Some(p) = najdi_cez_steam() {
        return Some(p);
    }
    ZNAME_CESTY.iter().map(PathBuf::from).find(|p| p.is_file())
}

/// Steam vetva automatickej detekcie (TLI 11c, krok 1). Register povie, kde
/// je hlavná inštalácia; `libraryfolders.vdf` v nej povie o ďalších
/// knižniciach (iný disk). Samotné parsovanie/skladanie ciest je v
/// `tli_overlay_core::tracker::steam` a testuje sa na Linuxe — tu je už len
/// register a jedno čítanie súboru.
fn najdi_cez_steam() -> Option<PathBuf> {
    let hlavna = steam_cesta_z_registra();
    let vdf_obsah = hlavna
        .as_ref()
        .and_then(|s| std::fs::read_to_string(s.join("steamapps").join("libraryfolders.vdf")).ok());
    let dalsie = vdf_obsah
        .as_deref()
        .map(steam::kniznice_z_vdf)
        .unwrap_or_default();
    let kniznice = steam::zluc_kniznice(hlavna, &dalsie);
    let kandidati = steam::kandidati_logu(&kniznice);
    steam::najdi_existujucu(&kandidati, |p| p.is_file())
}

#[cfg(windows)]
fn steam_cesta_z_registra() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    if let Ok(kluc) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam") {
        if let Ok(cesta) = kluc.get_value::<String, _>("SteamPath") {
            // Steam v registri drží cestu s `/`, Windows API aj appka ďalej
            // potrebujú `\`.
            return Some(PathBuf::from(cesta.replace('/', "\\")));
        }
    }
    // 32-bitový Steam na 64-bitovom Windowse píše do WOW6432Node; skúsime aj
    // bez neho pre prípad inej konfigurácie.
    for podcesta in [r"SOFTWARE\WOW6432Node\Valve\Steam", r"SOFTWARE\Valve\Steam"] {
        if let Ok(kluc) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(podcesta) {
            if let Ok(cesta) = kluc.get_value::<String, _>("InstallPath") {
                return Some(PathBuf::from(cesta.replace('/', "\\")));
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn steam_cesta_z_registra() -> Option<PathBuf> {
    None
}

/// Prečíta nové bajty a pretlačí ich cez `Sledovac`.
///
/// Rotáciu logu (hra pri štarte súbor skráti alebo premenuje) spoznáme podľa
/// toho, že súbor je menší než naša pozícia — vtedy začíname odznova (bez
/// toho, aby sa čokoľvek zdvojilo alebo stratilo, lebo nový súbor je od
/// nuly celý „nový"). Pri PRVOM čítaní cesty v tomto behu appky sa pozícia
/// načíta z disku (`stav::nacitaj_chvost`) — hráč appku často spustí až po
/// pár mapách a história z dnešného logu je presne to, čo chce vidieť, ale
/// netreba kvôli tomu zakaždým reparsovať aj to, čo appka už raz videla.
pub fn precitaj(stav: &mut StavTrackera, cesta: &Path) -> Result<usize, String> {
    let meta = std::fs::metadata(cesta).map_err(|e| format!("log sa nedá otvoriť: {e}"))?;
    let velkost = meta.len();

    if stav.cesta.as_deref() != Some(cesta) {
        // Cesta sa zmenila (alebo appka práve prvý raz číta) — `StavTrackera`
        // sa stavia nanovo, ale nastavenie „Pripomínať Enable Log" (ktoré sem
        // volajúci zapísal PRED týmto volaním, viď `spusti_vlakno`) sa pritom
        // nesmie stratiť.
        let pripominat = stav.pripomienka.zapnute();
        let chvost = crate::stav::nacitaj_chvost(cesta);
        *stav = StavTrackera { cesta: Some(cesta.to_path_buf()), chvost, ..Default::default() };
        stav.pripomienka.nastav_zapnute(pripominat);
    }
    let teraz = unixovy_cas();
    if stav.chvost.over_rotaciu(velkost) {
        // Hra pri reštarte premenovala/skrátila log — nová session, aj pre
        // pripomienku „Enable Log" (TLI 11d).
        stav.pripomienka.nova_session(teraz);
    }
    if velkost == stav.chvost.pozicia {
        return Ok(0);
    }

    let mut subor = std::fs::File::open(cesta).map_err(|e| format!("log sa nedá otvoriť: {e}"))?;
    subor
        .seek(SeekFrom::Start(stav.chvost.pozicia))
        .map_err(|e| format!("v logu sa nedá posunúť: {e}"))?;

    let kolko = (velkost - stav.chvost.pozicia).min(DAVKA_B) as usize;
    let mut bajty = vec![0u8; kolko];
    let precitane = subor
        .read(&mut bajty)
        .map_err(|e| format!("log sa nedá prečítať: {e}"))?;
    bajty.truncate(precitane);

    let riadky = stav.chvost.priprav_riadky(&bajty);
    crate::stav::uloz_chvost(cesta, &stav.chvost);

    let mut novych = 0;
    for riadok in &riadky {
        stav.pripomienka.spracuj_riadok(teraz, klasifikuj(riadok));
        if let Some(beh) = stav.sledovac.riadok(riadok) {
            stav.behy.push(beh);
            novych += 1;
        }
    }

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
    /// Vek každej known ceny v ľudskej podobe (`"400021" -> "pred 4 min"`).
    pub vek_cien: BTreeMap<String, String>,
    pub pocet_cien: usize,
    /// Pripomienka „Enable Log" (TLI 11d): keď je `true`, záložka Tracker má
    /// ukázať červený stav — logovanie v tejto session evidentne nebeží.
    pub pripomienka_aktivna: bool,
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
        pripomienka_aktivna: stav.pripomienka.aktivne(),
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

// ─────────────────────────────────────────────────────────────────────────────
// Stav cesty k logu pre Nastavenia (TLI 11c, krok 2)
// ─────────────────────────────────────────────────────────────────────────────

/// Koľko bajtov od začiatku logu stačí prehľadať na „Enable Log" značky.
/// Log vie mať desiatky MB — netreba čítať celý, značky (BagMgr, Spv3Open)
/// sa objavia hneď v prvých riadkoch session, ak je logovanie zapnuté.
const KONTROLA_ZNACIEK_B: usize = 4 * 1024 * 1024;

/// Čo sa ukáže vedľa poľa „Cesta k logu" — ikonka a vysvetlenie.
#[derive(Debug, Clone, Serialize)]
pub struct StavCesty {
    pub cesta: Option<String>,
    pub existuje: bool,
    pub velkost_b: Option<u64>,
    pub upravene_pred_s: Option<u64>,
    /// Súbor je síce nájdený, ale je prázdny alebo skoro prázdny — hráč si
    /// log zapol len teraz a hra doň ešte nestihla nič napísať.
    pub takmer_prazdny: bool,
    /// Súbor má rozumnú veľkosť, ale nenašli sa v ňom očakávané značky —
    /// vyzerá to, že "Enable Log" v hre je vypnuté a toto je log z iného
    /// behu/inej appky.
    pub chyba_vypnuty_log: bool,
}

/// `rucna` je to, čo je PRÁVE napísané v poli nastavení (môže byť prázdne —
/// vtedy sa berie auto-detekcia, rovnako ako pri čítaní).
pub fn stav_cesty(rucna: &str) -> StavCesty {
    let Some(cesta) = najdi_log(rucna) else {
        return StavCesty {
            cesta: None,
            existuje: false,
            velkost_b: None,
            upravene_pred_s: None,
            takmer_prazdny: false,
            chyba_vypnuty_log: false,
        };
    };

    let meta = std::fs::metadata(&cesta).ok();
    let velkost = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let upravene_pred_s = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|cas| cas.elapsed().ok())
        .map(|d| d.as_secs());

    let takmer_prazdny = velkost < 1024;
    let chyba_vypnuty_log = !takmer_prazdny && !obsahuje_ocakavane_znacky(&cesta);

    StavCesty {
        cesta: Some(cesta.display().to_string()),
        existuje: true,
        velkost_b: Some(velkost),
        upravene_pred_s,
        takmer_prazdny,
        chyba_vypnuty_log,
    }
}

fn obsahuje_ocakavane_znacky(cesta: &Path) -> bool {
    let Ok(mut subor) = std::fs::File::open(cesta) else {
        return false;
    };
    let mut bajty = vec![0u8; KONTROLA_ZNACIEK_B];
    let Ok(precitane) = subor.read(&mut bajty) else {
        return false;
    };
    bajty.truncate(precitane);
    let text = tli_overlay_core::tracker::log::dekoduj(&bajty);
    text.contains("BagMgr") || text.contains("Spv3Open")
}

// ─────────────────────────────────────────────────────────────────────────────
// Živé čítanie na pozadí (TLI 11c, krok 3)
// ─────────────────────────────────────────────────────────────────────────────

/// Udalosť, ktorú vlákno pošle UI po úspešnom prečítaní novej dávky — okná
/// na ňu reagujú okamžitým prekreslením namiesto čakania na vlastný interval
/// (ten ostáva ako záložka, keby appka udalosť prepočula).
pub const UDALOST_ZMENA: &str = "tracker-zmena";

/// Udalosť pripomienky „Enable Log" (TLI 11d) — počúva ju toast nad hrou aj
/// hlavné okno (červený/zelený stav v záložke Tracker). Payload: `{ "typ":
/// "upozornenie" | "ok", "text": "…" }`.
pub const UDALOST_PRIPOMIENKA: &str = "tracker-pripomienka";

/// Text toastu/notifikácie presne podľa zadania (TLI 11d, bod 3).
const TEXT_UPOZORNENIA: &str =
    "⚠️ Zapni v hre Settings → Other Settings → Enable Log — inak sa táto session netrackuje";
const TEXT_OK: &str = "✅ Logovanie beží";

#[derive(Clone, Serialize)]
struct PripomienkaPayload {
    typ: &'static str,
    text: &'static str,
}

/// Pretlačí udalosti z `SledovacPripomienky` do appky: Tauri udalosť (ju
/// počúva toast aj Tracker tab) a pri upozornení aj Windows notifikáciu.
fn oznam_pripomienku(app: &AppHandle, udalost: tli_overlay_core::tracker::pripomienka::UdalostPripomienky) {
    use tli_overlay_core::tracker::pripomienka::UdalostPripomienky as U;
    let payload = match udalost {
        U::ZobrazUpozornenie | U::ZopakujUpozornenie => {
            posli_windows_notifikaciu(app);
            PripomienkaPayload { typ: "upozornenie", text: TEXT_UPOZORNENIA }
        }
        U::LogovanieBeziOk => PripomienkaPayload { typ: "ok", text: TEXT_OK },
    };
    let _ = app.emit(UDALOST_PRIPOMIENKA, payload);
}

#[cfg(windows)]
fn posli_windows_notifikaciu(app: &AppHandle) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("Tvoj Tatko TLI Overlay")
        .body(TEXT_UPOZORNENIA)
        .show();
}

#[cfg(not(windows))]
fn posli_windows_notifikaciu(_app: &AppHandle) {}

/// Spustí vlákno na pozadí, ktoré sleduje log nezávisle od toho, či je
/// niektoré okno otvorené — appka má fungovať aj schovaná v tray.
///
/// Mechanizmus je DVOJVRSTVOVÝ presne podľa zadania: `notify` hlási zmeny
/// priečinka s logom takmer okamžite (reakcia pod sekundu), `recv_timeout`
/// zároveň zaisťuje, že sa skúsi čítať aj bez jedinej udalosti aspoň raz za
/// sekundu — keby `notify` na danom systéme/disku udalosť nezachytilo
/// (napr. sieťová jednotka), appka to aj tak dobehne pollingom.
///
/// Sleduje sa PRIEČINOK, nie priamo súbor: rotácia log súbor premenuje/znova
/// vytvorí, a watch na už neexistujúci inode by mlčal ďalej.
// `watcher` sa priraďuje, ale nič z neho sa už nečíta — jeho jediná úloha je
// ZOSTAŤ ŽIVÝ (vlastniť OS handle na sledovanie), čo `cargo check` nevie
// odlíšiť od mŕtveho priradenia.
#[allow(unused_assignments, unused_variables)]
pub fn spusti_vlakno(app: AppHandle) {
    std::thread::spawn(move || {
        use notify::{RecommendedWatcher, RecursiveMode, Watcher};
        use std::sync::mpsc::{channel, RecvTimeoutError};

        let (tx, rx) = channel::<notify::Result<notify::Event>>();
        let mut watcher: Option<RecommendedWatcher> = None;
        let mut sledovany_priecinok: Option<PathBuf> = None;

        loop {
            // Daň (`tracker_dan`) sa uplatňuje až pri zostavovaní prehľadu pre
            // UI (`prehlad_pre_ui`), tu sa zámerne nepoužíva.
            let (zapnuty, rucna, pripominat) = {
                let stav = app.state::<Stav>();
                let n = stav.nastavenia.lock().unwrap();
                (n.tracker_zapnuty, n.cesta_logu.clone(), n.pripominat_enable_log)
            };

            let cesta = if zapnuty { najdi_log(&rucna) } else { None };

            // (Pre)nastav watcher na priečinok aktuálnej cesty, keď sa zmenila.
            let priecinok = cesta.as_deref().and_then(Path::parent).map(Path::to_path_buf);
            if priecinok != sledovany_priecinok {
                watcher = priecinok.as_ref().and_then(|p| {
                    let tx = tx.clone();
                    match notify::recommended_watcher(move |udalost| {
                        let _ = tx.send(udalost);
                    }) {
                        Ok(mut w) => match w.watch(p, RecursiveMode::NonRecursive) {
                            Ok(()) => Some(w),
                            Err(_) => None,
                        },
                        Err(_) => None,
                    }
                });
                sledovany_priecinok = priecinok;
            }

            // Čaká na udalosť NAJVIAC 1 s — buď príde skôr (notify) a číta sa
            // hneď, alebo vyprší časový limit (záložný polling).
            match rx.recv_timeout(Duration::from_millis(1000)) {
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => watcher = None,
            }
            // Vyprázdni prípadné ďalšie nahromadené udalosti — stačí jedno
            // čítanie na koniec frontu, netreba spracovať každú zvlášť.
            while rx.try_recv().is_ok() {}

            if !zapnuty {
                continue;
            }
            let Some(cesta) = cesta else { continue };

            let tracker = app.state::<Tracker>();
            let (novych, pripomienka_udalosti) = {
                let mut t = tracker.0.lock().unwrap();
                t.pripomienka.nastav_zapnute(pripominat);
                let vysledok = precitaj(&mut t, &cesta);
                let novych = match &vysledok {
                    Ok(n) => *n,
                    Err(e) => {
                        t.chyba = Some(e.clone());
                        0
                    }
                };
                if vysledok.is_ok() {
                    t.chyba = None;
                }
                // Tik beží KAŽDÚ iteráciu (á najviac 1 s), nezávisle od toho,
                // či `precitaj` našlo nové bajty — lehota sa počíta aj vtedy,
                // keď sa v logu nedeje ÚPLNE NIČ.
                t.pripomienka.tik(unixovy_cas());
                (novych, t.pripomienka.odober_udalosti())
            };
            if novych > 0 {
                let _ = app.emit(UDALOST_ZMENA, novych);
            }
            for udalost in pripomienka_udalosti {
                oznam_pripomienku(&app, udalost);
            }
        }
    });
}

#[cfg(test)]
mod testy {
    use super::*;
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU32, Ordering};

    static POCITADLO: AtomicU32 = AtomicU32::new(0);

    /// Vlastný dočasný súbor na test, nezávislý od toho, čo robia ostatné
    /// testy bežiace paralelne v tom istom procese.
    fn docasny_subor() -> PathBuf {
        let n = POCITADLO.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("tli-overlay-test-{}-{}-{}.log", std::process::id(), n, unixovy_cas()));
        p
    }

    fn napis(cesta: &Path, obsah: &str) {
        std::fs::write(cesta, obsah).unwrap();
    }

    fn pripis(cesta: &Path, obsah: &str) {
        let mut f = std::fs::OpenOptions::new().append(true).open(cesta).unwrap();
        f.write_all(obsah.as_bytes()).unwrap();
    }

    #[test]
    fn precitaj_cita_prirastkovo_bez_zmeny_nic_nenacita_a_rotacia_pokracuje() {
        let cesta = docasny_subor();
        napis(&cesta, "prvy\ndruhy\n");

        let mut stav = StavTrackera::default();
        let n1 = precitaj(&mut stav, &cesta).unwrap();
        // Ani jeden riadok nie je rozpoznaná udalosť (dokončená mapa) — oba sú
        // appke neznáme, čo je presne to, čo má táto cesta overiť.
        assert_eq!(n1, 0);
        assert_eq!(prehlad_pre_ui(&stav, true, false).neznamych_riadkov, 3);

        pripis(&cesta, "treti\n");
        precitaj(&mut stav, &cesta).unwrap();
        assert_eq!(
            prehlad_pre_ui(&stav, true, false).neznamych_riadkov,
            5,
            "pridaný riadok sa mal dočítať navyše, nie nahradiť predošlý počet"
        );

        // Bez zmeny súboru druhé čítanie nenájde nič nové.
        let n3 = precitaj(&mut stav, &cesta).unwrap();
        assert_eq!(n3, 0);
        assert_eq!(prehlad_pre_ui(&stav, true, false).neznamych_riadkov, 5);

        // Rotácia: hra súbor nahradí menším — má sa dočítať celý nový súbor
        // od začiatku, nič navyše a nič chýbajúce.
        napis(&cesta, "x\n");
        precitaj(&mut stav, &cesta).unwrap();
        assert_eq!(prehlad_pre_ui(&stav, true, false).neznamych_riadkov, 7);

        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn precitaj_na_neexistujucom_subore_vrati_chybu_nie_paniku() {
        let cesta = docasny_subor(); // nikdy sa nezapísal
        let mut stav = StavTrackera::default();
        assert!(precitaj(&mut stav, &cesta).is_err());
    }

    // ─── Pripomienka „Enable Log" (TLI 11d) — wiring cez precitaj ──────────

    #[test]
    fn precitaj_bez_hernych_riadkov_necha_pripomienku_cakat_na_tik() {
        // Samotné `precitaj` nad logom bez enable-log značiek NIČ nespustí —
        // lehota sa vyhodnocuje v `tik`, ktorý volá vlákno na pozadí, nie
        // `precitaj`. Toto len overuje, že čítanie riadkov, ktoré appka
        // nepozná, pripomienku neaktivuje predčasne.
        let cesta = docasny_subor();
        napis(&cesta, "nič užitočné tu nie je, len výplň.\n");

        let mut stav = StavTrackera::default();
        stav.pripomienka.nastav_zapnute(true);
        precitaj(&mut stav, &cesta).unwrap();

        assert!(!stav.pripomienka.aktivne());
        assert!(stav.pripomienka.odober_udalosti().is_empty());
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn nahly_prichod_bagmgr_riadku_po_upozorneni_vypne_cerveny_stav() {
        // Simuluje presne to, čo zadanie žiada overiť: „nový log bez herných
        // riadkov → upozornenie; po pridaní riadkov → upozornenie zmizne."
        // Lehotu simuluje priamy `tik` s posunutým (nie reálne odčakaným) časom.
        let cesta = docasny_subor();
        napis(&cesta, "nič užitočné tu nie je, len výplň.\n");

        let mut stav = StavTrackera::default();
        stav.pripomienka.nastav_zapnute(true);
        precitaj(&mut stav, &cesta).unwrap();

        let teraz = unixovy_cas();
        stav.pripomienka.tik(teraz + tli_overlay_core::tracker::pripomienka::LEHOTA_S);
        assert!(stav.pripomienka.aktivne(), "po vypršaní lehoty má byť aktívne upozornenie");

        pripis(&cesta, "BagMgr@:InitBagData PageId = 1 SlotId=1 ConfigBaseId=1 Num=1\n");
        precitaj(&mut stav, &cesta).unwrap();

        assert!(!stav.pripomienka.aktivne(), "enable-log značka má upozornenie zrušiť");
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn rotacia_logu_spusti_pre_pripomienku_novu_session() {
        use tli_overlay_core::tracker::pripomienka::LEHOTA_S;

        let cesta = docasny_subor();
        napis(&cesta, "BagMgr@:InitBagData PageId = 1 SlotId=1 ConfigBaseId=1 Num=1\n");

        let mut stav = StavTrackera::default();
        stav.pripomienka.nastav_zapnute(true);
        precitaj(&mut stav, &cesta).unwrap();
        assert!(!stav.pripomienka.aktivne(), "enable-log značka hneď na začiatku je v poriadku");

        // Rotácia: hra log skráti/nahradí menším súborom bez enable-log značky.
        napis(&cesta, "x\n");
        precitaj(&mut stav, &cesta).unwrap();

        let teraz = unixovy_cas();
        stav.pripomienka.tik(teraz + LEHOTA_S);
        assert!(
            stav.pripomienka.aktivne(),
            "nová session (po rotácii) má svoju vlastnú čerstvú lehotu, nemá si pamätať starú značku"
        );
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn vypnuta_pripomienka_cez_precitaj_nic_neprodukuje() {
        let cesta = docasny_subor();
        napis(&cesta, "nič užitočné tu nie je, len výplň.\n");

        let mut stav = StavTrackera::default(); // pripomienka je predvolene vypnutá
        precitaj(&mut stav, &cesta).unwrap();
        stav.pripomienka.tik(unixovy_cas() + tli_overlay_core::tracker::pripomienka::LEHOTA_S);

        assert!(!stav.pripomienka.aktivne());
        assert!(stav.pripomienka.odober_udalosti().is_empty());
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn stav_cesty_neexistujuca_cesta() {
        let s = stav_cesty("/neexistuje/isto/UE_game.log");
        assert!(!s.existuje);
        assert_eq!(s.cesta, None);
        assert!(!s.chyba_vypnuty_log);
    }

    #[test]
    fn stav_cesty_takmer_prazdny_nehlasi_vypnuty_log() {
        let cesta = docasny_subor();
        napis(&cesta, "x");
        let s = stav_cesty(&cesta.display().to_string());
        assert!(s.existuje);
        assert!(s.takmer_prazdny);
        assert!(!s.chyba_vypnuty_log, "prázdny log je iná situácia než vypnutý Enable Log");
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn stav_cesty_rozpozna_vypnuty_log_bez_znaciek() {
        let cesta = docasny_subor();
        napis(&cesta, &"nič užitočné tu nie je, len výplň kvôli veľkosti súboru.\n".repeat(50));
        let s = stav_cesty(&cesta.display().to_string());
        assert!(s.existuje);
        assert!(!s.takmer_prazdny);
        assert!(s.chyba_vypnuty_log);
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn stav_cesty_so_znackou_bagmgr_je_v_poriadku() {
        let cesta = docasny_subor();
        let obsah = format!(
            "{}\nBagMgr@:InitBagData PageId = 1 SlotId=1 ConfigBaseId=1 Num=1\n",
            "výplň kvôli veľkosti súboru ".repeat(100)
        );
        napis(&cesta, &obsah);
        let s = stav_cesty(&cesta.display().to_string());
        assert!(!s.chyba_vypnuty_log);
        let _ = std::fs::remove_file(&cesta);
    }
}
