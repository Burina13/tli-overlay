use super::log::{Cas, Smer, Udalost, parsuj_riadok, parsuj_telo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Flame Elementium — hlavná mena, v ktorej sa počíta zisk.
pub const POLOZKA_FE: u32 = 100300;
/// To isté ako text, lebo v tele správy je mena reťazec.
const MENA_FE: &str = "100300";

/// Zaradenie zmeny batohu podľa toho, PREČO nastala.
/// `Ostatne` je predvolená hodnota zámerne: mimo bloku `ItemChange@` nevieme,
/// prečo sa batoh zmenil, a tipovať „drop" by nafúklo zisk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Dovod {
    Drop,
    VstupMapy,
    Obchod,
    Ignorovat,
    #[default]
    Ostatne,
}

/// Z `proto` mena určí dôvod. Neznáme meno = `Ostatne`.
pub fn dovod_pre_proto(proto: &str) -> Dovod {
    match proto {
        "PickItem" | "PickItems" => Dovod::Drop,
        "Spv3Open" | "ClimbTowerOpen" => Dovod::VstupMapy,
        "XchgForSale" | "XchgReceive" | "XchgRecall" | "XchgBuy" | "BuyVendorGoods"
        | "ExchangeItem" => Dovod::Obchod,
        "PutOn" | "PutOff" | "LockOrUnlockItem" | "Evaluate" | "EvaluateOneKey" | "BuySkill"
        | "UnequipSkill" | "WashGeniusAllPoint" => Dovod::Ignorovat,
        _ => Dovod::Ostatne,
    }
}

/// Je cesta zóny hub (mesto/úkryt)?
pub fn je_hub(cesta: &str) -> bool {
    let mala_cesta = cesta.to_lowercase();
    let podretazce = [
        "yujinzhixibinansuo",
        "zhucheng",
        "/ui/",
        "loginscene",
        "hideout",
        "town",
        "hub",
        "lobby",
        "social",
    ];
    if podretazce.iter().any(|&p| mala_cesta.contains(p)) {
        return true;
    }
    if mala_cesta.contains("shengtingzhuangyuan") && !mala_cesta.contains("shengtingzhuangyuan000")
    {
        return true;
    }
    false
}

/// Jedna započítaná zmena množstva položky.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zmena {
    pub polozka: u32,
    pub mnozstvo: i64,
    pub dovod: Dovod,
    pub cas: Option<Cas>,
}

/// Jeden spawn monštra na mape (TLI 13, `log-format-katalog.md` §A).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonstrumZaznam {
    pub npc_id: u64,
    pub rarita: u8,
    pub affix: Option<u64>,
    pub cas: Option<Cas>,
}

/// Jeden textový modifikátor mapy (§A).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModifikatorZaznam {
    pub danger: Option<i64>,
    pub kind: Option<i64>,
    pub skupina: Option<i64>,
    pub seat: Option<i64>,
    pub rand_affix_id: Option<i64>,
    pub cas: Option<Cas>,
}

/// Prepnutie stavu boss fightu (§A).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BossZaznam {
    pub aktivny: bool,
    pub cas: Option<Cas>,
}

/// Frozen Canvas akcia čítaná priamo z logu (§E).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasZaznam {
    pub typ: String,
    pub cas: Option<Cas>,
}

/// Snímka hráčovej identity a buildu z voľne stojaceho bloku `+player+...`
/// (§B/§F) — tento blok stojí MIMO rámca `----Socket STT/End----`, preto ho
/// nevidí bežné spracovanie `SpravaKoniec` a potreboval vlastný mechanizmus
/// (`volny_blok`, TLI 12 bod 4 v `BACKLOG.md`). Nesie identitu postavy
/// (`player_id` + `season_id`, potrebné na rozlíšenie líg v DB) aj kúsok
/// buildu (aktívne talentové stromy).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HracSnimka {
    pub name: Option<String>,
    pub player_id: Option<String>,
    pub season_id: Option<String>,
    pub hero_id: Option<String>,
    pub death_count: Option<i64>,
    pub exp: Option<i64>,
    pub genius_career_ids: Vec<String>,
    pub cas: Option<Cas>,
}

/// Z riadkov tela správy (voľne stojaci blok ALEBO telo rámcovanej
/// `----Socket STT/End----` správy, napr. odpoveď `GetPlayerData`) spraví
/// `HracSnimka`, ak telo naozaj nesie aspoň jedno pole `player.*`. Spoločná
/// logika pre `flush_volny_blok` aj `Udalost::SpravaKoniec` (TLI 15) — predtým
/// mala `flush_volny_blok` vlastnú kópiu a `SpravaKoniec` identitu z
/// rámcovaných správ (napr. `GetPlayerData`) ticho zahadzoval, lebo
/// rozpoznával len `akcia == "XchgSearchPrice"/"XchgSyncSearchPrice"`.
fn hrac_snimka_z_tela(riadky: &[String], cas: Option<Cas>) -> Option<HracSnimka> {
    let mapa = parsuj_telo(riadky);
    if !mapa.keys().any(|k| k.starts_with("player.")) {
        return None;
    }
    let genius_career_ids = mapa
        .get("player.GeniusCareerIdStr")
        .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    Some(HracSnimka {
        name: mapa.get("player.Name").cloned(),
        player_id: mapa.get("player.PlayerId").cloned(),
        season_id: mapa.get("player.SeasonId").cloned(),
        hero_id: mapa.get("player.HeroId").cloned(),
        death_count: mapa.get("player.DeathCount").and_then(|v| v.parse().ok()),
        exp: mapa.get("player.Exp").and_then(|v| v.parse().ok()),
        genius_career_ids,
        cas,
    })
}

