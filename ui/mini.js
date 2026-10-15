const { invoke } = window.__TAURI__.core;

// Prevedie milisekundy na citatelny text: "3 m 05 s" alebo "42 s".
// Pre zapornu hodnotu alebo nulu vracia pomlcku.
function cas(ms) {
    if (typeof ms !== "number" || ms <= 0) {
        return "—";
    }
    
    const celkoveSekundy = Math.floor(ms / 1000);
    const minuty = Math.floor(celkoveSekundy / 60);
    const sekundy = celkoveSekundy % 60;

    if (minuty > 0) {
        // Zabezpecenie formatu dvoch cislic pre sekundy (napr. 05 s)
        const sekundyFormat = sekundy < 10 ? "0" + sekundy : sekundy;
        return minuty + " m " + sekundyFormat + " s";
    }
    
    return sekundy + " s";
}

// Formatuje hodnotu na pozadovany FE format.
// Ziska hodnoty nad 100 bez desatinnych miest, do 100 s dvomi.
function fe(h) {
    if (h === null || h === undefined || !isFinite(h)) {
        return "—";
    }
    
    if (Math.abs(h) >= 100) {
        return Math.round(h) + " FE";
    }
    
    return h.toFixed(2) + " FE";
}

// Ziska cisty nazov zlozky z celej cesty zadanej hrou.
// Hľadá sa posledný kus cesty a následne sa odstráni koncovka.
function analyzujCestuMapy(cesta) {
    if (!cesta) return "Neznáma mapa";
    
    // Ziskanie poslednej casti cesty
    const casti = cesta.split("/");
    const subor = casti[casti.length - 1];
    
    // Odstranenie vsetkeho za poslednou bodkou
    const indexBodky = subor.lastIndexOf(".");
    if (indexBodky > 0) {
        return subor.substring(0, indexBodky);
    }
    
    return subor;
}

// Stiahne udaje z backandu a prekresli panel.
async function obnov() {
    try {
        const p = await invoke("tracker_prehlad");
        
        const panel = document.getElementById("mini");
        const mapaEl = document.getElementById("mini-mapa");
        const ziskEl = document.getElementById("mini-zisk");
        const casEl = document.getElementById("mini-cas");
        const zahEl = document.getElementById("mini-zah");

        // Ked nebezi mapa
        if (!p || !p.zivo) {
            panel.classList.add("necinny");
            mapaEl.textContent = "Žiadna mapa nebeží";
            ziskEl.textContent = "—";
            ziskEl.className = "cislo"; // Zrusime farebne triedy
            casEl.textContent = "—";
            zahEl.textContent = "—";
            return;
        }

        // Ked mapa bezi, aktivujeme vzhlad a vyplnime data
        panel.classList.remove("necinny");
        
        const zivo = p.zivo;
        mapaEl.textContent = analyzujCestuMapy(zivo.zona);
        
        // Nastavenie cisteho zisku vratane farby textu
        ziskEl.textContent = fe(zivo.cisty_zisk);
        if (zivo.cisty_zisk > 0) {
            ziskEl.className = "cislo zisk-kladny";
        } else if (zivo.cisty_zisk < 0) {
            ziskEl.className = "cislo zisk-zaporny";
        } else {
            ziskEl.className = "cislo";
        }

        // Nastavenie ziveho casu
        casEl.textContent = cas(zivo.trvanie_ms);

        // Vypocet FE za hodinu. Pripona "FE" sa tu UZ NEPISE — popisok nad
        // cislom je "FE/h" a tri hodnoty s priponou sa do 260 px nezmestia.
        if (zivo.trvanie_ms > 0) {
            const ziskZaHodinu = (zivo.cisty_zisk * 3600000) / zivo.trvanie_ms;
            zahEl.textContent = fe(ziskZaHodinu).replace(" FE", "");
        } else {
            zahEl.textContent = "—";
        }

    } catch (chyba) {
        // Zachytavame a logujeme chyby bez padu skriptu, aby sa dalsi interval vykonal
        console.error("Chyba pri stahovani dat do mini-panela:", chyba);
    }
}

// Spustenie hned po natiahnuti stranky a cyklenie kazde 2 sekundy — to je
// zalozka, zive citanie na pozadi (TLI 11c) posle udalost hned po novych
// riadkoch a tá nizsie spusti prekreslenie okamzite.
document.addEventListener("DOMContentLoaded", () => {
    obnov();
    setInterval(obnov, 2000);
    if (window.__TAURI__.event) {
        window.__TAURI__.event.listen("tracker-zmena", () => obnov());
    }
});
