use std::collections::BTreeMap;

/// Čas z hlavičky riadku. Ukladá pôvodné hodnoty z logu, aby sa nestratila presnosť
/// a nebolo nutné ťahať externé závislosti na prácu s časom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Cas {
    pub rok: u16,
    pub mesiac: u8,
    pub den: u8,
    pub hodina: u8,
    pub minuta: u8,
    pub sekunda: u8,
    pub ms: u16,
}

impl Cas {
    /// Čas v tvare `RRRR-MM-DD HH:MM:SS`. Používa sa ako STABILNÝ podpis behu
    /// (`tracker::sync::kluc_behu`): poradové `id` platí len v rámci jedného
    /// čítania logu, takže po reštarte appky by sa tie isté mapy poslali na
    /// server znova ako nové.
    pub fn text(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.rok, self.mesiac, self.den, self.hodina, self.minuta, self.sekunda
        )
    }

    /// Konverzia na milisekundy slúži na rýchle výpočty trvania medzi udalosťami v logu.
    /// Zjednodušene počíta dni od 1. januára, pretože beh hry málokedy prekročí jeden rok.
    pub fn ms_od_zaciatku_roka(&self) -> i64 {
        let priestupny = self.rok % 4 == 0 && (self.rok % 100 != 0 || self.rok % 400 == 0);
        let mut dni_v_mesiacoch = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        if priestupny {
            dni_v_mesiacoch[1] = 29;
        }

        let mut dni = (self.den as i64) - 1;
        for i in 0..((self.mesiac as usize).saturating_sub(1).min(11)) {
            dni += dni_v_mesiacoch[i] as i64;
        }

        dni * 86_400_000
            + (self.hodina as i64) * 3_600_000
            + (self.minuta as i64) * 60_000
            + (self.sekunda as i64) * 1_000
            + (self.ms as i64)
    }

    /// Bezpečný výpočet rozdielu časov rieši aj beh hry počas Silvestra, kedy sa rok
    /// preklopí a jednoduchý rozdiel by vrátil masívne záporné číslo.
    pub fn rozdiel_ms(&self, skorsi: &Cas) -> i64 {
        let mut diff = self.ms_od_zaciatku_roka() - skorsi.ms_od_zaciatku_roka();
        
        // Ak je rozdiel menší ako pol roka dozadu, zrejme došlo k prechodu na nový rok.
        if diff < -15_552_000_000 {
            let priestupny = skorsi.rok % 4 == 0 && (skorsi.rok % 100 != 0 || skorsi.rok % 400 == 0);
            let dni_v_roku = if priestupny { 366 } else { 365 };
            diff += dni_v_roku * 86_400_000;
        }
        
        diff
    }
}

/// Združuje všetky vlastnosti aktualizácie slotu v inventári, pretože hra
/// hlási kompletný stav slotu pri každej, aj menšej zmene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatohZmena {
    pub stranka: u32,
    pub slot: u32,
    pub polozka: u32,
    pub num: i64,
    pub je_init: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Smer {
    Odoslana,
    Prijata,
    Push,
}

/// Enumerácia typov udalostí uľahčuje typovo bezpečné spracovanie logu vo vyšších
/// vrstvách aplikácie bez nutnosti opätovného parsovania textu.
#[derive(Debug, Clone, PartialEq)]
pub enum Udalost {
    Batoh(BatohZmena),
    BatohOdstranenie { stranka: u32, slot: u32 },
    Kontext { proto: String, zaciatok: bool },
    Zona { cesta: String },
    Uroven { uid: u64, typ: u32, id: u64 },
    SpravaZaciatok { smer: Smer, akcia: String, syn_id: u64 },
    SpravaKoniec,
    TeloSpravy { surovy: String },
    /// `AttributeMgr:Log(BaseId=X, GoldId=Y) DPS: stara -> nova` (aj `DEF:`) —
    /// hra si prepočíta vlastný výkonový ukazovateľ pri zmene výbavy/talentov/
    /// pactu. `nazov` je zámerne `String`, nie enum: hra môže časom logovať aj
    /// iné štatistiky týmto istým tvarom (TLI 12, `log-format.md` §10).
    VykonPostavy { base_id: u64, gold_id: u64, nazov: String, stara: f64, nova: f64 },
}