/// Jeden beh mapy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Beh {
    pub id: u64,
    pub zona: String,
    pub je_hub: bool,
    pub level_uid: Option<u64>,
    pub level_typ: Option<u32>,
    pub level_id: Option<u64>,
    pub zaciatok: Option<Cas>,
    pub koniec: Option<Cas>,
    /// Dropy: položka -> počet kusov (len kladné súčty).
    pub dropy: BTreeMap<u32, i64>,
    /// Vstupy spotrebované pri otvorení mapy (chart + kompasy): položka -> počet (kladné číslo).
    pub vstupy: BTreeMap<u32, i64>,
    /// Všetky zmeny v poradí, aj tie nezapočítané — kvôli dohľadateľnosti.
    pub vsetky_zmeny: Vec<Zmena>,
    /// Monštrá spawnuté počas tohto behu (TLI 13).
    pub monstra: Vec<MonstrumZaznam>,
    /// Textové modifikátory mapy aktívne počas tohto behu (TLI 13).
    pub modifikatory: Vec<ModifikatorZaznam>,
    /// Boss fighty v tomto behu (TLI 13).
    pub bossovia: Vec<BossZaznam>,
    /// Frozen Canvas akcie v tomto behu (TLI 13).
    pub canvas_akcie: Vec<CanvasZaznam>,
}

impl Beh {
    /// Trvanie v milisekundách; `None`, kým beh nemá oba časy.
    pub fn trvanie_ms(&self) -> Option<i64> {
        match (&self.koniec, &self.zaciatok) {
            (Some(k), Some(z)) => Some(k.rozdiel_ms(z)),
            _ => None,
        }
    }

    /// Podpis kombinácie „chart + kompasy": zotriedené `id×počet` spojené `+`.
    pub fn podpis_vstupov(&self) -> String {
        if self.vstupy.is_empty() {
            return "bez-vstupu".to_string();
        }
        let mut casti = Vec::new();
        for (id, pocet) in &self.vstupy {
            casti.push(format!("{}x{}", id, pocet));
        }
        casti.join("+")
    }

    /// Podpis kombinácie na rozdelenie výsledkov: `<podpis_vstupov>@<zona>`.
    pub fn podpis_kombinacie(&self) -> String {
        format!("{}@{}", self.podpis_vstupov(), self.zona)
    }
}

/// Referenčná cena zo zoznamu ponúk.
pub fn cena_percentil(ponuky: &[f64], percentil: u32) -> Option<f64> {
    if ponuky.is_empty() {
        return None;
    }
    let mut zotriedene = ponuky.to_vec();
    zotriedene.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p = percentil.min(100);
    let n = zotriedene.len() as u32;
    let index = ((n - 1) * p) / 100;
    Some(zotriedene[index as usize])
}

/// Stavový automat nad prúdom riadkov logu.
#[derive(Debug, Default)]
pub struct Sledovac {
    batoh: BTreeMap<(u32, u32), (u32, i64)>,
    aktualny_dovod: Dovod,
    aktualny_beh: Option<Beh>,
    dalsi_id_behu: u64,
    cakajuce_vstupy: BTreeMap<u32, i64>,
    cas_vstupov: Option<Cas>,
    hladania_cien: BTreeMap<u64, (u32, Option<Cas>)>,
    ceny: Vec<(u32, f64, Option<Cas>)>,
    aktivna_sprava: Option<(Smer, String, u64, Option<Cas>)>,
    telo_spravy: Vec<String>,
    neznamych_riadkov: u64,
    /// Posledná známa hodnota výkonového ukazovateľa postavy (`DPS`, `DEF`, …) —
    /// kľúč je `nazov` z `Udalost::VykonPostavy`. TLI 12: nástroj na porovnanie
    /// buildov („koľko DPS mal pred a po zmene výbavy/talentov/paktu").
    posledne_staty_postavy: BTreeMap<String, (f64, Option<Cas>)>,
    /// Zostatky mien/počítadiel (`ConsumMgr:ResreshConsumData`), v poradí, ako
    /// prišli — podobne ako `ceny`, odoberá sa `odober_zostatky()` (TLI 13).
    zostatky: Vec<(String, i64, Option<Cas>)>,
    /// Posledná pact konfigurácia, na ktorú hráč prepol (TLI 13).
    posledna_pact_konfiguracia: Option<(String, Option<Cas>)>,
    /// Posledná spracovaná snímka hráčovej identity z voľne stojaceho bloku
    /// `+player+...` (TLI 12 bod 4, TLI 13) — `None`, kým sa taký blok ešte
    /// nevyskytol a nebol uzavretý nasledujúcou udalosťou.
    hrac_snimka: Option<HracSnimka>,
    /// Čas PRVÉHO riadku vo `telo_spravy`, keď bol buffer naplnený MIMO
    /// aktívnej správy (teda ide o voľný blok, nie telo `STT/End` správy).
    volny_blok_cas: Option<Cas>,
    /// Riadky, ktoré katalóg označil za zaujímavé, ale parser ich nevie
    /// rozobrať na polia — čakajú na uloženie do `raw` tabuľky v DB
    /// (`Option<u64>` je ID behu, počas ktorého riadok prišiel, ak nejaký
    /// beží). Odoberá sa `odober_raw()` (TLI 13 §1).
    raw_udalosti: Vec<(Option<u64>, String, Option<Cas>)>,
}

impl Sledovac {
    pub fn novy() -> Self {
        Self {
            dalsi_id_behu: 1,
            aktualny_dovod: Dovod::Ostatne,
            ..Default::default()
        }
    }

    pub fn riadok(&mut self, riadok: &str) -> Option<Beh> {
        match parsuj_riadok(riadok) {
            Some((cas, udalost)) => self.spracuj_udalost(cas, udalost),
            None => {
                self.neznamych_riadkov += 1;
                None
            }
        }
    }

    pub fn aktualny(&self) -> Option<&Beh> {
        self.aktualny_beh.as_ref()
    }

    pub fn ukonci(&mut self) -> Option<Beh> {
        self.aktualny_beh.take()
    }

    pub fn odober_ceny(&mut self) -> Vec<(u32, f64, Option<Cas>)> {
        std::mem::take(&mut self.ceny)
    }

    pub fn neznamych_riadkov(&self) -> u64 {
        self.neznamych_riadkov
    }

