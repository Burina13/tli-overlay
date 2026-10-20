use serde::{Deserialize, Serialize};
use crate::suradnice::Pole;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Odporucanie {
    pub akcia: String,
    pub nazov: String,
    pub ciel: Option<i64>,
    /// TLI 29: `Option<bool>`, NIE holý `bool` — server túto kartu vôbec
    /// nepošle pre ťahy, ktoré nie sú `devour` (patrol/swap/koniec_dňa), a
    /// predtým sa to prejavilo ako `null` proti neoptionálnemu typu, čo
    /// zhodilo parsovanie CELEJ odpovede („Server odpovedal niečím, čomu
    /// nerozumiem."), hoci server inak odpovedal úplne správne.
    pub cez_scratch: Option<bool>,
    pub nastroj: Option<String>,
    pub dar: Option<String>,
    pub dovod: String,
    pub hodnota: Option<f64>,
    /// TLI 32: index do `Analyza::akcie_dole`, keď server vie toto odporúčanie
    /// priradiť ku konkrétnemu tlačidlu spodnej lišty ("Go to Cell N",
    /// "Consume"/"Replace"/"End"...). `None`, keď sa zhoda nenašla (iný panel,
    /// alebo akcia bez tlačidla v serverovom katalógu) — appka sa vtedy vráti
    /// k pôvodnému zvýrazneniu cely. Rovnaká `Option` opatrnosť ako pri
    /// `cez_scratch` (TLI 29): server toto pole posiela vždy, ale appka proti
    /// staršiemu serveru nesmie padnúť na chýbajúcom poli.
    pub tlacidlo: Option<i64>,
}

impl Default for Odporucanie {
    fn default() -> Self {
        Self {
            akcia: "".to_string(),
            nazov: "".to_string(),
            ciel: None,
            cez_scratch: None,
            nastroj: None,
            dar: None,
            dovod: "".to_string(),
            hodnota: None,
            tlacidlo: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Alternativa {
    pub akcia: String,
    pub nazov: String,
    pub ciel: Option<i64>,
    pub hodnota: Option<f64>,
    pub istota: Option<f64>,
    pub dovod: String,
    /// TLI 32 — rovnaký význam ako `Odporucanie::tlacidlo`.
    pub tlacidlo: Option<i64>,
}

impl Default for Alternativa {
    fn default() -> Self {
        Self {
            akcia: "".to_string(),
            nazov: "".to_string(),
            ciel: None,
            hodnota: None,
            istota: None,
            dovod: "".to_string(),
            tlacidlo: None,
        }
    }
}

/// Jedna kartička z obrazovky „Select a Torture Instrument" (TLI 25), už
/// vyhodnotená serverom: tier, poradie, jedna veta prečo a súradnice, kde
/// kartička na obrazovke leží.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KartickaNastroja {
    pub index: i64,
    /// `None` = server nástroj v katalógu nepozná. Taká kartička sa kreslí
    /// sivo, nikdy sa neodporúča a znižuje celkovú istotu.
    pub id: Option<String>,
    pub nazov: String,
    pub anglicky: String,
    /// "S" | "A" | "B" | "C" | "?" (neznámy).
    pub tier: String,
    pub poradie: i64,
    pub dovod: String,
    pub skore: Option<f64>,
    pub odporucany: bool,
    pub neznamy: bool,
    /// Skóre sa opiera hlavne o simuláciu, reálnych drilov je zatiaľ málo.
    pub malo_dat: bool,
    /// Naša simulácia a komunitné meranie si pri tomto nástroji odporujú.
    pub sporne: bool,
    #[serde(rename = "box")]
    pub box_: Option<Box01>,
}

/// Jedno tlačidlo zo spodnej lišty akcie (TLI 32) — "Select a Destination"
/// ("Go to Cell N" / pomenovaná miestnosť) alebo "Make Your Decision"
/// ("Consume"/"Replace"/"End"). Rovnaká konvencia ako `Pole`: `text` namiesto
/// `index`, lebo poradie v zozname nič neznamená, páruje sa podľa textu
/// (server, `_najdi_tlacidlo`), nie podľa pozície.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AkciaDole {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Analyza {
    pub ok: bool,
    pub odporucanie: Option<Odporucanie>,
    pub alternativy: Vec<Alternativa>,
    pub istota: Option<f64>,
    pub istota_rozpoznania: Option<f64>,
    pub skontroluj: Vec<String>,
    pub polia: Vec<Pole>,
    /// TLI 32: tlačidlá spodnej lišty, keď ich server rozpoznal — prázdne,
    /// keď panel nie je na obrázku viditeľný. Staršie servery toto pole
    /// neposielajú; `#[serde(default)]` dá prázdny zoznam a appka sa správa
    /// presne ako doteraz (zvýrazní celu).
    pub akcie_dole: Vec<AkciaDole>,
    pub trvanie_ms: Option<u64>,
    /// TLI 25: tá istá klávesa (F8) chodí na viac obrazoviek mechaniky a
    /// server povie, ktorú práve prečítal — "doska" (predvolené, pôvodné
    /// správanie), "vyber_nastroja" alebo "ine" (zatiaľ nepodporovaná).
    /// Staršie servery pole neposielajú; `Default` preto vracia "doska",
    /// aby nová appka proti starému serveru fungovala ako doteraz.
    pub obrazovka: String,
    pub nastroje: Vec<KartickaNastroja>,
    /// TLI 35 — "Select a Warden's Treasure" (`obrazovka == "vyber_pokladu"`).
    /// Rovnaký tvar položky ako `nastroje` (server ich preto drží v tej istej
    /// `KartickaNastroja`, len v inom poli), ale iný výpočet: keď je po ruke
    /// posledná doska toho istého hráča, server poradie prepočíta Monte Carlom
    /// priamo nad ňou namiesto katalógového priemeru.
    pub poklady: Vec<KartickaNastroja>,
    /// TLI 36 — univerzálny poradca (`obrazovka == "poradca"`). Možnosti,
    /// ktoré hráč na obrazovke reálne má: ten istý tvar položky ako
    /// `nastroje`/`poklady` (appka ich kreslí rovnako — zadanie: „rovnaké
    /// zobrazenie ako pri známych typoch"), len ich nevyrobil špecializovaný
    /// modul, ale poradca nad pravidlami hry. `tier` tu NIE JE tier karty:
    /// server ho odvodil z poradia (S = ber túto, A = druhá v poradí), aby
    /// appka nepotrebovala druhú farebnú logiku.
    pub moznosti: Vec<KartickaNastroja>,
    /// Strojový slug typu obrazovky (`make_your_decision`, `tartarus_lobby`,
    /// …). Appka ho len loguje — zbiera sa podľa neho na serveri.
    pub typ_obrazovky: String,
    /// Názov obrazovky po slovensky do nadpisu panela.
    pub nazov_obrazovky: String,
    /// Istota poradcu slovom: "nizka" | "stredna" | "vysoka".
    pub istota_slovom: String,
    /// Id riadku učenia na serveri — appka ho nikam neposiela sama, je to
    /// len stopa do logu, aby sa dala snímka spätne nájsť.
    pub vyber_id: Option<i64>,
    /// Krátky popis obrazovky, keď je `obrazovka == "ine"`.
    pub popis: String,
    /// TLI 37 — cieľ (Objective) PRÁVE BEŽIACEHO drilu. Server ho posiela
    /// v KAŽDEJ tartarus odpovedi, nie len na obrazovke, kde sa zadal, aby
    /// ho panel vedel ukazovať stále („Consume 2/4"). `None` znamená „žiadny
    /// bežiaci cieľ" alebo starší server — appka sa vtedy správa ako pred
    /// TLI 37.
    pub objective: Option<Objective>,
    /// Potvrdenie pre hráča na obrazovke Objective („Objective: Požretie
    /// (Consume) 4× — stratégia upravená"). Mimo tej obrazovky prázdne.
    pub potvrdenie: String,
    /// 2–3 vety plánu po slovensky k tomuto cieľu. Mimo obrazovky Objective
    /// prázdne — priebežný stav cieľa nesie `objective`.
    pub plan: Vec<String>,
}

/// Cieľ behu Tartarusu (TLI 37). Všetko je `#[serde(default)]`, takže
/// odpoveď staršieho servera sa parsuje ďalej a chýbajúce pole appku
/// nezhodí — to isté poučenie ako `cez_scratch` v TLI 29.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Objective {
    /// Strojový kód (`survival_of_the_fittest`) — appka ho len loguje.
    pub kod: String,
    /// Anglický názov z hry („Survival of the Fittest").
    pub anglicky: String,
    /// Slovenský názov z katalógu; keď objective v katalógu nie je, je to
    /// ten istý text ako `anglicky`.
    pub nazov: String,
    /// Anglická podmienka z obrazovky („Consume 4 times").
    pub podmienka: String,
    /// Slovenský názov akcie, ktorú treba robiť („Požretie (Consume)").
    pub akcia_sk: String,
    /// Koľkokrát sa to má stať; `None`, keď sa číslo nedalo prečítať.
    pub ciel_pocet: Option<i64>,
    /// Koľkokrát sa to už stalo — ODHAD ZDOLA z F8 snímok dosky (server do
    /// hry nevidí). `None` = merať sa to z dosky nedá.
    pub aktualny: Option<i64>,
    /// Koľko ešte chýba; `None` z tých istých dôvodov ako `aktualny`.
    pub zostava: Option<i64>,
    pub splneny: bool,
    /// Hotový text priebehu („2/4"), prázdny keď sa nedá zmerať.
    pub progres_text: String,
    /// "odhad" | "nemerateľné" — appka to hovorí hráčovi nahlas, aby
    /// nepovažoval číslo za presné.
    pub progres_presnost: String,
    /// `false` = objective, ktorý katalóg na serveri ešte nepozná.
    pub v_katalogu: bool,
}

