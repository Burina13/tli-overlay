/* ikony.js — malé SVG ikony do panela s dôvodom (TLI 8).
 *
 * PREČO VLASTNÉ SVG A NIE SŤAHOVANIE Z `tli-web`: overlay beží cez obrazovku
 * hry a panel sa musí vykresliť OKAMŽITE aj vtedy, keď server práve
 * nedosiahnuteľný je. Preto je tu ten istý vizuálny jazyk ako na webe
 * (`stacks/games/tli-web/static/kruh.js`), ale ako lokálny, offline zdroj —
 * žiadny sieťový request navyše a žiadne čakanie na obrázok.
 *
 * Klasický skript (nie ES modul) — `index.html` ho načíta pred `overlay.js`,
 * rovnako ako ostatné súbory tohto UI.
 */
(function (globalny) {
    "use strict";

    // Farby rarít — tie isté ako na webe (tier 0..5).
    var TIER_FARBA = ["#9ca3af", "#60a5fa", "#fde047", "#c084fc", "#f87171", "#ff6ad5"];
    var TIER_SK = ["Normálny", "Magický", "Vzácny", "Legendárny", "Najvyšší", "Dúhový"];
    var BEAST_SK = { mammoth: "Mamut", lion: "Lev", wolf: "Vlk", rat: "Potkan" };

    // Portréty štyroch väzňov — zjednodušené oproti webu (panel ich kreslí
    // ~28 px, detail by sa aj tak stratil), ale rovnaká silueta.
    var VAZEN = {
        mammoth: '<path d="M34 20C16 20 4 33 4 49c0 13 7 23 16 27 2 1 5 0 5-2 0-18 3-36 11-51 1-2 0-3-2-3zM66 20c18 0 30 13 30 29 0 13-7 23-16 27-2 1-5 0-5-2 0-18-3-36-11-51-1-2 0-3 2-3z" opacity="0.55"/>' +
                 '<path d="M50 10c-12 0-19 9-20 20-1 10 1 18 4 24 3 5 9 8 16 8s13-3 16-8c3-6 5-14 4-24-1-11-8-20-20-20z"/>' +
                 '<path d="M45 50c2 15 1 27-1 36-1 5 2 8 6 8s7-3 6-8c-2-9-3-21-1-36z"/>' +
                 '<path d="M39 52C22 57 6 69 10 82c2 6 9 5 9-1 0-10 9-19 22-22zM61 52c17 5 33 17 29 30-2 6-9 5-9-1 0-10-9-19-22-22z" fill="#efe6d2"/>',
        lion: '<circle cx="50" cy="50" r="44" opacity="0.45"/>' +
              '<circle cx="32" cy="27" r="9"/><circle cx="68" cy="27" r="9"/>' +
              '<circle cx="50" cy="50" r="27"/>' +
              '<path d="M50 56c-9 0-15 4-15 9s7 9 15 9 15-4 15-9-6-9-15-9z" opacity="0.5"/>',
        wolf: '<path d="M24 8l6 30-12-6zM76 8l-6 30 12-6z" opacity="0.6"/>' +
              '<path d="M26 14l10 26c4-3 9-5 14-5s10 2 14 5l10-26 4 36c0 14-11 28-28 38C33 78 22 64 22 50z"/>' +
              '<path d="M50 62c-5 0-9 3-9 6 0 5 4 12 9 18 5-6 9-13 9-18 0-3-4-6-9-6z" opacity="0.5"/>',
        rat: '<circle cx="25" cy="30" r="17"/><circle cx="75" cy="30" r="17"/>' +
             '<circle cx="25" cy="30" r="9" opacity="0.5"/><circle cx="75" cy="30" r="9" opacity="0.5"/>' +
             '<path d="M50 18c15 0 24 11 24 24 0 8-3 15-7 21-4 7-10 15-17 21-7-6-13-14-17-21-4-6-7-13-7-21 0-13 9-24 24-24z"/>' +
             '<path d="M34 66L14 62M34 70L15 74M66 66l20-4M66 70l19 4" stroke="currentColor" stroke-width="2" fill="none" opacity="0.7"/>',
    };

    // Ikony cieľovej akcie — kreslené v mriežke 24×24, `currentColor`.
    var AKCIA = {
        patrol: '<path d="M4 12h12m0 0-4-4m4 4-4 4" stroke="currentColor" stroke-width="2.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/><circle cx="20" cy="12" r="2.5"/>',
        devour: '<path d="M4 6c4 3 12 3 16 0l-2 6H6zM6 14h12l-2 5c-3 2-5 2-8 0z" /><path d="M9 12v3M12 12v3M15 12v3" stroke="#1a1a1a" stroke-width="1.2"/>',
        swap: '<path d="M5 9h11l-3-3m3 3-3 3M19 15H8l3-3m-3 3 3 3" stroke="currentColor" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
        zvol_nastroj: '<path d="M14.5 3a5 5 0 0 0-4.3 7.5L3 17.7 6.3 21l7.2-7.2A5 5 0 1 0 14.5 3z"/>',
        zvol_dar: '<path d="M3 9h18v3H3zM4 13h16v8H4zM11 4a2.5 2.5 0 1 0-3 3h3zm2 0a2.5 2.5 0 1 1 3 3h-3zM11 7h2v14h-2z"/>',
        event_prijat: '<path d="m4 13 5 5L20 6" stroke="currentColor" stroke-width="2.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
        event_odmietnut: '<path d="M6 6l12 12M18 6 6 18" stroke="currentColor" stroke-width="2.6" fill="none" stroke-linecap="round"/>',
        koniec_dna: '<circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="2.2" fill="none"/><path d="M12 7v5l3 2" stroke="currentColor" stroke-width="2.2" fill="none" stroke-linecap="round"/>',
        // Frozen Canvas: položenie paintu na plátno.
        polozit: '<path d="M4 4h7v7H4zM13 13h7v7h-7z" opacity="0.5"/><path d="M11 7h7l-2.5-2.5M18 7l-2.5 2.5" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
    };

    function svg(vnutro, viewBox, trieda, popis) {
        return '<svg class="' + trieda + '" viewBox="' + viewBox + '" role="img" aria-label="' +
            popis.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/"/g, "&quot;") +
            '" fill="currentColor">' + vnutro + "</svg>";
    }

    /** Portrét väzňa (beast + tier) alebo null, keď je väzeň neznámy. */
    function ikonaVazna(inmate) {
        if (!inmate || !VAZEN[inmate.beast]) return null;
        var tier = typeof inmate.tier === "number" ? inmate.tier : 0;
        var popis = (BEAST_SK[inmate.beast] || inmate.beast) + " · " + (TIER_SK[tier] || "?");
        var el = document.createElement("span");
        el.className = "panel-ikona panel-ikona-vazen";
        el.style.color = TIER_FARBA[tier] || TIER_FARBA[0];
        el.title = popis;
        el.innerHTML = svg(VAZEN[inmate.beast], "0 0 100 100", "ikona-svg", popis);
        return el;
    }

    /** Štvorček paintu (Frozen Canvas) s jeho levelom. */
    function ikonaPaintu(paint) {
        if (!paint || !paint.typ) return null;
        var popis = paint.typ + (paint.level ? " Lv" + paint.level : "");
        var el = document.createElement("span");
        el.className = "panel-ikona panel-ikona-paint" +
            (paint.kategoria === "gameplay" ? " paint-gameplay" : " paint-monster");
        el.title = popis;
        // Farba sa odvodí z názvu typu — stabilne (rovnaký typ = rovnaká farba),
        // ale NIE je to herná farba paintu; hru sme na ne nikdy nemerali.
        var sucet = 0;
        for (var i = 0; i < paint.typ.length; i++) sucet = (sucet * 31 + paint.typ.charCodeAt(i)) % 360;
        el.style.setProperty("--paint-odtien", String(sucet));
        var text = document.createElement("span");
        text.className = "paint-level";
        text.textContent = paint.level ? "Lv" + paint.level : "?";
        el.appendChild(text);
        return el;
    }

    /** Malá ikona cieľovej akcie (patrola, požretie, nástroj…). */
    function ikonaAkcie(typ, popis) {
        var kresba = AKCIA[typ];
        if (!kresba) return null;
        var el = document.createElement("span");
        el.className = "panel-ikona panel-ikona-akcia";
        el.title = popis || typ;
        el.innerHTML = svg(kresba, "0 0 24 24", "ikona-svg", popis || typ);
        return el;
    }

    globalny.TLIIkony = {
        ikonaVazna: ikonaVazna,
        ikonaPaintu: ikonaPaintu,
        ikonaAkcie: ikonaAkcie,
        TIER_FARBA: TIER_FARBA,
        TIER_SK: TIER_SK,
        BEAST_SK: BEAST_SK,
    };
})(window);
