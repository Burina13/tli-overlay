        <section id="panel-analyza" class="panel skryte">
            <div class="riadok-akcii">
                <h2>Analýza</h2>
                <button type="button" class="btn-vedlajsi" id="btn-analyza-stare-logy">Načítať staré logy</button>
            </div>
            <p class="vysvetlivka" id="analyza-import-stav"></p>

            <div class="riadok-prepinacov analyza-filter">
                <label>
                    Mapa (časť cesty)
                    <input type="text" id="analyza-f-mapa" placeholder="napr. XueYuRongLu">
                </label>
                <label>
                    Minimálny čistý zisk (FE)
                    <input type="number" id="analyza-f-min-zisk" placeholder="napr. 50">
                </label>
                <button type="button" class="btn-hlavny" id="btn-analyza-hladaj">Hľadať</button>
            </div>

            <div class="riadok-akcii">
                <h3 class="nadpis-tabulky">Výsledky</h3>
                <span>
                    <button type="button" class="btn-vedlajsi" id="btn-analyza-csv">Export CSV</button>
                    <button type="button" class="btn-vedlajsi" id="btn-analyza-json">Export JSON</button>
                </span>
            </div>
            <table class="tabulka" id="tabulka-analyza">
                <thead>
                    <tr>
                        <th>Mapa</th>
                        <th>Chart a kompasy</th>
                        <th>Začiatok</th>
                        <th class="cislo">Trvanie</th>
                        <th class="cislo">Čistý zisk</th>
                        <th class="cislo">FE/h</th>
                        <th class="cislo">Monštrá</th>
                        <th class="cislo">Rares/boss</th>
                    </tr>
                </thead>
                <tbody></tbody>
            </table>
            <p class="prazdne" id="analyza-prazdne">Zatiaľ nič — klikni „Hľadať" alebo najprv „Načítať staré logy".</p>

            <div id="analyza-detail" class="skryte">
                <h3 class="nadpis-tabulky">Monštrá podľa rarity — <span id="analyza-detail-mapa"></span></h3>
                <table class="tabulka" id="tabulka-analyza-detail">
                    <thead><tr><th>Rarita</th><th class="cislo">Počet</th></tr></thead>
                    <tbody></tbody>
                </table>
            </div>

            <div id="analyza-taktiky-sekcia">
                <div class="riadok-akcii">
                    <h3 class="nadpis-tabulky">Taktiky</h3>
                    <span>
                        <button type="button" class="btn-vedlajsi" id="btn-taktiky-obnov">Prepočítať</button>
                        <button type="button" class="btn-vedlajsi" id="btn-taktiky-csv">Export CSV</button>
                        <button type="button" class="btn-vedlajsi" id="btn-taktiky-json">Export JSON</button>
                    </span>
                </div>
                <p class="vysvetlivka">
                    Taktika = podpis setupu behu (chart a kompasy, mapa a mód, modifikátory
                    mapy, build). Zoradené podľa priemerného FE/h — prvý riadok je „najlepšia
                    taktika za hodinu". Vyber riadky a pomenuj ich (viac vybraných = zlúčenie
                    do jednej taktiky).
                </p>
                <p class="vysvetlivka" id="taktiky-odporucanie"></p>

                <table class="tabulka" id="tabulka-taktiky">
                    <thead>
                        <tr>
                            <th></th>
                            <th>Taktika</th>
                            <th class="cislo">Behov</th>
                            <th class="cislo">Ocenených</th>
                            <th class="cislo">Priemer zisk</th>
                            <th class="cislo">Medián</th>
                            <th class="cislo">Min / Max</th>
                            <th class="cislo">p10 / p90</th>
                            <th class="cislo">Odchýlka</th>
                            <th class="cislo">FE/h</th>
                            <th class="cislo">Náklad</th>
                            <th class="cislo">ROI</th>
                            <th class="cislo">Zisk/riziko</th>
                        </tr>
                    </thead>
                    <tbody></tbody>
                </table>
                <p class="prazdne" id="taktiky-prazdne">Zatiaľ žiadne behy na zoskupenie do taktík.</p>

                <div class="riadok-prepinacov">
                    <label>
                        Názov pre vybrané taktiky
                        <input type="text" id="taktiky-f-nazov" placeholder="napr. Steel Forge farm T7">
                    </label>
                    <button type="button" class="btn-hlavny" id="btn-taktiky-pomenuj">Pomenovať / zlúčiť vybrané</button>
                </div>
                <p class="vysvetlivka" id="taktiky-pomenovanie-stav"></p>

                <div id="taktiky-navrhy"></div>
            </div>
        </section>
