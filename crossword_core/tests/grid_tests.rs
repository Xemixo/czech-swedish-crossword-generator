use std::path::Path;
use crossword_core::{
    Cell, CzechLetter, Direction, Grid, LangLocale,
};

#[test]
fn test_czech_letter_basic() {
    assert_eq!(CzechLetter::COUNT, 42);
    assert_eq!(CzechLetter::ALL.len(), 42);

    // Diacritic letters
    assert_eq!("A".parse::<CzechLetter>().unwrap(), CzechLetter::A);
    assert_eq!("á".parse::<CzechLetter>().unwrap(), CzechLetter::Á);
    assert_eq!("Á".parse::<CzechLetter>().unwrap(), CzechLetter::Á);
    assert_eq!("č".parse::<CzechLetter>().unwrap(), CzechLetter::Č);
    assert_eq!("Č".parse::<CzechLetter>().unwrap(), CzechLetter::Č);
    assert_eq!("ď".parse::<CzechLetter>().unwrap(), CzechLetter::Ď);
    assert_eq!("é".parse::<CzechLetter>().unwrap(), CzechLetter::É);
    assert_eq!("ě".parse::<CzechLetter>().unwrap(), CzechLetter::Ě);
    assert_eq!("í".parse::<CzechLetter>().unwrap(), CzechLetter::Í);
    assert_eq!("ň".parse::<CzechLetter>().unwrap(), CzechLetter::Ň);
    assert_eq!("ó".parse::<CzechLetter>().unwrap(), CzechLetter::Ó);
    assert_eq!("ř".parse::<CzechLetter>().unwrap(), CzechLetter::Ř);
    assert_eq!("š".parse::<CzechLetter>().unwrap(), CzechLetter::Š);
    assert_eq!("ť".parse::<CzechLetter>().unwrap(), CzechLetter::Ť);
    assert_eq!("ú".parse::<CzechLetter>().unwrap(), CzechLetter::Ú);
    assert_eq!("ů".parse::<CzechLetter>().unwrap(), CzechLetter::Ů);
    assert_eq!("ý".parse::<CzechLetter>().unwrap(), CzechLetter::Ý);
    assert_eq!("ž".parse::<CzechLetter>().unwrap(), CzechLetter::Ž);

    // Digraph CH handling
    assert_eq!("CH".parse::<CzechLetter>().unwrap(), CzechLetter::Ch);
    assert_eq!("ch".parse::<CzechLetter>().unwrap(), CzechLetter::Ch);
    assert_eq!("Ch".parse::<CzechLetter>().unwrap(), CzechLetter::Ch);
    assert_eq!(CzechLetter::Ch.to_str(), "CH");
    assert_eq!(format!("{}", CzechLetter::Ch), "CH");
    assert_eq!(CzechLetter::Ch.as_u8(), 13);
    assert_eq!(CzechLetter::from_u8(13), Some(CzechLetter::Ch));

    // Verify all 42 round-trip through u8
    for (idx, &letter) in CzechLetter::ALL.iter().enumerate() {
        assert_eq!(letter.as_u8(), idx as u8);
        assert_eq!(CzechLetter::from_u8(idx as u8), Some(letter));
    }
    assert_eq!(CzechLetter::from_u8(42), None);

    // Invalid tokens
    assert!(CzechLetter::from_token("").is_err());
    assert!(CzechLetter::from_token("ABC").is_err());
    assert!(CzechLetter::from_token("1").is_err());
}

