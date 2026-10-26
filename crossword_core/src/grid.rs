use std::fmt;
use std::str::FromStr;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::letter::{CzechLetter, CzechLetterError};

/// Grid and XML parsing errors.
#[derive(Error, Debug)]
pub enum GridError {
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("Missing attribute '{0}' on element <{1}>")]
    MissingAttribute(String, String),
    #[error("Invalid integer in attribute '{0}': {1}")]
    InvalidInt(String, std::num::ParseIntError),
    #[error("Invalid cell coordinate ({0}, {1}) for grid of size {2}x{3}")]
    OutOfBounds(u8, u8, u8, u8),
    #[error("Missing <Puzzle> element in XML")]
    MissingPuzzleElement,
    #[error("Czech letter error: {0}")]
    Letter(#[from] CzechLetterError),
    #[error("Grid error: {0}")]
    Other(String),
}

/// Slot identifier type.
pub type SlotId = usize;

/// Word direction in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Horizontal => write!(f, "Horizontal"),
            Direction::Vertical => write!(f, "Vertical"),
        }
    }
}

/// Language locale for the crossword puzzle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LangLocale {
    #[default]
    Cs,
    Other(String),
}

impl FromStr for LangLocale {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("cs") {
            Ok(LangLocale::Cs)
        } else {
            Ok(LangLocale::Other(s.to_string()))
        }
    }
}

impl LangLocale {
    /// Returns the string slice representation of the locale code (e.g. "cs").
    pub fn as_str(&self) -> &str {
        match self {
            LangLocale::Cs => "cs",
            LangLocale::Other(s) => s.as_str(),
        }
    }
}


impl fmt::Display for LangLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LangLocale::Cs => write!(f, "cs"),
            LangLocale::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Clue box data containing directional clue texts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ClueCell {
    /// Clue text for the rightward (horizontal) word slot.
    pub text_h: Option<String>,
    /// Clue text for the downward (vertical) word slot.
    pub text_v: Option<String>,
}

/// A cell in the crossword grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cell {
    /// Unfilled or filled standard letter cell.
    Blank(Option<CzechLetter>),
    /// Shaded clue cell with text/arrows.
    Clue(ClueCell),
    /// Decorative/logo cell.
    Symbol,
    /// Highlighted secret solution (tajenka) letter cell with 1-based or 0-based index.
    Solution(Option<CzechLetter>, u8),
}

impl Cell {
    /// Returns true if this cell can be filled with a letter (Blank or Solution).
    #[inline]
    pub fn is_letter_cell(&self) -> bool {
        matches!(self, Cell::Blank(_) | Cell::Solution(_, _))
    }

    /// Returns the letter in this cell if filled.
    #[inline]
    pub fn letter(&self) -> Option<CzechLetter> {
        match self {
            Cell::Blank(l) | Cell::Solution(l, _) => *l,
            _ => None,
        }
    }

    /// Sets or clears the letter in this cell (no-op for Clue and Symbol).
    #[inline]
    pub fn set_letter(&mut self, letter: Option<CzechLetter>) {
        match self {
            Cell::Blank(l) => *l = letter,
            Cell::Solution(l, _) => *l = letter,
            _ => {}
        }
    }
}

/// A contiguous word slot in the grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordSlot {
    /// Unique identifier for this slot within the grid.
    pub id: SlotId,
    /// Direction of the word (Horizontal or Vertical).
    pub direction: Direction,
    /// `(col, row)` of the first letter cell.
    pub start: (u8, u8),
    /// Total number of letter cells in this slot (Czech "CH" counts as 1 cell).
    pub length: u8,
    /// `(col, row)` of the preceding clue box, if adjacent.
    pub clue_pos: Option<(u8, u8)>,
    /// Coordinates `(col, row)` of all constituent letter cells.
    pub cells: Vec<(u8, u8)>,
    /// Assigned word letters, if filled.
    pub assigned: Option<Vec<CzechLetter>>,
}

