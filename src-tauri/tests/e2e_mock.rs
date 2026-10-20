//! E2E proti MOCK serveru: celá cesta JPEG → HTTP → JSON → obdĺžniky overlayu,
//! bez závislosti na živom `tli.tvojtatko.sk`. Mock drží kontrakt z
//! `stacks/games/tli-web/README.md` § „Kontrakt API pre overlay klienta".
//!
//! Prečo vlastný `TcpListener` a nie testovací framework: appka má jedinú sieťovú
//! závislosť (`ureq`) a pribrať kvôli testu celé tokio by bolo viac rizika než úžitku.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

use tli_overlay_core::api;
use tli_overlay_core::nastavenia::Nastavenia;
use tli_overlay_core::suradnice::{box01_na_css, pole_na_css, OknoFyz};

const ODPOVED_CANVAS: &str = r#"{
  "ok": true,
  "modul": "canvas",
  "platno": {},
  "kroky": [
    {
      "poradie": 1,
      "zdroj": {"typ_slotu": "ruka", "index": 0,
                "box": {"x": 0.10, "y": 0.90, "w": 0.08, "h": 0.08}},
      "ciel": {"typ_bunky": "platno", "riadok": 3, "stlpec": 3,
               "box": {"x": 0.30, "y": 0.30, "w": 0.08, "h": 0.08}},
      "dovod": "Vezmi dwarf Lv1 zo slotu 0 v ruke a polož ho na bunku (riadok 3, stĺpec 3)."
    },
    {
      "poradie": 2,
      "zdroj": {"typ_slotu": "ruka", "index": 1,
                "box": {"x": 0.20, "y": 0.90, "w": 0.08, "h": 0.08}},
      "ciel": {"typ_bunky": "platno", "riadok": 4, "stlpec": 2,
               "box": {"x": 0.20, "y": 0.40, "w": 0.08, "h": 0.08}},
      "dovod": "Vezmi dwarf Lv1 zo slotu 1..."
    }
  ],
  "istota": 0.9,
  "istota_rozpoznania": 0.95,
  "skontroluj": [],
  "trvanie_ms": 4300,
  "subor": "2026-10-16-040000-abcd1234.png"
}"#;

const ODPOVED: &str = r#"{
  "ok": true,
  "board": {},
  "odporucanie": {"akcia":"devour","nazov":"Požretie","ciel":6,"cez_scratch":false,
                  "nastroj":null,"dar":null,"dovod":"Požretie zvyšuje raritu a nestojí nič.",
                  "hodnota":15.25},
  "alternativy": [{"akcia":"swap","nazov":"Výmena","ciel":null,"hodnota":14.57,
                   "istota":0.32,"dovod":"Výmena pripraví lepšiu pozíciu."}],
  "istota": 0.41,
  "istota_rozpoznania": 0.87,
  "skontroluj": ["cells[7].inmate.tier"],
  "polia": [{"index":6,"x":0.47,"y":0.08,"w":0.09,"h":0.09},
            {"index":7,"x":0.56,"y":0.08,"w":0.09,"h":0.09}],
  "trvanie_ms": 4120,
  "subor": "2026-10-16-203455-a1b2c3d4.jpg",
  "cas_s": 0.41
}"#;

/// Jedno spojenie, jedna odpoveď. Vracia (port, vlákno) a cez kanál to, čo klient poslal.
fn mock(
    stav: u16,
    telo: &'static str,
) -> (u16, std::sync::mpsc::Receiver<(String, Vec<u8>)>) {
    let posluchac = TcpListener::bind("127.0.0.1:0").expect("mock sa nedá nabindovať");
    let port = posluchac.local_addr().unwrap().port();
    let (odosielatel, prijimac) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let (mut spojenie, _) = posluchac.accept().expect("mock neprijal spojenie");
        let mut citac = BufReader::new(spojenie.try_clone().unwrap());

        let mut hlavicky = String::new();
        let mut dlzka = 0usize;
        loop {
            let mut riadok = String::new();
            if citac.read_line(&mut riadok).unwrap_or(0) == 0 {
                break;
            }
            if riadok.to_ascii_lowercase().starts_with("content-length:") {
                dlzka = riadok.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
            }
            hlavicky.push_str(&riadok);
            if riadok == "\r\n" || riadok == "\n" {
                break;
            }
        }
        let mut telo_poziadavky = vec![0u8; dlzka];
        if dlzka > 0 {
            let _ = citac.read_exact(&mut telo_poziadavky);
        }
        let _ = odosielatel.send((hlavicky, telo_poziadavky));

        let odpoved = format!(
            "HTTP/1.1 {stav} X\r\nContent-Type: application/json; charset=utf-8\r\n\
Content-Length: {}\r\nConnection: close\r\n\r\n{telo}",
            telo.as_bytes().len()
        );
        let _ = spojenie.write_all(odpoved.as_bytes());
        let _ = spojenie.flush();
    });

    (port, prijimac)
}

