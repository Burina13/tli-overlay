# Archív — záložky „Solvery" a „Analýza" (TLI 65, 20.10.2026)

**Kedy:** 20.10.2026, úloha `inbox/2026-10-20-0931-mcp.md` (TLI 65), Richard cez MCP.

**Odkiaľ:**
- `ui/hlavne.html` — sekcia `#panel-solvery` (riadky 248–267) a `#panel-analyza`
  (riadky 269–370, vrátane podsekcie „Taktiky"), plus dve tlačidlá záložiek v `<nav class="zalozky">`.
- `ui/hlavne.js` — blok „Analýza (TLI 13)" + „Taktiky (TLI 13b)" (riadky 1490–1717)
  a obsluha `btn-taktiky-pomenuj` (riadky 1517–1538 po prvom výreze).

**Prečo:** Richard 20.10.2026 09:30: „Ja nepotrebujem analyzer v appke!" Appka má mať
podľa jeho zadania len **Tracker, Inventory, Economy, Nastavenia** a overlaye
(Tartarus Ctrl+Q, Canvas Ctrl+W, ceny F10, loot overlay, minimapa). Záložka Solvery
bola len prehľad dvoch klávesových skratiek (tie sú aj v Nastaveniach), záložka
Analýza bol prehliadač histórie behov s exportmi a štatistikou taktík.

**Čo sa NEodstránilo a prečo:**
- **Rust backend ostáva** — príkazy `analyza_vyhladaj`, `analyza_taktiky`,
  `analyza_export_csv/json`, `taktika_pomenuj_alebo_zluc` a `core::tracker::taktiky`
  sú v kóde ďalej. Sú to funkcie nad tou istou DB, akú používa Tracker a Economy;
  vytrhávať ich kvôli zrušenému UI by bolo riziko bez viditeľného prínosu.
  Appka ich po tejto zmene nevolá z ničoho.
- **Záložka Guide ostáva** — v Richardovom zozname nie je, ale nie je to ani
  simulátor, ani analyzer, ani tréner; je to offline sprievodca (Erika 1).
  Čaká sa na jeho slovo, či ju vyhodiť tiež.

**Rollback:** vrátiť fragmenty z tohto priečinka späť na uvedené miesta
(`hlavne.html.panel-solvery.frag`, `hlavne.html.panel-analyza.frag`,
`hlavne.js.analyza-a-taktiky.frag`), vrátiť dve tlačidlá do `<nav class="zalozky">`
a dopísať späť `await analyzaHladaj();` + `await taktikyNacitaj();` do štartu.
Alebo jednoducho `git revert` commitu TLI 65 v tomto repozitári.
