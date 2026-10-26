//! Tajenka (secret solution) embedder — highlights a path of cells spelling a hidden phrase.
//!
//! # Overview
//!
//! In Czech crosswords, a **tajenka** is a secret message or punchline spelled by
//! designated cells in the completed puzzle, read in numerical sequence (1-based index).
//!
//! ## Core Features
//!
//! 1. **Path Representation**:
//!    - [`TajenkaRequest`]: input holding the secret phrase ([`CzechLetter`] tokens)
//!      and an optional explicit cell coordinate path.
//!    - [`TajenkaPath`]: sequence of grid coordinates mapped to their target letters.
//!    - [`TajenkaError`]: typed errors for collisions, infeasible letter placements,
//!      length mismatches, or grid capacity issues.
//!
//! 2. **Auto-Selection Heuristic**:
//!    - When no explicit path is given, auto-selects $N$ letter cells distributed
//!      across distinct word slots to prevent visual clustering.
//!    - Enforces reading order ($C_0 < C_1 < \dots < C_{N-1}$) across the grid.
//!    - Balances spatial distribution and maximizes candidate dictionary support.
//!
//! 3. **Feasibility Pre-Check**:
//!    - Verifies that for every covering slot across a tajenka cell, the dictionary
//!      contains candidate words matching the target letter at the corresponding offset.
//!
//! 4. **CSP Solver Integration**:
//!    - Converts selected grid cells to [`Cell::Solution(Some(letter), index)`].
//!    - When [`crate::solver::solve`] is invoked, slot candidate domains are
//!      immediately constrained to words matching the secret letters.

use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use thiserror::Error;

use crate::dict::Dictionary;
use crate::grid::{Cell, Grid, Intersection, SlotId, WordSlot};
use crate::letter::{CzechLetter, CzechLetterError};
use crate::solver::{
    check_ac3_with_slots, solve, Solution, Solver, SolverConfig, SolverError,
};

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors encountered during tajenka path selection, validation, or embedding.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TajenkaError {
    /// Explicit path length does not match the secret phrase length.
    #[error("Tajenka length mismatch: text has {text_len} letters but path has {path_len} cells")]
    LengthMismatch { text_len: usize, path_len: usize },

    /// Cell coordinate is out-of-bounds, not a letter cell, or duplicated.
    #[error("Tajenka cell collision: cell ({0}, {1}) is duplicated, out of bounds, or not a letter cell")]
    CellCollision(u8, u8),

    /// A required letter cannot be placed at a cell because no dictionary words
    /// of matching length satisfy the constraint in a covering slot.
    #[error("Infeasible tajenka word placement: cannot place letter '{letter}' at cell ({x}, {y}) for slot {slot_id}")]
    Infeasible {
        x: u8,
        y: u8,
        letter: CzechLetter,
        slot_id: SlotId,
    },

    /// The grid contains fewer letter cells than required by the tajenka phrase.
    #[error("Not enough letter cells in grid for tajenka of length {needed}: only {available} available")]
    NotEnoughCells { needed: usize, available: usize },

    /// No feasible cell sequence could be found that satisfies all letter constraints.
    #[error("No feasible tajenka path found matching the secret phrase")]
    NoFeasiblePath,

    /// Tajenka phrase is empty.
    #[error("Empty tajenka phrase requested")]
    EmptyPhrase,

    /// CSP solver error while solving with tajenka constraints.
    #[error("Solver error: {0}")]
    Solver(#[from] SolverError),

    /// Czech letter parsing error.
    #[error("Czech letter error: {0}")]
    Letter(#[from] CzechLetterError),

    /// Underlying grid error.
    #[error("Grid error: {0}")]
    Grid(String),
}

// ---------------------------------------------------------------------------
// Request and Path
// ---------------------------------------------------------------------------

/// A request to embed a tajenka (secret solution) into a crossword puzzle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TajenkaRequest {
    /// Target secret phrase tokenized into Czech crossword letters.
    pub text: Vec<CzechLetter>,
    /// Optional explicit path of grid coordinates `(x, y)`.
    /// If `None`, the auto-selection heuristic is used.
    pub path: Option<Vec<(u8, u8)>>,
}

impl TajenkaRequest {
    /// Creates a request with auto-path selection.
    pub fn new(text: Vec<CzechLetter>) -> Self {
        Self { text, path: None }
    }

    /// Creates a request with an explicit sequence of cell coordinates.
    pub fn with_path(text: Vec<CzechLetter>, path: Vec<(u8, u8)>) -> Self {
        Self {
            text,
            path: Some(path),
        }
    }

    /// Parses a phrase string into a tajenka request with auto-path selection.
    pub fn from_phrase(phrase: &str) -> Result<Self, CzechLetterError> {
        let text = CzechLetter::parse_word(phrase)?;
        Ok(Self::new(text))
    }

    /// Parses a phrase string into a tajenka request with an explicit path.
    pub fn from_phrase_with_path(
        phrase: &str,
        path: Vec<(u8, u8)>,
    ) -> Result<Self, CzechLetterError> {
        let text = CzechLetter::parse_word(phrase)?;
        Ok(Self::with_path(text, path))
    }
}

/// A validated sequence of coordinates and their assigned secret letters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TajenkaPath {
    /// Ordered sequence of `((x, y), letter)` tuples.
    pub cells: Vec<((u8, u8), CzechLetter)>,
}