fn nastavenia(port: u16) -> Nastavenia {
    let mut n = Nastavenia::default();
    // `chyba_konfiguracie()` chce https, ale mock hovorí čistým HTTP — posiela sa
    // priamo cez `klient`, teda tou istou cestou ako naostro, len bez TLS.
    n.url_servera = format!("http://127.0.0.1:{port}");
    n.kluc = "tli_test_kluc".to_string();
    n
}

#[path = "../src/klient.rs"]
mod klient;

#[test]
fn e2e_analyza_az_po_obdlzniky_overlayu() {
    let (port, prijimac) = mock(200, ODPOVED);
    let n = nastavenia(port);

    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xD9];
    let v = klient::posli_analyzu(&n, jpeg.clone()).expect("mock mal odpovedať 200");

    // 1. Klient poslal presne to, čo kontrakt chce.
    let (hlavicky, telo) = prijimac.recv().expect("mock nič nedostal");
    assert!(hlavicky.starts_with("POST /api/analyza "), "zlá cesta: {hlavicky}");
    assert!(hlavicky.to_lowercase().contains("authorization: bearer tli_test_kluc"));
    assert!(hlavicky.to_lowercase().contains("content-type: image/jpeg"));
    assert_eq!(telo, jpeg);

    // 2. Odpoveď sa rozparsovala.
    assert!(v.analyza.ok);
    let o = v.analyza.odporucanie.as_ref().expect("odporúčanie chýba");
    assert_eq!(o.akcia, "devour");
    assert_eq!(o.ciel, Some(6));
    assert_eq!(v.analyza.alternativy.len(), 1);
    assert_eq!(v.analyza.istota, Some(0.41));

    // 3. Políčka sa prepočítali na overlay nad oknom 2560×1440 pri 125 % DPI.
    let okno = OknoFyz { x: -2560, y: 0, sirka: 2560, vyska: 1440 };
    let polia = api::pouzitelne_polia(&v.analyza);
    assert_eq!(polia.len(), 2);
    let css = pole_na_css(polia[0], &okno, 1.25);
    assert!((css.x - 0.47 * 2560.0 / 1.25).abs() < 1e-9);
    assert!((css.y - 0.08 * 1440.0 / 1.25).abs() < 1e-9);

    // 4. Neisté políčko zo `skontroluj` sa pozná podľa indexu.
    assert!(api::je_neiste(&v.analyza, 7));
    assert!(!api::je_neiste(&v.analyza, 6));
}

