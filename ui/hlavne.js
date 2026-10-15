const { invoke } = window.__TAURI__.core;

const zalozky = document.querySelectorAll(".zalozka");
const panely = document.querySelectorAll(".panel");

function prepni(tlacidlo) {
    zalozky.forEach((t) => t.classList.remove("aktivna"));
    panely.forEach((p) => p.classList.add("skryte"));
    tlacidlo.classList.add("aktivna");
    document.getElementById(tlacidlo.dataset.panel).classList.remove("skryte");
}

zalozky.forEach((tlacidlo) => {
    tlacidlo.addEventListener("click", () => prepni(tlacidlo));
});

// `?tab=panel-solvery` otvorí rovno danú záložku — slúži len CI náhľadu
// (screenshot každej záložky zvlášť), appka sama o sebe URL parametre nepoužíva.
const ziadanaZalozka = new URLSearchParams(window.location.search).get("tab");
if (ziadanaZalozka) {
    const tlacidlo = document.querySelector(`.zalozka[data-panel="${ziadanaZalozka}"]`);
    if (tlacidlo) {
        prepni(tlacidlo);
    }
}

document.getElementById("btn-otvor-nastavenia").addEventListener("click", async () => {
    try {
        await invoke("otvor_nastavenia");
    } catch (e) {
        console.error(e);
    }
});

document.addEventListener("DOMContentLoaded", async () => {
    try {
        const v = await invoke("verzia");
        document.getElementById("verzia-text").textContent = `v${v}`;
    } catch (e) {
        console.error(e);
    }

    try {
        const nastavenia = await invoke("nacitaj_nastavenia");
        document.getElementById("hotkey-tartarus").textContent = nastavenia.hotkey || "F8";
        document.getElementById("hotkey-canvas").textContent = nastavenia.hotkey_canvas || "F9";
    } catch (e) {
        console.error(e);
    }
});
