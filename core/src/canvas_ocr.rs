//! Čisté (bez WinRT) parsovanie textu, ktorý appka prečíta z obrazovky Frozen
//! Canvasu cez `Windows::Media::Ocr` (TLI 59b časť 2, `2026-10-20-0005-agent.md`,
//! bod 1). Samotné volanie OCR engine je vo `windows-rs` a žije v
//! `src-tauri/src/ocr.rs` za `#[cfg(windows)]` — nedá sa spustiť ani otestovať
//! na CT 111 (Linux). Toto je naopak tá časť, ktorú vieme SKUTOČNE overiť:
//! vzory textu nižšie sú odpísané priamo zo zadania TLI 59 („Fill a Adjacent
//! Cell with a Monster Paint (0/3)“, „Current Max Paint Level7“ — OCR bez
//! medzery pred číslom je bežný artefakt, keď je číslo vo vlastnom farebnom
//! rámiku hneď vedľa textu).
//!
//! **Čo tu NIE JE:** `kluc_riadok`/`kluc_stlpec` (bunka s výkričníkom, okolo
//! ktorej sa cieľ počíta) sa z textu cieľa vôbec neodvodí — to je vizuálna
//! vlastnosť bunky (TLI 59b, bod 2 toho istého zadania), nie text. Vracia sa
//! preto vždy `None` a dopĺňa ju až vyššia vrstva, ktorá spája OCR s
//! rozpoznaním obsahu buniek.
//!
//! **Smer (`ciel.smer`) sa odvodzuje len z jedinej overenej vety** „Fill a
//! <X> Cell …“ (presne tá, čo prišla zo screenshotu 15.10.2026 a má v
//! `canvas_solver.rs`/`canvas_solver.py` implementované pravidlo pre
//! `smer == "adjacent"`). Keď text tento tvar nemá, radšej sa vráti prázdny
//! reťazec než uhádnutý smer — prázdny smer solver bezpečne odmietne
//! (`navrhni_kroky` vyžaduje presne `"adjacent"`), kým vymyslený smer by sa
//! mohol omylom zhodovať.

use crate::canvas::CielMiesania;
use crate::canvas_tahy::{faza_z_nadpisu, VyberKariet, MAX_KARIET};

/// Nájde v (možno viacriadkovom) OCR texte riadok s cieľom miešania v tvare
/// `"<popis> (<aktuálne>/<požadované>)"` a voliteľný riadok s popisom efektu
/// hneď za ním, a poskladá z nich `CielMiesania`.
///
/// Vracia `None`, keď text neobsahuje žiadny riadok so vzorom `(číslo/číslo)`
/// — to znamená, že vôbec nejde o text cieľa (napr. prázdny panel alebo iná
/// obrazovka).
pub fn parsuj_ciel_text(text: &str) -> Option<CielMiesania> {
    let riadky: Vec<&str> = text.lines().map(str::trim).filter(|r| !r.is_empty()).collect();

    let (index, popis, pocet_aktualny, pocet_pozadovany) =
        riadky.iter().enumerate().find_map(|(i, riadok)| {
            let (popis, aktualny, pozadovany) = rozdel_popis_a_pocet(riadok)?;
            Some((i, popis, aktualny, pozadovany))
        })?;

    let efekt_popis = riadky.get(index + 1).map(|r| r.to_string()).unwrap_or_default();
    let smer = odvod_smer(&popis);

    Some(CielMiesania {
        smer,
        popis,
        efekt_popis,
        pocet_pozadovany,
        pocet_aktualny,
        kluc_riadok: None,
        kluc_stlpec: None,
    })
}

