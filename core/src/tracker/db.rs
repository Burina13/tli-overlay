//! Lokálna SQLite databáza všetkého, čo sa dá z `UE_game.log` vyčítať (TLI 13).
//!
//! Beží na všetkých platformách (Windows appka aj `cargo test` na Linuxe v CI)
//! vďaka `rusqlite` s `bundled` sqlite — žiadna systémová závislosť. Schéma má
//! verziu (`schema_info`) a `migruj()` je idempotentné: dá sa zavolať pri
//! každom otvorení appky bez obáv, že by druhýkrát niečo pokazilo.
//!
//! Zápisy sú zámerne idempotentné (dedup cez `UNIQUE` + `INSERT OR IGNORE`),
//! aby sa live čítanie logu aj spätný import mohli nad tým istým súborom
//! zavolať koľkokoľvek chcú bez toho, aby zdvojili riadky (TLI 13 §1
//! „Duplicity sa rozpoznajú podľa času a hashu riadku").

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use super::ceny::{Cena, Cennik, Nastavenie, Zdroj};
use super::log::Cas;
use super::stav::{Beh, Dovod, HracSnimka, NalezCeny};

/// Aktuálna verzia schémy. Zvyšuje sa pri KAŽDEJ zmene tabuliek; `migruj()`
/// podľa nej vie, ktoré kroky ešte treba spustiť.
/// `4` (TLI 14) pridalo `tracker_upload_stav` — kam appka minule dohrala
/// denný upload KAŽDÉHO súboru. Musí prežiť reštart appky: poslanie
/// `pozicia_od = 0` nad súborom, ktorý server už čiastočne má, by server
/// vyhodnotil ako rotáciu a existujúci log zbytočne archivoval.
/// `3` (TLI 20) pridalo do `ceny` stĺpce `median_fe`/`pocet_ponuk`/`sezona`/
/// `zdroj`/`zapisane` (cez `ALTER TABLE`, dáta ostávajú).
/// `2` (TLI 13b) pridalo `taktiky`/`taktiky_priradenie` — pomenovanie a
/// zlúčenie signatúr taktík hráčom. Všetky `CREATE TABLE IF NOT EXISTS` sú
/// samy o sebe idempotentné, verzia je tu najmä na diagnostiku a budúce kroky,
/// ktoré by už idempotentné CREATE nestačili.
pub const SCHEMA_VERZIA: i32 = 4;

/// Otvorí (alebo vytvorí) databázu na danej ceste a rovno ju zmigruje na
/// najnovšiu schému. Jediné miesto, odkiaľ sa má DB otvárať — inak sa dá
/// ľahko zabudnúť na migráciu pri novom mieste volania.
pub fn otvor(cesta: &Path) -> rusqlite::Result<Connection> {
    if let Some(priecinok) = cesta.parent() {
        let _ = std::fs::create_dir_all(priecinok);
    }
    let conn = Connection::open(cesta)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    // Viacero `StavTrackera` inštancií (napr. testy v `src-tauri`, ktoré si
    // každá otvára DB na tej istej predvolenej ceste) sa vie trafiť do seba
    // pri súbežnom zápise — WAL dovolí jedného writera naraz, ale s
    // predvoleným `busy_timeout=0` by druhý okamžite dostal `SQLITE_BUSY`
    // namiesto krátkeho počkania.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migruj(&conn)?;
    Ok(conn)
}

/// Otvorí databázu výhradne v pamäti — na testy a na krátkodobé výpočty
/// (napr. report zo spätného importu pred tým, než sa appka rozhodne, kam
/// výsledok uložiť natrvalo).
pub fn otvor_v_pamati() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migruj(&conn)?;
    Ok(conn)
}