#[test]
fn e2e_chyba_401_je_slovenska_veta() {
    let (port, _p) = mock(401, r#"{"ok": false, "chyba": "Neplatný kľúč.", "kod": 401}"#);
    let chyba = klient::posli_analyzu(&nastavenia(port), vec![0xFF, 0xD8])
        .expect_err("401 má byť chyba");
    assert_eq!(chyba, "Neplatný kľúč.");
}

#[test]
fn e2e_chyba_422_bez_tela_dostane_nasu_vetu() {
    let (port, _p) = mock(422, "nie je to JSON");
    let chyba = klient::posli_analyzu(&nastavenia(port), vec![0xFF, 0xD8])
        .expect_err("422 má byť chyba");
    assert!(chyba.contains("dosku Tartarus"), "neočakávaná veta: {chyba}");
}

// TLI 32 — telo presne podľa reálnej analýzy `2026-10-18-105603-67f8c864.json`
// (CT 114), len skrátené na podstatné polia: patrol na cieľ 6 ("Go to Cell 7"),
// alternatíva na cieľ 7 ("Go to Cell 8"), tri tlačidlá "Select a Destination"
// z reálneho `gemini-beh.py` volania nad tou istou snímkou (úloha
// `2026-10-18-1101-mcp.md`, krok 1).
const ODPOVED_S_TLACIDLOM: &str = r#"{
  "ok": true,
  "board": {},
  "odporucanie": {"akcia":"patrol","nazov":"Patrola","ciel":6,"cez_scratch":false,
                  "nastroj":null,"dar":null,
                  "dovod":"Zamknuté dvere prinášajú nový materiál pre reťaz.",
                  "hodnota":33.4667,"tlacidlo":1},
  "alternativy": [{"akcia":"patrol","nazov":"Patrola","ciel":7,"hodnota":33.05,
                   "istota":0.1833,"dovod":"...","tlacidlo":2}],
  "istota": 0.332,
  "istota_rozpoznania": 0.93,
  "skontroluj": [],
  "polia": [{"index":6,"x":0.68,"y":0.2,"w":0.082,"h":0.115},
            {"index":7,"x":0.68,"y":0.35,"w":0.082,"h":0.115}],
  "akcie_dole": [
    {"text": "Go to Cell 6", "x": 0.233, "y": 0.691, "w": 0.174, "h": 0.177},
    {"text": "Go to Cell 7", "x": 0.413, "y": 0.691, "w": 0.174, "h": 0.177},
    {"text": "Go to Cell 8", "x": 0.592, "y": 0.691, "w": 0.174, "h": 0.177}
  ],
  "trvanie_ms": 11368,
  "subor": "2026-10-18-105603-67f8c864.jpg"
}"#;

/// E2E (TLI 32): JPEG → HTTP → JSON s `akcie_dole` → appka vie priradiť
/// tlačidlo odporúčaniu AJ alternatíve, prepočítané na overlay rovnakým
/// prepočtom ako cely (`box01_na_css`).
#[test]
fn e2e_doska_s_tlacidlom_spodnej_listy() {
    let (port, _p) = mock(200, ODPOVED_S_TLACIDLOM);
    let n = nastavenia(port);

    let v = klient::posli_analyzu(&n, vec![0xFF, 0xD8]).expect("mock mal odpovedať 200");
    assert_eq!(v.analyza.akcie_dole.len(), 3);

    let o = v.analyza.odporucanie.as_ref().expect("odporúčanie chýba");
    let tlacidlo = api::tlacidlo_podla_indexu(&v.analyza, o.tlacidlo).expect("tlačidlo sa nenašlo");
    assert_eq!(tlacidlo.text, "Go to Cell 7");

    let alt_tlacidlo = api::tlacidlo_podla_indexu(&v.analyza, v.analyza.alternativy[0].tlacidlo)
        .expect("tlačidlo alternatívy sa nenašlo");
    assert_eq!(alt_tlacidlo.text, "Go to Cell 8");

    // Rovnaký prepočet na CSS ako pri celách — nad oknom 2560×1440 pri 125 % DPI.
    let okno = OknoFyz { x: -2560, y: 0, sirka: 2560, vyska: 1440 };
    let css = box01_na_css(tlacidlo.x, tlacidlo.y, tlacidlo.w, tlacidlo.h, &okno, 1.25);
    assert!((css.x - 0.413 * 2560.0 / 1.25).abs() < 1e-9);
    assert!((css.w - 0.174 * 2560.0 / 1.25).abs() < 1e-9);
}