/// Rozdelí jeden riadok na `(popis, aktuálne, požadované)`, keď obsahuje
/// záverečnú zátvorku v tvare `(X/Y)` (medzery okolo `/` aj v zátvorke sú
/// tolerované — bežný OCR artefakt).
fn rozdel_popis_a_pocet(riadok: &str) -> Option<(String, i64, i64)> {
    let otvorka = riadok.rfind('(')?;
    let zatvorka = riadok.rfind(')')?;
    if zatvorka <= otvorka {
        return None;
    }

    let vnutro = &riadok[otvorka + 1..zatvorka];
    let (lavo, pravo) = vnutro.split_once('/')?;
    let aktualny: i64 = lavo.trim().parse().ok()?;
    let pozadovany: i64 = pravo.trim().parse().ok()?;

    // `split_whitespace().join(" ")` zároveň orezáva aj zráža viacnásobné
    // medzery z OCR šumu (`ciel_tolerantny_na_nadbytocne_medzery_z_ocr`).
    let popis = riadok[..otvorka].split_whitespace().collect::<Vec<_>>().join(" ");
    if popis.is_empty() {
        return None;
    }

    Some((popis, aktualny, pozadovany))
}

/// Z vety `"Fill a <X> Cell …"` vytiahne `<X>` a vráti ho malými písmenami
/// (`"adjacent"`). Keď veta tento tvar nemá, vráti prázdny reťazec — pozri
/// doc-komentár modulu vyššie, prečo je to bezpečnejšie než hádať.
fn odvod_smer(popis: &str) -> String {
    let slova: Vec<&str> = popis.split_whitespace().collect();
    let ocistene = |s: &str| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string();

    let index_cell = match slova.iter().position(|s| ocistene(s).eq_ignore_ascii_case("cell")) {
        Some(i) => i,
        None => return String::new(),
    };
    if index_cell == 0 {
        return String::new();
    }

    ocistene(slova[index_cell - 1]).to_lowercase()
}

/// Nájde v OCR texte frázu „Current Max Paint Level“ (porovnáva sa bez
/// ohľadu na veľkosť písmen) a vráti číslo, ktoré za ňou nasleduje — či už
/// hneď bez medzery (`"...Level7"`, najčastejší reálny prípad, lebo číslo je
/// v hre vo vlastnom farebnom rámiku hneď vedľa nápisu), cez medzeru/dvojbodku
/// (`"...Level: 7"`), alebo na ĎALŠOM riadku (`"...Level\n7"`, keď OCR
/// rozdelí nápis a číslo do dvoch samostatných čiar).
///
/// Vracia `None`, keď fráza chýba alebo za ňou nie je rozumne blízko žiadna
/// číslica (zámerne úzke okno pár oddeľovacích znakov — nech sa nenaviaže na
/// číslo z úplne inej časti panelu).
pub fn parsuj_max_level(text: &str) -> Option<i64> {
    const FRAZA: &str = "current max paint level";
    let maly = text.to_lowercase();
    let zaciatok = maly.find(FRAZA)?;
    // ASCII fráza aj pôvodný text majú pri `to_lowercase()` rovnakú dĺžku v
    // bajtoch (mení sa len veľkosť písmen, nie počet znakov) — preto sa dá
    // index z malého reťazca priamo použiť na pôvodný.
    let za_frazou = &text[zaciatok + FRAZA.len()..];

    // Preskoč najprv krátky úsek oddeľovačov (medzery, dvojbodka, nové
    // riadky) — najviac niekoľko znakov, nech sa pri chýbajúcom čísle
    // nenaviaže na číslicu zo vzdialeného textu.
    let bez_oddelovacov = za_frazou.trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    let oddelovacov_preskocenych = za_frazou.len() - bez_oddelovacov.len();
    if oddelovacov_preskocenych > 6 {
        return None;
    }

    let cislice: String = bez_oddelovacov.chars().take_while(|c| c.is_ascii_digit()).collect();
    if cislice.is_empty() {
        return None;
    }
    cislice.parse().ok()
}