impl Default for Analyza {
    fn default() -> Self {
        Self {
            ok: false,
            odporucanie: None,
            alternativy: vec![],
            istota: None,
            istota_rozpoznania: None,
            skontroluj: vec![],
            polia: vec![],
            akcie_dole: vec![],
            trvanie_ms: None,
            obrazovka: "doska".to_string(),
            nastroje: vec![],
            poklady: vec![],
            moznosti: vec![],
            typ_obrazovky: String::new(),
            nazov_obrazovky: String::new(),
            istota_slovom: String::new(),
            vyber_id: None,
            popis: "".to_string(),
            objective: None,
            potvrdenie: String::new(),
            plan: vec![],
        }
    }
}

/// Kartičky, ktoré sa dajú nakresliť nad hrou (majú nenulový box), zoradené
/// podľa poradia od najlepšej. Rovnaká konvencia ako `pouzitelne_polia`.
pub fn pouzitelne_karticky(a: &Analyza) -> Vec<&KartickaNastroja> {
    let mut v: Vec<&KartickaNastroja> = a
        .nastroje
        .iter()
        .filter(|k| k.box_.map(|b| b.w > 0.0 && b.h > 0.0).unwrap_or(false))
        .collect();
    v.sort_by_key(|k| k.poradie);
    v
}

/// To isté ako `pouzitelne_karticky`, len nad `poklady` (TLI 35).
pub fn pouzitelne_karticky_pokladu(a: &Analyza) -> Vec<&KartickaNastroja> {
    let mut v: Vec<&KartickaNastroja> = a
        .poklady
        .iter()
        .filter(|k| k.box_.map(|b| b.w > 0.0 && b.h > 0.0).unwrap_or(false))
        .collect();
    v.sort_by_key(|k| k.poradie);
    v
}

/// To isté ako `pouzitelne_karticky`, len nad `moznosti` (TLI 36).
pub fn pouzitelne_moznosti(a: &Analyza) -> Vec<&KartickaNastroja> {
    let mut v: Vec<&KartickaNastroja> = a
        .moznosti
        .iter()
        .filter(|k| k.box_.map(|b| b.w > 0.0 && b.h > 0.0).unwrap_or(false))
        .collect();
    v.sort_by_key(|k| k.poradie);
    v
}

/// Text toastu pre obrazovku, ktorú server zatiaľ nevie čítať. Snímka je
/// uložená na serveri, takže z nej vie agent nový typ obrazovky zaradiť —
/// hráč nemá čo robiť, len to vedieť.
pub fn text_toastu_neznama(a: &Analyza) -> String {
    if a.popis.trim().is_empty() {
        "Túto obrazovku zatiaľ neviem prečítať. Snímku som uložil — pribudne do zoznamu v #torchlight."
            .to_string()
    } else {
        format!(
            "Túto obrazovku zatiaľ neviem prečítať ({}). Snímku som uložil — pribudne do zoznamu v #torchlight.",
            a.popis.trim()
        )
    }
}