/// Helper na vyťahovanie hodnôt formátu Kľúč=Hodnota. Povoľuje chýbajúce aj
/// viacnásobné medzery, pretože formátovač v engine je nekonzistentný.
/// Zastaví sa aj na `,`/`)` — potrebné pre tvary ako `AttributeMgr:Log(BaseId=X, GoldId=Y)`,
/// kde `Kľúč=Hodnota` nekončí medzerou ani `]`, ale zoznamom v zátvorke.
fn extract_val<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let i = s.find(key)?;
    let mut rest = s[i + key.len()..].trim_start();
    if rest.starts_with('=') {
        rest = rest[1..].trim_start();
    }
    let end = rest
        .find(|c: char| c.is_whitespace() || c == ']' || c == ',' || c == ')')
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(&rest[..end])
}

/// `AttributeMgr:Log(BaseId=3210, GoldId=112139) DPS: 2839721.56 -> 2581565.05`
/// Izolované do vlastnej funkcie kvôli čitateľnosti — `parsuj_riadok` by inak
/// mal jeden obrovský reťazec podmienok navyše.
fn parsuj_vykon_postavy(riadok: &str) -> Option<Udalost> {
    let znacka = "AttributeMgr:Log(";
    let od = riadok.find(znacka)? + znacka.len();
    let rest = &riadok[od..];
    let base_id: u64 = extract_val(rest, "BaseId")?.parse().ok()?;
    let gold_id: u64 = extract_val(rest, "GoldId")?.parse().ok()?;
    let zavierka = rest.find(')')?;
    let chvost = rest[zavierka + 1..].trim_start();
    let dvojbodka = chvost.find(':')?;
    let nazov = chvost[..dvojbodka].trim().to_string();
    if nazov.is_empty() {
        return None;
    }
    let hodnoty = chvost[dvojbodka + 1..].trim();
    let mut casti = hodnoty.split("->").map(|s| s.trim());
    let stara: f64 = casti.next()?.parse().ok()?;
    let nova: f64 = casti.next()?.parse().ok()?;
    Some(Udalost::VykonPostavy { base_id, gold_id, nazov, stara, nova })
}

/// Izoluje logiku parsovania jedného riadku kvôli testovateľnosti a čistote.
/// V prípade, že formát nepatrí medzi známe, bezpečne ignoruje riadok.
pub fn parsuj_riadok(riadok: &str) -> Option<(Option<Cas>, Udalost)> {
    let cas = parsuj_cas(riadok);

    if riadok.contains("BagMgr@:Modfy BagItem") || riadok.contains("BagMgr@:InitBagData") {
        let je_init = riadok.contains("InitBagData");
        let stranka = extract_val(riadok, "PageId").and_then(|s| s.parse().ok()).unwrap_or(0);
        let slot = extract_val(riadok, "SlotId").and_then(|s| s.parse().ok()).unwrap_or(0);
        let polozka = extract_val(riadok, "ConfigBaseId").and_then(|s| s.parse().ok()).unwrap_or(0);
        let mut num = extract_val(riadok, "Num").and_then(|s| s.parse().ok()).unwrap_or(0);
        if num < 0 {
            num = 0;
        }
        return Some((cas, Udalost::Batoh(BatohZmena { stranka, slot, polozka, num, je_init })));
    }

    if riadok.contains("BagMgr@:RemoveBagItem") {
        let stranka = extract_val(riadok, "PageId").and_then(|s| s.parse().ok()).unwrap_or(0);
        let slot = extract_val(riadok, "SlotId").and_then(|s| s.parse().ok()).unwrap_or(0);
        return Some((cas, Udalost::BatohOdstranenie { stranka, slot }));
    }

    if riadok.contains("AttributeMgr:Log(") {
        if let Some(udalost) = parsuj_vykon_postavy(riadok) {
            return Some((cas, udalost));
        }
    }

    if riadok.contains("ItemChange@") && riadok.contains("ProtoName") {
        if let Some(proto) = extract_val(riadok, "ProtoName") {
            // Značka `start`/`end` stojí AŽ ZA menom protokolu. Hľadať ju v celom
            // riadku by zlyhalo na proto mene, ktoré samo obsahuje „start“.
            let za_proto = riadok
                .find("ProtoName")
                .map(|i| &riadok[i + "ProtoName".len()..])
                .unwrap_or("");
            let znacka = za_proto.split_whitespace().nth(1).unwrap_or("");
            let zaciatok = znacka.eq_ignore_ascii_case("start");
            return Some((cas, Udalost::Kontext { proto: proto.to_string(), zaciatok }));
        }
    }

    // Zámerne sa nehľadá celá veta naraz: hra medzi slová občas vloží viac medzier
    // a jedna zmena vo formátovaní by inak zhodila detekciu hraníc behu.
    if riadok.contains("SceneLevelMgr@")
        && riadok.contains("OpenMainWorld")
        && riadok.contains("InMainLevelPath")
    {
        let i = riadok.find("InMainLevelPath").unwrap() + "InMainLevelPath".len();
        let mut cesta = riadok[i..].trim_start();
        if let Some(zvysok) = cesta.strip_prefix('=') {
            cesta = zvysok.trim_start();
        }
        return Some((cas, Udalost::Zona { cesta: cesta.trim().to_string() }));
    }

    // `LevelMgr@` je povinný: samotné „LevelId =" sa vyskytuje aj v telách
    // socketových správ a tie sa tu nesmú tváriť ako prechod úrovne.
    if riadok.contains("LevelMgr@") && riadok.contains("LevelUid") && riadok.contains("LevelId") {
        let i = riadok.find("LevelId").unwrap() + "LevelId".len();
        let rest = riadok[i..].trim_start();
        let rest = if let Some(z) = rest.strip_prefix('=') { z.trim_start() } else { rest };
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 3 {
            return Some((
                cas,
                Udalost::Uroven {
                    uid: parts[0].parse().unwrap_or(0),
                    typ: parts[1].parse().unwrap_or(0),
                    id: parts[2].parse().unwrap_or(0),
                },
            ));
        }
    }

    if let Some(_) = riadok.find("----Socket ") {
        if riadok.contains(" End----") {
            return Some((cas, Udalost::SpravaKoniec));
        }
        if let Some(stt) = riadok.find(" STT----") {
            let start_akcia = stt + 8;
            if let Some(end_akcia_rel) = riadok[start_akcia..].find("----") {
                let akcia = &riadok[start_akcia..start_akcia + end_akcia_rel];
                let smer = if riadok.contains("SendMessage") {
                    Smer::Odoslana
                } else if riadok.contains("RecvMessage") {
                    Smer::Prijata
                } else {
                    Smer::Push
                };
                let syn_id = extract_val(riadok, "SynId").and_then(|s| s.parse().ok()).unwrap_or(0);
                return Some((cas, Udalost::SpravaZaciatok { smer, akcia: akcia.to_string(), syn_id }));
            }
        }
    }

    let mut telo = riadok;
    if let Some(game_idx) = telo.find("[Game] ") {
        telo = &telo[game_idx + 7..];
    } else {
        while telo.starts_with('[') {
            if let Some(idx) = telo.find(']') {
                telo = telo[idx + 1..].trim_start();
            } else {
                break;
            }
        }
    }

    if telo.starts_with('+') || telo.starts_with('|') {
        return Some((cas, Udalost::TeloSpravy { surovy: telo.trim_end().to_string() }));
    }

    None
}

