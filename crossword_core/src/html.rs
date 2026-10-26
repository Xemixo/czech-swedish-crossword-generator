//! HTML crossword exporter with modern, interactive, and responsive UI.
//!
//! Generates a standalone, beautiful HTML document containing:
//! - Interactive playable crossword grid with cell navigation (arrow keys, auto-advance).
//! - Clear numbered Across (Vodorovně) and Down (Svisle) clue definition lists.
//! - Tajenka (secret solution) tracker with real-time feedback and highlighted cells.
//! - Toggle for Solution view / Player view.
//! - Print-friendly stylesheet (`@media print`).

use std::collections::HashMap;

use crate::grid::{Cell, Direction, Grid, WordSlot};
use crate::solver::Solution;

/// Options for HTML export.
#[derive(Debug, Clone)]
pub struct HtmlExportOptions {
    /// Puzzle title displayed in header.
    pub title: String,
    /// Subtitle or author/dictionary info.
    pub subtitle: String,
    /// If true, initially displays filled letters.
    pub show_solution: bool,
}

impl Default for HtmlExportOptions {
    fn default() -> Self {
        Self {
            title: "Švédská křížovka".to_string(),
            subtitle: "Vygenerováno pomocí cwgen".to_string(),
            show_solution: true,
        }
    }
}

/// Clue item prepared for HTML rendering.
#[derive(Debug, Clone)]
pub struct HtmlClue {
    pub number: u16,
    pub direction: Direction,
    pub length: u8,
    pub start: (u8, u8),
    pub cells: Vec<(u8, u8)>,
    pub word: String,
    pub clue: String,
    pub difficulty: Option<crate::dict::Difficulty>,
    pub has_tajenka: bool,
}