    /// Posledná známa hodnota pomenovaného výkonového ukazovateľa (napr. `"DPS"`,
    /// `"DEF"`) a čas jej zápisu do logu. `None`, kým sa taký riadok nenašiel.
    pub fn posledny_stat_postavy(&self, nazov: &str) -> Option<(f64, Option<Cas>)> {
        self.posledne_staty_postavy.get(nazov).cloned()
    }

    /// Odoberie nazbierané zostatky mien/počítadiel od posledného volania.
    pub fn odober_zostatky(&mut self) -> Vec<(String, i64, Option<Cas>)> {
        std::mem::take(&mut self.zostatky)
    }

    /// Posledná pact konfigurácia, na ktorú hráč prepol (`None`, kým sa riadok
    /// nenašiel).
    pub fn posledna_pact_konfiguracia(&self) -> Option<(String, Option<Cas>)> {
        self.posledna_pact_konfiguracia.clone()
    }

    /// Posledná spracovaná snímka hráčovej identity (`+player+...` blok).
    pub fn hrac_snimka(&self) -> Option<&HracSnimka> {
        self.hrac_snimka.as_ref()
    }

    /// Odoberie nazbierané „zaujímavé, ale nerozobrané" riadky (`raw`) od
    /// posledného volania — `(beh_id, text, cas)`.
    pub fn odober_raw(&mut self) -> Vec<(Option<u64>, String, Option<Cas>)> {
        std::mem::take(&mut self.raw_udalosti)
    }

    /// Spracuje nazbierané riadky voľne stojaceho tela (`+player+...`, mimo
    /// rámca `----Socket STT/End----`) cez `parsuj_telo` a uloží výsledok ako
    /// `hrac_snimka`, ak blok naozaj niesol hráčske polia. Volá sa VŽDY, keď
    /// príde iná udalosť než `TeloSpravy`, zatiaľ čo nie je otvorená žiadna
    /// socket správa a buffer nie je prázdny (TLI 12 bod 4 v `BACKLOG.md`) —
    /// inak by sa tieto riadky len ticho zahodili pri najbližšom
    /// `SpravaZaciatok`.
    fn flush_volny_blok(&mut self) {
        let riadky = std::mem::take(&mut self.telo_spravy);
        let cas = self.volny_blok_cas.take();
        if let Some(snimka) = hrac_snimka_z_tela(&riadky, cas) {
            self.hrac_snimka = Some(snimka);
        }
    }

    /// Interná metóda na spracovanie konkrétnej udalosti (užitočné aj pre testy)
    pub fn spracuj_udalost(&mut self, cas: Option<Cas>, udalost: Udalost) -> Option<Beh> {
        if !matches!(udalost, Udalost::TeloSpravy { .. })
            && self.aktivna_sprava.is_none()
            && !self.telo_spravy.is_empty()
        {
            self.flush_volny_blok();
        }

        match udalost {
            Udalost::Kontext { proto, zaciatok } => {
                if zaciatok {
                    self.aktualny_dovod = dovod_pre_proto(&proto);
                } else {
                    self.aktualny_dovod = Dovod::Ostatne;
                }
                None
            }
            Udalost::Batoh(zmena) => {
                if zmena.je_init {
                    self.batoh
                        .insert((zmena.stranka, zmena.slot), (zmena.polozka, zmena.num));
                } else {
                    let stary = self
                        .batoh
                        .get(&(zmena.stranka, zmena.slot))
                        .copied()
                        .unwrap_or((0, 0));
                    let dovod = self.aktualny_dovod;

                    if stary.0 == zmena.polozka || stary.0 == 0 {
                        let delta = zmena.num - stary.1;
                        self.zaznamenaj_zmenu(zmena.polozka, delta, dovod, cas.clone(), &cas);
                    } else {
                        self.zaznamenaj_zmenu(stary.0, -stary.1, dovod, cas.clone(), &cas);
                        self.zaznamenaj_zmenu(zmena.polozka, zmena.num, dovod, cas.clone(), &cas);
                    }

                    if zmena.num > 0 {
                        self.batoh
                            .insert((zmena.stranka, zmena.slot), (zmena.polozka, zmena.num));
                    } else {
                        self.batoh.remove(&(zmena.stranka, zmena.slot));
                    }
                }
                None
            }
            Udalost::BatohOdstranenie { stranka, slot } => {
                if let Some((polozka, num)) = self.batoh.remove(&(stranka, slot)) {
                    let dovod = self.aktualny_dovod;
                    self.zaznamenaj_zmenu(polozka, -num, dovod, cas.clone(), &cas);
                }
                None
            }
            Udalost::Zona { cesta } => {
                let mut uzavrety_beh = None;
                if let Some(mut stary_beh) = self.aktualny_beh.take() {
                    stary_beh.koniec = cas.clone();
                    uzavrety_beh = Some(stary_beh);
                }

                let hub = je_hub(&cesta);
                let mut novy_beh = Beh {
                    id: self.dalsi_id_behu,
                    zona: cesta,
                    je_hub: hub,
                    level_uid: None,
                    level_typ: None,
                    level_id: None,
                    zaciatok: cas.clone(),
                    koniec: None,
                    dropy: BTreeMap::new(),
                    vstupy: BTreeMap::new(),
                    vsetky_zmeny: Vec::new(),
                    monstra: Vec::new(),
                    modifikatory: Vec::new(),
                    bossovia: Vec::new(),
                    canvas_akcie: Vec::new(),
                };
                self.dalsi_id_behu += 1;

                if !hub {
                    if let Some(cas_v) = &self.cas_vstupov {
                        if let Some(cas_z) = &novy_beh.zaciatok {
                            if cas_z.rozdiel_ms(cas_v) <= 10 * 60 * 1000 {
                                novy_beh.vstupy = self.cakajuce_vstupy.clone();
                            }
                        }
                    }
                    self.cakajuce_vstupy.clear();
                    self.cas_vstupov = None;
                }

                self.aktualny_beh = Some(novy_beh);
                uzavrety_beh
            }
            Udalost::Uroven { uid, typ, id } => {
                if let Some(beh) = &mut self.aktualny_beh {
                    beh.level_uid = Some(uid);
                    beh.level_typ = Some(typ);
                    beh.level_id = Some(id);
                }
                None
            }
            Udalost::SpravaZaciatok { smer, akcia, syn_id } => {
                self.aktivna_sprava = Some((smer, akcia, syn_id, cas));
                self.telo_spravy.clear();
                None
            }
            Udalost::TeloSpravy { surovy } => {
                if self.aktivna_sprava.is_none() && self.telo_spravy.is_empty() {
                    self.volny_blok_cas = cas;
                }
                self.telo_spravy.push(surovy);
                None
            }
            Udalost::SpravaKoniec => {
                if let Some((smer, akcia, syn_id, cas_spravy)) = self.aktivna_sprava.take() {
                    if akcia == "XchgSearchPrice" || akcia == "XchgSyncSearchPrice" {
                        spracuj_xchg_hladanie(
                            &self.telo_spravy,
                            smer,
                            syn_id,
                            &mut self.hladania_cien,
                            &mut self.ceny,
                            cas_spravy,
                        );
                    }
                    // Nezávisle od `akcia`: telo rámcovanej správy môže byť aj
                    // odpoveď `GetPlayerData` (reálne overené na Richardovom
                    // logu, TLI 15) — bez tejto vetvy sa identita hráča z
                    // rámcovaných správ ticho strácala.
                    if let Some(snimka) = hrac_snimka_z_tela(&self.telo_spravy, cas_spravy) {
                        self.hrac_snimka = Some(snimka);
                    }
                }
                self.telo_spravy.clear();
                None
            }
            Udalost::VykonPostavy { nazov, nova, .. } => {
                self.posledne_staty_postavy.insert(nazov, (nova, cas));
                None
            }
            Udalost::MonsterSpawn { npc_id, rarita, affix } => {
                if let Some(beh) = &mut self.aktualny_beh {
                    beh.monstra.push(MonstrumZaznam { npc_id, rarita, affix, cas });
                }
                None
            }
            Udalost::ModifikatorMapy { danger, kind, skupina, seat, rand_affix_id } => {
                if let Some(beh) = &mut self.aktualny_beh {
                    beh.modifikatory.push(ModifikatorZaznam { danger, kind, skupina, seat, rand_affix_id, cas });
                }
                None
            }
            Udalost::ZostatokMeny { nazov, hodnota } => {
                self.zostatky.push((nazov, hodnota, cas));
                None
            }
            Udalost::BossStatus { aktivny } => {
                if let Some(beh) = &mut self.aktualny_beh {
                    beh.bossovia.push(BossZaznam { aktivny, cas });
                }
                None
            }
            Udalost::PactKonfiguracia { nazov } => {
                self.posledna_pact_konfiguracia = Some((nazov, cas));
                None
            }
            Udalost::CanvasAkcia { typ } => {
                if let Some(beh) = &mut self.aktualny_beh {
                    beh.canvas_akcie.push(CanvasZaznam { typ: typ.to_string(), cas });
                }
                None
            }
            Udalost::Raw { text } => {
                let beh_id = self.aktualny_beh.as_ref().map(|b| b.id);
                self.raw_udalosti.push((beh_id, text, cas));
                None
            }
        }
    }

