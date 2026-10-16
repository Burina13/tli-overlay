//! Denný upload surových logov a DB exportu na server (TLI 14, 1/N) — LEN
//! rozhodovanie, ktoré bajty sú nové od posledného uploadu, a parsovanie
//! odpovede servera. Rovnaký vzor ako `sync.rs` (TLI 11, krok 5): appka toto
//! volanie ešte NEMÁ zapojené do UI ani do pozadia (`BACKLOG.md` pod
//! „TLI 14 —"), tento modul je pripravený základ pre ďalšiu časť.
//!
//! Server kontrakt: `POST /api/tracker/upload`
//! (`docs/fakty/hry/torchlight-infinite/README.md` v ops repe, implementácia
//! `stacks/games/tli-web/app.py`). Samotné čítanie súboru, gzip kompresiu
//! a HTTP odoslanie rieši `src-tauri`, presne tak ako živé čítanie logu
//! rieši `src-tauri/src/tracker.rs` nad `chvost.rs`.

use serde::{Deserialize, Serialize};

/// Stav uploadu JEDNÉHO súboru — perzistovateľný, ale ZÁMERNE oddelený od
/// `chvost::StavChvosta` (živé čítanie): upload beží raz denne a navyše pri
/// ukončení hry, takže typicky zaostáva za živým čítaním a potrebuje vlastnú
/// pozíciu, nie tú istú.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StavUploaduSuboru {
    pub nazov: String,
    pub nahrana_pozicia: u64,
    /// Odtlačok prvých bajtov súboru v čase posledného uploadu. Prázdny
    /// reťazec znamená „ešte nikdy nenahrané" — vtedy sa rotácia nerieši,
    /// pošle sa jednoducho všetko od nuly.
    pub odtlacok_zaciatku: String,
}

/// FNV-1a nad presne tými bajtami, čo sa mu dajú (bez orezania na 256 TU —
/// o to sa stará volajúci, pozri `odtlacok_okna`). Rovnaký algoritmus a dôvod
/// ako `kluc_behu` v `sync.rs` (žiadna kryptografická závislosť netreba).
fn fnv1a(bajty: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    let prime: u64 = 0x100000001b3;
    for b in bajty {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(prime);
    }
    format!("{hash:016x}")
}

/// Odtlačok OKNA s PEVNE DANOU dĺžkou `okno_do` (najviac 256 bajtov).
///
/// Dôležité: dĺžka okna sa neurčuje z aktuálnej veľkosti súboru (tá rastie
/// s každým uploadom), ale z pozície, ktorú appka mala nahranú v čase, keď
/// sa odtlačok naposledy počítal. Vďaka tomu appka s logom KRATŠÍM než
/// 256 bajtov nevyhodnotí vlastný legitímny prírastok ako rotáciu — keby sa
/// namiesto toho hashovalo vždy „prvých 256 bajtov AKÉHOKOĽVEK aktuálneho
/// obsahu", okno by sa s každým appendom ticho rozširovalo a odtlačok by sa
/// zakaždým zmenil, hoci sa súbor len predĺžil.
fn odtlacok_okna(obsah: &[u8], okno_do: u64) -> String {
    let okno = (okno_do.min(256) as usize).min(obsah.len());
    fnv1a(&obsah[..okno])
}

/// Odkiaľ (a či ide o rotáciu) má ďalší upload tohto súboru pokračovať.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub pozicia_od: u64,
    pub rotovalo: bool,
}

/// Rozhodne, odkiaľ v `obsah` (aktuálny CELÝ obsah súboru na disku) má ďalší
/// upload pokračovať. Rotácia (nová session, appka prepísala súbor s tým
/// istým menom od nuly) sa pozná podľa toho, že sa zmenil odtlačok PRVÝCH
/// `stav.nahrana_pozicia` bajtov (najviac 256) ALEBO je súbor kratší, než
/// kam appka minule dohrala — presne ten istý princíp ako
/// `chvost::StavChvosta::over_rotaciu`, len nad iným stavom.
pub fn priprav_segment(stav: &StavUploaduSuboru, obsah: &[u8]) -> Segment {
    let uz_niekedy_nahrane = !stav.odtlacok_zaciatku.is_empty();
    let rotovalo = uz_niekedy_nahrane
        && ((obsah.len() as u64) < stav.nahrana_pozicia
            || odtlacok_okna(obsah, stav.nahrana_pozicia) != stav.odtlacok_zaciatku);
    Segment { pozicia_od: if rotovalo { 0 } else { stav.nahrana_pozicia }, rotovalo }
}