// ── Frozen Canvas (TLI 4b) — `?modul=canvas`, kontrakt v
// docs/fakty/hry/torchlight-infinite/canvas-api-kontrakt.md. Odpoveď nesie
// `kroky[]` (sekvenciu ČO→KAM), nie jedno `odporucanie` ako Tartarus, lebo
// cieľ typicky žiada viac umiestnení naraz.

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Box01 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KrokZdroj {
    pub typ_slotu: String,
    pub index: i64,
    #[serde(rename = "box")]
    pub box_: Option<Box01>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KrokCiel {
    pub typ_bunky: String,
    pub riadok: i64,
    pub stlpec: i64,
    #[serde(rename = "box")]
    pub box_: Option<Box01>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Krok {
    pub poradie: i64,
    pub zdroj: KrokZdroj,
    pub ciel: KrokCiel,
    pub dovod: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalyzaCanvas {
    pub ok: bool,
    pub modul: String,
    pub kroky: Vec<Krok>,
    pub istota: Option<f64>,
    pub istota_rozpoznania: Option<f64>,
    pub skontroluj: Vec<String>,
    pub trvanie_ms: Option<u64>,
    /// TLI 4c (`2026-10-15-0028-mcp.md`): `true`/`false` = smer aktívneho
    /// cieľa má/nemá aspoň `canvas.yml:prah_overenia_parov` potvrdených párov
    /// pred/po; `null` keď plátno nemá rozpoznaný cieľ (nedá sa posúdiť).
    /// Kontrakt: `canvas-api-kontrakt.md` § `pravidlo_overene`.
    pub pravidlo_overene: Option<bool>,
    /// TLI 69: odkiaľ server vzal dosah Color Mixing Effectu — `"ram"` (hra
    /// cieľové bunky sama žlto orámovala a appka ich rozpoznala lokálne,
    /// `canvas_ramy.rs`), `"smer"` (rám na snímke nebol, dosah sa odvodil
    /// z názvu smeru) alebo prázdne. Panel podľa toho hráčovi napíše, či sú
    /// cieľové bunky isté, alebo odhadnuté.
    pub zdroj_dosahu: String,
    /// TLI 8: doplnený stav plátna a ruky zo servera, prenesený do UI BEZ
    /// interpretácie (`serde_json::Value`) — panel z neho berie len paint
    /// v zdrojovom slote ruky, aby pri odporúčaní ukázal jeho ikonu. Rust
    /// o tvare plátna nič nevie a vedieť nepotrebuje; keby server pole
    /// nepriložil, ostane `null` a panel ikonu paintu jednoducho nenakreslí.
    pub platno: Option<serde_json::Value>,
}

impl Default for AnalyzaCanvas {
    fn default() -> Self {
        Self {
            ok: false,
            modul: "canvas".to_string(),
            kroky: vec![],
            istota: None,
            istota_rozpoznania: None,
            skontroluj: vec![],
            trvanie_ms: None,
            pravidlo_overene: None,
            zdroj_dosahu: String::new(),
            platno: None,
        }
    }
}

/// Krok, ktorého zdroj aj cieľ majú súradnice (iné odpadli, lebo rozpoznávanie
/// pre ne nevrátilo box — rovnaká konvencia ako Tartarus `pouzitelne_polia`).
pub fn pouzitelne_kroky(a: &AnalyzaCanvas) -> Vec<&Krok> {
    a.kroky
        .iter()
        .filter(|k| {
            k.zdroj.box_.map(|b| b.w > 0.0 && b.h > 0.0).unwrap_or(false)
                && k.ciel.box_.map(|b| b.w > 0.0 && b.h > 0.0).unwrap_or(false)
        })
        .collect()
}

// ── Ceny z obrazovky trhu (TLI 22) — `?modul=ceny`, kontrakt v
// docs/fakty/hry/torchlight-infinite/market-ceny-kontrakt.md. Na rozdiel od
// Tartarus/Canvas sa nekreslí nič nad hrou — len toast a zápis do cenníka —
// takže odpoveď nenesie žiadne súradnice políčok.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalyzaCeny {
    pub ok: bool,
    pub modul: String,
    /// Presný anglický názov položky, ako ho server prečítal z obrázka.
    pub polozka_nazov: String,
    /// `None`, keď sa `polozka_nazov` nenašiel v cenníku položiek — appka
    /// cenu ukáže v Trackeri, ale NEODOŠLE ju do zdieľania s partiou
    /// (`/api/ceny` potrebuje číselné ID).
    pub config_base_id: Option<u32>,
    pub mena: String,
    /// Najnižšie viditeľné ceny (3–5), zoradené VZOSTUPNE.
    pub ceny: Vec<f64>,
    /// Referenčná hodnota na uloženie do cenníka — najnižšia z `ceny`.
    pub p10: Option<f64>,
    pub pocet_ponuk: u32,
    pub istota: Option<f64>,
    pub istota_rozpoznania: Option<f64>,
    pub skontroluj: Vec<String>,
    pub trvanie_ms: Option<u64>,
}

impl Default for AnalyzaCeny {
    fn default() -> Self {
        Self {
            ok: false,
            modul: "ceny".to_string(),
            polozka_nazov: "".to_string(),
            config_base_id: None,
            mena: "FE".to_string(),
            ceny: vec![],
            p10: None,
            pocet_ponuk: 0,
            istota: None,
            istota_rozpoznania: None,
            skontroluj: vec![],
            trvanie_ms: None,
        }
    }
}

/// Text toastu „Cena uložená: <položka> = X FE" (zadanie bod 3). Keď server
/// položku v cenníku nenašiel (`config_base_id` chýba), text to hovorí
/// namiesto sumy, nech hráč vie, že cena sa nezdieľa s partiou.
pub fn text_toastu_ceny(a: &AnalyzaCeny) -> String {
    match a.p10 {
        Some(cena) if a.config_base_id.is_some() => {
            format!("Cena uložená: {} = {:.0} {}", a.polozka_nazov, cena, a.mena)
        }
        Some(cena) => {
            format!(
                "Cena prečítaná: {} = {:.0} {} (nenašla sa v cenníku, nezdieľa sa s partiou)",
                a.polozka_nazov, cena, a.mena
            )
        }
        None => format!("Na trhu sa nenašla žiadna čitateľná cena pre „{}“.", a.polozka_nazov),
    }
}

/// Odpoveď `POST /api/minimapa` (TLI 62/3, `minimapa-api-kontrakt.md`) —
/// **vždy 200**, aj keď sa snímka neprijala (`prijate: false`). Appka z nej
/// potrebuje len `prijate` (vyhodiť súbor z fronty na disku) a `dost_dat`
/// (prestať fotiť TÚTO mapu) — zvyšné polia kontraktu (`behov`, `snimok`…)
/// sú len pre server/Richardovo hlásenie, appka ich nepotrebuje.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MinimapaOdpoved {
    pub ok: bool,
    pub prijate: bool,
    pub dost_dat: bool,
}

/// Jedna mapa v odpovedi `GET /api/minimapa/stav`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MinimapaMapaStav {
    pub dost_dat: bool,
}

/// Odpoveď `GET /api/minimapa/stav` — appka sa pýta raz za čas
/// (`DostDatStav::ma_sa_opytat_servera`), nie pri každej snímke. `zapnute:
/// false` znamená, že appka má zber úplne vypnúť (rovnaké ako `503` pri
/// `POST /api/minimapa`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MinimapaStavOdpoved {
    pub ok: bool,
    pub zapnute: bool,
    pub mapy: std::collections::HashMap<String, MinimapaMapaStav>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChybaServera {
    pub ok: bool,
    pub chyba: String,
    pub kod: Option<u16>,
}

impl Default for ChybaServera {
    fn default() -> Self {
        Self {
            ok: false,
            chyba: "".to_string(),
            kod: None,
        }
    }
}

