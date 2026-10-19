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
use tli_overlay_core::tracker::agregacie::{
    ocen_beh, ocen_beh_v_case, prehlad, top_dropy, DropRiadok, Prehlad, VysledokBehu,
};
use tli_overlay_core::tracker::ceny::{vek_text, Cena, Cennik, Nastavenie};
use tli_overlay_core::tracker::chvost::StavChvosta;
use tli_overlay_core::nastavenia::Nastavenia;
use tli_overlay_core::tracker::db::{self, OceneneHodnoty, ZaznamCeny};
use tli_overlay_core::tracker::pripomienka::{klasifikuj, SledovacPripomienky};
use tli_overlay_core::tracker::stav::{Beh, Sledovac, STRANKA_BATOHU_OSTATNE};
use tli_overlay_core::tracker::steam;
use tli_overlay_core::tracker::trh::je_otvorenie_trhu;

use crate::stav::Stav;

/// Spočíta hodnoty na uloženie do DB (TLI 13) — zámerne BEZ dane: `polozky`
/// tabuľka má surové počty za každú položku a daň je len zobrazovacia
/// preferencia (`tracker_dan` v nastaveniach), ktorá sa dá dopočítať kedykoľvek
/// neskôr z uložených surových dát. Ukladať ju rovno by znamenalo, že zmena
/// prepínača „Daň z predaja" v nastaveniach tíško rozíde živý prehľad
/// s tým, čo appka už natrvalo zapísala.
fn ocenenie_pre_db(beh: &Beh, cennik: &Cennik) -> OceneneHodnoty {
    let Some(v) = ocen_beh(beh, cennik, &Nastavenie { dan: false }) else {
        return OceneneHodnoty::default();
    };
    let fe_za_hodinu = if v.trvanie_ms > 0 {
        Some(v.cisty_zisk / (v.trvanie_ms as f64 / 3_600_000.0))
    } else {
        None
    };
    OceneneHodnoty {
        hodnota_dropov_fe: Some(v.hodnota_dropov),
        hodnota_vstupov_fe: Some(v.hodnota_vstupov),
        cisty_zisk_fe: Some(v.cisty_zisk),
        fe_za_hodinu,
    }
}

/// Uloží jeden uzavretý beh (aj monštrá/modifikátory/bossovia/canvas, ktoré
/// nazbieral) do lokálnej DB. Chyba zápisu sa len zaloguje do `StavTrackera`
/// — appka nesmie prestať trackovať naživo len preto, že sa jeden riadok
/// nepodarilo zapísať na disk (DB môže byť napr. práve otvorená iným
/// procesom počas kopírovania).
fn uloz_beh_do_db(stav: &mut StavTrackera, beh: &Beh) {
    let Some(conn) = &mut stav.db else { return };
    let ocenenie = ocenenie_pre_db(beh, &stav.cennik);
    let mut identita = stav.sledovac.hrac_snimka().cloned().unwrap_or_default();
    if identita.player_id.is_none() {
        // TLI 19 bug E: „Enable Log" sa zapína na začiatku KAŽDEJ session a
        // identita (GetPlayerData/`+player+...`) chodí pri PRIHLÁSENÍ — teda
        // typicky PRED tým, než ju hráč stihne zapnúť. Živé čítanie preto v
        // danej session nemusí vidieť identitu VÔBEC, kým spätný import vidí
        // logy od úplného začiatku (odtiaľ „oprava ecec22f sa v živej ceste
        // neprejavuje" z TLI 19 zadania). Záloha: posledná známa identita z
        // tej istej lokálnej DB — jeden inštalovaný klient appky = jeden hráč.
        if let Ok(Some(posledna)) = db::posledna_znama_identita(conn) {
            identita = posledna;
        }
    }
    match db::uloz_beh(conn, beh, &identita, &ocenenie) {
        Ok((beh_id, _)) => {
            if let Some((nazov, cas)) = stav.sledovac.posledna_pact_konfiguracia() {
                let dps = stav.sledovac.posledny_stat_postavy("DPS").map(|(v, _)| v);
                let def = stav.sledovac.posledny_stat_postavy("DEF").map(|(v, _)| v);
                let _ = db::uloz_build_snapshot(conn, beh_id, &identita, Some(&nazov), dps, def, cas.as_ref());
            }
        }
        Err(e) => {
            stav.chyba_db = Some(format!("Zápis do lokálnej databázy zlyhal: {e}"));
        }
    }
}

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
    /// Lokálna databáza (TLI 13) — `None`, keď sa pri štarte nepodarilo
    /// otvoriť/zmigrovať (napr. poškodený súbor); appka vtedy ďalej traja
    /// naživo, len bez trvalého ukladania (`chyba_db` nesie dôvod pre UI).
    db: Option<rusqlite::Connection>,
    pub chyba_db: Option<String>,
    /// Po ktorý `zapisane` sa ceny už odoslali partii (TLI 20). Drží sa len
    /// v pamäti: po reštarte appky sa dávka pošle znova a server ju zahodí
    /// ako duplicitu — je to lacnejšie než ďalší stav na disku.
    pub ceny_poslane_do: i64,
    /// Serverový čas poslednej úspešne stiahnutej dávky cien partie
    /// (`?od=` pri ďalšom sťahovaní).
    pub ceny_stiahnute_do: i64,
    /// Denný upload logov (TLI 14, dotiahnuté úlohou `2026-10-16-1528-mcp.md`
    /// bod 4): kedy appka naposledy SKÚSILA kolo uploadu (úspešne aj
    /// neúspešne) — predtým sa chyba siete/servera v tomto vlákne vždy ticho
    /// zahodila (`let _ = upload_kolo(&app);`) a `tracker_upload_stav` v DB
    /// ostávala navždy prázdna bez AKEJKOĽVEK stopy, že sa o niečo pokúsila.
    pub posledna_kontrola_uploadu: Option<u64>,
    /// Text poslednej chyby uploadu, `None` keď posledný pokus prešiel (alebo
    /// sa ešte nestal) — zobrazuje sa v záložke Tracker vedľa „Nahrať log
    /// teraz", nielen po ručnom kliku na to tlačidlo.
    pub posledna_chyba_uploadu: Option<String>,
    /// TLI 22: `precitaj` v tejto dávke uvidel otvorenie obrazovky trhu
    /// (`AuctionHouseShowItemGoodV2 Run`). Vlákno na pozadí to po
    /// `odober_trh_otvoreny()` premení na jednu analýzu `Modul::Ceny` —
    /// rovnaký vzor ako `pripomienka.odober_udalosti()` vyššie.
    trh_otvoreny: bool,
}