/// Bajty, ktoré sa majú v tomto uploade naozaj poslať — prázdne pole, keď je
/// appka už úplne dohnaná (nič nové od posledného uploadu).
pub fn novy_obsah(segment: Segment, obsah: &[u8]) -> &[u8] {
    let od = segment.pozicia_od as usize;
    if od >= obsah.len() {
        &[]
    } else {
        &obsah[od..]
    }
}

/// Stav, ktorý sa má uložiť PO úspešnom uploade — podľa toho, čo server
/// skutočne potvrdil (`prijata_pozicia_do`), nie podľa toho, čo appka
/// posielala (server mohol prekryv odrezať sám, pozri `app.py`).
pub fn po_uspesnom_uploade(
    nazov: &str,
    cely_obsah_po_uploade: &[u8],
    prijata_pozicia_do: u64,
) -> StavUploaduSuboru {
    StavUploaduSuboru {
        nazov: nazov.to_string(),
        nahrana_pozicia: prijata_pozicia_do,
        odtlacok_zaciatku: odtlacok_okna(cely_obsah_po_uploade, prijata_pozicia_do),
    }
}

/// Odpoveď servera na `POST /api/tracker/upload`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OdpovedUploadu {
    pub ok: bool,
    pub subor: String,
    pub prijata_pozicia_do: u64,
    pub bajtov_novych: u64,
    pub ulozeny_ako: String,
    /// Vyplnené LEN pri HTTP 409 (medzera v uploade) — appka si podľa toho
    /// vie sama prerezať lokálny súbor a poslať správny segment znova.
    pub ocakavana_pozicia: Option<u64>,
    pub chyba: Option<String>,
}

