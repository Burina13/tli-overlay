//! Spätný import celého textu logu (aj záložných `UE_game-backup-*.log`) do
//! SQLite databázy (TLI 13 §1 „Načítať staré logy"). Na rozdiel od `tracker.rs`
//! v `src-tauri` (ktorý číta prírastkovo od poslednej pozície) tu ide o CELÝ
//! obsah súboru naraz — appka ho vie prečítať a dekódovať (`src-tauri` rieši
//! samotné otvorenie/čítanie disku, lebo to v `core` zámerne nie je), tento
//! modul z neho spraví hotové `Beh` záznamy a uloží ich.
//!
//! Dedup je DVOJVRSTVOVÝ: `db::uloz_beh`/`uloz_cenu`/`uloz_zostatok`/`uloz_raw`
//! sú idempotentné samy osebe (prirodzený kľúč / `hash_riadku`), takže
//! opätovné spustenie nad TÝM ISTÝM súborom (alebo nad súbormi, ktoré sa
//! prekrývajú — rotovaný log a jeho záloha majú často spoločný kus histórie)
//! nič nezdvojí. Táto vrstva navyše počíta, koľko toho bolo nové a koľko sa
//! preskočilo, aby appka mala čo ukázať v hlásení o importe.

use rusqlite::Connection;

use super::ceny::{Cena, Cennik, Nastavenie};
use super::db::{self, OceneneHodnoty, ZaznamCeny};
use super::log::dekoduj;
use super::stav::{HracSnimka, Sledovac};

/// Výsledok jedného behu importu (jeden súbor alebo jedno volanie
/// `importuj_text`).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct VysledokImportu {
    pub pocet_riadkov: usize,
    pub pocet_novych_behov: usize,
    pub pocet_preskocenych_behov: usize,
    pub pocet_monster: usize,
    pub pocet_cien: usize,
    pub pocet_zostatkov: usize,
    pub neznamych_riadkov: u64,
    /// Koľko behov dostalo hodnotu až v druhom prechode (TLI 20).
    pub pocet_precenenych_behov: usize,
}

impl VysledokImportu {
    fn pripocitaj(&mut self, iny: VysledokImportu) {
        self.pocet_riadkov += iny.pocet_riadkov;
        self.pocet_novych_behov += iny.pocet_novych_behov;
        self.pocet_preskocenych_behov += iny.pocet_preskocenych_behov;
        self.pocet_monster += iny.pocet_monster;
        self.pocet_cien += iny.pocet_cien;
        self.pocet_zostatkov += iny.pocet_zostatkov;
        self.neznamych_riadkov += iny.neznamych_riadkov;
        self.pocet_precenenych_behov += iny.pocet_precenenych_behov;
    }
}

