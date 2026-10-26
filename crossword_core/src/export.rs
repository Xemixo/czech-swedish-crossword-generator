//! CwdStudio XML exporter — writes a filled grid back to the CwdStudio XML format.
//!
//! # Overview
//!
//! The exporter regenerates a complete, compliant `<CwdStudio>` XML document matching
//! section 9 of `PROJECT.md`:
//!
//! 1. Reconstructs root `<CwdStudio>` and `<Puzzle>` elements.
//! 2. Emits all grid cells sorted deterministically by `(y, x)`:
//!    - `<BlankCell x="..." y="..." letter="..." />`
//!    - `<ClueCell x="..." y="..." textHorizontal="..." textVertical="..." />`
//!    - `<SymbolCell x="..." y="..." />`
//!    - `<SolutionCell x="..." y="..." letter="...">\n        <index>...</index>\n      </SolutionCell>`
//! 3. Preserves verbatim original `<NumberTable>`, `<Style>`, `<Form>`, and `<Preview>`
//!    blocks from [`crate::grid::TemplateMetadata`], or provides standard fallbacks.
//! 4. Ensures UTF-8 NFC character normalization and proper XML special character escaping.

use unicode_normalization::UnicodeNormalization;

use crate::grid::{Cell, Grid};
use crate::solver::Solution;

// ---------------------------------------------------------------------------
// Default Fallbacks
// ---------------------------------------------------------------------------

/// Default fallback `<NumberTable>` element if none is present in metadata.
pub const DEFAULT_NUMBER_TABLE: &str = "<NumberTable word=\"\" table=\"\" />";

/// Default fallback `<Form>` element if none is present in metadata.
pub const DEFAULT_FORM: &str = "<Form dictionary=\"cs_demo.dict\" />";

/// Default fallback `<Style>` block matching CwdStudio standard default profile.
pub const DEFAULT_STYLE: &str = r#"<Style shapeBorders="true" cellSpacing="10.0" crosswordBorder="false" crosswordBorderColor="cmyk(0,0,0,100)" crosswordBorderStroke="basic(5.0,1,1)" name="default" multiLingualName="true" defaultProfile="true">
      <textParams horizontalTextMargin="0.05" verticalTextMargin="0.05" shrinkFont="true" shrinkFontLimit="2.5" typographicTouch="true" textHyphenation="true" horizontalTextAlign="2" verticalTextAlign="5" lineSpacingCoef="1.0" tracking="0.0" kerning="true" defaultProfile="false" />
      <clueCellParams middleLineStroke="basic(2.0,0,1)" middleLineColor="cmyk(0,0,0,100)" fontColor="cmyk(0,0,0,100)" font="font(arial,0,12.0)" bgColor="cmyk(0,0,0,34)" shape="roundrect(20.0,20.0)" borderColor="cmyk(0,0,0,100)" borderStroke="basic(2.0,1,1)" defaultProfile="false" />
      <blankCellParams fontColor="cmyk(0,0,0,100)" font="font(arial,0,64.0)" bgColor="cmyk(0,0,0,0)" shape="roundrect(20.0,20.0)" borderColor="cmyk(0,0,0,100)" borderStroke="basic(1.0,1,1)" defaultProfile="false" />
      <helpCellParams fontColor="cmyk(0,0,0,100)" font="font(arial,0,12.0)" bgColor="cmyk(0,0,0,34)" shape="roundrect(20.0,20.0)" borderColor="cmyk(0,0,0,100)" borderStroke="basic(2.0,1,1)" defaultProfile="false" />
      <textCellParams fontColor="cmyk(0,0,0,100)" font="font(arial,0,20.0)" bgColor="cmyk(0,0,0,34)" shape="roundrect(20.0,20.0)" borderColor="cmyk(0,0,0,100)" borderStroke="basic(2.0,1,1)" defaultProfile="false" />
      <solutionCellParams drawSolutionArrow="false" drawSolutionArrowStroke="false" solutionArrowColor="cmyk(0,0,0,100)" arrowSize="0.25" fontColor="cmyk(0,0,0,100)" font="font(arial,1,64.0)" bgColor="cmyk(0,4,42,6)" shape="roundrect(20.0,20.0)" borderColor="cmyk(0,0,0,100)" borderStroke="basic(1.0,1,1)" defaultProfile="false">
        <index font="font(arial,0,14.0)" color="rgb(0,0,0)" showIndexes="true" showCircle="true" circleStroke="basic(1.0,2,0)" circleSize="12" posX="18" posY="18" defaultProfile="false" />
      </solutionCellParams>
      <symbolCellParams glyphCode="153" text="" font="font(wingdings,1,80.0)" color="cmyk(0,0,0,80)" imageFile="" type="1" imageSize="90" bgColor="cmyk(0,0,0,34)" shape="roundrect(20.0,20.0)" borderColor="cmyk(0,0,0,100)" borderStroke="basic(2.0,1,1)" defaultProfile="false" />
      <wordsParams drawArrows="false" drawArrowStroke="false" arrowColor="cmyk(0,0,0,100)" arrowWidth="0.15" arrowHeight="0.15" defaultProfile="false" />
      <numberCwdParams numberFont="font(Arial,0,20.0)" numberColor="rgb(0,0,0)" tableNumberColor="rgb(0,0,0)" tableNumberFont="font(Arial,0,28.0)" tableLetterColor="rgb(0,0,128)" tableLetterFont="font(Arial,1,38.0)" cellStrokeColor="rgb(0,0,0)" cellFill="rgb(255,255,255)" cellStroke="basic(2.0,1,1)" verticalMargin="6" horizontalMargin="10" cellWidth="50" cellHeight="80" cellMargin="20" position="1" defaultProfile="false" />
    </Style>"#;

