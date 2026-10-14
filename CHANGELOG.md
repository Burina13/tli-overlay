# Zoznam zmien

Projekt využíva sémantické verzovanie (Semantic Versioning). Verzie sú označované v tvare `<major>.<minor>.<patch>` a git tagy na releasy v tvare `tli-v<verzia>`.

## 0.2.0 — 15.10.2026

Frozen Canvas (TLI 4b) — druhá, nezávislá mechanika vedľa Tartarus.

- Nová klávesová skratka (predvolene F9, nastaviteľná vedľa F8) pre analýzu obrazovky
  Frozen Canvas — iný endpoint servera (`?modul=canvas`), iný tvar odpovede.
- Overlay vykresľuje SEKVENCIU krokov ČO→KAM: pre každý krok dva zelené rámiky (paint
  v ruke aj cieľová bunka na plátne) s rovnakým číslom v rohu; prvý krok plný a hrubý,
  ďalšie tenšie a priehľadnejšie, nech je poradie jasné na prvý pohľad.
- Panel s dôvodom zobrazuje prvý krok hlavne, ďalšie kroky ako očíslovaný zoznam pod ním —
  rovnaká poloha ako pri Tartarus, nikdy sa neprekrýva s plátnom ani s rukou.
- Solver na serveri rieši VÝHRADNE smer miešania "adjacent" (jediný s overeným presným
  pravidlom z reálnych screenshotov) — iné smery zatiaľ vedome odmietne, kým sa dáta
  nepotvrdia (Richard zbiera dáta 16.10.2026).
- Nastavenia appky majú druhé pole pre hotkey Frozen Canvas, oddelené od Tartarus.

## 0.1.0 — 14.10.2026

Prvé vydanie aplikácie Tvoj Tatko TLI Overlay pre sezónnu mechaniku Tartarus (SS14).

- Pridaná automatická detekcia a robenie snímky okna (GDI BitBlt) klávesovou skratkou F8.
- Pridaná HTTP integrácia s API pre získavanie ťahov (`ureq`).
- Implementované samostatné, priehľadné overlay okno bez rámu, ktoré je priechodné pre myš a zobrazuje odporúčaný ťah, dôvod po slovensky, percentuálnu istotu a 2 alternatívne ťahy.
- Pridané vizuálne zvýraznenie cieľového políčka v hernej doske.
- Podpora pre monitory s upraveným DPI (100 %, 125 %, 150 %) a logika pre výber správneho monitora podľa toho, kde je umiestnené okno hry (rieši problémy na multi-monitor setupoch).
- Pridaný slovenský preklad pre chybové stavy: 401 (zlý kľúč), 403 (chýba rola), 422 (doska nenájdená), 429 (rate limit 30 za minútu).
- Podpora dynamického a bezpečného používania klávesu Esc, ktorý overlay zatvára bez toho, aby trvale narušoval herné skratky (kradnutie inputu hre).
- Implementovaná kontrola exkluzívneho fullscreenu (detekcia čiernej snímky) so zobrazením varovania s radou pre zmenu nastavení hry.
- Implementované statické upozorňovanie na aktualizácie formou prečítania JSON manifestu, bez priamej inštalácie aktualizácie aplikáciou.
- Nasadené prísne bezpečnostné CI kontroly, ktoré v zdrojovom kóde a závislostiach strážia absenciu akýchkoľvek hookov a memory read/write funkcií, čo zaisťuje plný súlad s podmienkami hry.
- Pevne fixovaný WiX UpgradeCode inštalátora v systéme (zabránenie chybného prepisovania od cudzích repozitárov).