/// Extrahuje čas ručným indexovaním, pretože tvar `[YYYY.MM.DD-HH.MM.SS:ms]`
/// je konštantný a nevyžaduje ťažký chronologický parser.
pub fn parsuj_cas(riadok: &str) -> Option<Cas> {
    let z = riadok.find('[')?;
    let k = riadok[z..].find(']')? + z;
    let vnutro = &riadok[z + 1..k];
    
    let casti: Vec<&str> = vnutro.split(|c| c == '.' || c == '-' || c == ':').collect();
    if casti.len() == 7 {
        return Some(Cas {
            rok: casti[0].parse().ok()?,
            mesiac: casti[1].parse().ok()?,
            den: casti[2].parse().ok()?,
            hodina: casti[3].parse().ok()?,
            minuta: casti[4].parse().ok()?,
            sekunda: casti[5].parse().ok()?,
            ms: casti[6].parse().ok()?,
        });
    }
    None
}

/// Transformuje vizuálnu štruktúru výpisu (strom s využitím `|` a odsadenia) 
/// na jednoznačnú plošnú mapu, ktorá sa ľahšie filtruje a spracúva.
pub fn parsuj_telo(riadky: &[String]) -> BTreeMap<String, String> {
    let mut vysledok = BTreeMap::new();
    let mut minuly_riadok = String::new();

    for riadok in riadky {
        let mut telo = riadok.trim_end();
        let mut hodnota = None;

        if telo.ends_with(']') {
            if let Some(pos) = telo.rfind(" [") {
                hodnota = Some(&telo[pos + 2..telo.len() - 1]);
                telo = &telo[..pos];
            }
        }

        let cesta_str = if telo.starts_with('|') {
            if let Some(plus_idx) = telo.find('+') {
                let byte_idx = minuly_riadok
                    .char_indices()
                    .nth(plus_idx)
                    .map(|(i, _)| i)
                    .unwrap_or(minuly_riadok.len());
                
                let prefix = &minuly_riadok[..byte_idx];
                format!("{}{}", prefix, &telo[plus_idx..])
            } else {
                telo.to_string()
            }
        } else {
            telo.trim_start().to_string()
        };

        minuly_riadok = cesta_str.clone();

        if let Some(h) = hodnota {
            let kluce: Vec<&str> = cesta_str.split('+').filter(|k| !k.is_empty()).collect();
            let cesta = kluce.join(".");
            vysledok.insert(cesta, h.to_string());
        }
    }

    vysledok
}