// ---------------------------------------------------------------------------
// XML Character Escaping & NFC Normalization
// ---------------------------------------------------------------------------

/// Normalizes text to Unicode NFC and escapes XML special characters (`&`, `<`, `>`, `"`, `'`).
pub fn escape_xml(s: &str) -> String {
    let normalized: String = s.nfc().collect();
    let mut escaped = String::with_capacity(normalized.len());
    for c in normalized.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

// ---------------------------------------------------------------------------
// Exporter Implementation
// ---------------------------------------------------------------------------

/// High-level exporter providing XML generation functions.
#[derive(Debug, Clone, Copy, Default)]
pub struct Exporter;

impl Exporter {
    /// Serializes a [`Grid`] to a complete CwdStudio XML document string.
    pub fn to_xml(grid: &Grid) -> String {
        export_grid_to_xml(grid)
    }

    /// Applies a [`Solution`] to a clone of the grid and exports it to CwdStudio XML.
    pub fn export_solution(grid: &Grid, solution: &Solution) -> String {
        let mut solved_grid = grid.clone();
        let slots = solved_grid.extract_slots();
        solution.apply_to_grid(&mut solved_grid, &slots);
        export_grid_to_xml(&solved_grid)
    }
}

/// Converts a [`Grid`] into a complete, well-formed CwdStudio XML document string.
pub fn export_grid_to_xml(grid: &Grid) -> String {
    let mut xml = String::with_capacity(8192);

    // 1. XML declaration
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");

    // 2. <CwdStudio> root element with preserved or default attributes
    let cwd_type = grid
        .template_meta
        .cwd_type
        .as_deref()
        .unwrap_or("crossword");
    let cwd_comment = grid
        .template_meta
        .cwd_comment
        .as_deref()
        .unwrap_or("");
    let dictionary = grid
        .template_meta
        .dictionary
        .as_deref()
        .unwrap_or("cs_demo.dict");

    xml.push_str(&format!(
        "<CwdStudio type=\"{}\" comment=\"{}\" dictionary=\"{}\">\n",
        escape_xml(cwd_type),
        escape_xml(cwd_comment),
        escape_xml(dictionary)
    ));

    // 3. <Puzzle> element
    xml.push_str(&format!(
        "  <Puzzle width=\"{}\" height=\"{}\" langLocaleId=\"{}\">\n",
        grid.width,
        grid.height,
        grid.lang.as_str()
    ));

    // 4. <Cells> element with cells sorted deterministically by (y, x)
    xml.push_str("    <Cells>\n");
    for y in 0..grid.height {
        for x in 0..grid.width {
            let cell = grid.cell(x, y).expect("Coordinate within grid bounds");
            match cell {
                Cell::Blank(None) => {
                    xml.push_str(&format!("      <BlankCell x=\"{}\" y=\"{}\" />\n", x, y));
                }
                Cell::Blank(Some(letter)) => {
                    xml.push_str(&format!(
                        "      <BlankCell x=\"{}\" y=\"{}\" letter=\"{}\" />\n",
                        x,
                        y,
                        escape_xml(letter.to_str())
                    ));
                }
                Cell::Clue(clue) => {
                    let th = clue.text_h.as_deref().unwrap_or("");
                    let tv = clue.text_v.as_deref().unwrap_or("");
                    xml.push_str(&format!(
                        "      <ClueCell x=\"{}\" y=\"{}\" textHorizontal=\"{}\" textVertical=\"{}\" />\n",
                        x,
                        y,
                        escape_xml(th),
                        escape_xml(tv)
                    ));
                }
                Cell::Symbol => {
                    xml.push_str(&format!("      <SymbolCell x=\"{}\" y=\"{}\" />\n", x, y));
                }
                Cell::Solution(letter, index) => {
                    match letter {
                        Some(l) => {
                            xml.push_str(&format!(
                                "      <SolutionCell x=\"{}\" y=\"{}\" letter=\"{}\">\n        <index>{}</index>\n      </SolutionCell>\n",
                                x,
                                y,
                                escape_xml(l.to_str()),
                                index
                            ));
                        }
                        None => {
                            xml.push_str(&format!(
                                "      <SolutionCell x=\"{}\" y=\"{}\">\n        <index>{}</index>\n      </SolutionCell>\n",
                                x,
                                y,
                                index
                            ));
                        }
                    }
                }
            }
        }
    }
    xml.push_str("    </Cells>\n");

    // 5. <NumberTable> element (verbatim from template or default)
    let number_table = grid
        .template_meta
        .number_table_raw
        .as_deref()
        .unwrap_or(DEFAULT_NUMBER_TABLE)
        .trim();
    xml.push_str(&format!("    {}\n", number_table));

    // 6. <Style> element (verbatim from template or default)
    let style = grid
        .template_meta
        .style_raw
        .as_deref()
        .unwrap_or(DEFAULT_STYLE)
        .trim();
    xml.push_str(&format!("    {}\n", style));

    // 7. Close </Puzzle>
    xml.push_str("  </Puzzle>\n");

    // 8. <Form> element (verbatim from template or default)
    let form = grid.template_meta.form_raw.as_deref().map(|f| f.trim().to_string()).unwrap_or_else(|| {
        format!("<Form dictionary=\"{}\" />", escape_xml(dictionary))
    });
    xml.push_str(&format!("  {}\n", form));

    // 9. <Preview> element if present in metadata
    if let Some(ref preview) = grid.template_meta.preview_raw {
        xml.push_str(&format!("  {}\n", preview.trim()));
    }

    // 10. Close </CwdStudio>
    xml.push_str("</CwdStudio>\n");

    xml
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Cell, ClueCell, Grid};
    use crate::letter::CzechLetter;

    #[test]
    fn test_escape_xml() {
        let raw = "Tom & Jerry <friends> \"quote\" 'apostrophe'";
        let escaped = escape_xml(raw);
        assert_eq!(
            escaped,
            "Tom &amp; Jerry &lt;friends&gt; &quot;quote&quot; &apos;apostrophe&apos;"
        );
    }

    #[test]
    fn test_nfc_normalization() {
        // Combining character sequence: 'c' + caron U+030C -> NFC 'č' U+010D
        let decomposed = "c\u{030c}";
        let escaped = escape_xml(decomposed);
        assert_eq!(escaped, "č");
    }

    #[test]
    fn test_export_minimal_grid_roundtrip() {
        let mut grid = Grid::new(2, 2);
        grid.set_cell(0, 0, Cell::Symbol).unwrap();
        grid.set_cell(
            1,
            0,
            Cell::Clue(ClueCell {
                text_h: Some("Vodorovně & vpravo".to_string()),
                text_v: None,
            }),
        )
        .unwrap();
        grid.set_cell(0, 1, Cell::Blank(Some(CzechLetter::A))).unwrap();
        grid.set_cell(1, 1, Cell::Solution(Some(CzechLetter::Ch), 1)).unwrap();

        let xml = Exporter::to_xml(&grid);
        assert!(xml.contains("<CwdStudio"));
        assert!(xml.contains("<SymbolCell x=\"0\" y=\"0\" />"));
        assert!(xml.contains("textHorizontal=\"Vodorovně &amp; vpravo\""));
        assert!(xml.contains("<BlankCell x=\"0\" y=\"1\" letter=\"A\" />"));
        assert!(xml.contains("<SolutionCell x=\"1\" y=\"1\" letter=\"CH\">"));
        assert!(xml.contains("<index>1</index>"));

        // Parse back from exported XML
        let parsed = Grid::from_xml(&xml).expect("Exported XML must parse back cleanly");
        assert_eq!(parsed.width, 2);
        assert_eq!(parsed.height, 2);
        assert_eq!(*parsed.cell(0, 0).unwrap(), Cell::Symbol);
        match parsed.cell(1, 0).unwrap() {
            Cell::Clue(c) => {
                assert_eq!(c.text_h.as_deref(), Some("Vodorovně & vpravo"));
                assert!(c.text_v.is_none());
            }
            other => panic!("Expected ClueCell, got {:?}", other),
        }
        assert_eq!(*parsed.cell(0, 1).unwrap(), Cell::Blank(Some(CzechLetter::A)));
        assert_eq!(
            *parsed.cell(1, 1).unwrap(),
            Cell::Solution(Some(CzechLetter::Ch), 1)
        );
    }
}