/// Generates a complete, standalone, interactive HTML crossword document.
pub fn export_grid_to_html(
    grid: &Grid,
    slots: &[WordSlot],
    solution: Option<&Solution>,
    options: Option<&HtmlExportOptions>,
) -> String {
    let default_opts = HtmlExportOptions::default();
    let opts = options.unwrap_or(&default_opts);

    // 1. Assign standard crossword numbers to slots and cells
    // Cells that start a horizontal slot or a vertical slot receive a sequence number in reading order.
    let mut cell_numbers: HashMap<(u8, u8), u16> = HashMap::new();
    let mut next_number: u16 = 1;

    for y in 0..grid.height {
        for x in 0..grid.width {
            let coord = (x, y);
            let starts_h = slots.iter().any(|s| s.direction == Direction::Horizontal && s.start == coord);
            let starts_v = slots.iter().any(|s| s.direction == Direction::Vertical && s.start == coord);
            if starts_h || starts_v {
                cell_numbers.insert(coord, next_number);
                next_number += 1;
            }
        }
    }

    // 2. Build Across and Down clue lists with definitions
    let mut across_clues: Vec<HtmlClue> = Vec::new();
    let mut down_clues: Vec<HtmlClue> = Vec::new();

    // Map each cell to its horizontal and vertical slot numbers
    let mut cell_h_slot: HashMap<(u8, u8), u16> = HashMap::new();
    let mut cell_v_slot: HashMap<(u8, u8), u16> = HashMap::new();

    // Also map clue position (cx, cy) to slot number for horizontal and vertical
    let mut clue_pos_h_slot: HashMap<(u8, u8), u16> = HashMap::new();
    let mut clue_pos_v_slot: HashMap<(u8, u8), u16> = HashMap::new();

    for slot in slots {
        let num = *cell_numbers.get(&slot.start).unwrap_or(&0);

        let (word, clue, difficulty) = if let Some(sol) = solution {
            if let Some(entry) = sol.assignments.get(&slot.id) {
                (entry.word_str(), entry.clue.clone(), entry.difficulty)
            } else {
                ("".to_string(), "".to_string(), None)
            }
        } else {
            ("".to_string(), "".to_string(), None)
        };

        let has_tajenka = slot.cells.iter().any(|&(cx, cy)| {
            matches!(grid.cell(cx, cy), Some(Cell::Solution(_, _)))
        });

        for &c in &slot.cells {
            match slot.direction {
                Direction::Horizontal => { cell_h_slot.insert(c, num); }
                Direction::Vertical => { cell_v_slot.insert(c, num); }
            }
        }

        if let Some(pos) = slot.clue_pos {
            match slot.direction {
                Direction::Horizontal => { clue_pos_h_slot.insert(pos, num); }
                Direction::Vertical => { clue_pos_v_slot.insert(pos, num); }
            }
        }

        let item = HtmlClue {
            number: num,
            direction: slot.direction,
            length: slot.length,
            start: slot.start,
            cells: slot.cells.clone(),
            word,
            clue: if clue.is_empty() { "Definice není k dispozici".to_string() } else { clue },
            difficulty,
            has_tajenka,
        };

        match slot.direction {
            Direction::Horizontal => across_clues.push(item),
            Direction::Vertical => down_clues.push(item),
        }
    }

    across_clues.sort_by_key(|c| c.number);
    down_clues.sort_by_key(|c| c.number);

    // 3. Extract Tajenka (secret solution) cells sorted by index
    let mut tajenka_cells: Vec<(u8, (u8, u8), String)> = Vec::new();
    for y in 0..grid.height {
        for x in 0..grid.width {
            if let Some(Cell::Solution(letter_opt, idx)) = grid.cell(x, y) {
                let letter_str = letter_opt.map(|l| format!("{}", l)).unwrap_or_default();
                tajenka_cells.push((*idx, (x, y), letter_str));
            }
        }
    }
    tajenka_cells.sort_by_key(|(idx, _, _)| *idx);
    let secret_phrase: String = tajenka_cells.iter().map(|(_, _, l)| l.as_str()).collect();

    // 4. Construct complete HTML document
    let mut html = String::with_capacity(32768);

    html.push_str("<!DOCTYPE html>\n<html lang=\"cs\">\n<head>\n");
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!("  <title>{}</title>\n", escape_html(&opts.title)));
    html.push_str("  <style>\n");
    html.push_str(HTML_CSS);
    html.push_str("  </style>\n</head>\n<body>\n");

    // Header section
    html.push_str("  <div class=\"app-container\">\n");
    html.push_str("    <header class=\"app-header\">\n");
    html.push_str("      <div class=\"header-left\">\n");
    html.push_str(&format!("        <h1>{}</h1>\n", escape_html(&opts.title)));
    html.push_str(&format!("        <p class=\"subtitle\">{}</p>\n", escape_html(&opts.subtitle)));
    html.push_str("      </div>\n");
    html.push_str("      <div class=\"header-controls\">\n");
    html.push_str("        <button id=\"btn-toggle-solution\" class=\"btn btn-primary\" onclick=\"toggleSolution()\">\n");
    html.push_str("          <span id=\"toggle-text\">Skrýt řešení</span>\n");
    html.push_str("        </button>\n");
    html.push_str("        <button id=\"btn-check\" class=\"btn btn-secondary\" onclick=\"checkAnswers()\">\n");
    html.push_str("          Zkontrolovat\n");
    html.push_str("        </button>\n");
    html.push_str("        <button id=\"btn-clear\" class=\"btn btn-secondary\" onclick=\"clearInputs()\">\n");
    html.push_str("          Vymazat\n");
    html.push_str("        </button>\n");
    html.push_str("        <button id=\"btn-print\" class=\"btn btn-outline\" onclick=\"window.print()\">\n");
    html.push_str("          Tisk\n");
    html.push_str("        </button>\n");
    html.push_str("        <button id=\"btn-zoom-out\" class=\"btn btn-secondary\" onclick=\"changeCellSize(-6)\" title=\"Zmenšit\">\n");
    html.push_str("          -\n");
    html.push_str("        </button>\n");
    html.push_str("        <button id=\"btn-zoom-reset\" class=\"btn btn-secondary\" onclick=\"resetCellSize()\" title=\"Přizpůsobit\">\n");
    html.push_str("          100%\n");
    html.push_str("        </button>\n");
    html.push_str("        <button id=\"btn-zoom-in\" class=\"btn btn-secondary\" onclick=\"changeCellSize(6)\" title=\"Zvětšit\">\n");
    html.push_str("          +\n");
    html.push_str("        </button>\n");
    html.push_str("      </div>\n");
    html.push_str("    </header>\n\n");

    // Tajenka Banner
    if !tajenka_cells.is_empty() {
        html.push_str("    <section class=\"tajenka-banner\" id=\"tajenka-section\">\n");
        html.push_str("      <div class=\"tajenka-header\">\n");
        html.push_str("        <span class=\"tajenka-badge\">Tajenka</span>\n");
        html.push_str("        <span class=\"tajenka-hint\">Doplňte písmena z označených políček křížovky</span>\n");
        html.push_str("      </div>\n");
        html.push_str("      <div class=\"tajenka-boxes\">\n");
        for (idx, coord, letter) in &tajenka_cells {
            html.push_str(&format!(
                "        <div class=\"tajenka-box\" data-idx=\"{}\" data-coord=\"{},{}\" data-solution=\"{}\">\n\
                 <span class=\"t-num\">{}</span>\n\
                 <span class=\"t-val\">{}</span>\n\
                 </div>\n",
                idx, coord.0, coord.1, escape_html(letter), idx, escape_html(letter)
            ));
        }
        html.push_str("      </div>\n");
        html.push_str(&format!(
            "      <div class=\"tajenka-solution-text\" id=\"tajenka-answer\" style=\"display:none;\">Řešení tajenky: <strong>{}</strong></div>\n",
            escape_html(&secret_phrase)
        ));
        html.push_str("    </section>\n\n");
    }

    // Prominent Sticky Active Clue Bar
    html.push_str("    <!-- Prominent Active Clue Banner -->\n");
    html.push_str("    <section class=\"active-clue-bar\" id=\"active-clue-bar\">\n");
    html.push_str("      <div class=\"ac-badge-dir\" id=\"ac-dir-badge\"><span class=\"icon\">→</span> <span id=\"ac-dir-text\">Vodorovně</span></div>\n");
    html.push_str("      <div class=\"ac-num-badge\" id=\"ac-num-badge\">#1</div>\n");
    html.push_str("      <div class=\"ac-text\" id=\"ac-clue-text\">Klikněte na jakékoliv políčko v křížovce pro zobrazení celého zadání</div>\n");
    html.push_str("      <div class=\"ac-meta\" id=\"ac-meta-text\"></div>\n");
    html.push_str("    </section>\n\n");

    // Floating Clue Tooltip / Popover for Magnified Hover
    html.push_str("    <div id=\"clue-floating-tooltip\" class=\"clue-floating-tooltip\" style=\"display: none;\"></div>\n\n");

    // Main layout: Grid + Clues
    html.push_str("    <main class=\"main-content\">\n");

    // Grid Container
    html.push_str("      <section class=\"grid-wrapper\">\n");
    html.push_str(&format!(
        "        <div class=\"crossword-grid\" id=\"grid\" style=\"--cols: {}; --rows: {};\">\n",
        grid.width, grid.height
    ));

    for y in 0..grid.height {
        for x in 0..grid.width {
            let coord = (x, y);
            let cell = grid.cell(x, y).unwrap();
            let cell_num = cell_numbers.get(&coord).copied();
            let h_slot = cell_h_slot.get(&coord).copied();
            let v_slot = cell_v_slot.get(&coord).copied();

            match cell {
                Cell::Symbol => {
                    html.push_str(&format!(
                        "          <div class=\"cell cell-black\" data-x=\"{}\" data-y=\"{}\"></div>\n",
                        x, y
                    ));
                }
                Cell::Clue(clue_cell) => {
                    let has_h = clue_cell.text_h.is_some();
                    let has_v = clue_cell.text_v.is_some();
                    let h_text = clue_cell.text_h.as_deref().unwrap_or("");
                    let v_text = clue_cell.text_v.as_deref().unwrap_or("");

                    let h_target_slot = clue_pos_h_slot.get(&coord).copied();
                    let v_target_slot = clue_pos_v_slot.get(&coord).copied();

                    let h_slot_attr = h_target_slot.map(|n| format!(" data-target-h=\"{}\"", n)).unwrap_or_default();
                    let v_slot_attr = v_target_slot.map(|n| format!(" data-target-v=\"{}\"", n)).unwrap_or_default();

                    if has_h && has_v {
                        let title = format!("→ {}\n↓ {}", h_text, v_text);
                        html.push_str(&format!(
                            "          <div class=\"cell cell-clue cell-clue-split\" data-x=\"{}\" data-y=\"{}\"{}{} data-clue-h=\"{}\" data-clue-v=\"{}\" title=\"{}\">\n\
                             <div class=\"clue-half clue-half-top\" data-clue-dir=\"h\" data-clue=\"{}\"><span class=\"clue-text\">{}</span><span class=\"clue-arrow\">→</span></div>\n\
                             <div class=\"clue-divider\"></div>\n\
                             <div class=\"clue-half clue-half-bottom\" data-clue-dir=\"v\" data-clue=\"{}\"><span class=\"clue-text\">{}</span><span class=\"clue-arrow\">↓</span></div>\n\
                             </div>\n",
                            x, y, h_slot_attr, v_slot_attr, escape_html(h_text), escape_html(v_text), escape_html(&title),
                            escape_html(h_text), escape_html(h_text),
                            escape_html(v_text), escape_html(v_text)
                        ));
                    } else if has_h {
                        let title = format!("→ {}", h_text);
                        html.push_str(&format!(
                            "          <div class=\"cell cell-clue cell-clue-single\" data-x=\"{}\" data-y=\"{}\"{} data-clue-h=\"{}\" title=\"{}\">\n\
                             <div class=\"clue-single\" data-clue-dir=\"h\" data-clue=\"{}\"><span class=\"clue-text\">{}</span><span class=\"clue-arrow\">→</span></div>\n\
                             </div>\n",
                            x, y, h_slot_attr, escape_html(h_text), escape_html(&title), escape_html(h_text), escape_html(h_text)
                        ));
                    } else if has_v {
                        let title = format!("↓ {}", v_text);
                        html.push_str(&format!(
                            "          <div class=\"cell cell-clue cell-clue-single\" data-x=\"{}\" data-y=\"{}\"{} data-clue-v=\"{}\" title=\"{}\">\n\
                             <div class=\"clue-single\" data-clue-dir=\"v\" data-clue=\"{}\"><span class=\"clue-text\">{}</span><span class=\"clue-arrow\">↓</span></div>\n\
                             </div>\n",
                            x, y, v_slot_attr, escape_html(v_text), escape_html(&title), escape_html(v_text), escape_html(v_text)
                        ));
                    } else {
                        html.push_str(&format!(
                            "          <div class=\"cell cell-clue cell-clue-empty\" data-x=\"{}\" data-y=\"{}\"></div>\n",
                            x, y
                        ));
                    }
                }
                Cell::Blank(letter_opt) => {
                    let letter = letter_opt.map(|l| format!("{}", l)).unwrap_or_default();
                    let num_html = cell_num.map(|n| format!("<span class=\"cell-num\">{}</span>", n)).unwrap_or_default();
                    let h_attr = h_slot.map(|n| format!(" data-h=\"{}\"", n)).unwrap_or_default();
                    let v_attr = v_slot.map(|n| format!(" data-v=\"{}\"", n)).unwrap_or_default();

                    html.push_str(&format!(
                        "          <div class=\"cell cell-letter\" data-x=\"{}\" data-y=\"{}\" data-val=\"{}\"{}{}>\n\
                         {}\n\
                         <input type=\"text\" class=\"cell-input\" maxlength=\"2\" autocomplete=\"off\" value=\"{}\" data-solution=\"{}\">\n\
                         </div>\n",
                        x, y, escape_html(&letter), h_attr, v_attr, num_html, escape_html(&letter), escape_html(&letter)
                    ));
                }
                Cell::Solution(letter_opt, tajenka_idx) => {
                    let letter = letter_opt.map(|l| format!("{}", l)).unwrap_or_default();
                    let num_html = cell_num.map(|n| format!("<span class=\"cell-num\">{}</span>", n)).unwrap_or_default();
                    let h_attr = h_slot.map(|n| format!(" data-h=\"{}\"", n)).unwrap_or_default();
                    let v_attr = v_slot.map(|n| format!(" data-v=\"{}\"", n)).unwrap_or_default();

                    html.push_str(&format!(
                        "          <div class=\"cell cell-letter cell-tajenka\" data-x=\"{}\" data-y=\"{}\" data-val=\"{}\" data-tajenka-idx=\"{}\"{}{}>\n\
                         {}\n\
                         <span class=\"cell-tajenka-badge\">{}</span>\n\
                         <input type=\"text\" class=\"cell-input\" maxlength=\"2\" autocomplete=\"off\" value=\"{}\" data-solution=\"{}\">\n\
                         </div>\n",
                        x, y, escape_html(&letter), tajenka_idx, h_attr, v_attr, num_html, tajenka_idx, escape_html(&letter), escape_html(&letter)
                    ));
                }
            }
        }
    }

    html.push_str("        </div>\n");
    html.push_str("      </section>\n\n");

    // Clues Section (Across & Down)
    html.push_str("      <aside class=\"clues-panel\">\n");

    // Across Clues
    html.push_str("        <div class=\"clues-column\">\n");
    html.push_str("          <h2 class=\"clues-title\"><span class=\"icon\">→</span> Vodorovně</h2>\n");
    html.push_str("          <ul class=\"clue-list\" id=\"across-clues\">\n");
    for clue in &across_clues {
        let tajenka_badge = if clue.has_tajenka { "<span class=\"t-pill\">Tajenka</span>" } else { "" };
        let diff_badge = match clue.difficulty {
            Some(crate::dict::Difficulty::Easy) => "<span class=\"diff-pill diff-easy\">Snadná</span>",
            Some(crate::dict::Difficulty::Medium) => "<span class=\"diff-pill diff-medium\">Střední</span>",
            Some(crate::dict::Difficulty::Hard) => "<span class=\"diff-pill diff-hard\">Těžká</span>",
            None => "",
        };
        let cells_attr = clue.cells.iter().map(|c| format!("{},{}", c.0, c.1)).collect::<Vec<_>>().join(";");
        html.push_str(&format!(
            "            <li class=\"clue-item\" data-num=\"{}\" data-dir=\"h\" data-cells=\"{}\">\n\
             <div class=\"clue-meta\">\n\
             <span class=\"clue-num\">{}.</span>\n\
             <span class=\"clue-len\">({} písm.)</span>\n\
             {}{}\n\
             </div>\n\
             <div class=\"clue-text\">{}</div>\n\
             <div class=\"clue-answer\" style=\"display:none;\">Odpověď: <strong>{}</strong></div>\n\
             </li>\n",
            clue.number, cells_attr, clue.number, clue.length, diff_badge, tajenka_badge, escape_html(&clue.clue), escape_html(&clue.word)
        ));
    }
    html.push_str("          </ul>\n");
    html.push_str("        </div>\n\n");

    // Down Clues
    html.push_str("        <div class=\"clues-column\">\n");
    html.push_str("          <h2 class=\"clues-title\"><span class=\"icon\">↓</span> Svisle</h2>\n");
    html.push_str("          <ul class=\"clue-list\" id=\"down-clues\">\n");
    for clue in &down_clues {
        let tajenka_badge = if clue.has_tajenka { "<span class=\"t-pill\">Tajenka</span>" } else { "" };
        let diff_badge = match clue.difficulty {
            Some(crate::dict::Difficulty::Easy) => "<span class=\"diff-pill diff-easy\">Snadná</span>",
            Some(crate::dict::Difficulty::Medium) => "<span class=\"diff-pill diff-medium\">Střední</span>",
            Some(crate::dict::Difficulty::Hard) => "<span class=\"diff-pill diff-hard\">Těžká</span>",
            None => "",
        };
        let cells_attr = clue.cells.iter().map(|c| format!("{},{}", c.0, c.1)).collect::<Vec<_>>().join(";");
        html.push_str(&format!(
            "            <li class=\"clue-item\" data-num=\"{}\" data-dir=\"v\" data-cells=\"{}\">\n\
             <div class=\"clue-meta\">\n\
             <span class=\"clue-num\">{}.</span>\n\
             <span class=\"clue-len\">({} písm.)</span>\n\
             {}{}\n\
             </div>\n\
             <div class=\"clue-text\">{}</div>\n\
             <div class=\"clue-answer\" style=\"display:none;\">Odpověď: <strong>{}</strong></div>\n\
             </li>\n",
            clue.number, cells_attr, clue.number, clue.length, diff_badge, tajenka_badge, escape_html(&clue.clue), escape_html(&clue.word)
        ));
    }
    html.push_str("          </ul>\n");
    html.push_str("        </div>\n");

    html.push_str("      </aside>\n");
    html.push_str("    </main>\n");

    // Footer
    html.push_str("    <footer class=\"app-footer\">\n");
    html.push_str("      <p>Vygenerováno pomocí <strong>cwgen</strong> &bull; Švédská křížovka</p>\n");
    html.push_str("    </footer>\n");
    html.push_str("  </div>\n\n");

    // Interactive JavaScript
    html.push_str("  <script>\n");
    html.push_str(HTML_JS);
    html.push_str("  </script>\n");
    html.push_str("</body>\n</html>\n");

    html
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const HTML_CSS: &str = r#"
:root {
  --primary: #4f46e5;
  --primary-hover: #4338ca;
  --primary-light: #e0e7ff;
  --secondary: #0f172a;
  --accent: #f59e0b;
  --accent-light: #fef3c7;
  --bg-page: #f8fafc;
  --bg-card: #ffffff;
  --border-color: #cbd5e1;
  --text-main: #1e293b;
  --text-muted: #64748b;
  --grid-black: #0f172a;
  --cell-size: 60px;
  --success: #10b981;
  --danger: #ef4444;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  background-color: var(--bg-page);
  color: var(--text-main);
  line-height: 1.5;
  padding: 16px;
  overflow-x: auto;
}