/// Kódovanie textového logu, rozpoznané podľa BOM na jeho úplnom začiatku
/// (`log-format.md` §0). Unreal prepne na UTF-16 LE, len čo sa do logu
/// dostane znak mimo ANSI (napr. emoji v mene postavy) — preto sa to nedá
/// predpokladať vopred a treba to zistiť z bajtov.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Kodovanie {
    #[default]
    Utf8,
    Utf16Le,
    Utf16Be,
}

impl Kodovanie {
    /// Rozpozná kódovanie z BOM na začiatku `bajty` a vráti ho spolu
    /// s dĺžkou samotnej BOM značky (tú treba pred dekódovaním odstrihnúť).
    /// Keď BOM chýba, ide o obyčajné UTF-8 bez značky.
    pub fn z_bom(bajty: &[u8]) -> (Kodovanie, usize) {
        if bajty.starts_with(&[0xFF, 0xFE]) {
            (Kodovanie::Utf16Le, 2)
        } else if bajty.starts_with(&[0xFE, 0xFF]) {
            (Kodovanie::Utf16Be, 2)
        } else if bajty.starts_with(&[0xEF, 0xBB, 0xBF]) {
            (Kodovanie::Utf8, 3)
        } else {
            (Kodovanie::Utf8, 0)
        }
    }

    /// Lossy konverzia garantuje, že parser nikdy nepadne na nevalidnom
    /// znaku. `bajty` tu už BEZ BOM — tú odstrihol volajúci podľa `z_bom`.
    pub fn dekoduj(&self, bajty: &[u8]) -> String {
        match self {
            Kodovanie::Utf16Le => {
                let u16_bajty: Vec<u16> = bajty
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&u16_bajty)
            }
            Kodovanie::Utf16Be => {
                let u16_bajty: Vec<u16> = bajty
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&u16_bajty)
            }
            Kodovanie::Utf8 => String::from_utf8_lossy(bajty).into_owned(),
        }
    }
}

