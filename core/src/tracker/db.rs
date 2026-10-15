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

use super::log::Cas;
use super::stav::{Beh, Dovod, HracSnimka};

/// Aktuálna verzia schémy. Zvyšuje sa pri KAŽDEJ zmene tabuliek; `migruj()`
/// podľa nej vie, ktoré kroky ešte treba spustiť.
pub const SCHEMA_VERZIA: i32 = 1;

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
        ",
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

/// Uloží jednu trhovú cenu (z `Sledovac::odober_ceny()`). `INSERT OR IGNORE`
/// kvôli dedup na `(config_base_id, cena_fe, cas)`.
pub fn uloz_cenu(conn: &Connection, config_base_id: u32, cena_fe: f64, cas: Option<&Cas>) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO ceny (config_base_id, cena_fe, cas) VALUES (?1,?2,?3)",
        params![config_base_id, cena_fe, cas.map(|c| c.text())],
    )?;
    Ok(())
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

/// Vráti aktuálnu verziu schémy uloženú v `schema_info` — na diagnostiku
/// a testy migrácie.
pub fn precitaj_verziu(conn: &Connection) -> rusqlite::Result<Option<i32>> {
    conn.query_row("SELECT verzia FROM schema_info WHERE id = 1", [], |r| r.get(0))
        .optional()
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
        uloz_cenu(&conn, 3004, 0.21, cas(1).as_ref()).unwrap();
        uloz_cenu(&conn, 3004, 0.21, cas(1).as_ref()).unwrap(); // duplicitný import
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
    fn hash_riadku_je_stabilny_a_citlivy_na_zmenu() {
        let a = hash_riadku("rovnaky text");
        let b = hash_riadku("rovnaky text");
        let c = hash_riadku("iny text");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
