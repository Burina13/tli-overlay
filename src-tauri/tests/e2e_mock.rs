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
use tli_overlay_core::suradnice::{pole_na_css, OknoFyz};

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
