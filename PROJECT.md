# Swedish Crossword (Švédská křížovka) Generator Engine

> **Status**: Architecture & scaffolding phase (Phase 0)  
> **Language**: Rust (2021 edition, Cargo workspace)  
> **Reference**: CwdStudio XML + `.dict` file format reverse-engineered from `reference/`

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Workspace Layout](#2-workspace-layout)
3. [Reverse-Engineered Schemas](#3-reverse-engineered-schemas)
   - 3.1 [CwdStudio XML Grid Schema](#31-cwdstudio-xml-grid-schema)
   - 3.2 [CwdStudio `.dict` Binary Format](#32-cwdstudio-dict-binary-format)
4. [Domain Rules — Czech Alphabet](#4-domain-rules--czech-alphabet)
5. [Core Data Model](#5-core-data-model)
6. [Trie / DAWG Indexer](#6-trie--dawg-indexer)
7. [CSP Solver Design](#7-csp-solver-design)
8. [Tajenka (Secret Solution) Embedding](#8-tajenka-secret-solution-embedding)
9. [CwdStudio XML Export](#9-cwdstudio-xml-export)
10. [5-Phase Delivery Roadmap](#10-5-phase-delivery-roadmap)
11. [Open Questions & Decision Log](#11-open-questions--decision-log)

---

## 1. Project Overview

A **Swedish Crossword** (Czech: *Švédská křížovka*) is a grid-based word puzzle in which:

- **Clue boxes** (shaded cells) contain the clue text with directional arrows, replacing the numbered-clue approach of American-style crosswords.
- **Letter cells** (white/blank cells) are filled in by the solver.
- Words run **horizontally** or **vertically**, starting immediately after a clue box or the grid edge, and ending at the next clue box or edge.
- A special subset of cells can spell a **tajenka** (secret solution), marked visually with a highlight/circle and read in a predefined order.

This engine will:
1. Parse a CwdStudio XML grid template (defining the skeleton of clue boxes, blank cells, and symbol cells).
2. Load a Czech word + clue dictionary from CwdStudio's proprietary `.dict` format (or an open TSV alternative).
3. Run a CSP (Constraint Satisfaction Problem) solver to fill in all word slots.
4. Optionally embed a tajenka by biasing word selection so that highlighted cells spell the desired secret word.
5. Export the completed puzzle back to a CwdStudio-compatible XML file.

---

## 2. Workspace Layout

```
MyCWD/
├── Cargo.toml                  ← workspace root
├── PROJECT.md                  ← this document
├── reference/
│   ├── reference.xml           ← CwdStudio empty grid template (8x8)
│   ├── cs_demo.dict            ← small Czech demo dictionary (~479 entries)
│   └── cs_default.dict         ← full Czech dictionary (~2000+ entries)
│   └── sample_dict.txt         ← testing czech definitions dictionary (~30)
├── crossword_core/             ← library crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── grid.rs             ← Cell types, Grid, WordSlot, Arrow
│       ├── dict.rs             ← DictLoader, DictEntry, DAWG/Trie
│       ├── solver.rs           ← CSP engine (MRV + AC-3)
│       ├── tajenka.rs          ← Tajenka path extractor & biaser
│       └── export.rs           ← CwdStudio XML serializer
└── crossword_cli/              ← binary crate
    ├── Cargo.toml
    └── src/
        └── main.rs             ← `cwgen` CLI entry point
```

---

## 3. Reverse-Engineered Schemas

### 3.1 CwdStudio XML Grid Schema

**Source**: `reference/reference.xml` (89 lines, UTF-8, XML 1.0)

#### Top-level structure

```xml
<?xml version="1.0" encoding="UTF-8"?>
<CwdStudio type="crossword" comment="" dictionary="cs_demo.dict">
  <Puzzle width="8" height="8" langLocaleId="cs">
    <Cells> ... </Cells>
    <NumberTable word="" table="" />
    <Style ...> ... </Style>
  </Puzzle>
  <Form dictionary="cs_demo.dict" />
  <Preview> ... base64 PNG ... </Preview>
</CwdStudio>
```

#### Cell types inside `<Cells>`

| XML element   | Role                                      | Key attributes                                 |
|---------------|-------------------------------------------|------------------------------------------------|
| `<BlankCell>` | Letter cell (solver fills this)           | `x`, `y`                                       |
| `<ClueCell>`  | Clue box (shaded, contains text+arrows)   | `x`, `y`, `textHorizontal`, `textVertical`     |
| `<SymbolCell>`| Decorative/logo cell                      | `x`, `y`                                       |
| `<SolutionCell>`| Highlighted letter cell (part of tajenka) | `x`, `y`, plus `<index>` sub-element          |

Coordinates are **0-based (col=x, row=y)** with `(0,0)` at the **top-left** corner.

#### ClueCell arrows

A `ClueCell` at `(x, y)` introduces adjacent word slot(s):
- Non-empty `textHorizontal` → **horizontal** word slot starting at `(x+1, y)`.
- Non-empty `textVertical`   → **vertical** word slot starting at `(x, y+1)`.
- Both attributes can be non-empty simultaneously (double-arrow cell).

A `ClueCell` does **not** itself contribute a letter to any word.

#### `<Style>` and `<Preview>`

The `<Style>` block is purely visual (fonts, colours, borders). The engine treats it as **opaque** — copied verbatim to output XML. The `<Preview>` block is a base64-encoded PNG thumbnail, optionally omitted on export.

---

### 3.2 CwdStudio `.dict` Binary Format

**Sources**: `cs_demo.dict` (555,396 bytes), `cs_default.dict` (1,062,452 bytes)

> **WARNING**: The `.dict` format is **proprietary and encrypted**. It cannot be read as plain text or decompressed with standard algorithms.

#### Reverse-engineered binary layout

```
Offset  Size   Content
------  -----  ----------------------------------------------------------
0       20     ASCII magic: "CwdStudio dictionary"
20       1     Null terminator (0x00)
21      32     Word-count table: 16 x little-endian uint16
               Entry i (0-based) = word count for length (i + 2)
               Covers word lengths 2 through 17 inclusive
53      rest   Encrypted word+clue data block (proprietary cipher)
```

#### Word-count table for cs_demo.dict

| Length | Count | Length | Count |
|--------|-------|--------|-------|
| 2      | 33    | 10     | 52    |
| 3      | 61    | 11     | 48    |
| 4      | 24    | 12     | 58    |
| 5      | 49    | 13     | 26    |
| 6      | 21    | 14     | 20    |
| 7      | 11    | 15     | 18    |
| 8      |  0    | 16     |  1    |
| 9      | 29    | 17     | 28    |
| **Total** | **479** | | |

#### Encryption characteristics

The data payload (offset 53+) has been confirmed **strongly encrypted**:

- Byte frequency is near-perfectly uniform (max 2304, min 2056, expected flat 2169).
- Standard DEFLATE / zlib / GZip decompression fails.
- Consistent with AES-CTR, RC4, or a proprietary XOR-stream cipher.

**Resolution**: Until the cipher is cracked, the engine will consume an **open UTF-8 TSV dictionary**:

```tsv
# Czech crossword dictionary — UTF-8, LF line endings
# WORD<TAB>CLUE  (CH treated as single grapheme token)
AHOJ	Pozdrav
BRNO	Moravska metropole
CTYRI	Cislice za trojkou
```

Lines starting with `#` or empty lines are ignored. Words are stored **UPPERCASE**.

---

## 4. Domain Rules — Czech Alphabet

### Alphabet definition (42 graphemes)

```
A  A'  B  C  C^  D  D'  E  E'  E^  F  G  H  CH
I  I'  J  K  L   M  N   N' O   O'  P  Q  R  R^
S  S^  T  T' U   U' U^  V  W   X   Y  Y' Z  Z^
```

(Using ASCII approximations; actual Unicode: Á Č Ď É Ě Í Ň Ó Ř Š Ť Ú Ů Ý Ž)

### Critical rule: CH occupies exactly ONE grid cell

1. `CzechLetter` is a 42-variant enum, not a raw `char`.
2. String `"NACH"` parses to 3 cells: `[N, A, CH]`.
3. Intersections involving CH must match the full digraph — no split across two cells.
4. The trie has 42 edges per node (not 26).

### Parsing rule for CH

Scan left-to-right. If the current character is `C` and the next is `H`, emit `CzechLetter::Ch` and advance two positions; otherwise emit the single-character letter.

### CzechLetter encoding

```rust
#[repr(u8)]
pub enum CzechLetter {
    A=0,  Á=1,  B=2,  C=3,  Č=4,  D=5,  Ď=6,  E=7,  É=8,  Ě=9,
    F=10, G=11, H=12, Ch=13, I=14, Í=15, J=16, K=17, L=18, M=19,
    N=20, Ň=21, O=22, Ó=23, P=24, Q=25, R=26, Ř=27, S=28, Š=29,
    T=30, Ť=31, U=32, Ú=33, Ů=34, V=35, W=36, X=37, Y=38, Ý=39,
    Z=40, Ž=41,
}
```

All strings are NFC-normalized. Words are stored uppercase internally.

---

## 5. Core Data Model

```
Grid {
    width:  u8,
    height: u8,
    cells:  Vec<Vec<Cell>>,  // [row][col]
    lang:   LangLocale,
}

enum Cell {
    Blank(Option<CzechLetter>),          // None = unfilled
    Clue(ClueCell),
    Symbol,
    Solution(Option<CzechLetter>, u8),   // (letter, tajenka_index)
}

struct ClueCell {
    text_h: Option<String>,   // clue for rightward word
    text_v: Option<String>,   // clue for downward word
}

struct WordSlot {
    id:        SlotId,
    direction: Direction,        // Horizontal | Vertical
    start:     (u8, u8),         // (col, row) of first letter cell
    length:    u8,               // number of letter cells; CH = 1
    clue_pos:  Option<(u8, u8)>,
    cells:     Vec<(u8, u8)>,
    assigned:  Option<Vec<CzechLetter>>,
}

struct Intersection {
    h_slot: SlotId,
    h_pos:  u8,
    v_slot: SlotId,
    v_pos:  u8,
}
```

---

## 6. Trie / DAWG Indexer

### Level-indexed Array Trie (Phase 2)

```rust
struct Trie {
    nodes: Vec<TrieNode>,       // nodes[0] is root
}

struct TrieNode {
    children:    [Option<u32>; 42],  // indexed by CzechLetter as u8
    is_terminal: bool,
    entry_ids:   SmallVec<[u32; 2]>,
}
```

### Length-partitioned Bitset Index

For a slot of length L with constraint `(position p, letter l)`:

```
LengthIndex::query(L, constraints) -> BitVec
  = AND over all constraint bitsets
  = entry IDs satisfying all positional constraints simultaneously
```

Each bitset operation over N=2000 entries takes N/64 = 32 word-wide AND operations — O(1) in practice.

### DAWG upgrade (Phase 3 stretch)

Replace the trie with a **Minimal DAWG** (CDAWG construction). For a 2000-word Czech dictionary the expected node count is < 50,000, fitting comfortably in L2 cache and halving memory vs. a plain trie.

---

## 7. CSP Solver Design

### Problem formulation

- **Variables**: One `WordSlot` per slot.
- **Domains**: `Dict.words_of_length(slot.length)` for each slot.
- **Constraints**: For each `Intersection`, `word_h[h_pos] == word_v[v_pos]`.

### Algorithm: AC-3 Pre-solve + MRV Backtracking

#### Step 1 — AC-3 arc consistency

Prune domains before any branching:

```
worklist = all (Xi, Xj) pairs with shared intersection
while worklist not empty:
    (Xi, Xj) = worklist.pop()
    removed = revise(Di, Dj)
    if removed:
        if |Di| == 0 → UNSATISFIABLE
        for Xk in neighbours(Xi) \ {Xj}: add (Xk, Xi) to worklist
```

Typically reduces domains 60–90% before first branch.

#### Step 2 — Backtracking

```
fn backtrack(assignment, csp):
    if complete → return assignment
    slot = MRV(csp, assignment)           // fewest remaining values; ties by degree
    for word in LCV(slot, assignment, csp): // least constraining value first
        if consistent(word, slot, assignment):
            assign(slot, word)
            inferences = forward_check(slot, word, csp)
            if inferences != FAILURE:
                result = backtrack(assignment, csp)
                if result != FAILURE → return result
            undo(slot); undo(inferences)
    return FAILURE
```

#### Forward checking

After assigning a word, immediately prune all intersecting unassigned slots' domains. If any domain empties → backtrack immediately.

### Performance targets

| Grid      | Dict   | Target     |
|-----------|--------|------------|
| 8×8       | 500    | < 5 ms     |
| 12×12     | 2 000  | < 500 ms   |
| 15×15     | 2 000  | < 10 s     |

---

## 8. Tajenka (Secret Solution) Embedding

A **tajenka** is a hidden message spelled by selected cells in the completed grid, read in a numbered order.

### Embedding strategy

1. **Path selection**: Choose `|T|` distinct letter cells distributed across the grid (one per distinct slot preferred to avoid visual clustering).
2. **Solver biasing**: Add soft priority boosts to word candidates that place the correct tajenka letter at the required cell position. Hard constraints (intersections) are never violated.
3. **Feasibility pre-check**: Verify that at least one word in each affected slot's domain has the required letter at the required position. Report `TajenkaError::Infeasible` if not.
4. **XML marking**: Each tajenka cell is emitted as `<SolutionCell>` with a `<index>` sub-element.

### API sketch

```rust
pub struct TajenkaRequest {
    pub text: Vec<CzechLetter>,
    pub path: Option<Vec<(u8, u8)>>, // None = auto-select
}

pub fn embed_tajenka(
    grid:    &mut Grid,
    request: &TajenkaRequest,
    solver:  &mut Solver,
) -> Result<TajenkaPath, TajenkaError>;
```

---

## 9. CwdStudio XML Export

The exporter regenerates a complete `<CwdStudio>` XML document:

1. Copy `<Style>`, `<NumberTable>`, `<Form>` blocks verbatim from the input template.
2. Emit all cells sorted by `(y, x)` for determinism.
3. Fill `textHorizontal` / `textVertical` on `<ClueCell>` with the assigned `DictEntry.clue`.
4. Emit tajenka cells as `<SolutionCell>` with `<index>` sub-elements.
5. NFC-normalise all output strings; escape XML special characters.
6. `<Preview>` omitted or regenerated as PNG (Phase 5 decision).

---

## 10. 5-Phase Delivery Roadmap

### Phase 0 — Scaffolding ✅ DONE

- [x] Inspect reference files; reverse-engineer XML and `.dict` schemas.
- [x] Initialize Cargo workspace (`crossword_core` + `crossword_cli`).
- [x] Write PROJECT.md architectural blueprint.
- [x] Create stub module files (`grid.rs`, `dict.rs`, `solver.rs`, `tajenka.rs`, `export.rs`).

---

### Phase 1 — Grid Parser & Data Model

**Goal**: Parse a CwdStudio XML template into `Grid`; enumerate all `WordSlot`s and `Intersection`s.

Tasks:
- `CzechLetter` enum + `from_str` / `to_str` with CH digraph handling.
- `Cell`, `Grid`, `ClueCell`, `WordSlot`, `Intersection` structs.
- `Grid::from_xml(xml: &str) -> Result<Grid>` via `quick-xml`.
- `Grid::extract_slots() -> Vec<WordSlot>`.
- `Grid::find_intersections() -> Vec<Intersection>`.
- Unit tests: parse `reference.xml`; assert correct slot/intersection inventory.

**Exit criteria**: `cargo test` passes; `reference.xml` parses correctly.

---

### Phase 2 — Dictionary Loader & Trie Indexer

**Goal**: Load word+clue pairs from UTF-8 TSV; build Trie and bitset length-index.

Tasks:
- `DictEntry { word: Vec<CzechLetter>, clue: String }`.
- `DictLoader::from_tsv(path) -> Result<Vec<DictEntry>>`.
- `Trie` with insert, terminal lookup, and `words_of_length` traversal.
- `LengthIndex` bitset table and `query(length, constraints)`.
- Criterion benchmark: query latency < 50 µs for 2000-word dictionary.
- *(Stretch)* DAWG compression.

**Exit criteria**: Correct query results; benchmarks pass.

---

### Phase 3 — CSP Solver

**Goal**: Fill all `WordSlot`s with consistent words.

Tasks:
- `Csp` struct (grid + index + domain vectors).
- AC-3 pre-processor.
- MRV variable selector + degree tie-breaker.
- LCV value orderer.
- Forward-checking propagator.
- `Solver::solve(csp) -> Option<Solution>`.
- Integration test: solve reference 8×8 grid.
- Performance test: < 5 ms for 8×8.

**Exit criteria**: Valid solution produced; performance target met.

---

### Phase 4 — Tajenka Embedding

**Goal**: Bias the solver to embed a user-specified secret word.

Tasks:
- `TajenkaRequest`, `TajenkaPath`, `TajenkaError`.
- Auto path-selection heuristic.
- Solver priority boost for tajenka cells.
- Feasibility pre-check.
- CLI flag: `--tajenka <word>`.
- Integration test: embed `BRNO`; verify.

**Exit criteria**: `cwgen --tajenka BRNO` embeds correctly.

---

### Phase 5 — XML Exporter & CLI Polish

**Goal**: Write completed puzzle to CwdStudio-compatible XML.

Tasks:
- `Exporter::to_xml(solution, template) -> String`.
- Verbatim copy of `<Style>`, `<NumberTable>`, `<Form>`.
- `<SolutionCell>` with `<index>` for tajenka.
- Clue text NFC-normalisation + XML escaping.
- `--output <path>` CLI flag.
- End-to-end round-trip test.

**Exit criteria**: Output XML opens in CwdStudio without errors.

---

## 11. Open Questions & Decision Log

| #     | Question                                         | Status      | Decision                                        |
|-------|--------------------------------------------------|-------------|--------------------------------------------------|
| OQ-01 | Crack `.dict` binary encryption?                 | **Open**    | Build TSV loader first; crack dict as bonus      |
| OQ-02 | Regenerate `<Preview>` PNG on export?            | **Open**    | Omit in Phase 5; add renderer later              |
| OQ-03 | Max supported grid size?                         | **Open**    | Target 15×15; validate in Phase 3               |
| OQ-04 | Handle grids with no valid solution?             | **Open**    | Return `Err(SolverError::NoSolution)` + diag    |
| OQ-05 | Thread-parallelism in solver?                    | **Open**    | Add `rayon` in Phase 3 stretch goal              |
| OQ-06 | Support for other locales (Slovak, Polish)?      | **Open**    | Abstract `Alphabet` trait; Czech first           |
| OQ-07 | Tajenka path: user-specified vs auto?            | **Decided** | Both; `--tajenka-path` for manual override       |

---

*Document version: 0.1 — Phase 0 complete.*
*Last updated: 2026-10-15*