/// Spracuje celý text logu (výsledok `log::dekoduj` nad bajtami súboru) cez
/// nový `Sledovac` od nuly a uloží všetko do `conn`. Identita hráča
/// (`player_id`/`season_id`/`hero_id`) sa priebežne aktualizuje podľa
/// `+player+...` blokov nájdených V TOMTO SÚBORE — behy pred prvým takým
/// blokom (bežné na začiatku session, log-format.md §9) sa uložia s
/// identitou `HracSnimka::default()` (teda prázdnym, nie neznámym, kľúčom —
/// `db::uloz_beh` to rieši).
pub fn importuj_text(conn: &mut Connection, text: &str) -> rusqlite::Result<VysledokImportu> {
    let mut sledovac = Sledovac::novy();
    let mut vysledok = VysledokImportu::default();
    let mut identita = HracSnimka::default();
    // ID behov vložených V TOMTO importe — v druhom prechode sa im dopočíta
    // hodnota z cien, ktoré sa našli až neskôr v tom istom súbore.
    let mut nove_behy: Vec<i64> = Vec::new();

    for riadok in text.lines() {
        if let Some(beh) = sledovac.riadok(riadok) {
            if let Some(novsia) = sledovac.hrac_snimka() {
                identita = novsia.clone();
            }
            uloz_jeden_beh(conn, &sledovac, &beh, &identita, &mut vysledok, &mut nove_behy)?;
        }
        vysledok.pocet_riadkov += 1;
    }

    // Posledný beh v súbore je stále otvorený (žiadna ďalšia `Zona` ho
    // neuzavrela) — aj tak ho chceme v DB, len s `koniec = None`.
    if let Some(beh) = sledovac.ukonci() {
        if let Some(novsia) = sledovac.hrac_snimka() {
            identita = novsia.clone();
        }
        uloz_jeden_beh(conn, &sledovac, &beh, &identita, &mut vysledok, &mut nove_behy)?;
    }

    // Ceny z CELÉHO súboru. Zároveň sa z nich stavia cenník pre druhý prechod
    // nižšie — pri spätnom importe sa cena položky typicky nájde až potom, ako
    // ju hráč nafarmil, takže v čase ukladania behu ešte známa nebola.
    let sezona = identita.season_id.clone();
    let teraz = cas_zapisu();
    let mut cennik = Cennik::novy();
    for nalez in sledovac.odober_ceny() {
        db::uloz_cenu(conn, &ZaznamCeny::z_nalezu(&nalez, sezona.clone(), teraz))?;
        // Vek ceny sa pri spätnom importe berie z HERNÉHO času riadku, nie
        // z „teraz" — inak by mesiac starý log vyzeral ako čerstvé ceny.
        let kedy = nalez
            .cas
            .as_ref()
            .and_then(|c| c.unix_sekundy())
            .unwrap_or(teraz.max(0) as u64);
        cennik.vloz_v_case(
            Cena::vlastna(nalez.polozka, nalez.p10, nalez.median, nalez.pocet_ponuk, kedy),
            kedy,
        );
        vysledok.pocet_cien += 1;
    }
    for (nazov, hodnota, cas) in sledovac.odober_zostatky() {
        db::uloz_zostatok(conn, &nazov, hodnota, cas.as_ref())?;
        vysledok.pocet_zostatkov += 1;
    }
    for (beh_id, raw_text, cas) in sledovac.odober_raw() {
        // `beh_id` z `Sledovac` je poradové číslo v RÁMCI TOHTO importu, nie
        // DB `id` — `raw` tabuľka ho preto ukladá len informačne (`None`,
        // keď chceme byť presní) radšej než priradiť k nesprávnemu behu.
        let _ = beh_id;
        db::uloz_raw(conn, None, &raw_text, cas.as_ref())?;
    }

    // ── Druhý prechod (TLI 20) ─────────────────────────────────────────
    // Behy sa ukladali s `OceneneHodnoty::default()` (teda `cisty_zisk_fe =
    // NULL`), lebo cenník v tej chvíli ešte neexistoval. Teraz ho máme z
    // celého súboru, tak im hodnotu dopočítame. Beh, ktorého položky cenník
    // nepozná ani teraz, ostane bez hodnoty — nie na nule.
    let nastavenie = Nastavenie::default();
    for beh_id in nove_behy {
        if db::precen_beh(conn, beh_id, &cennik, &nastavenie)? {
            vysledok.pocet_precenenych_behov += 1;
        }
    }

    vysledok.neznamych_riadkov = sledovac.neznamych_riadkov();
    Ok(vysledok)
}

/// Výsledok jednorazového prepočtu histórie (TLI 68 bod e).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct VysledokPrepoctu {
    /// Koľko behov z logu sa v DB našlo a prepísalo.
    pub behov_prepocitanych: usize,
    /// Koľko behov z logu v DB vôbec nie je (nič sa pre ne nevytváralo).
    pub behov_mimo_db: usize,
    /// Koľko prepísaných behov dostalo aj novú ocenenú hodnotu.
    pub behov_precenenych: usize,
}

/// **TLI 68 bod e)** — prejde CELÝ text logu opraveným `Sledovac`om
/// a prepíše položky tých behov, ktoré už v DB sú.
///
/// Oproti `importuj_text` nič nevkladá: beh, ktorý v DB nie je, sa preskočí
/// (`behov_mimo_db`). Zmysel je opraviť nafúknuté dropy z obdobia medzi
/// TLI 56 a TLI 68, nie dopĺňať históriu — tú rieši tlačidlo „Načítať staré
/// logy". Volajúci si MUSÍ pred týmto spraviť kópiu súboru DB (appka to robí
/// v `src-tauri/src/tracker.rs`, `prepocitaj_historiu_tli68`).
pub fn prepocitaj_text(conn: &mut Connection, text: &str) -> rusqlite::Result<VysledokPrepoctu> {
    let mut prepocet = Prepocet::novy();
    for riadok in text.lines() {
        prepocet.riadok(conn, riadok)?;
    }
    prepocet.dokonci(conn)
}

