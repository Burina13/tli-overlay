//! Zdieľanie cien v partii (TLI 20).
//!
//! Prečo vôbec: cena z in-game Exchange sa do logu zapíše len vtedy, keď si ju
//! hráč SÁM vyhľadá. Richard ceny ručne aktualizovať nebude a ani nemá —
//! v trojčlennej partii stačí, aby si drahú vec vyhľadal **ktokoľvek jeden**,
//! a ocenenie dostanú všetci. Preto sa ceny posielajú na `tli.tvojtatko.sk`
//! a sťahujú sa späť ako agregát partie.
//!
//! Čo sa posiela a čo NIE:
//! - posielajú sa výhradne ceny z **vlastného** vyhľadania (`zdroj =
//!   vlastny_trh`). Keby sa posielali aj stiahnuté ceny od partie, agregát by
//!   si sám sebe potvrdzoval vlastné číslo a „tri nezávislé zdroje" by
//!   znamenali jeden zdroj prečítaný trikrát;
//! - posielajú sa len **rozdielovo** — podľa `zapisane` (unixový čas zápisu do
//!   lokálnej DB), nie celý cenník pri každom kole;
//! - neposiela sa nič o hráčovi okrem toho, čo server aj tak vie z kľúča.
//!
//! Tento modul je čistá logika (žiadna sieť ani disk) — HTTP robí
//! `src-tauri/src/klient.rs`, rovnako ako pri `sync.rs`.

use serde::{Deserialize, Serialize};

use super::ceny::{Cena, Cennik, Zdroj};
use super::db::ZaznamCeny;

/// Koľko cien najviac odíde v jednej dávke. Pri prvom zapnutí má hráč v DB
/// kľudne tisíce riadkov histórie a nemá zmysel z toho robiť jednu obrovskú
/// požiadavku — zvyšok odíde v ďalšom kole o 5 minút.
pub const MAX_NA_DAVKU: usize = 500;

/// Ako často sa ceny sťahujú a posielajú (sekundy). TITrack posiela á 60 s
/// a sťahuje á 5 min; my posielame aj sťahujeme á 5 min — partia sú traja
/// ľudia, nie trh s tisíckami hráčov, a častejší upload by nič nezmenil.
pub const INTERVAL_S: u64 = 300;

/// Jedna cena pripravená na odoslanie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CenaNaOdoslanie {
    pub polozka: u32,
    pub p10: f64,
    pub median: Option<f64>,
    pub pocet_ponuk: Option<u32>,
    pub sezona: Option<String>,
    /// Herný čas z logu (`RRRR-MM-DD HH:MM:SS`), ak ho riadok niesol.
    pub cas: Option<String>,
    /// Unixový čas zápisu do lokálnej DB — server podľa neho vie, ktorá cena
    /// hráča je najnovšia.
    pub zapisane: i64,
}

impl From<&ZaznamCeny> for CenaNaOdoslanie {
    fn from(z: &ZaznamCeny) -> Self {
        Self {
            polozka: z.polozka,
            p10: z.cena_fe,
            median: z.median_fe,
            pocet_ponuk: z.pocet_ponuk,
            sezona: z.sezona.clone(),
            cas: z.cas.clone(),
            zapisane: z.zapisane,
        }
    }
}

/// Vyberie z DB záznamov tie, ktoré sa majú poslať. Nezmyselné hodnoty
/// (nekonečno, nula, záporné) sa zahodia — jedna pokazená cena nesmie pokaziť
/// medián celej partii.
///
/// Posielajú sa ceny z **vlastného pozorovania** hráča — `vlastny_trh`
/// (vyhľadanie na Exchange, log) aj `obrazovka` (TLI 22: prečítané z
/// obrazovky trhu cez Gemini vision, keď log cenu nenapísal). Obe sú to, čo
/// hráč sám videl na trhu; `partia` (prevzaté od iných) a `rucne` sa
/// neposielajú — rovnaký dôvod ako vyššie (žiadne self-confirmation).
pub fn vyber_na_odoslanie(zaznamy: &[ZaznamCeny], max: usize) -> Vec<CenaNaOdoslanie> {
    zaznamy
        .iter()
        .filter(|z| z.zdroj == "vlastny_trh" || z.zdroj == "obrazovka")
        .filter(|z| z.cena_fe.is_finite() && z.cena_fe > 0.0 && z.cena_fe < 1e9)
        .map(CenaNaOdoslanie::from)
        .take(max)
        .collect()
}

#[derive(Serialize)]
struct TeloUploadu<'a> {
    klient: &'a str,
    ceny: &'a [CenaNaOdoslanie],
}

/// JSON telo pre `POST /api/ceny`.
pub fn telo_uploadu(ceny: &[CenaNaOdoslanie], klient_verzia: &str) -> String {
    serde_json::to_string(&TeloUploadu { klient: klient_verzia, ceny })
        .unwrap_or_else(|_| "{}".to_string())
}

/// Odpoveď na upload cien.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OdpovedUploadu {
    pub ok: bool,
    pub prijatych: u64,
    pub preskocenych: u64,
}

