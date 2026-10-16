//! Denný upload surových logov a DB exportu na server (TLI 14) — LEN
//! rozhodovanie, ktoré bajty sú nové od posledného uploadu a aký kus z nich
//! poslať TERAZ, plus parsovanie odpovede servera. Samotné otvorenie súboru,
//! seek/read, gzip kompresiu a HTTP odoslanie rieši `src-tauri`, presne tak
//! ako živé čítanie logu rieši `src-tauri/src/tracker.rs` nad `chvost.rs`.
//!
//! **Zámerne pracuje len s OKNAMI, nikdy s celým obsahom súboru v pamäti**
//! (TLI 19/20 dopĺňajúca úloha, 16.10.2026): `UE_game.log` vie za deň
//! narásť na stovky MB a appka nesmie kvôli jednému kolu uploadu držať
//! v pamäti celý súbor. Rozpoznanie rotácie potrebuje len prvých
//! ≤256 bajtov súboru (`odtlacok_okna`), nie všetko medzi nimi a aktuálnou
//! pozíciou — volajúci si preto prečíta len to malé okno a samostatne kúsok
//! na odoslanie (`dlzka_kusku`), nikdy celý rozsah naraz.
//!
//! Server kontrakt: `POST /api/tracker/upload`
//! (`docs/fakty/hry/torchlight-infinite/README.md` v ops repe, implementácia
//! `stacks/games/tli-web/app.py`). Server limituje gzip telo na 16 MB
//! (`TRACKER_UPLOAD_MAX`) a rozbalený obsah na 128 MB
//! (`TRACKER_UPLOAD_ROZBALENE_MAX`) — `KUSOK_BAJTOV` necháva veľkú rezervu
//! pod oboma stropmi aj pre menej stlačiteľný obsah.

use serde::{Deserialize, Serialize};

/// Stav uploadu JEDNÉHO súboru — perzistovateľný (ukladá sa do lokálnej DB,
/// `db::uloz_upload_stav`), lebo appka po reštarte NESMIE poslať `pozicia_od
/// = 0` nad súborom, ktorý server už čiastočne má — to by server vyhodnotil
/// ako rotáciu a existujúci log by zbytočne archivoval. Zámerne oddelený od
/// `chvost::StavChvosta` (živé čítanie): upload beží v inom rytme (á
/// niekoľko minút + pri ukončení appky), nie pri každom novom riadku.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StavUploaduSuboru {
    pub nazov: String,
    pub nahrana_pozicia: u64,
    /// Odtlačok prvých `min(256, nahrana_pozicia)` bajtov súboru v čase
    /// posledného uploadu. Prázdny reťazec znamená „ešte nikdy nenahrané" —
    /// vtedy sa rotácia nerieši, pošle sa jednoducho všetko od nuly.
    pub odtlacok_zaciatku: String,
}

/// Najväčší počet SUROVÝCH (nekomprimovaných) bajtov v jednom segmente
/// uploadu. Textový herný log sa gzipom zmenší zvyčajne 5-10×, 8 MiB
/// surových bajtov preto s veľkou rezervou prejde oboma serverovými
/// stropmi aj pri menej stlačiteľnom obsahu (viď modul vyššie).
pub const KUSOK_BAJTOV: u64 = 8 * 1024 * 1024;

/// Ako často (v sekundách) sa má skúsiť ďalší kus uploadu — á 10 minút, nie
/// á 5 ako `ceny_sync::INTERVAL_S`: log je rádovo väčší prenos a nie je
/// dôvod ho posielať rovnako často ako malé JSON dávky cien. Dosť časté na
/// to, aby Richard nikdy nemusel kopírovať log ručne (zadanie
/// `2026-10-16-1208-mcp.md`), dosť zriedkavé na to, aby to nebrzdilo hru.
pub const INTERVAL_S: u64 = 600;

/// FNV-1a nad presne tými bajtami, čo dostane (orezanie na najviac 256 B
/// robí volajúci v `odtlacok_okna` aj v `src-tauri`, keď číta súbor zo
/// disku) — rovnaký algoritmus a dôvod ako `kluc_behu` v `sync.rs` (žiadna
/// kryptografická závislosť netreba).
fn fnv1a(bajty: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    let prime: u64 = 0x100000001b3;
    for b in bajty {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(prime);
    }
    format!("{hash:016x}")
}

/// Odtlačok okna zo ZAČIATKU súboru — `okno` musí byť presne tých najviac
/// 256 bajtov, ktoré volajúci prečítal od pozície 0 (dlhší vstup sa orezáva,
/// aby sa dalo bezpečne poslať aj o niečo väčšie okno bez zmeny výsledku).
fn odtlacok_okna(okno: &[u8]) -> String {
    fnv1a(&okno[..okno.len().min(256)])
}

