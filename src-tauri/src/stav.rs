use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;
use tli_overlay_core::nastavenia::Nastavenia;
use tli_overlay_core::tracker::chvost::StavChvosta;

pub struct Stav {
    pub nastavenia: Mutex<Nastavenia>,
    pub beh: Mutex<bool>,
    pub zobrazene_od: Mutex<Option<Instant>>,
    /// Zber dát (TLI 4c) — koľko snímok sa za tento beh appky úspešne poslalo
    /// a uložilo na server v režime "Zber dát". Len v pamäti (reštart appky
    /// počítadlo vynuluje) — Richard má za jeden deň zberu jedno spustenie,
    /// presný súčet beztak drží server (DB `analyzy`), toto je len okamžitá
    /// spätná väzba "fungovalo mi to práve teraz".
    pub pocitadlo_zber: Mutex<u32>,
    /// Kľúče máp, ktoré už partiový server dostal (TLI 11, krok 5). Len v pamäti:
    /// po reštarte appky sa dávka pošle znova a server ju podľa toho istého
    /// kľúča ticho zahodí (`INSERT OR IGNORE`) — druhý zoznam na disku by bol
    /// len ďalšie miesto, ktoré sa vie rozísť s pravdou na serveri.
    pub poslane_behy: Mutex<BTreeSet<String>>,
    /// Značka času, odkedy je viditeľný toast pripomienky „Enable Log"
    /// (TLI 11d) — rovnaký vzor ako `zobrazene_od` pri overlayi: časovač na
    /// automatické skrytie porovná značky, aby novší toast nezhasol starší
    /// časovač práve zobrazeného okna.
    pub pripomienka_zobrazena_od: Mutex<Option<Instant>>,
}

/// Sluzi na bezpecne ziskanie zlozky na ulozenie konfiguracie pre hracie konto z operacneho systemu.
/// DOVOD: Aplikacia si potrebuje ukladat nastavenia trvalo mimo instalacneho priestoru, pretoze v dalsej verzii by ju instalator prepisal.
pub fn cesta_suboru() -> PathBuf {
    let mut cesta = if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
    } else {
        // Fallback prevazne pre CI a testovacie prostredia
        if let Ok(home) = std::env::var("HOME") {
            let mut p = PathBuf::from(home);
            p.push(".config");
            p
        } else {
            PathBuf::from(".")
        }
    };
    cesta.push("sk.tvojtatko.tli-overlay");
    cesta.push("nastavenia.json");
    cesta
}

/// Ziska lokalnu kopiu suboru s konfiguraciou v systeme po starte.
/// DOVOD: Inicializuje aplikaciu klucom pre pristup. Bez neho nastroj nefunguje a pri nenajdeni
/// poskytne defaultnu (vycistenu) hodnotu struktury.
pub fn nacitaj() -> Nastavenia {
    let cesta = cesta_suboru();
    if let Ok(obsah) = std::fs::read_to_string(&cesta) {
        Nastavenia::z_jsonu(&obsah)
    } else {
        Nastavenia::z_jsonu("{}")
    }
}

