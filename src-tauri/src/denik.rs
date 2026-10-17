//! Lokálny rotovaný denník appky (TLI 24, `2026-10-17-1238-mcp.md` bod 4) —
//! predtým appka nemala ŽIADEN súbor s priebehom behu (hotkey, snímka, HTTP,
//! upload), len `okna.log` s chybami pri práci s oknami. Keď u hráča appka
//! „nič nespraví", nedalo sa ani zistiť, či hotkey vôbec dorazil.
//!
//! Rozhodovanie (kedy rotovať, v akom poradí premenovať súbory) je v
//! `tli_overlay_core::denik` a má tam jednotkové testy bez dotyku s diskom —
//! tu je len samotné I/O. Zápis je VŽDY best-effort: chyba v logovaní nesmie
//! appku zhodiť ani zablokovať, je to diagnostika, nie dáta.
use std::io::Write;
use std::path::PathBuf;
use tli_overlay_core::denik as logika;

fn cesta_priecinka() -> PathBuf {
    let mut c = crate::stav::cesta_suboru();
    c.pop(); // preč "nastavenia.json", ostáva priečinok appky
    c.push("logs");
    c
}

fn cesta_suboru(nazov: &str) -> PathBuf {
    let mut c = cesta_priecinka();
    c.push(nazov);
    c
}

/// Pred zápisom rotuje, keď je aktuálny súbor nad hranicou. Mazanie/premenovanie
/// sú `best effort` — chýbajúci súbor v reťazci jednoducho preskočí.
fn rotuj_ak_treba(priecinok: &std::path::Path) {
    let aktualny = priecinok.join(logika::NAZOV_AKTUALNEHO);
    let Ok(meta) = std::fs::metadata(&aktualny) else {
        return; // súbor ešte neexistuje, niet čo rotovať
    };
    if !logika::ma_sa_rotovat(meta.len()) {
        return;
    }
    if let Some(zmazat) = logika::subor_na_zmazanie(logika::MAX_SUBOROV) {
        let _ = std::fs::remove_file(priecinok.join(zmazat));
    }
    for (odkial, kam) in logika::poradie_premenovani(logika::MAX_SUBOROV) {
        let _ = std::fs::rename(priecinok.join(&odkial), priecinok.join(&kam));
    }
}

/// Pripíše jeden riadok denníka. `kontext` je krátke slovo (`"hotkey"`,
/// `"okno"`, `"snimka"`, `"http"`, `"upload"`...), `text` je ľudská veta.
pub fn zapis(kontext: &str, text: &str) {
    let priecinok = cesta_priecinka();
    if std::fs::create_dir_all(&priecinok).is_err() {
        return;
    }
    rotuj_ak_treba(&priecinok);

    let cas = crate::tracker::unixovy_cas();
    let riadok = logika::riadok(cas, kontext, text);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cesta_suboru(logika::NAZOV_AKTUALNEHO))
    {
        let _ = writeln!(f, "{riadok}");
    }
}

/// Priečinok s logmi — pre tlačidlo „Otvoriť log" v Nastaveniach aj pre
/// prípadnú ďalšiu diagnostiku. Vytvorí ho, keď ešte neexistuje (appka sa
/// mohla spustiť a hneď skončiť bez jediného zápisu), nech „Otvoriť log"
/// vždy otvorí niečo, nie chybu „priečinok neexistuje".
pub fn cesta_na_otvorenie() -> PathBuf {
    let priecinok = cesta_priecinka();
    let _ = std::fs::create_dir_all(&priecinok);
    priecinok
}
