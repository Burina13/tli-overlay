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
use tli_overlay_core::tracker::stav::{Beh, Sledovac, POLOZKA_FE, STRANKA_BATOHU_OSTATNE, STRANKY_BATOHU_SLEDOVANE};
use tli_overlay_core::tracker::steam;
use tli_overlay_core::tracker::trh::je_otvorenie_trhu;
use tli_overlay_core::tracker::varovania::SledovacVarovania;

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

/// Po koľkých sekundách ticha (pri bežiacej hre) appka ukáže „Log sa
/// neposiela na server" — rovnaká hranica ako `UPLOAD_VAROVANIE_PO_S`
/// v `ui/hlavne.js` (pätička dashboardu), len na strane Rustu pre toast nad
/// hrou (TLI 48 bod 1a). Dve nezávislé miesta s rovnakou hodnotou, nie jeden
/// zdieľaný zdroj — JS a Rust si medzi sebou konštanty neposielajú nikde inde
/// v appke a zavádzať to len pre toto jedno číslo by bolo neúmerné.
pub const UPLOAD_VAROVANIE_PO_S: u64 = 15 * 60;

/// TLI 61 (zadanie `2026-10-19-2241-mcp.md`): Richard 19.10.2026 ~21:40 —
/// overlay mu ukazoval „SESSION ČAS 1173 m 41 s" (~19,5 h), hoci práve hral
/// pár desiatok minút. Príčina: `stat_reset_unix` (vyššie, TLI 56) sa
/// nastaví LEN ručným kliknutím na „Reset Stats" a od zavedenia spätného
/// načítania behov z DB (TLI 56) sa `celkom` v `Prehlad` preto ticho sčítaval
/// cez VIACERO dní a reštartov appky, kým naň hráč nekliklo — presne to, čo
/// TITrack rieši automatickým štartom novej session po dlhšej nečinnosti.
/// Prah 30 minút je priamo zo zadania.
pub const SESSION_NECINNOST_S: u64 = 30 * 60;

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
    /// TLI 48 bod 4: veľkosť logu v momente, keď appka cestu k nemu otvorila
    /// (nanovo, alebo po rotácii) — teda hranica medzi „starým obsahom, ktorý
    /// appka len DOBIEHA" a „naozaj novými riadkami, ktoré hra píše PRÁVE
    /// TERAZ". `None`, kým appka ešte neotvorila žiadny log.
    povodna_velkost_logu: Option<u64>,
    /// TLI 48 bod 1a/1c: opakované toasty „Log sa neposiela na server" a
    /// „Sortni inventár" — rovnaký stav, ktorý už vidí pätička dashboardu
    /// (`vykresliPatickaUploadu`/`vykresliVyzvuNaSort` v `ui/hlavne.js`), len
    /// navyše posielaný nad hru. Dve samostatné inštancie, nech obnovovanie
    /// jednej nereštartuje časovanie druhej.
    pub varovanie_uploadu: SledovacVarovania,
    pub varovanie_sortu: SledovacVarovania,
    /// TLI 49 bod 2: kedy appka naposledy DOKONČILA kolo synchronizácie cien
    /// s partiou (`sync_cien_kolo`) — nezávisle od toho, či sa pritom nejaká
    /// cena naozaj zmenila. Footer dashboardu z toho ukazuje „ceny
    /// aktualizované HH:MM"; `None`, kým appka ešte ani raz kolo nedokončila.
    pub posledna_synchronizacia_cien: Option<u64>,
    /// TLI 55 (zadanie `2026-10-19-1715-mcp.md`): skutočný (`unixovy_cas()`)
    /// čas, kedy appka PRVÝKRÁT uvidela aktuálne bežiaci beh — `(beh.id, kedy)`.
    /// `Beh::trvanie_ms()` vracia `None`/`0`, kým mapa beží (chýba `koniec`),
    /// takže naň sa živý čas oprieť nedá. Riešenie zámerne NEPOUŽÍVA
    /// `beh.zaciatok` (čas z HLAVIČKY LOGU — ten je lokálny, ale appka ho
    /// vie porovnať len s `unixovy_cas()`, čo je skutočné UTC; rozdiel časových
    /// pásiem by živý čas posunul o celé hodiny). Namiesto toho appka meria
    /// VLASTNÝ wall-clock čas od chvíle, keď live čítanie logu Zónu
    /// zaregistrovalo — presnosť na interval čítania (rádovo sekundy), čo
    /// stačí na „tiká každú sekundu ako TITrack". Pri appke spustenej
    /// uprostred už bežiacej mapy (dobiehanie logu) preto časovač začne od
    /// momentu spustenia appky, nie od skutočného začiatku mapy — known
    /// limitation, zdokumentované v `CHANGELOG.md`.
    zivy_zaciatok_real: Option<(u64, u64)>,
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
        // TLI 56 (`2026-10-19-1740-mcp.md` bod 3): rovnaká OFFLINE CACHE
        // filozofia ako pri cenníku vyššie, len pre behy. Appka doteraz
        // RUNS/Recent Runs/FE-HOUR po KAŽDOM reštarte vynulovala, hoci tie
        // isté mapy už dávno ležia v tejto DB — `chvost.pozicia` sa pri
        // reštarte nevracia na začiatok, takže live čítanie logu staré
        // riadky znova neuvidí. `nacitaj_zive_behy` vráti len to, čo ešte
        // neprekryl posledný „Reset Stats" (`tracker_meta.stat_reset_unix`).
        // `#[cfg(not(test))]`: `cesta_db_suboru()` nie je v testoch izolovaná
        // (žiadny dočasný adresár na beh) — je to ten istý skutočný súbor pre
        // VŠETKY testy v tomto builde (`$HOME/.config/.../tracker.sqlite3`),
        // ktoré navyše bežia paralelne. Testy preto už dávno spoliehajú na
        // to, že odtiaľ načítaný `cennik`/`behy` môže byť hocičo, a buď si
        // vlastné dáta prepíšu (`stav.cennik.vloz(...)`), alebo si DB
        // nahradia vlastnou `db::otvor_v_pamati()`. Správny fix (spätné
        // načítanie behov pri štarte appky) by v builde pre testy spôsobilo
        // presne to, čomu sa doteraz vyhýbali — testy typu
        // „net_worth_sa_uz_nescitava_s_cistym_ziskom..." počítajú s PRESNÝM
        // súčtom nad `stav.behy`, ktorý by sa krížovou kontamináciou zo
        // zdieľaného súboru stal nedeterministickým. `nacitaj_zive_behy` má
        // vlastné jednotkové testy nad izolovanou `otvor_v_pamati()` DB
        // (`core/src/tracker/db.rs`), tie overujú správnosť už aj bez tohto
        // prepojenia.
        #[cfg(not(test))]
        let behy = db.as_ref().and_then(|c| db::nacitaj_zive_behy(c).ok()).unwrap_or_default();
        #[cfg(test)]
        let behy = Vec::new();
        Self {
            sledovac: Sledovac::novy(),
            cennik,
            behy,
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
            povodna_velkost_logu: None,
            varovanie_uploadu: SledovacVarovania::novy(),
            // TLI 57: sort sa opakuje oveľa zriedkavejšie než upload — pozri
            // `OPAKOVAT_PO_S_SORT`.
            varovanie_sortu: SledovacVarovania::novy_s_intervalom(
                tli_overlay_core::tracker::varovania::OPAKOVAT_PO_S_SORT,
            ),
            posledna_synchronizacia_cien: None,
            zivy_zaciatok_real: None,
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

    /// Aktuálne bežiaci beh mapy (ak nejaký) — pre moduly MIMO trackera,
    /// ktoré potrebujú len zónu/`je_hub`/`level_uid`, nie celé API trackera
    /// (TLI 62/3, zber minimapy: appka nemá druhý sledovač `UE_game.log`,
    /// túto informáciu už parsuje tracker, TLI 11c).
    pub fn aktualny_beh(&self) -> Option<&Beh> {
        self.sledovac.aktualny()
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
                        // TLI 49 bod 2: predtým tu bolo `c.kedy` — teda čas,
                        // kedy NIEKTO V PARTII (alebo titrack.ninja) tú cenu
                        // VYHĽADAL, nie kedy ju TÁTO appka stiahla. Richard na
                        // vlastnom PC videl 8 919 riadkov `ceny` so zdrojom
                        // `titrack` bez čitateľného „kedy stiahnuté" — presne
                        // preto, že `zapisane` niesol cudzí, často starý čas.
                        // `Cennik.kedy` (vyššie, `zapis_do_cennika`) ZOSTÁVA
                        // `c.kedy` zámerne — tým sa riadi čerstvosť/doveryhodnosť
                        // ceny (`vaha_veku`), tam ide o vek SAMOTNEJ CENY.
                        // `zapisane` v lokálnej DB je iný údaj: kedy TÁTO appka
                        // záznam dostala — na to slúži „ceny aktualizované
                        // HH:MM" v pätičke aj triedenie v `ceny_pred`.
                        zapisane: teraz as i64,
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
    /// nemaž") — toto nemaže žiadny súbor, len vyprázdni zoznam v pamäti.
    ///
    /// TLI 56: od zavedenia spätného načítania behov z DB (`StavTrackera::
    /// default`, `db::nacitaj_zive_behy`) by vyprázdnenie LEN pamäte
    /// nevydržalo do ďalšieho reštartu appky — `behy` by sa znova naplnilo
    /// tými istými starými mapami. Preto sa „teraz" zapisuje aj do
    /// `tracker_meta.stat_reset_unix`: ďalšie spätné načítanie (aj po
    /// reštarte) oreže všetko spred tohto okamihu rovnako, ako je už teraz
    /// orezané v pamäti.
    pub fn resetuj_zive_staty(&mut self) {
        self.behy.clear();
        if let Some(conn) = &self.db {
            let _ = db::nastav_meta(conn, "stat_reset_unix", &unixovy_cas().to_string());
        }
    }

    /// TLI 61: automatický náprotivok ručného „Reset Stats"/„Reset session".
    /// Volá sa z `precitaj()` pri KAŽDOM tiku (teda aj keď appka beží, ale
    /// hra práve nie — presne to je „pri štarte appky" aj „pri štarte hry"
    /// naraz, bez toho, aby appka musela osobitne sledovať proces hry):
    /// porovná „teraz" s `tracker_meta.posledna_aktivita_unix` (čas
    /// POSLEDNÉHO reálneho nového obsahu v logu, nastavuje `precitaj` nižšie)
    /// a keď je medzera väčšia než `SESSION_NECINNOST_S`, zavolá
    /// `resetuj_zive_staty()` presne tak, ako keby hráč klikol na tlačidlo.
    ///
    /// `!self.behy.is_empty()` je poistka proti opakovanému resetu na každý
    /// tik počas dlhšej nečinnosti (appka otvorená, hra zavretá): po prvom
    /// automatickom resete je `behy` prázdne, takže ďalšie tiky už nemajú čo
    /// čistiť a mlčia, kým nepribudne nová mapa.
    fn over_automaticky_reset_session(&mut self, teraz: u64) {
        let Some(conn) = self.db.as_ref() else { return };
        let posledna_aktivita: Option<u64> = db::precitaj_meta(conn, "posledna_aktivita_unix")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok());
        let necinna_pridlho = match posledna_aktivita {
            None => true,
            Some(p) => teraz.saturating_sub(p) > SESSION_NECINNOST_S,
        };
        if necinna_pridlho && !self.behy.is_empty() {
            self.resetuj_zive_staty();
            crate::denik::zapis(
                "session",
                &format!(
                    "automatický reset session — nečinnosť dlhšia než {} min",
                    SESSION_NECINNOST_S / 60
                ),
            );
        }
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
    let obsahy = precitaj_logy_v_priecinku(cesta_aktualneho_logu)?;

    tli_overlay_core::tracker::import::importuj_subory(conn, &obsahy)
        .map_err(|e| format!("Import do databázy zlyhal: {e}"))
}

/// **TLI 68 bod e)** — JEDNORAZOVÝ prepočet nafúknutej histórie.
///
/// Behy uložené od TLI 56 (0.20.9, 19.10.2026) do tejto verzie môžu mať
/// dropy nafúknuté na celý stack z batohu, keď sa appka reštartovala
/// uprostred hrania (`StavChvosta` si drží pozíciu v logu cez reštart, stav
/// batohu nie). Prepočet prejde logy, ktoré má hráč ešte na disku, opraveným
/// `Sledovac`om a prepíše položky tých behov, ktoré v DB naozaj sú.
///
/// **Pred prepočtom sa vždy spraví KÓPIA súboru DB** (`tracker.sqlite3.
/// pred-tli68-<unix>`) a overí sa, že nie je prázdna — bez toho sa prepočet
/// vôbec nespustí. Nič sa nemaže: pôvodné hodnoty ostávajú v tej kópii
/// a behy, ktorých log už na disku nie je, sa nedotknú vôbec.
///
/// Beží RAZ (zarážka `tracker_meta.prepocet_tli68`) a na vlákne trackera,
/// nie pri štarte UI — nad gigabajtovým logom to trvá sekundy.
#[cfg(not(test))]
fn prepocitaj_historiu_tli68(stav: &mut StavTrackera, cesta_logu: &Path) {
    const ZARAZKA: &str = "prepocet_tli68";

    let Some(conn) = stav.db.as_ref() else { return };
    match db::precitaj_meta(conn, ZARAZKA) {
        Ok(Some(_)) => return, // už sa to raz spravilo
        Ok(None) => {}
        Err(_) => return,
    }

    let zdroj = crate::stav::cesta_db_suboru();
    let kam = zdroj.with_extension(format!("sqlite3.pred-tli68-{}", unixovy_cas()));
    // WAL: časť zápisov môže ležať v `-wal`, preto sa pred kópiou
    // checkpointuje — inak by záloha bola staršia než to, čo appka ukazuje.
    let _ = conn.pragma_update(None, "wal_checkpoint", "TRUNCATE");
    if let Err(e) = std::fs::copy(&zdroj, &kam) {
        crate::denik::zapis(
            "tracker",
            &format!("TLI 68: prepočet sa NESPUSTIL — zálohu DB sa nepodarilo spraviť ({e})"),
        );
        return;
    }
    match std::fs::metadata(&kam) {
        Ok(m) if m.len() > 0 => {}
        _ => {
            crate::denik::zapis(
                "tracker",
                "TLI 68: prepočet sa NESPUSTIL — záloha DB je prázdna alebo sa nedá prečítať",
            );
            return;
        }
    }

    let subory = match najdi_logy_v_priecinku(cesta_logu) {
        Ok(o) => o,
        Err(e) => {
            crate::denik::zapis("tracker", &format!("TLI 68: prepočet sa NESPUSTIL — {e}"));
            return;
        }
    };

    let Some(conn) = stav.db.as_mut() else { return };
    // Súbor po súbore, nie všetky naraz — rotované `UE_game-backup-*.log`
    // majú bežne stovky MB a appka ich nemá prečo držať v pamäti súčasne.
    let mut spolu = tli_overlay_core::tracker::import::VysledokPrepoctu::default();
    let mut chyba: Option<String> = None;
    for cesta in &subory {
        let Ok(bajty) = std::fs::read(cesta) else { continue };
        let text = tli_overlay_core::tracker::log::dekoduj(&bajty);
        drop(bajty);
        match tli_overlay_core::tracker::import::prepocitaj_text(conn, &text) {
            Ok(v) => {
                spolu.behov_prepocitanych += v.behov_prepocitanych;
                spolu.behov_mimo_db += v.behov_mimo_db;
                spolu.behov_precenenych += v.behov_precenenych;
            }
            Err(e) => {
                chyba = Some(e.to_string());
                break;
            }
        }
    }
    match chyba {
        None => {
            let _ = db::nastav_meta(conn, ZARAZKA, &unixovy_cas().to_string());
            crate::denik::zapis(
                "tracker",
                &format!(
                    "TLI 68: história prepočítaná z logu — prepísaných behov {}, \
                     mimo DB {}, precenených {}; pôvodné hodnoty sú v {}",
                    spolu.behov_prepocitanych,
                    spolu.behov_mimo_db,
                    spolu.behov_precenenych,
                    kam.display()
                ),
            );
        }
        Some(e) => crate::denik::zapis(
            "tracker",
            &format!("TLI 68: prepočet zlyhal ({e}) — pôvodné dáta ostávajú, zarážka sa nenastavila"),
        ),
    }
}

/// Nájde všetky `UE_game*.log` v priečinku daného logu, zoradené
/// (spoločné pre „Načítať staré logy" aj prepočet TLI 68).
fn najdi_logy_v_priecinku(cesta_logu: &Path) -> Result<Vec<PathBuf>, String> {
    let priecinok = cesta_logu
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
    Ok(subory)
}

/// To isté, ale rovno s obsahom súborov — „Načítať staré logy" ich potrebuje
/// naraz (`importuj_subory`).
fn precitaj_logy_v_priecinku(cesta_logu: &Path) -> Result<Vec<Vec<u8>>, String> {
    let mut obsahy = Vec::new();
    for cesta in najdi_logy_v_priecinku(cesta_logu)? {
        match std::fs::read(&cesta) {
            Ok(b) => obsahy.push(b),
            Err(e) => return Err(format!("Súbor {} sa nedá prečítať: {e}", cesta.display())),
        }
    }
    Ok(obsahy)
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
        // TLI 48 bod 4: hranica „odtiaľto je to naozaj NOVÉ" sa kreslí HNEĎ
        // pri otvorení cesty — všetko, čo appka ešte len DOBIEHA (`chvost.
        // pozicia` pri prvom otvorení môže byť aj 0, teda celý stomegový
        // log), sa nesmie správať ako práve prebiehajúca udalosť.
        stav.povodna_velkost_logu = Some(velkost);
        // TLI 68 bod e): jednorazový prepočet nafúknutej histórie. Beží na
        // vlákne trackera (nie pri štarte UI) a vnútri si sám overí zarážku
        // `tracker_meta.prepocet_tli68` aj to, že sa podarila záloha DB.
        #[cfg(not(test))]
        prepocitaj_historiu_tli68(stav, cesta);
    }
    let teraz = unixovy_cas();
    // TLI 61: kontrola nečinnosti beží PRED early-returnom nižšie — má
    // zafungovať aj vtedy, keď appka beží, ale log sa dlho nehol vôbec
    // (hráč medzitým hru zavrel), nielen pri práve prichádzajúcich riadkoch.
    stav.over_automaticky_reset_session(teraz);
    if stav.chvost.over_rotaciu(velkost) {
        // Hra pri reštarte premenovala/skrátila log — nová session, aj pre
        // pripomienku „Enable Log" (TLI 11d). Nový súbor je od pozície 0 už
        // naozaj živý (hra ho práve píše po reštarte), hranica sa preto
        // posúva na začiatok, nie na jeho aktuálnu (medzičasom narastenú)
        // veľkosť.
        stav.pripomienka.nova_session(teraz);
        stav.povodna_velkost_logu = Some(0);
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

    // TLI 48 bod 4 (pokr.): pozícia PRED touto dávkou rozhoduje za celú dávku
    // naraz — keď appka ešte dobieha starý obsah, ani jeden riadok z nej sa
    // nepočíta za „živé" otvorenie trhu, aj keby sa `AuctionHouseShowItemGoodV2
    // Run` v nej našlo (zvyčajne s hodinovým/denným odstupom starý záznam,
    // dávno zavretý — appka naň pri štarte reagovala snímkou CELEJ obrazovky
    // a hlásením „Na trhu sa nenašla žiadna čitateľná cena", nahlásil Richard
    // 19.10.2026).
    let davka_je_zo_starej_historie =
        stav.povodna_velkost_logu.is_some_and(|p| stav.chvost.pozicia < p);
    let riadky = stav.chvost.priprav_riadky(&bajty);
    crate::stav::uloz_chvost(cesta, &stav.chvost);

    // TLI 61: „posledná aktivita" smie napredovať LEN pri skutočne ŽIVOM
    // obsahu, nie pri dobiehaní starej histórie po štarte appky (rovnaká
    // podmienka ako `trh_otvoreny` vyššie) — inak by appka po dlhej
    // nečinnosti pri prvom čítaní starého logu omylom nahlásila „práve teraz"
    // a automatický reset by sa nikdy nespustil.
    if !davka_je_zo_starej_historie {
        if let Some(conn) = &stav.db {
            let _ = db::nastav_meta(conn, "posledna_aktivita_unix", &teraz.to_string());
        }
    }

    let mut novych = 0;
    let mut uzavrete_behy = Vec::new();
    for riadok in &riadky {
        stav.pripomienka.spracuj_riadok(teraz, klasifikuj(riadok));
        if !davka_je_zo_starej_historie && je_otvorenie_trhu(riadok) {
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

    // TLI 68: čo sa nezapočítalo do dropov a prečo. Ide to do denníka appky
    // (nie do DB a nie do štatistík) — bez toho by hráč videl len „drop sa
    // nezobrazil" a nemal by kde zistiť, že appke po reštarte chýba základ.
    for (stranka, slot, polozka, num, cas) in stav.sledovac.odober_neznamy_zaklad() {
        crate::denik::zapis(
            "tracker",
            &format!(
                "neznámy základ: stránka {stranka} slot {slot} položka {polozka} \
                 Num={num}{} — zmena sa do dropov NEzapočítala (appka tento slot \
                 v tejto session ešte nevidela; sortni inventár v hre)",
                cas.as_ref().map(|c| format!(" ({})", c.text())).unwrap_or_default()
            ),
        );
    }

    // Beh sa do DB ukladá AŽ TERAZ (nie priamo v slučke vyššie) — ceny
    // nájdené v TEJTO ISTEJ dávke (vyššie) sú tak už v cenníku a ocenenie
    // uloženého behu z nich profituje, nie len z cien zo starších dávok.
    for beh in &uzavrete_behy {
        uloz_beh_do_db(stav, beh);
    }
    stav.behy.extend(uzavrete_behy);

    // TLI 55: zachytí skutočný wall-clock čas, odkedy appka vidí AKTUÁLNE
    // bežiaci (nehubový) beh — viď komentár pri `zivy_zaciatok_real` vyššie.
    // Nastavuje sa len RAZ za beh (pri zmene `id`), nie pri každom volaní,
    // inak by sa časovač pri každej dávke vrátil na nulu.
    match stav.sledovac.aktualny() {
        Some(b) if !b.je_hub => {
            if stav.zivy_zaciatok_real.map(|(id, _)| id) != Some(b.id) {
                stav.zivy_zaciatok_real = Some((b.id, teraz));
            }
        }
        _ => stav.zivy_zaciatok_real = None,
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
    /// CURRENT INVENTORY (TLI 47, rozšírené TLI 53) — aktuálny obsah
    /// stránok `STRANKY_BATOHU_SLEDOVANE` (zručnosti, meny vrátane FE,
    /// ostatné materiály; NIE výstroj a NIE sklad), s cenou a hodnotou za
    /// kus. Skryté položky (`Nastavenia::batoh_skryte_polozky`) tu NIE SÚ —
    /// appka ich nezahadzuje nikde inde, len z tohto zoznamu na predaj.
    pub batoh: Vec<PolozkaBatohu>,
    /// Hodnota CELÉHO aktuálneho majetku: FE 1:1 + všetko ostatné z `batoh`
    /// (bez skrytých) ocenené cenníkom, s daňou podľa nastavenia — presne
    /// TITrackova formula (`api/routes/inventory.py`), zhodnotené TLI 53
    /// (zadanie `2026-10-19-1453-mcp.md` bod A). **Do TLI 53 appka namiesto
    /// skutočného FE zostatku pripočítavala `cisty_zisk` (kumulatívny zisk
    /// z odohraných máp) — to bola náhrada za chýbajúci prístup k stránke
    /// 102, Net Worth bol preto systematicky podhodnotený.** `cisty_zisk`
    /// ostáva ako samostatná metrika v `prehlad`, do `net_worth_fe` sa už
    /// nepočíta — sčítanie oboch by teraz FE zostatok započítalo dvakrát.
    pub net_worth_fe: f64,
    /// `true`, keď appka už videla aspoň jeden dump stránky 103 „ostatné"
    /// (hráč v hre použil „Sort"). TLI 53: predtým bolo „Sortni inventár"
    /// upozornenie naviazané na `batoh.is_empty()`, ale `batoh` odteraz
    /// obsahuje aj stránky 101/102, ktoré appka vidí hneď pri prihlásení aj
    /// BEZ Sortu (FE zostatok) — `batoh` by teda takmer nikdy nebol prázdny
    /// a upozornenie na nesortnuté materiály by prestalo fungovať. UI toto
    /// pole používa namiesto `batoh.length === 0` len pre toto konkrétne
    /// upozornenie; `net_worth_fe`/zoznam `batoh` sa ukazujú vždy, bez
    /// ohľadu naň (aj neúplný súčet je lepší než žiadny).
    pub materialy_sortovane: bool,
    /// TLI 56 bod 1 (posledná odrážka): `None`, keď appka v tejto session
    /// VIDELA ŽIVO všetky sledované stránky batohu (101/102/103) — `batoh`
    /// je teda úplne čerstvý. Keď aspoň jedna stránka chýba a appka ju
    /// nahradila poslednou uloženou snímkou z DB (`mnozstva_s_cachou`), nesie
    /// unixový čas TEJ snímky, aby UI vedelo ukázať „stav z HH:MM" namiesto
    /// toho, aby tváril neúplný/starý súčet za aktuálny.
    pub batoh_cache_cas: Option<u64>,
    /// Unixový čas (sekundy), kedy appka naposledy DOKONČILA kolo
    /// synchronizácie cien s partiou (TLI 49 bod 2) — `None`, kým appka ešte
    /// ani raz kolo nedokončila. JS si z neho sám urobí „HH:MM" v lokálnom
    /// čase hráča (`new Date(s * 1000).toLocaleTimeString()`); appka
    /// časové pásmo nikde inde neprevádza a nemá začínať práve tu.
    pub ceny_synchronizovane_o: Option<u64>,
    /// **TLI 68.** `false`, keď appka v tejto session ešte nevidela dump
    /// aspoň jednej stránky batohu (`BagMgr@:InitBagData`) — typicky po
    /// reštarte appky uprostred hrania. Dovtedy sa prvá zmena každého
    /// neznámeho slotu do dropov NEzapočítava (inak by z nej vyšiel celý
    /// stack z batohu) a UI má ukázať „Sortni inventár pre presné dropy".
    pub zaklad_dropov_uplny: bool,
    /// **TLI 68.** Koľko zmien slotu appka od svojho štartu vynechala pre
    /// neznámy základ — koľko dropov teda dnes v štatistikách CHÝBA.
    pub dropov_bez_zakladu: u64,
    /// TLI 61: unixový čas posledného (ručného alebo automatického) resetu
    /// session (`tracker_meta.stat_reset_unix`) — `None`, kým appka ešte
    /// nikdy nič nereportovala. Mini-panel aj hlavné okno z neho ukážu
    /// „session od HH:MM" (rovnaký vzor prevodu na strane JS ako
    /// `ceny_synchronizovane_o` vyššie).
    pub session_od_unix: Option<u64>,
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
    /// Vek ceny v ľudskej podobe (TLI 49 bod 2, rovnaký text ako
    /// `PrehladProUi::vek_cien`) — `None`, keď appka cenu nepozná vôbec.
    pub cena_vek: Option<String>,
}

/// TLI 56 bod 1 (posledná odrážka) a bod 2 (zadanie `2026-10-19-1740-mcp.md`):
/// appka po KAŽDOM reštarte vidí z `Sledovac` len to, čo sa stihlo prečítať
/// odvtedy (FE/materiály hra posiela len pri prihlásení/Sorte, nie znova pri
/// každom riadku) — Current Inventory aj NET WORTH preto po reštarte padali
/// na nulu alebo na čiastočný súčet (napr. videná bola len jedna stránka),
/// hoci appka presne tie isté dáta už mala uložené z PREDOŠLEJ session.
///
/// Pre KAŽDÚ sledovanú stránku samostatne: keď appka v tejto session na danej
/// stránke už niečo videla, použije ŽIVÉ dáta a rovno ich uloží ako novú
/// snímku (nech má z čoho vychádzať pri ĎALŠOM reštarte). Keď stránku ešte
/// nevidela, siahne po poslednej uloženej snímke TEJ ISTEJ stránky z DB.
/// Vracia zlúčenú mapu cez všetky stránky a (keď aspoň jedna stránka použila
/// cache) najstarší čas tej cache — presne to UI ukáže ako „stav z HH:MM".
///
/// DB čítanie/zápis je `#[cfg(not(test))]` — z toho istého dôvodu ako spätné
/// načítanie `behy` v `StavTrackera::default()` vyššie: `cesta_db_suboru()`
/// nie je v testoch izolovaná, je to JEDEN zdieľaný súbor pre všetky testy
/// v builde a desiatky existujúcich testov (napr.
/// `vystroj_na_strance_100_sa_do_current_inventory_nedostane`) spoliehajú na
/// to, že prázdna živá stránka znamená prázdny `batoh`, nie krížovo
/// kontaminovanú snímku z INEJ paralelne bežiacej testovacej funkcie.
/// `uloz_batoh_snapshot`/`nacitaj_batoh_snapshot` majú vlastné jednotkové
/// testy nad izolovanou `otvor_v_pamati()` DB (`core/src/tracker/db.rs`).
#[cfg_attr(test, allow(unused_variables))]
fn mnozstva_s_cachou(stav: &StavTrackera, teraz: u64) -> (BTreeMap<u32, i64>, Option<u64>) {
    let mut spolu = BTreeMap::new();
    #[cfg_attr(test, allow(unused_mut))]
    let mut cache_cas: Option<u64> = None;
    for &stranka in &STRANKY_BATOHU_SLEDOVANE {
        let zive = stav.sledovac.batoh_stranka(stranka);
        if !zive.is_empty() {
            #[cfg(not(test))]
            if let Some(conn) = &stav.db {
                let _ = db::uloz_batoh_snapshot(conn, stranka, &zive, teraz as i64);
            }
            for (polozka, pocet) in zive {
                *spolu.entry(polozka).or_insert(0) += pocet;
            }
            continue;
        }
        #[cfg(not(test))]
        if let Some(conn) = &stav.db {
            if let Ok((ulozene, cas)) = db::nacitaj_batoh_snapshot(conn, stranka) {
                for (polozka, pocet) in ulozene {
                    *spolu.entry(polozka).or_insert(0) += pocet;
                }
                if let Some(c) = cas {
                    let c = c as u64;
                    cache_cas = Some(cache_cas.map_or(c, |existujuci| existujuci.min(c)));
                }
            }
        }
    }
    (spolu, cache_cas)
}

/// Postaví zoznam `batoh` z aktuálneho stavu `Sledovac` (s cache fallbackom
/// z DB, `mnozstva_s_cachou`) + cenník, nad `STRANKY_BATOHU_SLEDOVANE`
/// (TLI 53 — predtým len `STRANKA_BATOHU_OSTATNE`, pozri
/// `PrehladProUi::net_worth_fe`). Skryté položky (`skryte_polozky`) sa
/// v zozname UKÁŽU (appka nič nezahadzuje ticho), len appka ich označí
/// príznakom — UI rozhoduje, či ich pri predajnom zozname vynechá, ale do
/// `net_worth_fe` sa nerátajú (sú to veci, čo si hráč necháva, nie majetok
/// na predaj). Samotná mena `POLOZKA_FE` má cenu vždy 1:1 a daň sa na ňu
/// neuplatňuje — rovnaký princíp ako `core::tracker::ceny::ocen_v_case`,
/// len appka tu navyše potrebuje aj `cena_vek`/`cena_zastarana` pre UI,
/// preto sa nevolá priamo zdieľaná funkcia.
fn aktualny_batoh(stav: &StavTrackera, nastavenie: &Nastavenie, skryte_polozky: &[u32], teraz: u64) -> (Vec<PolozkaBatohu>, f64, Option<u64>) {
    let (mnozstva, cache_cas) = mnozstva_s_cachou(stav, teraz);
    let mut zoznam = Vec::with_capacity(mnozstva.len());
    let mut hodnota_spolu = 0.0;
    for (polozka, pocet) in mnozstva {
        let skryte = skryte_polozky.contains(&polozka);
        let je_fe = polozka == POLOZKA_FE;
        let cena = stav.cennik.cena(polozka);
        let cena_za_kus = if je_fe { Some(1.0) } else { cena.map(|c| c.fe) };
        let hodnota = cena_za_kus.map(|c| {
            let mut h = c * (pocet as f64);
            if nastavenie.dan && !je_fe {
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
            cena_zastarana: !je_fe && cena.is_some() && stav.cennik.je_zastarana(polozka, teraz),
            skryte,
            cena_vek: if je_fe { None } else { cena.map(|c| vek_text(teraz.saturating_sub(c.kedy))) },
        });
    }
    (zoznam, hodnota_spolu, cache_cas)
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
    // TLI 55: `na_zivo` cez `ocen_beh` berie `trvanie_ms` z `Beh::trvanie_ms()`,
    // ktorý vráti `0`, kým mapa beží (chýba `koniec`) — presne preto mini-panel
    // doteraz pri ŽIVEJ mape ukazoval ČAS aj FE/h ako „—", hoci ZISK (na
    // trvaní nezávislý) bol vidieť správne. Pre PRÁVE bežiaci beh sa `0`
    // nahradí odhadom zo skutočného wall-clock štartu (`zivy_zaciatok_real`).
    // `posledna_mapa` nižšie túto náhradu nepotrebuje — tam ide o UŽ UZAVRETÝ
    // beh so skutočným `koniec`, takže `trvanie_ms()` je presné z logu.
    let zivo = stav.sledovac.aktualny().and_then(|b| {
        let mut z = na_zivo(b)?;
        if z.trvanie_ms <= 0 {
            if let Some((id, zaciatok_real)) = stav.zivy_zaciatok_real {
                if id == b.id {
                    z.trvanie_ms = (teraz.saturating_sub(zaciatok_real) * 1000) as i64;
                }
            }
        }
        Some(z)
    });
    // Posledná dokončená mapa — `behy` sú v poradí, v akom ich log priniesol,
    // takže posledný nehubový je ten, z ktorého hráč práve vyšiel.
    let posledna_mapa = stav.behy.iter().rev().find_map(na_zivo);

    let (batoh, hodnota_batohu, batoh_cache_cas) = aktualny_batoh(stav, &nastavenie, skryte_polozky, teraz);
    // TLI 53: NIE `+ suhrn.celkom.cisty_zisk`. `hodnota_batohu` už od TLI 53
    // obsahuje aj skutočný FE zostatok (stránka 102), takže pripočítanie
    // kumulatívneho zisku z máp by ten istý FE zarátalo druhý raz.
    let net_worth_fe = hodnota_batohu;
    let materialy_sortovane = !stav.sledovac.batoh_stranka(STRANKA_BATOHU_OSTATNE).is_empty();

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
        materialy_sortovane,
        zaklad_dropov_uplny: stav.sledovac.zaklad_dropov_uplny(),
        dropov_bez_zakladu: stav.sledovac.pocet_dropov_bez_zakladu(),
        batoh_cache_cas,
        ceny_synchronizovane_o: stav.posledna_synchronizacia_cien,
        session_od_unix: stav
            .db
            .as_ref()
            .and_then(|c| db::precitaj_meta(c, "stat_reset_unix").ok().flatten())
            .and_then(|s| s.parse().ok()),
    }
}

/// Jeden riadok pohľadu „Economy" (TLI 48 bod 3) — plný prehliadač cien.
/// Meno aj ikonu položky appka dopĺňa v JS z `polozky-seed-en.json`/`/ikony/
/// polozka/<id>` (rovnaký zdroj, aký už používa mini-panel) — Rust posiela
/// len to, čo z cenníka a lokálnej DB naozaj vie.
#[derive(Debug, Clone, Serialize)]
pub struct RiadokEconomy {
    pub polozka: u32,
    pub cena_fe: f64,
    pub zdroj: String,
    pub vek_s: u64,
    /// Zmena ceny za posledných 24 h v percentách. `None`, keď appka položku
    /// pred 24 h ešte nepoznala (iná informácia než „0 % zmena").
    pub zmena_24h_percent: Option<f64>,
}

/// Plný zoznam cien pre „Economy" (TLI 48 bod 3) — na rozdiel od
/// `prehlad_pre_ui` (ktorý posiela len vek cien pre položky V BATOHU/behoch)
/// toto je CELÝ cenník, zoradený podľa ID položky (`Cennik::vsetky`).
/// Kategórie (TLI 49 bod 1) sa SEM nepridávajú — appka ich dostane
/// samostatne z `ui/polozky-kategorie-en.json` (JS si ich spojí s `polozka`
/// na klientovi, rovnako ako meno z `polozky-seed-en.json`), takže táto
/// funkcia nemusí vedieť nič o kategóriách a nemení sa pri ich zmene.
pub fn economy_prehlad(stav: &StavTrackera, teraz: u64) -> Vec<RiadokEconomy> {
    let hranica_24h = teraz.saturating_sub(24 * 3600) as i64;
    let ceny_pred_24h = stav
        .db
        .as_ref()
        .and_then(|c| db::ceny_pred(c, hranica_24h).ok())
        .unwrap_or_default();

    stav.cennik
        .vsetky()
        .into_iter()
        .map(|cena| {
            let zmena_24h_percent = ceny_pred_24h.get(&cena.polozka).and_then(|&stara| {
                (stara.abs() > f64::EPSILON).then(|| (cena.fe - stara) / stara * 100.0)
            });
            RiadokEconomy {
                polozka: cena.polozka,
                cena_fe: cena.fe,
                zdroj: cena.zdroj.kod().to_string(),
                vek_s: teraz.saturating_sub(cena.kedy),
                zmena_24h_percent,
            }
        })
        .collect()
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

    let zaciatok = std::time::Instant::now();
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

    // TLI 49 bod 2: appka doteraz nikde nezapisovala, ŽE sa kolo vôbec
    // stalo — len výsledok (`zhrnutie`) v UI, ktorý hráč väčšinou nevidí
    // (beží na pozadí). Footer dashboardu („ceny aktualizované HH:MM")
    // potrebuje vedieť kedy POSLEDNÝ RAZ appka kolo DOKONČILA, nezávisle od
    // toho, či sa pritom niečo zmenilo — preto sa čas zapisuje vždy sem,
    // nie len pri `zmenenych > 0` vyššie.
    {
        let mut t = tracker.0.lock().unwrap();
        t.posledna_synchronizacia_cien = Some(unixovy_cas());
    }
    crate::denik::zapis(
        "ceny",
        &format!(
            "Sync cien: odoslaných {poslanych}, stiahnutých {stiahnutych} (z toho {zmenenych} nových), {} ms",
            zaciatok.elapsed().as_millis()
        ),
    );
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
            let (zapnuty, rucna, pripominat, ceny_obrazovka_zapnute, upload_logov) = {
                let stav = app.state::<Stav>();
                let n = stav.nastavenia.lock().unwrap();
                (
                    n.tracker_zapnuty,
                    n.cesta_logu.clone(),
                    n.pripominat_enable_log,
                    n.ceny_obrazovka_zapnute,
                    n.upload_logov,
                )
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
                    let (novych, pripomienka_udalosti, trh_otvoreny, ukaz_upload, ukaz_sort, sort_problem) = {
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
                        let teraz = unixovy_cas();
                        // Tik beží KAŽDÚ iteráciu (á najviac 1 s), nezávisle od toho,
                        // či `precitaj` našlo nové bajty — lehota sa počíta aj vtedy,
                        // keď sa v logu nedeje ÚPLNE NIČ.
                        t.pripomienka.tik(teraz);

                        // TLI 48 bod 1a/1c — rovnaký stav ako pätička dashboardu
                        // (`vykresliPatickaUploadu`/`vykresliVyzvuNaSort`), len
                        // navyše ako opakovaný toast nad hrou.
                        let hra_bezi = t.sledovac.aktualny().is_some();
                        let upload_problem = upload_logov
                            && (t.posledna_chyba_uploadu.is_some()
                                || (hra_bezi
                                    && t.posledna_kontrola_uploadu.is_some_and(|posledny| {
                                        teraz.saturating_sub(posledny) > UPLOAD_VAROVANIE_PO_S
                                    })));
                        let sort_problem = (hra_bezi || !t.behy.is_empty())
                            && t.sledovac.batoh_stranka(STRANKA_BATOHU_OSTATNE).is_empty();
                        let ukaz_upload = t.varovanie_uploadu.vyhodnot(teraz, upload_problem);
                        let ukaz_sort = t.varovanie_sortu.vyhodnot(teraz, sort_problem);

                        (
                            novych,
                            t.pripomienka.odober_udalosti(),
                            t.odober_trh_otvoreny(),
                            ukaz_upload,
                            ukaz_sort,
                            sort_problem,
                        )
                    };
                    if novych > 0 {
                        let _ = app.emit(UDALOST_ZMENA, novych);
                    }
                    for udalost in pripomienka_udalosti {
                        oznam_pripomienku(&app, udalost);
                    }
                    // Obe sú nezávislé od Enable-Log toastu vyššie aj od seba
                    // navzájom — zdieľajú jedno okno (`ukaz_pripomienku_toast`,
                    // „novší toast vyhráva"), rovnaký vzor ako ceny/chyba/info.
                    // Dĺžka presahuje `OPAKOVAT_PO_S`, aby stihla ďalšie obnovenie
                    // skôr, než by sama zhasla.
                    if ukaz_sort {
                        crate::ukaz_pripomienku_toast(
                            &app,
                            "Sortni inventár v hre",
                            "upozornenie",
                            tli_overlay_core::tracker::varovania::OPAKOVAT_PO_S + 10,
                        );
                    } else if !sort_problem {
                        // TLI 57: hráč práve sortol (prvý `InitBagData` po
                        // probléme) — toast má zmiznúť HNEĎ, nie čakať na
                        // vlastný (teraz oveľa dlhší) časovač.
                        crate::skry_pripomienku_ak_text(&app, "Sortni inventár v hre");
                    }
                    if ukaz_upload {
                        crate::ukaz_pripomienku_toast(
                            &app,
                            "Log sa neposiela na server",
                            "upozornenie",
                            tli_overlay_core::tracker::varovania::OPAKOVAT_PO_S + 10,
                        );
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

    // ─── TLI 48 bod 4 — auto-trigger ceny len z NOVÝCH riadkov, nie z histórie ──

    #[test]
    fn historicke_otvorenie_trhu_pri_prvom_citani_nenastavi_trh_otvoreny() {
        // Presne nahlásený bug (19.10.2026): appka sa spustí, log na disku už
        // obsahuje staré (dávno zavreté) otvorenie trhu — to sa pri prvom
        // dočítaní NESMIE brať ako práve prebiehajúca udalosť.
        let cesta = docasny_subor();
        napis(
            &cesta,
            "[2026.10.14-22.23.45:283][305]TLLua: Display: [Game] AuctionHouseShowItemGoodV2 Run\n",
        );

        let mut stav = StavTrackera::default();
        precitaj(&mut stav, &cesta).unwrap();

        assert!(!stav.odober_trh_otvoreny(), "historický riadok z prvého čítania sa nemá počítať");
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn otvorenie_trhu_pridane_po_prvom_citani_je_naozaj_zive() {
        let cesta = docasny_subor();
        napis(&cesta, "nič užitočné tu nie je, len výplň.\n");

        let mut stav = StavTrackera::default();
        precitaj(&mut stav, &cesta).unwrap(); // appka "dobehla" existujúci obsah

        pripis(
            &cesta,
            "[2026.10.19-08.00.00:000][1]TLLua: Display: [Game] AuctionHouseShowItemGoodV2 Run\n",
        );
        precitaj(&mut stav, &cesta).unwrap();

        assert!(stav.odober_trh_otvoreny(), "riadok pridaný PO otvorení appky je živá udalosť");
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn historicke_otvorenie_trhu_ostava_mrtve_aj_naprieč_viacerymi_davkami_dobiehania() {
        // TLI 49 bod 4: Richardov skutočný log má ~90 MB — appka ho pri
        // prvom otvorení doháňa po `DAVKA_B` (2 MB) kúskoch, NIE naraz.
        // `historicke_otvorenie_trhu_pri_prvom_citani_nenastavi_trh_otvoreny`
        // (vyššie) overuje len JEDNO volanie `precitaj` nad malým súborom —
        // toto overuje, že historický riadok ostane mŕtvy aj vtedy, keď sa
        // (desiatky volaní `precitaj` za sebou) k nemu appka dostane až
        // v neskoršej dávke, nie hneď v prvej.
        let cesta = docasny_subor();
        let vyplnovy_riadok = "x".repeat(200) + "\n"; // ~200 B na riadok
        let riadkov_pred = (DAVKA_B as usize / vyplnovy_riadok.len()) * 3; // cez tri dávky výplne
        let mut obsah = vyplnovy_riadok.repeat(riadkov_pred);
        obsah.push_str(
            "[2026.10.14-22.23.45:283][305]TLLua: Display: [Game] AuctionHouseShowItemGoodV2 Run\n",
        );
        obsah.push_str(&vyplnovy_riadok.repeat(10));
        napis(&cesta, &obsah);

        let mut stav = StavTrackera::default();
        let povodna_velkost = std::fs::metadata(&cesta).unwrap().len();
        let mut precitane_spolu = 0u64;
        // Volaj `precitaj` v slučke presne tak, ako to robí vlákno appky na
        // pozadí — kým sa pozícia nedobehne na pôvodnú veľkosť súboru.
        loop {
            precitaj(&mut stav, &cesta).unwrap();
            assert!(
                !stav.odober_trh_otvoreny(),
                "historický riadok sa nesmie počítať za živý v ŽIADNEJ dávke dobiehania"
            );
            let pozicia = stav.chvost.pozicia;
            assert!(pozicia >= precitane_spolu, "pozícia sa nemá vrátiť späť");
            precitane_spolu = pozicia;
            if pozicia >= povodna_velkost {
                break;
            }
        }
        assert!(precitane_spolu > DAVKA_B, "test nemá zmysel, ak sa zmestil do jednej dávky");

        // Až TERAZ pridaný riadok (po úplnom dobehnutí) je naozaj živý.
        pripis(
            &cesta,
            "[2026.10.19-09.54.00:000][1]TLLua: Display: [Game] AuctionHouseShowItemGoodV2 Run\n",
        );
        precitaj(&mut stav, &cesta).unwrap();
        assert!(stav.odober_trh_otvoreny(), "riadok PO úplnom dobehnutí histórie je živá udalosť");

        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn rotacia_logu_posunie_hranicu_na_zaciatok_noveho_suboru() {
        // Po reštarte hry je nový (menší) súbor od prvého riadku živý —
        // appka naň nemá čakať, kým "dobehne" jeho vlastnú (novú) veľkosť.
        let cesta = docasny_subor();
        napis(&cesta, &"vyplnovy riadok dlhsi nez novy subor\n".repeat(5));

        let mut stav = StavTrackera::default();
        precitaj(&mut stav, &cesta).unwrap();

        napis(
            &cesta,
            "[2026.10.19-08.00.00:000][1]TLLua: Display: [Game] AuctionHouseShowItemGoodV2 Run\n",
        );
        precitaj(&mut stav, &cesta).unwrap();

        assert!(stav.odober_trh_otvoreny(), "riadok hneď v novom (rotovanom) súbore je živý");
        let _ = std::fs::remove_file(&cesta);
    }

    // ─── TLI 48 bod 3 — Economy: plný prehliadač cien ──────────────────────

    #[test]
    fn economy_prehlad_vracia_cenu_zdroj_vek_a_zmenu_24h() {
        use tli_overlay_core::tracker::ceny::{Cena, Zdroj};

        let mut stav = StavTrackera::default();
        let conn = db::otvor_v_pamati().unwrap();
        let teraz = 100_000u64;
        db::uloz_cenu(
            &conn,
            &ZaznamCeny {
                polozka: 1001,
                cena_fe: 10.0,
                median_fe: None,
                pocet_ponuk: None,
                sezona: None,
                zdroj: "vlastny_trh".into(),
                cas: None,
                zapisane: (teraz - 25 * 3600) as i64,
            },
        )
        .unwrap();
        stav.db = Some(conn);
        stav.cennik.vloz_v_case(
            Cena { polozka: 1001, fe: 12.0, zdroj: Zdroj::Titrack, kedy: teraz, median: None, pocet_ponuk: 0, zdrojov: 1 },
            teraz,
        );

        let riadky = economy_prehlad(&stav, teraz);
        assert_eq!(riadky.len(), 1);
        let r = &riadky[0];
        assert_eq!(r.polozka, 1001);
        assert_eq!(r.cena_fe, 12.0);
        assert_eq!(r.zdroj, "titrack");
        assert_eq!(r.vek_s, 0);
        let zmena = r.zmena_24h_percent.expect("historická cena existuje, zmena sa má vypočítať");
        assert!((zmena - 20.0).abs() < 0.001, "(12-10)/10*100 = 20%, bolo {zmena}");
    }

    #[test]
    fn economy_prehlad_bez_historie_nema_zmenu_24h() {
        use tli_overlay_core::tracker::ceny::{Cena, Zdroj};

        let mut stav = StavTrackera::default();
        stav.db = Some(db::otvor_v_pamati().unwrap());
        let teraz = 100_000u64;
        stav.cennik.vloz_v_case(
            Cena { polozka: 2002, fe: 5.0, zdroj: Zdroj::VlastnyTrh, kedy: teraz, median: None, pocet_ponuk: 0, zdrojov: 1 },
            teraz,
        );

        let riadky = economy_prehlad(&stav, teraz);
        assert_eq!(riadky.len(), 1);
        assert_eq!(riadky[0].zmena_24h_percent, None, "appka pred 24 h položku ešte nepoznala");
    }

    // ─── TLI 49 bod 2 — lokálna DB si pri cenách od partie/titracku pamätá,
    // KEDY ich TÁTO appka stiahla, nie kedy ich niekto v partii vyhľadal ──

    #[test]
    fn prijmi_ceny_partie_zapise_do_db_cas_stiahnutia_appkou_nie_cas_ceny() {
        use tli_overlay_core::tracker::ceny_sync::{AgregatPartie, CenaPartie};

        let mut stav = StavTrackera::default();
        stav.db = Some(db::otvor_v_pamati().unwrap());

        // Cena vznikla (niekto v partii ju vyhľadal) dávno predtým, než ju
        // táto appka vôbec stiahla — presne scenár, pri ktorom Richard na
        // vlastnom PC videl v DB staré/nečitateľné „kedy".
        let cas_vzniku_ceny = 1_000u64;
        let cas_stiahnutia_appkou = 50_000u64;
        let agregat = AgregatPartie {
            ok: true,
            cas: cas_stiahnutia_appkou as i64,
            ceny: vec![CenaPartie {
                polozka: 777,
                fe: 42.0,
                zdrojov: 2,
                pocet_ponuk: 5,
                kedy: cas_vzniku_ceny as i64,
                istota: "dobra".to_string(),
                zdroj: "titrack".to_string(),
            }],
        };

        stav.prijmi_ceny_partie(&agregat, cas_stiahnutia_appkou);

        let conn = stav.db.as_ref().unwrap();
        let (zapisane, cas): (i64, Option<String>) = conn
            .query_row(
                "SELECT zapisane, cas FROM ceny WHERE config_base_id = 777",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            zapisane, cas_stiahnutia_appkou as i64,
            "zapisane má byť čas STIAHNUTIA appkou, nie čas vzniku ceny u partie/titracku"
        );
        assert_eq!(cas, None, "herný čas z logu appka pre cenu od partie/titracku nepozná");

        // Cennik (freshness/dôveryhodnosť) sa naopak riadi časom SAMOTNEJ
        // ceny — to je nezávislý údaj a táto oprava ho nemá meniť.
        assert_eq!(stav.cennik.cena(777).unwrap().kedy, cas_vzniku_ceny);
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

    // TLI 53 (`2026-10-19-1453-mcp.md` bod A) — appka do tejto chvíle čítala
    // do CURRENT INVENTORY/NET WORTH len stránku 103 ("ostatné"), hoci FE aj
    // ostatné meny/crafting materiály (embery, memory thready, divinity,
    // wedge) ležia v reálnom logu na stránke 102 — overené priamo na
    // Richardovom uploadnutom `UE_game.log`. Nasledujúce testy pokrývajú
    // opravu: FE sa počíta 1:1 bez dane, ostatné položky stránky 102 sa
    // cenia cenníkom ako doteraz stránka 103, a `net_worth_fe` už nepripočíta
    // `cisty_zisk` navyše (to by FE zarátalo dvakrát).

    #[test]
    fn current_inventory_zahrna_fe_zo_stranky_102_oceneny_1_1_bez_dane() {
        let mut stav = StavTrackera::default();
        stav.sledovac.riadok("BagMgr@:InitBagData PageId = 102 SlotId = 1 ConfigBaseId = 100300 Num = 12000");

        let p = prehlad_pre_ui(&stav, true, true, &[]);
        let fe = p.batoh.iter().find(|r| r.config_base_id == POLOZKA_FE).expect("FE musí byť v CURRENT INVENTORY");
        assert_eq!(fe.mnozstvo, 12000);
        assert_eq!(fe.cena_za_kus_fe, Some(1.0));
        assert_eq!(fe.hodnota_fe, Some(12000.0));
        assert!(!fe.cena_zastarana);
        assert_eq!(p.net_worth_fe, 12000.0);
    }

    #[test]
    fn current_inventory_zahrna_komoditu_zo_stranky_102_s_cenou_z_cennika() {
        let mut stav = StavTrackera::default();
        // 200003 = "Fine Ember" v reálnom logu tiež na stránke 102.
        stav.sledovac.riadok("BagMgr@:InitBagData PageId = 102 SlotId = 2 ConfigBaseId = 200003 Num = 10");
        stav.cennik.vloz(Cena::vlastna(200003, 5.0, 5.0, 3, unixovy_cas()));

        let p = prehlad_pre_ui(&stav, true, true, &[]);
        let ember = p.batoh.iter().find(|r| r.config_base_id == 200003).expect("embery musia byť v CURRENT INVENTORY");
        assert_eq!(ember.mnozstvo, 10);
        assert_eq!(ember.cena_za_kus_fe, Some(5.0));
        // Daň (87,5 %) sa na bežnú komoditu uplatní, na FE nie.
        assert_eq!(ember.hodnota_fe, Some(10.0 * 5.0 * 0.875));
        assert_eq!(p.net_worth_fe, 10.0 * 5.0 * 0.875);
    }

    #[test]
    fn vystroj_na_strance_100_sa_do_current_inventory_nedostane() {
        let mut stav = StavTrackera::default();
        stav.sledovac.riadok("BagMgr@:InitBagData PageId = 100 SlotId = 1 ConfigBaseId = 8125 Num = 1");

        let p = prehlad_pre_ui(&stav, true, false, &[]);
        assert!(p.batoh.is_empty());
        assert_eq!(p.net_worth_fe, 0.0);
    }

    #[test]
    fn net_worth_sa_uz_nescitava_s_cistym_ziskom_aby_nedoslo_k_dvojitemu_zarataniu_fe() {
        // Pred TLI 53 appka počítala `net_worth_fe = hodnota_batohu + cisty_zisk`
        // a hodnota_batohu nikdy nemala FE (čítala len stránku 103) — teraz, keď
        // hodnota_batohu FE obsahuje, by sčítanie oboch FE zarátalo dvakrát.
        let mut stav = StavTrackera::default();
        // Aktuálny FE zostatok (stránka 102) — 500.
        stav.sledovac.riadok("BagMgr@:InitBagData PageId = 102 SlotId = 1 ConfigBaseId = 100300 Num = 500");
        // Odohraný beh s vlastným čistým ziskom 300 FE, nezávislý od aktuálneho
        // zostatku — keby `net_worth_fe` ešte stále pripočítaval `cisty_zisk`,
        // vyšlo by 800, nie 500.
        let mut beh = prazdny_beh_pre_test("Mapa1");
        beh.dropy.insert(POLOZKA_FE, 300);
        stav.behy.push(beh);

        let p = prehlad_pre_ui(&stav, true, false, &[]);
        assert_eq!(p.prehlad.celkom.cisty_zisk, 300.0, "test predpokladá nenulový cisty_zisk, inak nič nedokazuje");
        assert_eq!(p.net_worth_fe, 500.0);
    }

    // ─── TLI 56 (`2026-10-19-1740-mcp.md`): „Reset Stats" prežíva reštart ──
    //
    // `StavTrackera::default()`/`mnozstva_s_cachou` samotné spätné
    // načítanie z DB v tomto builde (`#[cfg(test)]`) zámerne nespúšťajú —
    // dôvod je v komentári pri oboch miestach (zdieľaný neizolovaný súbor
    // DB naprieč všetkými testami tohto crate). Čo je testovateľné aj tu:
    // že `resetuj_zive_staty` naozaj zapíše marker do DB, ktorú appka
    // reálne má otvorenú — zvyšok (že toto `tracker_meta.stat_reset_unix`
    // orezáva `nacitaj_zive_behy`) už overujú testy v `core/src/tracker/db.rs`.

    #[test]
    fn reset_zivych_statov_zapise_marker_do_db_ked_je_otvorena() {
        let mut stav = StavTrackera::default();
        stav.db = Some(db::otvor_v_pamati().unwrap());
        let mut beh = prazdny_beh_pre_test("Mapa1");
        beh.dropy.insert(POLOZKA_FE, 10);
        stav.behy.push(beh);

        stav.resetuj_zive_staty();

        assert!(stav.behy.is_empty(), "pamäťová časť resetu sa nemení");
        let conn = stav.db.as_ref().unwrap();
        let marker = db::precitaj_meta(conn, "stat_reset_unix").unwrap();
        assert!(marker.is_some(), "reset musí prežiť reštart appky, nie len vyprázdniť pamäť");
        assert!(marker.unwrap().parse::<u64>().is_ok(), "marker má byť unixový čas ako text");
    }

    #[test]
    fn reset_zivych_statov_nepada_bez_otvorenej_db() {
        // `StavTrackera::default()` v builde pre testy `db` field aj tak
        // nastaví (otvorí reálny súbor) — explicitný `None` tu simuluje
        // appku, ktorej sa DB nepodarilo otvoriť vôbec (`chyba_db`).
        let mut stav = StavTrackera::default();
        stav.db = None;
        stav.behy.push(prazdny_beh_pre_test("Mapa1"));
        stav.resetuj_zive_staty();
        assert!(stav.behy.is_empty());
    }

    // ─── TLI 68 (`2026-10-20-1213-mcp.md`): základ dropov po reštarte ──────
    //
    // Bod b) zadania: stav v DB s Voidlands Beacon 48, reštart appky, log
    // zmení slot na 49 → drop je ×0 (neznámy základ), alebo ×1, ak základ
    // v session existoval. NIKDY ×49.

    #[test]
    fn tli68_snimka_batohu_z_db_nie_je_zakladom_dropov() {
        // „Reštartovaná" appka: `Sledovac` je čistý, DB má snímku batohu
        // z predošlej session (Voidlands Beacon 48).
        let mut stav = StavTrackera::default();
        let conn = db::otvor_v_pamati().unwrap();
        let mut snimka = std::collections::BTreeMap::new();
        snimka.insert(400033u32, 48i64);
        db::uloz_batoh_snapshot(&conn, STRANKA_BATOHU_OSTATNE, &snimka, 1_000).unwrap();
        stav.db = Some(conn);

        stav.sledovac.riadok(
            "[2026.10.20-10.10.00:000][1]GameLog: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Art/Maps/01SD/SD_ShengHuoLing0203/SD_ShengHuoLing0203",
        );
        stav.sledovac.riadok("[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] ItemChange@ ProtoName=PickItem start");
        stav.sledovac.riadok(
            "[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] BagMgr@:Modfy BagItem PageId = 103 SlotId = 3 ConfigBaseId = 400033 Num = 49",
        );
        stav.sledovac.riadok("[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] ItemChange@ ProtoName=PickItem end");

        let beh = stav.sledovac.aktualny().unwrap();
        assert_eq!(
            beh.dropy.get(&400033),
            None,
            "snímka z DB slúži LEN na Inventory/Net Worth — ako základ dropov sa použiť nesmie, a bez základu sa nepočíta nič (nikdy ×49)"
        );
        let p = prehlad_pre_ui(&stav, true, false, &[]);
        assert!(!p.zaklad_dropov_uplny, "UI má ukázať výzvu na Sort");
        assert_eq!(p.dropov_bez_zakladu, 1);
    }

    #[test]
    fn tli68_so_zakladom_zo_session_je_to_drop_jeden() {
        let mut stav = StavTrackera::default();
        // Dump stránky v TEJTO session (prihlásenie alebo Sort) — 48 kusov.
        stav.sledovac.riadok(
            "[2026.10.20-10.00.00:000][1]GameLog: Display: [Game] BagMgr@:InitBagData PageId = 103 SlotId = 3 ConfigBaseId = 400033 Num = 48",
        );
        stav.sledovac.riadok(
            "[2026.10.20-10.10.00:000][1]GameLog: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Art/Maps/01SD/SD_ShengHuoLing0203/SD_ShengHuoLing0203",
        );
        stav.sledovac.riadok("[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] ItemChange@ ProtoName=PickItem start");
        stav.sledovac.riadok(
            "[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] BagMgr@:Modfy BagItem PageId = 103 SlotId = 3 ConfigBaseId = 400033 Num = 49",
        );
        stav.sledovac.riadok("[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] ItemChange@ ProtoName=PickItem end");

        let beh = stav.sledovac.aktualny().unwrap();
        assert_eq!(beh.dropy.get(&400033), Some(&1), "48 -> 49 je presne jeden kus");
    }

    // Bod c) zadania: reset session (ručný aj automatický) nuluje LEN
    // štatistiky session — nikdy nie stav batohu ani základ dropov.

    #[test]
    fn tli68_reset_session_nemeni_zaklad_dropov() {
        let mut stav = StavTrackera::default();
        stav.db = Some(db::otvor_v_pamati().unwrap());
        for stranka in [100u32, 101, 102, 103] {
            stav.sledovac.riadok(&format!(
                "[2026.10.20-10.00.00:000][1]GameLog: Display: [Game] BagMgr@:InitBagData PageId = {stranka} SlotId = 3 ConfigBaseId = 400033 Num = 48"
            ));
        }
        stav.sledovac.riadok(
            "[2026.10.20-10.10.00:000][1]GameLog: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Art/Maps/01SD/SD_ShengHuoLing0203/SD_ShengHuoLing0203",
        );
        assert!(stav.sledovac.zaklad_dropov_uplny());

        // Uprostred mapy: ručný aj automatický reset session.
        stav.resetuj_zive_staty();
        db::nastav_meta(stav.db.as_ref().unwrap(), "posledna_aktivita_unix", "1000").unwrap();
        stav.behy.push(prazdny_beh_pre_test("Mapa1"));
        stav.over_automaticky_reset_session(1000 + SESSION_NECINNOST_S + 1);

        assert!(
            stav.sledovac.zaklad_dropov_uplny(),
            "reset session sa nesmie dotknúť základu dropov"
        );
        assert_eq!(
            stav.sledovac.batoh_stranka(STRANKA_BATOHU_OSTATNE).get(&400033),
            Some(&48),
            "reset session sa nesmie dotknúť stavu batohu"
        );

        // A drop po resete je stále ×1, nie ×49.
        stav.sledovac.riadok("[2026.10.20-10.12.00:000][1]GameLog: Display: [Game] ItemChange@ ProtoName=PickItem start");
        stav.sledovac.riadok(
            "[2026.10.20-10.12.00:000][1]GameLog: Display: [Game] BagMgr@:Modfy BagItem PageId = 103 SlotId = 3 ConfigBaseId = 400033 Num = 49",
        );
        stav.sledovac.riadok("[2026.10.20-10.12.00:000][1]GameLog: Display: [Game] ItemChange@ ProtoName=PickItem end");
        let beh = stav.sledovac.aktualny().unwrap();
        assert_eq!(beh.dropy.get(&400033), Some(&1));
    }

    // ─── TLI 61: automatický reset session po 30 min nečinnosti ────────────

    #[test]
    fn automaticky_reset_sa_spusti_ked_posledna_aktivita_je_stara() {
        let mut stav = StavTrackera::default();
        stav.db = Some(db::otvor_v_pamati().unwrap());
        stav.behy.push(prazdny_beh_pre_test("Mapa1"));
        let conn = stav.db.as_ref().unwrap();
        db::nastav_meta(conn, "posledna_aktivita_unix", "1000").unwrap();

        stav.over_automaticky_reset_session(1000 + SESSION_NECINNOST_S + 1);

        assert!(stav.behy.is_empty(), "po 30+ min nečinnosti sa živé behy majú vyprázdniť presne ako pri ručnom Reset Stats");
        let conn = stav.db.as_ref().unwrap();
        let marker = db::precitaj_meta(conn, "stat_reset_unix").unwrap();
        assert!(marker.is_some(), "automatický reset musí zapísať stat_reset_unix rovnako ako ručný");
    }

    #[test]
    fn automaticky_reset_sa_nespusti_pocas_suvislej_hry() {
        let mut stav = StavTrackera::default();
        stav.db = Some(db::otvor_v_pamati().unwrap());
        stav.behy.push(prazdny_beh_pre_test("Mapa1"));
        let conn = stav.db.as_ref().unwrap();
        db::nastav_meta(conn, "posledna_aktivita_unix", "1000").unwrap();

        // Len 5 minút od poslednej aktivity — session musí ostať netknutá.
        stav.over_automaticky_reset_session(1000 + 300);

        assert_eq!(stav.behy.len(), 1, "krátka medzera medzi tikmi nesmie vynulovať bežiacu session");
    }

    #[test]
    fn automaticky_reset_bez_markera_v_db_prebehne_len_raz() {
        // Čerstvá appka (nikdy nereportovala `posledna_aktivita_unix`) sa
        // nemá správať ako nekonečná slučka resetov na každý tik — po prvom
        // vyprázdnení `behy` je `necinna_pridlho && !self.behy.is_empty()`
        // vždy false, kým nepribudne nová mapa.
        let mut stav = StavTrackera::default();
        stav.db = Some(db::otvor_v_pamati().unwrap());
        stav.behy.push(prazdny_beh_pre_test("Mapa1"));

        stav.over_automaticky_reset_session(50_000);
        assert!(stav.behy.is_empty());

        // Druhé volanie o pár sekúnd neskôr nemá čo robiť (žiadny panic,
        // žiadny ďalší zápis potrebný) — len si overíme, že to naozaj nič
        // nerozbije a `behy` ostáva prázdne.
        stav.over_automaticky_reset_session(50_005);
        assert!(stav.behy.is_empty());
    }

    // ─── TLI 55: živý čas a FE/h počas bežiacej mapy ────────────────────────

    /// Reálna sekvencia riadkov z Richardovho servera (19.10.2026, mapa
    /// `SD_DuiLongJuQiang211` zo screenshotu v zadaní `2026-10-19-1715-mcp.md`)
    /// — otvorenie mapy chartom (`Spv3Open`) a prechod do nej (`LevelMgr@` →
    /// `SceneLevelMgr@ OpenMainWorld END!`), bez uzavretia (mapa stále beží).
    fn napis_vstup_do_mapy(cesta: &Path, zona: &str) {
        pripis(cesta, "[2026.10.19-16.45.52:424][914]TLLua: Display: [Game] ItemChange@ ProtoName=Spv3Open start\n");
        pripis(cesta, "[2026.10.19-16.45.52:424][914]TLLua: Display: [Game] ItemChange@ ProtoName=Spv3Open end\n");
        pripis(cesta, "[2026.10.19-16.45.52:472][929]TLShipping: Display: [Game] LevelMgr@ LevelUid, LevelType, LevelId = 1091202 3 5026\n");
        pripis(cesta, &format!(
            "[2026.10.19-16.45.53:738][202]TLLua: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = {zona}\n"
        ));
    }

    #[test]
    fn ziva_mapa_bez_konca_ukazuje_bezici_cas_namiesto_nuly() {
        // Pred opravou: `Beh::trvanie_ms()` vracia `None`/`0`, kým mapa beží
        // (chýba `koniec`) — presne to Richard nahlásil ako ČAS a FE/h „—"
        // počas mapy, hoci ZISK (na trvaní nezávislý) bol vidieť správne.
        let cesta = docasny_subor();
        napis(&cesta, "");
        napis_vstup_do_mapy(&cesta, "/Game/Art/Maps/01SD/SD_DuiLongJuQiang211/SD_DuiLongJuQiang211");

        let mut stav = StavTrackera::default();
        precitaj(&mut stav, &cesta).unwrap();

        let zivo = prehlad_pre_ui(&stav, true, false, &[]).zivo.expect("mapa má bežať");
        assert!(zivo.zona.contains("SD_DuiLongJuQiang211"));

        // Simuluje plynutie 5 sekúnd BEZ nového čítania logu — presne to, čo
        // by malo v appke robiť lokálne tikanie po sekundách medzi dvoma
        // pollami backendu.
        let id = stav.sledovac.aktualny().unwrap().id;
        stav.zivy_zaciatok_real = Some((id, unixovy_cas().saturating_sub(5)));
        let zivo2 = prehlad_pre_ui(&stav, true, false, &[]).zivo.expect("mapa stále beží");
        assert!(zivo2.trvanie_ms >= 5000, "živý čas mal narásť na aspoň 5000 ms, je {}", zivo2.trvanie_ms);

        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn ziva_mapa_zaciatok_real_sa_nemeni_pri_dalsom_citani_tej_istej_mapy() {
        // Keby sa `zivy_zaciatok_real` prepisoval pri KAŽDOM volaní `precitaj`
        // (nielen pri zmene bežiaceho behu), časovač by sa vracal na nulu pri
        // každom pricitaní nových riadkov tej istej ešte otvorenej mapy.
        let cesta = docasny_subor();
        napis(&cesta, "");
        napis_vstup_do_mapy(&cesta, "/Game/Art/Maps/01SD/SD_DuiLongJuQiang211/SD_DuiLongJuQiang211");

        let mut stav = StavTrackera::default();
        precitaj(&mut stav, &cesta).unwrap();
        let id = stav.sledovac.aktualny().unwrap().id;
        // O čosi „staršie", aby sa dal odlíšiť od nového zachytenia. Hodnota
        // sa drží v premennej (NIE prepočítava znova pri assercii) — dva
        // samostatné volania `unixovy_cas()` oddelené reálnym behom testu by
        // sa mohli (zriedka) líšiť o sekundu a test by bol krehký.
        let ocakavany_cas = unixovy_cas().saturating_sub(42);
        stav.zivy_zaciatok_real = Some((id, ocakavany_cas));

        // Ďalší riadok (napr. drop) v TEJ ISTEJ, stále otvorenej mape.
        pripis(&cesta, "[2026.10.19-16.46.10:001][914]TLLua: Display: [Game] BagMgr@:Modfy BagItem PageId = 100 SlotId=5 ConfigBaseId=12345 Num=1\n");
        precitaj(&mut stav, &cesta).unwrap();

        assert_eq!(
            stav.zivy_zaciatok_real,
            Some((id, ocakavany_cas)),
            "zachytený začiatok sa nesmie posunúť, kým beží tá istá mapa"
        );
        let _ = std::fs::remove_file(&cesta);
    }

    #[test]
    fn uzavreta_mapa_pouziva_skutocne_trvanie_z_logu_nie_zivy_odhad() {
        // Uzavretý beh (má aj `koniec`) musí ďalej ukazovať presné trvanie
        // z logu — živý odhad (`zivy_zaciatok_real`) je len náhrada PRE
        // BEŽIACU mapu a nesmie prepísať už hotové číslo v `posledna_mapa`.
        let cesta = docasny_subor();
        napis(&cesta, "");
        napis_vstup_do_mapy(&cesta, "/Game/Art/Maps/01SD/SD_GeBuLinShanZhai/SD_GeBuLinShanZhai");
        // Návrat do hubu o 60 s neskôr uzavrie beh s presným `trvanie_ms`.
        pripis(&cesta, "[2026.10.19-16.46.53:738][202]TLLua: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Art/Maps/Town/Town\n");

        let mut stav = StavTrackera::default();
        precitaj(&mut stav, &cesta).unwrap();

        let p = prehlad_pre_ui(&stav, true, false, &[]);
        assert!(p.zivo.is_none(), "v hube nemá bežať žiadna mapa");
        let posledna = p.posledna_mapa.expect("posledná mapa musí ostať");
        assert_eq!(posledna.trvanie_ms, 60_000, "trvanie uzavretej mapy sa počíta z logu (16:45:53 -> 16:46:53)");

        let _ = std::fs::remove_file(&cesta);
    }
}