/// Prekladá technické HTTP kódy na ľudskou rečou podané odpovede,
/// aby hráč pochopil, v čom nastal problém a čo má urobiť ďalej.
pub fn sprava_chyby(stav: u16, telo: &str) -> String {
    if let Ok(chyba_srv) = serde_json::from_str::<ChybaServera>(telo) {
        if !chyba_srv.chyba.trim().is_empty() {
            return chyba_srv.chyba;
        }
    }

    match stav {
        400 => "Server screenshotu nerozumel — posiela sa len PNG alebo JPEG do 4 MB. Skús to znova; ak to trvá, nahlás to v #torchlight."
            .to_string(),
        401 => "Server neprijal tvoj kľúč. Skontroluj si ho v nastaveniach — nový si vydáš na tli.tvojtatko.sk/kluc alebo príkazom /tli kluc v Discorde."
            .to_string(),
        403 => "Tvoj účet nemá rolu Torchlight, takže ťa server nepustí. Napíš o ňu Richardovi v Discorde."
            .to_string(),
        404 => "Adresa servera je zlá. V nastaveniach má byť https://tli.tvojtatko.sk."
            .to_string(),
        413 => "Screenshot je pre server priveľký (limit sú 4 MB). Skús to znova — ak sa to opakuje, nahlás to v #torchlight."
            .to_string(),
        422 => "Na obrázku som nenašiel dosku Tartarus. Maj ju celú na obrazovke a skús to znova."
            .to_string(),
        429 => "Priveľa požiadaviek za sebou — server púšťa 30 za minútu. Počkaj chvíľu a stlač hotkey znova."
            .to_string(),
        500..=599 => "Server má poruchu a analýzu teraz nespraví. Skús to o chvíľu znova; ak to trvá, nahlás to v #torchlight."
            .to_string(),
        _ => format!("Neznáma chyba servera (HTTP {stav})."),
    }
}

/// TLI 67b (`2026-10-20-1348-mcp.md`, bod 1) — koľko sekúnd appka čaká na
/// odpoveď na RUČNÚ analýzu (F8 Tartarus aj F9/Ctrl+W Frozen Canvas), kým to
/// vzdá a napíše hráčovi, že server neodpovedá.
///
/// Do 20.10.2026 to bolo **60 s** a prejavilo sa to presne tak, ako to Richard
/// o 13:48 opísal („a teraz sa mi to celé seklo"): Canvas požiadavka na
/// zaneprázdnenom serveri trvala **34 267 ms** (ráno 2,6–3,2 s) a appka na ňu
/// celý ten čas mlčky čakala.
///
/// **Hodnotu 10 s zadal Richard a je to jeho rozhodnutie — ale podľa
/// nameraných dát je tesná.** Z posledných 471 analýz na serveri (merané
/// 20.10.2026 z `analyzy.trvanie_ms` na CT 114):
///
/// | obrazovka | medián | p95 | nad 10 s |
/// |---|---|---|---|
/// | doska (Tartarus) | 6 639 ms | 12 175 ms | 10 z 92 |
/// | Frozen Canvas | 2 896 ms | 14 601 ms | 23 z 241 |
/// | výber nástroja | 4 780 ms | 6 638 ms | 0 z 63 |
/// | poradca (neznáma obrazovka) | 5 596 ms | 12 282 ms | 3 z 22 |
///
/// Teda **asi každá desiata analýza, ktorá dnes dobehne, po tejto zmene
/// skončí hláškou „server neodpovedá"**. Nie je to prehliadnutie: hráč stlačí
/// znova (nové stlačenie starú požiadavku zruší) a počítadlo sekúnd mu ukáže,
/// že appka žije — „zaseknuté" to teda nie je. Keby to prekážalo, hodnota nad
/// p95 je **20 s** a je to jedno číslo na jednom mieste.
///
/// Zber na pozadí má vlastný, dlhší timeout (`klient.rs`) — tam na nikoho
/// nečaká hráč a znovuodoslanie stojí ďalšiu snímku.
pub const TIMEOUT_ANALYZY_S: u64 = 10;

/// Text, ktorý hráč uvidí, keď server do `TIMEOUT_ANALYZY_S` neodpovie
/// (TLI 67b, bod 1 — znenie je doslova zo zadania). Je to `const`, a nie
/// `format!`, aby sa dal POROVNAŤ: `main.rs` podľa neho pozná, že panel má
/// zhasnúť na prvý klik bez ohľadu na nastavenie autoskrytia.
pub const SPRAVA_TIMEOUTU: &str = "Server teraz neodpovedá – skús o chvíľu";

/// Formátuje chybovú hlášku priamo na vrstve sieťového prenosu (napr. timeouty),
/// aby sme oddelili aplikačné chyby servera od zlyhaní pripojenia.
pub fn sprava_siete(detail: &str) -> String {
    format!(
        "Nepodarilo sa spojiť so serverom. Skontroluj internet a adresu v nastaveniach \
(tlačidlo „Test pripojenia“). Detail: {detail}"
    )
}

/// TLI 29: odpoveď s HTTP 2xx, ktorej telo sa nedalo rozobrať na očakávaný
/// tvar (napr. server pridá pole nekompatibilného typu, alebo medzi appkou
/// a serverom je proxy, čo vracia HTML). Predtým appka ukázala len generické
/// „Server odpovedal niečím, čomu nerozumiem." bez HTTP kódu — hráč aj
/// Richard tak nevedeli rozlíšiť, či ide o server, proxy alebo appku samu.
/// Celé telo (do 200 znakov) ide len do denníka (`klient::posli_analyzu*`),
/// toto je krátky text pre toast.
pub fn sprava_neparsovatelnej_odpovede(stav: u16) -> String {
    format!("Server {stav} poslal odpoveď, ktorej appka nerozumie — skús znova. \
Detail je v logu (Nastavenia → Otvoriť log).")
}

/// Filtruje poškodené či zámerne skryté oblasti (vrátené s nulovou veľkosťou),
/// aby sme sa nepokúšali vykresliť v overlayi neviditeľné alebo poškodené štvorce.
pub fn pouzitelne_polia(a: &Analyza) -> Vec<&Pole> {
    a.polia.iter().filter(|p| p.w > 0.0 && p.h > 0.0).collect()
}

/// Identifikuje políčka, ktorými si server nie je celkom istý,
/// aby sme ich vizuálne odlíšili a dali hráčovi možnosť manuálne si overiť ťah.
pub fn je_neiste(a: &Analyza, index: u32) -> bool {
    let hladane = format!("[{}]", index);
    a.skontroluj.iter().any(|s| s.contains(&hladane))
}

/// Tlačidlo zo spodnej lišty (TLI 32) zodpovedajúce indexu z `tlacidlo`
/// (`Odporucanie`/`Alternativa`) — `None`, keď index chýba, mimo rozsah, alebo
/// má nulovú veľkosť (rovnaká konvencia ako `pouzitelne_polia`/
/// `pouzitelne_karticky`: appka nekreslí neviditeľné štvorce).
pub fn tlacidlo_podla_indexu(a: &Analyza, index: Option<i64>) -> Option<&AkciaDole> {
    let i = usize::try_from(index?).ok()?;
    let t = a.akcie_dole.get(i)?;
    if t.w > 0.0 && t.h > 0.0 { Some(t) } else { None }
}