.app-container {
  width: 100%;
  max-width: 100%;
  margin: 0 auto;
}

.app-header {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  background: var(--bg-card);
  padding: 20px 24px;
  border-radius: 16px;
  box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05), 0 2px 4px -2px rgba(0,0,0,0.05);
  margin-bottom: 24px;
}

.header-left h1 {
  font-size: 26px;
  font-weight: 800;
  color: var(--secondary);
  letter-spacing: -0.5px;
}

.header-left .subtitle {
  font-size: 14px;
  color: var(--text-muted);
  margin-top: 2px;
}

.header-controls {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  padding: 10px 16px;
  border-radius: 8px;
  cursor: pointer;
  border: none;
  transition: all 0.15s ease-in-out;
}

.btn-primary {
  background: var(--primary);
  color: #fff;
}
.btn-primary:hover {
  background: var(--primary-hover);
}

.btn-secondary {
  background: #f1f5f9;
  color: var(--secondary);
  border: 1px solid var(--border-color);
}
.btn-secondary:hover {
  background: #e2e8f0;
}

.btn-outline {
  background: transparent;
  color: var(--text-muted);
  border: 1px solid var(--border-color);
}
.btn-outline:hover {
  background: #f8fafc;
  color: var(--text-main);
}

/* Tajenka Section */
.tajenka-banner {
  background: linear-gradient(135deg, #fffbeb 0%, #fef3c7 100%);
  border: 2px solid #fde68a;
  border-radius: 16px;
  padding: 18px 24px;
  margin-bottom: 24px;
  box-shadow: 0 4px 6px -1px rgba(245, 158, 11, 0.1);
}

.tajenka-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.tajenka-badge {
  background: var(--accent);
  color: #fff;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.5px;
  padding: 4px 10px;
  border-radius: 6px;
}

.tajenka-hint {
  font-size: 13px;
  color: #92400e;
  font-weight: 500;
}

.tajenka-boxes {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.tajenka-box {
  width: 46px;
  height: 52px;
  background: #fff;
  border: 2px solid #f59e0b;
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 4px rgba(0,0,0,0.05);
  position: relative;
}

.tajenka-box .t-num {
  font-size: 10px;
  font-weight: 700;
  color: #b45309;
  position: absolute;
  top: 2px;
  left: 4px;
}

.tajenka-box .t-val {
  font-size: 20px;
  font-weight: 800;
  color: #78350f;
  margin-top: 6px;
}

.tajenka-solution-text {
  margin-top: 10px;
  font-size: 14px;
  color: #92400e;
}

/* Prominent Sticky Active Clue Bar */
.active-clue-bar {
  position: sticky;
  top: 12px;
  z-index: 100;
  background: #ffffff;
  border: 2px solid var(--primary);
  border-radius: 14px;
  padding: 14px 20px;
  margin-bottom: 20px;
  display: flex;
  align-items: center;
  gap: 14px;
  box-shadow: 0 10px 25px -5px rgba(79, 70, 229, 0.2), 0 8px 10px -6px rgba(79, 70, 229, 0.15);
  backdrop-filter: blur(8px);
  animation: fadeIn 0.2s ease-in-out;
}

.ac-badge-dir {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: var(--primary);
  color: #fff;
  font-weight: 800;
  font-size: 13px;
  padding: 6px 14px;
  border-radius: 8px;
  letter-spacing: 0.5px;
  flex-shrink: 0;
  text-transform: uppercase;
}

.ac-num-badge {
  background: #e0e7ff;
  color: #3730a3;
  font-weight: 800;
  font-size: 14px;
  padding: 6px 10px;
  border-radius: 8px;
  flex-shrink: 0;
}

.ac-text {
  font-size: 16px;
  font-weight: 600;
  color: var(--secondary);
  line-height: 1.4;
  flex: 1;
}

.ac-meta {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
  background: #f1f5f9;
  padding: 6px 12px;
  border-radius: 8px;
  white-space: nowrap;
}

/* Floating Clue Tooltip / Popover */
.clue-floating-tooltip {
  position: fixed;
  z-index: 1000;
  background: #0f172a;
  color: #f8fafc;
  padding: 12px 16px;
  border-radius: 10px;
  box-shadow: 0 10px 25px -3px rgba(0, 0, 0, 0.4), 0 4px 6px -4px rgba(0, 0, 0, 0.3);
  font-size: 14px;
  line-height: 1.45;
  max-width: 320px;
  pointer-events: none;
  opacity: 0;
  transform: translateY(6px);
  transition: opacity 0.15s ease, transform 0.15s ease;
  border: 1px solid #334155;
}

.clue-floating-tooltip.visible {
  opacity: 1;
  transform: translateY(0);
}

.clue-floating-tooltip .tt-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-bottom: 6px;
}
.clue-floating-tooltip .tt-item:last-child {
  margin-bottom: 0;
}
.clue-floating-tooltip .tt-arrow {
  font-weight: 900;
  color: #38bdf8;
  font-size: 16px;
  line-height: 1.2;
}
.clue-floating-tooltip .tt-text {
  font-weight: 500;
  color: #f1f5f9;
}

/* Main Content Layout */
.main-content {
  display: grid;
  grid-template-columns: minmax(0, auto) minmax(320px, 1fr);
  gap: 24px;
  align-items: start;
}

@media (max-width: 1200px) {
  .main-content {
    grid-template-columns: 1fr;
  }
}

/* Grid Styles */
.grid-wrapper {
  background: var(--bg-card);
  padding: 16px;
  border-radius: 16px;
  box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05);
  display: flex;
  justify-content: flex-start;
  align-items: flex-start;
  overflow-x: auto;
  overflow-y: visible;
  max-width: 100%;
  scrollbar-width: thin;
}