/// Prepočet kŕmený RIADOK PO RIADKU — to isté, čo `prepocitaj_text`, len bez
/// toho, aby volajúci musel mať celý súbor v pamäti naraz.
///
/// Appka to takto robiť MUSÍ: Richardove rotované logy majú bežne cez 900 MB
/// a `std::fs::read` + dekódovanie na `String` by z toho spravilo cez 2 GB
/// naraz — v Ruste je zlyhaná alokácia `abort`, teda pád appky, a keďže
/// zarážka `prepocet_tli68` sa nastavuje až po úspechu, appka by padala pri
/// každom ďalšom štarte.
pub struct Prepocet {
    sledovac: Sledovac,
    identita: HracSnimka,
    prepisane: Vec<i64>,
    vysledok: VysledokPrepoctu,
}

impl Default for Prepocet {
    fn default() -> Self {
        Self::novy()
    }
}

impl Prepocet {
    pub fn novy() -> Self {
        Self {
            sledovac: Sledovac::novy(),
            identita: HracSnimka::default(),
            prepisane: Vec::new(),
            vysledok: VysledokPrepoctu::default(),
        }
    }

    /// Jeden riadok logu. Keď ním skončil beh, hneď sa prepíše v DB.
    pub fn riadok(&mut self, conn: &mut Connection, riadok: &str) -> rusqlite::Result<()> {
        if let Some(beh) = self.sledovac.riadok(riadok) {
            if let Some(novsia) = self.sledovac.hrac_snimka() {
                self.identita = novsia.clone();
            }
            prepis_jeden_beh(
                conn,
                &beh,
                &self.identita,
                &mut self.vysledok,
                &mut self.prepisane,
            )?;
        }
        Ok(())
    }

    /// Uzavrie posledný beh a precení všetko prepísané. Volá sa po KAŽDOM
    /// súbore — cenník sa skladá z toho istého súboru, rovnako ako pri
    /// importe.
    pub fn dokonci(mut self, conn: &mut Connection) -> rusqlite::Result<VysledokPrepoctu> {
        self.dokonci_vnutorne(conn)?;
        Ok(self.vysledok)
    }

    fn dokonci_vnutorne(&mut self, conn: &mut Connection) -> rusqlite::Result<()> {
        let sledovac = &mut self.sledovac;
        let vysledok = &mut self.vysledok;
        let prepisane = &mut self.prepisane;
        let identita = &mut self.identita;

        if let Some(beh) = sledovac.ukonci() {
            if let Some(novsia) = sledovac.hrac_snimka() {
                *identita = novsia.clone();
            }
            prepis_jeden_beh(conn, &beh, identita, vysledok, prepisane)?;
        }

    // Rovnaký druhý prechod ako pri importe: cenník z CELÉHO súboru a až
    // potom ocenenie — inak by prepočítaný beh ostal bez hodnoty len preto,
    // že cena jeho dropu sa v logu našla až o pár máp neskôr.
    let mut cennik = Cennik::novy();
    let teraz = cas_zapisu();
    for nalez in sledovac.odober_ceny() {
        let kedy = nalez
            .cas
            .as_ref()
            .and_then(|c| c.unix_sekundy())
            .unwrap_or(teraz.max(0) as u64);
        cennik.vloz_v_case(
            Cena::vlastna(nalez.polozka, nalez.p10, nalez.median, nalez.pocet_ponuk, kedy),
            kedy,
        );
    }
    let nastavenie = Nastavenie::default();
    for beh_id in std::mem::take(prepisane) {
        if db::precen_beh(conn, beh_id, &cennik, &nastavenie)? {
            vysledok.behov_precenenych += 1;
        }
    }

        Ok(())
    }
}