impl Default for StavTrackera {
    fn default() -> Self {
        let (db, chyba_db) = match db::otvor(&crate::stav::cesta_db_suboru()) {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(format!("Lokálna databáza sa nedá otvoriť: {e}"))),
        };
        // OFFLINE CACHE (TLI 20): cenník sa po štarte postaví z lokálnej DB.
        // Bez toho mal hráč po každom reštarte appky prázdny cenník a behy
        // ocenené na 0 FE, kým si znova niečo nevyhľadal na Exchange — a
        // presne to bol problém, kvôli ktorému TLI 20 vzniklo. Chyba čítania
        // tu nie je dôvod appku zhodiť: prázdny cenník sa doplní za behu.
        let cennik = db
            .as_ref()
            .and_then(|c| db::nacitaj_cennik(c).ok())
            .unwrap_or_else(Cennik::novy);
        Self {
            sledovac: Sledovac::novy(),
            cennik,
            behy: Vec::new(),
            chvost: StavChvosta::novy(),
            cesta: None,
            chyba: None,
            pripomienka: SledovacPripomienky::novy(false),
            db,
            chyba_db,
            ceny_poslane_do: 0,
            ceny_stiahnute_do: 0,
            posledna_kontrola_uploadu: None,
            posledna_chyba_uploadu: None,
            trh_otvoreny: false,
        }
    }
}

impl StavTrackera {
    /// Prístup k DB pre príkazy, ktoré nad ňou pracujú zvonku (sync cien).
    pub fn db(&self) -> Option<&rusqlite::Connection> {
        self.db.as_ref()
    }

    /// Aktuálna cesta k sledovanému logu pre upload (TLI 14) — `None`, kým
    /// appka ešte žiadny log nenašla/neprečítala.
    pub fn cesta_logu(&self) -> Option<&Path> {
        self.cesta.as_deref()
    }

    /// Zapíše stiahnutý agregát partie do cenníka aj do lokálnej DB, aby ho
    /// appka mala aj po reštarte bez siete (offline cache).
    pub fn prijmi_ceny_partie(
        &mut self,
        agregat: &tli_overlay_core::tracker::ceny_sync::AgregatPartie,
        teraz: u64,
    ) -> usize {
        let zmenenych = tli_overlay_core::tracker::ceny_sync::zapis_do_cennika(
            agregat,
            &mut self.cennik,
            teraz,
        );
        if let Some(conn) = &self.db {
            for c in &agregat.ceny {
                if !c.fe.is_finite() || c.fe <= 0.0 {
                    continue;
                }
                let _ = db::uloz_cenu(
                    conn,
                    &ZaznamCeny {
                        polozka: c.polozka,
                        cena_fe: c.fe,
                        median_fe: None,
                        pocet_ponuk: Some(c.pocet_ponuk),
                        sezona: None,
                        // TLI 43: server už posiela, ktorý zdroj za túto
                        // položku vyhral (`"partia"` alebo `"titrack"`) —
                        // predtým tu bolo natvrdo "partia", čo by titrack
                        // ceny v lokálnej DB tíško označilo nesprávne.
                        zdroj: c.zdroj.clone(),
                        cas: None,
                        zapisane: c.kedy,
                    },
                );
            }
        }
        zmenenych
    }

    /// „Reset Stats" (TLI 45, dashboard podľa TITracku). VYMAŽE len živé
    /// behy tejto relácie (čo kreslí záložka Tracker/dashboard) — nič iné:
    /// pozícia čítania logu (`sledovac`/`chvost`) aj lokálna DB pre záložku
    /// Analýza ostávajú nedotknuté, takže dlhodobá história sa nestratí a
    /// appka nezačne čítať log odznova. Presne to vyžaduje zadanie („Nič
    /// nemaž") — toto nemaže žiadny súbor, len vyprázdni zoznam v pamäti,
    /// ktorý sa rovnako nanovo napĺňa z bežiaceho logu od každého reštartu.
    pub fn resetuj_zive_staty(&mut self) {
        self.behy.clear();
    }

    /// TLI 22: zistí a VYNULUJE, či posledné `precitaj` uvidelo otvorenie
    /// obrazovky trhu — volajúci (vlákno na pozadí) má na `true` zareagovať
    /// práve raz, nie pri každom ďalšom tiku.
    pub fn odober_trh_otvoreny(&mut self) -> bool {
        std::mem::take(&mut self.trh_otvoreny)
    }

    /// Zapíše cenu prečítanú z obrazovky trhu (TLI 22, `?modul=ceny`) do
    /// cenníka aj do lokálnej DB — rovnaký pár krokov ako pri vlastnom
    /// vyhľadaní na Exchange (`precitaj` vyššie), len s `Zdroj::Obrazovka`.
    pub fn uloz_cenu_z_obrazovky(&mut self, polozka: u32, najnizsia: f64, pocet_ponuk: u32, teraz: u64) -> bool {
        let zmenilo_sa = self.cennik.vloz_v_case(
            Cena::z_obrazovky(polozka, najnizsia, pocet_ponuk, teraz),
            teraz,
        );
        if let Some(conn) = &self.db {
            let sezona = self.sledovac.hrac_snimka().and_then(|h| h.season_id.clone());
            let _ = db::uloz_cenu(
                conn,
                &ZaznamCeny::z_obrazovky(polozka, najnizsia, pocet_ponuk, sezona, teraz as i64),
            );
        }
        zmenilo_sa
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

/// Tlačidlo „Načítať staré logy" (TLI 13 §1). Nájde v priečinku aktuálneho
/// logu VŠETKY súbory `UE_game*.log` (teda aj aktuálny, aj
/// `UE_game-backup-*.log` zálohy) a spracuje ich cez spätný import.
/// Bezpečné spustiť opakovane — `import::importuj_subory` je idempotentný,
/// takže by/ani by nemalo vadiť, že živé čítanie (`precitaj`) do DB medzitým
/// mohlo uložiť časť tej istej histórie.
pub fn nacitaj_stare_logy(
    stav: &mut StavTrackera,
    cesta_aktualneho_logu: &Path,
) -> Result<tli_overlay_core::tracker::import::VysledokImportu, String> {
    let Some(conn) = &mut stav.db else {
        return Err("Lokálna databáza nie je dostupná.".to_string());
    };
    let priecinok = cesta_aktualneho_logu
        .parent()
        .ok_or_else(|| "Log nemá nadradený priečinok.".to_string())?;

    let mut subory: Vec<PathBuf> = std::fs::read_dir(priecinok)
        .map_err(|e| format!("Priečinok s logmi sa nedá prečítať: {e}"))?
        .filter_map(|z| z.ok())
        .map(|z| z.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("UE_game") && n.ends_with(".log"))
                .unwrap_or(false)
        })
        .collect();
    subory.sort();

    let mut obsahy = Vec::new();
    for cesta in &subory {
        match std::fs::read(cesta) {
            Ok(b) => obsahy.push(b),
            Err(e) => return Err(format!("Súbor {} sa nedá prečítať: {e}", cesta.display())),
        }
    }