.grid-wrapper::-webkit-scrollbar {
  height: 8px;
}
.grid-wrapper::-webkit-scrollbar-thumb {
  background: #cbd5e1;
  border-radius: 4px;
}

.crossword-grid {
  display: grid;
  grid-template-columns: repeat(var(--cols), var(--cell-size));
  grid-template-rows: repeat(var(--rows), var(--cell-size));
  border: 2px solid var(--secondary);
  background: var(--secondary);
  gap: 1px;
  user-select: none;
  flex-shrink: 0;
  margin: 0 auto;
}

.cell {
  position: relative;
  background: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--cell-size);
  height: var(--cell-size);
}

.cell-black {
  background: var(--grid-black);
}

.cell-clue {
  background: #f8fafc;
  padding: 3px 4px;
  overflow: hidden;
  color: #1e293b;
  text-align: left;
  display: flex;
  flex-direction: column;
  justify-content: center;
  border: 1px solid #cbd5e1;
  transition: all 0.15s ease;
  position: relative;
  cursor: pointer;
}

.cell-clue:hover {
  background: #ffffff;
  border-color: #3b82f6;
  box-shadow: 0 4px 14px rgba(59, 130, 246, 0.25);
  z-index: 20;
}

.cell-clue-split {
  padding: 2px 3px;
}

