use std::ffi::c_void;
use std::io::Cursor;
use tli_overlay_core::obraz::bgra_na_rgb;
use tli_overlay_core::suradnice::OknoFyz;
use windows::Win32::Foundation::HWND;

/// Nulový `HWND` znamená pre `GetDC`/`ReleaseDC` CELÚ virtuálnu plochu — teda
/// všetky monitory naraz, vrátane záporných súradníc vľavo od hlavného.
/// Vo `windows` 0.58 je `HWND` ukazovateľ, preto `null_mut()`, nie `plocha()`.
fn plocha() -> HWND {
    HWND(std::ptr::null_mut())
}
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, CAPTUREBLT, DIB_RGB_COLORS, HGDIOBJ,
    SRCCOPY,
};

pub struct Snimka {
    pub sirka: u32,
    pub vyska: u32,
    pub rgb: Vec<u8>,
}

/// Zachyti obrazovy obsah z virtualnej plochy nad poskytnutym obdlznikom.
/// DOVOD: Potrebujeme ziskat screenshot hry, na ktoru presne ukazuje konfiguracia a suradnice okenneho
/// procesu, no bez toho, aby sme sa napajali priamo do beziaceho herneho klienta. Volanie
/// obycajneho GDI umoznuje zachytit hru bez nabehu na ToS nalezitosti (vyhradne iba citame).
pub fn zachyt(obdlznik: &OknoFyz) -> Result<Snimka, String> {
    if obdlznik.sirka == 0 || obdlznik.vyska == 0 {
        return Err("Okno má nulové rozmery, nedá sa odfotiť.".to_string());
    }

    // BEZPECNOST: Praca s GDI, vsetky alokacie handleov su poctivo zaznamenane a na konci
    // alebo v pripade chyby sa rucne mazacu/uvolnuju.
    unsafe {
        let hdc_obrazovka = GetDC(plocha());
        if hdc_obrazovka.is_invalid() {
            return Err("Nepodarilo sa získať prístup k obrazovke (DC).".to_string());
        }

        let hdc_pamat = CreateCompatibleDC(hdc_obrazovka);
        if hdc_pamat.is_invalid() {
            ReleaseDC(plocha(), hdc_obrazovka);
            return Err("Nepodarilo sa vytvoriť pamäťový kontext obrazovky.".to_string());
        }

        let hbitmap = CreateCompatibleBitmap(hdc_obrazovka, obdlznik.sirka as i32, obdlznik.vyska as i32);
        if hbitmap.is_invalid() {
            let _ = DeleteDC(hdc_pamat);
            ReleaseDC(plocha(), hdc_obrazovka);
            return Err("Nepodarilo sa vytvoriť bitmapu na zachytenie.".to_string());
        }

        let stary_objekt = SelectObject(hdc_pamat, HGDIOBJ(hbitmap.0));
        if stary_objekt.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(hbitmap.0));
            let _ = DeleteDC(hdc_pamat);
            ReleaseDC(plocha(), hdc_obrazovka);
            return Err("Nepodarilo sa vybrať bitmapu do pamäte.".to_string());
        }

        let ziskane = BitBlt(
            hdc_pamat,
            0,
            0,
            obdlznik.sirka as i32,
            obdlznik.vyska as i32,
            hdc_obrazovka,
            obdlznik.x,
            obdlznik.y,
            SRCCOPY | CAPTUREBLT,
        );

        if ziskane.is_err() {
            SelectObject(hdc_pamat, stary_objekt);
            let _ = DeleteObject(HGDIOBJ(hbitmap.0));
            let _ = DeleteDC(hdc_pamat);
            ReleaseDC(plocha(), hdc_obrazovka);
            return Err("Kopírovanie pixelov z obrazovky zlyhalo.".to_string());
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: obdlznik.sirka as i32,
                biHeight: -(obdlznik.vyska as i32), // zhora nadol
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [std::mem::zeroed(); 1],
        };

        // Riadky GDI musi mat zarovnane na 4 bajty
        let stride = (obdlznik.sirka * 32 + 31) / 32 * 4;
        let pamat_velkost = (stride * obdlznik.vyska) as usize;
        let mut bgra = vec![0u8; pamat_velkost];

        // MSDN: bitmapa odovzdaná do `GetDIBits` NESMIE byť v tej chvíli vybratá
        // v device contexte — inak funkcia vráti 0 a snímka by bola prázdna.
        // Preto sa najprv vracia pôvodný objekt a až potom sa čítajú pixely.
        SelectObject(hdc_pamat, stary_objekt);

        let vysledok_get = GetDIBits(
            hdc_pamat,
            hbitmap,
            0,
            obdlznik.vyska,
            Some(bgra.as_mut_ptr() as *mut c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        let _ = DeleteObject(HGDIOBJ(hbitmap.0));
        let _ = DeleteDC(hdc_pamat);
        ReleaseDC(plocha(), hdc_obrazovka);

        if vysledok_get == 0 {
            return Err("Nepodarilo sa získať farby zo zhotovenej bitmapy.".to_string());
        }

        let rgb = bgra_na_rgb(&bgra, obdlznik.sirka, obdlznik.vyska, false);

        Ok(Snimka {
            sirka: obdlznik.sirka,
            vyska: obdlznik.vyska,
            rgb,
        })
    }
}

/// Skonvertuje pripravenu a analyzovanu bitmapu s RGB do JPEG komprimovaneho formatu vhodneho na upload.
/// DOVOD: Setrenie siete. Obraz musi odist bleskovo z klientskeho stroja na server kvoli rychlej spatnej vazbe hracovi.
pub fn do_jpegu(s: &Snimka, max_dlzka: u32, kvalita: u8) -> Result<Vec<u8>, String> {
    let raw_obrazok = image::RgbImage::from_raw(s.sirka, s.vyska, s.rgb.clone())
        .ok_or_else(|| "Zlyhala transformácia dát do RGB štruktúry.".to_string())?;

    let (nova_sirka, nova_vyska) =
        tli_overlay_core::zmensenie::nove_rozmery(s.sirka, s.vyska, max_dlzka);

    let upraveny = image::imageops::resize(
        &raw_obrazok,
        nova_sirka,
        nova_vyska,
        image::imageops::FilterType::Triangle,
    );

    let mut jpeg_data = Vec::new();
    let mut cursor = Cursor::new(&mut jpeg_data);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, kvalita);

    encoder
        .encode(
            &upraveny,
            nova_sirka,
            nova_vyska,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("Chyba pri vytváraní JPEGu: {}", e))?;

    Ok(jpeg_data)
}

/// Zisti, ci snimka je prakticky cierny obraz.
/// DOVOD: Sluzi na skoru identifikaciu exclusive fullscreen okna pred zacatim draheho procesu zmensovania a posielania.
pub fn je_cierna(s: &Snimka) -> bool {
    tli_overlay_core::obraz::je_takmer_cierny(&s.rgb, 8)
}