/// Zapisuje pouzivatelom modifikovane data a spristupnuje novu konfiguraciu pri dalsej analzye.
/// DOVOD: Perzistencia hesiel k API a klavesovych skratiek je nutna pre standardny operacny chod.
pub fn uloz(n: &Nastavenia) -> Result<(), String> {
    let cesta = cesta_suboru();

    if let Some(priecinok) = cesta.parent() {
        if let Err(e) = std::fs::create_dir_all(priecinok) {
            return Err(format!("Nepodarilo sa vytvoriť priečinok pre nastavenia: {}", e));
        }
    }

    let json = n.do_jsonu();
    if let Err(e) = std::fs::write(&cesta, json) {
        return Err(format!("Nepodarilo sa uložiť nastavenia do súboru: {}", e));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Pozícia v hernom logu (TLI 11c) — nech appka po reštarte nemusí znova
// reparsovať aj to, čo už raz videla. Žije v samostatnom súbore vedľa
// `nastavenia.json`, nie v ňom: je to prevádzkový stav, nie nastavenie od
// hráča, a jeho strata (zmazaný/poškodený súbor) nesmie zobrať nastavenia.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct UlozenyChvost {
    cesta: String,
    chvost: StavChvosta,
}

fn cesta_chvosta_suboru() -> PathBuf {
    let mut c = cesta_suboru();
    c.set_file_name("tracker-pozicia.json");
    c
}

/// Načíta uloženú pozíciu v logu — ale LEN keď uložená cesta sedí s tou, čo
/// sa práve číta. Inak by appka po zmene cesty (iná hra, iný disk) omylom
/// „pokračovala" v súbore, ktorý vôbec nečíta, a buď preskočila jeho
/// začiatok, alebo (pri menšom súbore) zbytočne čakala na rotáciu.
pub fn nacitaj_chvost(cesta: &Path) -> StavChvosta {
    let Ok(obsah) = std::fs::read_to_string(cesta_chvosta_suboru()) else {
        return StavChvosta::novy();
    };
    chvost_z_obsahu(&obsah, cesta)
}

/// Rozhodnutie „sedí uložená cesta s tou, čo sa číta teraz" vytiahnuté do
/// čistej funkcie bez dotyku s diskom — dá sa otestovať priamo, bez súborov
/// aj bez `$APPDATA`/`$HOME` (na tie sa viaže `cesta_chvosta_suboru()`).
fn chvost_z_obsahu(obsah: &str, cesta: &Path) -> StavChvosta {
    let Ok(ulozeny) = serde_json::from_str::<UlozenyChvost>(obsah) else {
        return StavChvosta::novy();
    };
    if ulozeny.cesta == cesta.display().to_string() {
        ulozeny.chvost
    } else {
        StavChvosta::novy()
    }
}

/// Uloží pozíciu po každom úspešnom čítaní, ktoré posunulo stav ďalej.
/// Zápis je zámerne best-effort (chyba sa tichy ignoruje) — strata tohto
/// súboru appku nezastaví, len pri ďalšom reštarte prečíta log znova od
/// začiatku.
pub fn uloz_chvost(cesta: &Path, chvost: &StavChvosta) {
    let subor = cesta_chvosta_suboru();
    let ulozeny = UlozenyChvost { cesta: cesta.display().to_string(), chvost: chvost.clone() };
    let Ok(json) = serde_json::to_string(&ulozeny) else {
        return;
    };
    if let Some(priecinok) = subor.parent() {
        let _ = std::fs::create_dir_all(priecinok);
    }
    let _ = std::fs::write(&subor, json);
}

#[cfg(test)]
mod testy_chvosta {
    use super::*;

    #[test]
    fn chvost_sa_obnovi_len_pri_zhode_cesty() {
        let cesta = Path::new("/hra/UE_game.log");
        let mut chvost = StavChvosta::novy();
        chvost.priprav_riadky(b"abc\n");
        let ulozeny = UlozenyChvost { cesta: cesta.display().to_string(), chvost };
        let json = serde_json::to_string(&ulozeny).unwrap();

        let obnoveny = chvost_z_obsahu(&json, cesta);
        assert_eq!(obnoveny.pozicia, 4);

        // Iná cesta (iná hra/iný disk) sa nesmie „pokračovať" v cudzom súbore.
        let ina_cesta = Path::new("/ina/hra.log");
        let cerstvy = chvost_z_obsahu(&json, ina_cesta);
        assert_eq!(cerstvy.pozicia, 0);
    }

    #[test]
    fn poskodeny_alebo_chybajuci_obsah_da_cerstvy_stav() {
        let cesta = Path::new("/hra/UE_game.log");
        assert_eq!(chvost_z_obsahu("nie je to json", cesta).pozicia, 0);
        assert_eq!(chvost_z_obsahu("", cesta).pozicia, 0);
    }
}