#[cfg(test)]
mod testy {
    use super::*;

    /// Staršia odpoveď servera (pred TLI 25) pole `obrazovka` neposiela —
    /// nová appka ju musí brať ako dosku, inak by po nasadení klienta pred
    /// serverom prestalo F8 kresliť čokoľvek.
    #[test]
    fn odpoved_bez_pola_obrazovka_je_doska() {
        let a: Analyza = serde_json::from_str(r#"{"ok": true, "polia": []}"#).unwrap();
        assert_eq!(a.obrazovka, "doska");
        assert!(a.nastroje.is_empty());
    }

    /// TLI 29 regresia — reálny incident 18.10.2026 01:34:21: server odpovedal
    /// 200 OK a správne rozpoznal dosku (deň 19, `destinations` otvorené), ale
    /// top odporúčanie bolo `patrol` (nie `devour`), ktorý `cez_scratch` vôbec
    /// nemá — `naj.get("cez_scratch")` na serveri vtedy vrátilo `None` →
    /// `null` v JSON-e. Keď bol `cez_scratch` holý `bool`, celé parsovanie
    /// `Analyza` zlyhalo a appka ukázala „Server odpovedal niečím, čomu
    /// nerozumiem." namiesto odporúčania — presne telo z uloženej analýzy
    /// `2026-10-18-013422-16348a9c.json` na CT 114 (skrátené na podstatné polia).
    #[test]
    fn odporucanie_s_cez_scratch_null_sa_parsuje() {
        let json = r#"{
            "ok": true, "modul": "tartarus", "obrazovka": "doska",
            "odporucanie": {
                "akcia": "patrol", "nazov": "Patrola", "ciel": 4,
                "cez_scratch": null, "nastroj": null, "dar": null,
                "dovod": "Zamknuté dvere prinášajú nový materiál pre reťaz.",
                "hodnota": 13.9417
            },
            "istota": 0.057
        }"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        let odporucanie = a.odporucanie.unwrap();
        assert_eq!(odporucanie.akcia, "patrol");
        assert_eq!(odporucanie.cez_scratch, None);
    }