.clue-half {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 3px;
  overflow: hidden;
  padding: 1px 2px;
  border-radius: 3px;
}

.clue-half:hover {
  background: #e0e7ff;
}

.clue-half .clue-text {
  font-size: 10px;
  font-weight: 500;
  line-height: 1.2;
  color: #1e293b;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  word-break: break-word;
}

.clue-half .clue-arrow {
  font-weight: 900;
  font-size: 13px;
  color: #2563eb;
  flex-shrink: 0;
}

.clue-divider {
  height: 1px;
  background: #cbd5e1;
  width: 100%;
  margin: 1px 0;
}

.clue-single {
  height: 100%;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
  overflow: hidden;
  padding: 2px 3px;
  border-radius: 4px;
}

.clue-single:hover {
  background: #e0e7ff;
}

.clue-single .clue-text {
  font-size: 11px;
  font-weight: 500;
  line-height: 1.22;
  color: #1e293b;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  word-break: break-word;
}

.clue-single .clue-arrow {
  font-weight: 900;
  font-size: 15px;
  color: #2563eb;
  flex-shrink: 0;
}

.cell-letter {
  cursor: text;
}

.cell-num {
  position: absolute;
  top: 2px;
  left: 3px;
  font-size: 11px;
  font-weight: 700;
  color: var(--text-muted);
  line-height: 1;
  pointer-events: none;
  z-index: 2;
}

