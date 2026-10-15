//! Živé čítanie logu po dávkach (TLI 11c) — všetko, čo sa dá vyriešiť na
//! samotných bajtoch, bez skutočného súboru: rozpoznanie a zapamätanie
//! kódovania naprieč dávkami, delenie na kompletné riadky (nedokončený
//! posledný riadok sa odloží na ďalšiu dávku) a rozpoznanie rotácie.
//! Otvorenie/seek/read súboru rieši volajúci v `src-tauri`.

use super::log::Kodovanie;

/// Stav, ktorý treba držať medzi dvoma čítaniami toho istého súboru. Dá sa
/// perzistovať (je `Serialize`/`Deserialize`) — appka tak po reštarte
/// nemusí znova prechádzať celý, aj stomegový log od začiatku.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StavChvosta {
    /// Koľko bajtov zo súboru je už spracovaných.
    pub pozicia: u64,
    /// Zvyšok posledného, ešte nedokončeného riadku z predošlej dávky.
    pub zvysok: String,
    /// Kódovanie zistené z BOM na začiatku súboru. `None`, kým sme ešte
    /// neprečítali ani prvý bajt.
    pub kodovanie: Option<Kodovanie>,
    /// Pri UTF-16 môže dávka skončiť uprostred 2-bajtovej jednotky — ten
    /// osamelý bajt sa odloží sem a predradí sa pred ďalšiu dávku.
    pub nedoparovany_bajt: Option<u8>,
}

impl StavChvosta {
    pub fn novy() -> Self {
        Self::default()
    }

    /// Rozpozná rotáciu (hra log pri reštarte premenuje/skráti) podľa toho,
    /// že aktuálna veľkosť súboru je menšia, než kam sme už dočítali. Vtedy
    /// sa stav vynuluje na začiatok — volajúci nabudúce prečíta nový súbor
    /// od nuly a nič sa nezdvojí ani nestratí (vynulovaná pozícia = celý
    /// nový súbor je „nové" bajty). Nový súbor môže mať aj iné kódovanie,
    /// preto sa zabúda aj ono.
    ///
    /// Vracia `true`, keď k rotácii došlo.
    pub fn over_rotaciu(&mut self, aktualna_velkost: u64) -> bool {
        if aktualna_velkost < self.pozicia {
            *self = StavChvosta::default();
            true
        } else {
            false
        }
    }