/// Nájde v OCR texte ÚROVŇOVÝ ODZNAK jednej namaľovanej bunky platna ("Lv3",
/// "Lv 3", "LV: 12"…) a vráti jeho číslo (TLI 59b2b, bod 1 — level painta sa
/// pri rozpoznávaní jednej bunky číta z malého výrezu odznaku v jej pravom
/// dolnom rohu, OCR naň volá `src-tauri/src/ocr.rs`, parsovanie textu je tu).
/// Zámerne ÚZKY vzor — len `"lv"` bez ohľadu na veľkosť písmen a potom
/// najviac pár oddeľovačov pred číslom, rovnaký štýl ako `parsuj_max_level`
/// vyššie, nie všeobecné „nájdi prvé číslo v texte" (to by sa mohlo naviazať
/// na číslo z inej časti výrezu, keby OCR zachytil aj okolie odznaku).
pub fn parsuj_level_badge(text: &str) -> Option<i64> {
    let maly = text.to_lowercase();
    let zaciatok = maly.find("lv")?;
    let za_fraza = &text[zaciatok + 2..];

    let bez_oddelovacov = za_fraza.trim_start_matches(|c: char| c.is_whitespace() || c == ':' || c == '.');
    let oddelovacov_preskocenych = za_fraza.len() - bez_oddelovacov.len();
    if oddelovacov_preskocenych > 4 {
        return None;
    }

    let cislice: String = bez_oddelovacov.chars().take_while(|c| c.is_ascii_digit()).collect();
    if cislice.is_empty() {
        return None;
    }
    cislice.parse().ok()
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn ciel_jednoriadkovy_presny_vzor_zo_zadania() {
        let ciel = parsuj_ciel_text("Fill a Adjacent Cell with a Monster Paint (0/3)").unwrap();
        assert_eq!(ciel.popis, "Fill a Adjacent Cell with a Monster Paint");
        assert_eq!(ciel.smer, "adjacent");
        assert_eq!(ciel.pocet_aktualny, 0);
        assert_eq!(ciel.pocet_pozadovany, 3);
        assert_eq!(ciel.efekt_popis, "");
        assert_eq!(ciel.kluc_riadok, None);
        assert_eq!(ciel.kluc_stlpec, None);
    }

    #[test]
    fn ciel_s_efekt_popisom_na_dalsom_riadku() {
        let text = "Fill a Adjacent Cell with a Monster Paint (0/3)\n\
                     Level up the Adjacent Monster Paint by 1 level(s)";
        let ciel = parsuj_ciel_text(text).unwrap();
        assert_eq!(ciel.efekt_popis, "Level up the Adjacent Monster Paint by 1 level(s)");
        assert_eq!(ciel.smer, "adjacent");
    }

    #[test]
    fn ciel_rozbehnuty_stav_nenulove_aktualne() {
        let ciel = parsuj_ciel_text("Fill a Adjacent Cell with a Monster Paint (2/3)").unwrap();
        assert_eq!(ciel.pocet_aktualny, 2);
        assert_eq!(ciel.pocet_pozadovany, 3);
    }

    #[test]
    fn ciel_tolerantny_na_nadbytocne_medzery_z_ocr() {
        let ciel = parsuj_ciel_text("Fill  a   Adjacent  Cell with a Monster Paint   (0 / 3)").unwrap();
        assert_eq!(ciel.popis, "Fill a Adjacent Cell with a Monster Paint");
        assert_eq!(ciel.pocet_aktualny, 0);
        assert_eq!(ciel.pocet_pozadovany, 3);
        assert_eq!(ciel.smer, "adjacent");
    }

    #[test]
    fn ciel_bez_vety_fill_a_cell_necha_smer_prazdny() {
        // Zatiaľ overené len pre smer "adjacent" (canvas_solver.rs). Iný
        // text cieľa sa smie rozpoznať (počty áno), ale smer sa radšej
        // nechá prázdny než uhádnutý — solver ho bezpečne odmietne.
        let ciel = parsuj_ciel_text("Match Three Horizontal Paints in a Row (1/3)").unwrap();
        assert_eq!(ciel.pocet_aktualny, 1);
        assert_eq!(ciel.pocet_pozadovany, 3);
        assert_eq!(ciel.smer, "");
    }

    #[test]
    fn ciel_bez_zatvoriek_vracia_none() {
        assert_eq!(parsuj_ciel_text("Current Max Paint Level7"), None);
        assert_eq!(parsuj_ciel_text(""), None);
        assert_eq!(parsuj_ciel_text("Fill a Adjacent Cell with a Monster Paint"), None);
    }

    #[test]
    fn ciel_nepocitatelny_obsah_zatvoriek_vracia_none() {
        // Zátvorka bez '/' (napr. OCR šum) sa nemá vydávať za cieľ.
        assert_eq!(parsuj_ciel_text("Nejaký iný text (poznámka)"), None);
    }

    #[test]
    fn max_level_presny_vzor_zo_zadania_bez_medzery() {
        assert_eq!(parsuj_max_level("Current Max Paint Level7"), Some(7));
    }

    #[test]
    fn max_level_s_medzerou() {
        assert_eq!(parsuj_max_level("Current Max Paint Level 7"), Some(7));
    }

    #[test]
    fn max_level_s_dvojbodkou() {
        assert_eq!(parsuj_max_level("Current Max Paint Level: 12"), Some(12));
    }

    #[test]
    fn max_level_na_samostatnom_riadku() {
        assert_eq!(parsuj_max_level("Current Max Paint Level\n7"), Some(7));
    }

    #[test]
    fn max_level_case_insensitive() {
        assert_eq!(parsuj_max_level("CURRENT MAX PAINT LEVEL7"), Some(7));
    }

    #[test]
    fn max_level_v_okolitom_texte_panelu() {
        let text = "Frozen Canvas\nCurrent Max Paint Level7\nConfirm";
        assert_eq!(parsuj_max_level(text), Some(7));
    }

    #[test]
    fn max_level_viacciferne_cislo() {
        assert_eq!(parsuj_max_level("Current Max Paint Level15"), Some(15));
    }

    #[test]
    fn max_level_chybajuca_fraza_vracia_none() {
        assert_eq!(parsuj_max_level("Fill a Adjacent Cell with a Monster Paint (0/3)"), None);
        assert_eq!(parsuj_max_level(""), None);
    }

    #[test]
    fn max_level_fraza_bez_cisla_vracia_none() {
        assert_eq!(parsuj_max_level("Current Max Paint Level"), None);
    }

    #[test]
    fn max_level_cislo_prilis_daleko_sa_neberie() {
        // Viac než pár oddeľovacích znakov medzi frázou a číslom — radšej
        // žiadny výsledok než naviazanie na vzdialené, nesúvisiace číslo.
        assert_eq!(parsuj_max_level("Current Max Paint Level           7"), None);
    }

    #[test]
    fn level_badge_presny_vzor_zo_screenshotu() {
        assert_eq!(parsuj_level_badge("Lv3"), Some(3));
    }

    #[test]
    fn level_badge_s_medzerou_a_viacciferne() {
        assert_eq!(parsuj_level_badge("Lv 12"), Some(12));
        assert_eq!(parsuj_level_badge("LV: 7"), Some(7));
    }

    #[test]
    fn level_badge_case_insensitive() {
        assert_eq!(parsuj_level_badge("lv2"), Some(2));
        assert_eq!(parsuj_level_badge("LV2"), Some(2));
    }

    #[test]
    fn level_badge_chybajuce_cislo_vracia_none() {
        assert_eq!(parsuj_level_badge("Lv"), None);
        assert_eq!(parsuj_level_badge(""), None);
        assert_eq!(parsuj_level_badge("Level up!"), None);
    }

    #[test]
    fn level_badge_cislo_prilis_daleko_sa_neberie() {
        assert_eq!(parsuj_level_badge("Lv           3"), None);
    }
}