    #[test]
    fn parsovanie_vyberu_nastroja() {
        let json = r#"{
            "ok": true, "modul": "tartarus-nastroj", "obrazovka": "vyber_nastroja",
            "vyber_id": 7, "istota": 0.82,
            "odporucanie": {"nazov": "Rozseknutá železná panna", "dovod": "Lebo."},
            "nastroje": [
              {"index": 0, "id": "maiden", "nazov": "Rozseknutá železná panna",
               "anglicky": "Severed Iron Maiden", "tier": "S", "poradie": 1,
               "dovod": "Najlepšia z troch.", "skore": 2.08, "odporucany": true,
               "neznamy": false, "malo_dat": true, "sporne": false,
               "box": {"x": 0.18, "y": 0.28, "w": 0.19, "h": 0.42}},
              {"index": 1, "id": null, "nazov": "Čosi", "anglicky": "Čosi",
               "tier": "?", "poradie": 2, "dovod": "Nepoznám.", "skore": null,
               "odporucany": false, "neznamy": true, "malo_dat": true,
               "sporne": false, "box": null}
            ]
        }"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        assert_eq!(a.obrazovka, "vyber_nastroja");
        assert_eq!(a.vyber_id, Some(7));
        assert_eq!(a.nastroje.len(), 2);
        assert!(a.nastroje[0].odporucany && a.nastroje[0].tier == "S");
        assert!(a.nastroje[1].neznamy && a.nastroje[1].id.is_none());
        // Kartička bez boxu sa nad hrou nekreslí — ostáva len v paneli.
        let kreslitelne = pouzitelne_karticky(&a);
        assert_eq!(kreslitelne.len(), 1);
        assert_eq!(kreslitelne[0].id.as_deref(), Some("maiden"));
    }

    #[test]
    fn pouzitelne_karticky_su_zoradene_podla_poradia_nie_podla_indexu() {
        let mut a = Analyza::default();
        for (index, poradie) in [(0, 3), (1, 1), (2, 2)] {
            a.nastroje.push(KartickaNastroja {
                index,
                poradie,
                box_: Some(Box01 { x: 0.1, y: 0.1, w: 0.2, h: 0.2 }),
                ..Default::default()
            });
        }
        let v = pouzitelne_karticky(&a);
        assert_eq!(v.iter().map(|k| k.index).collect::<Vec<_>>(), vec![1, 2, 0]);
    }

    #[test]
    fn parsovanie_vyberu_pokladu() {
        // TLI 35 — presne 4 kartičky z Richardovho F8 screenshotu 18.10.2026
        // 13:21, kontext_dosky znamená, že server odporúčanie prepočítal
        // Monte Carlom nad jeho poslednou doskou, nie len katalógovým priemerom.
        let json = r#"{
            "ok": true, "modul": "tartarus-poklad", "obrazovka": "vyber_pokladu",
            "kontext_dosky": true, "istota": 0.77,
            "odporucanie": {"nazov": "Mosadzný roh sváru", "dovod": "Lebo."},
            "poklady": [
              {"index": 0, "id": "horn", "nazov": "Mosadzný roh sváru",
               "anglicky": "Brass Horn of Strife", "tier": "C", "poradie": 1,
               "dovod": "Hneď spustí požretie u 7 väzňov.", "skore": 0.895,
               "odporucany": true, "neznamy": false, "malo_dat": false,
               "sporne": false, "box": {"x": 0.35, "y": 0.30, "w": 0.17, "h": 0.40}},
              {"index": 1, "id": "wine", "nazov": "Karmínový ročník",
               "anglicky": "Crimson Vintage", "tier": "S", "poradie": 2,
               "dovod": "Zdvihne raritu 0 väzňom.", "skore": 1.261,
               "odporucany": false, "neznamy": false, "malo_dat": false,
               "sporne": false, "box": {"x": 0.55, "y": 0.30, "w": 0.17, "h": 0.40}}
            ]
        }"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        assert_eq!(a.obrazovka, "vyber_pokladu");
        assert_eq!(a.poklady.len(), 2);
        // Výber pokladu nemá `vyber_id` (TLI 35 nemá vlastnú tabuľku učenia
        // ako nástroje — poslednú dosku si pamätá `analyzy`, nie nový riadok).
        assert_eq!(a.vyber_id, None);
        assert!(a.poklady[0].odporucany && a.poklady[0].id.as_deref() == Some("horn"));
        // Katalógovo je Víno najlepšie (tier S), ale kontext dosky (TLI 35)
        // odporučí Roh — presne to appka má ukázať, nie opraviť na „logickejšie".
        assert_eq!(a.poklady[1].tier, "S");
        assert!(!a.poklady[1].odporucany);
        let kreslitelne = pouzitelne_karticky_pokladu(&a);
        assert_eq!(kreslitelne.len(), 2);
        assert_eq!(kreslitelne[0].id.as_deref(), Some("horn"));
    }

    #[test]
    fn parsovanie_obrazovky_objective() {
        // TLI 37 — odpoveď servera pre obrazovku „Objective" z Richardovej
        // snímky 18.10.2026 13:23:25 (Stage Level 77).
        let json = r#"{
            "ok": true, "modul": "tartarus-objective", "obrazovka": "objective",
            "istota": 0.75,
            "potvrdenie": "Objective: Požretie (Consume) 4× — stratégia upravená",
            "plan": ["Ber požretie.", "Nenechaj deň prejsť naprázdno."],
            "objective": {
              "kod": "survival_of_the_fittest",
              "anglicky": "Survival of the Fittest",
              "nazov": "Prežije najsilnejší",
              "podmienka": "Consume 4 times",
              "akcia_sk": "Požretie (Consume)",
              "ciel_pocet": 4, "aktualny": 2, "zostava": 2, "splneny": false,
              "progres_text": "2/4", "progres_presnost": "odhad", "v_katalogu": true
            },
            "moznosti": [
              {"index": 0, "nazov": "Confirm", "anglicky": "Confirm", "tier": "S",
               "poradie": 1, "dovod": "Potvrdíš cieľ.", "odporucany": true,
               "neznamy": false, "malo_dat": false, "sporne": false,
               "box": {"x": 0.58, "y": 0.68, "w": 0.16, "h": 0.06}}
            ]
        }"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        assert_eq!(a.obrazovka, "objective");
        assert!(a.potvrdenie.contains("stratégia upravená"));
        assert_eq!(a.plan.len(), 2);
        let o = a.objective.as_ref().expect("objective má prísť");
        assert_eq!(o.anglicky, "Survival of the Fittest");
        assert_eq!(o.ciel_pocet, Some(4));
        assert_eq!(o.zostava, Some(2));
        assert_eq!(o.progres_text, "2/4");
        assert!(!o.splneny && o.v_katalogu);
        assert_eq!(pouzitelne_moznosti(&a).len(), 1);
    }

    #[test]
    fn starsi_server_bez_objective_appku_nerozbije() {
        // Rovnaké poučenie ako `cez_scratch` v TLI 29: chýbajúce pole nesmie
        // zhodiť parsovanie CELEJ odpovede — appka sa má správať ako pred
        // TLI 37, nie napísať „server odpovedal niečím, čomu nerozumiem".
        let a: Analyza =
            serde_json::from_str(r#"{"ok": true, "obrazovka": "doska"}"#).unwrap();
        assert!(a.objective.is_none());
        assert_eq!(a.potvrdenie, "");
        assert!(a.plan.is_empty());
    }

    #[test]
    fn objective_bez_merateľneho_priebehu_sa_parsuje_s_null_hodnotami() {
        // Server priebeh niektorých cieľov zmerať nedokáže (do hry nevidí) a
        // posiela `null` — appka to musí uniesť a NEPREVIESŤ na nulu, inak by
        // hráčovi ukázala „0/4" tam, kde pravdivá odpoveď je „neviem".
        let json = r#"{"ok": true, "obrazovka": "doska", "objective": {
            "anglicky": "Riddle", "podmienka": "Do something odd",
            "ciel_pocet": null, "aktualny": null, "zostava": null,
            "progres_text": "", "progres_presnost": "nemerateľné"}}"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        let o = a.objective.as_ref().unwrap();
        assert_eq!(o.aktualny, None);
        assert_eq!(o.ciel_pocet, None);
        assert_eq!(o.progres_presnost, "nemerateľné");
    }

    #[test]
    fn poradca_sa_parsuje_a_kreslitelne_su_zoradene_podla_poradia() {
        // TLI 36 — odpoveď servera pre obrazovku bez špecializovaného modulu.
        // Tretia možnosť nemá box (server ju označil ako neumiestnenú), takže
        // sa nad hru nekreslí — ale z odpovede nezmizne, panel ju vypíše.
        let json = r#"{
            "ok": true,
            "obrazovka": "poradca",
            "typ_obrazovky": "make_your_decision",
            "nazov_obrazovky": "Rozhodnutie o cieli väzňa",
            "istota": 0.8,
            "istota_slovom": "vysoka",
            "odporucanie": { "akcia": "", "nazov": "Consume", "dovod": "Zdvihne raritu." },
            "moznosti": [
                { "index": 1, "poradie": 2, "nazov": "Replace", "tier": "A",
                  "box": { "x": 0.41, "y": 0.69, "w": 0.17, "h": 0.16 } },
                { "index": 0, "poradie": 1, "nazov": "Consume", "tier": "S",
                  "odporucany": true,
                  "box": { "x": 0.23, "y": 0.69, "w": 0.17, "h": 0.16 } },
                { "index": 2, "poradie": 3, "nazov": "End", "tier": "C" }
            ]
        }"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        assert_eq!(a.obrazovka, "poradca");
        assert_eq!(a.typ_obrazovky, "make_your_decision");
        assert_eq!(a.nazov_obrazovky, "Rozhodnutie o cieli väzňa");
        assert_eq!(a.istota_slovom, "vysoka");
        assert_eq!(a.moznosti.len(), 3);

        let kreslitelne = pouzitelne_moznosti(&a);
        assert_eq!(kreslitelne.len(), 2, "možnosť bez boxu sa nad hru nekreslí");
        assert_eq!(kreslitelne[0].nazov, "Consume");
        assert!(kreslitelne[0].odporucany);
        assert_eq!(kreslitelne[1].nazov, "Replace");
    }

    #[test]
    fn stara_odpoved_bez_poli_poradcu_je_dalej_citatelna() {
        // Appka musí fungovať aj proti serveru pred TLI 36 — chýbajúce polia
        // dá `#[serde(default)]` ako prázdne, nie ako chybu parsovania.
        let a: Analyza = serde_json::from_str(r#"{"ok": true, "obrazovka": "doska"}"#).unwrap();
        assert!(a.moznosti.is_empty());
        assert_eq!(a.typ_obrazovky, "");
        assert_eq!(a.istota_slovom, "");
        assert!(pouzitelne_moznosti(&a).is_empty());
    }

    #[test]
    fn toast_neznamej_obrazovky_znesie_aj_prazdny_popis() {
        let mut a = Analyza::default();
        a.obrazovka = "ine".to_string();
        assert!(text_toastu_neznama(&a).contains("#torchlight"));
        a.popis = "Warden's Cache gift selection".to_string();
        assert!(text_toastu_neznama(&a).contains("Warden's Cache"));
    }

    #[test]
    fn parsovanie_analyzy() {
        let json = r#"{
            "ok": true,
            "odporucanie": {
                "akcia": "Click",
                "nazov": "Zlato",
                "dovod": "Najlepsi tah"
            },
            "polia": [
                { "index": 0, "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2 }
            ]
        }"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        assert!(a.ok);
        assert_eq!(a.odporucanie.unwrap().akcia, "Click");
        assert_eq!(a.polia.len(), 1);
        assert!(a.istota.is_none());
    }

    #[test]
    fn sprava_chyby_berie_text_zo_servera_ak_je() {
        let json = r#"{"ok": false, "chyba": "Tvoj špeciálny problém."}"#;
        assert_eq!(sprava_chyby(401, json), "Tvoj špeciálny problém.");
    }

    #[test]
    fn sprava_chyby_fallback_podla_kodu() {
        let json = "{}";
        assert!(sprava_chyby(401, json).contains("Server neprijal tvoj kľúč"));
        assert!(sprava_chyby(403, json).contains("rolu Torchlight"));
        assert!(sprava_chyby(422, json).contains("dosku Tartarus"));
        assert!(sprava_chyby(429, json).contains("30 za minútu"));
        assert!(sprava_chyby(503, json).contains("poruch"));
        assert_eq!(sprava_chyby(999, json), "Neznáma chyba servera (HTTP 999).");
    }

    #[test]
    fn je_neiste_hlada_index() {
        let a = Analyza {
            skontroluj: vec!["cells[7].inmate.tier".to_string()],
            ..Default::default()
        };
        assert!(je_neiste(&a, 7));
        assert!(!je_neiste(&a, 1));
    }

    #[test]
    fn parsovanie_analyzy_canvas() {
        // Tvar presne podľa docs/fakty/hry/torchlight-infinite/canvas-api-kontrakt.md.
        let json = r#"{
            "ok": true,
            "modul": "canvas",
            "kroky": [
                {
                    "poradie": 1,
                    "zdroj": {"typ_slotu": "ruka", "index": 0,
                              "box": {"x": 0.1, "y": 0.9, "w": 0.08, "h": 0.08}},
                    "ciel": {"typ_bunky": "platno", "riadok": 3, "stlpec": 3,
                             "box": {"x": 0.3, "y": 0.3, "w": 0.08, "h": 0.08}},
                    "dovod": "Vezmi dwarf Lv1 zo slotu 0..."
                },
                {
                    "poradie": 2,
                    "zdroj": {"typ_slotu": "ruka", "index": 1, "box": null},
                    "ciel": {"typ_bunky": "platno", "riadok": 4, "stlpec": 2, "box": null},
                    "dovod": "Chýba súradnica"
                }
            ],
            "istota": 0.9,
            "skontroluj": []
        }"#;
        let a: AnalyzaCanvas = serde_json::from_str(json).unwrap();
        assert!(a.ok);
        assert_eq!(a.modul, "canvas");
        assert_eq!(a.kroky.len(), 2);
        assert_eq!(a.kroky[0].poradie, 1);
        assert_eq!(a.kroky[0].zdroj.index, 0);
        assert_eq!(a.kroky[0].ciel.riadok, 3);
        assert!(a.kroky[0].zdroj.box_.is_some());
        assert!(a.kroky[1].zdroj.box_.is_none());
        assert_eq!(a.istota, Some(0.9));
    }

    #[test]
    fn parsovanie_analyzy_canvas_s_pravidlo_overene() {
        let json = r#"{"ok": true, "modul": "canvas", "kroky": [],
                       "istota": 0.9, "skontroluj": [], "pravidlo_overene": false}"#;
        let a: AnalyzaCanvas = serde_json::from_str(json).unwrap();
        assert_eq!(a.pravidlo_overene, Some(false));
    }

    /// TLI 69: starší server pole `zdroj_dosahu` neposiela. `#[serde(default)]`
    /// nad `AnalyzaCanvas` ho musí dosadiť ako prázdne — keby parsovanie
    /// spadlo, klient 0.27.0 by nad starším serverom nevykreslil NIČ.
    #[test]
    fn chybajuci_zdroj_dosahu_je_prazdny_nie_chyba() {
        let json = r#"{"ok": true, "modul": "canvas", "kroky": [], "istota": 0.9,
                       "skontroluj": [], "pravidlo_overene": true}"#;
        let a: AnalyzaCanvas = serde_json::from_str(json).expect("má sa dať parsovať");
        assert_eq!(a.zdroj_dosahu, "");
        assert_eq!(a.pravidlo_overene, Some(true));
    }

    #[test]
    fn chybajuce_pravidlo_overene_je_none_nie_chyba() {
        // Starší server (pred TLI 4c) toto pole nevracia vôbec — parsovanie
        // nesmie spadnúť, len sa to má brať ako "neznáme".
        let json = r#"{"ok": true, "modul": "canvas", "kroky": [], "istota": 0.9, "skontroluj": []}"#;
        let a: AnalyzaCanvas = serde_json::from_str(json).unwrap();
        assert_eq!(a.pravidlo_overene, None);
    }

    #[test]
    fn parsovanie_analyzy_ceny() {
        let json = r#"{
            "ok": true,
            "modul": "ceny",
            "polozka_nazov": "Sparks of Moth Fire",
            "config_base_id": 1001,
            "mena": "FE",
            "ceny": [12.0, 13.5, 14.0],
            "p10": 12.0,
            "pocet_ponuk": 3,
            "istota": 0.9,
            "skontroluj": []
        }"#;
        let a: AnalyzaCeny = serde_json::from_str(json).unwrap();
        assert!(a.ok);
        assert_eq!(a.modul, "ceny");
        assert_eq!(a.config_base_id, Some(1001));
        assert_eq!(a.ceny, vec![12.0, 13.5, 14.0]);
        assert_eq!(a.p10, Some(12.0));
    }

    #[test]
    fn chybajuce_pole_v_starsej_odpovedi_je_none_nie_chyba() {
        // Starší server (pred TLI 22) toto nikdy nevracia — no `AnalyzaCeny` sa
        // nikdy neparsuje zo staršieho servera v praxi; test len istí, že
        // `#[serde(default)]` znesie čiastočný JSON.
        let json = r#"{"ok": true, "modul": "ceny", "polozka_nazov": "X", "ceny": []}"#;
        let a: AnalyzaCeny = serde_json::from_str(json).unwrap();
        assert_eq!(a.config_base_id, None);
        assert_eq!(a.p10, None);
        assert_eq!(a.mena, "FE");
    }

    #[test]
    fn text_toastu_ceny_so_znamou_polozkou() {
        let a = AnalyzaCeny {
            polozka_nazov: "Sparks of Moth Fire".to_string(),
            config_base_id: Some(1001),
            p10: Some(12.0),
            mena: "FE".to_string(),
            ..Default::default()
        };
        assert_eq!(text_toastu_ceny(&a), "Cena uložená: Sparks of Moth Fire = 12 FE");
    }

    #[test]
    fn text_toastu_ceny_s_nenajdenou_polozkou() {
        let a = AnalyzaCeny {
            polozka_nazov: "Neznáma vec".to_string(),
            config_base_id: None,
            p10: Some(5.0),
            mena: "FE".to_string(),
            ..Default::default()
        };
        let text = text_toastu_ceny(&a);
        assert!(text.contains("nenašla v cenníku") || text.contains("nezdieľa"));
    }

    #[test]
    fn text_toastu_ceny_bez_ceny() {
        let a = AnalyzaCeny {
            polozka_nazov: "Prázdna ponuka".to_string(),
            p10: None,
            ..Default::default()
        };
        assert!(text_toastu_ceny(&a).contains("nenašla žiadna čitateľná cena"));
    }

    #[test]
    fn pouzitelne_kroky_vynecha_kroky_bez_boxu() {
        let mut a = AnalyzaCanvas { ok: true, ..Default::default() };
        a.kroky.push(Krok {
            poradie: 1,
            zdroj: KrokZdroj { typ_slotu: "ruka".to_string(), index: 0,
                              box_: Some(Box01 { x: 0.1, y: 0.1, w: 0.05, h: 0.05 }) },
            ciel: KrokCiel { typ_bunky: "platno".to_string(), riadok: 1, stlpec: 1,
                             box_: Some(Box01 { x: 0.2, y: 0.2, w: 0.05, h: 0.05 }) },
            dovod: "ok".to_string(),
        });
        a.kroky.push(Krok {
            poradie: 2,
            zdroj: KrokZdroj { typ_slotu: "ruka".to_string(), index: 1, box_: None },
            ciel: KrokCiel { typ_bunky: "platno".to_string(), riadok: 2, stlpec: 1, box_: None },
            dovod: "chyba box".to_string(),
        });
        let pouzitelne = pouzitelne_kroky(&a);
        assert_eq!(pouzitelne.len(), 1);
        assert_eq!(pouzitelne[0].poradie, 1);
    }

    // ── TLI 32 — tlačidlo spodnej lišty ──────────────────────────────────────

    /// Starší server toto pole vôbec neposiela — appka sa musí správať presne
    /// ako predtým TLI 32 (prázdny zoznam, `tlacidlo` v odporúčaní `None`).
    #[test]
    fn odpoved_bez_akcie_dole_je_prazdny_zoznam() {
        let json = r#"{"ok": true, "odporucanie": {"akcia": "patrol", "ciel": 4}}"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        assert!(a.akcie_dole.is_empty());
        assert_eq!(a.odporucanie.unwrap().tlacidlo, None);
    }

    /// Reálny tvar `akcie_dole` z `2026-10-18-105603-67f8c864.jpg` ("Select a
    /// Destination", 3 tlačidlá) — server priradil `tlacidlo: 1` k odporúčaniu.
    #[test]
    fn parsovanie_akcie_dole_a_najdenie_tlacidla() {
        let json = r#"{
            "ok": true, "modul": "tartarus", "obrazovka": "doska",
            "odporucanie": {"akcia": "patrol", "nazov": "Patrola", "ciel": 6,
                            "dovod": "...", "tlacidlo": 1},
            "alternativy": [
                {"akcia": "patrol", "nazov": "Patrola", "ciel": 7, "tlacidlo": 2}
            ],
            "akcie_dole": [
                {"text": "Go to Cell 6", "x": 0.233, "y": 0.691, "w": 0.174, "h": 0.177},
                {"text": "Go to Cell 7", "x": 0.413, "y": 0.691, "w": 0.174, "h": 0.177},
                {"text": "Go to Cell 8", "x": 0.592, "y": 0.691, "w": 0.174, "h": 0.177}
            ],
            "istota": 0.332
        }"#;
        let a: Analyza = serde_json::from_str(json).unwrap();
        assert_eq!(a.akcie_dole.len(), 3);
        let best = tlacidlo_podla_indexu(&a, a.odporucanie.as_ref().unwrap().tlacidlo).unwrap();
        assert_eq!(best.text, "Go to Cell 7");
        let alt = tlacidlo_podla_indexu(&a, a.alternativy[0].tlacidlo).unwrap();
        assert_eq!(alt.text, "Go to Cell 8");
    }

    #[test]
    fn tlacidlo_podla_indexu_vracia_none_mimo_rozsahu_a_bez_indexu() {
        let a = Analyza {
            akcie_dole: vec![AkciaDole { text: "End".to_string(), x: 0.5, y: 0.7, w: 0.17, h: 0.16 }],
            ..Default::default()
        };
        assert!(tlacidlo_podla_indexu(&a, None).is_none());
        assert!(tlacidlo_podla_indexu(&a, Some(5)).is_none());
        assert!(tlacidlo_podla_indexu(&a, Some(-1)).is_none());
        assert!(tlacidlo_podla_indexu(&a, Some(0)).is_some());
    }

    /// Tlačidlo s nulovou veľkosťou (server ho vrátil, ale appka ho nemá kresliť)
    /// sa správa rovnako ako `pouzitelne_polia`/`pouzitelne_karticky` — zahodí sa.
    #[test]
    fn tlacidlo_podla_indexu_zahodi_nulovu_velkost() {
        let a = Analyza {
            akcie_dole: vec![AkciaDole { text: "End".to_string(), x: 0.5, y: 0.7, w: 0.0, h: 0.0 }],
            ..Default::default()
        };
        assert!(tlacidlo_podla_indexu(&a, Some(0)).is_none());
    }

    /// TLI 62/3 — presný tvar odpovede z kontraktu (`minimapa-api-kontrakt.md`),
    /// aj s poliami, ktoré appka vôbec nečíta (`behov`, `snimok`…) — tie
    /// nesmú parsovanie zhodiť, len sa zahodia.
    #[test]
    fn minimapa_odpoved_sa_parsuje_s_neznamymi_poliami() {
        let json = r#"{"ok": true, "prijate": true, "mapa": "XZ_Foo", "behov": 3,
            "behov_pouzitelnych": 2, "snimok": 118, "dost_dat": false,
            "potrebujem_behov": 6, "subor": "x.jpg"}"#;
        let o: MinimapaOdpoved = serde_json::from_str(json).unwrap();
        assert!(o.ok);
        assert!(o.prijate);
        assert!(!o.dost_dat);
    }

    #[test]
    fn minimapa_odpoved_bez_poli_padne_na_predvolene() {
        let o: MinimapaOdpoved = serde_json::from_str("{}").unwrap();
        assert!(!o.ok);
        assert!(!o.prijate);
        assert!(!o.dost_dat);
    }

    #[test]
    fn minimapa_stav_sa_parsuje_a_vracia_dost_dat_na_mapu() {
        let json = r#"{"ok": true, "zapnute": true,
            "mapy": {"XZ_Foo": {"behov": 8, "behov_pouzitelnych": 8, "snimok": 2400, "dost_dat": true}},
            "hotove": ["XZ_Foo"], "dost_behov": 8}"#;
        let s: MinimapaStavOdpoved = serde_json::from_str(json).unwrap();
        assert!(s.zapnute);
        assert!(s.mapy.get("XZ_Foo").unwrap().dost_dat);
        assert!(s.mapy.get("XZ_Ina").is_none());
    }
}