    tli_overlay_core::tracker::import::importuj_subory(conn, &obsahy)
        .map_err(|e| format!("Import do databázy zlyhal: {e}"))
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
    let mut uzavrete_behy = Vec::new();
    for riadok in &riadky {
        stav.pripomienka.spracuj_riadok(teraz, klasifikuj(riadok));
        if je_otvorenie_trhu(riadok) {
            stav.trh_otvoreny = true;
        }
        if let Some(beh) = stav.sledovac.riadok(riadok) {
            uzavrete_behy.push(beh);
            novych += 1;
        }
    }

    // TLI 20: z každého vyhľadania na Exchange sa drží p10 (referenčná cena),
    // medián aj počet ponúk — a cena ide do DB AJ so sezónou, nech sa dá
    // neskôr oddeliť cenník tejto ligy od cien zo starej.
    let sezona = stav
        .sledovac
        .hrac_snimka()
        .and_then(|h| h.season_id.clone());
    for nalez in stav.sledovac.odober_ceny() {
        stav.cennik.vloz_v_case(
            Cena::vlastna(nalez.polozka, nalez.p10, nalez.median, nalez.pocet_ponuk, teraz),
            teraz,
        );
        if let Some(conn) = &stav.db {
            let _ = db::uloz_cenu(
                conn,
                &ZaznamCeny::z_nalezu(&nalez, sezona.clone(), teraz as i64),
            );
        }
    }
    if let Some(conn) = &stav.db {
        for (nazov, hodnota, cas) in stav.sledovac.odober_zostatky() {
            let _ = db::uloz_zostatok(conn, &nazov, hodnota, cas.as_ref());
        }
        for (_, text, cas) in stav.sledovac.odober_raw() {
            let _ = db::uloz_raw(conn, None, &text, cas.as_ref());
        }
    }

    // Beh sa do DB ukladá AŽ TERAZ (nie priamo v slučke vyššie) — ceny
    // nájdené v TEJTO ISTEJ dávke (vyššie) sú tak už v cenníku a ocenenie
    // uloženého behu z nich profituje, nie len z cien zo starších dávok.
    for beh in &uzavrete_behy {
        uloz_beh_do_db(stav, beh);
    }
    stav.behy.extend(uzavrete_behy);

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
    /// „Čo padlo" pre mini-panel (TLI 25) — najhodnotnejšie dropy tejto mapy.
    pub top_dropy: Vec<DropRiadok>,
}

/// Koľko riadkov „čo padlo" sa posiela do mini-panela. Päť je horná hranica
/// toho, čo sa dá za behu mapy prebehnúť očami; zvyšok je v záložke Tracker.
const TOP_DROPOV: usize = 5;

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
    /// Výzva na vyhľadanie cien na Exchange (TLI 20) — `None`, keď netreba nič.
    pub vyzva_ceny: Option<VyzvaCeny>,
    /// Vek poslednej kontroly denného uploadu logov v ľudskej podobe
    /// (TLI 14/21), `None` kým appka ešte ani raz neskúsila. Na rozdiel od
    /// `stav-upload` po ručnom kliku „Nahrať log teraz" toto ukazuje stav AJ
    /// z tichého behu na pozadí (`2026-10-16-1528-mcp.md` bod 4).
    pub posledna_kontrola_uploadu_pred: Option<String>,
    /// To isté ako vyššie, ale v surových sekundách (TLI 47, zadanie
    /// `2026-10-19-0744-mcp.md` bod 1a) — UI z toho vie odvodiť prah
    /// „> 15 min pri bežiacej hre", čo sa z ľudského textu „pred X" parsovať
    /// nedá spoľahlivo (jednotky sa menia: min/h/deň).
    pub posledny_upload_pred_s: Option<u64>,
    /// Text poslednej chyby uploadu, `None` keď posledný pokus prešiel.
    pub posledna_chyba_uploadu: Option<String>,
    /// Koľko cien appka prečítala z obrazovky trhu (TLI 22) za posledných
    /// 24 h — počítadlo „dnešných cien" zo zadania bod 3.
    pub pocet_cien_z_obrazovky_24h: i64,
    /// Posledná DOKONČENÁ mapa (TLI 25) — mini-panel ukazuje „čo padlo"
    /// aj medzi mapami, keď `zivo` je `None`. Hubové behy sa preskakujú.
    pub posledna_mapa: Option<Zivo>,
    /// CURRENT INVENTORY (TLI 47) — aktuálny obsah batohu („ostatné", nie
    /// výstroj/zručnosti/meny a NIE sklad), s cenou a hodnotou za kus. Skryté
    /// položky (`Nastavenia::batoh_skryte_polozky`) tu NIE SÚ — appka ich
    /// nezahadzuje nikde inde, len z tohto zoznamu na predaj.
    pub batoh: Vec<PolozkaBatohu>,
    /// Súčet hodnoty `batoh` (bez skrytých) + čistý zisk doteraz odohraných
    /// máp — zadanie TLI 47 bod 2: „net worth = batoh × ceny + aktuálny beh".
    pub net_worth_fe: f64,
}

/// Jedna položka aktuálneho batohu pre záložku CURRENT INVENTORY (TLI 47).
#[derive(Debug, Clone, Serialize)]
pub struct PolozkaBatohu {
    pub config_base_id: u32,
    pub mnozstvo: i64,
    /// `None`, keď appka cenu tejto položky nepozná vôbec — UI to má ukázať
    /// ako „bez ceny", nie ako 0 (nula a „neviem" sú dve rôzne veci, rovnaký
    /// princíp ako pri mini-paneli TLI 25).
    pub cena_za_kus_fe: Option<f64>,
    pub hodnota_fe: Option<f64>,
    /// Cena existuje, ale je staršia než 24 h (TLI 20) — súčet s ňou počíta,
    /// ale UI má na to upozorniť.
    pub cena_zastarana: bool,
    pub skryte: bool,
}

/// Postaví zoznam `batoh` z aktuálneho stavu `Sledovac` + cenník. Skryté
/// položky (`skryte_polozky`) sa v zozname UKÁŽU (appka nič nezahadzuje ticho),
/// len appka ich označí príznakom — UI rozhoduje, či ich pri predajnom
/// zozname vynechá, ale napr. do `net_worth_fe` sa nerátajú (sú to veci, čo
/// si hráč necháva, nie majetok na predaj).
fn aktualny_batoh(stav: &StavTrackera, nastavenie: &Nastavenie, skryte_polozky: &[u32], teraz: u64) -> (Vec<PolozkaBatohu>, f64) {
    let mnozstva = stav.sledovac.batoh_stranka(STRANKA_BATOHU_OSTATNE);
    let mut zoznam = Vec::with_capacity(mnozstva.len());
    let mut hodnota_spolu = 0.0;
    for (polozka, pocet) in mnozstva {
        let skryte = skryte_polozky.contains(&polozka);
        let cena = stav.cennik.cena(polozka);
        let cena_za_kus = cena.map(|c| c.fe);
        let hodnota = cena_za_kus.map(|c| {
            let mut h = c * (pocet as f64);
            if nastavenie.dan {
                h *= 0.875;
            }
            h
        });
        if !skryte {
            hodnota_spolu += hodnota.unwrap_or(0.0);
        }
        zoznam.push(PolozkaBatohu {
            config_base_id: polozka,
            mnozstvo: pocet,
            cena_za_kus_fe: cena_za_kus,
            hodnota_fe: hodnota,
            cena_zastarana: cena.is_some() && stav.cennik.je_zastarana(polozka, teraz),
            skryte,
        });
    }
    (zoznam, hodnota_spolu)
}