/// E2E Frozen Canvas (TLI 4b): JPEG → HTTP `?modul=canvas` → JSON → PÁR
/// zelených rámikov (zdroj v ruke + cieľ na plátne) na krok, prepočítaných
/// na overlay presne tou istou funkciou ako Tartarus.
#[test]
fn e2e_canvas_analyza_az_po_parovane_obdlzniky() {
    let (port, prijimac) = mock(200, ODPOVED_CANVAS);
    let n = nastavenia(port);

    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xD9];
    let v = klient::posli_analyzu_canvas(&n, jpeg.clone(), "1-3.2-3")
        .expect("mock mal odpovedať 200");

    // 1. Klient poslal presne to, čo kontrakt chce — na modul=canvas endpoint.
    let (hlavicky, telo) = prijimac.recv().expect("mock nič nedostal");
    // TLI 69: v ceste je aj zoznam buniek, ktoré hra na snímke žlto orámovala.
    assert!(
        hlavicky.starts_with("POST /api/analyza?modul=canvas&ramy=1-3.2-3 "),
        "zlá cesta: {hlavicky}"
    );
    assert!(hlavicky.to_lowercase().contains("authorization: bearer tli_test_kluc"));
    assert_eq!(telo, jpeg);

    // 2. Odpoveď sa rozparsovala — DVA kroky, oba so zdrojom aj cieľom.
    assert!(v.analyza.ok);
    assert_eq!(v.analyza.modul, "canvas");
    assert_eq!(v.analyza.kroky.len(), 2);
    assert_eq!(v.analyza.kroky[0].poradie, 1);
    assert_eq!(v.analyza.kroky[0].ciel.riadok, 3);

    // 3. Oba kroky majú box na zdroji aj cieli → oba sú použiteľné.
    let pouzitelne = api::pouzitelne_kroky(&v.analyza);
    assert_eq!(pouzitelne.len(), 2);

    // 4. Súradnice sa prepočítajú na overlay nad oknom 2560×1440 pri 125 % DPI —
    // rovnakou funkciou (`box01_na_css`) ako políčka Tartarus (`pole_na_css`).
    let okno = OknoFyz { x: -2560, y: 0, sirka: 2560, vyska: 1440 };
    let krok1 = &v.analyza.kroky[0];
    let zdroj_box = krok1.zdroj.box_.expect("zdroj box chýba");
    let ciel_box = krok1.ciel.box_.expect("cieľ box chýba");
    let zdroj_css = box01_na_css(zdroj_box.x, zdroj_box.y, zdroj_box.w, zdroj_box.h, &okno, 1.25);
    let ciel_css = box01_na_css(ciel_box.x, ciel_box.y, ciel_box.w, ciel_box.h, &okno, 1.25);
    assert!((zdroj_css.x - 0.10 * 2560.0 / 1.25).abs() < 1e-9);
    assert!((ciel_css.x - 0.30 * 2560.0 / 1.25).abs() < 1e-9);
    // Zdroj a cieľ sú na iných súradniciach — dôkaz, že sa nepomiešali.
    assert!((zdroj_css.x - ciel_css.x).abs() > 1.0);
}

// ── Guide a synchronizácia s partiou (TLI 11, kroky 4 a 5) ──────────────────

const ODPOVED_GUIDE: &str = r#"{
  "ok": true,
  "verzia": "a1b2c3d4e5f6",
  "zdroj": "https://tlicompendium.com/en/build-guides/erika1-lvl1-to-ss20-ultimate-guide",
  "tahak": {"slug": "tahak-na-start", "nazov": "Ťahák", "html": "<p>Začni tu.</p>"},
  "kapitoly": [
    {"slug": "uvod", "nazov": "Úvod",
     "html": "<p><img src=\"/ikony/skill/thunder-spike.webp\" alt=\"Thunder Spike\"> a <script>zle()</script> koniec</p>"}
  ],
  "ikony": {"skill/thunder-spike.webp": "data:image/webp;base64,QUFB"}
}"#;

const ODPOVED_SYNC: &str = r#"{"ok": true, "prijatych": 2, "preskocenych": 1}"#;

#[test]
fn e2e_guide_od_servera_az_po_html_pre_okno() {
    let (port, prijimac) = mock(200, ODPOVED_GUIDE);
    let n = nastavenia(port);

    let telo = klient::stiahni_guide(&n, "?ak_novsie_ako=stara")
        .expect("guide sa nestiahol")
        .expect("server poslal 204, hoci mal poslať obsah");

    let (hlavicky, _) = prijimac.recv().expect("mock nedostal požiadavku");
    assert!(hlavicky.contains("GET /api/guide?ak_novsie_ako=stara"), "{hlavicky}");
    assert!(hlavicky.contains("Authorization: Bearer tli_test_kluc"));

    let g = tli_overlay_core::guide::z_odpovede(&telo, 1_000).expect("odpoveď sa nedá prečítať");
    assert_eq!(g.verzia, "a1b2c3d4e5f6");
    assert_eq!(g.stiahnuty, 1_000);

    let html = tli_overlay_core::guide::priprav_html(&g.kapitoly[0].html, &g.ikony);
    // Ikona sa vložila priamo do HTML (webview `img-src 'self' data:`)…
    assert!(html.contains("src=\"data:image/webp;base64,QUFB\""), "{html}");
    assert!(!html.contains("/ikony/"), "{html}");
    // …a skript zo servera sa do okna nedostane.
    assert!(!html.to_lowercase().contains("<script"), "{html}");
}