/// Jednorazové dekódovanie CELÉHO obsahu (so svojou vlastnou BOM na začiatku).
/// Na čítanie logu po menších dávkach (kde BOM príde len v prvej z nich) slúži
/// `StavChvosta` v `chvost.rs` — ten si kódovanie zapamätá naprieč dávkami.
pub fn dekoduj(bajty: &[u8]) -> String {
    let (kodovanie, posun) = Kodovanie::z_bom(bajty);
    kodovanie.dekoduj(&bajty[posun..])
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn test_batoh_zmena_bez_init() {
        let r = "[2025.10.18-08.57.57:488][486]GameLog: Display: [Game] BagMgr@:Modfy BagItem PageId = 103 SlotId = 23 ConfigBaseId = 400021 Num = 5";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::Batoh(BatohZmena { stranka: 103, slot: 23, polozka: 400021, num: 5, je_init: false }));
    }

    #[test]
    fn test_batoh_zmena_s_init() {
        let r = "BagMgr@:InitBagData PageId = 1 SlotId=2 ConfigBaseId=3 Num=4";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::Batoh(BatohZmena { stranka: 1, slot: 2, polozka: 3, num: 4, je_init: true }));
    }

    #[test]
    fn test_batoh_odstranenie() {
        let r = "BagMgr@:RemoveBagItem PageId=1 SlotId = 2";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::BatohOdstranenie { stranka: 1, slot: 2 });
    }

    #[test]
    fn test_kontext() {
        let r = "ItemChange@ ProtoName=PickItems start";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::Kontext { proto: "PickItems".to_string(), zaciatok: true });
    }

    #[test]
    fn test_zona() {
        let r = "SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Maps/Zone1";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::Zona { cesta: "/Game/Maps/Zone1".to_string() });
    }

    #[test]
    fn test_uroven() {
        let r = "LevelMgr@ LevelUid, LevelType, LevelId = 123 456 789";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::Uroven { uid: 123, typ: 456, id: 789 });
    }

    #[test]
    fn test_sprava_zaciatok() {
        let r = "----Socket SendMessage STT----XchgSearchPrice----SynId = 11942";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::SpravaZaciatok { smer: Smer::Odoslana, akcia: "XchgSearchPrice".to_string(), syn_id: 11942 });
    }

    #[test]
    fn test_sprava_koniec() {
        let r = "----Socket RecvMessage End----";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::SpravaKoniec);
    }

    #[test]
    fn test_telo_spravy() {
        let r = "[cas][ok]GameLog: Display: [Game] +prices+1+currency [100300]";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::TeloSpravy { surovy: "+prices+1+currency [100300]".to_string() });
    }

    #[test]
    fn test_bez_prefixu() {
        let r = "BagMgr@:RemoveBagItem PageId=9 SlotId=9";
        let (cas, ud) = parsuj_riadok(r).unwrap();
        assert!(cas.is_none());
        assert_eq!(ud, Udalost::BatohOdstranenie { stranka: 9, slot: 9 });
    }

    #[test]
    fn test_viacnasobne_medzery() {
        let r = "BagMgr@:RemoveBagItem PageId  =   9   SlotId = 9";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::BatohOdstranenie { stranka: 9, slot: 9 });
    }

    #[test]
    fn test_neznama_udalost() {
        let r = "[cas]GameLog: Display: [Game] Random string that does not match";
        let vysledok = parsuj_riadok(r);
        assert!(vysledok.is_none());
    }

    #[test]
    fn test_zaporne_num() {
        let r = "BagMgr@:Modfy BagItem PageId=1 SlotId=1 ConfigBaseId=1 Num = -5";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(ud, Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 1, num: 0, je_init: false }));
    }

    #[test]
    fn test_parsuj_telo() {
        let riadky = vec![
            "+worldInitArgs+ownerPlayerId [814971382814871552]".to_string(),
            "|             +seed [1532413606]".to_string(),
        ];
        let mapa = parsuj_telo(&riadky);
        assert_eq!(mapa.get("worldInitArgs.ownerPlayerId").unwrap(), "814971382814871552");
        assert_eq!(mapa.get("worldInitArgs.seed").unwrap(), "1532413606");
    }

    #[test]
    fn test_vykon_postavy_dps() {
        let r = "[2026.10.14-22.23.06:532][103]TLLua: Display: [Game] AttributeMgr:Log(BaseId=3210, GoldId=112139) DPS: 2839721.5618293 -> 2581565.0562085";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(
            ud,
            Udalost::VykonPostavy {
                base_id: 3210,
                gold_id: 112139,
                nazov: "DPS".to_string(),
                stara: 2839721.5618293,
                nova: 2581565.0562085,
            }
        );
    }

    #[test]
    fn test_vykon_postavy_def_bez_goldid() {
        let r = "AttributeMgr:Log(BaseId=4727, GoldId=0) DEF: 100.5 -> 120.25";
        let (_, ud) = parsuj_riadok(r).unwrap();
        assert_eq!(
            ud,
            Udalost::VykonPostavy {
                base_id: 4727,
                gold_id: 0,
                nazov: "DEF".to_string(),
                stara: 100.5,
                nova: 120.25,
            }
        );
    }

    #[test]
    fn test_vykon_postavy_nezhoda_sa_ignoruje() {
        // Zámerne pokazený tvar (chýba šípka) — nesmie spadnúť, len sa ignoruje.
        let r = "AttributeMgr:Log(BaseId=1, GoldId=2) DPS: 100.0 iba jedna hodnota";
        assert!(parsuj_riadok(r).is_none());
    }

    #[test]
    fn test_dekoduj() {
        assert_eq!(dekoduj(&[0xFF, 0xFE, 0x41, 0x00]), "A"); // UTF-16 LE
        assert_eq!(dekoduj(&[0xFE, 0xFF, 0x00, 0x41]), "A"); // UTF-16 BE
        assert_eq!(dekoduj(&[0xEF, 0xBB, 0xBF, 0x41]), "A"); // UTF-8 BOM
        assert_eq!(dekoduj(&[0x41]), "A"); // UTF-8
    }

    #[test]
    fn test_rozdiel_ms() {
        let stary = Cas { rok: 2024, mesiac: 12, den: 31, hodina: 23, minuta: 59, sekunda: 59, ms: 0 };
        let novy = Cas { rok: 2025, mesiac: 1, den: 1, hodina: 0, minuta: 0, sekunda: 1, ms: 0 };
        assert_eq!(novy.rozdiel_ms(&stary), 2000);
    }


}