pub fn z_odpovede_uploadu(telo: &str) -> Result<OdpovedUploadu, String> {
    let o: OdpovedUploadu = serde_json::from_str(telo)
        .map_err(|_| "Server odpovedal na ceny niečím, čomu appka nerozumie.".to_string())?;
    if !o.ok {
        return Err("Server ceny odmietol.".to_string());
    }
    Ok(o)
}

/// Jedna cena v agregáte partie (`GET /api/ceny`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CenaPartie {
    pub polozka: u32,
    /// Medián cez hráčov z ich NAJNOVŠÍCH cien pre túto položku.
    pub fe: f64,
    /// Koľko rôznych hráčov do tejto ceny prispelo.
    pub zdrojov: u32,
    /// Súčet ponúk naprieč prispievateľmi — orientačná hĺbka trhu.
    pub pocet_ponuk: u32,
    /// Unixový čas najnovšieho príspevku.
    pub kedy: i64,
    /// `overena` (3+ zdroje) / `dobra` (2) / `slaba` (1).
    pub istota: String,
}

impl Default for CenaPartie {
    fn default() -> Self {
        Self {
            polozka: 0,
            fe: 0.0,
            zdrojov: 0,
            pocet_ponuk: 0,
            kedy: 0,
            istota: "slaba".to_string(),
        }
    }
}

/// Celá odpoveď `GET /api/ceny`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgregatPartie {
    pub ok: bool,
    /// Serverový čas odpovede — klient si ho odloží a nabudúce pošle ako `od`.
    pub cas: i64,
    pub ceny: Vec<CenaPartie>,
}

pub fn z_odpovede_agregatu(telo: &str) -> Result<AgregatPartie, String> {
    let a: AgregatPartie = serde_json::from_str(telo)
        .map_err(|_| "Server poslal ceny partie v tvare, ktorému appka nerozumie.".to_string())?;
    if !a.ok {
        return Err("Server ceny partie neposkytol.".to_string());
    }
    Ok(a)
}

/// Zapíše stiahnutý agregát do cenníka. Vracia počet cien, ktoré sa naozaj
/// zmenili — hráčova vlastná ČERSTVÁ cena partiu prebije a nezmení sa nič
/// (rozhoduje `Cennik::vloz_v_case`, teda dôveryhodnosť × váha veku).
pub fn zapis_do_cennika(agregat: &AgregatPartie, cennik: &mut Cennik, teraz: u64) -> usize {
    let mut zmenenych = 0;
    for c in &agregat.ceny {
        if !c.fe.is_finite() || c.fe <= 0.0 {
            continue;
        }
        let cena = Cena {
            polozka: c.polozka,
            fe: c.fe,
            zdroj: Zdroj::Partia,
            kedy: c.kedy.max(0) as u64,
            median: None,
            pocet_ponuk: c.pocet_ponuk,
            zdrojov: c.zdrojov,
        };
        if cennik.vloz_v_case(cena, teraz) {
            zmenenych += 1;
        }
    }
    zmenenych
}

/// Veta pre UI po jednom kole synchronizácie cien.
pub fn zhrnutie(poslanych: usize, stiahnutych: usize, zmenenych: usize) -> String {
    if poslanych == 0 && stiahnutych == 0 {
        return "Ceny sú zosúladené s partiou.".to_string();
    }
    format!(
        "Ceny: odoslaných {}, od partie prišlo {} (z toho {} nových).",
        poslanych, stiahnutych, zmenenych
    )
}

#[cfg(test)]
mod testy {
    use super::*;

    fn zaznam(polozka: u32, fe: f64, zdroj: &str, zapisane: i64) -> ZaznamCeny {
        ZaznamCeny {
            polozka,
            cena_fe: fe,
            median_fe: Some(fe * 1.2),
            pocet_ponuk: Some(5),
            sezona: Some("1501".to_string()),
            zdroj: zdroj.to_string(),
            cas: Some("2026-10-16 10:00:00".to_string()),
            zapisane,
        }
    }

    #[test]
    fn posielaju_sa_len_vlastne_ceny() {
        let zaznamy = vec![
            zaznam(1, 1.0, "vlastny_trh", 100),
            zaznam(2, 2.0, "partia", 101),
            zaznam(3, 3.0, "rucne", 102),
        ];
        let vybrane = vyber_na_odoslanie(&zaznamy, MAX_NA_DAVKU);
        assert_eq!(vybrane.len(), 1, "cena stiahnutá od partie sa nesmie poslať späť");
        assert_eq!(vybrane[0].polozka, 1);
    }

    #[test]
    fn posiela_sa_aj_cena_z_obrazovky_trhu() {
        // TLI 22: aj to je vlastné pozorovanie hráča (čítanie trhu okom cez
        // Gemini vision namiesto cez log), rovnako dôveryhodné pre partiu ako
        // vlastny_trh — nie prevzaté od niekoho iného.
        let zaznamy = vec![
            zaznam(1, 1.0, "vlastny_trh", 100),
            zaznam(2, 2.0, "obrazovka", 101),
            zaznam(3, 3.0, "partia", 102),
            zaznam(4, 4.0, "rucne", 103),
        ];
        let vybrane = vyber_na_odoslanie(&zaznamy, MAX_NA_DAVKU);
        assert_eq!(vybrane.len(), 2);
        assert!(vybrane.iter().any(|c| c.polozka == 1));
        assert!(vybrane.iter().any(|c| c.polozka == 2));
    }

