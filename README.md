# Czech Swedish Crossword Generator (Švédské křížovky)

[![Release](https://img.shields.io/github/v/release/Xemixo/czech-swedish-crossword-generator?color=indigo)](https://github.com/Xemixo/czech-swedish-crossword-generator/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange.svg)](https://www.rust-lang.org/)
[![GitHub Pages](https://img.shields.io/badge/Web%20Demo-Online-success)](https://xemixo.github.io/czech-swedish-crossword-generator/)

> **Vysoce výkonný procedurální generátor a CSP řešitel švédských křížovek v Rustu s podporou tajenek, CwdStudio XML a interaktivního webového luštění.**

Vyzkoušejte si luštění online: **[https://xemixo.github.io/czech-swedish-crossword-generator/](https://xemixo.github.io/czech-swedish-crossword-generator/)**

---

## 🌟 Klíčové vlastnosti

- ⚡ **Bleskový CSP řešitel**: Algoritmus *Constraint Satisfaction Problem* s propagací omezení **AC-3** a heuristikou **MRV** (Minimum Remaining Values) vyřeší i mřížku 15×15 do 20 milisekund.
- 🔤 **Plná podpora češtiny a spřežky CH**: Diakritika (`Á`, `Č`, `Ď`, `É`, `Ě`, `Í`, `Ň`, `Ó`, `Ř`, `Š`, `Ť`, `Ú`, `Ů`, `Ý`, `Ž`) a správné zacházení se spřežkou **CH** jako s jediným písmenem v křížovkářské síti.
- 🎯 **Vkládání vlastních tajenek**: Možnost zadat libovolné tajenkové slovo. Generátor automaticky najde vhodnou sekvenci políček a předvyplní tajenku do mřížky.
- 📚 **Kvalitní slovník s obtížností**: Obsahuje přes **19 500 hesel** s definicemi, rozdělených do úrovní:
  - `easy` (Snadná) — vhodné pro začátečníky a děti
  - `medium` (Střední) — klasická křížovkářská obtížnost
  - `hard` (Těžká) — náročné definice
  - `all / mix` — kompletní slovní zásoba
- 📐 **Procedurální generátor mřížek**: Automatické rozvržení políček a šipek pro libovolné rozměry od **4×4 do 30×30**.
- 📄 **Duální export**:
  - **Interaktivní HTML**: Moderní responzivní stránka pro luštění v prohlížeči (kontrola chyb, odhalení řešení, počítadlo tajenky, tiskový styl pro A4).
  - **CwdStudio XML**: Plně kompatibilní formát pro křížovkářský software CwdStudio.

---

## 🚀 Rychlý start (Bez instalace)

### 🪟 Windows (x64)
1. Stáhněte si **[czech-swedish-crossword-v1.0.0-windows-x64.zip](https://github.com/Xemixo/czech-swedish-crossword-generator/releases/download/v1.0.0/czech-swedish-crossword-v1.0.0-windows-x64.zip)**.
2. Rozbalte archiv do libovolné složky.
3. Dvakrát klikněte na **`Spustit_krizovku.bat`**.
4. Vyberte rozměr, obtížnost a případnou tajenku — po vygenerování se křížovka automaticky otevře ve vašem webovém prohlížeči.

### 🍎 macOS / 🐧 Linux
1. Stáhněte archiv **[czech-swedish-crossword-v1.0.0-macos.zip](https://github.com/Xemixo/czech-swedish-crossword-generator/releases/download/v1.0.0/czech-swedish-crossword-v1.0.0-macos.zip)**.
2. Otevřete terminál a spusťte:
   ```bash
   ./spustit_krizovku.command
   ```

---

## 💻 Použití z příkazové řádky (CLI)

Kompilace a spuštění pomocí nástroje `cargo`:

```bash
# Klonování repozitáře
git clone https://github.com/Xemixo/czech-swedish-crossword-generator.git
cd czech-swedish-crossword-generator

# Sestavení optimalizované verze
cargo build --release

# Spuštění interaktivního režimu
./target/release/cwgen
```

### Příklady příkazů

```bash
# Snadná křížovka 10x10 s tajenkou "LES"
cwgen --size 10x10 -D easy -s "LES"

# Střední křížovka 12x10 s tajenkou "PRAHA"
cwgen --size 12x10 -D medium -s "PRAHA"

# Velká křížovka 20x20
cwgen --size 20x20 -D easy

# Explicitní výstup do HTML a XML
cwgen -S 15x15 -D medium -s "HVĚZDA" --html krizovka.html -o krizovka.xml

# Zobrazení křížovky přímo v terminálu v ASCII podobě
cwgen -S 8x8 --ascii
```

### Přehled parametrů

| Přepínač | Dlouhý název | Výchozí hodnota | Popis |
|---|---|---|---|
| `-S`, `--size` | `[ROZMĚR]` | `10x10` | Rozměr mřížky (např. `10x10`, `12x10`, `15x15`) |
| `-D`, `--difficulty` | `[OBTÍŽNOST]` | `mix` | Preferovaná obtížnost: `easy`, `medium`, `hard`, `all` |
| `-s`, `--secret` | `[TAJENKA]` | `none` | Slovo nebo fráze tajenky (např. `LES`, `HVĚZDA`) |
| `-d`, `--dictionary` | `[CESTA]` | `reference/word_difficulty.tsv` | Cesta k TSV slovníku |
| `-t`, `--template` | `[CESTA]` | `none` | Vstupní XML šablona CwdStudio |
| `--html` | `[CESTA]` | `output/*.html` | Cesta k výstupnímu interaktivnímu HTML |
| `-o`, `--output` | `[CESTA]` | `output/*.xml` | Cesta k výstupnímu CwdStudio XML |
| `--ascii` | — | `false` | Vykreslení křížovky do terminálu |
| `--seed` | `[ČÍSLO]` | `náhodný` | Seed pro generátor náhodných čísel |

---

## 🏗️ Architektura projektu

Projekt je uspořádán jako Cargo workspace rozdělený na znovupoužitelnou knihovnu a CLI aplikaci:

```
czech-swedish-crossword-generator/
├── Cargo.toml                  ← Workspace manifest
├── crossword_core/             ← Knihovna křížovkářského jádra
│   ├── src/
│   │   ├── grid.rs             ← Reprezentace buněk, mřížky a slotů
│   │   ├── letter.rs           ← Model české abecedy a spřežky CH
│   │   ├── dict.rs             ← Načítání TSV a DAWG/Trie indexer
│   │   ├── layout.rs           ← Procedurální generátor rozvržení
│   │   ├── solver.rs           ← CSP řešitel (AC-3 + MRV)
│   │   ├── tajenka.rs          ← Plánování cesty a vkládání tajenek
│   │   ├── html.rs             ← Exportér interaktivního HTML s UI
│   │   └── export.rs           ← Exportér do CwdStudio XML
│   └── tests/                  ← Integrační testy (dict, grid, solver, tajenka, export)
├── crossword_cli/              ← Spustitelná CLI aplikace
│   └── src/
│       ├── lib.rs              ← CLI argumenty a orchestrace běhu
│       └── main.rs             ← Vstupní bod programu (cwgen)
├── reference/
│   ├── word_difficulty.tsv     ← Hlavní slovník (19,591 ověřených hesel s obtížností)
│   ├── reference.xml           ← CwdStudio referenční šablona (8x8)
│   ├── sample_dict.txt         ← Testovací slovník pro integrační testy
│   └── template_10x10.xml      ← Testovací 10x10 šablona
├── docs/                       ← Webová stránka pro GitHub Pages
│   └── index.html              ← Online luštění a prezentace projektu
├── Spustit_krizovku.bat        ← Spouštěcí dávkový soubor pro Windows
├── spustit_krizovku.command    ← Spouštěcí skript pro macOS
└── README.md
```

---

## 🧪 Spuštění testů

Všechny testy (jednotkové i integrační) spustíte příkazem:

```bash
cargo test
```

---

## 📄 Licence

Tento projekt je licencován pod licencí **MIT** — podrobnosti naleznete v souboru [LICENSE](LICENSE).
Vyvinul Boris Patloch ([@Xemixo](https://github.com/Xemixo)).