#[test]
fn e2e_guide_204_znamena_mame_najnovsie() {
    let (port, _prijimac) = mock(204, "");
    let n = nastavenia(port);
    assert!(klient::stiahni_guide(&n, "?ak_novsie_ako=a1b2c3d4e5f6")
        .expect("204 nie je chyba")
        .is_none());
}

#[test]
fn e2e_sync_posle_mapy_a_precita_zhrnutie() {
    use tli_overlay_core::tracker::agregacie::VysledokBehu;
    use tli_overlay_core::tracker::sync;

    let vysledky = vec![
        VysledokBehu {
            id: 1,
            zona: "Oaza".to_string(),
            podpis_vstupov: "400021x1".to_string(),
            podpis_kombinacie: "400021x1@Oaza".to_string(),
            zaciatok: "2026-10-15 20:00:00".to_string(),
            trvanie_ms: 300_000,
            hodnota_dropov: 468.0,
            hodnota_vstupov: 3.1,
            cisty_zisk: 464.9,
            bez_ceny: vec![],
            najstarsia_cena: None,
            zastarane: vec![],
            top_dropy: vec![],
        },
        // Bez času začiatku — podpis by nebol stabilný, takže sa neposiela.
        VysledokBehu {
            id: 2,
            zona: "Lava".to_string(),
            podpis_vstupov: "400021x1".to_string(),
            podpis_kombinacie: "400021x1@Lava".to_string(),
            zaciatok: String::new(),
            trvanie_ms: 240_000,
            hodnota_dropov: 306.0,
            hodnota_vstupov: 3.1,
            cisty_zisk: 302.9,
            bez_ceny: vec![],
            najstarsia_cena: None,
            zastarane: vec![],
            top_dropy: vec![],
        },
    ];

    let davka = sync::vyber_neposlane(&vysledky, &std::collections::BTreeSet::new(), 200);
    assert_eq!(davka.len(), 1, "beh bez času začiatku sa nemá posielať");

    let (port, prijimac) = mock(200, ODPOVED_SYNC);
    let n = nastavenia(port);
    let odpoved = klient::posli_tracker_sync(&n, &sync::telo_poziadavky(&davka, "0.9.0"))
        .expect("sync zlyhal");

    let (hlavicky, telo_poziadavky) = prijimac.recv().expect("mock nedostal požiadavku");
    assert!(hlavicky.contains("POST /api/tracker/sync"), "{hlavicky}");
    let poslane = String::from_utf8_lossy(&telo_poziadavky);
    assert!(poslane.contains("\"klient\":\"0.9.0\""), "{poslane}");
    assert!(poslane.contains("\"zona\":\"Oaza\""), "{poslane}");
    assert!(!poslane.contains("Lava"), "{poslane}");

    let v = sync::z_odpovede(&odpoved).expect("odpoveď sa nedá prečítať");
    assert_eq!((v.prijatych, v.preskocenych), (2, 1));
    assert!(sync::zhrnutie(&v).contains('2'));
}

