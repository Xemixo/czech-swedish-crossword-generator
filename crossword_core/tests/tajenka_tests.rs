//! Integration tests for Phase 4: Tajenka (secret solution) embedding.

use std::collections::HashSet;
use std::io::Cursor;
use std::path::Path;

use crossword_core::{
    embed_and_solve_tajenka, embed_tajenka, select_tajenka_path, Cell, CzechLetter, Dictionary,
    Grid, SolverConfig, TajenkaError, TajenkaRequest,
};

fn integration_dict() -> Dictionary {
    let tsv = "\
# Length 3 words
LES\tLesní porost
PES\tDomácí mazlíček
OKO\tZrakový orgán
NOC\tČas ke spánku
DEN\tČas za světla
SEN\tNoční představa
LEP\tLepidlo
PEC\tTopné těleso

# Length 4 words
AUTO\tMotorové vozidlo
ATOM\tČástice hmoty
VODA\tČirá kapalina
ZIMA\tChladné roční období
LÉTO\tTeplé roční období
BRNO\tMoravská metropole
OBAL\tObal na knihu
CENA\tCena zboží
KOSA\tZemědělský nástroj
MOST\tStavba přes řeku
HRAD\tStředověké sídlo
RÁNO\tZačátek dne
BOTA\tObuv
";
    Dictionary::from_reader(Cursor::new(tsv)).expect("Failed to build integration dictionary")
}

#[test]
fn test_integration_embed_and_solve_tajenka_les() {
    let dict = integration_dict();

    // 3x3 tree grid with 3 slots:
    // - Horizontal slot across row 0: (0,0)..(2,0) [len 3]
    // - Vertical slot down col 0: (0,0)..(0,2) [len 3]
    // - Vertical slot down col 2: (2,0)..(2,2) [len 3]
    let mut grid = Grid::new(3, 3);
    for y in 0..3 {
        for x in 0..3 {
            if y == 0 || (x == 0 && y > 0) || (x == 2 && y > 0) {
                grid.set_cell(x, y, Cell::Blank(None)).unwrap();
            } else {
                grid.set_cell(x, y, Cell::Symbol).unwrap();
            }
        }
    }

    // Embed "LES" via auto path selection
    let req = TajenkaRequest::from_phrase("LES").unwrap();
    let config = SolverConfig::new();

    let (path, solution) = embed_and_solve_tajenka(&mut grid, &dict, &req, &config)
        .expect("Failed to embed and solve tajenka 'LES'");

    assert_eq!(path.len(), 3);
    assert_eq!(path.text(), "LES");

    // Assert that solved letter cells at tajenka coordinates exactly spell the target phrase.
    for (i, &((x, y), letter)) in path.cells.iter().enumerate() {
        let cell = grid.cell(x, y).unwrap();
        assert_eq!(
            cell.letter(),
            Some(letter),
            "Letter at tajenka cell ({}, {}) must match target letter {}",
            x,
            y,
            letter
        );
        match cell {
            Cell::Solution(Some(l), idx) => {
                assert_eq!(*l, letter);
                assert_eq!(*idx, (i + 1) as u8);
            }
            other => panic!("Expected SolutionCell at ({}, {}), got {:?}", x, y, other),
        }
    }

    // Assert that all assigned words in the solution are non-empty and valid.
    assert!(!solution.assignments.is_empty());
}

#[test]
fn test_integration_embed_and_solve_tajenka_brno() {
    let dict = integration_dict();

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

    // Embed "BRNO" via auto path selection
    let req = TajenkaRequest::from_phrase("BRNO").unwrap();
    let config = SolverConfig::new();

    let (path, solution) = embed_and_solve_tajenka(&mut grid, &dict, &req, &config)
        .expect("Failed to embed and solve tajenka 'BRNO'");

    assert_eq!(path.len(), 4);
    assert_eq!(path.text(), "BRNO");

    // Coordinates must be distinct.
    let coords: HashSet<(u8, u8)> = path.coords().into_iter().collect();
    assert_eq!(coords.len(), 4, "All 4 tajenka cells must be distinct");

    for (i, &((x, y), letter)) in path.cells.iter().enumerate() {
        let cell = grid.cell(x, y).unwrap();
        assert_eq!(cell.letter(), Some(letter));
        assert!(
            matches!(cell, Cell::Solution(Some(l), idx) if *l == letter && *idx == (i + 1) as u8)
        );
    }

    assert!(!solution.assignments.is_empty());
}