impl TajenkaPath {
    /// Creates a new `TajenkaPath`.
    pub fn new(cells: Vec<((u8, u8), CzechLetter)>) -> Self {
        Self { cells }
    }

    /// Returns the cell coordinates in sequence order.
    pub fn coords(&self) -> Vec<(u8, u8)> {
        self.cells.iter().map(|&(coord, _)| coord).collect()
    }

    /// Returns the assigned letters in sequence order.
    pub fn letters(&self) -> Vec<CzechLetter> {
        self.cells.iter().map(|&(_, letter)| letter).collect()
    }

    /// Formats the secret phrase as a string.
    pub fn text(&self) -> String {
        CzechLetter::format_word(&self.letters())
    }

    /// Number of letters in the tajenka path.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns true if the path contains zero letters.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

impl Deref for TajenkaPath {
    type Target = [((u8, u8), CzechLetter)];

    fn deref(&self) -> &Self::Target {
        &self.cells
    }
}

// ---------------------------------------------------------------------------
// Path Selection and Validation
// ---------------------------------------------------------------------------

/// Validates an explicit path or auto-selects a distributed path of cells
/// for the secret phrase, pre-checking dictionary feasibility.
pub fn select_tajenka_path(
    grid: &Grid,
    dict: &Dictionary,
    request: &TajenkaRequest,
) -> Result<TajenkaPath, TajenkaError> {
    if request.text.is_empty() {
        return Err(TajenkaError::EmptyPhrase);
    }

    let slots = grid.extract_slots();

    match &request.path {
        Some(explicit_path) => validate_explicit_path(grid, &slots, dict, &request.text, explicit_path),
        None => auto_select_path(grid, &slots, dict, &request.text),
    }
}

/// Validates an explicit path provided by the caller.
fn validate_explicit_path(
    grid: &Grid,
    slots: &[WordSlot],
    dict: &Dictionary,
    text: &[CzechLetter],
    path: &[(u8, u8)],
) -> Result<TajenkaPath, TajenkaError> {
    if path.len() != text.len() {
        return Err(TajenkaError::LengthMismatch {
            text_len: text.len(),
            path_len: path.len(),
        });
    }

    let mut seen_coords = HashSet::new();

    // 1. Verify bounds, cell types, and uniqueness.
    for &(x, y) in path {
        if !seen_coords.insert((x, y)) {
            return Err(TajenkaError::CellCollision(x, y));
        }
        match grid.cell(x, y) {
            Some(cell) if cell.is_letter_cell() => {}
            _ => return Err(TajenkaError::CellCollision(x, y)),
        }
    }

    // 2. Feasibility check: for each slot covering one or more tajenka cells,
    // verify the dictionary has words satisfying all positional constraints simultaneously.
    for slot in slots {
        let mut slot_constraints = Vec::new();
        for (i, (&coord, &letter)) in path.iter().zip(text.iter()).enumerate() {
            if let Some(pos) = slot.cells.iter().position(|&c| c == coord) {
                slot_constraints.push((pos, letter, i, coord));
            }
        }

        if !slot_constraints.is_empty() {
            let query_pairs: Vec<(usize, CzechLetter)> = slot_constraints
                .iter()
                .map(|&(pos, l, _, _)| (pos, l))
                .collect();
            let matching = dict.query_constraints(slot.length as usize, &query_pairs);
            if matching.is_empty() {
                let (_, letter, _, (cx, cy)) = slot_constraints[0];
                return Err(TajenkaError::Infeasible {
                    x: cx,
                    y: cy,
                    letter,
                    slot_id: slot.id,
                });
            }
        }
    }

    let items = path.iter().copied().zip(text.iter().copied()).collect();
    Ok(TajenkaPath::new(items))
}

/// Auto-selects $N$ cells in reading order distributed across distinct word slots.
fn auto_select_path(
    grid: &Grid,
    slots: &[WordSlot],
    dict: &Dictionary,
    text: &[CzechLetter],
) -> Result<TajenkaPath, TajenkaError> {
    let n = text.len();

    // 1. Build index mapping: cell (x, y) -> list of (slot_id, pos_in_slot, slot_length).
    let mut cell_slots: HashMap<(u8, u8), Vec<(SlotId, usize, u8)>> = HashMap::new();
    for slot in slots {
        for (pos, &coord) in slot.cells.iter().enumerate() {
            cell_slots
                .entry(coord)
                .or_default()
                .push((slot.id, pos, slot.length));
        }
    }

    // 2. Collect all letter cells belonging to at least one slot in reading order.
    let mut letter_cells = Vec::new();
    for y in 0..grid.height {
        for x in 0..grid.width {
            let coord = (x, y);
            if grid.cell(x, y).map_or(false, |c| c.is_letter_cell()) && cell_slots.contains_key(&coord) {
                letter_cells.push(coord);
            }
        }
    }

    let m = letter_cells.len();
    if m < n {
        return Err(TajenkaError::NotEnoughCells {
            needed: n,
            available: m,
        });
    }

    let intersections = Grid::find_intersections_for_slots(slots);
    let base_ac3_ok = check_ac3_with_slots(grid, slots, &intersections, dict);

    // 3. Depth-first priority search with candidate heuristic ordering.
    //
    // Candidate evaluation criteria:
    // a. Minimal slot conflicts with previously selected cells (prefer distinct slots).
    // b. Proximity to ideal reading position k * M / N + M / (2N).
    // c. Higher dictionary candidate support.
    struct SearchContext<'a> {
        grid: &'a Grid,
        slots: &'a [WordSlot],
        intersections: &'a [Intersection],
        base_ac3_ok: bool,
        letter_cells: &'a [(u8, u8)],
        cell_slots: &'a HashMap<(u8, u8), Vec<(SlotId, usize, u8)>>,
        dict: &'a Dictionary,
        text: &'a [CzechLetter],
        n: usize,
        m: usize,
        steps: usize,
    }