/// Jeden beh prepočtu: hub sa preskakuje, zvyšok ide do `prepis_polozky_behu`.
/// Appka volá `prepocitaj_text` nad každým súborom logu zvlášť (rotované
/// zálohy majú stovky MB a nemá ich prečo držať v pamäti naraz), preto tu
/// variant „nad viacerými súbormi" zámerne nie je.
fn prepis_jeden_beh(
    conn: &mut Connection,
    beh: &super::stav::Beh,
    identita: &HracSnimka,
    vysledok: &mut VysledokPrepoctu,
    prepisane: &mut Vec<i64>,
) -> rusqlite::Result<()> {
    if beh.je_hub {
        return Ok(());
    }
    match db::prepis_polozky_behu(conn, beh, identita)? {
        Some(id) => {
            vysledok.behov_prepocitanych += 1;
            prepisane.push(id);
        }
        None => vysledok.behov_mimo_db += 1,
    }
    Ok(())
}

/// Unixový čas zápisu. `core` systémové hodiny inak nepoužíva, tu je to
/// nevyhnutné: podľa `zapisane` sa vyberá, ktoré ceny sa ešte neposlali partii.
fn cas_zapisu() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn uloz_jeden_beh(
    conn: &mut Connection,
    sledovac: &Sledovac,
    beh: &super::stav::Beh,
    identita: &HracSnimka,
    vysledok: &mut VysledokImportu,
    nove_behy: &mut Vec<i64>,
) -> rusqlite::Result<()> {
    if beh.je_hub {
        return Ok(());
    }
    // Cenník v tejto chvíli ešte nie je úplný (ceny sa v logu nájdu aj neskôr),
    // preto sa beh ukladá bez hodnoty a doplní ju druhý prechod v `importuj_text`.
    let ocenenie = OceneneHodnoty::default();
    let (beh_id, bol_novy) = db::uloz_beh(conn, beh, identita, &ocenenie)?;
    if bol_novy {
        vysledok.pocet_novych_behov += 1;
        vysledok.pocet_monster += beh.monstra.len();
        nove_behy.push(beh_id);
    } else {
        vysledok.pocet_preskocenych_behov += 1;
        return Ok(());
    }

    if let Some((nazov, cas)) = sledovac.posledna_pact_konfiguracia() {
        let dps = sledovac.posledny_stat_postavy("DPS").map(|(v, _)| v);
        let def = sledovac.posledny_stat_postavy("DEF").map(|(v, _)| v);
        db::uloz_build_snapshot(conn, beh_id, identita, Some(&nazov), dps, def, cas.as_ref())?;
    }
    Ok(())
}

