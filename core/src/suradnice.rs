use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OknoFyz {
    pub x: i32,
    pub y: i32,
    pub sirka: u32,
    pub vyska: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub sirka: u32,
    pub vyska: u32,
    pub mierka: f64,
    /// Meno monitora od OS (na Windowse napr. `\\.\DISPLAY1`), `None` keď ho
    /// Tauri/OS nevie zistiť (napr. v CI bez skutočnej obrazovky). TLI 26,
    /// bod 2: dva fyzické monitory s ROVNAKÝM rozlíšením a mierkou (Richardov
    /// prípad — dva 2560×1440 monitory) musia dostať RÔZNY kľúč uloženého
    /// rozloženia, inak si navzájom prepisujú polohu panelov — pozri
    /// `monitor_signatura`.
    pub nazov: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pole {
    pub index: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ObdlznikCss {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Prienik okien musíme počítať, aby sme pri hrách roztiahnutých cez viaceré monitory
/// zistili, z ktorého monitora máme prebrať nastavenia mierky (DPI scaling).
pub fn prekryv(a: &OknoFyz, m: &Monitor) -> i64 {
    let l1 = a.x as i64;
    let r1 = a.x as i64 + a.sirka as i64;
    let t1 = a.y as i64;
    let b1 = a.y as i64 + a.vyska as i64;

    let l2 = m.x as i64;
    let r2 = m.x as i64 + m.sirka as i64;
    let t2 = m.y as i64;
    let b2 = m.y as i64 + m.vyska as i64;

    let pre_x = 0i64.max(r1.min(r2) - l1.max(l2));
    let pre_y = 0i64.max(b1.min(b2) - t1.max(t2));

    pre_x * pre_y
}

/// Zvolí monitor, na ktorom je zobrazená najväčšia časť hry, pretože ten určuje
/// správanie myši a vykresľovanie overlay okna operačným systémom.
pub fn monitor_okna<'a>(okno: &OknoFyz, monitory: &'a [Monitor]) -> Option<&'a Monitor> {
    let mut naj_monitor = None;
    let mut naj_prienik = 0;

    for m in monitory {
        let p = prekryv(okno, m);
        if p > naj_prienik {
            naj_prienik = p;
            naj_monitor = Some(m);
        }
    }

    naj_monitor
}

/// Prepočítava normalizované súradnice 0-1 (x, y, w, h) do CSS pixelov nášho okna.
/// Ochranné orezanie zabezpečuje, že prvok nikdy nevykreslíme mimo viditeľnú plochu,
/// čo by mohlo znefunkčniť interakciu hráča. Spoločný základ pre `pole_na_css`
/// (Tartarus) aj pre Frozen Canvas krokov `zdroj`/`ciel` (TLI 4b, `Box01` v `api.rs`)
/// — JEDNA implementácia prepočtu, nie dve, ktoré by sa mohli rozísť.
pub fn box01_na_css(x: f64, y: f64, w: f64, h: f64, okno: &OknoFyz, mierka: f64) -> ObdlznikCss {
    let m = if mierka <= 0.0 { 1.0 } else { mierka };

    let x = x.clamp(0.0, 1.0);
    let y = y.clamp(0.0, 1.0);

    let mut w = w.max(0.0);
    let mut h = h.max(0.0);

    if x + w > 1.0 { w = 1.0 - x; }
    if y + h > 1.0 { h = 1.0 - y; }

    let w_log = okno.sirka as f64 / m;
    let h_log = okno.vyska as f64 / m;

    ObdlznikCss {
        x: x * w_log,
        y: y * h_log,
        w: w * w_log,
        h: h * h_log,
    }
}

/// Prepočítava normalizované súradnice (0-1) zaslané serverom do CSS pixelov nášho okna.
pub fn pole_na_css(pole: &Pole, okno: &OknoFyz, mierka: f64) -> ObdlznikCss {
    box01_na_css(pole.x, pole.y, pole.w, pole.h, okno, mierka)
}

/// Inverzná funkcia k `box01_na_css` — TLI 21 (posúvanie okien nad hrou): JS
/// v okne `overlay` pozná len CSS pixely, kde hráč panel pretiahol/zväčšil,
/// ale ukladať sa musí zlomok okna hry (nech to sedí aj po zmene rozlíšenia
/// či DPI). Prepočet robí JEDNO miesto v jadre, nie druhá kópia matematiky
/// v JavaScripte, ktorá by sa s touto časom rozišla.
pub fn css_na_box01(css: &ObdlznikCss, okno: &OknoFyz, mierka: f64) -> Pole {
    let m = if mierka <= 0.0 { 1.0 } else { mierka };
    let w_log = okno.sirka as f64 / m;
    let h_log = okno.vyska as f64 / m;

    let x = if w_log > 0.0 { css.x / w_log } else { 0.0 };
    let y = if h_log > 0.0 { css.y / h_log } else { 0.0 };
    let w = if w_log > 0.0 { css.w / w_log } else { 0.0 };
    let h = if h_log > 0.0 { css.h / h_log } else { 0.0 };

    Pole {
        index: 0,
        x: x.clamp(0.0, 1.0),
        y: y.clamp(0.0, 1.0),
        w: w.max(0.0),
        h: h.max(0.0),
    }
}

/// Oddeľovač medzi menom monitora a zvyškom kľúča (rozlíšenie@mierka). Nesmie
/// sa objaviť v Windows názve zariadenia (`\\.\DISPLAY1`), tie '#' nepoužívajú.
const ZNACKA_MENA: char = '#';

/// Stabilný textový kľúč monitora (meno + rozlíšenie + mierka) — TLI 21/26
/// (posúvanie overlay okien) ním rozlišuje uložené rozloženia medzi monitormi.
///
/// **TLI 26, bod 2:** do 0.18.2 kľúč obsahoval LEN rozlíšenie a mierku — dva
/// FYZICKY rôzne monitory s rovnakým rozlíšením (Richardove dva 2560×1440)
/// preto zdieľali jedno uložené rozloženie a prepisovali si navzájom polohu
/// panelov. Meno monitora od OS (`m.nazov`) je teraz súčasťou kľúča, keď ho
/// appka pozná — dve fyzicky odlišné obrazovky tak dostanú odlišný kľúč aj
/// pri identickom rozlíšení/mierke. Keď meno nepozná (CI, testy), kľúč je
/// rovnaký ako predtým (len rozlíšenie@mierka) — `poloha()` v `rozlozenie.rs`
/// vie takýto "starý formát" kľúča nájsť aj spätne cez
/// `monitor_signatura_bez_mena`, nech sa staré uložené pozície hráčov pri
/// aktualizácii nestratia.
pub fn monitor_signatura(m: &Monitor) -> String {
    let rozlisenie = format!("{}x{}@{}", m.sirka, m.vyska, (m.mierka * 100.0).round() as i64);
    match m.nazov.as_deref().filter(|n| !n.is_empty()) {
        Some(nazov) => format!("{nazov}{ZNACKA_MENA}{rozlisenie}"),
        None => rozlisenie,
    }
}

/// Inverzná pomôcka k `monitor_signatura` pre MIGRÁCIU starých uložených
/// polôh (TLI 26): z kľúča s menom monitora vytiahne "starý" tvar bez mena
/// (čistý `rozlíšenie@mierka`), pod akým appka ukladala polohy pred touto
/// zmenou. Keď `sig` meno vôbec neobsahuje, vráti `None` — niet z čoho
/// odvodiť iný (starší) kľúč, lebo oba tvary sú už zhodné.
pub fn monitor_signatura_bez_mena(sig: &str) -> Option<String> {
    sig.rsplit_once(ZNACKA_MENA).map(|(_, rozlisenie)| rozlisenie.to_string())
}

/// Nájdenie stredu potrebujeme pre presné smerovanie navigačných vizualizácií (šípky),
/// ktoré ukazujú hráčovi, kam má presunúť myš.
pub fn stred_css(r: &ObdlznikCss) -> (f64, f64) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// Konverzia fyzických pixelov z Windows API na logické pixely webového prostredia (CSS),
/// aby sme vedeli správne nastaviť rozmery overlay kontajnera v prehliadači.
pub fn logicka_velkost(okno: &OknoFyz, mierka: f64) -> (f64, f64) {
    let m = if mierka <= 0.0 { 1.0 } else { mierka };
    (okno.sirka as f64 / m, okno.vyska as f64 / m)
}

#[cfg(test)]
mod testy {
    use super::*;

    fn takmer(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn rovnake_css(a: ObdlznikCss, b: ObdlznikCss) -> bool {
        takmer(a.x, b.x) && takmer(a.y, b.y) && takmer(a.w, b.w) && takmer(a.h, b.h)
    }

    #[test]
    fn pole_na_css_pri_100_percentach() {
        let okno = OknoFyz { x: 0, y: 0, sirka: 1920, vyska: 1080 };
        let pole = Pole { index: 0, x: 0.5, y: 0.25, w: 0.1, h: 0.1 };
        let css = pole_na_css(&pole, &okno, 1.0);
        assert!(rovnake_css(css, ObdlznikCss { x: 960.0, y: 270.0, w: 192.0, h: 108.0 }));
    }

    #[test]
    fn pole_na_css_pri_125_percentach() {
        let okno = OknoFyz { x: 0, y: 0, sirka: 1920, vyska: 1080 };
        let pole = Pole { index: 0, x: 0.5, y: 0.25, w: 0.1, h: 0.1 };
        let css = pole_na_css(&pole, &okno, 1.25);
        assert!(rovnake_css(css, ObdlznikCss { x: 768.0, y: 216.0, w: 153.6, h: 86.4 }));
    }

    #[test]
    fn pole_na_css_pri_150_percentach() {
        let okno = OknoFyz { x: 0, y: 0, sirka: 1920, vyska: 1080 };
        let pole = Pole { index: 0, x: 0.5, y: 0.25, w: 0.1, h: 0.1 };
        let css = pole_na_css(&pole, &okno, 1.5);
        assert!(rovnake_css(css, ObdlznikCss { x: 640.0, y: 180.0, w: 128.0, h: 72.0 }));
    }

    #[test]
    fn pole_mimo_rozsahu_sa_oreze() {
        let okno = OknoFyz { x: 0, y: 0, sirka: 1000, vyska: 1000 };
        let pole = Pole { index: 0, x: -0.2, y: 0.9, w: 5.0, h: 5.0 };
        let css = pole_na_css(&pole, &okno, 1.0);
        assert!(rovnake_css(css, ObdlznikCss { x: 0.0, y: 900.0, w: 1000.0, h: 100.0 }));
    }

    #[test]
    fn nulova_mierka_nepadne() {
        let okno = OknoFyz { x: 0, y: 0, sirka: 1000, vyska: 1000 };
        let pole = Pole { index: 0, x: 1.0, y: 1.0, w: 1.0, h: 1.0 };
        let css = pole_na_css(&pole, &okno, 0.0);
        assert!(rovnake_css(css, ObdlznikCss { x: 1000.0, y: 1000.0, w: 0.0, h: 0.0 }));
    }

    #[test]
    fn monitor_sa_vyberie_podla_vacsieho_prekryvu() {
        let m_a = Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0, nazov: None };
        let m_b = Monitor { x: -2560, y: 0, sirka: 2560, vyska: 1440, mierka: 1.25, nazov: None };
        let m_c = Monitor { x: 1920, y: 0, sirka: 3840, vyska: 2160, mierka: 1.5, nazov: None };
        let monitory = vec![m_a, m_b, m_c];

        // Väčšina okna (1200 z 1600 px) leží na B, hoci jeho pravý okraj zasahuje na A.
        let o1 = OknoFyz { x: -1200, y: 100, sirka: 1600, vyska: 900 };
        assert_eq!(monitor_okna(&o1, &monitory).unwrap().mierka, 1.25);

        // A naopak: okno, ktoré len kúskom prečnieva na B, patrí monitoru A.
        let o1b = OknoFyz { x: -100, y: 100, sirka: 1600, vyska: 900 };
        assert_eq!(monitor_okna(&o1b, &monitory).unwrap().mierka, 1.0);

        let o2 = OknoFyz { x: 2000, y: 200, sirka: 1000, vyska: 800 };
        assert_eq!(monitor_okna(&o2, &monitory).unwrap().mierka, 1.5);

        let o3 = OknoFyz { x: 100, y: 100, sirka: 800, vyska: 600 };
        assert_eq!(monitor_okna(&o3, &monitory).unwrap().mierka, 1.0);
    }

    #[test]
    fn okno_na_hranici_dvoch_monitorov() {
        let m_a = Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0, nazov: None };
        let m_b = Monitor { x: -2560, y: 0, sirka: 2560, vyska: 1440, mierka: 1.25, nazov: None };
        let monitory = vec![m_a, m_b];

        let o = OknoFyz { x: -200, y: 0, sirka: 800, vyska: 600 };
        assert_eq!(monitor_okna(&o, &monitory).unwrap().mierka, 1.0);
    }

    #[test]
    fn ziadny_monitor_vrati_none() {
        let monitory = vec![];
        let o = OknoFyz { x: 0, y: 0, sirka: 800, vyska: 600 };
        assert!(monitor_okna(&o, &monitory).is_none());
    }

    #[test]
    fn box01_na_css_je_zaklad_pole_na_css() {
        // Rovnaký vstup cez oba vstupné body musí dať identický výsledok —
        // `pole_na_css` je len tenký obal nad `box01_na_css` (TLI 4b).
        let okno = OknoFyz { x: 0, y: 0, sirka: 1920, vyska: 1080 };
        let pole = Pole { index: 3, x: 0.2, y: 0.4, w: 0.15, h: 0.1 };
        let a = pole_na_css(&pole, &okno, 1.25);
        let b = box01_na_css(pole.x, pole.y, pole.w, pole.h, &okno, 1.25);
        assert!(rovnake_css(a, b));
    }

    #[test]
    fn logicka_velkost_pri_150() {
        let o = OknoFyz { x: 0, y: 0, sirka: 1920, vyska: 1080 };
        let (w, h) = logicka_velkost(&o, 1.5);
        assert!(takmer(w, 1280.0));
        assert!(takmer(h, 720.0));
    }

    #[test]
    fn monitor_signatura_rozlisuje_rozlisenie_aj_mierku() {
        let a = Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0, nazov: None };
        let b = Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.25, nazov: None };
        let c = Monitor { x: 0, y: 0, sirka: 2560, vyska: 1440, mierka: 1.0, nazov: None };
        assert_eq!(monitor_signatura(&a), "1920x1080@100");
        assert_eq!(monitor_signatura(&b), "1920x1080@125");
        assert_eq!(monitor_signatura(&c), "2560x1440@100");
        assert_ne!(monitor_signatura(&a), monitor_signatura(&b));
        assert_ne!(monitor_signatura(&a), monitor_signatura(&c));
    }

    /// TLI 26, bod 2 — regresia na 0.18.2: dva FYZICKY odlišné monitory s
    /// rovnakým rozlíšením a mierkou (Richardove dva 2560×1440) dostali do
    /// 0.18.2 TOTOŽNÝ kľúč, takže si prepisovali uloženú polohu panelov.
    /// Padá na 0.18.2 (kde `Monitor` meno vôbec nemá), prechádza po oprave.
    #[test]
    fn monitor_signatura_rozlisuje_fyzicky_rovnake_monitory_podla_mena() {
        let display1 = Monitor {
            x: 0, y: 0, sirka: 2560, vyska: 1440, mierka: 1.0,
            nazov: Some(r"\\.\DISPLAY1".to_string()),
        };
        let display2 = Monitor {
            x: 2560, y: 0, sirka: 2560, vyska: 1440, mierka: 1.0,
            nazov: Some(r"\\.\DISPLAY2".to_string()),
        };
        assert_ne!(monitor_signatura(&display1), monitor_signatura(&display2));
    }

    /// Keď appka meno nepozná (CI, testy, staršie appky bez tejto zmeny),
    /// kľúč ostáva presne v starom tvare — `poloha()` v `rozlozenie.rs` sa
    /// naň spolieha pri migrácii starých uložených polôh.
    #[test]
    fn monitor_signatura_bez_mena_je_zhodna_so_starym_tvarom() {
        let m = Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0, nazov: None };
        assert_eq!(monitor_signatura(&m), "1920x1080@100");
    }

    /// Prázdny reťazec mena (niektoré ovládače ho vracajú namiesto `None`) sa
    /// správa rovnako ako `None` — inak by appka ukladala kľúče s otravným
    /// osamoteným oddeľovačom "#1920x1080@100".
    #[test]
    fn monitor_signatura_prazdne_meno_sa_berie_ako_ziadne() {
        let m = Monitor {
            x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.0,
            nazov: Some(String::new()),
        };
        assert_eq!(monitor_signatura(&m), "1920x1080@100");
    }

    #[test]
    fn monitor_signatura_bez_mena_vytiahne_rozlisenie_zo_zlozeneho_kluca() {
        assert_eq!(
            monitor_signatura_bez_mena(r"\\.\DISPLAY1#2560x1440@100"),
            Some("2560x1440@100".to_string())
        );
        // Bez mena v kľúči niet čo odvodiť — je to už ten istý (starý) tvar.
        assert_eq!(monitor_signatura_bez_mena("1920x1080@100"), None);
    }

    /// Presne Richardovo nahlásené usporiadanie troch monitorov
    /// (`2026-10-17-2231-mcp.md`): dva rovnaké 2560×1440 vedľa seba a tretí
    /// 1200×1920 NA VÝŠKU posunutý do záporných súradníc (portrait monitor
    /// umiestnený vľavo hore od "0,0"). Overuje, že `monitor_okna` správne
    /// vyberie monitor aj v tomto rozložení a že DISPLAY1/DISPLAY2 dostanú
    /// pri rovnakom rozlíšení rôzny kľúč.
    #[test]
    fn tri_monitory_richardovo_usporiadanie_zaporna_x_a_vyska() {
        let display1 = Monitor {
            x: 0, y: 0, sirka: 2560, vyska: 1440, mierka: 1.0,
            nazov: Some(r"\\.\DISPLAY1".to_string()),
        };
        let display2 = Monitor {
            x: 2560, y: 0, sirka: 2560, vyska: 1440, mierka: 1.0,
            nazov: Some(r"\\.\DISPLAY2".to_string()),
        };
        // Portrétový monitor: šírka a výška fyzicky preHODENÉ oproti bežnému
        // landscape, posunutý do záporných súradníc oboch osí.
        let display3 = Monitor {
            x: -1200, y: -237, sirka: 1200, vyska: 1920, mierka: 1.0,
            nazov: Some(r"\\.\DISPLAY3".to_string()),
        };
        let monitory = vec![display1.clone(), display2.clone(), display3.clone()];

        let hra_na_jednotke = OknoFyz { x: 100, y: 100, sirka: 1920, vyska: 1080 };
        assert_eq!(monitor_okna(&hra_na_jednotke, &monitory).unwrap().nazov, display1.nazov);

        let hra_na_dvojke = OknoFyz { x: 2660, y: 100, sirka: 1920, vyska: 1080 };
        assert_eq!(monitor_okna(&hra_na_dvojke, &monitory).unwrap().nazov, display2.nazov);

        // Okno v hornej časti portrétového monitora — čiastočne v zápornom y.
        let hra_na_trojke = OknoFyz { x: -1100, y: -200, sirka: 1000, vyska: 800 };
        assert_eq!(monitor_okna(&hra_na_trojke, &monitory).unwrap().nazov, display3.nazov);

        // Kľúč pre rovnaké okno na DISPLAY1 vs. DISPLAY2 sa MUSÍ líšiť, hoci
        // obe majú identické rozlíšenie aj mierku — to je presne TLI 26 bod 2.
        let sig1 = monitor_signatura(monitor_okna(&hra_na_jednotke, &monitory).unwrap());
        let sig2 = monitor_signatura(monitor_okna(&hra_na_dvojke, &monitory).unwrap());
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn css_na_box01_je_inverzna_k_box01_na_css_pri_100_125_150() {
        let okno = OknoFyz { x: 0, y: 0, sirka: 1920, vyska: 1080 };
        let povodne = Pole { index: 0, x: 0.3, y: 0.4, w: 0.2, h: 0.15 };
        for mierka in [1.0, 1.25, 1.5] {
            let css = pole_na_css(&povodne, &okno, mierka);
            let spat = css_na_box01(&css, &okno, mierka);
            assert!(takmer(spat.x, povodne.x), "mierka {mierka}: x {} != {}", spat.x, povodne.x);
            assert!(takmer(spat.y, povodne.y), "mierka {mierka}: y");
            assert!(takmer(spat.w, povodne.w), "mierka {mierka}: w");
            assert!(takmer(spat.h, povodne.h), "mierka {mierka}: h");
        }
    }

    #[test]
    fn css_na_box01_oreze_zaporne_aj_nadrozmerne_hodnoty() {
        let okno = OknoFyz { x: 0, y: 0, sirka: 1000, vyska: 1000 };
        let css = ObdlznikCss { x: -50.0, y: 1500.0, w: 300.0, h: 300.0 };
        let p = css_na_box01(&css, &okno, 1.0);
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 1.0);
        assert!(takmer(p.w, 0.3));
        assert!(takmer(p.h, 0.3));
    }

    #[test]
    fn css_na_box01_nulova_mierka_nepadne() {
        let okno = OknoFyz { x: 0, y: 0, sirka: 1000, vyska: 1000 };
        let css = ObdlznikCss { x: 500.0, y: 500.0, w: 100.0, h: 100.0 };
        let p = css_na_box01(&css, &okno, 0.0);
        assert!(takmer(p.x, 0.5));
        assert!(takmer(p.w, 0.1));
    }

    #[test]
    fn monitor_signatura_rovnaky_monitor_da_rovnaky_kluc() {
        // Pozícia monitora (x/y) nie je súčasťou kľúča zámerne — rovnaký fyzický
        // monitor môže mať iné súradnice po prepnutí primárneho displeja vo
        // Windows, no rozloženie panelov na ňom má zostať to isté.
        let a = Monitor { x: 0, y: 0, sirka: 1920, vyska: 1080, mierka: 1.5, nazov: None };
        let b = Monitor { x: -1920, y: 200, sirka: 1920, vyska: 1080, mierka: 1.5, nazov: None };
        assert_eq!(monitor_signatura(&a), monitor_signatura(&b));
    }
}