    fn search(
        step: usize,
        min_idx: usize,
        chosen: &mut Vec<(usize, (u8, u8))>,
        slot_constraints: &mut HashMap<SlotId, Vec<(usize, CzechLetter)>>,
        ctx: &mut SearchContext<'_>,
    ) -> Option<Vec<(u8, u8)>> {
        ctx.steps += 1;
        if ctx.steps > 50_000 {
            return None;
        }

        if step == ctx.n {
            let candidate_coords: Vec<(u8, u8)> = chosen.iter().map(|&(_, coord)| coord).collect();
            // If the base template/dict configuration is AC-3 viable, verify that setting
            // these secret letters does not cause AC-3 to prune any intersecting slot to 0.
            if ctx.base_ac3_ok {
                let mut test_grid = ctx.grid.clone();
                for (i, &coord) in candidate_coords.iter().enumerate() {
                    let _ = test_grid.set_cell(
                        coord.0,
                        coord.1,
                        Cell::Solution(Some(ctx.text[i]), (i + 1) as u8),
                    );
                }
                if !check_ac3_with_slots(&test_grid, ctx.slots, ctx.intersections, ctx.dict) {
                    // AC-3 domain pruning wiped out at least one slot domain.
                    // Backtrack and try alternative tajenka cell paths across the grid.
                    return None;
                }
            }

            return Some(candidate_coords);
        }

        let remaining = ctx.n - 1 - step;
        let max_idx = ctx.m.saturating_sub(remaining + 1);
        if min_idx > max_idx {
            return None;
        }

        let letter = ctx.text[step];
        let ideal_idx = ((step as f64 + 0.5) * (ctx.m as f64 / ctx.n as f64)).floor() as usize;

        // Gather all feasible candidate cells for this step
        struct Candidate {
            idx: usize,
            coord: (u8, u8),
            conflicts: usize,
            dist: usize,
            min_support: usize,
            total_support: usize,
        }

        let mut candidates = Vec::new();
        for idx in min_idx..=max_idx {
            let coord = ctx.letter_cells[idx];
            if let Some(covering) = ctx.cell_slots.get(&coord) {
                if covering.is_empty() {
                    continue;
                }

                let mut feasible = true;
                let mut conflicts = 0;
                let mut min_support = usize::MAX;
                let mut total_support = 0;

                for &(slot_id, pos, slot_len) in covering {
                    if slot_constraints.contains_key(&slot_id) {
                        conflicts += 1;
                    }

                    // Check if adding this constraint produces non-empty candidate domain
                    let mut test_constraints = slot_constraints
                        .get(&slot_id)
                        .cloned()
                        .unwrap_or_default();
                    test_constraints.push((pos, letter));

                    let matching = ctx
                        .dict
                        .query_constraints(slot_len as usize, &test_constraints);
                    if matching.is_empty() {
                        feasible = false;
                        break;
                    }
                    let m_len = matching.len();
                    min_support = min_support.min(m_len);
                    total_support += m_len;
                }

                if feasible {
                    let dist = idx.abs_diff(ideal_idx);
                    candidates.push(Candidate {
                        idx,
                        coord,
                        conflicts,
                        dist,
                        min_support,
                        total_support,
                    });
                }
            }
        }

        // Sort candidates: minimal conflicts first, then highest bottleneck support,
        // then highest total support, and finally closest to ideal reading position.
        candidates.sort_by(|a, b| {
            a.conflicts
                .cmp(&b.conflicts)
                .then_with(|| b.min_support.cmp(&a.min_support))
                .then_with(|| b.total_support.cmp(&a.total_support))
                .then_with(|| a.dist.cmp(&b.dist))
        });

        for cand in candidates {
            let covering = ctx.cell_slots.get(&cand.coord).unwrap();
            for &(slot_id, pos, _) in covering {
                slot_constraints
                    .entry(slot_id)
                    .or_default()
                    .push((pos, letter));
            }

            chosen.push((cand.idx, cand.coord));

            if let Some(res) = search(step + 1, cand.idx + 1, chosen, slot_constraints, ctx) {
                return Some(res);
            }

            // Backtrack
            chosen.pop();
            for &(slot_id, _, _) in covering {
                if let Some(constraints) = slot_constraints.get_mut(&slot_id) {
                    constraints.pop();
                    if constraints.is_empty() {
                        slot_constraints.remove(&slot_id);
                    }
                }
            }
        }

        None
    }