/// An intersection point where a horizontal word slot and vertical word slot overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Intersection {
    /// ID of the horizontal slot.
    pub h_slot: SlotId,
    /// 0-based index of the intersecting letter within the horizontal slot.
    pub h_pos: u8,
    /// ID of the vertical slot.
    pub v_slot: SlotId,
    /// 0-based index of the intersecting letter within the vertical slot.
    pub v_pos: u8,
}

/// Preserved XML metadata blocks for exact CwdStudio export round-trips.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TemplateMetadata {
    pub cwd_type: Option<String>,
    pub cwd_comment: Option<String>,
    pub dictionary: Option<String>,
    pub style_raw: Option<String>,
    pub number_table_raw: Option<String>,
    pub form_raw: Option<String>,
    pub preview_raw: Option<String>,
}

/// 2D crossword grid representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    pub width: u8,
    pub height: u8,
    /// Cells indexed by `[row][col]`, i.e. `cells[y as usize][x as usize]`.
    pub cells: Vec<Vec<Cell>>,
    pub lang: LangLocale,
    pub template_meta: TemplateMetadata,
}

impl Grid {
    /// Creates a new grid with the given dimensions initialized with blank cells.
    pub fn new(width: u8, height: u8) -> Self {
        let cells = vec![vec![Cell::Blank(None); width as usize]; height as usize];
        Self {
            width,
            height,
            cells,
            lang: LangLocale::Cs,
            template_meta: TemplateMetadata::default(),
        }
    }

    /// Creates a standard Swedish-style crossword grid template of the given dimensions:
    /// - (0, 0) is a Symbol cell (corner logo/block)
    /// - (1..width, 0) are Clue cells pointing downward into each vertical slot
    /// - (0, 1..height) are Clue cells pointing rightward into each horizontal slot
    /// - (1..width, 1..height) are Blank playable letter cells
    pub fn new_swedish_template(width: u8, height: u8) -> Self {
        let mut grid = Self::new(width, height);
        let _ = grid.set_cell(0, 0, Cell::Symbol);
        for x in 1..width {
            let _ = grid.set_cell(
                x,
                0,
                Cell::Clue(ClueCell {
                    text_h: None,
                    text_v: Some(String::new()),
                }),
            );
        }
        for y in 1..height {
            let _ = grid.set_cell(
                0,
                y,
                Cell::Clue(ClueCell {
                    text_h: Some(String::new()),
                    text_v: None,
                }),
            );
        }
        grid
    }

    /// Returns a reference to the cell at `(x, y)` (`(col, row)`), or `None` if out of bounds.
    #[inline]
    pub fn cell(&self, x: u8, y: u8) -> Option<&Cell> {
        if x < self.width && y < self.height {
            Some(&self.cells[y as usize][x as usize])
        } else {
            None
        }
    }