/// Spracuje viacero súborov naraz (aktuálny `UE_game.log` + jeho
/// `UE_game-backup-*.log` zálohy) a spočíta výsledky dokopy. Poradie súborov
/// nie je dôležité pre SPRÁVNOSŤ (dedup je na úrovni jednotlivých behov/cien/
/// zostatkov/raw riadkov, nie na úrovni súboru) — len pre to, ktorý z
/// prekrývajúcich sa behov sa započíta ako „nový" a ktorý ako „preskočený".
pub fn importuj_subory(conn: &mut Connection, obsahy: &[Vec<u8>]) -> rusqlite::Result<VysledokImportu> {
    let mut celkovy = VysledokImportu::default();
    for bajty in obsahy {
        let text = dekoduj(bajty);
        let vysledok = importuj_text(conn, &text)?;
        celkovy.pripocitaj(vysledok);
    }
    Ok(celkovy)
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::tracker::db::{otvor_v_pamati, vyhladaj_behy, FilterBehov};

    const VZORKA: &str = "\
[2026.10.16-10.00.00:000][1]GameLog: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Maps/Mapa1
[2026.10.16-10.00.01:000][1]GameLog: Display: [Game] Monster Created: npc id 1 rarity 1
[2026.10.16-10.00.02:000][1]GameLog: Display: [Game] Monster Created: npc id 2 rarity 3
[2026.10.16-10.00.03:000][1]GameLog: Display: [Game] BagMgr@:Modfy BagItem PageId = 100 SlotId = 1 ConfigBaseId = 400021 Num = 1
[2026.10.16-10.02.00:000][1]GameLog: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Maps/Mapa2
[2026.10.16-10.02.01:000][1]GameLog: Display: [Game] Monster Created: npc id 3 rarity 2
";

    #[test]
    fn import_uzatvori_aj_posledny_otvoreny_beh() {
        let mut conn = otvor_v_pamati().unwrap();
        let vysledok = importuj_text(&mut conn, VZORKA).unwrap();

        assert_eq!(vysledok.pocet_novych_behov, 2, "Mapa1 aj stále otvorená Mapa2 sa majú uložiť");
        assert_eq!(vysledok.pocet_monster, 3);

        let riadky = vyhladaj_behy(&conn, &FilterBehov::default()).unwrap();
        assert_eq!(riadky.len(), 2);
        let mapa2 = riadky.iter().find(|r| r.zona.ends_with("Mapa2")).unwrap();
        assert_eq!(mapa2.pocet_monster, 1);
    }

    #[test]
    fn opakovany_import_toho_isteho_textu_nic_nezdvoji() {
        let mut conn = otvor_v_pamati().unwrap();
        importuj_text(&mut conn, VZORKA).unwrap();
        let druhy = importuj_text(&mut conn, VZORKA).unwrap();

        assert_eq!(druhy.pocet_novych_behov, 0);
        assert_eq!(druhy.pocet_preskocenych_behov, 2);

        let riadky = vyhladaj_behy(&conn, &FilterBehov::default()).unwrap();
        assert_eq!(riadky.len(), 2, "import tej istej histórie druhý raz nesmie pridať ďalšie riadky");
    }

    #[test]
    fn prelapajuce_sa_zalohy_sa_zratavaju_bez_duplicit() {
        // Simuluje dva súbory so spoločným úsekom histórie (bežné pri
        // rotovaných `UE_game-backup-*.log` zálohách).
        let mut conn = otvor_v_pamati().unwrap();
        let subor_a = VZORKA.as_bytes().to_vec();
        let subor_b = VZORKA.as_bytes().to_vec(); // identická "záloha"
        let vysledok = importuj_subory(&mut conn, &[subor_a, subor_b]).unwrap();

        assert_eq!(vysledok.pocet_novych_behov, 2);
        assert_eq!(vysledok.pocet_preskocenych_behov, 2);

        let riadky = vyhladaj_behy(&conn, &FilterBehov::default()).unwrap();
        assert_eq!(riadky.len(), 2);
    }

    // ─── TLI 68 bod e): prepočet nafúknutej histórie ──────────────────────

    /// Log s dvoma mapami. Stránka 103 dostane dump (`InitBagData`) až PRED
    /// druhou mapou — prvá mapa je teda presne ten prípad „appka sa
    /// reštartovala uprostred hrania a nepozná základ".
    const VZORKA_ZAKLAD: &str = "\
[2026.10.20-10.10.00:000][1]GameLog: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Maps/MapaA
[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] ItemChange@ ProtoName=PickItem start
[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] BagMgr@:Modfy BagItem PageId = 103 SlotId = 3 ConfigBaseId = 400033 Num = 49
[2026.10.20-10.11.00:000][1]GameLog: Display: [Game] ItemChange@ ProtoName=PickItem end
[2026.10.20-10.12.00:000][1]GameLog: Display: [Game] SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Maps/MapaB
";

    #[test]
    fn tli68_prepocet_prepise_nafuknute_dropy_a_nevytvori_nove_behy() {
        let mut conn = otvor_v_pamati().unwrap();

        // 1) Do DB sa dostane nafúknutý beh — simulujeme ho tak, že ho
        //    uložíme s položkou 400033 ×49 ručne cez bežný import, ktorému
        //    podstrčíme text so ZNÁMYM základom 0 (presne to appka pred
        //    opravou robila).
        let nafukla = VZORKA_ZAKLAD.replace(
            "[2026.10.20-10.10.00:000][1]GameLog: Display: [Game] SceneLevelMgr@",
            "[2026.10.20-10.09.00:000][1]GameLog: Display: [Game] BagMgr@:InitBagData PageId = 103 SlotId = 99 ConfigBaseId = 1 Num = 1\n[2026.10.20-10.10.00:000][1]GameLog: Display: [Game] SceneLevelMgr@",
        );
        let vysledok = importuj_text(&mut conn, &nafukla).unwrap();
        assert_eq!(vysledok.pocet_novych_behov, 2);
        let pred: i64 = conn
            .query_row(
                "SELECT SUM(mnozstvo) FROM polozky p JOIN behy b ON b.id = p.beh_id
                  WHERE b.zona LIKE '%MapaA' AND p.config_base_id = 400033",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pred, 49, "test predpokladá nafúknutú hodnotu, inak nič nedokazuje");

        // 2) Prepočet nad logom BEZ dumpu — prvá zmena slotu je neznámy
        //    základ, drop sa teda nezapočíta vôbec.
        let prepocet = prepocitaj_text(&mut conn, VZORKA_ZAKLAD).unwrap();
        assert_eq!(prepocet.behov_prepocitanych, 2);
        assert_eq!(prepocet.behov_mimo_db, 0);

        let po: Option<i64> = conn
            .query_row(
                "SELECT SUM(mnozstvo) FROM polozky p JOIN behy b ON b.id = p.beh_id
                  WHERE b.zona LIKE '%MapaA' AND p.config_base_id = 400033",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(po, None, "nafúknutý drop ×49 sa má prepočtom odstrániť");

        // 3) Prepočet nikdy nepridá beh, ktorý v DB nie je.
        let pocet_behov: i64 = conn.query_row("SELECT COUNT(*) FROM behy", [], |r| r.get(0)).unwrap();
        assert_eq!(pocet_behov, 2);
    }

    #[test]
    fn tli68_prepocet_vynuluje_stary_zisk_ked_sa_necena_neda_dopocitat() {
        // Nález z review diffu (20.10.2026): `precen_beh` nezapíše nič, keď
        // v tom istom súbore logu nie je ani jedna cena dropu. Bez vynulovania
        // by v `behy` ostal starý, nafúknutý ZISK vedľa už opravených položiek
        // — teda presne to číslo, na ktoré sa Richard sťažoval.
        let mut conn = otvor_v_pamati().unwrap();
        let nafukla = VZORKA_ZAKLAD.replace(
            "[2026.10.20-10.10.00:000][1]GameLog: Display: [Game] SceneLevelMgr@",
            "[2026.10.20-10.09.00:000][1]GameLog: Display: [Game] BagMgr@:InitBagData PageId = 103 SlotId = 99 ConfigBaseId = 1 Num = 1\n[2026.10.20-10.10.00:000][1]GameLog: Display: [Game] SceneLevelMgr@",
        );
        importuj_text(&mut conn, &nafukla).unwrap();
        let beh_id: i64 = conn
            .query_row("SELECT id FROM behy WHERE zona LIKE '%MapaA'", [], |r| r.get(0))
            .unwrap();
        conn.execute("UPDATE behy SET cisty_zisk_fe = 214.0, fe_za_hodinu = 8034.0 WHERE id = ?1", [beh_id])
            .unwrap();

        prepocitaj_text(&mut conn, VZORKA_ZAKLAD).unwrap();

        let (zisk, za_hodinu): (Option<f64>, Option<f64>) = conn
            .query_row("SELECT cisty_zisk_fe, fe_za_hodinu FROM behy WHERE id = ?1", [beh_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(zisk, None, "starý ZISK nesmie prežiť prepočet položiek");
        assert_eq!(za_hodinu, None, "ani FE/h");
    }

    #[test]
    fn tli68_prepocet_nad_neznamou_historiou_nic_nevytvori() {
        let mut conn = otvor_v_pamati().unwrap();
        let prepocet = prepocitaj_text(&mut conn, VZORKA_ZAKLAD).unwrap();
        assert_eq!(prepocet.behov_prepocitanych, 0);
        assert_eq!(prepocet.behov_mimo_db, 2);
        let pocet_behov: i64 = conn.query_row("SELECT COUNT(*) FROM behy", [], |r| r.get(0)).unwrap();
        assert_eq!(pocet_behov, 0, "prepočet nesmie zjaviť mapy, ktoré v Analýze nikdy neboli");
    }

    #[test]
    fn neznamy_riadok_sa_pocita_ale_nepada() {
        let mut conn = otvor_v_pamati().unwrap();
        let text = "nič užitočné tu nie je\nani tu nie\n";
        let vysledok = importuj_text(&mut conn, text).unwrap();
        assert_eq!(vysledok.neznamych_riadkov, 2);
        assert_eq!(vysledok.pocet_novych_behov, 0);
    }
}