/// Koľko položiek najviac vypíše výzva „vyhľadaj na Exchange". Viac než tri
/// si nikto pri hraní nezapamätá a výzva by sa zmenila na zoznam, ktorý sa
/// preskakuje.
const VYZVA_POLOZIEK: usize = 3;

/// Top dropy, ktoré nemajú cenu mladšiu než 24 h (TLI 20, zadanie bod 5).
///
/// Prečo to appka vôbec rieši: jedno vyhľadanie na in-game Exchange zapíše
/// cenu do logu **a cez partiu ju dostanú všetci traja**. Čiže tri kliky v
/// hre môžu doplniť ceny celej partii — ale len keď hráč vie, ktoré tri veci
/// má vyhľadať. Appka do hry nič neposiela a ani nemôže; je to NÁVOD, nie
/// automatika.
#[derive(Debug, Clone, Serialize)]
pub struct VyzvaCeny {
    /// Koľko rôznych položiek v posledných behoch nemá čerstvú cenu.
    pub pocet: usize,
    /// Najcennejšie z nich (podľa počtu kusov), najviac `VYZVA_POLOZIEK`.
    pub polozky: Vec<u32>,
}

/// Zostaví výzvu z už ocenených behov. `bez_ceny` (cenu nepoznáme vôbec) má
/// prednosť pred `zastarane` (cena je, ale je staršia než 24 h) — neznáma
/// cena skresľuje zisk viac než stará.
fn vyzva_z_behov(vysledky: &[VysledokBehu]) -> Option<VyzvaCeny> {
    let mut bez_ceny: BTreeMap<u32, i64> = BTreeMap::new();
    let mut zastarane: BTreeMap<u32, i64> = BTreeMap::new();
    // Len posledných 20 máp — výzva má hovoriť o tom, čo hráč farmí TERAZ.
    for v in vysledky.iter().rev().take(20) {
        for (polozka, pocet) in &v.bez_ceny {
            *bez_ceny.entry(*polozka).or_insert(0) += *pocet;
        }
        for (polozka, pocet) in &v.zastarane {
            *zastarane.entry(*polozka).or_insert(0) += *pocet;
        }
    }
    let pocet = bez_ceny.len() + zastarane.len();
    if pocet == 0 {
        return None;
    }
    let mut poradie: Vec<(u32, i64, u8)> = bez_ceny
        .into_iter()
        .map(|(p, n)| (p, n, 0u8))
        .chain(zastarane.into_iter().map(|(p, n)| (p, n, 1u8)))
        .collect();
    // Najprv úplne neznáme ceny, v rámci toho najviac nazbierané kusy.
    poradie.sort_by(|a, b| a.2.cmp(&b.2).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)));
    Some(VyzvaCeny {
        pocet,
        polozky: poradie.into_iter().take(VYZVA_POLOZIEK).map(|(p, _, _)| p).collect(),
    })
}