    /// Spracuje novú dávku bajtov prečítanú od `self.pozicia` (volajúci ich
    /// tam aj naozaj našiel — táto funkcia iba posunie počítadlo a vráti
    /// kompletné riadky na ďalšie spracovanie).
    pub fn priprav_riadky(&mut self, bajty: &[u8]) -> Vec<String> {
        self.pozicia += bajty.len() as u64;

        let mut na_dekodovanie = Vec::with_capacity(bajty.len() + 1);
        if let Some(b) = self.nedoparovany_bajt.take() {
            na_dekodovanie.push(b);
        }
        na_dekodovanie.extend_from_slice(bajty);

        if self.kodovanie.is_none() {
            let (kodovanie, dlzka_bom) = Kodovanie::z_bom(&na_dekodovanie);
            self.kodovanie = Some(kodovanie);
            na_dekodovanie.drain(0..dlzka_bom);
        }
        let kodovanie = self.kodovanie.unwrap_or_default();

        // UTF-16 číta po dvojiciach bajtov — keď dávka skončí na nepárnom
        // počte, posledný osamelý bajt čaká na partnera v ďalšej dávke.
        if matches!(kodovanie, Kodovanie::Utf16Le | Kodovanie::Utf16Be) && na_dekodovanie.len() % 2 == 1 {
            self.nedoparovany_bajt = na_dekodovanie.pop();
        }

        let text = kodovanie.dekoduj(&na_dekodovanie);
        let mut data = std::mem::take(&mut self.zvysok);
        data.push_str(&text);

        let konci_riadkom = data.ends_with('\n');
        let mut riadky: Vec<&str> = data.split('\n').collect();
        if !konci_riadkom {
            self.zvysok = riadky.pop().unwrap_or("").to_string();
        }

        riadky
            .into_iter()
            .map(|r| r.trim_end_matches('\r').to_string())
            .collect()
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn jedna_cela_davka_utf8() {
        let mut s = StavChvosta::novy();
        let riadky = s.priprav_riadky(b"prvy\ndruhy\n");
        assert_eq!(riadky, vec!["prvy", "druhy", ""]);
        assert_eq!(s.pozicia, 11);
        assert_eq!(s.zvysok, "");
    }

    #[test]
    fn ciastocny_riadok_sa_dokonci_az_v_dalsej_davke() {
        let mut s = StavChvosta::novy();
        let prve = s.priprav_riadky(b"zaciatok rov");
        assert_eq!(prve, Vec::<String>::new());
        assert_eq!(s.zvysok, "zaciatok rov");

        let druhe = s.priprav_riadky(b"nakej vety\nnovy riadok\n");
        assert_eq!(druhe, vec!["zaciatok rovnakej vety", "novy riadok", ""]);
    }

    #[test]
    fn viacriadkovy_socket_blok_sa_da_poskladat_z_viacerych_davok() {
        // Rozdelenie zámerne padá DOVNÚTRA prvého riadku, nie na jeho konci —
        // presne to je prípad, ktorý pôvodný (nebufferovaný) kód nezvládal.
        let cely_blok = "----Socket SendMessage STT----Nieco----SynId = 1\n\
                          +prices+1+currency [100]\n\
                          ----Socket SendMessage End----\n";
        let (prva, druha) = cely_blok.as_bytes().split_at(30);
        let mut s = StavChvosta::novy();
        let mut vsetky = s.priprav_riadky(prva);
        vsetky.extend(s.priprav_riadky(druha));
        assert_eq!(
            vsetky,
            vec![
                "----Socket SendMessage STT----Nieco----SynId = 1",
                "+prices+1+currency [100]",
                "----Socket SendMessage End----",
                "",
            ]
        );
    }

    #[test]
    fn crlf_konce_riadkov_sa_orezu() {
        let mut s = StavChvosta::novy();
        let riadky = s.priprav_riadky(b"a\r\nb\r\n");
        assert_eq!(riadky, vec!["a", "b", ""]);
    }

    #[test]
    fn utf16_le_s_bom_cez_viacero_davok() {
        let mut s = StavChvosta::novy();
        // "ab\n" v UTF-16 LE s BOM, rozdelené na dve dávky uprostred znaku.
        let cele: Vec<u8> = {
            let mut v = vec![0xFF, 0xFE];
            for ch in "ab\n".encode_utf16() {
                v.extend_from_slice(&ch.to_le_bytes());
            }
            v
        };
        // Rozdelenie presne uprostred bajtov znaku 'b' (nepárna hranica).
        let (prva, druha) = cele.split_at(5);
        let mut vsetky = s.priprav_riadky(prva);
        vsetky.extend(s.priprav_riadky(druha));
        assert_eq!(vsetky, vec!["ab", ""]);
        assert_eq!(s.kodovanie, Some(Kodovanie::Utf16Le));
    }

    #[test]
    fn utf16_be_s_bom() {
        let mut s = StavChvosta::novy();
        let mut bajty = vec![0xFE, 0xFF];
        for ch in "x\n".encode_utf16() {
            bajty.extend_from_slice(&ch.to_be_bytes());
        }
        let riadky = s.priprav_riadky(&bajty);
        assert_eq!(riadky, vec!["x", ""]);
    }

    #[test]
    fn kodovanie_zistene_raz_plati_aj_pre_davky_bez_bom() {
        // Presne ten bug, ktorý mal pôvodný per-dávkový `dekoduj()`: druhá
        // a ďalšie dávky už BOM nemajú, lebo ten je v súbore len raz — bez
        // zapamätaného kódovania by sa omylom dekódovali ako UTF-8.
        let mut cele = vec![0xFF, 0xFE];
        for ch in "prva\ndruha\n".encode_utf16() {
            cele.extend_from_slice(&ch.to_le_bytes());
        }
        // Rozdelenie uprostred druhého riadku (nie na konci prvého) — druhá
        // dávka teda naozaj nemá BOM a ani nezačína čistou hranicou riadku.
        let (prva, druha) = cele.split_at(16);
        let mut s = StavChvosta::novy();
        let mut vsetky = s.priprav_riadky(prva);
        vsetky.extend(s.priprav_riadky(druha));
        assert_eq!(vsetky, vec!["prva", "druha", ""]);
    }

    #[test]
    fn rotacia_vynuluje_poziciu_zvysok_aj_kodovanie() {
        let mut s = StavChvosta::novy();
        s.priprav_riadky(&[0xFF, 0xFE, b'a', 0]);
        assert!(s.pozicia > 0);
        assert!(s.kodovanie.is_some());

        let rotovalo = s.over_rotaciu(2);
        assert!(rotovalo);
        assert_eq!(s.pozicia, 0);
        assert_eq!(s.zvysok, "");
        assert_eq!(s.kodovanie, None);
    }

    #[test]
    fn bez_rotacie_sa_nic_nemeni() {
        let mut s = StavChvosta::novy();
        s.priprav_riadky(b"abc\n");
        let pozicia_pred = s.pozicia;
        let rotovalo = s.over_rotaciu(1_000_000);
        assert!(!rotovalo);
        assert_eq!(s.pozicia, pozicia_pred);
    }

    #[test]
    fn perzistuje_sa_cez_json_bez_straty() {
        let mut s = StavChvosta::novy();
        s.priprav_riadky(b"nedokonceny riadok bez noveho");
        let json = serde_json::to_string(&s).unwrap();
        let s2: StavChvosta = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }
}