.cell-tajenka-badge {
  position: absolute;
  top: 2px;
  right: 3px;
  font-size: 10px;
  font-weight: 800;
  color: #b45309;
  background: var(--accent-light);
  border: 1px solid var(--accent);
  border-radius: 3px;
  padding: 0 3px;
  line-height: 1.1;
  pointer-events: none;
  z-index: 2;
}

.cell-tajenka {
  background: #fefce8;
}

.cell-input {
  width: 100%;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  text-align: center;
  font-size: 24px;
  font-weight: 800;
  color: var(--secondary);
  text-transform: uppercase;
  caret-color: var(--primary);
  z-index: 1;
}

.cell.highlight-word {
  background: #e0e7ff !important;
}

.cell.highlight-active {
  background: #c7d2fe !important;
}

.cell.cell-correct input {
  color: var(--success);
}
.cell.cell-incorrect input {
  color: var(--danger);
  background: #fef2f2;
}

/* Clues Panel */
.clues-panel {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
  max-height: 800px;
}

@media (max-width: 640px) {
  .clues-panel {
    grid-template-columns: 1fr;
  }
}

.clues-column {
  background: var(--bg-card);
  border-radius: 16px;
  padding: 20px;
  box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05);
  display: flex;
  flex-direction: column;
  max-height: 700px;
}

.clues-title {
  font-size: 17px;
  font-weight: 800;
  color: var(--secondary);
  margin-bottom: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
  border-bottom: 2px solid #f1f5f9;
  padding-bottom: 10px;
}

.clue-list {
  list-style: none;
  overflow-y: auto;
  padding-right: 6px;
}

.clue-list::-webkit-scrollbar {
  width: 6px;
}
.clue-list::-webkit-scrollbar-thumb {
  background: #cbd5e1;
  border-radius: 3px;
}

.clue-item {
  padding: 10px 12px;
  border-radius: 8px;
  margin-bottom: 8px;
  border: 1px solid transparent;
  cursor: pointer;
  transition: all 0.15s ease;
}

.clue-item:hover {
  background: #f8fafc;
  border-color: #e2e8f0;
}

.clue-item.active-clue {
  background: var(--primary-light);
  border-color: #c7d2fe;
}

.clue-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 2px;
}

.clue-num {
  font-weight: 800;
  color: var(--primary);
  font-size: 14px;
}

.clue-len {
  font-size: 12px;
  color: var(--text-muted);
}

.t-pill {
  font-size: 10px;
  font-weight: 700;
  background: #fef3c7;
  color: #b45309;
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid #fde68a;
}

.diff-pill {
  font-size: 10px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  text-transform: uppercase;
  letter-spacing: 0.02em;
}
.diff-easy {
  background: #dcfce7;
  color: #15803d;
  border: 1px solid #bbf7d0;
}
.diff-medium {
  background: #e0f2fe;
  color: #0369a1;
  border: 1px solid #bae6fd;
}
.diff-hard {
  background: #fee2e2;
  color: #b91c1c;
  border: 1px solid #fecaca;
}

.clue-text {
  font-size: 13px;
  color: #334155;
  line-height: 1.4;
}

.clue-answer {
  font-size: 12px;
  color: #047857;
  margin-top: 4px;
}

.app-footer {
  text-align: center;
  margin-top: 32px;
  font-size: 13px;
  color: var(--text-muted);
}

/* Print Stylesheet */
@media print {
  body {
    background: #fff;
    padding: 0;
  }
  .header-controls, .app-footer {
    display: none !important;
  }
  .main-content {
    display: block;
  }
  .crossword-grid {
    --cell-size: 34px;
    margin: 0 auto 20px auto;
  }
  .clues-panel {
    display: flex;
    max-height: none;
    page-break-before: auto;
  }
  .clues-column {
    max-height: none;
    box-shadow: none;
    border: 1px solid #cbd5e1;
  }
}
"#;