/// Rozparsuje odpoveď. Chybová vetva nerozlišuje 400/409/413 kódom (ten si
/// všíma `src-tauri` priamo z HTTP statusu, aby vedelo, kedy má skúsiť znova
/// od `ocakavana_pozicia`) — tu ide len o to, či telo dáva zmysel.
pub fn z_odpovede(telo: &str) -> Result<OdpovedUploadu, String> {
    let o: OdpovedUploadu = serde_json::from_str(telo)
        .map_err(|_| "Server odpovedal na upload niečím, čomu appka nerozumie.".to_string())?;
    if !o.ok {
        return Err(o.chyba.clone().unwrap_or_else(|| "Server upload odmietol.".to_string()));
    }
    Ok(o)
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn odtlacok_okna_je_stabilny_a_citlivy_na_zaciatok() {
        assert_eq!(odtlacok_okna(b"ahoj svet", 9), odtlacok_okna(b"ahoj svet", 9));
        assert_ne!(odtlacok_okna(b"ahoj svet", 9), odtlacok_okna(b"iny zaciatok", 12));
    }

    #[test]
    fn odtlacok_okna_sa_pocita_najviac_z_256_bajtov_aj_ked_je_okno_vacsie() {
        let a = vec![b'x'; 256];
        let mut b = a.clone();
        b.extend_from_slice(b"toto je za hranicou a nesmie zmenit odtlacok");
        // Okno 999 > 256 aj > dĺžka `a` — orezáva sa na min(256, dĺžka).
        assert_eq!(odtlacok_okna(&a, 999), odtlacok_okna(&b, 999));
    }

    #[test]
    fn odtlacok_okna_rastie_s_rastucim_oknom_pod_256_bajtov() {
        // Toto je presne bug, ktorému sa `priprav_segment` vyhýba tým, že
        // vždy porovnáva PEVNÉ okno (`stav.nahrana_pozicia`), nie „prvých
        // 256 bajtov aktuálneho súboru": krátky rastúci log by inak pri
        // každom appende vyzeral ako rotácia.
        let obsah = b"riadok 1\nriadok 2\nriadok 3\n";
        assert_ne!(odtlacok_okna(obsah, 9), odtlacok_okna(obsah, 18));
    }

    #[test]
    fn prvy_upload_posiela_vsetko_od_nuly() {
        let stav = StavUploaduSuboru::default();
        let obsah = b"cely subor\n";
        let segment = priprav_segment(&stav, obsah);
        assert_eq!(segment, Segment { pozicia_od: 0, rotovalo: false });
        assert_eq!(novy_obsah(segment, obsah), obsah);
    }

    #[test]
    fn dalsi_upload_posiela_len_pribudnute_bajty() {
        let povodny = b"riadok 1\nriadok 2\n";
        let stav = po_uspesnom_uploade("UE_game.log", povodny, povodny.len() as u64);

        let novy_cely = {
            let mut v = povodny.to_vec();
            v.extend_from_slice(b"riadok 3\n");
            v
        };
        let segment = priprav_segment(&stav, &novy_cely);
        assert!(!segment.rotovalo);
        assert_eq!(segment.pozicia_od, povodny.len() as u64);
        assert_eq!(novy_obsah(segment, &novy_cely), b"riadok 3\n");
    }

    #[test]
    fn kratky_rastuci_subor_pod_256_bajtov_sa_neoznaci_ako_rotacia() {
        // Regresia: predošlá implementácia hashovala „prvých 256 bajtov
        // AKTUÁLNEHO súboru" — pri súbore kratšom než 256 bajtov sa tak toto
        // okno s každým appendom ticho rozširovalo a odtlačok sa zakaždým
        // zmenil, hoci súbor len rástol. Tri po sebe idúce legitímne uploady
        // nesmú ani raz vyzerať ako rotácia.
        let mut obsah = b"riadok A\n".to_vec();
        let mut stav = po_uspesnom_uploade("UE_game.log", &obsah, obsah.len() as u64);
        for pridaj in ["riadok B\n", "riadok C\n"] {
            obsah.extend_from_slice(pridaj.as_bytes());
            let segment = priprav_segment(&stav, &obsah);
            assert!(!segment.rotovalo, "appka pod 256 B nesmie vyzerať ako rotácia");
            stav = po_uspesnom_uploade("UE_game.log", &obsah, obsah.len() as u64);
        }
    }

    #[test]
    fn appka_uplne_dohnana_posle_prazdny_segment() {
        let obsah = b"nic sa nezmenilo\n";
        let stav = po_uspesnom_uploade("UE_game.log", obsah, obsah.len() as u64);
        let segment = priprav_segment(&stav, obsah);
        assert!(novy_obsah(segment, obsah).is_empty());
    }

    #[test]
    fn rotacia_sa_pozna_podla_zmeny_odtlacku_a_posle_sa_vsetko_znova() {
        let stary = b"stara session riadok 1\nriadok 2\n";
        let stav = po_uspesnom_uploade("UE_game.log", stary, stary.len() as u64);

        // Hra log prepísala od nuly (nová session, ten istý názov súboru).
        let novy = b"uplne iny zaciatok\n";
        let segment = priprav_segment(&stav, novy);
        assert!(segment.rotovalo);
        assert_eq!(segment.pozicia_od, 0);
        assert_eq!(novy_obsah(segment, novy), novy);
    }

    #[test]
    fn rotacia_sa_pozna_aj_podla_kratsieho_suboru_pri_zhodnom_zaciatku() {
        // Nepravdepodobné, ale lacná poistka navyše: keby sa súbor skrátil
        // pod predošlú nahranú pozíciu, nejde o pokračovanie.
        let stav = StavUploaduSuboru {
            nazov: "UE_game.log".to_string(),
            nahrana_pozicia: 1000,
            odtlacok_zaciatku: odtlacok_okna(b"ab", 1000),
        };
        let kratsi_ale_rovnaky_zaciatok = b"ab";
        let segment = priprav_segment(&stav, kratsi_ale_rovnaky_zaciatok);
        assert!(segment.rotovalo);
        assert_eq!(segment.pozicia_od, 0);
    }

    #[test]
    fn z_odpovede_uspech() {
        let json = r#"{"ok": true, "subor": "UE_game.log", "prijata_pozicia_do": 120,
                       "bajtov_novych": 40}"#;
        let o = z_odpovede(json).unwrap();
        assert_eq!(o.prijata_pozicia_do, 120);
        assert_eq!(o.bajtov_novych, 40);
    }

    #[test]
    fn z_odpovede_medzera_409_nesie_ocakavanu_poziciu() {
        let json = r#"{"ok": false, "chyba": "medzera v uploade — chýbajú bajty",
                       "ocakavana_pozicia": 80}"#;
        let err = z_odpovede(json).unwrap_err();
        assert!(err.contains("medzera"));
        let o: OdpovedUploadu = serde_json::from_str(json).unwrap();
        assert_eq!(o.ocakavana_pozicia, Some(80));
    }

    #[test]
    fn z_odpovede_zle_telo() {
        let err = z_odpovede("toto nie je json").unwrap_err();
        assert!(err.contains("nerozumie"));
    }

    #[test]
    fn db_export_odpoved_nesie_nazov_suboru() {
        let json = r#"{"ok": true, "ulozeny_ako": "export-2026-10-16-060000-ab12.json"}"#;
        let o = z_odpovede(json).unwrap();
        assert_eq!(o.ulozeny_ako, "export-2026-10-16-060000-ab12.json");
    }
}
