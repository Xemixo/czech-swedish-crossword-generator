use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};

use crossword_core::{
    embed_tajenka, export_grid_to_html, solve_with_diagnostics, Dictionary,
    DifficultyChoice, Grid, HtmlExportOptions, LayoutConfig, SolverConfig, TajenkaRequest,
};

/// Swedish Crossword Generator & Solver (cwgen)
#[derive(Parser, Debug)]
#[command(
    name = "cwgen",
    author,
    version,
    about = "Swedish Crossword (Švédská křížovka) procedural generator and solver"
)]
pub struct CliArgs {
    /// Optional grid dimensions positional argument, e.g. "10x10", "12x10"
    #[arg(value_name = "SIZE", index = 1)]
    pub size: Option<String>,

    /// Optional tajenka / secret solution positional argument
    #[arg(value_name = "SECRET", index = 2)]
    pub secret_pos: Option<String>,

    /// Path to input grid XML template (optional)
    #[arg(short = 't', long = "template")]
    pub template: Option<PathBuf>,

    /// Grid dimensions flag, e.g. "--size 10x10" or "-S 10x10"
    #[arg(short = 'S', long = "size")]
    pub size_flag: Option<String>,

    /// Explicit grid width in cells
    #[arg(short = 'W', long = "width")]
    pub width: Option<u8>,

    /// Explicit grid height in cells
    #[arg(short = 'H', long = "height")]
    pub height: Option<u8>,

    /// Path to word-clue dictionary (TSV/text format)
    #[arg(short = 'd', long = "dictionary", default_value = "reference/word_difficulty.tsv")]
    pub dictionary: PathBuf,

    /// Preferred definition difficulty: easy (snadna), medium (stredni), hard (tezka), all/mix
    #[arg(short = 'D', long = "difficulty")]
    pub difficulty: Option<String>,

    /// Tajenka / secret solution string flag
    #[arg(short = 's', long = "secret")]
    pub secret: Option<String>,

    /// Optional manual coordinates of black cells inside grid, e.g. "1,1;7,1;4,4"
    #[arg(short = 'b', long = "black-cells")]
    pub black_cells: Option<String>,

    /// Automatically place asymmetric black cells inside the grid
    #[arg(short = 'a', long = "auto-black-cells")]
    pub auto_black_cells: bool,

    /// Minimum allowed word length (default: 2)
    #[arg(long = "min-len", default_value_t = 2)]
    pub min_len: u8,

    /// Maximum allowed word length (default: 7)
    #[arg(long = "max-len", default_value_t = 7)]
    pub max_len: u8,

    /// Optional random seed for layout variety
    #[arg(long = "seed")]
    pub seed: Option<u64>,

    /// Output path for the solved XML file
    #[arg(short = 'o', long = "output")]
    pub output: Option<PathBuf>,

    /// Output path for the solved interactive HTML file with definitions and UI
    #[arg(long = "html")]
    pub html: Option<PathBuf>,

    /// Print an ASCII representation of the solved puzzle to stdout
    #[arg(long = "ascii")]
    pub ascii: bool,
}

fn parse_size_str(s: &str) -> Result<(u8, u8)> {
    let s = s.trim().to_lowercase();
    let parts: Vec<&str> = s
        .split(|c: char| c == 'x' || c == '*' || c == ',' || c == ' ' || c == ';' || c == '×')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    if parts.len() == 1 {
        let n: u8 = parts[0]
            .parse()
            .with_context(|| format!("Invalid grid size '{}'", parts[0]))?;
        if n < 4 || n > 30 {
            anyhow::bail!("Grid dimensions must be between 4 and 30, got {}", n);
        }
        return Ok((n, n));
    }

    if parts.len() == 2 {
        let w: u8 = parts[0]
            .parse()
            .with_context(|| format!("Invalid grid width '{}'", parts[0]))?;
        let h: u8 = parts[1]
            .parse()
            .with_context(|| format!("Invalid grid height '{}'", parts[1]))?;
        if w < 4 || w > 30 || h < 4 || h > 30 {
            anyhow::bail!("Grid dimensions must be between 4 and 30, got {}x{}", w, h);
        }
        return Ok((w, h));
    }

    anyhow::bail!("Expected grid size like '10x10' or '12x10', got '{}'", s);
}