    /// Returns a mutable reference to the cell at `(x, y)`, or `None` if out of bounds.
    #[inline]
    pub fn cell_mut(&mut self, x: u8, y: u8) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            Some(&mut self.cells[y as usize][x as usize])
        } else {
            None
        }
    }

    /// Sets the cell at `(x, y)`.
    pub fn set_cell(&mut self, x: u8, y: u8, cell: Cell) -> Result<(), GridError> {
        if x < self.width && y < self.height {
            self.cells[y as usize][x as usize] = cell;
            Ok(())
        } else {
            Err(GridError::OutOfBounds(x, y, self.width, self.height))
        }
    }

    /// Sets the cell at `(x, y)` as a black/block cell.
    /// In Swedish crosswords, if the cell borders the start of a horizontal word to its right
    /// or a vertical word below it, it acts as a ClueCell to hold definitions and arrows.
    pub fn set_black_cell(&mut self, x: u8, y: u8) -> Result<(), GridError> {
        self.set_black_cells(&[(x, y)])
    }

    /// Sets multiple cells as black / clue block cells.
    /// In Swedish crosswords, cells that border the start of a horizontal word to their right
    /// or a vertical word below them act as ClueCells to hold their definitions and arrows (→, ↓).
    /// Cells that do not border any word starts remain solid Symbol cells.
    pub fn set_black_cells(&mut self, coords: &[(u8, u8)]) -> Result<(), GridError> {
        let coord_set: std::collections::HashSet<(u8, u8)> = coords.iter().copied().collect();

        for &(x, y) in coords {
            if x >= self.width || y >= self.height {
                return Err(GridError::OutOfBounds(x, y, self.width, self.height));
            }

            // Check if a horizontal word (length >= 2) starts at (x + 1, y)
            let mut starts_h = false;
            if x + 1 < self.width && !coord_set.contains(&(x + 1, y)) {
                let mut run_len = 0;
                let mut cx = x + 1;
                while cx < self.width && !coord_set.contains(&(cx, y)) {
                    if let Some(cell) = self.cell(cx, y) {
                        if cell.is_letter_cell() {
                            run_len += 1;
                            cx += 1;
                            continue;
                        }
                    }
                    break;
                }
                if run_len >= 2 {
                    starts_h = true;
                }
            }

            // Check if a vertical word (length >= 2) starts at (x, y + 1)
            let mut starts_v = false;
            if y + 1 < self.height && !coord_set.contains(&(x, y + 1)) {
                let mut run_len = 0;
                let mut cy = y + 1;
                while cy < self.height && !coord_set.contains(&(x, cy)) {
                    if let Some(cell) = self.cell(x, cy) {
                        if cell.is_letter_cell() {
                            run_len += 1;
                            cy += 1;
                            continue;
                        }
                    }
                    break;
                }
                if run_len >= 2 {
                    starts_v = true;
                }
            }

            if starts_h || starts_v {
                self.set_cell(x, y, Cell::Clue(ClueCell { text_h: None, text_v: None }))?;
            } else {
                self.set_cell(x, y, Cell::Symbol)?;
            }
        }
        Ok(())
    }

    /// Automatically places asymmetric black cells inside the letter area of the grid
    /// to divide large solid blocks into natural, solvable longer words (lengths 4..7)
    /// without creating any isolated (length 1 or 2) slots and relieving intersection density.
    pub fn apply_auto_black_cells(&mut self) -> Result<Vec<(u8, u8)>, GridError> {
        let config = crate::layout::LayoutConfig::default();
        self.apply_auto_black_cells_with_config(&config)
    }

    /// Automatically places asymmetric black cells using a custom `LayoutConfig`.
    pub fn apply_auto_black_cells_with_config(
        &mut self,
        config: &crate::layout::LayoutConfig,
    ) -> Result<Vec<(u8, u8)>, GridError> {
        // Collect any pre-existing solution or symbol cells to treat as forbidden
        let mut forbidden = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(cell) = self.cell(x, y) {
                    if matches!(cell, Cell::Solution(_, _)) {
                        forbidden.push((x, y));
                    }
                }
            }
        }

        let coords = crate::layout::generate_asymmetric_black_cells(self, &forbidden, config)?;
        self.set_black_cells(&coords)?;
        Ok(coords)
    }

    /// Exports this grid into a complete CwdStudio XML document string.
    pub fn to_xml(&self) -> String {
        crate::export::export_grid_to_xml(self)
    }

    /// Renders an ASCII box-drawing representation of the grid for terminal display.
    pub fn to_ascii(&self) -> String {
        let mut out = String::new();
        let col_width = 5;

        // Top border
        out.push('+');
        for _ in 0..self.width {
            out.push_str(&"-".repeat(col_width));
            out.push('+');
        }
        out.push('\n');

        for y in 0..self.height {
            out.push('|');
            for x in 0..self.width {
                let cell_str = match self.cell(x, y).unwrap() {
                    Cell::Blank(None) => " . ".to_string(),
                    Cell::Blank(Some(l)) => format!(" {} ", l),
                    Cell::Clue(clue) => {
                        let has_h = clue.text_h.is_some();
                        let has_v = clue.text_v.is_some();
                        if has_h && has_v {
                            "> v".to_string()
                        } else if has_h {
                            " -> ".to_string()
                        } else if has_v {
                            " |v ".to_string()
                        } else {
                            "[?]".to_string()
                        }
                    }
                    Cell::Symbol => "###".to_string(),
                    Cell::Solution(Some(l), idx) => format!("{}*{}", l, idx),
                    Cell::Solution(None, idx) => format!(" *{}", idx),
                };
                let formatted = format!("{:^5}", cell_str);
                out.push_str(&formatted);
                out.push('|');
            }
            out.push('\n');

            // Row separator
            out.push('+');
            for _ in 0..self.width {
                out.push_str(&"-".repeat(col_width));
                out.push('+');
            }
            out.push('\n');
        }

        out
    }

    /// Extracts all contiguous word slots (length >= 2) from the grid.
    ///
    /// Horizontal slots are enumerated first (row-by-row), followed by vertical slots (col-by-col).
    pub fn extract_slots(&self) -> Vec<WordSlot> {
        let mut slots = Vec::new();
        let mut next_id: SlotId = 0;

        // 1. Horizontal slots (row-by-row)
        for y in 0..self.height {
            let mut x = 0;
            while x < self.width {
                if self.cells[y as usize][x as usize].is_letter_cell() {
                    let start_x = x;
                    let mut cells = Vec::new();
                    while x < self.width && self.cells[y as usize][x as usize].is_letter_cell() {
                        cells.push((x, y));
                        x += 1;
                    }
                    let length = cells.len() as u8;
                    if length >= 2 {
                        let clue_pos = if start_x > 0 {
                            let left_cell = &self.cells[y as usize][(start_x - 1) as usize];
                            if matches!(left_cell, Cell::Clue(_)) {
                                Some((start_x - 1, y))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        slots.push(WordSlot {
                            id: next_id,
                            direction: Direction::Horizontal,
                            start: (start_x, y),
                            length,
                            clue_pos,
                            cells,
                            assigned: None,
                        });
                        next_id += 1;
                    }
                } else {
                    x += 1;
                }
            }
        }

        // 2. Vertical slots (column-by-column)
        for x in 0..self.width {
            let mut y = 0;
            while y < self.height {
                if self.cells[y as usize][x as usize].is_letter_cell() {
                    let start_y = y;
                    let mut cells = Vec::new();
                    while y < self.height && self.cells[y as usize][x as usize].is_letter_cell() {
                        cells.push((x, y));
                        y += 1;
                    }
                    let length = cells.len() as u8;
                    if length >= 2 {
                        let clue_pos = if start_y > 0 {
                            let top_cell = &self.cells[(start_y - 1) as usize][x as usize];
                            if matches!(top_cell, Cell::Clue(_)) {
                                Some((x, start_y - 1))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        slots.push(WordSlot {
                            id: next_id,
                            direction: Direction::Vertical,
                            start: (x, start_y),
                            length,
                            clue_pos,
                            cells,
                            assigned: None,
                        });
                        next_id += 1;
                    }
                } else {
                    y += 1;
                }
            }
        }

        slots
    }

    /// Finds all intersecting pairs of word slots in the grid.
    pub fn find_intersections(&self) -> Vec<Intersection> {
        let slots = self.extract_slots();
        Self::find_intersections_for_slots(&slots)
    }

    /// Finds all intersections given an already-extracted list of word slots.
    pub fn find_intersections_for_slots(slots: &[WordSlot]) -> Vec<Intersection> {
        let mut intersections = Vec::new();
        let h_slots: Vec<&WordSlot> = slots.iter().filter(|s| s.direction == Direction::Horizontal).collect();
        let v_slots: Vec<&WordSlot> = slots.iter().filter(|s| s.direction == Direction::Vertical).collect();

        for h in &h_slots {
            for v in &v_slots {
                let (h_start_x, h_y) = h.start;
                let (v_x, v_start_y) = v.start;

                // Check if vertical slot's column falls within horizontal slot's x-span
                // AND horizontal slot's row falls within vertical slot's y-span
                if v_x >= h_start_x
                    && v_x < h_start_x + h.length
                    && h_y >= v_start_y
                    && h_y < v_start_y + v.length
                {
                    let h_pos = v_x - h_start_x;
                    let v_pos = h_y - v_start_y;
                    intersections.push(Intersection {
                        h_slot: h.id,
                        h_pos,
                        v_slot: v.id,
                        v_pos,
                    });
                }
            }
        }

        intersections
    }

    /// Parses a CwdStudio XML grid template into a `Grid`.
    pub fn from_xml(xml: &str) -> Result<Self, GridError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut width: Option<u8> = None;
        let mut height: Option<u8> = None;
        let mut lang = LangLocale::Cs;
        let mut meta = TemplateMetadata {
            number_table_raw: extract_tag_block(xml, "NumberTable"),
            style_raw: extract_tag_block(xml, "Style"),
            form_raw: extract_tag_block(xml, "Form"),
            preview_raw: extract_tag_block(xml, "Preview"),
            ..TemplateMetadata::default()
        };

        let mut buf = Vec::new();
        let mut cells: Option<Vec<Vec<Cell>>> = None;

        let mut in_solution_cell: Option<(u8, u8, Option<CzechLetter>, u8)> = None;

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let name = e.local_name();
                    match name.as_ref() {
                        b"CwdStudio" => {
                            for attr in e.attributes() {
                                let attr = attr.map_err(quick_xml::Error::from)?;
                                match attr.key.as_ref() {
                                    b"type" => {
                                        meta.cwd_type = Some(String::from_utf8_lossy(&attr.value).to_string());
                                    }
                                    b"comment" => {
                                        meta.cwd_comment = Some(String::from_utf8_lossy(&attr.value).to_string());
                                    }
                                    b"dictionary" => {
                                        meta.dictionary = Some(String::from_utf8_lossy(&attr.value).to_string());
                                    }
                                    _ => {}
                                }
                            }
                        }
                        b"Puzzle" => {
                            for attr in e.attributes() {
                                let attr = attr.map_err(quick_xml::Error::from)?;
                                match attr.key.as_ref() {
                                    b"width" => {
                                        let val = String::from_utf8_lossy(&attr.value);
                                        width = Some(val.parse::<u8>().map_err(|err| GridError::InvalidInt("width".to_string(), err))?);
                                    }
                                    b"height" => {
                                        let val = String::from_utf8_lossy(&attr.value);
                                        height = Some(val.parse::<u8>().map_err(|err| GridError::InvalidInt("height".to_string(), err))?);
                                    }
                                    b"langLocaleId" => {
                                        let val = String::from_utf8_lossy(&attr.value);
                                        lang = val.parse().unwrap_or(LangLocale::Cs);
                                    }
                                    _ => {}
                                }
                            }
                            if let (Some(w), Some(h)) = (width, height) {
                                cells = Some(vec![vec![Cell::Blank(None); w as usize]; h as usize]);
                            }
                        }
                        b"BlankCell" => {
                            if let Some(ref mut grid_cells) = cells {
                                let (x, y, letter) = parse_blank_cell_attrs(e)?;
                                validate_coords(x, y, width.unwrap(), height.unwrap())?;
                                grid_cells[y as usize][x as usize] = Cell::Blank(letter);
                            }
                        }
                        b"ClueCell" => {
                            if let Some(ref mut grid_cells) = cells {
                                let (x, y, clue) = parse_clue_cell_attrs(e)?;
                                validate_coords(x, y, width.unwrap(), height.unwrap())?;
                                grid_cells[y as usize][x as usize] = Cell::Clue(clue);
                            }
                        }
                        b"SymbolCell" => {
                            if let Some(ref mut grid_cells) = cells {
                                let (x, y) = parse_coords(e, "SymbolCell")?;
                                validate_coords(x, y, width.unwrap(), height.unwrap())?;
                                grid_cells[y as usize][x as usize] = Cell::Symbol;
                            }
                        }
                        b"SolutionCell" => {
                            let (x, y, letter, index) = parse_solution_cell_attrs(e)?;
                            in_solution_cell = Some((x, y, letter, index));
                        }
                        b"index" if in_solution_cell.is_some() => {
                            for attr in e.attributes() {
                                let attr = attr.map_err(quick_xml::Error::from)?;
                                if attr.key.as_ref() == b"value" {
                                    let val = String::from_utf8_lossy(&attr.value);
                                    if let Ok(idx) = val.parse::<u8>() {
                                        if let Some((_, _, _, ref mut cur_idx)) = in_solution_cell {
                                            *cur_idx = idx;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::Empty(ref e) => {
                    let name = e.local_name();
                    match name.as_ref() {
                        b"Form" => {
                            for attr in e.attributes() {
                                let attr = attr.map_err(quick_xml::Error::from)?;
                                if attr.key.as_ref() == b"dictionary" && meta.dictionary.is_none() {
                                    meta.dictionary = Some(String::from_utf8_lossy(&attr.value).to_string());
                                }
                            }
                        }
                        b"BlankCell" => {
                            if let Some(ref mut grid_cells) = cells {
                                let (x, y, letter) = parse_blank_cell_attrs(e)?;
                                validate_coords(x, y, width.unwrap(), height.unwrap())?;
                                grid_cells[y as usize][x as usize] = Cell::Blank(letter);
                            }
                        }
                        b"ClueCell" => {
                            if let Some(ref mut grid_cells) = cells {
                                let (x, y, clue) = parse_clue_cell_attrs(e)?;
                                validate_coords(x, y, width.unwrap(), height.unwrap())?;
                                grid_cells[y as usize][x as usize] = Cell::Clue(clue);
                            }
                        }
                        b"SymbolCell" => {
                            if let Some(ref mut grid_cells) = cells {
                                let (x, y) = parse_coords(e, "SymbolCell")?;
                                validate_coords(x, y, width.unwrap(), height.unwrap())?;
                                grid_cells[y as usize][x as usize] = Cell::Symbol;
                            }
                        }
                        b"SolutionCell" => {
                            if let Some(ref mut grid_cells) = cells {
                                let (x, y, letter, index) = parse_solution_cell_attrs(e)?;
                                validate_coords(x, y, width.unwrap(), height.unwrap())?;
                                grid_cells[y as usize][x as usize] = Cell::Solution(letter, index);
                            }
                        }
                        _ => {}
                    }
                }
                Event::Text(ref e) if in_solution_cell.is_some() => {
                    let txt = e.unescape()?.trim().to_string();
                    if let Ok(idx) = txt.parse::<u8>() {
                        if let Some((_, _, _, ref mut cur_idx)) = in_solution_cell {
                            *cur_idx = idx;
                        }
                    }
                }
                Event::End(ref e) => {
                    let name = e.local_name();
                    match name.as_ref() {
                        b"SolutionCell" => {
                            if let Some((x, y, letter, index)) = in_solution_cell.take() {
                                if let Some(ref mut grid_cells) = cells {
                                    validate_coords(x, y, width.unwrap(), height.unwrap())?;
                                    grid_cells[y as usize][x as usize] = Cell::Solution(letter, index);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        let width = width.ok_or(GridError::MissingPuzzleElement)?;
        let height = height.ok_or(GridError::MissingPuzzleElement)?;
        let cells = cells.ok_or(GridError::MissingPuzzleElement)?;

        Ok(Self {
            width,
            height,
            cells,
            lang,
            template_meta: meta,
        })
    }
}

fn validate_coords(x: u8, y: u8, w: u8, h: u8) -> Result<(), GridError> {
    if x < w && y < h {
        Ok(())
    } else {
        Err(GridError::OutOfBounds(x, y, w, h))
    }
}

fn parse_coords(e: &quick_xml::events::BytesStart<'_>, tag: &str) -> Result<(u8, u8), GridError> {
    let mut x = None;
    let mut y = None;
    for attr in e.attributes() {
        let attr = attr.map_err(quick_xml::Error::from)?;
        match attr.key.as_ref() {
            b"x" => {
                let s = String::from_utf8_lossy(&attr.value);
                x = Some(s.parse::<u8>().map_err(|err| GridError::InvalidInt("x".to_string(), err))?);
            }
            b"y" => {
                let s = String::from_utf8_lossy(&attr.value);
                y = Some(s.parse::<u8>().map_err(|err| GridError::InvalidInt("y".to_string(), err))?);
            }
            _ => {}
        }
    }
    let x = x.ok_or_else(|| GridError::MissingAttribute("x".to_string(), tag.to_string()))?;
    let y = y.ok_or_else(|| GridError::MissingAttribute("y".to_string(), tag.to_string()))?;
    Ok((x, y))
}

fn parse_blank_cell_attrs(e: &quick_xml::events::BytesStart<'_>) -> Result<(u8, u8, Option<CzechLetter>), GridError> {
    let (x, y) = parse_coords(e, "BlankCell")?;
    let mut letter = None;
    for attr in e.attributes() {
        let attr = attr.map_err(quick_xml::Error::from)?;
        if attr.key.as_ref() == b"letter" || attr.key.as_ref() == b"value" {
            let s = String::from_utf8_lossy(&attr.value);
            if !s.is_empty() {
                letter = Some(CzechLetter::from_token(&s)?);
            }
        }
    }
    Ok((x, y, letter))
}

fn parse_clue_cell_attrs(e: &quick_xml::events::BytesStart<'_>) -> Result<(u8, u8, ClueCell), GridError> {
    let (x, y) = parse_coords(e, "ClueCell")?;
    let mut text_h = None;
    let mut text_v = None;
    for attr in e.attributes() {
        let attr = attr.map_err(quick_xml::Error::from)?;
        match attr.key.as_ref() {
            b"textHorizontal" => {
                let unescaped = attr.unescape_value().map_err(quick_xml::Error::from)?;
                let s = unescaped.trim().to_string();
                if !s.is_empty() {
                    text_h = Some(s);
                }
            }
            b"textVertical" => {
                let unescaped = attr.unescape_value().map_err(quick_xml::Error::from)?;
                let s = unescaped.trim().to_string();
                if !s.is_empty() {
                    text_v = Some(s);
                }
            }
            _ => {}
        }
    }
    Ok((x, y, ClueCell { text_h, text_v }))
}

fn parse_solution_cell_attrs(e: &quick_xml::events::BytesStart<'_>) -> Result<(u8, u8, Option<CzechLetter>, u8), GridError> {
    let (x, y) = parse_coords(e, "SolutionCell")?;
    let mut letter = None;
    let mut index = 0;
    for attr in e.attributes() {
        let attr = attr.map_err(quick_xml::Error::from)?;
        match attr.key.as_ref() {
            b"letter" | b"value" => {
                let s = String::from_utf8_lossy(&attr.value);
                if !s.is_empty() {
                    letter = Some(CzechLetter::from_token(&s)?);
                }
            }
            b"index" => {
                let s = String::from_utf8_lossy(&attr.value);
                if let Ok(idx) = s.parse::<u8>() {
                    index = idx;
                }
            }
            _ => {}
        }
    }
    Ok((x, y, letter, index))
}

fn extract_tag_block(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}", tag);
    let start_idx = xml.find(&start_tag)?;
    let remainder = &xml[start_idx..];

    if let Some(first_gt) = remainder.find('>') {
        if remainder[..first_gt].trim_end().ends_with('/') {
            return Some(remainder[..=first_gt].to_string());
        }
    }

    let end_tag = format!("</{}>", tag);
    let end_idx = remainder.find(&end_tag)?;
    Some(remainder[..end_idx + end_tag.len()].to_string())
}