const HTML_JS: &str = r#"
let isSolutionVisible = true;
let activeDirection = 'h'; // 'h' or 'v'

function init() {
  const inputs = document.querySelectorAll('.cell-input');
  
  inputs.forEach(input => {
    input.addEventListener('focus', onCellFocus);
    input.addEventListener('input', onCellInput);
    input.addEventListener('keydown', onCellKeydown);
    input.addEventListener('click', onCellClick);
  });

  const clueItems = document.querySelectorAll('.clue-item');
  clueItems.forEach(item => {
    item.addEventListener('click', onClueClick);
  });

  // Setup Clue Cell interactions (hover tooltip + click to focus target word)
  initClueCells();

  updateTajenkaDisplay();
}

function initClueCells() {
  const tooltip = document.getElementById('clue-floating-tooltip');
  const clueCells = document.querySelectorAll('.cell-clue');

  clueCells.forEach(cell => {
    // Hover popover
    cell.addEventListener('mouseenter', (e) => {
      const clueH = cell.dataset.clueH;
      const clueV = cell.dataset.clueV;
      if (!clueH && !clueV) return;

      let content = '';
      if (clueH) {
        content += `<div class="tt-item"><span class="tt-arrow">→</span><span class="tt-text">${escapeHtml(clueH)}</span></div>`;
      }
      if (clueV) {
        content += `<div class="tt-item"><span class="tt-arrow">↓</span><span class="tt-text">${escapeHtml(clueV)}</span></div>`;
      }

      tooltip.innerHTML = content;
      tooltip.classList.add('visible');
      positionTooltip(e, tooltip);
    });

    cell.addEventListener('mousemove', (e) => {
      positionTooltip(e, tooltip);
    });

    cell.addEventListener('mouseleave', () => {
      tooltip.classList.remove('visible');
    });

    // Click on clue halves or entire single clue cell
    const halves = cell.querySelectorAll('.clue-half');
    if (halves.length > 0) {
      halves.forEach(half => {
        half.addEventListener('click', (e) => {
          e.stopPropagation();
          const dir = half.dataset.clueDir;
          const targetSlot = dir === 'h' ? cell.dataset.targetH : cell.dataset.targetV;
          if (targetSlot) {
            jumpToSlot(targetSlot, dir);
          }
        });
      });
    } else {
      const single = cell.querySelector('.clue-single');
      if (single) {
        single.addEventListener('click', (e) => {
          e.stopPropagation();
          const dir = single.dataset.clueDir;
          const targetSlot = dir === 'h' ? cell.dataset.targetH : cell.dataset.targetV;
          if (targetSlot) {
            jumpToSlot(targetSlot, dir);
          }
        });
      }
    }
  });
}

function positionTooltip(e, tooltip) {
  const x = e.clientX + 16;
  const y = e.clientY + 16;
  const maxRight = window.innerWidth - tooltip.offsetWidth - 16;
  const maxBottom = window.innerHeight - tooltip.offsetHeight - 16;
  tooltip.style.left = Math.min(x, maxRight) + 'px';
  tooltip.style.top = Math.min(y, maxBottom) + 'px';
}

function jumpToSlot(slotNum, dir) {
  activeDirection = dir;
  const clueItem = document.querySelector(`.clue-item[data-num="${slotNum}"][data-dir="${dir}"]`);
  if (clueItem) {
    const cells = clueItem.dataset.cells.split(';').map(c => c.split(',').map(Number));
    if (cells.length > 0) {
      focusCell(cells[0][0], cells[0][1]);
    }
  }
}

function onCellFocus(e) {
  const cell = e.target.closest('.cell');
  highlightWordForCell(cell, activeDirection);
}

function onCellClick(e) {
  const cell = e.target.closest('.cell');
  // Toggle direction if clicking the already active cell
  if (cell.classList.contains('highlight-active')) {
    activeDirection = activeDirection === 'h' ? 'v' : 'h';
  }
  highlightWordForCell(cell, activeDirection);
}

function onCellInput(e) {
  const input = e.target;
  let val = input.value.toUpperCase();
  input.value = val;
  
  updateTajenkaDisplay();

  // Auto-advance to next cell in current word
  if (val.length > 0) {
    moveToNextCell(input, activeDirection, 1);
  }
}

function onCellKeydown(e) {
  const input = e.target;
  const cell = input.closest('.cell');
  const x = parseInt(cell.dataset.x, 10);
  const y = parseInt(cell.dataset.y, 10);

  if (e.key === 'Backspace' && input.value === '') {
    moveToNextCell(input, activeDirection, -1);
    e.preventDefault();
  } else if (e.key === 'ArrowRight') {
    focusCell(x + 1, y);
    e.preventDefault();
  } else if (e.key === 'ArrowLeft') {
    focusCell(x - 1, y);
    e.preventDefault();
  } else if (e.key === 'ArrowDown') {
    focusCell(x, y + 1);
    e.preventDefault();
  } else if (e.key === 'ArrowUp') {
    focusCell(x, y - 1);
    e.preventDefault();
  }
}

function focusCell(x, y) {
  const target = document.querySelector(`.cell[data-x="${x}"][data-y="${y}"] input`);
  if (target) {
    target.focus();
    target.select();
  }
}

function moveToNextCell(currentInput, dir, step) {
  const cell = currentInput.closest('.cell');
  const slotNum = dir === 'h' ? cell.dataset.h : cell.dataset.v;
  if (!slotNum) return;

  const clueItem = document.querySelector(`.clue-item[data-num="${slotNum}"][data-dir="${dir}"]`);
  if (!clueItem) return;

  const cells = clueItem.dataset.cells.split(';').map(c => c.split(',').map(Number));
  const curX = parseInt(cell.dataset.x, 10);
  const curY = parseInt(cell.dataset.y, 10);

  const curIdx = cells.findIndex(c => c[0] === curX && c[1] === curY);
  if (curIdx !== -1) {
    const nextIdx = curIdx + step;
    if (nextIdx >= 0 && nextIdx < cells.length) {
      focusCell(cells[nextIdx][0], cells[nextIdx][1]);
    }
  }
}