    let mut ctx = SearchContext {
        grid,
        slots,
        intersections: &intersections,
        base_ac3_ok,
        letter_cells: &letter_cells,
        cell_slots: &cell_slots,
        dict,
        text,
        n,
        m,
        steps: 0,
    };

    let mut chosen = Vec::with_capacity(n);
    let mut slot_constraints = HashMap::new();

    let path_coords = search(0, 0, &mut chosen, &mut slot_constraints, &mut ctx)
        .ok_or(TajenkaError::NoFeasiblePath)?;

    let path_items = path_coords.into_iter().zip(text.iter().copied()).collect();
    Ok(TajenkaPath::new(path_items))
}

// ---------------------------------------------------------------------------
// Public Embedding Functions
// ---------------------------------------------------------------------------

/// Embeds a tajenka request into a grid by selecting a path and updating
/// the designated cells to [`Cell::Solution`].
///
/// The cells are populated with `Cell::Solution(Some(letter), 1-based index)`.
/// This fixes the secret letters so that subsequent calls to [`crate::solver::solve`]
/// enforce the tajenka during CSP constraint propagation.
pub fn embed_tajenka(
    grid: &mut Grid,
    dict: &Dictionary,
    request: &TajenkaRequest,
) -> Result<TajenkaPath, TajenkaError> {
    let path = select_tajenka_path(grid, dict, request)?;

    for (i, &((x, y), letter)) in path.cells.iter().enumerate() {
        let index = (i + 1) as u8;
        grid.set_cell(x, y, Cell::Solution(Some(letter), index))
            .map_err(|e| TajenkaError::Grid(e.to_string()))?;
    }

    Ok(path)
}