/// Odkiaľ (a či ide o rotáciu) má ďalší upload tohto súboru pokračovať.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub pozicia_od: u64,
    pub rotovalo: bool,
}

/// Rozhodne, odkiaľ má ďalší upload pokračovať.
///
/// `okno_zaciatku` je úvod súboru PREČÍTANÝ VOLAJÚCIM s presne
/// `stav.nahrana_pozicia.min(256)` bajtmi (nie aktuálnou veľkosťou súboru —
/// to by pri krátkom rastúcom logu vyzeralo ako rotácia pri každom
/// appende, pozri regresný test nižšie). `aktualna_velkost` je veľkosť
/// súboru TERAZ. Rotácia (nová session, appka prepísala súbor s tým istým
/// menom od nuly) sa pozná podľa toho, že sa zmenil odtlačok tohto okna,
/// ALEBO je súbor kratší, než kam appka minule dohrala.
pub fn priprav_segment(stav: &StavUploaduSuboru, okno_zaciatku: &[u8], aktualna_velkost: u64) -> Segment {
    let uz_niekedy_nahrane = !stav.odtlacok_zaciatku.is_empty();
    let rotovalo = uz_niekedy_nahrane
        && (aktualna_velkost < stav.nahrana_pozicia
            || odtlacok_okna(okno_zaciatku) != stav.odtlacok_zaciatku);
    Segment { pozicia_od: if rotovalo { 0 } else { stav.nahrana_pozicia }, rotovalo }
}

/// Koľko bajtov má volajúci prečítať OD `segment.pozicia_od` a poslať v
/// tomto kole — najviac `KUSOK_BAJTOV`, nikdy viac. Appka, ktorá bola dlho
/// vypnutá (veľký nedohnaný rozdiel), tak pošle viacero menších kôl namiesto
/// jedného obrovského POSTu — volajúci volá toto aj `po_uspesnom_uploade`
/// v slučke, kým nevráti 0.
pub fn dlzka_kusku(segment: Segment, aktualna_velkost: u64) -> u64 {
    aktualna_velkost.saturating_sub(segment.pozicia_od).min(KUSOK_BAJTOV)
}

