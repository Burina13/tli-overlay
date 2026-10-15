// Náhľad toastu presne so znením zo zadania (TLI 11d, bod 3) — rovnaký vzor
// ako `nahlad-canvas.js`: reálne `pripomienka.js` sa zaregistruje na udalosť
// `pripomienka-obsah` cez `nahlad-stub.js`, tento súbor ten istý poslucháč
// po načítaní zavolá s ukážkovým payloadom.
const posli = () => {
    const f = (window.__nahlad_poslucháči || {})["pripomienka-obsah"];
    if (f) {
        f({
            payload: {
                text: "⚠️ Zapni v hre Settings → Other Settings → Enable Log — inak sa táto session netrackuje",
                trieda: "varovanie",
            },
        });
    }
};
if (document.readyState === "complete") posli();
else window.addEventListener("load", posli);
