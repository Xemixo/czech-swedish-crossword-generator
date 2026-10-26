//! crossword_core — Swedish Crossword (Švédská křížovka) engine library.
//!
//! Exposes:
//! - Czech letter alphabet & diacritics handling (Phase 1)
//! - Grid & cell types, XML parser, slot & intersection extractors (Phase 1)
//! - Dictionary loader & Trie/DAWG indexer (Phase 2)
//! - CSP solver with MRV + arc-consistency (Phase 3)
//! - Tajenka (secret solution) embedder (Phase 4)
//! - XML serializer back to CwdStudio format (Phase 5)

pub mod letter;
pub mod grid;
pub mod dict;
pub mod solver;
pub mod tajenka;
pub mod export;
pub mod html;
pub mod layout;

pub use letter::{CzechLetter, CzechLetterError};
pub use grid::{
    Cell, ClueCell, Direction, Grid, GridError, Intersection, LangLocale, SlotId, TemplateMetadata,
    WordSlot,
};
pub use dict::{
    DictEntry, DictError, DictLoader, Dictionary, Difficulty, DifficultyChoice, LengthIndex,
    SlotRequirement, Trie, TrieNode,
};
pub use solver::{
    check_ac3, check_ac3_with_slots, solve, solve_with_diagnostics, solve_with_slots,
    solve_with_slots_with_diagnostics, ConstraintDetail, FailureStage, ProgressCallback,
    SlotDiagnostic, Solution, Solver, SolverConfig, SolverDiagnostics, SolverError,
};
pub use tajenka::{
    embed_and_solve_tajenka, embed_tajenka, embed_tajenka_with_solver, select_tajenka_path,
    TajenkaError, TajenkaPath, TajenkaRequest,
};
pub use export::{
    escape_xml, export_grid_to_xml, Exporter, DEFAULT_FORM, DEFAULT_NUMBER_TABLE, DEFAULT_STYLE,
};
pub use html::{export_grid_to_html, HtmlClue, HtmlExportOptions};
pub use layout::{generate_asymmetric_black_cells, get_valid_line_patterns, LayoutConfig};