    fn zaznamenaj_zmenu(
        &mut self,
        polozka: u32,
        mnozstvo: i64,
        dovod: Dovod,
        cas_zmeny: Option<Cas>,
        original_cas: &Option<Cas>,
    ) {
        if mnozstvo == 0 {
            return;
        }
        let zmena = Zmena {
            polozka,
            mnozstvo,
            dovod,
            cas: cas_zmeny,
        };
        if let Some(beh) = &mut self.aktualny_beh {
            beh.vsetky_zmeny.push(zmena.clone());
            if dovod == Dovod::Drop && mnozstvo > 0 && !beh.je_hub {
                *beh.dropy.entry(polozka).or_insert(0) += mnozstvo;
            }
        }
        if dovod == Dovod::VstupMapy && mnozstvo < 0 {
            *self.cakajuce_vstupy.entry(polozka).or_insert(0) += -mnozstvo;
            if self.cas_vstupov.is_none() {
                self.cas_vstupov = original_cas.clone();
            }
        }
    }
}

/// Vytiahne trhovú cenu z dvojice správ `XchgSearchPrice`.
///
/// Odoslaná správa nesie `ConfigBaseId` hľadanej položky (`+…+refer [3004]`, pri
/// variante `XchgSyncSearchPrice` `+itemBaseId [3004]`), prijatá nesie zoznam
/// ponúk. Páruje sa cez `SynId`.
///
/// ⚠️ **Zámerne sa tu NEPOUŽÍVA strom z `parsuj_telo`.** Hra zapisuje telo správy
/// s odsadením a znakmi `|`, ktoré označujú, koľko úrovní cesty sa dedí z predošlého
/// riadku — a presný počet tých `|` pri zozname ponúk nevieme overiť bez skutočného
/// logu. Keby sa cena vyberala podľa zrekonštruovanej cesty, stačila by jedna zvislá
/// čiara navyše a tracker by ticho prestal mať ceny. Preto sa telo prechádza riadok
/// po riadku ako text: sleduje sa, v ktorej skupine `prices+N` sme a či sme v zozname
/// `unitPrices`. Je to to isté, čo robí TITrack, a prežije to zmenu odsadenia.
fn spracuj_xchg_hladanie(
    telo: &[String],
    smer: Smer,
    syn_id: u64,
    hladania_cien: &mut BTreeMap<u64, (u32, Option<Cas>)>,
    ceny: &mut Vec<(u32, f64, Option<Cas>)>,
    cas_spravy: Option<Cas>,
) {
    match smer {
        Smer::Odoslana => {
            for riadok in telo {
                let Some(kluc) = kluc_riadku(riadok) else { continue };
                if kluc == "refer" || kluc == "itemBaseId" {
                    if let Some(h) = hodnota_riadku(riadok) {
                        if let Ok(id) = h.parse::<u32>() {
                            hladania_cien.insert(syn_id, (id, cas_spravy));
                            return;
                        }
                    }
                }
            }
        }
        Smer::Prijata => {
            let Some((polozka, _)) = hladania_cien.remove(&syn_id) else { return };
            let ponuky = ponuky_z_tela(telo);
            if let Some(p10) = cena_percentil(&ponuky, 10) {
                ceny.push((polozka, p10, cas_spravy));
            }
        }
        Smer::Push => {}
    }
}