/// Embeds a tajenka and immediately solves the puzzle using the CSP solver,
/// applying the solved words back onto the grid.
pub fn embed_and_solve_tajenka(
    grid: &mut Grid,
    dict: &Dictionary,
    request: &TajenkaRequest,
    config: &SolverConfig,
) -> Result<(TajenkaPath, Solution), TajenkaError> {
    let path = embed_tajenka(grid, dict, request)?;
    let solution = solve(grid, dict, config)?;
    let slots = grid.extract_slots();
    solution.apply_to_grid(grid, &slots);
    Ok((path, solution))
}

/// Convenience wrapper for embedding tajenka using a [`Solver`] instance.
pub fn embed_tajenka_with_solver(
    grid: &mut Grid,
    request: &TajenkaRequest,
    solver: &mut Solver<'_>,
) -> Result<TajenkaPath, TajenkaError> {
    embed_tajenka(grid, solver.dict, request)
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::Dictionary;
    use crate::grid::{Grid, WordSlot};
    use crate::letter::CzechLetter;
    use std::io::Cursor;

    fn test_dict() -> Dictionary {
        let tsv = "\
LES\tLes
PES\tPes
OKO\tOko
NOC\tNoc
DEN\tDen
AUTO\tAuto
ATOM\tAtom
VODA\tVoda
ZIMA\tZima
LÉTO\tLéto
BRNO\tMěsto Brno
OBAL\tObal na knihu
CENA\tCena zboží
KOSA\tZemědělský nástroj
MOST\tStavba přes řeku
HRAD\tStředověké sídlo
RÁNO\tZačátek dne
BOTA\tObuv
";
        Dictionary::from_reader(Cursor::new(tsv)).expect("Failed to load test dict")
    }

    /// Builds a minimal grid with two crossing slots:
    /// - Horizontal slot across row 1: (0, 1), (1, 1), (2, 1), (3, 1) [len 4]
    /// - Vertical slot across col 0: (0, 0), (0, 1), (0, 2), (0, 3) [len 4]
    /// All other cells are Cell::Symbol.
    fn crossing_2slot_grid() -> (Grid, Vec<WordSlot>) {
        let mut grid = Grid::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                if y == 1 || x == 0 {
                    grid.set_cell(x, y, Cell::Blank(None)).unwrap();
                } else {
                    grid.set_cell(x, y, Cell::Symbol).unwrap();
                }
            }
        }
        let slots = grid.extract_slots();
        (grid, slots)
    }

    #[test]
    fn test_tajenka_request_from_phrase() {
        let req = TajenkaRequest::from_phrase("BRNO").expect("Failed to parse phrase");
        assert_eq!(req.text.len(), 4);
        assert_eq!(
            req.text,
            vec![
                CzechLetter::B,
                CzechLetter::R,
                CzechLetter::N,
                CzechLetter::O
            ]
        );
        assert!(req.path.is_none());
    }

    #[test]
    fn test_tajenka_explicit_path_success() {
        let dict = test_dict();
        let (mut grid, _) = crossing_2slot_grid();

        // Row 1 has cells (0, 1), (1, 1), (2, 1), (3, 1).
        // Cell (0, 1) intersects column 0 at pos 1.
        // In test_dict(), "OBAL" has 'B' at pos 1, and "BRNO" has 'B' at pos 0!
        let req = TajenkaRequest::from_phrase_with_path(
            "BRNO",
            vec![(0, 1), (1, 1), (2, 1), (3, 1)],
        )
        .unwrap();

        let path = embed_tajenka(&mut grid, &dict, &req).expect("Explicit path should succeed");
        assert_eq!(path.len(), 4);
        assert_eq!(path.text(), "BRNO");

        // Verify cells in grid are converted to Cell::Solution
        for (i, &(coord, letter)) in path.iter().enumerate() {
            let (x, y) = coord;
            let cell = grid.cell(x, y).unwrap();
            assert_eq!(cell.letter(), Some(letter));
            match cell {
                Cell::Solution(Some(l), idx) => {
                    assert_eq!(*l, letter);
                    assert_eq!(*idx, (i + 1) as u8);
                }
                other => panic!("Expected SolutionCell at ({}, {}), got {:?}", x, y, other),
            }
        }

        // Verify that CSP solver can solve this grid with the embedded tajenka!
        let config = SolverConfig::new();
        let solution = solve(&grid, &dict, &config).expect("Solver should solve with tajenka");
        let slots = grid.extract_slots();
        solution.apply_to_grid(&mut grid, &slots);

        // Check that cells (0, 1)..(3, 1) spell "BRNO"
        let spelled: Vec<CzechLetter> = vec![(0, 1), (1, 1), (2, 1), (3, 1)]
            .into_iter()
            .map(|(x, y)| grid.cell(x, y).unwrap().letter().unwrap())
            .collect();
        assert_eq!(CzechLetter::format_word(&spelled), "BRNO");
    }

    #[test]
    fn test_tajenka_auto_path_selection_and_solve() {
        let dict = test_dict();
        // 4x7 grid with 4 horizontal slots separated by symbol rows:
        // - Row 0: (0,0)..(3,0) [len 4] -> BOTA
        // - Row 2: (0,2)..(3,2) [len 4] -> HRAD
        // - Row 4: (0,4)..(3,4) [len 4] -> CENA
        // - Row 6: (0,6)..(3,6) [len 4] -> AUTO
        let mut grid = Grid::new(4, 7);
        for y in 0..7 {
            for x in 0..4 {
                if y % 2 == 0 {
                    grid.set_cell(x, y, Cell::Blank(None)).unwrap();
                } else {
                    grid.set_cell(x, y, Cell::Symbol).unwrap();
                }
            }
        }

        // Tajenka "BRNO"
        let req = TajenkaRequest::from_phrase("BRNO").unwrap();
        let config = SolverConfig::new();

        let (path, solution) = embed_and_solve_tajenka(&mut grid, &dict, &req, &config)
            .expect("Embedding and solve should succeed");

        assert_eq!(path.len(), 4);
        assert_eq!(path.text(), "BRNO");

        // Assert that solved letter cells at tajenka coordinates exactly spell the target phrase.
        for (i, &((x, y), letter)) in path.cells.iter().enumerate() {
            let cell = grid.cell(x, y).unwrap();
            assert_eq!(
                cell.letter(),
                Some(letter),
                "Cell ({}, {}) must spell letter {}",
                x,
                y,
                letter
            );
            assert!(
                matches!(cell, Cell::Solution(Some(l), idx) if *l == letter && *idx == (i + 1) as u8)
            );
        }

        assert!(!solution.assignments.is_empty());
    }

    #[test]
    fn test_tajenka_error_length_mismatch() {
        let dict = test_dict();
        let (mut grid, _) = crossing_2slot_grid();

        // 4 letters ("BRNO") but only 3 coordinates
        let req = TajenkaRequest::from_phrase_with_path(
            "BRNO",
            vec![(0, 1), (1, 1), (2, 1)],
        )
        .unwrap();

        let err = embed_tajenka(&mut grid, &dict, &req).unwrap_err();
        assert!(matches!(
            err,
            TajenkaError::LengthMismatch {
                text_len: 4,
                path_len: 3
            }
        ));
    }

    #[test]
    fn test_tajenka_error_cell_collision() {
        let dict = test_dict();
        let (mut grid, _) = crossing_2slot_grid();

        // Duplicate cell (1, 1) in explicit path
        let req = TajenkaRequest::from_phrase_with_path(
            "LES",
            vec![(0, 1), (1, 1), (1, 1)],
        )
        .unwrap();

        let err = embed_tajenka(&mut grid, &dict, &req).unwrap_err();
        assert!(matches!(err, TajenkaError::CellCollision(1, 1)));
    }

    #[test]
    fn test_tajenka_error_not_letter_cell() {
        let dict = test_dict();
        let (mut grid, _) = crossing_2slot_grid();

        // (2, 2) is a SymbolCell in crossing_2slot_grid
        let req = TajenkaRequest::from_phrase_with_path(
            "LES",
            vec![(2, 2), (1, 1), (0, 1)],
        )
        .unwrap();

        let err = embed_tajenka(&mut grid, &dict, &req).unwrap_err();
        assert!(matches!(err, TajenkaError::CellCollision(2, 2)));
    }

    #[test]
    fn test_tajenka_error_infeasible_letter() {
        let dict = test_dict();
        let (mut grid, _) = crossing_2slot_grid();

        // Position where no 4-letter word has letter 'Q' (Q doesn't exist in dict)
        let q_letter = CzechLetter::Q;
        let req = TajenkaRequest::with_path(
            vec![q_letter],
            vec![(0, 1)],
        );

        let err = embed_tajenka(&mut grid, &dict, &req).unwrap_err();
        assert!(matches!(err, TajenkaError::Infeasible { letter, .. } if letter == CzechLetter::Q));
    }

    #[test]
    fn test_tajenka_error_not_enough_cells() {
        let dict = test_dict();
        let mut grid = Grid::new(2, 1);
        grid.set_cell(0, 0, Cell::Blank(None)).unwrap();
        grid.set_cell(1, 0, Cell::Blank(None)).unwrap();

        // Need 4 cells for "BRNO", but grid only has 2
        let req = TajenkaRequest::from_phrase("BRNO").unwrap();
        let err = embed_tajenka(&mut grid, &dict, &req).unwrap_err();
        assert!(matches!(
            err,
            TajenkaError::NotEnoughCells {
                needed: 4,
                available: 2
            }
        ));
    }

    #[test]
    fn test_tajenka_error_empty_phrase() {
        let dict = test_dict();
        let (mut grid, _) = crossing_2slot_grid();
        let req = TajenkaRequest::new(Vec::new());
        let err = embed_tajenka(&mut grid, &dict, &req).unwrap_err();
        assert_eq!(err, TajenkaError::EmptyPhrase);
    }
}
