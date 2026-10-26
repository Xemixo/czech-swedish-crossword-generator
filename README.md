# Czech Swedish Crossword Generator (Švédské křížovky)

[![Release](https://img.shields.io/github/v/release/Xemixo/czech-swedish-crossword-generator?color=blue)](https://github.com/Xemixo/czech-swedish-crossword-generator/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange.svg)](https://www.rust-lang.org/)
[![Web Demo](https://img.shields.io/badge/Web%20Demo-Online-success)](https://xemixo.github.io/czech-swedish-crossword-generator/)

Procedurální generátor a CSP řešitel švédských křížovek v jazyce Rust s podporou tajenek a interaktivního webového luštění.

Online luštění a ukázky: **[https://xemixo.github.io/czech-swedish-crossword-generator/](https://xemixo.github.io/czech-swedish-crossword-generator/)**

---

## Vlastnosti

- **CSP řešitel (AC-3 + MRV)**: Propagace omezení AC-3 a heuristika Minimum Remaining Values (MRV) pro rychlé prohledávání stavového prostoru.
- **Česká abeceda a spřežka CH**: Správné zacházení se všemi českými znaky s diakritikou. Spřežka CH je reprezentována jako samostatný křížovkářský znak na jedné pozici.
- **Vkládání vlastních tajenek**: Možnost zadat libovolnou tajenku (`-s "TAJENKA"`). Algoritmus automaticky najde vhodnou sekvenci letter buněk v mřížce.
- **Kategorizovaný slovník**: Přes 19 500 hesel s definicemi roztříděných do kategorií Snadná, Střední a Těžká pro volbu obtížnosti.
- **Procedurální generování rozvržení**: Automatické rozmístění legendových polí a černých buněk od 4x4 do 30x30.
- **Interaktivní HTML export**: Samostatná webová stránka pro luštění v prohlížeči (kontrola chyb, nápověda, počítadlo tajenky, tiskový styl pro A4).

---

## Rychlý start (předpřipravené balíčky)

### Windows x64
1. Stáhněte archiv **[czech-swedish-crossword-v1.0.0-windows-x64.zip](https://github.com/Xemixo/czech-swedish-crossword-generator/releases/download/v1.0.0/czech-swedish-crossword-v1.0.0-windows-x64.zip)**.
2. Rozbalte soubor.
3. Spusťte `Spustit_krizovku.bat` dvojklikem (nevyžaduje instalaci Rustu ani dalších závislostí).

### macOS a Linux
1. Stáhněte archiv **[czech-swedish-crossword-v1.0.0-macos.zip](https://github.com/Xemixo/czech-swedish-crossword-generator/releases/download/v1.0.0/czech-swedish-crossword-v1.0.0-macos.zip)**.
2. V terminálu spusťte:
   ```bash
   ./spustit_krizovku.command
   ```

---

## Sestavení ze zdrojových kódů

Vyžaduje nainstalovaný Rust (toolchain 2021).

```bash
git clone https://github.com/Xemixo/czech-swedish-crossword-generator.git
cd czech-swedish-crossword-generator
cargo build --release
```

Zkompilovaná binárka se nachází v `target/release/cwgen`.

---

## Použití z příkazové řádky

```bash
# Interaktivní režim
cwgen

# Snadná křížovka 10x10 s tajenkou "LES"
cwgen --size 10x10 -D easy -s "LES"

# Střední křížovka 12x10 s tajenkou "PRAHA"
cwgen --size 12x10 -D medium -s "PRAHA"

# Křížovka 20x20 s výstupem do zadaného souboru
cwgen --size 20x20 -D easy --html output/krizovka.html

# Zobrazení mřížky v terminálu (ASCII)
cwgen -S 8x8 --ascii
```

### Přehled parametrů

| Přepínač | Argument | Výchozí | Popis |
|---|---|---|---|
| `-S`, `--size` | `[ROZMĚR]` | `10x10` | Rozměr mřížky (např. `10x10`, `12x10`, `15x15`) |
| `-D`, `--difficulty` | `[OBTÍŽNOST]` | `mix` | Obtížnost: `easy`, `medium`, `hard`, `all` |
| `-s`, `--secret` | `[TAJENKA]` | `none` | Slovo nebo fráze tajenky |
| `-d`, `--dictionary` | `[CESTA]` | `reference/word_difficulty.tsv` | Cesta k TSV slovníku |
| `-t`, `--template` | `[CESTA]` | `none` | Vstupní XML šablona |
| `--html` | `[CESTA]` | `output/*.html` | Výstupní interaktivní HTML |
| `-o`, `--output` | `[CESTA]` | `output/*.xml` | Výstupní XML |
| `--ascii` | — | `false` | Vykreslení mřížky do konzole |
| `--seed` | `[ČÍSLO]` | náhodný | Seed náhodného generátoru |

---

## Architektura projektu

- `crossword_core/`: knihovna pro reprezentaci buněk (`grid.rs`), českou abecedu (`letter.rs`), načítání slovníku (`dict.rs`), generování rozvržení (`layout.rs`), CSP řešitel (`solver.rs`), vkládání tajenek (`tajenka.rs`) a HTML exportér (`html.rs`).
- `crossword_cli/`: CLI nástroj `cwgen` pro volání z příkazové řádky.
- `reference/`: TSV slovník (`word_difficulty.tsv`) a šablony pro testy.
- `docs/`: podklady pro GitHub Pages webovou prezentaci.

---

## Spuštění testů

```bash
cargo test
```

---

## Licence

MIT License. Autor: Boris Patloch ([@Xemixo](https://github.com/Xemixo)).