/// Vytvorí chýbajúce tabuľky/indexy a posunie `schema_info.verzia`. Volá sa sama
/// z `otvor()`/`otvor_v_pamati()` — netreba ju volať ručne, len keď treba
/// zmigrovať existujúce pripojenie druhýkrát (napr. v testoch nad `:memory:`,
/// ktoré si pripojenie otvárajú samé).
pub fn migruj(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_info (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            verzia INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS behy (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            player_id TEXT,
            season_id TEXT,
            hero_id TEXT,
            zona TEXT NOT NULL,
            je_hub INTEGER NOT NULL DEFAULT 0,
            level_uid INTEGER,
            level_typ INTEGER,
            level_id INTEGER,
            podpis_vstupov TEXT NOT NULL,
            zaciatok_cas TEXT,
            koniec_cas TEXT,
            trvanie_ms INTEGER,
            hodnota_dropov_fe REAL,
            hodnota_vstupov_fe REAL,
            cisty_zisk_fe REAL,
            fe_za_hodinu REAL,
            UNIQUE(player_id, season_id, zona, zaciatok_cas)
        );
        CREATE INDEX IF NOT EXISTS idx_behy_cas ON behy(zaciatok_cas);
        CREATE INDEX IF NOT EXISTS idx_behy_zona ON behy(zona);
        CREATE INDEX IF NOT EXISTS idx_behy_podpis ON behy(podpis_vstupov);

        CREATE TABLE IF NOT EXISTS polozky (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            beh_id INTEGER NOT NULL REFERENCES behy(id) ON DELETE CASCADE,
            config_base_id INTEGER NOT NULL,
            mnozstvo INTEGER NOT NULL,
            typ TEXT NOT NULL,
            cas TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_polozky_beh ON polozky(beh_id);
        CREATE INDEX IF NOT EXISTS idx_polozky_cas ON polozky(cas);
        CREATE INDEX IF NOT EXISTS idx_polozky_typ ON polozky(typ);

        CREATE TABLE IF NOT EXISTS monstra (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            beh_id INTEGER NOT NULL REFERENCES behy(id) ON DELETE CASCADE,
            npc_id INTEGER NOT NULL,
            rarita INTEGER NOT NULL,
            affix INTEGER,
            cas TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_monstra_beh ON monstra(beh_id);
        CREATE INDEX IF NOT EXISTS idx_monstra_rarita ON monstra(rarita);
        CREATE INDEX IF NOT EXISTS idx_monstra_cas ON monstra(cas);

        CREATE TABLE IF NOT EXISTS modifikatory_mapy (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            beh_id INTEGER NOT NULL REFERENCES behy(id) ON DELETE CASCADE,
            danger INTEGER,
            kind INTEGER,
            skupina INTEGER,
            seat INTEGER,
            rand_affix_id INTEGER,
            cas TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_modifikatory_beh ON modifikatory_mapy(beh_id);

        CREATE TABLE IF NOT EXISTS ceny (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            config_base_id INTEGER NOT NULL,
            cena_fe REAL NOT NULL,
            cas TEXT,
            UNIQUE(config_base_id, cena_fe, cas)
        );
        CREATE INDEX IF NOT EXISTS idx_ceny_polozka ON ceny(config_base_id);
        CREATE INDEX IF NOT EXISTS idx_ceny_cas ON ceny(cas);

        CREATE TABLE IF NOT EXISTS zostatky (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nazov TEXT NOT NULL,
            hodnota INTEGER NOT NULL,
            cas TEXT,
            UNIQUE(nazov, cas)
        );
        CREATE INDEX IF NOT EXISTS idx_zostatky_nazov ON zostatky(nazov);
        CREATE INDEX IF NOT EXISTS idx_zostatky_cas ON zostatky(cas);

        CREATE TABLE IF NOT EXISTS bossovia (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            beh_id INTEGER NOT NULL REFERENCES behy(id) ON DELETE CASCADE,
            stav TEXT NOT NULL,
            cas TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_bossovia_beh ON bossovia(beh_id);

        CREATE TABLE IF NOT EXISTS frozen_canvas_akcie (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            beh_id INTEGER NOT NULL REFERENCES behy(id) ON DELETE CASCADE,
            typ TEXT NOT NULL,
            cas TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_canvas_beh ON frozen_canvas_akcie(beh_id);
        CREATE INDEX IF NOT EXISTS idx_canvas_cas ON frozen_canvas_akcie(cas);

        CREATE TABLE IF NOT EXISTS build_snapshoty (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            beh_id INTEGER NOT NULL REFERENCES behy(id) ON DELETE CASCADE,
            hero_id TEXT,
            pact_konfiguracia TEXT,
            genius_career_ids TEXT,
            dps REAL,
            def REAL,
            cas TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_build_beh ON build_snapshoty(beh_id);

        CREATE TABLE IF NOT EXISTS raw_udalosti (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            beh_id INTEGER REFERENCES behy(id) ON DELETE SET NULL,
            text TEXT NOT NULL,
            cas TEXT,
            hash_riadku TEXT NOT NULL,
            UNIQUE(hash_riadku, cas)
        );
        CREATE INDEX IF NOT EXISTS idx_raw_cas ON raw_udalosti(cas);

        CREATE TABLE IF NOT EXISTS taktiky (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nazov TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS taktiky_priradenie (
            podpis TEXT PRIMARY KEY,
            taktika_id INTEGER NOT NULL REFERENCES taktiky(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_taktiky_priradenie_taktika ON taktiky_priradenie(taktika_id);

        -- TLI 14: kam appka minule dohrala denný upload KAŽDÉHO súboru
        -- (meno je kľúč, appka vie uploadovať aj zálohové logy popri živom).
        CREATE TABLE IF NOT EXISTS tracker_upload_stav (
            subor TEXT PRIMARY KEY,
            nahrana_pozicia INTEGER NOT NULL,
            odtlacok_zaciatku TEXT NOT NULL
        );
        ",
    )?;

    // TLI 20: `ceny` dostala ďalšie stĺpce. Pridávajú sa `ALTER TABLE`-om, nie
    // prepísaním `CREATE TABLE` — v existujúcej DB hráča už ceny sú a prepis
    // tabuľky by znamenal migráciu dát (a riziko, že sa stratia). `ALTER TABLE
    // ADD COLUMN` je v SQLite lacné a chyba „duplicate column name" pri druhom
    // spustení je očakávaná, preto sa ticho prehliada.
    for stlpec in [
        "median_fe REAL",
        "pocet_ponuk INTEGER",
        "sezona TEXT",
        "zdroj TEXT NOT NULL DEFAULT 'vlastny_trh'",
        "zapisane INTEGER NOT NULL DEFAULT 0",
    ] {
        let _ = conn.execute_batch(&format!("ALTER TABLE ceny ADD COLUMN {stlpec};"));
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_ceny_zapisane ON ceny(zapisane);",
    )?;

    conn.execute(
        "INSERT INTO schema_info (id, verzia) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET verzia = excluded.verzia",
        params![SCHEMA_VERZIA],
    )?;
    Ok(())
}

fn cas_text(cas: &Option<Cas>) -> Option<String> {
    cas.as_ref().map(|c| c.text())
}

fn dovod_text(dovod: Dovod) -> &'static str {
    match dovod {
        Dovod::Drop => "drop",
        Dovod::VstupMapy => "vstup_mapy",
        Dovod::Obchod => "obchod",
        Dovod::Ignorovat => "ignorovat",
        Dovod::Ostatne => "ostatne",
    }
}

/// Jednoduchý, rýchly hash riadku (FNV-1a, 64-bit) na dedup v `raw_udalosti` —
/// netreba kryptografickú silu, len aby sa ten istý riadok pri opakovanom
/// importe spoľahlivo zrazil na rovnaký kľúč.
pub fn hash_riadku(riadok: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for b in riadok.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// Výsledok jedného behu, hotový na uloženie — `Beh` z `stav.rs` plus to, čo
/// sa z neho dá vypočítať cennikom (hodnota dropov/vstupov/zisku). Oddelené od
/// `agregacie::VysledokBehu`, aby `db` nemusel ťahať celé UI-orientované
/// typy — potrebuje len polia, ktoré sa naozaj ukladajú.
#[derive(Debug, Clone, Default)]
pub struct OceneneHodnoty {
    pub hodnota_dropov_fe: Option<f64>,
    pub hodnota_vstupov_fe: Option<f64>,
    pub cisty_zisk_fe: Option<f64>,
    pub fe_za_hodinu: Option<f64>,
}

/// Uloží jeden uzavretý beh aj so všetkým, čo sa počas neho nazbieralo
/// (dropy/vstupy, monštrá, modifikátory, bossovia, canvas akcie) — v JEDNEJ
/// transakcii. Idempotentné: keď beh (podľa `player_id+season_id+zona+zaciatok`)
/// už existuje, vráti jeho `id` a nepridá nič duplicitne. Druhá hodnota
/// v návrate hovorí, či bol beh PRI TOMTO volaní novo vložený (`true`) alebo
/// už existoval (`false`) — používa to `import.rs` na report „X nových, Y
/// preskočených".
pub fn uloz_beh(
    conn: &mut Connection,
    beh: &Beh,
    identita: &HracSnimka,
    ocenenie: &OceneneHodnoty,
) -> rusqlite::Result<(i64, bool)> {
    let tx = conn.transaction()?;

    // Prirodzený kľúč ide do stĺpcov ako PRÁZDNY REŤAZEC, nikdy ako `NULL` —
    // SQLite považuje dve `NULL` hodnoty v `UNIQUE` za RÔZNE (nikdy si
    // nekolidujú), takže behy bez rozpoznanej identity (veľmi bežné, pozri
    // `log-format.md` §9 — krátke session bez `+player+...` bloku) by sa
    // pri opakovanom importe inak zakaždým zdvojili.
    let player_id = identita.player_id.clone().unwrap_or_default();
    let season_id = identita.season_id.clone().unwrap_or_default();
    let zaciatok_cas = cas_text(&beh.zaciatok).unwrap_or_default();

    tx.execute(
        "INSERT INTO behy (
            player_id, season_id, hero_id, zona, je_hub, level_uid, level_typ, level_id,
            podpis_vstupov, zaciatok_cas, koniec_cas, trvanie_ms,
            hodnota_dropov_fe, hodnota_vstupov_fe, cisty_zisk_fe, fe_za_hodinu
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
         ON CONFLICT(player_id, season_id, zona, zaciatok_cas) DO NOTHING",
        params![
            player_id,
            season_id,
            identita.hero_id,
            beh.zona,
            beh.je_hub as i64,
            beh.level_uid.map(|v| v as i64),
            beh.level_typ,
            beh.level_id.map(|v| v as i64),
            beh.podpis_vstupov(),
            zaciatok_cas,
            cas_text(&beh.koniec),
            beh.trvanie_ms(),
            ocenenie.hodnota_dropov_fe,
            ocenenie.hodnota_vstupov_fe,
            ocenenie.cisty_zisk_fe,
            ocenenie.fe_za_hodinu,
        ],
    )?;
    // `changes() == 0` znamená, že `ON CONFLICT ... DO NOTHING` zahodil
    // vloženie, lebo beh (podľa prirodzeného kľúča) už existuje — detaily
    // (položky, monštrá, ...) sa teda tiež už uložili pri predošlom importe
    // a NESMÚ sa zapísať druhýkrát, inak by sa zdvojili.
    let bol_novy = tx.changes() > 0;

    let beh_id: i64 = tx.query_row(
        "SELECT id FROM behy WHERE
            player_id = ?1 AND season_id = ?2 AND zona = ?3 AND zaciatok_cas = ?4",
        params![player_id, season_id, beh.zona, zaciatok_cas],
        |r| r.get(0),
    )?;

    if !bol_novy {
        tx.commit()?;
        return Ok((beh_id, false));
    }

    for zmena in &beh.vsetky_zmeny {
        tx.execute(
            "INSERT INTO polozky (beh_id, config_base_id, mnozstvo, typ, cas) VALUES (?1,?2,?3,?4,?5)",
            params![beh_id, zmena.polozka, zmena.mnozstvo, dovod_text(zmena.dovod), cas_text(&zmena.cas)],
        )?;
    }

    for m in &beh.monstra {
        tx.execute(
            "INSERT INTO monstra (beh_id, npc_id, rarita, affix, cas) VALUES (?1,?2,?3,?4,?5)",
            params![beh_id, m.npc_id as i64, m.rarita, m.affix.map(|a| a as i64), cas_text(&m.cas)],
        )?;
    }

    for m in &beh.modifikatory {
        tx.execute(
            "INSERT INTO modifikatory_mapy (beh_id, danger, kind, skupina, seat, rand_affix_id, cas)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![beh_id, m.danger, m.kind, m.skupina, m.seat, m.rand_affix_id, cas_text(&m.cas)],
        )?;
    }

    for b in &beh.bossovia {
        let stav = if b.aktivny { "aktivny" } else { "koniec" };
        tx.execute(
            "INSERT INTO bossovia (beh_id, stav, cas) VALUES (?1,?2,?3)",
            params![beh_id, stav, cas_text(&b.cas)],
        )?;
    }

    for c in &beh.canvas_akcie {
        tx.execute(
            "INSERT INTO frozen_canvas_akcie (beh_id, typ, cas) VALUES (?1,?2,?3)",
            params![beh_id, c.typ, cas_text(&c.cas)],
        )?;
    }

    tx.commit()?;
    Ok((beh_id, true))
}

/// Posledná známa neprázdna identita hráča z predošlého uloženého behu
/// (TLI 19 bug E) — záloha pre `uloz_beh_do_db` v `src-tauri`, keď živé
/// čítanie v AKTUÁLNEJ session ešte nevidelo žiadny `+player+...`/
/// `GetPlayerData` blok.
///
/// **Prečo to vôbec treba:** „Enable Log" sa musí zapínať na začiatku KAŽDEJ
/// session (hra si to nepamätá) a identita sa posiela pri PRIHLÁSENÍ — teda
/// typicky PRED tým, než hráč log stihne zapnúť. Spätný import (`import.rs`)
/// vidí logy od úplného začiatku vrátane prihlásenia, live čítanie nie —
/// presne preto oprava `ecec22f` identitu doplnila len tam. Jeden inštalovaný
/// klient appky = jeden hráč, takže identita sa medzi sessions nemení (okrem
/// sezóny, a tá sa prejaví hneď, ako prvý riadok identity v novej sezóne
/// naozaj príde a nahradí túto zálohu).
pub fn posledna_znama_identita(conn: &Connection) -> rusqlite::Result<Option<HracSnimka>> {
    conn.query_row(
        "SELECT player_id, season_id, hero_id FROM behy
         WHERE player_id IS NOT NULL AND player_id != ''
         ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            Ok(HracSnimka {
                player_id: row.get(0)?,
                season_id: row.get(1)?,
                hero_id: row.get(2)?,
                ..Default::default()
            })
        },
    )
    .optional()
}

/// Pripíše jeden nájdený build snapshot (DPS/DEF + pact konfigurácia +
/// talenty) k behu. Volá sa samostatne od `uloz_beh`, lebo tieto údaje žijú
/// v `Sledovac` globálne (nie v `Beh`) — volajúci ich spojí sám.
pub fn uloz_build_snapshot(
    conn: &Connection,
    beh_id: i64,
    identita: &HracSnimka,
    pact_konfiguracia: Option<&str>,
    dps: Option<f64>,
    def: Option<f64>,
    cas: Option<&Cas>,
) -> rusqlite::Result<()> {
    if pact_konfiguracia.is_none() && dps.is_none() && def.is_none() && identita.genius_career_ids.is_empty() {
        return Ok(());
    }
    let genius_json = serde_json::to_string(&identita.genius_career_ids).unwrap_or_default();
    conn.execute(
        "INSERT INTO build_snapshoty (beh_id, hero_id, pact_konfiguracia, genius_career_ids, dps, def, cas)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![beh_id, identita.hero_id, pact_konfiguracia, genius_json, dps, def, cas.map(|c| c.text())],
    )?;
    Ok(())
}

/// Jeden riadok tabuľky `ceny` (TLI 20). `cena_fe` je referenčná cena —
/// pri vlastnom vyhľadaní 10. percentil ponúk, pri cene od partie medián cez
/// hráčov. Stĺpec sa zámerne NEPREMENOVAL: v DB hráčov už dáta sú.
#[derive(Debug, Clone, PartialEq)]
pub struct ZaznamCeny {
    pub polozka: u32,
    pub cena_fe: f64,
    pub median_fe: Option<f64>,
    pub pocet_ponuk: Option<u32>,
    pub sezona: Option<String>,
    pub zdroj: String,
    /// Herný čas z logu (`RRRR-MM-DD HH:MM:SS`), ak ho riadok niesol.
    pub cas: Option<String>,
    /// Unixový čas zápisu do DB. Podľa neho sa vyberá, čo sa ešte neodoslalo
    /// partii — herný čas na to nestačí (môže chýbať a nie je monotónny).
    pub zapisane: i64,
}

impl ZaznamCeny {
    /// Záznam z vlastného vyhľadania na Exchange.
    pub fn z_nalezu(n: &NalezCeny, sezona: Option<String>, zapisane: i64) -> Self {
        Self {
            polozka: n.polozka,
            cena_fe: n.p10,
            median_fe: Some(n.median),
            pocet_ponuk: Some(n.pocet_ponuk),
            sezona,
            zdroj: "vlastny_trh".to_string(),
            cas: n.cas.as_ref().map(|c| c.text()),
            zapisane,
        }
    }
}

/// Uloží jednu trhovú cenu. `INSERT OR IGNORE` kvôli dedup na
/// `(config_base_id, cena_fe, cas)` — tá istá cena z toho istého okamihu sa
/// pri opakovanom importe nezdvojí.
pub fn uloz_cenu(conn: &Connection, z: &ZaznamCeny) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO ceny
            (config_base_id, cena_fe, cas, median_fe, pocet_ponuk, sezona, zdroj, zapisane)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            z.polozka, z.cena_fe, z.cas, z.median_fe, z.pocet_ponuk, z.sezona, z.zdroj, z.zapisane
        ],
    )?;
    Ok(())
}

/// Ceny zapísané PO zadanom okamihu — rozdielový vstup pre `POST /api/ceny`.
/// Posielajú sa len ceny z VLASTNÉHO vyhľadania: ceny stiahnuté od partie by
/// sa inak vrátili späť a agregát by sa sám sebou potvrdzoval.
pub fn ceny_od(conn: &Connection, od: i64, max: usize) -> rusqlite::Result<Vec<ZaznamCeny>> {
    let mut dotaz = conn.prepare(
        "SELECT config_base_id, cena_fe, median_fe, pocet_ponuk, sezona, zdroj, cas, zapisane
           FROM ceny
          WHERE zapisane > ?1 AND zdroj = 'vlastny_trh'
          ORDER BY zapisane ASC
          LIMIT ?2",
    )?;
    let riadky = dotaz.query_map(params![od, max as i64], |r| {
        Ok(ZaznamCeny {
            polozka: r.get::<_, i64>(0)? as u32,
            cena_fe: r.get(1)?,
            median_fe: r.get(2)?,
            pocet_ponuk: r.get::<_, Option<i64>>(3)?.map(|v| v as u32),
            sezona: r.get(4)?,
            zdroj: r.get(5)?,
            cas: r.get(6)?,
            zapisane: r.get(7)?,
        })
    })?;
    riadky.collect()
}

/// Postaví cenník z DB — najnovšia cena na položku podľa `zapisane`.
///
/// Toto je OFFLINE CACHE: appka po štarte pozná ceny aj bez siete a aj bez
/// toho, aby znova prečítala celý log. Bez nej mal hráč po každom reštarte
/// appky prázdny cenník a behy ocenené na 0 FE — presne ten problém, kvôli
/// ktorému TLI 20 vzniklo.
pub fn nacitaj_cennik(conn: &Connection) -> rusqlite::Result<Cennik> {
    let mut dotaz = conn.prepare(
        "SELECT c.config_base_id, c.cena_fe, c.median_fe, c.pocet_ponuk, c.zdroj, c.zapisane
           FROM ceny c
           JOIN (SELECT config_base_id, MAX(zapisane) AS max_z
                   FROM ceny GROUP BY config_base_id) n
             ON n.config_base_id = c.config_base_id AND n.max_z = c.zapisane
          GROUP BY c.config_base_id",
    )?;
    let riadky = dotaz.query_map([], |r| {
        Ok(Cena {
            polozka: r.get::<_, i64>(0)? as u32,
            fe: r.get(1)?,
            median: r.get(2)?,
            pocet_ponuk: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as u32,
            zdroj: Zdroj::z_kodu(&r.get::<_, String>(4)?),
            kedy: r.get::<_, i64>(5)?.max(0) as u64,
            zdrojov: 1,
        })
    })?;
    let mut cennik = Cennik::novy();
    for c in riadky {
        let c = c?;
        // `vloz_v_case` s časom tej istej ceny: pri rovnakej položke vyhrá
        // novšia/dôveryhodnejšia — rovnaké pravidlo ako počas behu.
        let kedy = c.kedy;
        cennik.vloz_v_case(c, kedy);
    }
    Ok(cennik)
}

/// Prepočíta hodnoty jedného už uloženého behu podľa zadaného cenníka a zapíše
/// ich do `behy` (TLI 20, druhý prechod spätného importu).
///
/// Číta `polozky` TOHO behu z DB (nie `Beh` v pamäti), takže sa dá spustiť aj
/// dlho po importe. Vracia `true`, keď sa hodnoty naozaj zapísali — teda keď
/// mal beh aspoň jednu položku so známou cenou. Beh, ktorého položky cenník
/// vôbec nepozná, sa NEPREPISUJE na nuly: `NULL` („nevieme") je pravdivejšie
/// než `0 FE` („nezarobil nič") a presne to bol pôvodný problém TLI 20.
pub fn precen_beh(
    conn: &Connection,
    beh_id: i64,
    cennik: &Cennik,
    nastavenie: &Nastavenie,
) -> rusqlite::Result<bool> {
    let mut dotaz = conn.prepare(
        "SELECT config_base_id, typ, SUM(mnozstvo) FROM polozky
          WHERE beh_id = ?1 GROUP BY config_base_id, typ",
    )?;
    let mut dropy: std::collections::BTreeMap<u32, i64> = Default::default();
    let mut vstupy: std::collections::BTreeMap<u32, i64> = Default::default();
    let riadky = dotaz.query_map(params![beh_id], |r| {
        Ok((r.get::<_, i64>(0)? as u32, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
    })?;
    for riadok in riadky {
        let (polozka, typ, mnozstvo) = riadok?;
        match typ.as_str() {
            "drop" if mnozstvo > 0 => *dropy.entry(polozka).or_insert(0) += mnozstvo,
            // `vstup_mapy` je v DB uložený záporne (spotreba) — na ocenenie
            // potrebujeme kladný počet spotrebovaných kusov.
            "vstup_mapy" if mnozstvo < 0 => *vstupy.entry(polozka).or_insert(0) += -mnozstvo,
            _ => {}
        }
    }
    if dropy.is_empty() && vstupy.is_empty() {
        return Ok(false);
    }

    let ocenene_dropy = super::ceny::ocen(&dropy, cennik, nastavenie);
    let ocenene_vstupy = super::ceny::ocen(&vstupy, cennik, nastavenie);
    let znama_cena = ocenene_dropy.bez_ceny.len() < dropy.len()
        || ocenene_vstupy.bez_ceny.len() < vstupy.len();
    if !znama_cena {
        return Ok(false);
    }

    let cisty = ocenene_dropy.fe - ocenene_vstupy.fe;
    let trvanie: Option<i64> = conn
        .query_row("SELECT trvanie_ms FROM behy WHERE id = ?1", params![beh_id], |r| r.get(0))
        .optional()?
        .flatten();
    let fe_za_hodinu = trvanie
        .filter(|t| *t > 0)
        .map(|t| cisty * 3_600_000.0 / t as f64);

    conn.execute(
        "UPDATE behy SET hodnota_dropov_fe = ?1, hodnota_vstupov_fe = ?2,
                         cisty_zisk_fe = ?3, fe_za_hodinu = ?4
          WHERE id = ?5",
        params![ocenene_dropy.fe, ocenene_vstupy.fe, cisty, fe_za_hodinu, beh_id],
    )?;
    Ok(true)
}

/// Uloží jeden zostatok meny/počítadla (z `Sledovac::odober_zostatky()`).
pub fn uloz_zostatok(conn: &Connection, nazov: &str, hodnota: i64, cas: Option<&Cas>) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO zostatky (nazov, hodnota, cas) VALUES (?1,?2,?3)",
        params![nazov, hodnota, cas.map(|c| c.text())],
    )?;
    Ok(())
}

/// Uloží jeden „raw" riadok (z `Sledovac::odober_raw()`). `beh_id` je `None`,
/// keď riadok prišiel mimo akéhokoľvek behu (napr. v menu/hube).
pub fn uloz_raw(conn: &Connection, beh_id: Option<i64>, text: &str, cas: Option<&Cas>) -> rusqlite::Result<()> {
    let hash = hash_riadku(text);
    conn.execute(
        "INSERT OR IGNORE INTO raw_udalosti (beh_id, text, cas, hash_riadku) VALUES (?1,?2,?3,?4)",
        params![beh_id, text, cas.map(|c| c.text()), hash],
    )?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Dotazy pre záložku „Analýza" (TLI 13 §2)
// ─────────────────────────────────────────────────────────────────────────────

/// Filter pre vyhľadávanie behov. Prázdne/`None` pole = nefiltruje sa podľa
/// neho. Dátumy sú v tvare `RRRR-MM-DD HH:MM:SS` (rovnaký tvar ako `Cas::text`),
/// porovnanie je textové, čo pri tomto formáte funguje rovnako ako chronologické.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FilterBehov {
    pub zona: Option<String>,
    pub podpis_vstupov: Option<String>,
    pub od_casu: Option<String>,
    pub do_casu: Option<String>,
    pub min_zisk_fe: Option<f64>,
    pub limit: i64,
}

/// Jeden riadok zoznamu vo výsledkoch vyhľadávania.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BehRiadok {
    pub id: i64,
    pub zona: String,
    pub podpis_vstupov: String,
    pub zaciatok_cas: Option<String>,
    pub trvanie_ms: Option<i64>,
    pub cisty_zisk_fe: Option<f64>,
    pub fe_za_hodinu: Option<f64>,
    pub pocet_monster: i64,
    pub pocet_rares_a_bossov: i64,
}

/// Vyhľadá behy podľa filtra, zoradené od najnovšieho. `limit <= 0` znamená
/// „bez stropu" (pri reálnej lige to appka samozrejme nikdy nevolá naostro —
/// UI si strop nastaví vždy, toto je len pre testy/export).
pub fn vyhladaj_behy(conn: &Connection, filter: &FilterBehov) -> rusqlite::Result<Vec<BehRiadok>> {
    let limit = if filter.limit > 0 { filter.limit } else { i64::MAX };
    let mut stmt = conn.prepare(
        "SELECT b.id, b.zona, b.podpis_vstupov, b.zaciatok_cas, b.trvanie_ms, b.cisty_zisk_fe, b.fe_za_hodinu,
                (SELECT COUNT(*) FROM monstra m WHERE m.beh_id = b.id) AS pocet_monster,
                (SELECT COUNT(*) FROM monstra m WHERE m.beh_id = b.id AND m.rarita >= 3) AS pocet_rares
         FROM behy b
         WHERE (?1 IS NULL OR b.zona = ?1)
           AND (?2 IS NULL OR b.podpis_vstupov = ?2)
           AND (?3 IS NULL OR b.zaciatok_cas >= ?3)
           AND (?4 IS NULL OR b.zaciatok_cas <= ?4)
           AND (?5 IS NULL OR b.cisty_zisk_fe >= ?5)
           AND b.je_hub = 0
         ORDER BY b.zaciatok_cas DESC
         LIMIT ?6",
    )?;
    let riadky = stmt
        .query_map(
            params![
                filter.zona,
                filter.podpis_vstupov,
                filter.od_casu,
                filter.do_casu,
                filter.min_zisk_fe,
                limit,
            ],
            |r| {
                Ok(BehRiadok {
                    id: r.get(0)?,
                    zona: r.get(1)?,
                    podpis_vstupov: r.get(2)?,
                    zaciatok_cas: r.get(3)?,
                    trvanie_ms: r.get(4)?,
                    cisty_zisk_fe: r.get(5)?,
                    fe_za_hodinu: r.get(6)?,
                    pocet_monster: r.get(7)?,
                    pocet_rares_a_bossov: r.get(8)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(riadky)
}

/// Súhrn monštier podľa rarity pre jeden beh — priamo odpoveď na Richardov
/// príkladný dotaz „koľko monštier a akej rarity mala ktorá mapa".
pub fn monstra_podla_rarity(conn: &Connection, beh_id: i64) -> rusqlite::Result<Vec<(u8, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT rarita, COUNT(*) FROM monstra WHERE beh_id = ?1 GROUP BY rarita ORDER BY rarita",
    )?;
    let riadky = stmt
        .query_map(params![beh_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(riadky)
}

/// To isté ako `monstra_podla_rarity`, ale naprieč CELOU zónou (všetky behy
/// s rovnakým `zona`) — priemer na mapu je `celkovy_pocet / pocet_behov`.
pub fn monstra_podla_rarity_pre_zonu(conn: &Connection, zona: &str) -> rusqlite::Result<Vec<(u8, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT m.rarita, COUNT(*) FROM monstra m
         JOIN behy b ON b.id = m.beh_id
         WHERE b.zona = ?1
         GROUP BY m.rarita ORDER BY m.rarita",
    )?;
    let riadky = stmt
        .query_map(params![zona], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(riadky)
}

// ─────────────────────────────────────────────────────────────────────────────
// Záložka „Analýza" — taktiky (TLI 13b): zoskupenie behov podľa podpisu
// taktiky (chart+kompasy+mapa+mód+modifikátory+build) a ich pomenovanie/zlúčenie.
// Samotné zoskupenie a štatistiky počíta `taktiky.rs` (čisto, bez DB) — tu je
// len surové čítanie dát a perzistencia pomenovania/zlúčenia.
// ─────────────────────────────────────────────────────────────────────────────

/// Filter pre záložku Taktiky — obdobie, hrdina a liga (`season_id`).
/// Rovnaký vzor ako `FilterBehov`: `None`/prázdne pole = nefiltruje sa.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FilterTaktik {
    pub od_casu: Option<String>,
    pub do_casu: Option<String>,
    pub hero_id: Option<String>,
    pub season_id: Option<String>,
}

/// Jeden beh so všetkým, čo treba na zostavenie podpisu taktiky a jeho
/// štatistík. Zámerne ploché (žiadne vnorené typy z `stav.rs`) — `taktiky.rs`
/// z toho stavia `VstupTaktiky` bez závislosti na tom, odkiaľ dáta prišli.
#[derive(Debug, Clone, Default)]
pub struct RiadokPreTaktiku {
    pub beh_id: i64,
    pub podpis_vstupov: String,
    pub zona: String,
    pub level_typ: Option<i64>,
    pub hero_id: Option<String>,
    pub pact_konfiguracia: Option<String>,
    /// JSON pole (`["id1","id2",...]`), presne ako ho ukladá `uloz_build_snapshot`.
    pub genius_career_ids: Option<String>,
    pub cisty_zisk_fe: Option<f64>,
    pub hodnota_vstupov_fe: Option<f64>,
    pub fe_za_hodinu: Option<f64>,
    pub trvanie_ms: Option<i64>,
    pub zaciatok_cas: Option<String>,
}

/// Načíta behy (bez hubových) pre výpočet taktík, s POSLEDNÝM build snapshotom
/// daného behu (join na `MAX(id)` v rámci `beh_id` — build sa počas behu môže
/// zaznamenať viackrát, napr. pri zmene pact konfigurácie, berie sa finálny stav).
pub fn data_pre_taktiky(conn: &Connection, filter: &FilterTaktik) -> rusqlite::Result<Vec<RiadokPreTaktiku>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.podpis_vstupov, b.zona, b.level_typ, b.hero_id,
                bs.pact_konfiguracia, bs.genius_career_ids,
                b.cisty_zisk_fe, b.hodnota_vstupov_fe, b.fe_za_hodinu, b.trvanie_ms, b.zaciatok_cas
         FROM behy b
         LEFT JOIN (
             SELECT bs1.beh_id, bs1.pact_konfiguracia, bs1.genius_career_ids
             FROM build_snapshoty bs1
             INNER JOIN (SELECT beh_id, MAX(id) AS max_id FROM build_snapshoty GROUP BY beh_id) posledny
               ON bs1.beh_id = posledny.beh_id AND bs1.id = posledny.max_id
         ) bs ON bs.beh_id = b.id
         WHERE b.je_hub = 0
           AND (?1 IS NULL OR b.zaciatok_cas >= ?1)
           AND (?2 IS NULL OR b.zaciatok_cas <= ?2)
           AND (?3 IS NULL OR b.hero_id = ?3)
           AND (?4 IS NULL OR b.season_id = ?4)",
    )?;
    let riadky = stmt
        .query_map(
            params![filter.od_casu, filter.do_casu, filter.hero_id, filter.season_id],
            |r| {
                Ok(RiadokPreTaktiku {
                    beh_id: r.get(0)?,
                    podpis_vstupov: r.get(1)?,
                    zona: r.get(2)?,
                    level_typ: r.get(3)?,
                    hero_id: r.get(4)?,
                    pact_konfiguracia: r.get(5)?,
                    genius_career_ids: r.get(6)?,
                    cisty_zisk_fe: r.get(7)?,
                    hodnota_vstupov_fe: r.get(8)?,
                    fe_za_hodinu: r.get(9)?,
                    trvanie_ms: r.get(10)?,
                    zaciatok_cas: r.get(11)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(riadky)
}

/// Podpis modifikátorov mapy pre KAŽDÝ beh, ktorý aspoň jeden má — triedené
/// a spojené, aby ten istý súbor modifikátorov dal vždy ten istý reťazec bez
/// ohľadu na poradie, v akom ich log zapísal.
pub fn modifikator_podpisy(conn: &Connection) -> rusqlite::Result<std::collections::BTreeMap<i64, String>> {
    let mut stmt = conn.prepare(
        "SELECT beh_id, danger, kind, skupina, rand_affix_id FROM modifikatory_mapy
         ORDER BY beh_id, danger, kind, skupina, rand_affix_id",
    )?;
    let mut casti: std::collections::BTreeMap<i64, Vec<String>> = std::collections::BTreeMap::new();
    let riadky = stmt.query_map([], |r| {
        let beh_id: i64 = r.get(0)?;
        let danger: Option<i64> = r.get(1)?;
        let kind: Option<i64> = r.get(2)?;
        let skupina: Option<i64> = r.get(3)?;
        let rand_affix_id: Option<i64> = r.get(4)?;
        Ok((
            beh_id,
            format!(
                "{}.{}.{}.{}",
                danger.map(|v| v.to_string()).unwrap_or_default(),
                kind.map(|v| v.to_string()).unwrap_or_default(),
                skupina.map(|v| v.to_string()).unwrap_or_default(),
                rand_affix_id.map(|v| v.to_string()).unwrap_or_default(),
            ),
        ))
    })?;
    for riadok in riadky {
        let (beh_id, cast) = riadok?;
        casti.entry(beh_id).or_default().push(cast);
    }
    Ok(casti.into_iter().map(|(beh_id, v)| (beh_id, v.join("|"))).collect())
}

/// Uloží nové pomenovanie/zlúčenie taktiky: založí jeden riadok v `taktiky`
/// a napojí naň VŠETKY dané podpisy (`ON CONFLICT` prepíše predchádzajúce
/// priradenie, keď sa podpis presúva do inej skupiny). Vracia ID novej taktiky.
/// Pri bežnom premenovaní jednej taktiky (jeden podpis) toto zakladá vždy nový
/// riadok v `taktiky` — stará taktika ostane v tabuľke bez priradení
/// (neškodné, len nepoužívané meno; úklad nie je súčasťou tejto časti).
pub fn ulozit_taktiku(conn: &Connection, podpisy: &[String], nazov: &str) -> rusqlite::Result<i64> {
    conn.execute("INSERT INTO taktiky (nazov) VALUES (?1)", params![nazov])?;
    let id = conn.last_insert_rowid();
    for podpis in podpisy {
        conn.execute(
            "INSERT INTO taktiky_priradenie (podpis, taktika_id) VALUES (?1, ?2)
             ON CONFLICT(podpis) DO UPDATE SET taktika_id = excluded.taktika_id",
            params![podpis, id],
        )?;
    }
    Ok(id)
}

/// Načíta všetky priradenia podpis → (ID taktiky, názov) na zoskupenie v `taktiky.rs`.
pub fn nacitaj_priradenia_taktik(
    conn: &Connection,
) -> rusqlite::Result<std::collections::BTreeMap<String, (i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT tp.podpis, tp.taktika_id, t.nazov
         FROM taktiky_priradenie tp JOIN taktiky t ON t.id = tp.taktika_id",
    )?;
    let riadky = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))
        })?
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
    Ok(riadky)
}

/// Export zoznamu behov do CSV — jeden riadok na beh.
pub fn export_csv(riadky: &[BehRiadok]) -> String {
    let mut out = String::from(
        "id;zona;chart_a_kompasy;zaciatok;trvanie_s;cisty_zisk_fe;fe_za_hodinu;pocet_monster;pocet_rares_a_bossov\n",
    );
    for r in riadky {
        out.push_str(&format!(
            "{};{};{};{};{};{};{};{};{}\n",
            r.id,
            r.zona.replace(';', ","),
            r.podpis_vstupov,
            r.zaciatok_cas.as_deref().unwrap_or(""),
            r.trvanie_ms.map(|t| (t / 1000).to_string()).unwrap_or_default(),
            r.cisty_zisk_fe.map(|v| format!("{v:.2}")).unwrap_or_default(),
            r.fe_za_hodinu.map(|v| format!("{v:.2}")).unwrap_or_default(),
            r.pocet_monster,
            r.pocet_rares_a_bossov,
        ));
    }
    out
}

/// Export zoznamu behov do JSON (pole objektov).
pub fn export_json(riadky: &[BehRiadok]) -> String {
    serde_json::to_string_pretty(riadky).unwrap_or_else(|_| "[]".to_string())
}

/// Prevedie jeden stĺpec na `serde_json::Value` bez straty typu (čísla
/// ostanú čísla, nie reťazce) — potrebné, lebo `export_pre_upload` nepozná
/// vopred schému žiadnej konkrétnej tabuľky.
fn hodnota_na_json(v: rusqlite::types::Value) -> serde_json::Value {
    use rusqlite::types::Value as V;
    match v {
        V::Null => serde_json::Value::Null,
        V::Integer(i) => serde_json::Value::from(i),
        V::Real(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        V::Text(s) => serde_json::Value::String(s),
        // Táto DB blob stĺpce v exportovaných tabuľkách nepoužíva — pre
        // úplnosť aspoň pole bajtov, nech export ostane platný JSON aj keby
        // sa to niekedy zmenilo (žiadna nová závislosť len kvôli tomuto).
        V::Blob(b) => serde_json::Value::Array(b.into_iter().map(serde_json::Value::from).collect()),
    }
}

fn exportuj_tabulku(conn: &Connection, tabulka: &str) -> rusqlite::Result<serde_json::Value> {
    // `tabulka` je vždy jeden z pevných literálov nižšie — nikdy vstup od
    // používateľa — takže formátovanie priamo do SQL je tu bezpečné (rusqlite
    // parametre mená tabuliek/stĺpcov nepodporujú).
    let mut stmt = conn.prepare(&format!("SELECT * FROM {tabulka}"))?;
    let stlpce: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let riadky = stmt.query_map([], |r| {
        let mut objekt = serde_json::Map::with_capacity(stlpce.len());
        for (i, nazov) in stlpce.iter().enumerate() {
            let hodnota: rusqlite::types::Value = r.get(i)?;
            objekt.insert(nazov.clone(), hodnota_na_json(hodnota));
        }
        Ok(serde_json::Value::Object(objekt))
    })?;
    riadky.collect::<Result<Vec<_>, _>>().map(serde_json::Value::Array)
}

/// Export lokálnej DB pre denný upload na server (TLI 14, zadanie bod 1):
/// presne tabuľky, ktoré serverové prepočítanie modelov z TLI 13c (bayes,
/// regresia, bandit, change-point) a taktík z TLI 13b potrebuje — `behy`,
/// `polozky`, `monstra`, `modifikatory_mapy`, `ceny`, `taktiky`,
/// `taktiky_priradenie`. Zámerne VYNECHÁVA `zostatky`, `bossovia`,
/// `frozen_canvas_akcie`, `build_snapshoty` a `raw_udalosti` — to posledné
/// ide na server už raz, surové, v samotnom logu (zadanie bod 1), duplicitne
/// v DB exporte netreba.
///
/// Odoslanie (gzip, HTTP) rieši `src-tauri` — tento modul len postaví obsah.
pub fn export_pre_upload(conn: &Connection) -> rusqlite::Result<serde_json::Value> {
    let mut vysledok = serde_json::Map::new();
    vysledok.insert("schema_verzia".to_string(), serde_json::Value::from(SCHEMA_VERZIA));
    for tabulka in ["behy", "polozky", "monstra", "modifikatory_mapy", "ceny",
                    "taktiky", "taktiky_priradenie"] {
        vysledok.insert(tabulka.to_string(), exportuj_tabulku(conn, tabulka)?);
    }
    Ok(serde_json::Value::Object(vysledok))
}

/// Vráti aktuálnu verziu schémy uloženú v `schema_info` — na diagnostiku
/// a testy migrácie.
pub fn precitaj_verziu(conn: &Connection) -> rusqlite::Result<Option<i32>> {
    conn.query_row("SELECT verzia FROM schema_info WHERE id = 1", [], |r| r.get(0))
        .optional()
}

/// Stav denného uploadu daného súboru (TLI 14) — `None`, keď sa tento súbor
/// ešte nikdy nenahrával. MUSÍ sa čítať z disku pri každom kole uploadu,
/// nikdy len z pamäte: appka po reštarte nesmie mať dojem, že ešte nič
/// nenahrala, nad súborom, ktorý server už čiastočne má (viď dokumentácia
/// `upload::StavUploaduSuboru`).
pub fn nacitaj_upload_stav(
    conn: &Connection,
    subor: &str,
) -> rusqlite::Result<Option<super::upload::StavUploaduSuboru>> {
    conn.query_row(
        "SELECT subor, nahrana_pozicia, odtlacok_zaciatku FROM tracker_upload_stav
         WHERE subor = ?1",
        params![subor],
        |r| {
            Ok(super::upload::StavUploaduSuboru {
                nazov: r.get(0)?,
                nahrana_pozicia: r.get::<_, i64>(1)? as u64,
                odtlacok_zaciatku: r.get(2)?,
            })
        },
    )
    .optional()
}

/// Uloží stav po úspešnom uploade — volá sa PO KAŽDOM kole, nie len na konci
/// dňa, aby reštart appky uprostred veľkého dobiehania (`upload::KUSOK_BAJTOV`
/// po kole) nestratil už potvrdený pokrok.
pub fn uloz_upload_stav(
    conn: &Connection,
    stav: &super::upload::StavUploaduSuboru,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO tracker_upload_stav (subor, nahrana_pozicia, odtlacok_zaciatku)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(subor) DO UPDATE SET
            nahrana_pozicia = excluded.nahrana_pozicia,
            odtlacok_zaciatku = excluded.odtlacok_zaciatku",
        params![stav.nazov, stav.nahrana_pozicia as i64, stav.odtlacok_zaciatku],
    )?;
    Ok(())
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::tracker::stav::{BossZaznam, CanvasZaznam, ModifikatorZaznam, MonstrumZaznam, Zmena};
    use std::collections::BTreeMap;

    fn cas(sekunda: u8) -> Option<Cas> {
        Some(Cas { rok: 2026, mesiac: 10, den: 16, hodina: 10, minuta: 0, sekunda, ms: 0 })
    }

    fn prazdny_beh(id: u64, zona: &str, zaciatok_s: u8) -> Beh {
        Beh {
            id,
            zona: zona.to_string(),
            je_hub: false,
            level_uid: None,
            level_typ: None,
            level_id: None,
            zaciatok: cas(zaciatok_s),
            koniec: cas(zaciatok_s + 10),
            otvorenie_bez_spotreby: false,
            dropy: BTreeMap::new(),
            vstupy: BTreeMap::new(),
            vsetky_zmeny: vec![],
            monstra: vec![],
            modifikatory: vec![],
            bossovia: vec![],
            canvas_akcie: vec![],
        }
    }

    #[test]
    fn migracia_je_idempotentna_a_nastavi_verziu() {
        let conn = otvor_v_pamati().unwrap();
        assert_eq!(precitaj_verziu(&conn).unwrap(), Some(SCHEMA_VERZIA));
        // Druhé zavolanie nesmie spadnúť ani nič pokaziť.
        migruj(&conn).unwrap();
        assert_eq!(precitaj_verziu(&conn).unwrap(), Some(SCHEMA_VERZIA));
    }

    #[test]
    fn ulozenie_behu_a_jeho_precitanie() {
        let mut conn = otvor_v_pamati().unwrap();
        let mut beh = prazdny_beh(1, "MapaX", 0);
        beh.monstra.push(MonstrumZaznam { npc_id: 1, rarita: 1, affix: None, cas: cas(1) });
        beh.monstra.push(MonstrumZaznam { npc_id: 2, rarita: 3, affix: Some(9), cas: cas(2) });
        beh.monstra.push(MonstrumZaznam { npc_id: 3, rarita: 3, affix: None, cas: cas(3) });
        beh.modifikatory.push(ModifikatorZaznam {
            danger: Some(1), kind: Some(2), skupina: None, seat: None, rand_affix_id: Some(5), cas: cas(1),
        });
        beh.bossovia.push(BossZaznam { aktivny: true, cas: cas(5) });
        beh.canvas_akcie.push(CanvasZaznam { typ: "level_up_start".to_string(), cas: cas(6) });
        beh.vsetky_zmeny.push(Zmena { polozka: 400021, mnozstvo: 3, dovod: Dovod::Drop, cas: cas(2) });

        let identita = HracSnimka {
            player_id: Some("P1".to_string()),
            season_id: Some("1501".to_string()),
            hero_id: Some("1800".to_string()),
            ..Default::default()
        };
        let ocenenie = OceneneHodnoty { cisty_zisk_fe: Some(42.5), fe_za_hodinu: Some(100.0), ..Default::default() };

        let (beh_id, bol_novy) = uloz_beh(&mut conn, &beh, &identita, &ocenenie).unwrap();
        assert!(beh_id > 0);
        assert!(bol_novy);

        let riadky = vyhladaj_behy(&conn, &FilterBehov::default()).unwrap();
        assert_eq!(riadky.len(), 1);
        assert_eq!(riadky[0].zona, "MapaX");
        assert_eq!(riadky[0].pocet_monster, 3);
        assert_eq!(riadky[0].pocet_rares_a_bossov, 2);
        assert_eq!(riadky[0].cisty_zisk_fe, Some(42.5));

        let podla_rarity = monstra_podla_rarity(&conn, beh_id).unwrap();
        assert_eq!(podla_rarity, vec![(1, 1), (3, 2)]);
    }

    #[test]
    fn ulozenie_toho_isteho_behu_dvakrat_sa_nezdvoji() {
        let mut conn = otvor_v_pamati().unwrap();
        let mut beh = prazdny_beh(1, "MapaX", 0);
        beh.monstra.push(MonstrumZaznam { npc_id: 1, rarita: 1, affix: None, cas: cas(1) });
        let identita = HracSnimka { player_id: Some("P1".to_string()), season_id: Some("1501".to_string()), ..Default::default() };
        let ocenenie = OceneneHodnoty::default();

        let (id1, novy1) = uloz_beh(&mut conn, &beh, &identita, &ocenenie).unwrap();
        let (id2, novy2) = uloz_beh(&mut conn, &beh, &identita, &ocenenie).unwrap();
        assert_eq!(id1, id2, "ten istý beh (rovnaký player/season/zona/zaciatok) má vrátiť to isté ID");
        assert!(novy1);
        assert!(!novy2);

        let riadky = vyhladaj_behy(&conn, &FilterBehov::default()).unwrap();
        assert_eq!(riadky.len(), 1, "beh sa nesmie zdvojiť v tabuľke behy");
        assert_eq!(riadky[0].pocet_monster, 1, "ani monštrá sa nesmú zdvojiť pri opakovanom importe");
    }

    #[test]
    fn dedup_funguje_aj_bez_rozpoznanej_identity() {
        // Krátka session bez `+player+...` bloku (bežné, log-format.md §9) —
        // identita je kompletne `None`. Dedup sa nesmie spoliehať na to, že
        // dve `NULL` hodnoty v SQLite `UNIQUE` sú "rovnaké" (nie sú).
        let mut conn = otvor_v_pamati().unwrap();
        let beh = prazdny_beh(1, "MapaX", 0);
        let identita = HracSnimka::default();
        let ocenenie = OceneneHodnoty::default();

        let (id1, novy1) = uloz_beh(&mut conn, &beh, &identita, &ocenenie).unwrap();
        let (id2, novy2) = uloz_beh(&mut conn, &beh, &identita, &ocenenie).unwrap();
        assert_eq!(id1, id2);
        assert!(novy1);
        assert!(!novy2);

        let riadky = vyhladaj_behy(&conn, &FilterBehov::default()).unwrap();
        assert_eq!(riadky.len(), 1);
    }

    // ─── TLI 19 bug E: záloha identity pre živé čítanie ─────────────────

    #[test]
    fn posledna_znama_identita_prazdna_db_vracia_none() {
        let conn = otvor_v_pamati().unwrap();
        assert!(posledna_znama_identita(&conn).unwrap().is_none());
    }

    #[test]
    fn posledna_znama_identita_preskoci_behy_bez_identity_a_vrati_najnovsiu() {
        let mut conn = otvor_v_pamati().unwrap();
        let ocenenie = OceneneHodnoty::default();

        let s_identitou = HracSnimka {
            player_id: Some("947466339310800896".to_string()),
            season_id: Some("1501".to_string()),
            hero_id: Some("1800".to_string()),
            ..Default::default()
        };
        uloz_beh(&mut conn, &prazdny_beh(1, "Mapa1", 0), &s_identitou, &ocenenie).unwrap();

        // TLI 19 bug E: živé behy bez identity (chýbajúci GetPlayerData v
        // danej session) nesmú zahodiť/prebiť to, čo sa už raz zistilo.
        uloz_beh(&mut conn, &prazdny_beh(2, "Mapa2", 20), &HracSnimka::default(), &ocenenie).unwrap();

        let identita = posledna_znama_identita(&conn).unwrap().expect("mala sa nájsť identita z behu 1");
        assert_eq!(identita.player_id.as_deref(), Some("947466339310800896"));
        assert_eq!(identita.season_id.as_deref(), Some("1501"));
        assert_eq!(identita.hero_id.as_deref(), Some("1800"));
    }

    #[test]
    fn rozni_hraci_na_tej_istej_zone_a_case_su_dva_rozne_behy() {
        let mut conn = otvor_v_pamati().unwrap();
        let beh = prazdny_beh(1, "MapaX", 0);
        let ocenenie = OceneneHodnoty::default();

        let identita_a = HracSnimka { player_id: Some("A".to_string()), season_id: Some("1".to_string()), ..Default::default() };
        let identita_b = HracSnimka { player_id: Some("B".to_string()), season_id: Some("1".to_string()), ..Default::default() };

        uloz_beh(&mut conn, &beh, &identita_a, &ocenenie).unwrap();
        uloz_beh(&mut conn, &beh, &identita_b, &ocenenie).unwrap();

        let riadky = vyhladaj_behy(&conn, &FilterBehov::default()).unwrap();
        assert_eq!(riadky.len(), 2);
    }

    #[test]
    fn filter_podla_zony_a_minimalneho_zisku() {
        let mut conn = otvor_v_pamati().unwrap();
        let identita = HracSnimka::default();

        let beh_a = prazdny_beh(1, "MapaA", 0);
        uloz_beh(&mut conn, &beh_a, &identita, &OceneneHodnoty { cisty_zisk_fe: Some(10.0), ..Default::default() }).unwrap();
        let beh_b = prazdny_beh(2, "MapaB", 20);
        uloz_beh(&mut conn, &beh_b, &identita, &OceneneHodnoty { cisty_zisk_fe: Some(100.0), ..Default::default() }).unwrap();

        let vysledok = vyhladaj_behy(&conn, &FilterBehov { zona: Some("MapaB".to_string()), ..Default::default() }).unwrap();
        assert_eq!(vysledok.len(), 1);
        assert_eq!(vysledok[0].zona, "MapaB");

        let vysledok2 = vyhladaj_behy(&conn, &FilterBehov { min_zisk_fe: Some(50.0), ..Default::default() }).unwrap();
        assert_eq!(vysledok2.len(), 1);
        assert_eq!(vysledok2[0].zona, "MapaB");
    }

    #[test]
    fn hubovy_beh_sa_do_vyhladavania_nezapocitava() {
        let mut conn = otvor_v_pamati().unwrap();
        let mut hub = prazdny_beh(1, "Town", 0);
        hub.je_hub = true;
        uloz_beh(&mut conn, &hub, &HracSnimka::default(), &OceneneHodnoty::default()).unwrap();
        let riadky = vyhladaj_behy(&conn, &FilterBehov::default()).unwrap();
        assert!(riadky.is_empty());
    }

    #[test]
    fn ceny_zostatky_a_raw_sa_ukladaju_idempotentne() {
        let conn = otvor_v_pamati().unwrap();
        uloz_cenu(&conn, &ZaznamCeny { polozka: 3004, cena_fe: 0.21, median_fe: Some(0.3), pocet_ponuk: Some(4), sezona: None, zdroj: "vlastny_trh".into(), cas: cas(1).map(|c| c.text()), zapisane: 1000 }).unwrap();
        uloz_cenu(&conn, &ZaznamCeny { polozka: 3004, cena_fe: 0.21, median_fe: Some(0.3), pocet_ponuk: Some(4), sezona: None, zdroj: "vlastny_trh".into(), cas: cas(1).map(|c| c.text()), zapisane: 1000 }).unwrap(); // duplicitný import
        let pocet: i64 = conn.query_row("SELECT COUNT(*) FROM ceny", [], |r| r.get(0)).unwrap();
        assert_eq!(pocet, 1);

        uloz_zostatok(&conn, "Coin_810", 500, cas(1).as_ref()).unwrap();
        uloz_zostatok(&conn, "Coin_810", 500, cas(1).as_ref()).unwrap();
        let pocet_z: i64 = conn.query_row("SELECT COUNT(*) FROM zostatky", [], |r| r.get(0)).unwrap();
        assert_eq!(pocet_z, 1);

        uloz_raw(&conn, None, "čosi zaujímavé", cas(1).as_ref()).unwrap();
        uloz_raw(&conn, None, "čosi zaujímavé", cas(1).as_ref()).unwrap();
        let pocet_raw: i64 = conn.query_row("SELECT COUNT(*) FROM raw_udalosti", [], |r| r.get(0)).unwrap();
        assert_eq!(pocet_raw, 1);
    }

    #[test]
    fn build_snapshot_sa_neuklada_ked_je_uplne_prazdny() {
        let mut conn = otvor_v_pamati().unwrap();
        let beh = prazdny_beh(1, "MapaX", 0);
        let (beh_id, _) = uloz_beh(&mut conn, &beh, &HracSnimka::default(), &OceneneHodnoty::default()).unwrap();
        uloz_build_snapshot(&conn, beh_id, &HracSnimka::default(), None, None, None, None).unwrap();
        let pocet: i64 = conn.query_row("SELECT COUNT(*) FROM build_snapshoty", [], |r| r.get(0)).unwrap();
        assert_eq!(pocet, 0);

        uloz_build_snapshot(&conn, beh_id, &HracSnimka::default(), Some("DROPPS"), Some(100.0), None, cas(1).as_ref()).unwrap();
        let pocet2: i64 = conn.query_row("SELECT COUNT(*) FROM build_snapshoty", [], |r| r.get(0)).unwrap();
        assert_eq!(pocet2, 1);
    }

    #[test]
    fn export_csv_a_json_maju_hlavicku_a_su_parsovatelne() {
        let riadky = vec![BehRiadok {
            id: 1,
            zona: "MapaX".to_string(),
            podpis_vstupov: "400021x1".to_string(),
            zaciatok_cas: Some("2026-10-16 10:00:00".to_string()),
            trvanie_ms: Some(60_000),
            cisty_zisk_fe: Some(12.5),
            fe_za_hodinu: Some(750.0),
            pocet_monster: 10,
            pocet_rares_a_bossov: 2,
        }];
        let csv = export_csv(&riadky);
        assert!(csv.starts_with("id;zona;"));
        assert!(csv.contains("MapaX"));

        let json = export_json(&riadky);
        let spat: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(spat.len(), 1);
        assert_eq!(spat[0]["zona"], "MapaX");
    }

    #[test]
    fn data_pre_taktiky_cita_posledny_build_snapshot_a_filtruje_hub() {
        let mut conn = otvor_v_pamati().unwrap();
        let identita = HracSnimka { hero_id: Some("1800".to_string()), ..Default::default() };

        let mut hub = prazdny_beh(1, "Town", 0);
        hub.je_hub = true;
        uloz_beh(&mut conn, &hub, &identita, &OceneneHodnoty::default()).unwrap();

        let beh = prazdny_beh(2, "MapaX", 10);
        let (beh_id, _) = uloz_beh(
            &mut conn,
            &beh,
            &identita,
            &OceneneHodnoty { cisty_zisk_fe: Some(50.0), hodnota_vstupov_fe: Some(5.0), fe_za_hodinu: Some(300.0), ..Default::default() },
        )
        .unwrap();
        // Dva snapshoty za sebou — má sa zobrať ten NOVŠÍ (pact sa zmenil v priebehu behu).
        uloz_build_snapshot(&conn, beh_id, &identita, Some("PACT_STARY"), None, None, cas(11).as_ref()).unwrap();
        uloz_build_snapshot(&conn, beh_id, &identita, Some("PACT_NOVY"), None, None, cas(12).as_ref()).unwrap();

        let riadky = data_pre_taktiky(&conn, &FilterTaktik::default()).unwrap();
        assert_eq!(riadky.len(), 1, "hubový beh sa do taktík nepočíta");
        assert_eq!(riadky[0].zona, "MapaX");
        assert_eq!(riadky[0].hero_id.as_deref(), Some("1800"));
        assert_eq!(riadky[0].pact_konfiguracia.as_deref(), Some("PACT_NOVY"));
    }

    #[test]
    fn data_pre_taktiky_filter_podla_hrdinu_a_ligy() {
        let mut conn = otvor_v_pamati().unwrap();
        let hrdina_a = HracSnimka { hero_id: Some("A".to_string()), season_id: Some("S1".to_string()), ..Default::default() };
        let hrdina_b = HracSnimka { hero_id: Some("B".to_string()), season_id: Some("S1".to_string()), ..Default::default() };

        uloz_beh(&mut conn, &prazdny_beh(1, "MapaX", 0), &hrdina_a, &OceneneHodnoty::default()).unwrap();
        uloz_beh(&mut conn, &prazdny_beh(2, "MapaX", 20), &hrdina_b, &OceneneHodnoty::default()).unwrap();

        let filter = FilterTaktik { hero_id: Some("A".to_string()), ..Default::default() };
        let riadky = data_pre_taktiky(&conn, &filter).unwrap();
        assert_eq!(riadky.len(), 1);
        assert_eq!(riadky[0].hero_id.as_deref(), Some("A"));
    }

    #[test]
    fn modifikator_podpisy_su_triedene_a_deterministicke() {
        let mut conn = otvor_v_pamati().unwrap();
        let mut beh = prazdny_beh(1, "MapaX", 0);
        // Vloží sa v OPAČNOM poradí, než v akom má vyjsť zotriedený podpis.
        beh.modifikatory.push(ModifikatorZaznam {
            danger: Some(5), kind: Some(1), skupina: None, seat: None, rand_affix_id: None, cas: cas(2),
        });
        beh.modifikatory.push(ModifikatorZaznam {
            danger: Some(1), kind: Some(9), skupina: None, seat: None, rand_affix_id: None, cas: cas(1),
        });
        let (beh_id, _) = uloz_beh(&mut conn, &beh, &HracSnimka::default(), &OceneneHodnoty::default()).unwrap();

        let podpisy = modifikator_podpisy(&conn).unwrap();
        assert_eq!(podpisy.get(&beh_id).unwrap(), "1.9..|5.1..");
    }

    #[test]
    fn ulozenie_a_nacitanie_priradeni_taktik() {
        let conn = otvor_v_pamati().unwrap();
        let id = ulozit_taktiku(&conn, &["podpisA".to_string(), "podpisB".to_string()], "Steel Forge farm T7").unwrap();
        assert!(id > 0);

        let priradenia = nacitaj_priradenia_taktik(&conn).unwrap();
        assert_eq!(priradenia.get("podpisA"), Some(&(id, "Steel Forge farm T7".to_string())));
        assert_eq!(priradenia.get("podpisB"), Some(&(id, "Steel Forge farm T7".to_string())));

        // Presun podpisu do inej (novej) taktiky prepíše predchádzajúce priradenie.
        let id2 = ulozit_taktiku(&conn, &["podpisA".to_string()], "Iná taktika").unwrap();
        let priradenia2 = nacitaj_priradenia_taktik(&conn).unwrap();
        assert_eq!(priradenia2.get("podpisA"), Some(&(id2, "Iná taktika".to_string())));
        assert_eq!(priradenia2.get("podpisB"), Some(&(id, "Steel Forge farm T7".to_string())));
    }

    #[test]
    fn hash_riadku_je_stabilny_a_citlivy_na_zmenu() {
        let a = hash_riadku("rovnaky text");
        let b = hash_riadku("rovnaky text");
        let c = hash_riadku("iny text");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ── export_pre_upload (TLI 14, 1/N) ─────────────────────────────────────

    #[test]
    fn export_pre_upload_obsahuje_presne_zadane_tabulky() {
        let conn = otvor_v_pamati().unwrap();
        let export = export_pre_upload(&conn).unwrap();
        let obj = export.as_object().unwrap();
        for tabulka in ["schema_verzia", "behy", "polozky", "monstra",
                        "modifikatory_mapy", "ceny", "taktiky", "taktiky_priradenie"] {
            assert!(obj.contains_key(tabulka), "chýba kľúč {tabulka}");
        }
        // Presne tieto a žiadne iné — `zostatky`/`bossovia`/`frozen_canvas_akcie`/
        // `build_snapshoty`/`raw_udalosti` zadanie TLI 14 nežiada.
        assert_eq!(obj.len(), 8);
        assert_eq!(obj["schema_verzia"], serde_json::json!(SCHEMA_VERZIA));
        assert_eq!(obj["behy"], serde_json::json!([]));
    }

    #[test]
    fn export_pre_upload_vracia_realne_ulozene_riadky() {
        let mut conn = otvor_v_pamati().unwrap();
        let mut beh = prazdny_beh(1, "Oaza", 0);
        beh.monstra.push(MonstrumZaznam { npc_id: 42, rarita: 3, affix: None, cas: cas(1) });
        beh.modifikatory.push(ModifikatorZaznam {
            danger: Some(1), kind: Some(9), skupina: None, seat: None,
            rand_affix_id: None, cas: cas(1),
        });
        let identita = HracSnimka::default();
        let ocenenie = OceneneHodnoty {
            hodnota_dropov_fe: Some(100.0), hodnota_vstupov_fe: Some(10.0),
            cisty_zisk_fe: Some(90.0), fe_za_hodinu: Some(32400.0),
        };
        let (beh_id, _) = uloz_beh(&mut conn, &beh, &identita, &ocenenie).unwrap();
        uloz_cenu(&conn, &ZaznamCeny { polozka: 100300, cena_fe: 1.5, median_fe: None, pocet_ponuk: None, sezona: None, zdroj: "vlastny_trh".into(), cas: cas(2).map(|c| c.text()), zapisane: 2000 }).unwrap();
        ulozit_taktiku(&conn, &["podpisX".to_string()], "Moja taktika").unwrap();

        let export = export_pre_upload(&conn).unwrap();
        let behy = export["behy"].as_array().unwrap();
        assert_eq!(behy.len(), 1);
        assert_eq!(behy[0]["id"], serde_json::json!(beh_id));
        assert_eq!(behy[0]["zona"], serde_json::json!("Oaza"));
        assert_eq!(behy[0]["cisty_zisk_fe"], serde_json::json!(90.0));

        let monstra = export["monstra"].as_array().unwrap();
        assert_eq!(monstra.len(), 1);
        assert_eq!(monstra[0]["npc_id"], serde_json::json!(42));

        assert_eq!(export["modifikatory_mapy"].as_array().unwrap().len(), 1);
        assert_eq!(export["ceny"].as_array().unwrap().len(), 1);
        assert_eq!(export["taktiky"].as_array().unwrap().len(), 1);
        assert_eq!(export["taktiky"][0]["nazov"], serde_json::json!("Moja taktika"));
        // Sanity: serializuje sa to naspäť do platného JSON reťazca (presne
        // to, čo `src-tauri` pred gzipom a odoslaním zavolá).
        let text = serde_json::to_string(&export).unwrap();
        assert!(text.contains("Moja taktika"));
    }

    // ── upload_stav (TLI 14) ─────────────────────────────────────────────

    #[test]
    fn upload_stav_neexistujuceho_suboru_je_none() {
        let conn = otvor_v_pamati().unwrap();
        assert_eq!(nacitaj_upload_stav(&conn, "UE_game.log").unwrap(), None);
    }

    #[test]
    fn upload_stav_sa_ulozi_a_precita_spat() {
        let conn = otvor_v_pamati().unwrap();
        let stav = super::super::upload::StavUploaduSuboru {
            nazov: "UE_game.log".to_string(),
            nahrana_pozicia: 12345,
            odtlacok_zaciatku: "deadbeef".to_string(),
        };
        uloz_upload_stav(&conn, &stav).unwrap();
        assert_eq!(nacitaj_upload_stav(&conn, "UE_game.log").unwrap(), Some(stav));
    }

    #[test]
    fn upload_stav_druhe_ulozenie_prepise_prve_bez_duplicity() {
        let conn = otvor_v_pamati().unwrap();
        let mut stav = super::super::upload::StavUploaduSuboru {
            nazov: "UE_game.log".to_string(),
            nahrana_pozicia: 100,
            odtlacok_zaciatku: "aaa".to_string(),
        };
        uloz_upload_stav(&conn, &stav).unwrap();
        stav.nahrana_pozicia = 200;
        stav.odtlacok_zaciatku = "bbb".to_string();
        uloz_upload_stav(&conn, &stav).unwrap();

        let nacitany = nacitaj_upload_stav(&conn, "UE_game.log").unwrap().unwrap();
        assert_eq!(nacitany.nahrana_pozicia, 200);
        assert_eq!(nacitany.odtlacok_zaciatku, "bbb");

        let pocet: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracker_upload_stav", [], |r| r.get(0))
            .unwrap();
        assert_eq!(pocet, 1);
    }

    #[test]
    fn upload_stav_dvoch_ruznych_suborov_su_nezavisle() {
        let conn = otvor_v_pamati().unwrap();
        uloz_upload_stav(&conn, &super::super::upload::StavUploaduSuboru {
            nazov: "UE_game.log".to_string(),
            nahrana_pozicia: 10,
            odtlacok_zaciatku: "a".to_string(),
        }).unwrap();
        uloz_upload_stav(&conn, &super::super::upload::StavUploaduSuboru {
            nazov: "UE_game-backup-2026.10.14.log".to_string(),
            nahrana_pozicia: 20,
            odtlacok_zaciatku: "b".to_string(),
        }).unwrap();

        assert_eq!(nacitaj_upload_stav(&conn, "UE_game.log").unwrap().unwrap().nahrana_pozicia, 10);
        assert_eq!(
            nacitaj_upload_stav(&conn, "UE_game-backup-2026.10.14.log").unwrap().unwrap().nahrana_pozicia,
            20
        );
    }
}