#[test]
fn test_czech_letter_word_parsing() {
    // CHATA: [CH, A, T, A] (4 letters, not 5)
    let chata = CzechLetter::parse_word("CHATA").unwrap();
    assert_eq!(chata.len(), 4);
    assert_eq!(chata, vec![CzechLetter::Ch, CzechLetter::A, CzechLetter::T, CzechLetter::A]);
    assert_eq!(CzechLetter::format_word(&chata), "CHATA");

    // Case-insensitivity: "chata" -> [CH, A, T, A]
    let chata_lower = CzechLetter::parse_word("chata").unwrap();
    assert_eq!(chata_lower, chata);

    // PLACHTA: [P, L, A, CH, T, A] (6 letters, not 7)
    let plachta = CzechLetter::parse_word("PLACHTA").unwrap();
    assert_eq!(plachta.len(), 6);
    assert_eq!(
        plachta,
        vec![
            CzechLetter::P,
            CzechLetter::L,
            CzechLetter::A,
            CzechLetter::Ch,
            CzechLetter::T,
            CzechLetter::A
        ]
    );
    assert_eq!(CzechLetter::format_word(&plachta), "PLACHTA");

    // NACH: [N, A, CH] (3 letters)
    let nach = CzechLetter::parse_word("NACH").unwrap();
    assert_eq!(nach.len(), 3);
    assert_eq!(nach, vec![CzechLetter::N, CzechLetter::A, CzechLetter::Ch]);

    // ČTYŘI: [Č, T, Y, Ř, I]
    let ctyri = CzechLetter::parse_word("ČTYŘI").unwrap();
    assert_eq!(ctyri.len(), 5);
    assert_eq!(
        ctyri,
        vec![
            CzechLetter::Č,
            CzechLetter::T,
            CzechLetter::Y,
            CzechLetter::Ř,
            CzechLetter::I
        ]
    );

    // ŽLUŤOUČKÝ KŮŇ (full pangram components)
    let zlutoucky = CzechLetter::parse_word("ŽLUŤOUČKÝ").unwrap();
    assert_eq!(zlutoucky.len(), 9);
    assert_eq!(
        zlutoucky,
        vec![
            CzechLetter::Ž,
            CzechLetter::L,
            CzechLetter::U,
            CzechLetter::Ť,
            CzechLetter::O,
            CzechLetter::U,
            CzechLetter::Č,
            CzechLetter::K,
            CzechLetter::Ý
        ]
    );

    let kun = CzechLetter::parse_word("KŮŇ").unwrap();
    assert_eq!(kun.len(), 3);
    assert_eq!(kun, vec![CzechLetter::K, CzechLetter::Ů, CzechLetter::Ň]);

    // Decomposed NFD unicode: "C\u{030c}tyr\u{030c}i"
    let nfd = "C\u{030c}tyr\u{030c}i";
    let nfd_parsed = CzechLetter::parse_word(nfd).unwrap();
    assert_eq!(nfd_parsed, ctyri);
}

#[test]
fn test_reference_xml_parsing() {
    let xml_path = if Path::new("reference/reference.xml").exists() {
        "reference/reference.xml"
    } else {
        "../reference/reference.xml"
    };

    let xml = std::fs::read_to_string(xml_path).expect("Failed to read reference.xml");
    let grid = Grid::from_xml(&xml).expect("Failed to parse reference.xml");

    // Assert dimensions and locale
    assert_eq!(grid.width, 8);
    assert_eq!(grid.height, 8);
    assert_eq!(grid.lang, LangLocale::Cs);
    assert_eq!(grid.template_meta.dictionary.as_deref(), Some("cs_demo.dict"));

    // (0,0) is SymbolCell
    assert_eq!(*grid.cell(0, 0).unwrap(), Cell::Symbol);

    // Top row (1..7, 0) are ClueCells
    for x in 1..8 {
        match grid.cell(x, 0).unwrap() {
            Cell::Clue(clue) => {
                // In empty reference.xml, clue text is None (empty string)
                assert!(clue.text_h.is_none());
                assert!(clue.text_v.is_none());
            }
            other => panic!("Expected ClueCell at ({}, 0), got {:?}", x, other),
        }
    }

    // Left col (0, 1..7) are ClueCells
    for y in 1..8 {
        match grid.cell(0, y).unwrap() {
            Cell::Clue(_) => {}
            other => panic!("Expected ClueCell at (0, {}), got {:?}", y, other),
        }
    }

    // Interior (1..7, 1..7) are BlankCells (7x7 = 49 cells)
    let mut blank_count = 0;
    for y in 1..8 {
        for x in 1..8 {
            assert_eq!(*grid.cell(x, y).unwrap(), Cell::Blank(None));
            blank_count += 1;
        }
    }
    assert_eq!(blank_count, 49);
}