function highlightWordForCell(cell, dir) {
  // Clear previous highlights
  document.querySelectorAll('.cell.highlight-word, .cell.highlight-active').forEach(c => {
    c.classList.remove('highlight-word', 'highlight-active');
  });
  document.querySelectorAll('.clue-item.active-clue').forEach(item => {
    item.classList.remove('active-clue');
  });

  if (!cell) return;
  cell.classList.add('highlight-active');

  const slotNum = dir === 'h' ? cell.dataset.h : cell.dataset.v;
  const fallbackNum = dir === 'h' ? cell.dataset.v : cell.dataset.h;
  const actualNum = slotNum || fallbackNum;
  const actualDir = slotNum ? dir : (dir === 'h' ? 'v' : 'h');

  if (!actualNum) return;

  const clueItem = document.querySelector(`.clue-item[data-num="${actualNum}"][data-dir="${actualDir}"]`);
  if (clueItem) {
    clueItem.classList.add('active-clue');
    clueItem.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

    const cells = clueItem.dataset.cells.split(';').map(c => c.split(',').map(Number));
    cells.forEach(([cx, cy]) => {
      const cEl = document.querySelector(`.cell[data-x="${cx}"][data-y="${cy}"]`);
      if (cEl) cEl.classList.add('highlight-word');
    });

    // Update Prominent Sticky Active Clue Bar
    updateActiveClueBanner(actualNum, actualDir, clueItem);
  }
}

function updateActiveClueBanner(num, dir, clueItem) {
  const dirBadge = document.getElementById('ac-dir-badge');
  const numBadge = document.getElementById('ac-num-badge');
  const textEl = document.getElementById('ac-clue-text');
  const metaEl = document.getElementById('ac-meta-text');

  if (!dirBadge || !numBadge || !textEl || !metaEl || !clueItem) return;

  const isH = dir === 'h';
  dirBadge.innerHTML = `<span class="icon">${isH ? '→' : '↓'}</span> <span>${isH ? 'VODOROVNĚ' : 'SVISLE'}</span>`;
  numBadge.textContent = `#${num}`;

  const clueTextEl = clueItem.querySelector('.clue-text');
  const clueLenEl = clueItem.querySelector('.clue-len');
  textEl.textContent = clueTextEl ? clueTextEl.textContent : '';
  metaEl.textContent = clueLenEl ? clueLenEl.textContent : '';
}

function onClueClick(e) {
  const item = e.currentTarget;
  const dir = item.dataset.dir;
  activeDirection = dir;
  
  const cells = item.dataset.cells.split(';').map(c => c.split(',').map(Number));
  if (cells.length > 0) {
    focusCell(cells[0][0], cells[0][1]);
  }
}

function toggleSolution() {
  isSolutionVisible = !isSolutionVisible;
  const btnText = document.getElementById('toggle-text');
  const tajenkaAnswer = document.getElementById('tajenka-answer');
  const clueAnswers = document.querySelectorAll('.clue-answer');

  if (isSolutionVisible) {
    btnText.textContent = 'Skrýt řešení';
    document.querySelectorAll('.cell-input').forEach(input => {
      input.value = input.dataset.solution || '';
    });
    if (tajenkaAnswer) tajenkaAnswer.style.display = 'block';
    clueAnswers.forEach(el => el.style.display = 'block');
  } else {
    btnText.textContent = 'Zobrazit řešení';
    document.querySelectorAll('.cell-input').forEach(input => {
      input.value = '';
      input.closest('.cell').classList.remove('cell-correct', 'cell-incorrect');
    });
    if (tajenkaAnswer) tajenkaAnswer.style.display = 'none';
    clueAnswers.forEach(el => el.style.display = 'none');
  }
  updateTajenkaDisplay();
}

function checkAnswers() {
  document.querySelectorAll('.cell-input').forEach(input => {
    const val = input.value.trim().toUpperCase();
    const sol = (input.dataset.solution || '').trim().toUpperCase();
    const cell = input.closest('.cell');
    
    cell.classList.remove('cell-correct', 'cell-incorrect');
    if (val === '') {
      // blank
    } else if (val === sol) {
      cell.classList.add('cell-correct');
    } else {
      cell.classList.add('cell-incorrect');
    }
  });
}

function clearInputs() {
  document.querySelectorAll('.cell-input').forEach(input => {
    input.value = '';
    input.closest('.cell').classList.remove('cell-correct', 'cell-incorrect');
  });
  updateTajenkaDisplay();
}

function updateTajenkaDisplay() {
  document.querySelectorAll('.tajenka-box').forEach(box => {
    const [x, y] = box.dataset.coord.split(',');
    const cellInput = document.querySelector(`.cell[data-x="${x}"][data-y="${y}"] input`);
    const valEl = box.querySelector('.t-val');
    if (cellInput && valEl) {
      valEl.textContent = cellInput.value || (isSolutionVisible ? box.dataset.solution : '');
    }
  });
}

function changeCellSize(delta) {
  const root = document.documentElement;
  const current = parseInt(getComputedStyle(root).getPropertyValue('--cell-size')) || 64;
  const next = Math.max(36, Math.min(130, current + delta));
  root.style.setProperty('--cell-size', next + 'px');
}

function resetCellSize() {
  const root = document.documentElement;
  root.style.setProperty('--cell-size', '58px');
  const wrapper = document.querySelector('.grid-wrapper');
  if (wrapper) {
    wrapper.scrollLeft = 0;
  }
}

function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
}

document.addEventListener('DOMContentLoaded', init);
"#;

