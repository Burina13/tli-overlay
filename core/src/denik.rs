//! Čisto rozhodovacia časť lokálneho rotovaného denníka appky (TLI 24,
//! `2026-10-17-1238-mcp.md` bod 4) — appka doteraz nemala ŽIADEN súbor
//! s priebehom (hotkey, snímka, HTTP, upload), takže keď „appka nič
//! nespraví", nedalo sa zistiť ani to, či hotkey vôbec dorazil do appky.
//!
//! Samotné čítanie/zápis/premenovanie súborov rieši `src-tauri/src/denik.rs`
//! (potrebuje disk) — sem patrí len to, čo sa dá overiť `cargo test` bez
//! dotyku s diskom: kedy už treba rotovať a v akom poradí sa majú súbory
//! premenovať, aby sa žiadny obsah nestratil ani neprepísal sám sebou.

/// Najväčšia veľkosť AKTUÁLNEHO log súboru, kým sa pri ďalšom zápise
/// nerozbehne rotácia. Zadanie žiada „max 5×2 MB" — 2 MB je táto hranica.
pub const MAX_VELKOST_BAJTOV: u64 = 2 * 1024 * 1024;

/// Koľko súborov má denník držať SPOLU (aktuálny + staršie zálohy).
/// Zadanie žiada „max 5×2 MB" — teda 5 súborov vrátane aktuálneho.
pub const MAX_SUBOROV: u32 = 5;

/// Meno aktuálneho (dopisovaného) súboru.
pub const NAZOV_AKTUALNEHO: &str = "tli-overlay.log";

/// Meno jednej zálohy podľa poradia — `.1` je najčerstvejšia záloha.
pub fn nazov_zalohy(index: u32) -> String {
    format!("{NAZOV_AKTUALNEHO}.{index}")
}

/// Má sa pred ďalším zápisom rotovať? Rozhoduje sa podľa veľkosti súboru
/// PRED pripísaním nového riadku — rotácia teda môže aktuálny súbor nechať
/// tesne nad hranicou (o dĺžku jedného riadku), čo je v poriadku, lebo ide
/// o diagnostiku, nie o presný strop.
pub fn ma_sa_rotovat(aktualna_velkost: u64) -> bool {
    aktualna_velkost >= MAX_VELKOST_BAJTOV
}

/// Poradie premenovaní pri rotácii, OD NAJVYŠŠIEHO INDEXU — inak by sa
/// `.1` premenovalo na `.2` skôr, než sa pôvodné `.2` stihlo odsunúť na `.3`,
/// a obsah by sa prepísal sám sebou. Vracia `(odkiaľ, kam)`; posledný
/// index (`MAX_SUBOROV - 1`) sa nikam nepremenúva — len sa pred touto
/// postupnosťou zmaže, to si volajúci rieši sám (potrebuje vedieť, či
/// súbor na danom mieste vôbec existuje, čo táto čisto logická funkcia
/// nevie).
///
/// Príklad pri `MAX_SUBOROV = 5` (zálohy `.1`..`.4`):
/// `[(".3",".4"), (".2",".3"), (".1",".2"), ("tli-overlay.log",".1")]`.
pub fn poradie_premenovani(pocet_suborov: u32) -> Vec<(String, String)> {
    if pocet_suborov == 0 {
        return vec![];
    }
    let pocet_zaloh = pocet_suborov - 1;
    let mut plan = Vec::new();
    for index in (1..pocet_zaloh).rev() {
        plan.push((nazov_zalohy(index), nazov_zalohy(index + 1)));
    }
    if pocet_zaloh > 0 {
        plan.push((NAZOV_AKTUALNEHO.to_string(), nazov_zalohy(1)));
    }
    plan
}

/// Meno súboru, ktorý treba pred rotáciou ZMAZAŤ (najstaršia záloha, ktorá
/// by sa inak premenovaním vysunula mimo stráženého počtu). `None`, keď
/// podľa počtu súborov niet čo mazať (napr. `MAX_SUBOROV <= 1`).
pub fn subor_na_zmazanie(pocet_suborov: u32) -> Option<String> {
    if pocet_suborov < 2 {
        return None;
    }
    Some(nazov_zalohy(pocet_suborov - 1))
}