#[test]
fn test_slot_extraction_reference() {
    let xml_path = if Path::new("reference/reference.xml").exists() {
        "reference/reference.xml"
    } else {
        "../reference/reference.xml"
    };

    let xml = std::fs::read_to_string(xml_path).expect("Failed to read reference.xml");
    let grid = Grid::from_xml(&xml).expect("Failed to parse reference.xml");

    let slots = grid.extract_slots();

    // 7 horizontal slots + 7 vertical slots = 14 slots
    assert_eq!(slots.len(), 14);

    let h_slots: Vec<_> = slots.iter().filter(|s| s.direction == Direction::Horizontal).collect();
    let v_slots: Vec<_> = slots.iter().filter(|s| s.direction == Direction::Vertical).collect();

    assert_eq!(h_slots.len(), 7);
    assert_eq!(v_slots.len(), 7);

    // Horizontal slots: rows 1..=7, length 7, starting at (1, y), clue at (0, y)
    for (i, slot) in h_slots.iter().enumerate() {
        let expected_y = (i + 1) as u8;
        assert_eq!(slot.start, (1, expected_y));
        assert_eq!(slot.length, 7);
        assert_eq!(slot.clue_pos, Some((0, expected_y)));
        assert_eq!(slot.cells.len(), 7);
        for (pos, &(cx, cy)) in slot.cells.iter().enumerate() {
            assert_eq!(cx, (pos + 1) as u8);
            assert_eq!(cy, expected_y);
        }
    }

    // Vertical slots: cols 1..=7, length 7, starting at (x, 1), clue at (x, 0)
    for (i, slot) in v_slots.iter().enumerate() {
        let expected_x = (i + 1) as u8;
        assert_eq!(slot.start, (expected_x, 1));
        assert_eq!(slot.length, 7);
        assert_eq!(slot.clue_pos, Some((expected_x, 0)));
        assert_eq!(slot.cells.len(), 7);
        for (pos, &(cx, cy)) in slot.cells.iter().enumerate() {
            assert_eq!(cx, expected_x);
            assert_eq!(cy, (pos + 1) as u8);
        }
    }
}

#[test]
fn test_intersections_reference() {
    let xml_path = if Path::new("reference/reference.xml").exists() {
        "reference/reference.xml"
    } else {
        "../reference/reference.xml"
    };

    let xml = std::fs::read_to_string(xml_path).expect("Failed to read reference.xml");
    let grid = Grid::from_xml(&xml).expect("Failed to parse reference.xml");

    let slots = grid.extract_slots();
    let intersections = grid.find_intersections();

    // 7 horizontal slots x 7 vertical slots = 49 intersections
    assert_eq!(intersections.len(), 49);

    for isect in &intersections {
        let h = &slots[isect.h_slot];
        let v = &slots[isect.v_slot];

        assert_eq!(h.direction, Direction::Horizontal);
        assert_eq!(v.direction, Direction::Vertical);

        let h_cell = h.cells[isect.h_pos as usize];
        let v_cell = v.cells[isect.v_pos as usize];

        // The intersecting cell must be identical in both slots
        assert_eq!(h_cell, v_cell);
    }
}

#[test]
fn test_solution_cell_slot_extraction() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CwdStudio type="crossword">
  <Puzzle width="4" height="3" langLocaleId="cs">
    <Cells>
      <SymbolCell x="0" y="0" />
      <SymbolCell x="1" y="0" />
      <ClueCell x="2" y="0" textHorizontal="" textVertical="Dolu" />
      <SymbolCell x="3" y="0" />
      <ClueCell x="0" y="1" textHorizontal="Napoveda" textVertical="" />
      <BlankCell x="1" y="1" />
      <SolutionCell x="2" y="1" index="1" />
      <BlankCell x="3" y="1" />
      <SymbolCell x="0" y="2" />
      <SymbolCell x="1" y="2" />
      <BlankCell x="2" y="2" />
      <SymbolCell x="3" y="2" />
    </Cells>
  </Puzzle>