    #[test]
    fn nezmyselne_ceny_sa_zahodia() {
        let zaznamy = vec![
            zaznam(1, 0.0, "vlastny_trh", 100),
            zaznam(2, -5.0, "vlastny_trh", 101),
            zaznam(3, f64::INFINITY, "vlastny_trh", 102),
            zaznam(4, 1.5, "vlastny_trh", 103),
        ];
        let vybrane = vyber_na_odoslanie(&zaznamy, MAX_NA_DAVKU);
        assert_eq!(vybrane.len(), 1);
        assert_eq!(vybrane[0].polozka, 4);
    }

    #[test]
    fn davka_je_zhora_obmedzena() {
        let zaznamy: Vec<_> = (0..50).map(|i| zaznam(i, 1.0, "vlastny_trh", i as i64)).collect();
        assert_eq!(vyber_na_odoslanie(&zaznamy, 10).len(), 10);
    }

    #[test]
    fn telo_uploadu_je_platny_json() {
        let ceny = vyber_na_odoslanie(&[zaznam(400021, 1.5, "vlastny_trh", 10)], 10);
        let telo = telo_uploadu(&ceny, "0.15.0");
        let v: serde_json::Value = serde_json::from_str(&telo).unwrap();
        assert_eq!(v["klient"], "0.15.0");
        assert_eq!(v["ceny"][0]["polozka"], 400021);
        assert_eq!(v["ceny"][0]["p10"], 1.5);
        assert_eq!(v["ceny"][0]["sezona"], "1501");
    }

    #[test]
    fn odpoved_uploadu() {
        let o = z_odpovede_uploadu(r#"{"ok":true,"prijatych":3,"preskocenych":1}"#).unwrap();
        assert_eq!(o.prijatych, 3);
        assert!(z_odpovede_uploadu(r#"{"ok":false}"#).is_err());
        assert!(z_odpovede_uploadu("toto nie je json").is_err());
    }

    #[test]
    fn agregat_sa_zapise_do_cennika() {
        let telo = r#"{"ok":true,"cas":2000,"ceny":[
            {"polozka":400021,"fe":1.6,"zdrojov":3,"pocet_ponuk":21,"kedy":1900,"istota":"overena"},
            {"polozka":5080,"fe":0.0,"zdrojov":1,"pocet_ponuk":2,"kedy":1900,"istota":"slaba"}
        ]}"#;
        let a = z_odpovede_agregatu(telo).unwrap();
        assert_eq!(a.cas, 2000);
        let mut cennik = Cennik::novy();
        let zmenenych = zapis_do_cennika(&a, &mut cennik, 2000);
        assert_eq!(zmenenych, 1, "nulová cena sa nesmie zapísať");
        let c = cennik.cena(400021).unwrap();
        assert_eq!(c.fe, 1.6);
        assert_eq!(c.zdroj, Zdroj::Partia);
        assert_eq!(c.zdrojov, 3);
    }

    #[test]
    fn vlastna_cerstva_cena_prebije_partiu() {
        let mut cennik = Cennik::novy();
        cennik.vloz_v_case(Cena::vlastna(400021, 2.0, 2.5, 9, 1990), 2000);
        let a = z_odpovede_agregatu(
            r#"{"ok":true,"cas":2000,"ceny":[{"polozka":400021,"fe":1.6,"zdrojov":3,"kedy":1900,"istota":"overena"}]}"#,
        )
        .unwrap();
        assert_eq!(zapis_do_cennika(&a, &mut cennik, 2000), 0);
        assert_eq!(cennik.cena(400021).unwrap().fe, 2.0);
    }

    #[test]
    fn partia_prebije_tyzden_staru_vlastnu() {
        let mut cennik = Cennik::novy();
        let teraz = 10 * 86400;
        cennik.vloz_v_case(Cena::vlastna(400021, 2.0, 2.5, 9, 0), teraz);
        let telo = format!(
            r#"{{"ok":true,"cas":{t},"ceny":[{{"polozka":400021,"fe":1.6,"zdrojov":3,"kedy":{k},"istota":"overena"}}]}}"#,
            t = teraz,
            k = teraz - 600
        );
        let a = z_odpovede_agregatu(&telo).unwrap();
        assert_eq!(zapis_do_cennika(&a, &mut cennik, teraz as u64), 1);
        assert_eq!(cennik.cena(400021).unwrap().fe, 1.6);
    }

    #[test]
    fn zhrnutie_je_zrozumitelne() {
        assert_eq!(zhrnutie(0, 0, 0), "Ceny sú zosúladené s partiou.");
        assert!(zhrnutie(2, 5, 1).contains("odoslaných 2"));
    }
}
