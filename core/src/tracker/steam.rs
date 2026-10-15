//! Hľadanie `UE_game.log` cez Steam knižnice (TLI 11c).
//!
//! Celý modul je čisto reťazcová/cestová logika bez dotyku s registrom alebo
//! diskom — to robí volajúci v `src-tauri` (Windows registry, čítanie
//! `libraryfolders.vdf`) a sem len odovzdá, čo našiel. Vďaka tomu sa
//! parsovanie VDF aj skladanie kandidátnych ciest dá otestovať na Linuxe
//! s falošnými (vymyslenými) hodnotami.

use std::path::{Path, PathBuf};

/// Cesta od koreňa Steam knižnice po log, overená 15.10.2026 priamo na
/// Richardovom PC (`2026-10-15-2119-mcp.md`). Windows cesty nerozlišujú
/// veľkosť písmen, takže toto je jediné miesto, ktoré ju definuje — žiadne
/// ďalšie miesto v kóde si vlastnú kópiu nevymýšľa.
pub const SUFIX_LOGU: &str =
    r"steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log";

/// Rozparsuje Steam `libraryfolders.vdf` (formát Valve KeyValues) a vráti
/// cesty všetkých knižníc, ktoré v ňom majú riadok `"path"   "..."`.
/// Ostatné kľúče (`label`, `contentid`, `apps` …) sa ignorujú — zaujíma nás
/// len to, kde knižnica fyzicky leží.
pub fn kniznice_z_vdf(obsah: &str) -> Vec<PathBuf> {
    obsah
        .lines()
        .filter(|r| r.trim().starts_with("\"path\""))
        .filter_map(hodnota_vdf)
        .map(|v| PathBuf::from(v.replace("\\\\", "\\")))
        .collect()
}

/// Vytiahne druhú úvodzovkovanú hodnotu z riadku v tvare `"kľúč"␠␠"hodnota"`.
fn hodnota_vdf(riadok: &str) -> Option<String> {
    riadok.split('"').nth(3).map(str::to_string)
}

/// Zlúči hlavnú Steam inštaláciu (z registra) so zoznamom ďalších knižníc
/// (z `libraryfolders.vdf`) do jedného poradia bez duplicít. Hlavná
/// inštalácia ide prvá — je najpravdepodobnejšia a `libraryfolders.vdf` ju
/// v starších formátoch vôbec nespomína.
pub fn zluc_kniznice(hlavna: Option<PathBuf>, dalsie: &[PathBuf]) -> Vec<PathBuf> {
    let mut vysledok = Vec::new();
    if let Some(h) = hlavna {
        vysledok.push(h);
    }
    for k in dalsie {
        if !vysledok.contains(k) {
            vysledok.push(k.clone());
        }
    }
    vysledok
}

/// Z koreňov Steam knižníc postaví kandidátne cesty k logu — jednu na
/// knižnicu. Existenciu súboru si overí až volajúci (tu by na Linuxe/CI
/// rovnako nič neexistovalo).
///
/// Zreťazuje sa cez `format!` s pevnou `\`, nie cez `Path::join` — na Linuxe
/// (jadro behá v CI aj na `ubuntu-latest`) totiž `\` nie je oddeľovač ciest
/// a `join` by vrátil zmiešané `/` a `\`, hoci výsledná cesta je VŽDY
/// windowská (appka beží len na Windows).
pub fn kandidati_logu(kniznice: &[PathBuf]) -> Vec<PathBuf> {
    kniznice
        .iter()
        .map(|k| PathBuf::from(format!("{}\\{SUFIX_LOGU}", k.display())))
        .collect()
}

/// Prvá kandidátna cesta, pre ktorú `existuje` vráti `true`. Oddelené od
/// `kandidati_logu`, aby sa dalo testovať s vlastnou (falošnou) funkciou
/// existencie namiesto skutočného disku.
pub fn najdi_existujucu<F: Fn(&Path) -> bool>(kandidati: &[PathBuf], existuje: F) -> Option<PathBuf> {
    kandidati.iter().find(|p| existuje(p)).cloned()
}

#[cfg(test)]
mod testy {
    use super::*;

    const VZOR_VDF: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"contentid"		"123"
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"label"		""
	}
}
"#;

    #[test]
    fn vdf_vytiahne_vsetky_cesty_a_odescapuje_lomky() {
        let v = kniznice_z_vdf(VZOR_VDF);
        assert_eq!(
            v,
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
            ]
        );
    }

    #[test]
    fn vdf_ine_kluce_sa_ignoruju() {
        let v = kniznice_z_vdf(VZOR_VDF);
        assert!(!v.iter().any(|p| p.to_string_lossy().contains("123")));
    }

    #[test]
    fn vdf_prazdny_alebo_poskodeny_obsah_nepada() {
        assert_eq!(kniznice_z_vdf(""), Vec::<PathBuf>::new());
        assert_eq!(kniznice_z_vdf("{{{ nezmysel \" bez páru"), Vec::<PathBuf>::new());
    }

    #[test]
    fn zluc_kniznice_da_hlavnu_prvu_a_bez_duplicit() {
        let hlavna = PathBuf::from(r"C:\Program Files (x86)\Steam");
        let dalsie = kniznice_z_vdf(VZOR_VDF);
        let v = zluc_kniznice(Some(hlavna.clone()), &dalsie);
        assert_eq!(v[0], hlavna);
        assert_eq!(v.len(), 2, "hlavná knižnica je v zozname aj z vdf — nesmie sa zdvojiť");
    }

    #[test]
    fn zluc_kniznice_bez_hlavnej() {
        let dalsie = vec![PathBuf::from(r"D:\SteamLibrary")];
        let v = zluc_kniznice(None, &dalsie);
        assert_eq!(v, dalsie);
    }

    #[test]
    fn kandidati_logu_skladaju_spravnu_cestu() {
        let kniznice = vec![PathBuf::from(r"C:\Program Files (x86)\Steam")];
        let v = kandidati_logu(&kniznice);
        assert_eq!(
            v[0],
            PathBuf::from(
                r"C:\Program Files (x86)\Steam\steamapps\common\Torchlight Infinite\UE_Game\Torchlight\Saved\Logs\UE_game.log"
            )
        );
    }

    #[test]
    fn najdi_existujucu_vrati_prvu_zhodu() {
        // Porovnáva sa presnou zhodou, nie `starts_with` — na Linuxe (kde
        // tento test beží v CI) `\` nie je oddeľovač komponentov cesty, takže
        // "D:\existuje" by nebol prefixovým KOMPONENTOM "D:\existuje\...",
        // hoci reťazcovo ním je.
        let existujuca = PathBuf::from(r"D:\existuje\UE_game.log");
        let kandidati = vec![PathBuf::from(r"C:\neexistuje\UE_game.log"), existujuca.clone()];
        let najdena = najdi_existujucu(&kandidati, |p| *p == existujuca);
        assert_eq!(najdena, Some(existujuca));
    }

    #[test]
    fn najdi_existujucu_nic_nenajde() {
        let kandidati = vec![PathBuf::from(r"C:\neexistuje\UE_game.log")];
        assert_eq!(najdi_existujucu(&kandidati, |_| false), None);
    }
}
