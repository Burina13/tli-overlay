#!/usr/bin/env python3
"""Vyrobí náhľadové stránky pre CI z OSTRÝCH súborov UI.

Prečo sa generujú a nie ležia v repe: do 0.8.0 tu bola ručná kópia
`ui/nahlad-hlavne.html` a rozišla sa s originálom hneď, ako pribudla záložka
Guide — snímka z CI potom ukazovala staré okno, hoci appka bola nová.

Jediný rozdiel oproti ostrej appke je podstrčený `nahlad-stub.js` namiesto
Tauri (`window.__TAURI__.core.invoke` vracia ukážkové dáta) a vopred prepnutá
aktívna záložka.

**Záložka sa prepína v HTML, nie parametrom `?tab=` v JS.** Na Windows runneri
sa `?tab=` neprejavilo — všetky štyri snímky z 0.9.0 ukazovali Tracker, hoci
`?tema=` z tej istej URL fungovalo. Takto snímka na JS nezávisí vôbec.
"""
import pathlib
import re

ZALOZKY = ["panel-tracker", "panel-solvery", "panel-analyza", "panel-guide", "panel-nastavenia"]
TU = pathlib.Path(__file__).resolve().parent


def vyrob(zdroj: str, ciel: str, css: str, panel: str = "") -> None:
    t = (TU / zdroj).read_text(encoding="utf-8")
    assert t.count(css) == 1, f"v {zdroj} sa zmenil odkaz na CSS"
    t = t.replace(css, '<script src="nahlad-stub.js"></script>\n    ' + css, 1)
    t = t.replace("<title>Tvoj Tatko TLI Overlay</title>",
                  "<title>Náhľad hlavného okna</title>", 1)
    if panel:
        t = re.sub(r'class="zalozka aktivna"', 'class="zalozka"', t)
        t = t.replace(f'class="zalozka" data-panel="{panel}"',
                      f'class="zalozka aktivna" data-panel="{panel}"', 1)
        t = re.sub(r'(<section id="panel-[a-z]+" class="panel)(">)', r'\1 skryte\2', t)
        t = t.replace(f'<section id="{panel}" class="panel skryte">',
                      f'<section id="{panel}" class="panel">', 1)
        # Keby sa v `hlavne.html` zmenili triedy, nech to padne TU a nie ticho
        # v podobe štyroch rovnakých snímok.
        assert f'class="zalozka aktivna" data-panel="{panel}"' in t, panel
        assert f'<section id="{panel}" class="panel">' in t, panel
    (TU / ciel).write_text(t, encoding="utf-8")
    print("vygenerovane:", ciel)


def main() -> None:
    for panel in ZALOZKY:
        vyrob("hlavne.html", f"nahlad-hlavne-{panel}.html",
              '<link rel="stylesheet" href="hlavne.css">', panel)
    vyrob("mini.html", "nahlad-mini.html", '<link rel="stylesheet" href="mini.css">')


if __name__ == "__main__":
    main()