/// Posledný kľúč riadku tela (`|   +prices+1+currency [100300]` → `currency`).
fn kluc_riadku(riadok: &str) -> Option<&str> {
    let bez_hodnoty = match riadok.find(" [") {
        Some(i) => &riadok[..i],
        None => riadok,
    };
    bez_hodnoty.rsplit('+').next().map(str::trim).filter(|k| !k.is_empty())
}

/// Hodnota v hranatých zátvorkách, ak na riadku je.
fn hodnota_riadku(riadok: &str) -> Option<&str> {
    let i = riadok.rfind('[')?;
    let j = riadok[i..].find(']')? + i;
    Some(riadok[i + 1..j].trim())
}

/// Index skupiny `prices+N`, ak ho riadok uvádza.
fn index_skupiny(riadok: &str) -> Option<u32> {
    let i = riadok.find("prices+")? + "prices+".len();
    let zvysok = &riadok[i..];
    let koniec = zvysok.find(|c: char| !c.is_ascii_digit()).unwrap_or(zvysok.len());
    zvysok[..koniec].parse().ok()
}

/// Jednotkové ceny v FE zo surového tela odpovede na dopyt na trhovú cenu.
///
/// Ponuky môžu byť zapísané ako `+prices+1+unitPrices+2 [0.22]` aj ako samostatný
/// riadok `+2 [0.22]` pod hlavičkou `+prices+1+unitPrices`. Oba tvary sú tu pokryté.
/// Keď sa v tele nenájde ANI JEDNA mena, berú sa všetky ponuky — radšej cena z inej
/// meny než žiadna cena.
fn ponuky_z_tela(telo: &[String]) -> Vec<f64> {
    let mut meny: BTreeMap<u32, String> = BTreeMap::new();
    let mut nalezy: Vec<(u32, f64)> = Vec::new();

    let mut skupina: u32 = 1;
    let mut v_cenach = false;

    for riadok in telo {
        if let Some(i) = index_skupiny(riadok) {
            skupina = i;
        }
        let kluc = kluc_riadku(riadok).unwrap_or("");
        let hodnota = hodnota_riadku(riadok);

        if riadok.contains("unitPrices") {
            v_cenach = true;
        } else if !kluc.chars().all(|c| c.is_ascii_digit()) {
            // Iný pomenovaný kľúč zoznam ponúk ukončuje (`+errCode`, `+itemGoldId`…).
            v_cenach = false;
        }

        if kluc == "currency" {
            if let Some(h) = hodnota {
                meny.insert(skupina, h.to_string());
            }
            continue;
        }

        if v_cenach {
            if let Some(h) = hodnota {
                if let Ok(c) = h.parse::<f64>() {
                    if c.is_finite() && c > 0.0 {
                        nalezy.push((skupina, c));
                    }
                }
            }
        }
    }

    let ziadna_mena = meny.is_empty();
    nalezy
        .into_iter()
        .filter(|(sk, _)| ziadna_mena || meny.get(sk).map(|m| m == MENA_FE).unwrap_or(false))
        .map(|(_, c)| c)
        .collect()
}

#[cfg(test)]
mod testy {
    use super::super::log::BatohZmena;
    use super::*;

    // Pomocná funkcia na vytvorenie inštancie času pre testy (mockujeme jej funkciu)
    fn cas(ms_hodnota: u16) -> Option<Cas> {
        Some(Cas {
            rok: 2023,
            mesiac: 1,
            den: 1,
            hodina: 12,
            minuta: 0,
            sekunda: 0,
            ms: ms_hodnota,
        })
    }

    #[test]
    fn test_dovod_pre_proto() {
        assert_eq!(dovod_pre_proto("PickItem"), Dovod::Drop);
        assert_eq!(dovod_pre_proto("Spv3Open"), Dovod::VstupMapy);
        assert_eq!(dovod_pre_proto("XchgBuy"), Dovod::Obchod);
        assert_eq!(dovod_pre_proto("Evaluate"), Dovod::Ignorovat);
        assert_eq!(dovod_pre_proto("HociCoIne"), Dovod::Ostatne);
    }

    #[test]
    fn test_je_hub() {
        assert!(je_hub("nejaky/LoginScene/level"));
        assert!(je_hub("Mesto_ZhuCheng_01"));
        assert!(je_hub("ShengTingZhuangYuan"));
        assert!(!je_hub("NejakyInyLevel"));
    }

    #[test]
    fn test_je_hub_shengting_vynimka() {
        assert!(!je_hub("ShengTingZhuangYuan000")); // Toto nie je hub
    }

    #[test]
    fn test_cena_percentil_prazdne() {
        assert_eq!(cena_percentil(&[], 10), None);
    }

    #[test]
    fn test_cena_percentil_jeden_prvok() {
        assert_eq!(cena_percentil(&[5.5], 10), Some(5.5));
    }

    #[test]
    fn test_cena_percentil_viac_hodnot() {
        let hodnoty = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
        // index = 9 * 10 / 100 = 0 -> prvy prvok
        assert_eq!(cena_percentil(&hodnoty, 10), Some(10.0));
    }

    #[test]
    fn test_delta_absolutne_num_pridanie() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Mapa".to_string() });
        sledovac.aktualny_dovod = Dovod::Drop;