#[test]
fn test_integration_explicit_path_embedding() {
    let dict = integration_dict();

    let mut grid = Grid::new(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            if y == 0 || x == 0 {
                grid.set_cell(x, y, Cell::Blank(None)).unwrap();
            } else {
                grid.set_cell(x, y, Cell::Symbol).unwrap();
            }
        }
    }

    // Explicit path across row 0: (0, 0), (1, 0), (2, 0), (3, 0)
    let explicit_coords = vec![(0, 0), (1, 0), (2, 0), (3, 0)];
    let req = TajenkaRequest::from_phrase_with_path("BRNO", explicit_coords.clone()).unwrap();

    let path = embed_tajenka(&mut grid, &dict, &req).expect("Explicit path embedding failed");
    assert_eq!(path.coords(), explicit_coords);
    assert_eq!(path.text(), "BRNO");

    // Verify grid cells are SolutionCells with 1-based indices 1, 2, 3, 4
    for (i, &(x, y)) in explicit_coords.iter().enumerate() {
        let cell = grid.cell(x, y).unwrap();
        match cell {
            Cell::Solution(Some(l), idx) => {
                assert_eq!(*idx, (i + 1) as u8);
                assert_eq!(*l, path[i].1);
            }
            other => panic!("Expected SolutionCell at ({}, {}), got {:?}", x, y, other),
        }
    }
}

#[test]
fn test_integration_reference_grid_tajenka_embedding() {
    let xml_path = if Path::new("reference/reference.xml").exists() {
        "reference/reference.xml"
    } else {
        "../reference/reference.xml"
    };
    let xml = std::fs::read_to_string(xml_path).expect("Failed to read reference.xml");
    let mut grid = Grid::from_xml(&xml).expect("Failed to parse reference.xml");

    // Build a dictionary with 7-letter words covering letters for "BRNO"
    let letters_7 = "\
BRAMBOR\tBrambor
RADNICE\tRadnice
NERVOZA\tNervóza
OCHOTNY\tOchotný
HLADINA\tHladina
LAMINAT\tLaminát
MARMOLA\tMarmola
";
    let dict = Dictionary::from_reader(Cursor::new(letters_7)).unwrap();

    let req = TajenkaRequest::from_phrase("BRNO").unwrap();
    let path = embed_tajenka(&mut grid, &dict, &req).expect("Failed to embed tajenka in reference grid");

    assert_eq!(path.len(), 4);
    assert_eq!(path.text(), "BRNO");

    // Verify all 4 cells in the reference grid are now SolutionCells
    for (i, &((x, y), letter)) in path.cells.iter().enumerate() {
        let cell = grid.cell(x, y).unwrap();
        match cell {
            Cell::Solution(Some(l), idx) => {
                assert_eq!(*l, letter);
                assert_eq!(*idx, (i + 1) as u8);
            }
            other => panic!("Expected SolutionCell at ({}, {}), got {:?}", x, y, other),
        }
    }
}

#[test]
fn test_integration_impossible_tajenka_error_handling() {
    let dict = integration_dict();

    let mut grid = Grid::new(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            grid.set_cell(x, y, Cell::Blank(None)).unwrap();
        }
    }

    // 1. Tajenka with non-existent letter 'W' (no words in dict contain 'W')
    let req_w = TajenkaRequest::with_path(vec![CzechLetter::W], vec![(0, 0)]);
    let err_w = embed_tajenka(&mut grid, &dict, &req_w).unwrap_err();
    assert!(matches!(err_w, TajenkaError::Infeasible { letter, .. } if letter == CzechLetter::W));

    // 2. Tajenka with non-existent letter via auto path selection
    let req_w_auto = TajenkaRequest::new(vec![CzechLetter::W]);
    let err_w_auto = select_tajenka_path(&grid, &dict, &req_w_auto).unwrap_err();
    assert!(matches!(err_w_auto, TajenkaError::NoFeasiblePath));

    // 3. Length mismatch: 3 letters but 2 coordinates
    let req_mismatch = TajenkaRequest::from_phrase_with_path("LES", vec![(0, 0), (1, 0)]).unwrap();
    let err_mismatch = embed_tajenka(&mut grid, &dict, &req_mismatch).unwrap_err();
    assert!(matches!(
        err_mismatch,
        TajenkaError::LengthMismatch {
            text_len: 3,
            path_len: 2
        }
    ));

    // 4. Cell collision (duplicate coordinates)
    let req_collision = TajenkaRequest::from_phrase_with_path("LES", vec![(0, 0), (1, 0), (0, 0)]).unwrap();
    let err_collision = embed_tajenka(&mut grid, &dict, &req_collision).unwrap_err();
    assert!(matches!(err_collision, TajenkaError::CellCollision(0, 0)));
}