/// Stav, ktorý sa má uložiť PO úspešnom uploade — podľa toho, čo server
/// skutočne potvrdil (`prijata_pozicia_do`), nie podľa toho, čo appka
/// posielala (server mohol prekryv odrezať sám, pozri `app.py`).
///
/// `okno_zaciatku_po` je úvod súboru s `min(256, prijata_pozicia_do)`
/// bajtmi, prečítaný volajúcim PO uploade (keď `prijata_pozicia_do` prvý raz
/// prekročí 256, treba dlhšie okno, než aké mal volajúci k dispozícii pred
/// uploadom — preto samostatný malý čítací krok, nie recyklácia predošlého).
pub fn po_uspesnom_uploade(
    nazov: &str,
    okno_zaciatku_po: &[u8],
    prijata_pozicia_do: u64,
) -> StavUploaduSuboru {
    StavUploaduSuboru {
        nazov: nazov.to_string(),
        nahrana_pozicia: prijata_pozicia_do,
        odtlacok_zaciatku: odtlacok_okna(okno_zaciatku_po),
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
        assert_eq!(odtlacok_okna(b"ahoj svet"), odtlacok_okna(b"ahoj svet"));
        assert_ne!(odtlacok_okna(b"ahoj svet"), odtlacok_okna(b"iny zaciatok"));
    }

    #[test]
    fn odtlacok_okna_sa_pocita_najviac_z_256_bajtov_aj_ked_je_okno_vacsie() {
        let a = vec![b'x'; 256];
        let mut b = a.clone();
        b.extend_from_slice(b"toto je za hranicou a nesmie zmenit odtlacok");
        assert_eq!(odtlacok_okna(&a), odtlacok_okna(&b));
    }

    #[test]
    fn prvy_upload_posiela_vsetko_od_nuly() {
        let stav = StavUploaduSuboru::default();
        let segment = priprav_segment(&stav, b"", 11);
        assert_eq!(segment, Segment { pozicia_od: 0, rotovalo: false });
        assert_eq!(dlzka_kusku(segment, 11), 11);
    }

    #[test]
    fn dalsi_upload_posiela_len_pribudnute_bajty() {
        let povodny = b"riadok 1\nriadok 2\n";
        let stav = po_uspesnom_uploade("UE_game.log", povodny, povodny.len() as u64);

        // Súbor medzitým narástol o "riadok 3\n" — rovnaký úvod, len dlhší.
        let novy_cely_dlzka = povodny.len() as u64 + 9;
        let okno = &povodny[..povodny.len().min(256)];
        let segment = priprav_segment(&stav, okno, novy_cely_dlzka);
        assert!(!segment.rotovalo);
        assert_eq!(segment.pozicia_od, povodny.len() as u64);
        assert_eq!(dlzka_kusku(segment, novy_cely_dlzka), 9);
    }

    #[test]
    fn kratky_rastuci_subor_pod_256_bajtov_sa_neoznaci_ako_rotacia() {
        // Regresia: predošlá implementácia (pred refaktorom na okná) hashovala
        // „prvých 256 bajtov AKTUÁLNEHO súboru" — pri súbore kratšom než
        // 256 bajtov sa tak toto okno s každým appendom ticho rozširovalo a
        // odtlačok sa zakaždým zmenil, hoci súbor len rástol. Tri po sebe
        // idúce legitímne uploady nesmú ani raz vyzerať ako rotácia.
        let mut obsah = b"riadok A\n".to_vec();
        let mut stav = po_uspesnom_uploade("UE_game.log", &obsah, obsah.len() as u64);
        for pridaj in ["riadok B\n", "riadok C\n"] {
            obsah.extend_from_slice(pridaj.as_bytes());
            let okno_dlzka = stav.nahrana_pozicia.min(256) as usize;
            let okno: Vec<u8> = obsah[..okno_dlzka].to_vec();
            let segment = priprav_segment(&stav, &okno, obsah.len() as u64);
            assert!(!segment.rotovalo, "appka pod 256 B nesmie vyzerať ako rotácia");
            let okno_po = obsah[..(segment.pozicia_od + dlzka_kusku(segment, obsah.len() as u64))
                .min(256) as usize]
                .to_vec();
            stav = po_uspesnom_uploade("UE_game.log", &okno_po, obsah.len() as u64);
        }
    }

    #[test]
    fn appka_uplne_dohnana_posle_prazdny_kusok() {
        let obsah = b"nic sa nezmenilo\n";
        let stav = po_uspesnom_uploade("UE_game.log", obsah, obsah.len() as u64);
        let segment = priprav_segment(&stav, obsah, obsah.len() as u64);
        assert_eq!(dlzka_kusku(segment, obsah.len() as u64), 0);
    }

    #[test]
    fn rotacia_sa_pozna_podla_zmeny_odtlacku_a_posle_sa_vsetko_znova() {
        let stary = b"stara session riadok 1\nriadok 2\n";
        let stav = po_uspesnom_uploade("UE_game.log", stary, stary.len() as u64);

        // Hra log prepísala od nuly (nová session, ten istý názov súboru) —
        // úvod nového súboru je úplne iný než zapamätané okno.
        let novy_uvod = b"uplne iny zaciatok\n";
        let segment = priprav_segment(&stav, novy_uvod, novy_uvod.len() as u64);
        assert!(segment.rotovalo);
        assert_eq!(segment.pozicia_od, 0);
        assert_eq!(dlzka_kusku(segment, novy_uvod.len() as u64), novy_uvod.len() as u64);
    }

    #[test]
    fn rotacia_sa_pozna_aj_podla_kratsieho_suboru_pri_zhodnom_zaciatku() {
        // Nepravdepodobné, ale lacná poistka navyše: keby sa súbor skrátil
        // pod predošlú nahranú pozíciu, nejde o pokračovanie.
        let stav = StavUploaduSuboru {
            nazov: "UE_game.log".to_string(),
            nahrana_pozicia: 1000,
            odtlacok_zaciatku: odtlacok_okna(b"ab"),
        };
        let segment = priprav_segment(&stav, b"ab", 2);
        assert!(segment.rotovalo);
        assert_eq!(segment.pozicia_od, 0);
    }

    #[test]
    fn velky_rozdiel_sa_posiela_po_kuskoch() {
        // Appka bola dlho vypnutá: zaostáva o viac než `KUSOK_BAJTOV`.
        let stav = StavUploaduSuboru::default();
        let velkost = KUSOK_BAJTOV * 2 + 500;
        let segment = priprav_segment(&stav, b"", velkost);
        assert_eq!(dlzka_kusku(segment, velkost), KUSOK_BAJTOV);

        // Po prvom kole appka nahlási pokrok len o jeden kúsok, nie celý súbor.
        let stav2 = po_uspesnom_uploade("UE_game.log", b"", KUSOK_BAJTOV);
        let segment2 = priprav_segment(&stav2, b"", velkost);
        assert!(!segment2.rotovalo);
        assert_eq!(segment2.pozicia_od, KUSOK_BAJTOV);
        assert_eq!(dlzka_kusku(segment2, velkost), KUSOK_BAJTOV);

        let stav3 = po_uspesnom_uploade("UE_game.log", b"", KUSOK_BAJTOV * 2);
        let segment3 = priprav_segment(&stav3, b"", velkost);
        assert_eq!(dlzka_kusku(segment3, velkost), 500);
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