/// Jeden riadok denníka. Časová značka je zámerne SUROVÝ unixový čas (rovnaký
/// vzor ako existujúci `okna.log`, `stav::zapis_chybu_okna`) — appka nikde
/// inde dátum/čas needekóduje (žiadna závislosť na `chrono`), a surový
/// unixový čas sa aj tak dá kedykoľvek prepočítať pri čítaní.
pub fn riadok(cas_unix: u64, kontext: &str, text: &str) -> String {
    // Novým riadkom v `text` by appka rozbila jeden riadok denníka na viac —
    // najmä chybové hlášky zo servera môžu obsahovať preklady so zalomením.
    let text_bez_zalomenia = text.replace("\r\n", " ").replace(['\n', '\r'], " ");
    format!("{cas_unix} [{kontext}] {text_bez_zalomenia}")
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn ma_sa_rotovat_podla_hranice() {
        assert!(!ma_sa_rotovat(MAX_VELKOST_BAJTOV - 1));
        assert!(ma_sa_rotovat(MAX_VELKOST_BAJTOV));
        assert!(ma_sa_rotovat(MAX_VELKOST_BAJTOV + 1000));
    }

    #[test]
    fn poradie_premenovani_ide_od_najvyssieho_indexu() {
        let plan = poradie_premenovani(5);
        assert_eq!(
            plan,
            vec![
                (nazov_zalohy(3), nazov_zalohy(4)),
                (nazov_zalohy(2), nazov_zalohy(3)),
                (nazov_zalohy(1), nazov_zalohy(2)),
                (NAZOV_AKTUALNEHO.to_string(), nazov_zalohy(1)),
            ]
        );
    }

    #[test]
    fn poradie_premenovani_s_jednym_suborom_nerotuje_nic() {
        assert_eq!(poradie_premenovani(1), vec![]);
        assert_eq!(poradie_premenovani(0), vec![]);
    }

    #[test]
    fn subor_na_zmazanie_je_najstarsia_zaloha() {
        assert_eq!(subor_na_zmazanie(5).as_deref(), Some("tli-overlay.log.4"));
        assert_eq!(subor_na_zmazanie(1), None);
        assert_eq!(subor_na_zmazanie(0), None);
    }

    #[test]
    fn riadok_odstrani_zalomenia_aby_ostal_jeden_riadok() {
        let r = riadok(1700000000, "http", "chyba\nso zalomením\r\ninde");
        assert_eq!(r.matches('\n').count(), 0);
        assert!(r.starts_with("1700000000 [http] "));
        assert!(r.contains("chyba so zalomením inde"));
    }

    #[test]
    fn premenovania_sa_daju_aplikovat_bez_kolizie_na_simulovanom_disku() {
        // Simulácia disku ako mapa meno->obsah, aby sa dalo overiť, že
        // aplikovanie plánu V TOMTO PORADÍ nikdy neprepíše obsah, ktorý sa
        // ešte len chystá presunúť ďalej.
        use std::collections::HashMap;
        let mut disk: HashMap<String, String> = HashMap::new();
        disk.insert(NAZOV_AKTUALNEHO.to_string(), "najnovsie".to_string());
        disk.insert(nazov_zalohy(1), "stare-1".to_string());
        disk.insert(nazov_zalohy(2), "stare-2".to_string());
        disk.insert(nazov_zalohy(3), "stare-3".to_string());
        disk.insert(nazov_zalohy(4), "stare-4-zmaze-sa".to_string());

        if let Some(zmazat) = subor_na_zmazanie(MAX_SUBOROV) {
            disk.remove(&zmazat);
        }
        for (odkial, kam) in poradie_premenovani(MAX_SUBOROV) {
            if let Some(obsah) = disk.remove(&odkial) {
                disk.insert(kam, obsah);
            }
        }

        assert_eq!(disk.get(NAZOV_AKTUALNEHO), None);
        assert_eq!(disk.get(&nazov_zalohy(1)).map(String::as_str), Some("najnovsie"));
        assert_eq!(disk.get(&nazov_zalohy(2)).map(String::as_str), Some("stare-1"));
        assert_eq!(disk.get(&nazov_zalohy(3)).map(String::as_str), Some("stare-2"));
        assert_eq!(disk.get(&nazov_zalohy(4)).map(String::as_str), Some("stare-3"));
        assert_eq!(disk.len(), 4);
    }
}