fn parse_black_cell_coords(s: &str) -> Result<Vec<(u8, u8)>> {
    let mut coords = Vec::new();
    let tokens: Vec<&str> = s
        .split(|c: char| c == ';' || c.is_whitespace())
        .filter(|t| !t.trim().is_empty())
        .collect();

    if tokens.len() == 1 && tokens[0].matches(',').count() > 1 {
        let nums: Vec<&str> = tokens[0]
            .split(',')
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .collect();
        if nums.len() % 2 != 0 {
            anyhow::bail!("Expected pairs of coordinates (x,y), got odd number of values: {}", s);
        }
        for chunk in nums.chunks(2) {
            let x: u8 = chunk[0].parse().with_context(|| format!("Invalid coordinate '{}'", chunk[0]))?;
            let y: u8 = chunk[1].parse().with_context(|| format!("Invalid coordinate '{}'", chunk[1]))?;
            coords.push((x, y));
        }
    } else {
        for token in tokens {
            let parts: Vec<&str> = token.split(',').map(|p| p.trim()).collect();
            if parts.len() != 2 {
                anyhow::bail!("Expected coordinate pair 'x,y', got '{}'", token);
            }
            let x: u8 = parts[0].parse().with_context(|| format!("Invalid coordinate x '{}'", parts[0]))?;
            let y: u8 = parts[1].parse().with_context(|| format!("Invalid coordinate y '{}'", parts[1]))?;
            coords.push((x, y));
        }
    }
    Ok(coords)
}