// ── Podobrazovka s kartami (TLI 71) ─────────────────────────────────────────


/// Text jednej karty z OCR → jeden riadok.
///
/// Hra text karty láme na dva riadky („[Reimagine] changes the Color" /
/// „Mixing Objective and Effect") a OCR vracia to isté lámanie — serveru
/// ale musí ísť jedna veta, inak sa nespáruje s katalógom efektov.
pub fn ocisti_text_karty(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Zloží výsledok lokálneho OCR podobrazovky: text nadpisu + text KAŽDEJ
/// karty zvlášť (appka ich OCRuje po jednom cez pevné boxy
/// `canvas_rozlozenie::karta_box01`, nie celú oblasť naraz — inak by sa
/// dvojriadkové karty nedali od seba odlíšiť).
///
/// Vracia `None`, keď nadpis nie je ani jedna zo známych podobrazoviek —
/// vtedy ide o bežné plátno a všetko ostatné sa správa presne ako pred
/// TLI 71. `None` vráti aj vtedy, keď sa nadpis rozpoznal, ale ani jedna
/// karta nemá použiteľný text: poslať serveru fázu bez kariet by znamenalo
/// nahradiť jednu bezcennú radu druhou.
///
/// Karty sa berú ZHORA a prvá prázdna ukončí zoznam — hra ich kreslí
/// súvisle od vrchu, takže diera v strede by znamenala chybné OCR, nie
/// vynechanú ponuku.
pub fn parsuj_fazu_kariet(nadpis_text: &str, texty_kariet: &[String]) -> Option<VyberKariet> {
    let faza = faza_z_nadpisu(nadpis_text)?;
    let mut karty: Vec<String> = Vec::new();
    for t in texty_kariet.iter().take(MAX_KARIET) {
        let cisty = ocisti_text_karty(t);
        // Pod 4 znaky to nie je text karty, ale OCR šum z pozadia hry.
        if cisty.chars().count() < 4 {
            break;
        }
        karty.push(cisty);
    }
    if karty.is_empty() {
        return None;
    }
    Some(VyberKariet { faza, karty })
}

#[cfg(test)]
mod testy_karty {
    use super::*;
    use crate::canvas_tahy::Faza;

    const NADPIS: &str = "Select the Brushstroke direction";
    const REIMAGINE: &str = "[Reimagine] changes the Color\nMixing Objective and Effect";
    const SPREAD: &str = "[Spread] takes effect on";

    #[test]
    fn dvojriadkova_karta_sa_zlepi_do_jednej_vety() {
        assert_eq!(
            ocisti_text_karty(REIMAGINE),
            "[Reimagine] changes the Color Mixing Objective and Effect"
        );
    }

    #[test]
    fn realna_obrazovka_zo_zadania_sa_precita() {
        let v = parsuj_fazu_kariet(NADPIS, &[REIMAGINE.to_string(), SPREAD.to_string()])
            .expect("nadpis aj karty sú tam");
        assert_eq!(v.faza, Faza::VyberTahu);
        assert_eq!(v.karty.len(), 2);
        assert_eq!(v.karty[1], "[Spread] takes effect on");
    }

    #[test]
    fn bezne_platno_nie_je_podobrazovka_s_kartami() {
        assert!(parsuj_fazu_kariet("Current Max Paint Level7", &["nieco".to_string()]).is_none());
        assert!(parsuj_fazu_kariet("", &[]).is_none());
    }

    #[test]
    fn rozpoznany_nadpis_bez_kariet_vrati_none() {
        assert!(parsuj_fazu_kariet(NADPIS, &[]).is_none());
        assert!(parsuj_fazu_kariet(NADPIS, &["  ".to_string()]).is_none());
    }

    #[test]
    fn prazdna_karta_ukonci_zoznam() {
        let v = parsuj_fazu_kariet(
            NADPIS,
            &[REIMAGINE.to_string(), "".to_string(), SPREAD.to_string()],
        )
        .unwrap();
        assert_eq!(v.karty.len(), 1);
    }

    #[test]
    fn druha_podobrazovka_sa_rozpozna_tiez() {
        let v = parsuj_fazu_kariet(
            "Select Brushstroke Effect",
            &["Randomly changes its own effect direction".to_string()],
        )
        .unwrap();
        assert_eq!(v.faza, Faza::VyberEfektu);
    }
}