/// E2E cesta zdieľaných cien (TLI 20, zadanie: „upload → agregát → download"):
/// klient pošle, čo si sám vyhľadal na Exchange, a to, čo stiahne naspäť,
/// sa prejaví v cenníku — teda beh s dropom prestane mať hodnotu 0 FE.
#[test]
fn e2e_ceny_upload_a_download_naplnia_cennik() {
    use std::collections::BTreeMap;
    use tli_overlay_core::tracker::ceny::{ocen, Cennik, Nastavenie, Zdroj};
    use tli_overlay_core::tracker::ceny_sync;
    use tli_overlay_core::tracker::db::ZaznamCeny;

    let zaznamy = vec![
        ZaznamCeny {
            polozka: 400021,
            cena_fe: 1.5,
            median_fe: Some(2.0),
            pocet_ponuk: Some(7),
            sezona: Some("1501".to_string()),
            zdroj: "vlastny_trh".to_string(),
            cas: Some("2026-10-16 10:12:03".to_string()),
            zapisane: 1_760_000_000,
        },
        // Cena stiahnutá od partie sa NESMIE poslať späť — inak by si agregát
        // potvrdzoval sám seba a „tri zdroje" by znamenali jeden.
        ZaznamCeny {
            polozka: 5080,
            cena_fe: 0.2,
            median_fe: None,
            pocet_ponuk: None,
            sezona: None,
            zdroj: "partia".to_string(),
            cas: None,
            zapisane: 1_760_000_001,
        },
    ];
    let davka = ceny_sync::vyber_na_odoslanie(&zaznamy, ceny_sync::MAX_NA_DAVKU);
    assert_eq!(davka.len(), 1);

    // ── upload ──
    let (port, prijimac) = mock(200, r#"{"ok":true,"prijatych":1,"preskocenych":0}"#);
    let n = nastavenia(port);
    let odpoved = klient::posli_ceny(&n, &ceny_sync::telo_uploadu(&davka, "0.15.0"))
        .expect("upload cien zlyhal");
    let (hlavicky, telo) = prijimac.recv().expect("mock nedostal požiadavku");
    assert!(hlavicky.contains("POST /api/ceny"), "{hlavicky}");
    assert!(hlavicky.contains("Authorization: Bearer tli_test_kluc"));
    let poslane = String::from_utf8_lossy(&telo);
    assert!(poslane.contains("\"polozka\":400021"), "{poslane}");
    assert!(poslane.contains("\"median\":2.0"), "{poslane}");
    assert!(!poslane.contains("5080"), "cena od partie sa nesmie poslať späť: {poslane}");
    let o = ceny_sync::z_odpovede_uploadu(&odpoved).expect("odpoveď sa nedá prečítať");
    assert_eq!(o.prijatych, 1);

    // ── download ──
    let agregat_json = r#"{"ok":true,"cas":1760000500,"ceny":[
        {"polozka":400021,"fe":1.6,"zdrojov":3,"pocet_ponuk":21,"kedy":1760000400,"istota":"overena"},
        {"polozka":5080,"fe":0.25,"zdrojov":1,"pocet_ponuk":4,"kedy":1760000400,"istota":"slaba"}
    ]}"#;
    let (port2, prijimac2) = mock(200, agregat_json);
    let n2 = nastavenia(port2);
    let telo2 = klient::stiahni_ceny(&n2, 1_760_000_000).expect("stiahnutie cien zlyhalo");
    let (hlavicky2, _) = prijimac2.recv().expect("mock nedostal požiadavku");
    assert!(hlavicky2.contains("GET /api/ceny?od=1760000000"), "{hlavicky2}");

    let agregat = ceny_sync::z_odpovede_agregatu(&telo2).expect("agregát sa nedá prečítať");
    let mut cennik = Cennik::novy();
    let zmenenych = ceny_sync::zapis_do_cennika(&agregat, &mut cennik, 1_760_000_500);
    assert_eq!(zmenenych, 2);
    assert_eq!(cennik.cena(400021).unwrap().zdroj, Zdroj::Partia);
    assert_eq!(cennik.cena(400021).unwrap().zdrojov, 3);

    // ── a to hlavné: beh s dropmi už NEMÁ hodnotu 0 FE ──
    let mut dropy = BTreeMap::new();
    dropy.insert(400021u32, 3i64);
    dropy.insert(5080u32, 10i64);
    let v = ocen(&dropy, &cennik, &Nastavenie::default());
    assert!(v.bez_ceny.is_empty(), "po stiahnutí cien nesmie nič ostať bez ceny");
    assert!((v.fe - (3.0 * 1.6 + 10.0 * 0.25)).abs() < 1e-9, "{}", v.fe);
    assert!(v.fe > 0.0, "presne toto bol problém TLI 20: beh s dropmi za 0 FE");
}