pub fn run_cli(args: CliArgs) -> Result<()> {
    let total_start = Instant::now();
    println!("=== Swedish Crossword Generator & Solver (cwgen) ===");

    // 0. Resolve difficulty preference from flag or interactive prompt
    let difficulty_choice = if let Some(ref d_str) = args.difficulty {
        DifficultyChoice::parse(d_str).with_context(|| {
            format!("Neplatná obtížnost '{}'. Platné možnosti: easy/snadna (1), medium/stredni (2), hard/tezka (3), all/mix (4)", d_str)
        })?
    } else if io::stdin().is_terminal() {
        // Interactive prompt when running in terminal without explicit -D flag
        println!("Vyberte preferovanou obtížnost definic:");
        println!("  [1] Snadná (Easy)    - běžná slova a známé pojmy");
        println!("  [2] Střední (Medium) - standardní křížovkářská obtížnost [výchozí]");
        println!("  [3] Těžká (Hard)     - náročnější a odborné definice");
        println!("  [4] Libovolná / Mix  - bez preference obtížnosti");
        print!("Zvolte [1-4, výchozí: 2]: ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_ok() && !line.trim().is_empty() {
            DifficultyChoice::parse(line.trim()).unwrap_or(DifficultyChoice::Medium)
        } else {
            DifficultyChoice::Medium
        }
    } else {
        DifficultyChoice::Medium
    };

    let preferred_difficulty = difficulty_choice.target_difficulty();
    println!("Preferovaná obtížnost: {}", difficulty_choice.display_cs());

    // 1. Resolve secret from flag or positional arg
    let mut resolved_secret = args.secret.clone().or_else(|| args.secret_pos.clone());

    // 2. Load template or procedurally generate Swedish grid
    let (grid, is_procedural) = if let Some(ref tmpl_path) = args.template {
        println!("Loading template: {}", tmpl_path.display());
        let t0 = Instant::now();
        let template_xml = fs::read_to_string(tmpl_path)
            .with_context(|| format!("Failed to read template XML file at '{}'", tmpl_path.display()))?;
        let grid = Grid::from_xml(&template_xml)
            .with_context(|| format!("Failed to parse grid from '{}'", tmpl_path.display()))?;
        let t_template = t0.elapsed();
        println!("  -> Template loaded in {:.2?}", t_template);
        (grid, false)
    } else {
        // Procedural grid: resolve width and height
        let (width, height) = if let (Some(w), Some(h)) = (args.width, args.height) {
            (w, h)
        } else if let Some(ref s) = args.size_flag.as_ref().or(args.size.as_ref()) {
            parse_size_str(s)?
        } else {
            // Interactive terminal prompt for grid size
            print!("Enter grid size (e.g. 10x10, 12x10) [default: 10x10]: ");
            let _ = io::stdout().flush();
            let mut line = String::new();
            if io::stdin().read_line(&mut line).is_ok() && !line.trim().is_empty() {
                parse_size_str(line.trim())?
            } else {
                println!("10x10");
                (10, 10)
            }
        };

        // Interactive terminal prompt for secret solution if not already provided
        if resolved_secret.is_none() && args.size.is_none() && args.size_flag.is_none() {
            print!("Enter secret solution (tajenka) [press Enter to skip]: ");
            let _ = io::stdout().flush();
            let mut line = String::new();
            if io::stdin().read_line(&mut line).is_ok() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    resolved_secret = Some(trimmed.to_string());
                }
            }
        }

        println!("Generating procedural {}x{} Swedish crossword...", width, height);
        (Grid::new_swedish_template(width, height), true)
    };

    let dict_candidates = [
        args.dictionary.clone(),
        PathBuf::from("reference").join(&args.dictionary),
        PathBuf::from("reference/word_difficulty.tsv"),
        PathBuf::from("reference/cs_crossword_extended_quality_audited.tsv"),
        PathBuf::from("../reference/word_difficulty.tsv"),
        PathBuf::from("../reference/cs_crossword_extended_quality_audited.tsv"),
    ];
    let dict_path = dict_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or(args.dictionary.clone());

    println!("Loading dictionary: {}", dict_path.display());
    let t0 = Instant::now();
    let dict = Dictionary::from_file(&dict_path)
        .with_context(|| format!("Failed to load dictionary from '{}'", dict_path.display()))?;
    let t_dict = t0.elapsed();
    let (easy_cnt, med_cnt, hard_cnt, none_cnt) = dict.difficulty_counts();
    println!(
        "  -> Slovník načten: {} hesel v {:.2?} (Snadná: {}, Střední: {}, Těžká: {}{})",
        dict.len(),
        t_dict,
        easy_cnt,
        med_cnt,
        hard_cnt,
        if none_cnt > 0 { format!(", Bez zařazení: {}", none_cnt) } else { String::new() }
    );

    let dict_max = (1..=20)
        .rev()
        .find(|&l| dict.index.count_of_length(l) > 0)
        .unwrap_or(args.max_len as usize) as u8;
    let grid_cap = (grid.width.saturating_sub(1)).max(grid.height.saturating_sub(1));
    let max_len = args.max_len.min(dict_max).min(grid_cap);
    let min_len = args.min_len;

    let max_attempts = if is_procedural && args.seed.is_none() && args.black_cells.is_none() {
        if grid.width >= 25 || grid.height >= 25 {
            35
        } else {
            15
        }
    } else {
        1
    };

    let base_seed = args.seed.unwrap_or_else(|| {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42)
    });

    let mut final_grid = grid.clone();
    let mut final_solution = None;
    let mut last_error_diag = None;

    let procedural_steps = (40_000 * ((grid.width as usize * grid.height as usize) / 64).max(1)).min(80_000);
    let base_solver_config = SolverConfig {
        max_steps: if is_procedural { procedural_steps } else { SolverConfig::DEFAULT_MAX_STEPS },
        run_ac3: true,
        max_branching: Some(if grid.width >= 20 || grid.height >= 20 { 60 } else { 35 }),
        preferred_difficulty,
        progress_callback: None,
    };

    for attempt in 0..max_attempts {
        let mut cur_grid = grid.clone();
        let effective_seed = base_seed.wrapping_add((attempt as u64).wrapping_mul(7919));
        println!("Attempt {}/{} (seed: {})...", attempt + 1, max_attempts, effective_seed);

        if let Some(ref bc_str) = args.black_cells {
            let coords = parse_black_cell_coords(bc_str)
                .with_context(|| format!("Invalid black cell coordinates '{}'", bc_str))?;
            cur_grid.set_black_cells(&coords)
                .with_context(|| "Failed to set specified black cells on grid")?;
            if attempt == 0 {
                println!("  -> Placed {} black cells from --black-cells", coords.len());
            }
        } else if args.auto_black_cells || is_procedural {
            let layout_cfg = LayoutConfig {
                min_word_len: min_len,
                max_word_len: max_len,
                min_blacks: None,
                max_blacks: None,
                seed: Some(effective_seed),
            };
            match cur_grid.apply_auto_black_cells_with_config(&layout_cfg) {
                Ok(placed) => {
                    println!(
                        "  -> Placed {} black cells (words {}..={})",
                        placed.len(),
                        min_len,
                        max_len
                    );
                }
                Err(e) => {
                    println!("  -> Layout error: {}", e);
                    continue;
                }
            }
        }

        // Embed tajenka if requested
        if let Some(ref secret) = resolved_secret {
            let req = TajenkaRequest::from_phrase(secret)
                .with_context(|| format!("Invalid secret solution characters in \"{}\"", secret))?;
            match embed_tajenka(&mut cur_grid, &dict, &req) {
                Ok(path) => {
                    println!(
                        "  -> Embedded \"{}\" across {} cells",
                        path.text(),
                        path.len()
                    );
                }
                Err(e) => {
                    println!("  -> Tajenka embedding error: {}", e);
                    continue;
                }
            }
        }

        // Solve with CSP solver
        let slots = cur_grid.extract_slots();
        let total_slots = slots.len();
        let mut len_counts = std::collections::BTreeMap::new();
        for s in &slots {
            *len_counts.entry(s.cells.len()).or_insert(0) += 1;
        }
        println!("  -> Slots ({} total): {:?}", total_slots, len_counts);

        let pb = ProgressBar::new(total_slots as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{elapsed_precise}] {spinner:.green} [{bar:30.cyan/blue}] {pos}/{len} slots ({msg})")
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("#>-"),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb.set_message("0 steps");

        let pb_cb = pb.clone();
        let mut solver_config = base_solver_config.clone();
        if is_procedural {
            solver_config.max_steps = match attempt {
                0..=2 => 10_000,
                3..=6 => 25_000,
                7..=12 => 45_000,
                _ => procedural_steps,
            };
        }
        solver_config.progress_callback = Some(Arc::new(move |assigned, _total, steps| {
            pb_cb.set_position(assigned as u64);
            pb_cb.set_message(format!("{} steps", steps));
        }));

        let t0 = Instant::now();
        let solve_result = solve_with_diagnostics(&cur_grid, &dict, &solver_config);
        let t_solve = t0.elapsed();

        match solve_result {
            Ok(sol) => {
                pb.finish_and_clear();
                println!(
                    "  -> Solved all {} slots successfully in {:.2?}! (attempt {}/{})",
                    sol.assignments.len(),
                    t_solve,
                    attempt + 1,
                    max_attempts
                );
                final_grid = cur_grid;
                final_solution = Some((sol, slots));
                break;
            }
            Err((err, diag)) => {
                pb.finish_and_clear();
                last_error_diag = Some((err, diag));
                if attempt + 1 < max_attempts {
                    continue;
                }
            }
        }
    }

    let (solution, _slots) = match final_solution {
        Some(s) => s,
        None => {
            if let Some((err, diag)) = last_error_diag {
                eprintln!("\n{}", diag.format_report());
                return Err(err).with_context(|| {
                    "No valid crossword assignment satisfies the grid constraints (Unsatisfiable)"
                });
            } else {
                anyhow::bail!("Failed to generate a valid crossword after {} attempts", max_attempts);
            }
        }
    };

    let mut grid = final_grid;

    // 7. Apply solution back to grid
    let slots = grid.extract_slots();
    solution.apply_to_grid(&mut grid, &slots);

    // 8. Always print ASCII representation for immediate visual satisfaction
    println!("\nSolved Grid (ASCII):\n{}", grid.to_ascii());

    // Difficulty breakdown statistics
    let (easy_used, med_used, hard_used, none_used) = solution.difficulty_counts();
    let total_used = solution.assignments.len();
    println!("=== Statistiky obtížnosti vygenerované křížovky ===");
    println!("  Zvolená preference: {}", difficulty_choice.display_cs());
    println!("  Celkem slov v křížovce: {}", total_used);
    if total_used > 0 {
        let pct = |c: usize| (c as f64 / total_used as f64) * 100.0;
        println!("  • Snadná (Easy):    {:3} ({:5.1}%)", easy_used, pct(easy_used));
        println!("  • Střední (Medium): {:3} ({:5.1}%)", med_used, pct(med_used));
        println!("  • Těžká (Hard):     {:3} ({:5.1}%)", hard_used, pct(hard_used));
        if none_used > 0 {
            println!("  • Bez zařazení:     {:3} ({:5.1}%)", none_used, pct(none_used));
        }
    }

    // 9. Determine output paths (saved in 'output' folder with date, time, and sequence counter)
    let output_dir = PathBuf::from("output");
    fs::create_dir_all(&output_dir)
        .with_context(|| "Failed to create 'output' directory")?;

    let now = chrono::Local::now();
    let date_time_str = now.format("%Y-%m-%d_%H-%M-%S").to_string();

    let (xml_path, html_path) = match (&args.output, &args.html) {
        (Some(op), Some(hp)) => (op.clone(), hp.clone()),
        (Some(op), None) => (op.clone(), op.with_extension("html")),
        (None, Some(hp)) => (hp.with_extension("xml"), hp.clone()),
        (None, None) => {
            let mut counter = 1;
            loop {
                let base_name = format!("krizovka_{}x{}_{}_{:02}", grid.width, grid.height, date_time_str, counter);
                let xp = output_dir.join(format!("{}.xml", base_name));
                let hp = output_dir.join(format!("{}.html", base_name));
                if !xp.exists() && !hp.exists() {
                    break (xp, hp);
                }
                counter += 1;
            }
        }
    };

    println!("\nExporting solved XML to: {}", xml_path.display());
    let t0 = Instant::now();
    let exported_xml = grid.to_xml();
    fs::write(&xml_path, exported_xml)
        .with_context(|| format!("Failed to write output XML to '{}'", xml_path.display()))?;
    let t_export = t0.elapsed();
    println!("  -> Exported successfully in {:.2?}", t_export);

    // 10. Export interactive HTML
    println!("Exporting interactive HTML to: {}", html_path.display());
    let t0 = Instant::now();
    let html_opts = HtmlExportOptions {
        title: format!("Švédská křížovka {}×{}", grid.width, grid.height),
        subtitle: format!(
            "Obtížnost: {} • Slov: {} (Snadná: {} • Střední: {} • Těžká: {}){}",
            difficulty_choice.display_cs(),
            slots.len(),
            easy_used,
            med_used,
            hard_used,
            resolved_secret.as_ref().map(|s| format!(" • Tajenka: \"{}\"", s)).unwrap_or_default()
        ),
        show_solution: true,
    };
    let exported_html = export_grid_to_html(&grid, &slots, Some(&solution), Some(&html_opts));
    fs::write(&html_path, exported_html)
        .with_context(|| format!("Failed to write output HTML to '{}'", html_path.display()))?;
    let t_html = t0.elapsed();
    println!("  -> HTML exported successfully in {:.2?}", t_html);

    let total_elapsed = total_start.elapsed();
    println!("\nCelkový čas vytvoření křížovky: {:.2?}", total_elapsed);

    Ok(())
}