</CwdStudio>
"#;

    let grid = Grid::from_xml(xml).expect("Failed to parse custom XML");
    assert_eq!(grid.width, 4);
    assert_eq!(grid.height, 3);

    // (2, 1) should be SolutionCell
    assert_eq!(*grid.cell(2, 1).unwrap(), Cell::Solution(None, 1));
    assert!(grid.cell(2, 1).unwrap().is_letter_cell());

    let slots = grid.extract_slots();
    assert_eq!(slots.len(), 2);

    // Horizontal slot across row 1: length 3 at (1, 1), (2, 1), (3, 1)
    let h = slots.iter().find(|s| s.direction == Direction::Horizontal).unwrap();
    assert_eq!(h.start, (1, 1));
    assert_eq!(h.length, 3);
    assert_eq!(h.clue_pos, Some((0, 1)));
    assert_eq!(h.cells, vec![(1, 1), (2, 1), (3, 1)]);

    // Vertical slot across col 2: length 2 at (2, 1), (2, 2)
    let v = slots.iter().find(|s| s.direction == Direction::Vertical).unwrap();
    assert_eq!(v.start, (2, 1));
    assert_eq!(v.length, 2);
    assert_eq!(v.clue_pos, Some((2, 0)));
    assert_eq!(v.cells, vec![(2, 1), (2, 2)]);

    // Intersection at (2, 1)
    let isects = Grid::find_intersections_for_slots(&slots);
    assert_eq!(isects.len(), 1);
    assert_eq!(isects[0].h_slot, h.id);
    assert_eq!(isects[0].h_pos, 1); // cell (2, 1) is at index 1 of h
    assert_eq!(isects[0].v_slot, v.id);
    assert_eq!(isects[0].v_pos, 0); // cell (2, 1) is at index 0 of v
}

#[test]
fn test_black_cells_placement() {
    let xml_path = if Path::new("reference/reference.xml").exists() {
        "reference/reference.xml"
    } else {
        "../reference/reference.xml"
    };

    let xml = std::fs::read_to_string(xml_path).expect("Failed to read reference.xml");
    let mut grid = Grid::from_xml(&xml).expect("Failed to parse reference.xml");

    // Initially 8x8 grid has 49 blanks and 14 slots of length 7
    assert_eq!(grid.extract_slots().len(), 14);

    // Apply auto black cells
    let placed = grid.apply_auto_black_cells().expect("Failed to apply auto black cells");
    assert!(!placed.is_empty(), "Should place black cells to ease generation");

    // All slots must be valid word lengths between 2 and 8
    let slots = grid.extract_slots();
    assert!(!slots.is_empty());
    for s in &slots {
        assert!(s.length >= 2 && s.length <= 8, "Slot length was invalid: {}", s.length);
    }
}

#[test]
fn test_asymmetric_black_cells_10x10() {
    let xml_path = if Path::new("reference/template_10x10.xml").exists() {
        "reference/template_10x10.xml"
    } else {
        "../reference/template_10x10.xml"
    };

    let xml = std::fs::read_to_string(xml_path).expect("Failed to read template_10x10.xml");
    let mut grid = Grid::from_xml(&xml).expect("Failed to parse template_10x10.xml");

    let placed = grid.apply_auto_black_cells().expect("Failed to apply auto black cells");
    assert!(!placed.is_empty(), "Should place black cells");

    // All slots must be between 2 and 9 (bounded by grid dimension), encouraging longer words
    let slots = grid.extract_slots();
    assert!(!slots.is_empty());
    let mut long_word_count = 0;
    for s in &slots {
        assert!(s.length >= 2 && s.length <= 9, "Slot length was invalid: {}", s.length);
        if s.length >= 5 {
            long_word_count += 1;
        }
    }
    assert!(long_word_count > 0, "Should generate plenty of longer words (>= 5 letters)");
}