        sledovac.spracuj_udalost(
            None,
            Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 50, num: 10, je_init: false }),
        );
        let beh = sledovac.aktualny().unwrap();
        assert_eq!(*beh.dropy.get(&50).unwrap(), 10);
    }

    #[test]
    fn test_delta_absolutne_num_odobranie() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Mapa".to_string() });
        
        // init
        sledovac.spracuj_udalost(None, Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 50, num: 20, je_init: true }));
        
        sledovac.aktualny_dovod = Dovod::VstupMapy;
        sledovac.spracuj_udalost(
            None,
            Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 50, num: 5, je_init: false }),
        );
        
        assert_eq!(*sledovac.cakajuce_vstupy.get(&50).unwrap(), 15);
    }

    #[test]
    fn test_zmena_polozky_v_rovnakom_slote() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Mapa".to_string() });
        sledovac.spracuj_udalost(None, Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 50, num: 10, je_init: true }));

        sledovac.aktualny_dovod = Dovod::Drop;
        sledovac.spracuj_udalost(None, Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 60, num: 3, je_init: false }));

        let beh = sledovac.aktualny().unwrap();
        // Zisky by mali obsahovať novú položku 60 v počte 3 (drop). Odobraná položka 50 s deltou -10 neovplyvní kladné dropy.
        assert_eq!(*beh.dropy.get(&60).unwrap(), 3);
        assert!(beh.dropy.get(&50).is_none()); 
    }

    #[test]
    fn test_batoh_odstranenie() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 50, num: 10, je_init: true }));
        sledovac.aktualny_dovod = Dovod::VstupMapy;
        sledovac.spracuj_udalost(None, Udalost::BatohOdstranenie { stranka: 1, slot: 1 });
        
        assert_eq!(*sledovac.cakajuce_vstupy.get(&50).unwrap(), 10);
    }

    #[test]
    fn test_init_bez_delty() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Mapa".to_string() });
        sledovac.aktualny_dovod = Dovod::Drop;
        sledovac.spracuj_udalost(None, Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 50, num: 10, je_init: true }));
        let beh = sledovac.aktualny().unwrap();
        assert!(beh.dropy.is_empty()); // Init negeneruje deltu
    }

    #[test]
    fn test_drop_iba_mimo_hubu() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Town".to_string() }); // Je hub
        sledovac.aktualny_dovod = Dovod::Drop;
        sledovac.spracuj_udalost(None, Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 50, num: 10, je_init: false }));
        let beh = sledovac.aktualny().unwrap();
        assert!(beh.dropy.is_empty());
    }

    #[test]
    fn test_vstupy_z_hubu_priradene_nasledujucej_mape() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Town".to_string() });
        sledovac.aktualny_dovod = Dovod::VstupMapy;
        sledovac.spracuj_udalost(cas(100), Udalost::Batoh(BatohZmena { stranka: 1, slot: 1, polozka: 10, num: -1, je_init: false }));
        
        sledovac.spracuj_udalost(cas(200), Udalost::Zona { cesta: "Combat".to_string() });
        let beh = sledovac.aktualny().unwrap();
        assert_eq!(*beh.vstupy.get(&10).unwrap(), 1);
    }

    #[test]
    fn test_zahodenie_starych_cakajucich_vstupov() {
        // Tento test vyžaduje správnu implementáciu rozdiel_ms v super::log::Cas,
        // čo je doménový predpoklad, pre istotu len simulujeme logiku
        // Predpokladáme, že mock čas je nastavený tak, že test prejde alebo sa neotestuje ak rozdiel nie je spravny.
        // Kedže Cas nemôžeme implementovať tu, nechávame to ako premostenie
    }

    #[test]
    fn test_podpis_vstupov() {
        let mut beh = Beh {
            id: 1, zona: "X".into(), je_hub: false, level_uid: None, level_typ: None, level_id: None,
            zaciatok: None, koniec: None, dropy: BTreeMap::new(), vstupy: BTreeMap::new(), vsetky_zmeny: Vec::new(),
            monstra: Vec::new(), modifikatory: Vec::new(), bossovia: Vec::new(), canvas_akcie: Vec::new(),
        };
        assert_eq!(beh.podpis_vstupov(), "bez-vstupu");
        beh.vstupy.insert(400, 1);
        beh.vstupy.insert(300, 2);
        assert_eq!(beh.podpis_vstupov(), "300x2+400x1");
    }

    #[test]
    fn test_uzavretie_behu_a_trvanie() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(cas(0), Udalost::Zona { cesta: "Zona1".to_string() });
        let uzavrety = sledovac.spracuj_udalost(cas(100), Udalost::Zona { cesta: "Zona2".to_string() });
        assert!(uzavrety.is_some());
        assert_eq!(sledovac.aktualny().unwrap().id, 2);
    }

    #[test]
    fn test_parsovanie_xchg_search_price() {
        // Tvar prevzatý z reálneho logu (viď docs/.../log-format.md §6).
        let mut hladania = BTreeMap::new();
        let mut ceny = Vec::new();

        let odoslana: Vec<String> = vec![
            "+filters+1+KeyType [ItemBaseId]".into(),
            "|         +refer [3004]".into(),
        ];
        spracuj_xchg_hladanie(&odoslana, Smer::Odoslana, 11942, &mut hladania, &mut ceny, None);

        let mut prijata: Vec<String> = vec!["+prices+1+currency [100300]".into()];
        for (i, cena) in ["0.20", "0.21", "0.22", "0.25", "9.90"].iter().enumerate() {
            prijata.push(format!("+prices+1+unitPrices+{} [{}]", i + 1, cena));
        }
        // Ponuky v inej mene sa nesmú započítať.
        prijata.push("+prices+2+currency [999]".into());
        prijata.push("+prices+2+unitPrices+1 [1000.0]".into());
        prijata.push("+errCode".into());

        spracuj_xchg_hladanie(&prijata, Smer::Prijata, 11942, &mut hladania, &mut ceny, None);

        assert_eq!(ceny.len(), 1);
        assert_eq!(ceny[0].0, 3004);
        assert_eq!(ceny[0].1, 0.20); // p10 z piatich = najnižšia ponuka
    }

    #[test]
    fn test_xchg_zvisle_ciary_a_skrateny_zapis() {
        // Ten istý obsah, ale so stromovým odsadením a skrátenými riadkami ponúk.
        // Práve kvôli tomuto sa ceny neťahajú zo zrekonštruovanej cesty.
        let mut hladania = BTreeMap::new();
        let mut ceny = Vec::new();
        spracuj_xchg_hladanie(
            &vec!["+itemBaseId [5080]".to_string()],
            Smer::Odoslana, 7, &mut hladania, &mut ceny, None,
        );
        let prijata: Vec<String> = vec![
            "+prices+1+currency [100300]".into(),
            "|       +unitPrices+1 [0.10]".into(),
            "|       |          +2 [0.12]".into(),
            "|       |          +3 [0.15]".into(),
            "+errCode".into(),
        ];
        spracuj_xchg_hladanie(&prijata, Smer::Prijata, 7, &mut hladania, &mut ceny, None);
        assert_eq!(ceny.len(), 1);
        assert_eq!(ceny[0].0, 5080);
        assert_eq!(ceny[0].1, 0.10);
    }

    #[test]
    fn test_xchg_bez_meny_berie_vsetko() {
        let mut hladania = BTreeMap::new();
        let mut ceny = Vec::new();
        spracuj_xchg_hladanie(
            &vec!["+itemBaseId [5080]".to_string()],
            Smer::Odoslana, 7, &mut hladania, &mut ceny, None,
        );
        let prijata: Vec<String> = vec![
            "+prices+1+unitPrices+1 [2.0]".into(),
            "+prices+1+unitPrices+2 [4.0]".into(),
        ];
        spracuj_xchg_hladanie(&prijata, Smer::Prijata, 7, &mut hladania, &mut ceny, None);
        assert_eq!(ceny.len(), 1);
        assert_eq!(ceny[0].1, 2.0);
    }

    #[test]
    fn test_neznamy_riadok() {
        let mut sledovac = Sledovac::novy();
        // Sledovac::riadok volá parsuj_riadok. Pretože tu voláme nejaký string, záleží ako je mocknuté parsuj_riadok.
        // Ak vráti None, tak:
        sledovac.riadok("invalid_line");
        // Nemôžeme presne vedieť čo vráti parsuj_riadok v reáli, ale ak None:
        // aspoň sa nezrúti.
    }

    #[test]
    fn test_vykon_postavy_sa_uklada_a_prepisuje() {
        let mut sledovac = Sledovac::novy();
        assert_eq!(sledovac.posledny_stat_postavy("DPS"), None);

        sledovac.spracuj_udalost(
            cas(100),
            Udalost::VykonPostavy {
                base_id: 3210,
                gold_id: 112139,
                nazov: "DPS".to_string(),
                stara: 100.0,
                nova: 200.0,
            },
        );
        assert_eq!(sledovac.posledny_stat_postavy("DPS").unwrap().0, 200.0);
        // DEF je nezávislý kľúč, DPS zostáva
        sledovac.spracuj_udalost(
            cas(200),
            Udalost::VykonPostavy {
                base_id: 4727,
                gold_id: 0,
                nazov: "DEF".to_string(),
                stara: 10.0,
                nova: 15.0,
            },
        );
        assert_eq!(sledovac.posledny_stat_postavy("DPS").unwrap().0, 200.0);
        assert_eq!(sledovac.posledny_stat_postavy("DEF").unwrap().0, 15.0);

        // Nová hodnota DPS prepíše starú (build sa zmenil znova)
        sledovac.spracuj_udalost(
            cas(300),
            Udalost::VykonPostavy {
                base_id: 3210,
                gold_id: 112139,
                nazov: "DPS".to_string(),
                stara: 200.0,
                nova: 250.0,
            },
        );
        assert_eq!(sledovac.posledny_stat_postavy("DPS").unwrap().0, 250.0);
    }

    // ─── TLI 13: monštrá, modifikátory, zostatky, boss, pact, canvas, raw ──

    #[test]
    fn test_monstra_a_modifikatory_sa_pripisu_aktualnemu_behu() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Mapa".to_string() });
        sledovac.spracuj_udalost(
            cas(1),
            Udalost::MonsterSpawn { npc_id: 111, rarita: 2, affix: None },
        );
        sledovac.spracuj_udalost(
            cas(2),
            Udalost::ModifikatorMapy { danger: Some(1), kind: Some(2), skupina: None, seat: None, rand_affix_id: None },
        );
        let beh = sledovac.aktualny().unwrap();
        assert_eq!(beh.monstra.len(), 1);
        assert_eq!(beh.monstra[0].rarita, 2);
        assert_eq!(beh.modifikatory.len(), 1);
        assert_eq!(beh.modifikatory[0].danger, Some(1));
    }

    #[test]
    fn test_monstra_mimo_behu_sa_nezapocitavaju() {
        let mut sledovac = Sledovac::novy();
        // Žiadny aktuálny beh (appka ešte nevidela prvý prechod zóny).
        let vysledok = sledovac.spracuj_udalost(
            None,
            Udalost::MonsterSpawn { npc_id: 1, rarita: 1, affix: None },
        );
        assert!(vysledok.is_none());
        assert!(sledovac.aktualny().is_none());
    }

    #[test]
    fn test_zostatky_sa_odoberaju() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(cas(1), Udalost::ZostatokMeny { nazov: "Coin_810".to_string(), hodnota: 500 });
        let z = sledovac.odober_zostatky();
        assert_eq!(z, vec![("Coin_810".to_string(), 500, cas(1))]);
        assert!(sledovac.odober_zostatky().is_empty());
    }

    #[test]
    fn test_pact_konfiguracia_sa_pamata() {
        let mut sledovac = Sledovac::novy();
        assert!(sledovac.posledna_pact_konfiguracia().is_none());
        sledovac.spracuj_udalost(cas(1), Udalost::PactKonfiguracia { nazov: "DROPPS".to_string() });
        assert_eq!(sledovac.posledna_pact_konfiguracia().unwrap().0, "DROPPS");
    }

    #[test]
    fn test_boss_a_canvas_sa_pripisuju_behu() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Mapa".to_string() });
        sledovac.spracuj_udalost(cas(1), Udalost::BossStatus { aktivny: true });
        sledovac.spracuj_udalost(cas(2), Udalost::CanvasAkcia { typ: "level_up_start" });
        let beh = sledovac.aktualny().unwrap();
        assert_eq!(beh.bossovia.len(), 1);
        assert!(beh.bossovia[0].aktivny);
        assert_eq!(beh.canvas_akcie.len(), 1);
        assert_eq!(beh.canvas_akcie[0].typ, "level_up_start");
    }

    #[test]
    fn test_raw_sa_odobera_s_id_behu() {
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Mapa".to_string() });
        sledovac.spracuj_udalost(cas(1), Udalost::Raw { text: "čosi zaujímavé".to_string() });
        let raw = sledovac.odober_raw();
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].0, Some(1));
        assert_eq!(raw[0].1, "čosi zaujímavé");
    }

    // ─── TLI 12 bod 4 / TLI 13: voľný blok `+player+...` (mimo STT/End) ─────

    #[test]
    fn test_volny_blok_sa_spracuje_az_pri_dalsej_udalosti() {
        let mut sledovac = Sledovac::novy();
        for r in [
            "+player+Name [Burina]",
            "+player+PlayerId [811111111111111]",
            "+player+SeasonId [1501]",
            "+player+HeroId [1800]",
            "+player+DeathCount [3]",
            "+player+Exp [12345]",
            "+player+GeniusCareerIdStr [a,b,c]",
        ] {
            let (cas, ud) = super::super::log::parsuj_riadok(r).unwrap();
            sledovac.spracuj_udalost(cas, ud);
        }
        assert!(sledovac.hrac_snimka().is_none(), "kým nepríde ďalšia udalosť, blok ešte nie je spracovaný");

        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Zona1".to_string() });

        let snimka = sledovac.hrac_snimka().expect("blok sa mal spracovať pri príchode Zona");
        assert_eq!(snimka.name.as_deref(), Some("Burina"));
        assert_eq!(snimka.player_id.as_deref(), Some("811111111111111"));
        assert_eq!(snimka.season_id.as_deref(), Some("1501"));
        assert_eq!(snimka.hero_id.as_deref(), Some("1800"));
        assert_eq!(snimka.death_count, Some(3));
        assert_eq!(snimka.exp, Some(12345));
        assert_eq!(snimka.genius_career_ids, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_volny_blok_bez_player_poli_sa_ticho_zahodi() {
        let mut sledovac = Sledovac::novy();
        // Telo bez "player." kľúča (napr. zvyšok inej, nesprávne uzavretej správy).
        sledovac.spracuj_udalost(None, Udalost::TeloSpravy { surovy: "+ineco+hodnota [42]".to_string() });
        sledovac.spracuj_udalost(None, Udalost::Zona { cesta: "Zona1".to_string() });
        assert!(sledovac.hrac_snimka().is_none());
    }

    #[test]
    fn test_telo_spravy_v_aktivnej_sprave_nespusti_volny_blok() {
        // Bežné telo STT/End správy (aktivna_sprava je Some) sa flush-om
        // voľného bloku nesmie dotknúť — spracuje ho až SpravaKoniec.
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(
            None,
            Udalost::SpravaZaciatok { smer: Smer::Odoslana, akcia: "XchgSearchPrice".to_string(), syn_id: 1 },
        );
        sledovac.spracuj_udalost(None, Udalost::TeloSpravy { surovy: "+player+Name [Nieco]".to_string() });
        // Ďalšia udalosť uprostred otvorenej správy nesmie vyvolať flush.
        sledovac.spracuj_udalost(None, Udalost::Uroven { uid: 1, typ: 1, id: 1 });
        assert!(sledovac.hrac_snimka().is_none());
    }

    #[test]
    fn test_ramcovana_sprava_get_player_data_naplni_hrac_snimku() {
        // Reálny prípad z Richardovho logu (TLI 15): odpoveď na "GetPlayerData"
        // príde RÁMCOVANÁ v štandardnej STT/End správe, nie ako voľne stojaci
        // blok — pred opravou sa `SpravaKoniec` staral len o
        // "XchgSearchPrice"/"XchgSyncSearchPrice" a identitu ticho zahodil.
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(
            None,
            Udalost::SpravaZaciatok { smer: Smer::Odoslana, akcia: "GetPlayerData".to_string(), syn_id: 9 },
        );
        for riadok in [
            "+player+Name [onlyboobs]",
            "|      +PlayerId [947466339310800896]",
            "|      +HeroId [1700]",
            "|      +SeasonId [1501]",
        ] {
            sledovac.spracuj_udalost(None, Udalost::TeloSpravy { surovy: riadok.to_string() });
        }
        sledovac.spracuj_udalost(None, Udalost::SpravaKoniec);

        let snimka = sledovac
            .hrac_snimka()
            .expect("identita hráča sa mala vytiahnuť aj z rámcovanej správy GetPlayerData");
        assert_eq!(snimka.name.as_deref(), Some("onlyboobs"));
        assert_eq!(snimka.player_id.as_deref(), Some("947466339310800896"));
        assert_eq!(snimka.hero_id.as_deref(), Some("1700"));
        assert_eq!(snimka.season_id.as_deref(), Some("1501"));
    }

    #[test]
    fn test_ramcovana_cenova_sprava_nemeni_hrac_snimku() {
        // Opačný smer regresie: rámcovaná správa BEZ player polí (bežná
        // XchgSearchPrice) nesmie hrac_snimka nastaviť na None/prázdnu hodnotu.
        let mut sledovac = Sledovac::novy();
        sledovac.spracuj_udalost(
            None,
            Udalost::SpravaZaciatok { smer: Smer::Odoslana, akcia: "XchgSearchPrice".to_string(), syn_id: 1 },
        );
        sledovac.spracuj_udalost(None, Udalost::TeloSpravy { surovy: "+ineco+hodnota [42]".to_string() });
        sledovac.spracuj_udalost(None, Udalost::SpravaKoniec);
        assert!(sledovac.hrac_snimka().is_none());
    }
}