/// Zostaví prehľad z aktuálneho stavu. Nič nečíta — to robí `precitaj`.
pub fn prehlad_pre_ui(stav: &StavTrackera, zapnuty: bool, dan: bool, skryte_polozky: &[u32]) -> PrehladProUi {
    let nastavenie = Nastavenie { dan };
    let teraz = unixovy_cas();
    let vysledky: Vec<VysledokBehu> = stav
        .behy
        .iter()
        .filter_map(|b| ocen_beh_v_case(b, &stav.cennik, &nastavenie, Some(teraz)))
        .collect();
    let suhrn = prehlad(&vysledky, &stav.behy);
    let vyzva_ceny = vyzva_z_behov(&vysledky);

    let mut vek_cien = BTreeMap::new();
    for cena in stav.cennik.vsetky() {
        let vek = teraz.saturating_sub(cena.kedy);
        vek_cien.insert(cena.polozka.to_string(), vek_text(vek));
    }

    let na_zivo = |b: &Beh| -> Option<Zivo> {
        if b.je_hub {
            return None;
        }
        let v = ocen_beh(b, &stav.cennik, &nastavenie)?;
        Some(Zivo {
            zona: b.zona.clone(),
            trvanie_ms: v.trvanie_ms,
            cisty_zisk: v.cisty_zisk,
            podpis_vstupov: v.podpis_vstupov,
            top_dropy: top_dropy(b, &stav.cennik, &nastavenie, TOP_DROPOV, teraz),
        })
    };
    let zivo = stav.sledovac.aktualny().and_then(na_zivo);
    // Posledná dokončená mapa — `behy` sú v poradí, v akom ich log priniesol,
    // takže posledný nehubový je ten, z ktorého hráč práve vyšiel.
    let posledna_mapa = stav.behy.iter().rev().find_map(na_zivo);

    let (batoh, hodnota_batohu) = aktualny_batoh(stav, &nastavenie, skryte_polozky, teraz);
    let net_worth_fe = hodnota_batohu + suhrn.celkom.cisty_zisk;

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
        vyzva_ceny,
        posledna_kontrola_uploadu_pred: stav
            .posledna_kontrola_uploadu
            .map(|kedy| vek_text(teraz.saturating_sub(kedy))),
        posledny_upload_pred_s: stav.posledna_kontrola_uploadu.map(|kedy| teraz.saturating_sub(kedy)),
        posledna_chyba_uploadu: stav.posledna_chyba_uploadu.clone(),
        pocet_cien_z_obrazovky_24h: stav
            .db
            .as_ref()
            .and_then(|c| db::pocet_cien_z_obrazovky_od(c, teraz.saturating_sub(24 * 3600) as i64).ok())
            .unwrap_or(0),
        posledna_mapa,
        batoh,
        net_worth_fe,
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
// Záložka „Analýza" (TLI 13 §2) — vyhľadávanie, detail, export
// ─────────────────────────────────────────────────────────────────────────────

/// Vyhľadá behy podľa filtra. `None`/chyba otvorenej DB vráti prázdny zoznam
/// (záložka Analýza má zostať použiteľná aj keď DB zlyhala pri štarte —
/// `stav.chyba_db` nesie dôvod, UI ho zobrazí vedľa prázdneho zoznamu).
pub fn analyza_vyhladaj(stav: &StavTrackera, filter: &db::FilterBehov) -> Result<Vec<db::BehRiadok>, String> {
    let Some(conn) = &stav.db else { return Ok(Vec::new()) };
    db::vyhladaj_behy(conn, filter).map_err(|e| e.to_string())
}

/// Súhrn monštier podľa rarity pre jeden beh — priamy podklad pre Richardov
/// príkladný dotaz „koľko monštier a akej rarity mala ktorá mapa".
pub fn analyza_monstra_podla_rarity(stav: &StavTrackera, beh_id: i64) -> Result<Vec<(u8, i64)>, String> {
    let Some(conn) = &stav.db else { return Ok(Vec::new()) };
    db::monstra_podla_rarity(conn, beh_id).map_err(|e| e.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Záložka „Analýza" — taktiky (TLI 13b)
// ─────────────────────────────────────────────────────────────────────────────

/// Zoskupí behy do taktík (podpis chart+kompasy+mapa+mód+modifikátory+build)
/// a vypočíta ich štatistiky — hotové na priamo zobrazenie v tabuľke.
/// Prázdna DB vráti prázdny zoznam, rovnako ako `analyza_vyhladaj`.
pub fn analyza_taktiky(
    stav: &StavTrackera,
    filter: &db::FilterTaktik,
) -> Result<tli_overlay_core::tracker::taktiky::AnalyzaTaktik, String> {
    use tli_overlay_core::tracker::taktiky;
    let Some(conn) = &stav.db else { return Ok(taktiky::AnalyzaTaktik::default()) };
    let riadky = db::data_pre_taktiky(conn, filter).map_err(|e| e.to_string())?;
    let modifikatory = db::modifikator_podpisy(conn).map_err(|e| e.to_string())?;
    let priradenia = db::nacitaj_priradenia_taktik(conn).map_err(|e| e.to_string())?;
    let vstupy = taktiky::priprav_vstupy(&riadky, &modifikatory);
    let skupiny = taktiky::zoskup(&vstupy, &priradenia);
    Ok(taktiky::priprav_analyzu(skupiny))
}

/// Pomenuje jednu taktiku alebo zlúči viac podpisov do jednej pomenovanej
/// taktiky — `podpisy` sú `TaktikaSkupina.podpisy` vybraných riadkov z UI.
pub fn taktika_pomenuj_alebo_zluc(stav: &StavTrackera, podpisy: &[String], nazov: &str) -> Result<i64, String> {
    let Some(conn) = &stav.db else {
        return Err("Lokálna databáza nie je otvorená.".to_string());
    };
    db::ulozit_taktiku(conn, podpisy, nazov).map_err(|e| e.to_string())
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
/// Jedno kolo synchronizácie cien s partiou (TLI 20).
///
/// Poradie je zámerné: najprv UPLOAD (nech partia hneď vie, čo som si
/// vyhľadal), až potom DOWNLOAD. Obe HTTP volania idú MIMO zámku nad
/// `StavTrackera` — inak by sa pri pomalej sieti zastavilo aj čítanie logu.
///
/// Chyba siete sa nevracia ako chyba appky: je úplne normálne, že hráč hrá
/// bez pripojenia na náš server. Vracia `Err` len preto, aby to vedelo
/// tlačidlo v UI zobraziť, keď o to človek sám požiada.
pub fn sync_cien_kolo(app: &AppHandle) -> Result<String, String> {
    use tli_overlay_core::tracker::ceny_sync;

    let n = {
        let stav = app.state::<Stav>();
        let n = stav.nastavenia.lock().unwrap().clone();
        n
    };
    if !n.ceny_sync {
        return Ok("Zdieľanie cien s partiou je vypnuté.".to_string());
    }
    if let Some(chyba) = n.chyba_konfiguracie() {
        return Err(chyba);
    }

    let tracker = app.state::<Tracker>();
    let (davka, od) = {
        let t = tracker.0.lock().unwrap();
        let Some(conn) = t.db() else {
            return Err("Lokálna databáza nie je otvorená, ceny sa nedajú poslať.".to_string());
        };
        let zaznamy = db::ceny_od(conn, t.ceny_poslane_do, ceny_sync::MAX_NA_DAVKU)
            .map_err(|e| format!("Ceny sa nedajú prečítať z databázy: {e}"))?;
        (
            ceny_sync::vyber_na_odoslanie(&zaznamy, ceny_sync::MAX_NA_DAVKU),
            t.ceny_stiahnute_do,
        )
    };

    let mut poslanych = 0usize;
    if !davka.is_empty() {
        let telo = ceny_sync::telo_uploadu(&davka, env!("CARGO_PKG_VERSION"));
        let odpoved = crate::klient::posli_ceny(&n, &telo)?;
        ceny_sync::z_odpovede_uploadu(&odpoved)?;
        poslanych = davka.len();
        // Za odoslané sa považuje aj to, čo server „preskočil" — mal to už
        // od nás, posielať to znova nemá zmysel.
        let posledny = davka.iter().map(|c| c.zapisane).max().unwrap_or(0);
        let mut t = tracker.0.lock().unwrap();
        t.ceny_poslane_do = t.ceny_poslane_do.max(posledny);
    }

    let odpoved = crate::klient::stiahni_ceny(&n, od)?;
    let agregat = ceny_sync::z_odpovede_agregatu(&odpoved)?;
    let stiahnutych = agregat.ceny.len();
    let zmenenych = {
        let teraz = unixovy_cas();
        let mut t = tracker.0.lock().unwrap();
        let zmenenych = t.prijmi_ceny_partie(&agregat, teraz);
        t.ceny_stiahnute_do = agregat.cas;
        zmenenych
    };
    if zmenenych > 0 {
        let _ = app.emit(UDALOST_ZMENA, 0);
        // TLI 43 bod 3 — „prepočítaj ocenenie behov druhým prechodom": cenník
        // sa práve rozšíril (typicky titrack.ninja pokryl položky, ktoré
        // partia nikdy nevyhľadala), takže staré behy uviaznuté na
        // `cisty_zisk_fe IS NULL` môžu teraz dostať hodnotu. Rovnaký princíp
        // ako druhý prechod pri importe logu (`import.rs`), len nad celou
        // históriou, nie len nad práve prečítaným úsekom. Bez vlákna navyše —
        // toto UŽ BEŽÍ na vlákne trackera, krátky dobeh (`LIMIT_PRECENENIA`)
        // stačí, zvyšok doženie ďalšie kolo o `ceny_sync::INTERVAL_S`.
        const LIMIT_PRECENENIA: usize = 200;
        let t = tracker.0.lock().unwrap();
        if let Some(conn) = t.db() {
            // Best-effort, rovnako ako ostatné zápisy v tomto bloku vyššie
            // (`let _ = db::uloz_cenu(...)`) — chyba tu appku nemá zastaviť,
            // ďalšie kolo o `ceny_sync::INTERVAL_S` to skúsi znova.
            let _ = db::precen_nevycislene(conn, &t.cennik, &Nastavenie::default(), LIMIT_PRECENENIA);
        }
    }
    Ok(ceny_sync::zhrnutie(poslanych, stiahnutych, zmenenych))
}

/// Prečíta presne `dlzka` bajtov od `od` v danom súbore. Samostatná funkcia
/// kvôli čitateľnosti volania v `upload_kolo_pre_subor` (dve volania s
/// rôznou dĺžkou — malé okno na odtlačok, veľký kúsok na odoslanie).
fn precitaj_usek(subor: &mut std::fs::File, od: u64, dlzka: u64) -> Result<Vec<u8>, String> {
    subor
        .seek(SeekFrom::Start(od))
        .map_err(|e| format!("v logu sa nedá posunúť: {e}"))?;
    let mut bajty = vec![0u8; dlzka as usize];
    subor.read_exact(&mut bajty).map_err(|e| format!("log sa nedá prečítať: {e}"))?;
    Ok(bajty)
}

/// Jedno kolo denného uploadu JEDNÉHO súboru (TLI 14) — pošle NAJVIAC jeden
/// kúsok (`upload::KUSOK_BAJTOV`); volajúci (`upload_kolo`) ho preto volá
/// v slučke, kým nevráti `poslanych == 0`.
///
/// Zámerne nikdy nedrží celý súbor v pamäti (dokumentácia `upload.rs`) —
/// prečíta len malé okno na rozpoznanie rotácie a samotný kúsok na odoslanie.
/// Stav sa ukladá do DB PO KAŽDOM kole, nie až na konci, aby reštart appky
/// uprostred veľkého dobiehania nestratil už potvrdený pokrok.
fn upload_kolo_pre_subor(
    app: &AppHandle,
    n: &Nastavenia,
    cesta: &Path,
) -> Result<u64, String> {
    use tli_overlay_core::tracker::upload;

    let Some(nazov) = cesta.file_name().and_then(|n| n.to_str()) else {
        return Err("Log má neplatné meno súboru.".to_string());
    };

    let tracker = app.state::<Tracker>();
    let stav_uploadu = {
        let t = tracker.0.lock().unwrap();
        let conn = t.db().ok_or_else(|| {
            "Lokálna databáza nie je otvorená, upload logu sa nedá sledovať.".to_string()
        })?;
        db::nacitaj_upload_stav(conn, nazov)
            .map_err(|e| format!("Stav uploadu sa nedá prečítať: {e}"))?
            .unwrap_or_default()
    };

    let mut subor = std::fs::File::open(cesta).map_err(|e| format!("log sa nedá otvoriť: {e}"))?;
    let velkost = subor.metadata().map_err(|e| format!("log sa nedá otvoriť: {e}"))?.len();

    let okno_pred = precitaj_usek(&mut subor, 0, stav_uploadu.nahrana_pozicia.min(256))?;
    let segment = upload::priprav_segment(&stav_uploadu, &okno_pred, velkost);
    let dlzka = upload::dlzka_kusku(segment, velkost);
    if dlzka == 0 {
        return Ok(0);
    }

    let kusok = precitaj_usek(&mut subor, segment.pozicia_od, dlzka)?;
    let odpoved = crate::klient::posli_upload_log(n, nazov, segment.pozicia_od, &kusok)?;
    if !odpoved.ok {
        // 409 (medzera) nesie `ocakavana_pozicia` — appka to tu len vracia ako
        // chybu pre toto kolo, ďalšie kolo prečíta stav z DB odznova a
        // `priprav_segment` prirodzene skúsi inú pozíciu (server mu medzitým
        // nič neuložil, takže `nahrana_pozicia` v DB ostáva presná).
        return Err(odpoved.chyba.unwrap_or_else(|| "Server upload odmietol.".to_string()));
    }

    let okno_po = precitaj_usek(&mut subor, 0, odpoved.prijata_pozicia_do.min(velkost).min(256))?;
    let novy_stav = upload::po_uspesnom_uploade(nazov, &okno_po, odpoved.prijata_pozicia_do);
    {
        let t = tracker.0.lock().unwrap();
        if let Some(conn) = t.db() {
            let _ = db::uloz_upload_stav(conn, &novy_stav);
        }
    }
    Ok(odpoved.bajtov_novych)
}

/// Jedno kolo denného uploadu (TLI 14) — nahradzuje ručné kopírovanie logu
/// na zdieľaný disk (zadanie `2026-10-16-1208-mcp.md`). Beží na pozadí á
/// `upload::INTERVAL_S`, rovnakým vzorom ako `sync_cien_kolo`: HTTP mimo
/// zámku nad trackerom, chyba siete sa nehlási ako chyba appky (hráč môže
/// hrať offline), len sa ticho skúsi nabudúce.
pub fn upload_kolo(app: &AppHandle) -> Result<String, String> {
    let vysledok = upload_kolo_vnutorne(app);

    // TLI 21 (`2026-10-16-1528-mcp.md` bod 4): predtým sa chyba tu zahodila
    // (`let _ = upload_kolo(&app);` vo `spusti_vlakno`) a appka nikde
    // nenechala stopu, ŽE sa o upload vôbec pokúsila — `tracker_upload_stav`
    // v DB ostávala prázdna, akoby sa trigger nikdy nespustil, hoci reálne
    // bežal a len padal na chybe HTTP/servera. Teraz sa VŽDY zapíše aspoň
    // čas posledného pokusu a prípadná chyba, nech záložka Tracker vie
    // ukázať stav, aj keď je upload ticho (na pozadí, bez vyvolania UI).
    let tracker = app.state::<Tracker>();
    let mut t = tracker.0.lock().unwrap();
    t.posledna_kontrola_uploadu = Some(unixovy_cas());
    t.posledna_chyba_uploadu = vysledok.as_ref().err().cloned();

    vysledok
}

fn upload_kolo_vnutorne(app: &AppHandle) -> Result<String, String> {
    let n = {
        let stav = app.state::<Stav>();
        let n = stav.nastavenia.lock().unwrap().clone();
        n
    };
    if !n.upload_logov {
        return Ok("Upload logov je vypnutý.".to_string());
    }
    if let Some(chyba) = n.chyba_konfiguracie() {
        return Err(chyba);
    }

    let cesta = {
        let tracker = app.state::<Tracker>();
        let t = tracker.0.lock().unwrap();
        t.cesta_logu().map(Path::to_path_buf)
    };
    let Some(cesta) = cesta else {
        return Ok("Cesta k logu zatiaľ nie je známa.".to_string());
    };

    // Najviac desať kôl naraz — appka dlho vypnutá (veľký nedohnaný rozdiel)
    // sa tak dobehne v niekoľkých kusoch v tomto volaní, ale aj tak nikdy
    // nezablokuje vlákno na neurčito, keby bol rozdiel obrovský.
    let mut poslanych_spolu = 0u64;
    for _ in 0..10 {
        let poslanych = upload_kolo_pre_subor(app, &n, &cesta)?;
        if poslanych == 0 {
            break;
        }
        poslanych_spolu += poslanych;
    }

    if poslanych_spolu == 0 {
        Ok("Upload je dohnaný, nič nové na poslanie.".to_string())
    } else {
        Ok(format!("Nahraných {poslanych_spolu} B."))
    }
}

pub fn spusti_vlakno(app: AppHandle) {
    std::thread::spawn(move || {
        use notify::{RecommendedWatcher, RecursiveMode, Watcher};
        use std::sync::mpsc::{channel, RecvTimeoutError};

        let (tx, rx) = channel::<notify::Result<notify::Event>>();
        let mut watcher: Option<RecommendedWatcher> = None;
        let mut sledovany_priecinok: Option<PathBuf> = None;
        // Prvé kolo synchronizácie cien nech prebehne hneď po štarte appky,
        // nie až o päť minút — hráč typicky spúšťa appku pred hraním a chce
        // mať ceny partie k dispozícii od prvej mapy.
        let mut posledny_sync_cien: u64 = 0;
        // Upload logov (TLI 14) — tiež hneď po štarte, nie až o desať minút:
        // appka sa typicky spúšťa krátko pred hraním a Richard má najväčší
        // úžitok z toho, keď je log na serveri skôr, než si naň vôbec spomenie.
        let mut posledny_upload: u64 = 0;

        loop {
            // Daň (`tracker_dan`) sa uplatňuje až pri zostavovaní prehľadu pre
            // UI (`prehlad_pre_ui`), tu sa zámerne nepoužíva.
            let (zapnuty, rucna, pripominat, ceny_obrazovka_zapnute) = {
                let stav = app.state::<Stav>();
                let n = stav.nastavenia.lock().unwrap();
                (n.tracker_zapnuty, n.cesta_logu.clone(), n.pripominat_enable_log, n.ceny_obrazovka_zapnute)
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

            // TLI 24 (`2026-10-17-1238-mcp.md` bod 5): donedávna bol celý
            // zvyšok tejto iterácie — VRÁTANE synchronizácie cien partie aj
            // uploadu logov nižšie — za jedným `continue`, ktorý závisel od
            // farm trackera (`zapnuty` aj úspešné `najdi_log`). Keď hráč
            // farm tracker nemal zapnutý alebo appka log ešte nenašla,
            // `upload_kolo`/`sync_cien_kolo` sa NIKDY nedostali na rad —
            // `tracker_upload_stav` tak v DB zostávalo prázdne bez ohľadu na
            // to, že `upload_logov` je VLASTNÉ, nezávislé nastavenie. Čítanie
            // samotného herného logu (nižšie v tomto bloku) zostáva viazané
            // na `zapnuty`/`cesta` — len sa už neťahá za sebou zvyšok tiku.
            if zapnuty {
                if let Some(cesta) = cesta {
                    let tracker = app.state::<Tracker>();
                    let (novych, pripomienka_udalosti, trh_otvoreny) = {
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
                        (novych, t.pripomienka.odober_udalosti(), t.odober_trh_otvoreny())
                    };
                    if novych > 0 {
                        let _ = app.emit(UDALOST_ZMENA, novych);
                    }
                    for udalost in pripomienka_udalosti {
                        oznam_pripomienku(&app, udalost);
                    }

                    // TLI 22: appka SAMA odfotí a pošle, len čo log ukáže otvorenie
                    // obrazovky trhu — žiadny hotkey netreba (ten je len nepovinný
                    // ručný fallback, `hotkey_trh`). `spusti_analyzu` má vlastný
                    // zámok `beh` proti súbehu s F8/F9 aj proti dvom rýchlo za sebou
                    // idúcim otvoreniam.
                    if trh_otvoreny && ceny_obrazovka_zapnute {
                        crate::spusti_analyzu(app.clone(), crate::Modul::Ceny, "auto");
                    }
                }
            }

            // Ceny partie (TLI 20) a upload logov (TLI 14) — á niekoľko minút,
            // v tom istom vlákne. Vlastné vlákno by si pýtalo ďalší zámok
            // a nič by nepridalo: toto beží aj tak každú sekundu a HTTP ide
            // mimo zámku nad trackerom. ZÁMERNE mimo bloku vyššie — obe sú
            // vlastné, nezávislé nastavenia (`ceny_sync`/`upload_logov`) a
            // nemajú dôvod čakať na farm tracker.
            let teraz = unixovy_cas();
            if teraz.saturating_sub(posledny_sync_cien)
                >= tli_overlay_core::tracker::ceny_sync::INTERVAL_S
            {
                posledny_sync_cien = teraz;
                // Chyba siete tu NIE JE dôvod na hlášku — hráč môže hrať bez
                // pripojenia a appka má v takom prípade ticho pokračovať
                // s vlastnými cenami. Zobrazí sa až keď o sync požiada sám.
                let _ = sync_cien_kolo(&app);
            }

            // Upload logov (TLI 14) — á `upload::INTERVAL_S`, rovnaký dôvod
            // zdieľaného vlákna ako vyššie pri cenách. Chyba siete sa opäť
            // nehlási: appka má ďalej fungovať offline, log sa dohoní pri
            // ďalšom kole.
            if teraz.saturating_sub(posledny_upload)
                >= tli_overlay_core::tracker::upload::INTERVAL_S
            {
                posledny_upload = teraz;
                let _ = upload_kolo(&app);
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
        assert_eq!(prehlad_pre_ui(&stav, true, false, &[]).neznamych_riadkov, 3);

        pripis(&cesta, "treti\n");
        precitaj(&mut stav, &cesta).unwrap();
        assert_eq!(
            prehlad_pre_ui(&stav, true, false, &[]).neznamych_riadkov,
            5,
            "pridaný riadok sa mal dočítať navyše, nie nahradiť predošlý počet"
        );

        // Bez zmeny súboru druhé čítanie nenájde nič nové.
        let n3 = precitaj(&mut stav, &cesta).unwrap();
        assert_eq!(n3, 0);
        assert_eq!(prehlad_pre_ui(&stav, true, false, &[]).neznamych_riadkov, 5);

        // Rotácia: hra súbor nahradí menším — má sa dočítať celý nový súbor
        // od začiatku, nič navyše a nič chýbajúce.
        napis(&cesta, "x\n");
        precitaj(&mut stav, &cesta).unwrap();
        assert_eq!(prehlad_pre_ui(&stav, true, false, &[]).neznamych_riadkov, 7);

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

    // ─── TLI 19 bug E: záloha identity pre živé čítanie ─────────────────

    fn prazdny_beh_pre_test(zona: &str) -> Beh {
        Beh {
            id: 1,
            zona: zona.to_string(),
            je_hub: false,
            level_uid: None,
            level_typ: None,
            level_id: None,
            zaciatok: None,
            koniec: None,
            otvorenie_bez_spotreby: false,
            dropy: BTreeMap::new(),
            vstupy: BTreeMap::new(),
            vsetky_zmeny: Vec::new(),
            monstra: Vec::new(),
            modifikatory: Vec::new(),
            bossovia: Vec::new(),
            canvas_akcie: Vec::new(),
        }
    }

    #[test]
    fn uloz_beh_do_db_pouzije_zalohu_identity_ked_live_nevidelo_ziadnu() {
        use tli_overlay_core::tracker::stav::HracSnimka;

        let mut stav = StavTrackera::default();
        // Izolovaná DB len pre tento test — `StavTrackera::default()` by inak
        // otvoril ten istý zdieľaný súborový `tracker.sqlite3` ako ostatné
        // testy v tomto module.
        stav.db = Some(db::otvor_v_pamati().unwrap());

        // Beh 1: predstiera, že v TEJTO session `GetPlayerData`/`+player+...`
        // naozaj prišlo (napr. appka bežala už pred prihlásením) — uloží sa
        // priamo cez `db::uloz_beh`, nezávisle od `stav.sledovac`.
        let identita = HracSnimka {
            player_id: Some("947466339310800896".to_string()),
            season_id: Some("1501".to_string()),
            hero_id: Some("1800".to_string()),
            ..Default::default()
        };
        {
            let conn = stav.db.as_mut().unwrap();
            db::uloz_beh(conn, &prazdny_beh_pre_test("Mapa1"), &identita, &OceneneHodnoty::default()).unwrap();
        }

        // Beh 2: živé čítanie v TEJTO session nevidelo ŽIADNY `+player+...`
        // blok (presne bug E — "Enable Log" sa zapol až po prihlásení) —
        // `stav.sledovac` je čerstvý, `hrac_snimka()` je `None`.
        assert!(stav.sledovac.hrac_snimka().is_none());
        uloz_beh_do_db(&mut stav, &prazdny_beh_pre_test("Mapa2"));

        let conn = stav.db.as_ref().unwrap();
        let player_id: String = conn
            .query_row("SELECT player_id FROM behy WHERE zona = 'Mapa2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            player_id, "947466339310800896",
            "beh bez vlastnej identity mal dostať zálohu z posledného známeho behu (bug E)"
        );
    }

    #[test]
    fn uloz_beh_do_db_necakanu_identitu_nenahradza_zalohou() {
        use tli_overlay_core::tracker::log::Udalost;
        use tli_overlay_core::tracker::stav::HracSnimka;

        let mut stav = StavTrackera::default();
        stav.db = Some(db::otvor_v_pamati().unwrap());

        let stara_identita = HracSnimka { player_id: Some("STARA".to_string()), ..Default::default() };
        {
            let conn = stav.db.as_mut().unwrap();
            db::uloz_beh(conn, &prazdny_beh_pre_test("Mapa1"), &stara_identita, &OceneneHodnoty::default()).unwrap();
        }

        // Live čítanie TOTO beh naozaj identifikovalo — záloha sa nesmie
        // použiť namiesto skutočne zistenej identity.
        for riadok in ["+player+Name [Burina]", "+player+PlayerId [NOVA]", "+player+SeasonId [1501]"] {
            stav.sledovac.spracuj_udalost(None, Udalost::TeloSpravy { surovy: riadok.to_string() });
        }
        stav.sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Mapa2".to_string() });

        uloz_beh_do_db(&mut stav, &prazdny_beh_pre_test("Mapa2"));

        let conn = stav.db.as_ref().unwrap();
        let player_id: String = conn
            .query_row("SELECT player_id FROM behy WHERE zona = 'Mapa2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(player_id, "NOVA");
    }

    // TLI 21 (`2026-10-16-1528-mcp.md` bod 4) — predtým sa chyba uploadu
    // vždy ticho zahodila (`let _ = upload_kolo(&app);`) a appka nikde
    // nenechala stopu, že sa o to vôbec pokúsila; `prehlad_pre_ui` teraz musí
    // vedieť vrátiť stav aj bez skutočného HTTP volania/AppHandle.

    #[test]
    fn prehlad_bez_pokusu_o_upload_nema_ani_kontrolu_ani_chybu() {
        let stav = StavTrackera::default();
        let p = prehlad_pre_ui(&stav, true, false, &[]);
        assert_eq!(p.posledna_kontrola_uploadu_pred, None);
        assert_eq!(p.posledna_chyba_uploadu, None);
    }

    #[test]
    fn prehlad_ukazuje_cas_poslednej_kontroly_a_chybu() {
        let mut stav = StavTrackera::default();
        stav.posledna_kontrola_uploadu = Some(unixovy_cas());
        stav.posledna_chyba_uploadu = Some("server vrátil 500".to_string());

        let p = prehlad_pre_ui(&stav, true, false, &[]);
        assert_eq!(p.posledna_kontrola_uploadu_pred.as_deref(), Some("teraz"));
        assert_eq!(p.posledna_chyba_uploadu.as_deref(), Some("server vrátil 500"));
    }

    #[test]
    fn prehlad_bez_chyby_po_uspesnom_pokuse_je_none() {
        let mut stav = StavTrackera::default();
        stav.posledna_kontrola_uploadu = Some(unixovy_cas());
        stav.posledna_chyba_uploadu = None;

        let p = prehlad_pre_ui(&stav, true, false, &[]);
        assert!(p.posledna_kontrola_uploadu_pred.is_some());
        assert_eq!(p.posledna_chyba_uploadu, None);
    }

    #[test]
    fn zmena_cesty_logu_resetuje_stav_uploadu() {
        // `precitaj` prestavia `StavTrackera` nanovo, keď sa zmení sledovaná
        // cesta (nová session/iný súbor) — stav uploadu patrí k TOMUTO súboru,
        // takže reset je správny, nie strata dát.
        let cesta_a = docasny_subor();
        napis(&cesta_a, "a\n");
        let mut stav = StavTrackera::default();
        precitaj(&mut stav, &cesta_a).unwrap();
        stav.posledna_kontrola_uploadu = Some(unixovy_cas());
        stav.posledna_chyba_uploadu = Some("chyba pre starý súbor".to_string());

        let cesta_b = docasny_subor();
        napis(&cesta_b, "b\n");
        precitaj(&mut stav, &cesta_b).unwrap();

        assert_eq!(stav.posledna_kontrola_uploadu, None);
        assert_eq!(stav.posledna_chyba_uploadu, None);

        let _ = std::fs::remove_file(&cesta_a);
        let _ = std::fs::remove_file(&cesta_b);
    }
}
