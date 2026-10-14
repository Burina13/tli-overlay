use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tli_overlay_core::nastavenia::Nastavenia;

pub struct Stav {
    pub nastavenia: Mutex<Nastavenia>,
    pub beh: Mutex<bool>,
    pub zobrazene_od: Mutex<Option<Instant>>,
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
