use std::io::Cursor;
use std::path::Path;

use crossword_core::{
    embed_tajenka, solve, Cell, ClueCell, CzechLetter, Dictionary, Exporter, Grid,
    SolverConfig, TajenkaRequest,
};

/// Reads `reference/reference.xml`.
fn load_reference_xml() -> String {
    let xml_path = if Path::new("reference/reference.xml").exists() {
        "reference/reference.xml"
    } else {
        "../reference/reference.xml"
    };
    std::fs::read_to_string(xml_path).expect("Failed to read reference.xml")
}

#[test]
fn test_roundtrip_reference_xml_export_unfilled() {
    let xml = load_reference_xml();
    let grid = Grid::from_xml(&xml).expect("Failed to parse reference.xml");

    // Export back to XML
    let exported_xml = grid.to_xml();

    // Verify key elements and structure
    assert!(exported_xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(exported_xml.contains("<CwdStudio type=\"crossword\" comment=\"\" dictionary=\"cs_demo.dict\">"));
    assert!(exported_xml.contains("<Puzzle width=\"8\" height=\"8\" langLocaleId=\"cs\">"));
    assert!(exported_xml.contains("<NumberTable word=\"\" table=\"\" />"));
    assert!(exported_xml.contains("<Style shapeBorders=\"true\""));
    assert!(exported_xml.contains("<Form dictionary=\"cs_demo.dict\" />"));
    assert!(exported_xml.contains("<Preview>"));

    // Parse the exported XML back into a Grid
    let reparsed = Grid::from_xml(&exported_xml).expect("Reparsing exported XML must succeed");

    assert_eq!(reparsed.width, grid.width);
    assert_eq!(reparsed.height, grid.height);
    assert_eq!(reparsed.lang, grid.lang);
    assert_eq!(
        reparsed.template_meta.dictionary,
        grid.template_meta.dictionary
    );

    // Verify all cells match exactly
    for y in 0..grid.height {
        for x in 0..grid.width {
            let orig = grid.cell(x, y).unwrap();
            let rep = reparsed.cell(x, y).unwrap();
            assert_eq!(
                orig, rep,
                "Cell mismatch at ({}, {}): orig={:?}, reparsed={:?}",
                x, y, orig, rep
            );
        }
    }
}

#[test]
fn test_solve_embed_and_export_roundtrip() {
    // 1. Build a 4x4 crossword with horizontal and vertical crossing slots
    // Horizontal row 1: (0, 1), (1, 1), (2, 1), (3, 1) [len 4]
    // Vertical col 0:   (0, 0), (0, 1), (0, 2), (0, 3) [len 4]
    // Clue cell at (0, 0)? No, let's put clue cells adjacent:
    // We create a grid:
    // (0, 0) = Symbol
    // Row 1: (0, 1) ClueCell with textHorizontal="Moravská metropole"
    // (1, 1)..(4, 1) = BlankCells (len 4)
    // Col 2: (2, 0) ClueCell with textVertical="Obuv"
    // (2, 1)..(2, 4) = BlankCells (len 4) crossing Row 1 at (2, 1)!
    let mut grid = Grid::new(5, 5);
    for y in 0..5 {
        for x in 0..5 {
            grid.set_cell(x, y, Cell::Symbol).unwrap();
        }
    }

    // Horizontal slot across row 1: clue at (0, 1), blanks at (1, 1)..(4, 1)
    grid.set_cell(
        0,
        1,
        Cell::Clue(ClueCell {
            text_h: Some("Moravská metropole".to_string()),
            text_v: None,
        }),
    )
    .unwrap();
    for x in 1..=4 {
        grid.set_cell(x, 1, Cell::Blank(None)).unwrap();
    }

    // Vertical slot down col 2: clue at (2, 0), blanks at (2, 1)..(2, 4)
    grid.set_cell(
        2,
        0,
        Cell::Clue(ClueCell {
            text_h: None,
            text_v: Some("Zemní olej".to_string()),
        }),
    )
    .unwrap();
    for y in 2..=4 {
        grid.set_cell(2, y, Cell::Blank(None)).unwrap();
    }

    let slots = grid.extract_slots();
    assert_eq!(slots.len(), 2);
    let h_slot = slots.iter().find(|s| s.direction == crossword_core::Direction::Horizontal).unwrap();
    let v_slot = slots.iter().find(|s| s.direction == crossword_core::Direction::Vertical).unwrap();
    assert_eq!(h_slot.length, 4);
    assert_eq!(v_slot.length, 4);

    // Dictionary with:
    // H: BRNO (B, R, N, O) -> cell (2, 1) is pos 1 in H -> letter 'R'!
    // V: ROPA (R, O, P, A) -> cell (2, 1) is pos 0 in V -> letter 'R'!
    // Both share letter 'R' at (2, 1)!
    let dict_tsv = "\
BRNO\tMoravská metropole
ROPA\tZemní olej
";
    let dict = Dictionary::from_reader(Cursor::new(dict_tsv)).unwrap();

    // 2. Embed tajenka "B" at cell (1, 1) (explicit path)
    let req = TajenkaRequest::from_phrase_with_path("B", vec![(1, 1)]).unwrap();
    let path = embed_tajenka(&mut grid, &dict, &req).expect("Embedding tajenka failed");
    assert_eq!(path.text(), "B");

    // 3. Solve grid
    let config = SolverConfig::new();
    let solution = solve(&grid, &dict, &config).expect("Solving must succeed");
    solution.apply_to_grid(&mut grid, &slots);

    // 4. Export to XML
    let xml = Exporter::to_xml(&grid);

    // Assert that the XML contains expected serialized structures
    assert!(xml.contains("<SolutionCell x=\"1\" y=\"1\" letter=\"B\">"));
    assert!(xml.contains("<index>1</index>"));
    assert!(xml.contains("<BlankCell x=\"2\" y=\"1\" letter=\"R\" />"));
    assert!(xml.contains("<BlankCell x=\"3\" y=\"1\" letter=\"N\" />"));
    assert!(xml.contains("<BlankCell x=\"4\" y=\"1\" letter=\"O\" />"));
    assert!(xml.contains("textHorizontal=\"Moravská metropole\""));
    assert!(xml.contains("textVertical=\"Zemní olej\""));

    // 5. Parse back into a Grid
    let reparsed = Grid::from_xml(&xml).expect("Reparsing solved XML must succeed");

    // Verify cell contents match
    match reparsed.cell(1, 1).unwrap() {
        Cell::Solution(Some(l), idx) => {
            assert_eq!(*l, CzechLetter::B);
            assert_eq!(*idx, 1);
        }
        other => panic!("Expected SolutionCell at (1, 1), got {:?}", other),
    }

    assert_eq!(
        *reparsed.cell(2, 1).unwrap(),
        Cell::Blank(Some(CzechLetter::R))
    );
    assert_eq!(
        *reparsed.cell(3, 1).unwrap(),
        Cell::Blank(Some(CzechLetter::N))
    );
    assert_eq!(
        *reparsed.cell(4, 1).unwrap(),
        Cell::Blank(Some(CzechLetter::O))
    );

    match reparsed.cell(2, 0).unwrap() {
        Cell::Clue(clue) => {
            assert_eq!(clue.text_v.as_deref(), Some("Zemní olej"));
        }
        other => panic!("Expected ClueCell at (2, 0), got {:?}", other),
    }
}
