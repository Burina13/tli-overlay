use std::ffi::c_void;
use tli_overlay_core::suradnice::OknoFyz;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, MAX_PATH, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible,
};

pub struct NajdeneOkno {
    pub hwnd: isize,
    pub nazov: String,
    pub proces: String,
    pub obdlznik: OknoFyz,
}

/// Zbavi text vsetkeho okrem alfanumerickych znakov a prevedie ho na male pismena.
/// DOVOD: Hrac moze zadat proces ako "Torchlight: Infinite" alebo "torchlightinfinite.exe",
/// no my to potrebujeme vzdy sparovat rovnako bez ohladu na medzery, velkosti pismen alebo pripony.
pub fn normalizuj(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Zisti, ci nazov okna alebo meno procesu pasuje aspon na jeden vzor z ciarkou oddeleneho zoznamu.
/// DOVOD: Jednoducha a flexibilna konfiguracia. Umoznuje hracovi najst hru na roznych platformach
/// (napr. Steam vs klientsky launcher), kde sa proces moze volat inak, ale stale ide o tu istu hru.
pub fn sedi(nazov_okna: &str, proces: &str, hladane: &str) -> bool {
    let hladane_casti = hladane.split(',');
    let norm_nazov = normalizuj(nazov_okna);
    let norm_proces = normalizuj(proces);

    for cast in hladane_casti {
        let norm_cast = normalizuj(cast);
        if norm_cast.len() > 2 {
            if norm_nazov.contains(&norm_cast) || norm_proces.contains(&norm_cast) {
                return true;
            }
        }
    }
    false
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // BEZPECNOST: Ukazovatel lparam sme vytvorili a predali my vo funkcii zoznam_okien()
    // Je zivotnosti len pocas trvania tohto volania, takze je platny.
    let zoznam: &mut Vec<NajdeneOkno> = unsafe { &mut *(lparam.0 as *mut Vec<NajdeneOkno>) };

    // BEZPECNOST: Vsetky FFI volania pouzivaju platny HWND, pricom zlyhanie vratia korektne,
    // a my pokracujeme dalej. Nikde sa neuvolnuje nevlastnena pamat.
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return BOOL(1);
        }

        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId() {
            return BOOL(1);
        }

        let mut nazov = String::new();
        let dlzka = GetWindowTextLengthW(hwnd);
        if dlzka > 0 {
            let mut buf: Vec<u16> = vec![0; (dlzka + 1) as usize];
            let precitane = GetWindowTextW(hwnd, &mut buf);
            if precitane > 0 {
                nazov = String::from_utf16_lossy(&buf[..precitane as usize]);
            }
        }

        if nazov.trim().is_empty() {
            return BOOL(1);
        }

        let mut rect = RECT::default();
        let ma_obdlznik = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut c_void,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok()
            || GetWindowRect(hwnd, &mut rect).is_ok();

        if !ma_obdlznik {
            return BOOL(1);
        }

        let sirka = (rect.right - rect.left).max(0) as u32;
        let vyska = (rect.bottom - rect.top).max(0) as u32;

        if sirka < 200 || vyska < 200 {
            return BOOL(1);
        }

        let mut proces = String::new();
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut buf = vec![0u16; MAX_PATH as usize];
            let mut dlzka_cesty = buf.len() as u32;
            if QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut dlzka_cesty,
            )
            .is_ok()
            {
                proces = String::from_utf16_lossy(&buf[..dlzka_cesty as usize]);
            }
            let _ = CloseHandle(handle);
        }

        zoznam.push(NajdeneOkno {
            hwnd: hwnd.0 as isize,
            nazov,
            proces,
            obdlznik: OknoFyz {
                x: rect.left,
                y: rect.top,
                sirka,
                vyska,
            },
        });
    }

    BOOL(1)
}

/// Vracia vsetky validne okenne procesy na ploche (ne-minimalizovane, viditelne a vacsie nez 200x200).
/// DOVOD: Potrebujeme ziskat zoznam beznych aplikacii (a hier) na systeme na naslednu analyzu
/// a vyfiltrovanie konkretnej hry, pre ktoru sa ma robit screenshot a overlay, no s ohladom na ToS pravidla (iba citanie).
pub fn zoznam_okien() -> Vec<NajdeneOkno> {
    let mut zoznam = Vec::new();
    let lparam = LPARAM(&mut zoznam as *mut _ as isize);

    // BEZPECNOST: Funkcii predavame nativny Windows C-callback s garanciou rovnakej signatury.
    // Lparam ukazuje na nase bezpecne namapovane Rust vektory.
    unsafe {
        let _ = EnumWindows(Some(enum_windows_callback), lparam);
    }
    zoznam
}

/// Najde najvacsie okno odpovedajuce poziadavke hladaneho retazca (titulok alebo meno procesu).
/// DOVOD: Hra moze mat otvorene rozne sekundarne okna (napr. spustatela, hlasenia chyb).
/// Rozmerova priorita zabezpeci, ze cielom nasho zachytenia bude samotne herne prostredie a nie launcher.
pub fn najdi_hru(hladane: &str) -> Option<NajdeneOkno> {
    zoznam_okien()
        .into_iter()
        .filter(|o| sedi(&o.nazov, &o.proces, hladane))
        .max_by_key(|o| o.obdlznik.sirka * o.obdlznik.vyska)
}

/// Poskytne zhodnotenie o aktualne aktivnom okne.
/// DOVOD: Vynimocna situacia (napr. zmenene okno alebo nesulad nazvu po aktualizacii).
/// Ak explicitne najdenie zlyha, pokusime sa aplikovat overlay priamo na to, co ma hrac prave pred ocami
/// za predpokladu, ze to nie sme my sami.
pub fn popredie() -> Option<NajdeneOkno> {
    // BEZPECNOST: Ziskavanie informacii o beziacich oknach je v operacnom systeme uplne beznym chovanim.
    // Nesnazime sa aplikovat ziaden injection, iba citame identifikatory.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId() {
            return None;
        }

        let mut rect = RECT::default();
        let ma_obdlznik = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut c_void,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok()
            || GetWindowRect(hwnd, &mut rect).is_ok();

        if !ma_obdlznik {
            return None;
        }

        Some(NajdeneOkno {
            hwnd: hwnd.0 as isize,
            nazov: String::new(),
            proces: String::new(),
            obdlznik: OknoFyz {
                x: rect.left,
                y: rect.top,
                sirka: (rect.right - rect.left).max(0) as u32,
                vyska: (rect.bottom - rect.top).max(0) as u32,
            },
        })
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn test_normalizuj() {
        assert_eq!(normalizuj("Torchlight: Infinite"), "torchlightinfinite");
        assert_eq!(normalizuj("TorchlightInfinite.exe"), "torchlightinfiniteexe");
    }

    #[test]
    fn test_sedi_hra() {
        assert!(sedi(
            "Torchlight: Infinite",
            "c:\\hry\\tli.exe",
            "Torchlight Infinite"
        ));
        assert!(sedi(
            "Hra",
            "TorchlightInfinite.exe",
            "Torchlight Infinite"
        ));
    }

    #[test]
    fn test_sedi_nase_okno_nesmie_sediet() {
        assert!(!sedi(
            "Tvoj Tatko TLI Overlay",
            "tli-overlay.exe",
            "Torchlight Infinite"
        ));
    }

    #[test]
    fn test_sedi_kratka_polozka_sa_ignoruje() {
        // Hladanie len na "x" by sedelo aj na "explorer.exe", preto ignorujeme kratke retazce.
        assert!(!sedi("Hra xy", "hra.exe", "x"));
    }
}
