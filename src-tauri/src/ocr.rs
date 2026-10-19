//! Rozpoznanie textu zo snímky obrazovky cez `Windows::Media::Ocr` (TLI 59b
//! časť 2, `2026-10-20-0005-agent.md`, bod 1) — jediný zdroj textu cieľa
//! miešania a „Current Max Paint Level" pre Frozen Canvas.
//!
//! **Tento súbor sa na CT 111 (Linux) NEDÁ ani skompilovať, nieto ešte
//! spustiť** — je celý za `#[cfg(windows)]` (`main.rs`), lebo `OcrEngine` je
//! WinRT API dostupné len na Windowse. To je OČAKÁVANÉ: parsovanie textu,
//! ktoré sa overiť DÁ (vzory ako „Fill a Adjacent Cell … (0/3)“ a „Current
//! Max Paint Level7“), je naschvál v čistom Rust module
//! `tli_overlay_core::canvas_ocr` (žiadne WinRT), s vlastnými testami
//! bežiacimi aj tu. Tento súbor je len tenká vrstva OKOLO toho parsovania:
//! vezme pixely, zavolá Windows OCR, vráti surový text ďalej.
//!
//! API overené priamo zo zdroja kraty `windows` 0.58.0 (vygenerované
//! bindingy, `src/Windows/Media/Ocr/mod.rs` a
//! `src/Windows/Graphics/Imaging/mod.rs`) — nie z pamäte — práve kvôli
//! poučeniu z TLI 40 (dva rôzne typy tej istej kraty). Napriek tomu je toto
//! JEDINÉ miesto v appke, ktoré Windows CI (`Testy` workflow) musí overiť
//! skutočným kompilátorom skôr, než sa to považuje za hotové.
//!
//! WinRT async (`RecognizeAsync`) sa čaká BLOKUJÚCO (`.get()`) — `windows-rs`
//! implementuje `IAsyncOperation::get()` ako vlastnú blokujúcu čakaciu
//! slučku (nepotrebuje tokio ani iný executor). Appka toto volá len z
//! vlastného vlákna zberu/analýzy (`zber_canvas.rs`, `klient.rs`), nikdy
//! z UI vlákna Tauri — rovnaký vzor, aký appka už používa pre `ureq`
//! (synchrónne, blokujúce HTTP volania mimo hlavného vlákna).

use tli_overlay_core::suradnice::Pole;
use windows::core::Result as WinResult;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, IBuffer};

use crate::snimka::Snimka;

/// Rozpozná VŠETOK text v celej snímke a vráti ho ako jeden reťazec (riadky
/// OCR enginu spojené `\n`). Úmyselne sa (zatiaľ) neOCRuje len výrez okolo
/// panelu cieľa/max levelu — tá oblasť ešte nie je zmeraná na reálnych
/// snímkach (na rozdiel od mriežky platna/ruky v `canvas_rozlozenie.rs`,
/// ktorá zmeraná JE). `tli_overlay_core::canvas_ocr::parsuj_ciel_text`/
/// `parsuj_max_level` si hľadaný vzor nájdu kdekoľvek v texte, takže OCR
/// celej snímky je funkčne správne, len drahšie — keď raz bude mať appka
/// nameraný `Pole` oblasti panelu (rovnakým postupom ako TLI 59b časť 1),
/// stačí pred volaním `rozpoznaj_text` zavolať `orez_box01` a OCRovať len
/// výrez (rýchlejšie aj presnejšie, menej cudzieho textu okolo).
pub fn rozpoznaj_text(s: &Snimka) -> Result<String, String> {
    let bitmapa = rgb_na_software_bitmap(&s.rgb, s.sirka, s.vyska)
        .map_err(|e| format!("Zostavenie SoftwareBitmap zlyhalo: {e}"))?;

    rozpoznaj_bitmapu(&bitmapa)
}

/// Ako `rozpoznaj_text`, ale len nad výrezom `oblast` (Box01 súradnice voči
/// celej snímke) — použi, keď je geometria panelu už zmeraná.
pub fn rozpoznaj_text_oblasti(s: &Snimka, oblast: Pole) -> Result<String, String> {
    let (vyrez, sirka, vyska) = tli_overlay_core::obraz::orez_box01(&s.rgb, s.sirka, s.vyska, oblast)
        .ok_or_else(|| "Oblasť na OCR leží mimo snímky alebo je nulová.".to_string())?;

    let bitmapa = rgb_na_software_bitmap(&vyrez, sirka, vyska)
        .map_err(|e| format!("Zostavenie SoftwareBitmap zlyhalo: {e}"))?;

    rozpoznaj_bitmapu(&bitmapa)
}

fn rozpoznaj_bitmapu(bitmapa: &SoftwareBitmap) -> Result<String, String> {
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| format!("OcrEngine sa nepodarilo vytvoriť (chýba jazykový balík OCR vo Windows?): {e}"))?;

    let vysledok = engine
        .RecognizeAsync(bitmapa)
        .map_err(|e| format!("RecognizeAsync sa nepodarilo spustiť: {e}"))?
        .get()
        .map_err(|e| format!("Čakanie na výsledok OCR zlyhalo: {e}"))?;

    let text = vysledok
        .Text()
        .map_err(|e| format!("OCR výsledok sa nepodarilo prečítať ako text: {e}"))?;

    Ok(text.to_string())
}

/// Surové RGB (3 bajty/pixel, zhora nadol) skonvertuje na `SoftwareBitmap`
/// vo formáte BGRA8, ktorý `OcrEngine::RecognizeAsync` vyžaduje. Alfa kanál
/// sa dopĺňa ako neprieadny `255` a ignoruje (`BitmapAlphaMode::Ignore`) —
/// OCR farbu priehľadnosti nepotrebuje, snímka z obrazovky žiadnu aj tak
/// nemá.
///
/// Jediná cesta, ako `windows-rs` dostane surové bajty do WinRT bufferu, je
/// cez `DataWriter` (zapíš bajty → `DetachBuffer()` vráti `IBuffer`) —
/// `SoftwareBitmap::CreateCopyWithAlphaFromBuffer` inú vstupnú cestu nemá.
fn rgb_na_software_bitmap(rgb: &[u8], sirka: u32, vyska: u32) -> WinResult<SoftwareBitmap> {
    let mut bgra = Vec::with_capacity(rgb.len() / 3 * 4);
    for pixel in rgb.chunks_exact(3) {
        bgra.push(pixel[2]); // B
        bgra.push(pixel[1]); // G
        bgra.push(pixel[0]); // R
        bgra.push(255); // A
    }

    let writer = DataWriter::new()?;
    writer.WriteBytes(&bgra)?;
    let buffer: IBuffer = writer.DetachBuffer()?;

    SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        sirka as i32,
        vyska as i32,
        BitmapAlphaMode::Ignore,
    )
}
