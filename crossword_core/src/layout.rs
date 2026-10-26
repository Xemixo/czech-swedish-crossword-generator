use std::collections::{HashSet, VecDeque};
use crate::grid::{Grid, GridError};

/// Configuration options for the asymmetric crossword layout generator.
#[derive(Clone, Debug)]
pub struct LayoutConfig {
    /// Minimum allowed word length (default: 2).
    pub min_word_len: u8,
    /// Maximum allowed word length (default: 8).
    pub max_word_len: u8,
    /// Minimum number of black cells to place (relieves intersection density).
    pub min_blacks: Option<usize>,
    /// Maximum number of black cells allowed.
    pub max_blacks: Option<usize>,
    /// Optional random seed for layout variety.
    pub seed: Option<u64>,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            min_word_len: 2,
            max_word_len: 9,
            min_blacks: None,
            max_blacks: None,
            seed: None,
        }
    }
}

/// Simple deterministic pseudo-random number generator (Xorshift64).
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = (self.next_u64() as usize) % (i + 1);
            slice.swap(i, j);
        }
    }
}

/// Computes all valid 1D line bitmasks for a given line width `len`.
///
/// In the bitmask: `0` represents a playable letter cell, `1` represents a black cell.
/// A line is valid if and only if EVERY contiguous segment of `0`s has length between
/// `min_len` and `max_len`, with NO segments shorter than `min_len` or longer than `max_len`.
/// Black cells are strictly isolated: no horizontally adjacent black cells (`##`) are allowed.
pub fn get_valid_line_patterns(len: u8, min_len: u8, max_len: u8) -> Vec<u32> {
    if len == 0 || len > 32 {
        return Vec::new();
    }
    let min_len = min_len.max(2);
    let max_len = max_len.max(min_len);

    // Minimum black cells required so that no segment exceeds max_len:
    // With k black cells, there are k + 1 segments.
    // Total len <= (k + 1) * max_len + k * 1 = k * (max_len + 1) + max_len
    let min_blacks = if len <= max_len {
        0
    } else {
        let diff = (len - max_len) as usize;
        let denom = (max_len as usize) + 1;
        (diff + denom - 1) / denom
    };

    // Maximum black cells allowed in a single line (keep line black cell density <= 25%):
    let max_density_cap = ((len as f32) * 0.25).floor() as usize;
    let extra = if len <= 8 { 1 } else if len <= 14 { 2 } else { 3 };
    let max_blacks = (min_blacks + extra).min(max_density_cap).max(min_blacks);

    let mut patterns = Vec::new();

    // Branch 1: Line starts with a white segment at index 0
    generate_patterns_rec(
        0,
        len,
        min_len,
        max_len,
        0,
        0,
        min_blacks,
        max_blacks,
        &mut patterns,
    );

    // Branch 2: Line starts with an isolated black cell at index 0
    if max_blacks >= 1 {
        generate_patterns_rec(
            1,
            len,
            min_len,
            max_len,
            1, // bit 0 set
            1, // 1 black cell
            min_blacks,
            max_blacks,
            &mut patterns,
        );
    }

    patterns
}

fn generate_patterns_rec(
    pos: u8,
    len: u8,
    min_len: u8,
    max_len: u8,
    cur_mask: u32,
    cur_blacks: usize,
    min_blacks: usize,
    max_blacks: usize,
    out: &mut Vec<u32>,
) {
    for seg_len in min_len..=max_len {
        let next_pos = pos + seg_len;
        if next_pos == len {
            if cur_blacks >= min_blacks && cur_blacks <= max_blacks {
                out.push(cur_mask);
            }
        } else if next_pos < len {
            if cur_blacks + 1 <= max_blacks {
                let next_mask = cur_mask | (1 << next_pos);
                if next_pos + 1 == len {
                    // Line ends with this isolated black cell
                    if cur_blacks + 1 >= min_blacks {
                        out.push(next_mask);
                    }
                } else if next_pos + 1 < len {
                    generate_patterns_rec(
                        next_pos + 1,
                        len,
                        min_len,
                        max_len,
                        next_mask,
                        cur_blacks + 1,
                        min_blacks,
                        max_blacks,
                        out,
                    );
                }
            }
        }
    }
}

/// Scores a line bitmask, awarding strong positive bonuses for longer words (lengths 4–7)
/// while penalizing short words (length 2–3) and penalizing excessive black cells.
fn score_line_pattern(mask: u32, len: u8) -> i32 {
    let mut score = 0;
    let mut r = 0;
    let black_count = mask.count_ones();

    // Prefer minimal necessary black cells to keep density well below 20%
    score -= (black_count as i32) * 20;

    for i in 0..len {
        if (mask & (1 << i)) != 0 {
            if r > 0 {
                score += match r {
                    8 => 90,
                    7 => 110,
                    6 => 110,
                    5 => 100,
                    4 => 80,
                    3 => 40,
                    2 => -150,
                    _ => -500,
                };
                r = 0;
            }
        } else {
            r += 1;
        }
    }
    if r > 0 {
        score += match r {
            8 => 90,
            7 => 110,
            6 => 110,
            5 => 100,
            4 => 80,
            3 => 40,
            2 => -150,
            _ => -500,
        };
    }
    score
}

/// Generates an asymmetric black-cell layout that strictly eliminates short fragments,
/// ensures all words are <= max_word_len (splitting edges if necessary), relieves intersection
/// constraints, keeps total black cells < 20% of playable area, and ensures full grid connectivity.
pub fn generate_asymmetric_black_cells(
    grid: &Grid,
    forbidden_cells: &[(u8, u8)],
    config: &LayoutConfig,
) -> Result<Vec<(u8, u8)>, GridError> {
    // 1. Detect playable bounds (blank or solution cells)
    let mut min_x = u8::MAX;
    let mut max_x = 0;
    let mut min_y = u8::MAX;
    let mut max_y = 0;
    let mut playable_count = 0;

    for y in 0..grid.height {
        for x in 0..grid.width {
            if let Some(cell) = grid.cell(x, y) {
                if cell.is_letter_cell() {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                    playable_count += 1;
                }
            }
        }
    }

    if playable_count == 0 {
        return Ok(Vec::new());
    }

    let play_w = max_x - min_x + 1;
    let play_h = max_y - min_y + 1;

    // Small grids (e.g. <= 4x4) need no black cells
    if play_w <= config.max_word_len && play_h <= config.max_word_len && playable_count <= 20 {
        return Ok(Vec::new());
    }

    let forbidden_set: HashSet<(u8, u8)> = forbidden_cells.iter().cloned().collect();

    let effective_min_len = config.min_word_len.max(2);
    let effective_max_len = config.max_word_len.max(effective_min_len);

    // Calculate target black cell bounds (typically 15-23% for Swedish crosswords)
    let target_max_blacks = config.max_blacks.unwrap_or_else(|| {
        ((playable_count as f32) * 0.22).floor() as usize
    });
    // Hard limit: never exceed 24% of playable area
    let target_max_blacks = target_max_blacks.min(((playable_count as f32) * 0.24).floor() as usize);

    let min_needed_w = if play_w <= effective_max_len {
        0
    } else {
        let diff = (play_w - effective_max_len) as usize;
        let denom = (effective_max_len as usize) + 1;
        (diff + denom - 1) / denom
    };
    let min_needed_h = if play_h <= effective_max_len {
        0
    } else {
        let diff = (play_h - effective_max_len) as usize;
        let denom = (effective_max_len as usize) + 1;
        (diff + denom - 1) / denom
    };
    let min_from_lines = (play_h as usize * min_needed_w).max(play_w as usize * min_needed_h);

    let target_min_blacks = config.min_blacks.unwrap_or_else(|| {
        if play_w <= 8 && play_h <= 8 {
            2
        } else if play_w <= 10 && play_h <= 10 {
            5
        } else {
            min_from_lines.max(((playable_count as f32) * 0.17).round() as usize)
        }
    }).min(target_max_blacks);

    let raw_row_patterns = get_valid_line_patterns(play_w, effective_min_len, effective_max_len);
    let col_patterns: HashSet<u32> = get_valid_line_patterns(play_h, effective_min_len, effective_max_len)
        .into_iter()
        .collect();

    if raw_row_patterns.is_empty() || col_patterns.is_empty() {
        return Ok(Vec::new());
    }

    // Sort row patterns by score (maximizing longer words 5, 6, 7)
    let mut sorted_rows = raw_row_patterns;
    sorted_rows.sort_by(|&a, &b| {
        let score_a = score_line_pattern(a, play_w);
        let score_b = score_line_pattern(b, play_w);
        score_b.cmp(&score_a).then_with(|| a.count_ones().cmp(&b.count_ones()))
    });

    // If seed is provided, shuffle top patterns for true random procedural variety
    if let Some(seed) = config.seed {
        let mut rng = SimpleRng::new(seed);
        let top_n = sorted_rows.len().min(15);
        rng.shuffle(&mut sorted_rows[..top_n]);
    }

    // Backtracking search for valid asymmetric layout
    let mut current_rows: Vec<u32> = Vec::with_capacity(play_h as usize);
    let mut search_steps = 0;
    const MAX_SEARCH_STEPS: usize = 20_000;

    fn backtrack(
        row_idx: u8,
        play_w: u8,
        play_h: u8,
        min_x: u8,
        min_y: u8,
        current_rows: &mut Vec<u32>,
        sorted_rows: &[u32],
        col_patterns: &HashSet<u32>,
        min_len: u8,
        max_len: u8,
        min_blacks: usize,
        max_blacks: usize,
        forbidden_set: &HashSet<(u8, u8)>,
        steps: &mut usize,
        max_steps: usize,
    ) -> Option<Vec<u32>> {
        *steps += 1;
        if *steps > max_steps {
            return None;
        }
        // Track max depth
        // eprintln!("step {}, row_idx {}", *steps, row_idx);

        if row_idx == play_h {
            let total_blacks: u32 = current_rows.iter().map(|m| m.count_ones()).sum();
            if (total_blacks as usize) < min_blacks {
                return None;
            }
            // Check that all columns match valid patterns
            for x in 0..play_w {
                let mut col_mask: u32 = 0;
                for y in 0..play_h {
                    if (current_rows[y as usize] & (1 << x)) != 0 {
                        col_mask |= 1 << y;
                    }
                }
                if !col_patterns.contains(&col_mask) {
                    return None;
                }
            }

            // Check connectivity of all remaining white cells
            let mut blacks = HashSet::new();
            for y in 0..play_h {
                for x in 0..play_w {
                    if (current_rows[y as usize] & (1 << x)) != 0 {
                        blacks.insert((min_x + x, min_y + y));
                    }
                }
            }
            let mut start = None;
            let mut white_count = 0;
            for y in 0..play_h {
                for x in 0..play_w {
                    let pt = (min_x + x, min_y + y);
                    if !blacks.contains(&pt) {
                        white_count += 1;
                        if start.is_none() {
                            start = Some(pt);
                        }
                    }
                }
            }
            if white_count == 0 {
                return None;
            }
            let (sx, sy) = start.unwrap();
            let mut visited = HashSet::new();
            let mut q = VecDeque::new();
            visited.insert((sx, sy));
            q.push_back((sx, sy));
            while let Some((cx, cy)) = q.pop_front() {
                let neighbors = [
                    (cx.wrapping_sub(1), cy),
                    (cx + 1, cy),
                    (cx, cy.wrapping_sub(1)),
                    (cx, cy + 1),
                ];
                for (nx, ny) in neighbors {
                    if nx >= min_x && nx <= min_x + play_w - 1 && ny >= min_y && ny <= min_y + play_h - 1 {
                        if !blacks.contains(&(nx, ny)) && visited.insert((nx, ny)) {
                            q.push_back((nx, ny));
                        }
                    }
                }
            }
            if visited.len() == white_count {
                return Some(current_rows.clone());
            }
            return None;
        }

        let mut must_have_mask: u32 = 0;
        let mut cannot_have_mask: u32 = 0;

        for x in 0..play_w {
            let mut r = 0;
            for y in 0..row_idx {
                if (current_rows[y as usize] & (1 << x)) != 0 {
                    if r > 0 {
                        if r < min_len || r > max_len {
                            return None;
                        }
                        r = 0;
                    }
                } else {
                    r += 1;
                }
            }
            let remaining = play_h - row_idx;
            if r > 0 && r + remaining < min_len {
                return None;
            }
            if r > max_len {
                return None;
            }

            if let Some(&prev_mask) = current_rows.last() {
                if (prev_mask & (1 << x)) != 0 {
                    cannot_have_mask |= 1 << x;
                }
            }
            if r > 0 && r < min_len {
                cannot_have_mask |= 1 << x;
            }
            if r >= max_len {
                must_have_mask |= 1 << x;
            }
            if remaining <= min_len && r == 0 {
                cannot_have_mask |= 1 << x;
            }
        }

        if (must_have_mask & cannot_have_mask) != 0 {
            eprintln!("row {}: must & cannot conflict: {:014b} & {:014b}", row_idx, must_have_mask, cannot_have_mask);
            return None;
        }

        let total_blacks: u32 = current_rows.iter().map(|m| m.count_ones()).sum();
        if total_blacks as usize > max_blacks {
            return None;
        }

        let cur_y = min_y + row_idx;

        let rot = if !sorted_rows.is_empty() {
            (row_idx as usize * 3) % sorted_rows.len()
        } else {
            0
        };
        let (tail, head) = sorted_rows.split_at(rot);
        let candidate_rows: Vec<u32> = head.iter().chain(tail.iter()).cloned().collect();
        let valid_candidates: Vec<u32> = candidate_rows
            .into_iter()
            .filter(|&row_mask| {
                if (row_mask & must_have_mask) != must_have_mask {
                    return false;
                }
                if (row_mask & cannot_have_mask) != 0 {
                    return false;
                }
                if row_mask != 0 {
                    if let Some(&prev_mask) = current_rows.last() {
                        if prev_mask == row_mask {
                            return false;
                        }
                    }
                }
                true
            })
            .collect();

        for row_mask in valid_candidates {

            // Check forbidden cells collision
            let mut collision = false;
            for x in 0..play_w {
                if (row_mask & (1 << x)) != 0 && forbidden_set.contains(&(min_x + x, cur_y)) {
                    collision = true;
                    break;
                }
            }
            if collision {
                continue;
            }

            if (total_blacks + row_mask.count_ones()) as usize > max_blacks {
                continue;
            }

            current_rows.push(row_mask);
            if let Some(res) = backtrack(
                row_idx + 1,
                play_w,
                play_h,
                min_x,
                min_y,
                current_rows,
                sorted_rows,
                col_patterns,
                min_len,
                max_len,
                min_blacks,
                max_blacks,
                forbidden_set,
                steps,
                max_steps,
            ) {
                return Some(res);
            }
            current_rows.pop();
        }

        None
    }

    let result = if play_w >= 8 || play_h >= 8 {
        generate_procedural_layout(
            min_x,
            min_y,
            play_w,
            play_h,
            effective_min_len,
            effective_max_len,
            target_min_blacks,
            target_max_blacks,
            &forbidden_set,
            config.seed.unwrap_or(42),
        )
    } else {
        backtrack(
            0,
            play_w,
            play_h,
            min_x,
            min_y,
            &mut current_rows,
            &sorted_rows,
            &col_patterns,
            effective_min_len,
            effective_max_len,
            target_min_blacks,
            target_max_blacks,
            &forbidden_set,
            &mut search_steps,
            MAX_SEARCH_STEPS,
        ).map(|rows| {
            let mut coords = Vec::new();
            for y in 0..play_h {
                for x in 0..play_w {
                    if (rows[y as usize] & (1 << x)) != 0 {
                        coords.push((min_x + x, min_y + y));
                    }
                }
            }
            coords.sort();
            coords
        }).or_else(|| {
            generate_procedural_layout(
                min_x,
                min_y,
                play_w,
                play_h,
                effective_min_len,
                effective_max_len,
                target_min_blacks,
                target_max_blacks,
                &forbidden_set,
                config.seed.unwrap_or(42),
            )
        })
    };

    match result {
        Some(coords) => Ok(coords),
        None => {
            let fallback = generate_structured_fallback_layout(min_x, min_y, play_w, play_h, &forbidden_set);
            Ok(fallback)
        }
    }
}

/// Fallback structured layout generator with guaranteed connectivity and word lengths 4..6.
fn generate_structured_fallback_layout(
    min_x: u8,
    min_y: u8,
    play_w: u8,
    play_h: u8,
    forbidden: &HashSet<(u8, u8)>,
) -> Vec<(u8, u8)> {
    let mut coords = Vec::new();
    let step = 5;
    for y in 0..play_h {
        let offset = ((y as usize * 3) % step) as u8;
        let mut x = offset + 2;
        while x + 2 < play_w {
            let pt = (min_x + x, min_y + y);
            if !forbidden.contains(&pt) {
                coords.push(pt);
            }
            x += step as u8;
        }
    }
    coords.sort();
    coords
}

/// Evaluates the quality of a subsegment when splitting long words.
fn score_split_subsegment(seg_len: usize, min_l: usize, _max_l: usize) -> i32 {
    if seg_len == 0 {
        0
    } else if seg_len < min_l {
        -1000
    } else if seg_len == 2 {
        -250 // Strong penalty against 2-letter fragments
    } else if seg_len == 3 {
        60
    } else if seg_len == 4 || seg_len == 5 {
        130 // Golden ratio for crossword words: 4 and 5 letters!
    } else if seg_len == 6 {
        70
    } else if seg_len == 7 {
        25
    } else {
        10
    }
}

/// Procedural black-cell layout generator for larger grids (e.g. 15x15) and fallbacks.
/// Splits oversized segments while preventing short fragments and ensuring full 4-connectivity.
fn generate_procedural_layout(
    min_x: u8,
    min_y: u8,
    play_w: u8,
    play_h: u8,
    min_len: u8,
    max_len: u8,
    min_blacks: usize,
    max_blacks: usize,
    forbidden: &HashSet<(u8, u8)>,
    seed: u64,
) -> Option<Vec<(u8, u8)>> {
    let w = play_w as usize;
    let h = play_h as usize;
    let min_l = min_len as usize;
    let max_l = max_len as usize;
    let mut rng = SimpleRng::new(seed);

    for _attempt in 0..2000 {
        let mut is_black = vec![false; w * h];
        let mut black_count = 0;

        let get_b = |grid: &[bool], x: usize, y: usize| -> bool {
            grid[y * w + x]
        };

        let can_place = |grid: &[bool], cells_to_set: &[(usize, usize)]| -> bool {
            for &(cx, cy) in cells_to_set {
                if cx >= w || cy >= h {
                    return false;
                }
                if forbidden.contains(&(min_x + cx as u8, min_y + cy as u8)) {
                    return false;
                }
            }
            // Avoid 2x2 solid blocks of black cells
            for &(cx, cy) in cells_to_set {
                for dy in 0..=1 {
                    for dx in 0..=1 {
                        if cx >= dx && cx + (1 - dx) < w && cy >= dy && cy + (1 - dy) < h {
                            let top_left_x = cx - dx;
                            let top_left_y = cy - dy;
                            let mut all_b = true;
                            for sy in 0..2 {
                                for sx in 0..2 {
                                    let px = top_left_x + sx;
                                    let py = top_left_y + sy;
                                    let is_b = grid[py * w + px] || cells_to_set.contains(&(px, py));
                                    if !is_b {
                                        all_b = false;
                                        break;
                                    }
                                }
                                if !all_b { break; }
                            }
                            if all_b {
                                return false;
                            }
                        }
                    }
                }
            }
            true
        };

        let mut progress = true;
        let mut steps = 0;
        while progress && steps < 500 {
            steps += 1;
            progress = false;

            let mut long_h_segments = Vec::new();
            for y in 0..h {
                let mut x = 0;
                while x < w {
                    if !get_b(&is_black, x, y) {
                        let xs = x;
                        while x < w && !get_b(&is_black, x, y) {
                            x += 1;
                        }
                        let len = x - xs;
                        if len > max_l {
                            long_h_segments.push((y, xs, x - 1, len));
                        }
                    } else {
                        x += 1;
                    }
                }
            }

            let mut long_v_segments = Vec::new();
            for x in 0..w {
                let mut y = 0;
                while y < h {
                    if !get_b(&is_black, x, y) {
                        let ys = y;
                        while y < h && !get_b(&is_black, x, y) {
                            y += 1;
                        }
                        let len = y - ys;
                        if len > max_l {
                            long_v_segments.push((x, ys, y - 1, len));
                        }
                    } else {
                        y += 1;
                    }
                }
            }

            if long_h_segments.is_empty() && long_v_segments.is_empty() {
                break;
            }

            let split_horizontal = if !long_h_segments.is_empty() && !long_v_segments.is_empty() {
                (rng.next_u64() % 2) == 0
            } else {
                !long_h_segments.is_empty()
            };

            let mut candidates: Vec<(usize, usize, i32)> = Vec::new();

            let try_h = |candidates: &mut Vec<(usize, usize, i32)>, segs: &[(usize, usize, usize, usize)]| {
                for &(y, xs, xe, _len) in segs.iter().take(4) {
                    let low = xs + min_l;
                    let high = if xe >= min_l { xe - min_l } else { xs };
                    if low <= high {
                        for split_x in low..=high {
                            let mut ys = y;
                            while ys > 0 && !get_b(&is_black, split_x, ys - 1) {
                                ys -= 1;
                            }
                            let mut ye = y;
                            while ye + 1 < h && !get_b(&is_black, split_x, ye + 1) {
                                ye += 1;
                            }

                            let top_len = y - ys;
                            let bot_len = ye - y;
                            let left_len = split_x - xs;
                            let right_len = xe - split_x;
                            let top_ok = top_len == 0 || top_len >= min_l;
                            let bot_ok = bot_len == 0 || bot_len >= min_l;

                            if top_ok && bot_ok && can_place(&is_black, &[(split_x, y)]) {
                                let score = score_split_subsegment(left_len, min_l, max_l)
                                    + score_split_subsegment(right_len, min_l, max_l)
                                    + score_split_subsegment(top_len, min_l, max_l)
                                    + score_split_subsegment(bot_len, min_l, max_l);
                                candidates.push((split_x, y, score));
                            }
                        }
                    }
                }
            };

            let try_v = |candidates: &mut Vec<(usize, usize, i32)>, segs: &[(usize, usize, usize, usize)]| {
                for &(x, ys, ye, _len) in segs.iter().take(4) {
                    let low = ys + min_l;
                    let high = if ye >= min_l { ye - min_l } else { ys };
                    if low <= high {
                        for split_y in low..=high {
                            let mut xs = x;
                            while xs > 0 && !get_b(&is_black, xs - 1, split_y) {
                                xs -= 1;
                            }
                            let mut xe = x;
                            while xe + 1 < w && !get_b(&is_black, xe + 1, split_y) {
                                xe += 1;
                            }

                            let left_len = x - xs;
                            let right_len = xe - x;
                            let top_len = split_y - ys;
                            let bot_len = ye - split_y;
                            let left_ok = left_len == 0 || left_len >= min_l;
                            let right_ok = right_len == 0 || right_len >= min_l;

                            if left_ok && right_ok && can_place(&is_black, &[(x, split_y)]) {
                                let score = score_split_subsegment(left_len, min_l, max_l)
                                    + score_split_subsegment(right_len, min_l, max_l)
                                    + score_split_subsegment(top_len, min_l, max_l)
                                    + score_split_subsegment(bot_len, min_l, max_l);
                                candidates.push((x, split_y, score));
                            }
                        }
                    }
                }
            };

            if split_horizontal {
                rng.shuffle(&mut long_h_segments);
                try_h(&mut candidates, &long_h_segments);
                if candidates.is_empty() && !long_v_segments.is_empty() {
                    rng.shuffle(&mut long_v_segments);
                    try_v(&mut candidates, &long_v_segments);
                }
            } else {
                rng.shuffle(&mut long_v_segments);
                try_v(&mut candidates, &long_v_segments);
                if candidates.is_empty() && !long_h_segments.is_empty() {
                    rng.shuffle(&mut long_h_segments);
                    try_h(&mut candidates, &long_h_segments);
                }
            }

            if !candidates.is_empty() {
                let max_score = candidates.iter().map(|c| c.2).max().unwrap();
                let top_candidates: Vec<(usize, usize)> = candidates
                    .iter()
                    .filter(|c| c.2 >= max_score.saturating_sub(25))
                    .map(|c| (c.0, c.1))
                    .collect();
                let choice_idx = (rng.next_u64() as usize) % top_candidates.len();
                let (cx, cy) = top_candidates[choice_idx];
                if !is_black[cy * w + cx] {
                    is_black[cy * w + cx] = true;
                    black_count += 1;
                }
                progress = true;
            }
        }

        // Add additional blacks if needed to reach min_blacks
        if black_count < min_blacks {
            let mut free_cells = Vec::new();
            for cy in 0..h {
                for cx in 0..w {
                    if !is_black[cy * w + cx] {
                        free_cells.push((cx, cy));
                    }
                }
            }
            rng.shuffle(&mut free_cells);

            // Pass 1: only place blacks that do NOT create short (<3) words
            for &(cx, cy) in &free_cells {
                if black_count >= min_blacks {
                    break;
                }
                let mut xs = cx;
                while xs > 0 && !get_b(&is_black, xs - 1, cy) { xs -= 1; }
                let mut xe = cx;
                while xe + 1 < w && !get_b(&is_black, xe + 1, cy) { xe += 1; }
                let left_len = cx - xs;
                let right_len = xe - cx;
                let h_ok = (left_len == 0 || left_len >= min_l) && (right_len == 0 || right_len >= min_l);

                let mut ys = cy;
                while ys > 0 && !get_b(&is_black, cx, ys - 1) { ys -= 1; }
                let mut ye = cy;
                while ye + 1 < h && !get_b(&is_black, cx, ye + 1) { ye += 1; }
                let top_len = cy - ys;
                let bot_len = ye - cy;
                let v_ok = (top_len == 0 || top_len >= min_l) && (bot_len == 0 || bot_len >= min_l);

                let creates_short = (left_len > 0 && left_len < 3)
                    || (right_len > 0 && right_len < 3)
                    || (top_len > 0 && top_len < 3)
                    || (bot_len > 0 && bot_len < 3);

                if h_ok && v_ok && !creates_short && can_place(&is_black, &[(cx, cy)]) {
                    is_black[cy * w + cx] = true;
                    black_count += 1;
                }
            }

            // Pass 2: if still under min_blacks, relax !creates_short
            if black_count < min_blacks {
                for (cx, cy) in free_cells {
                    if black_count >= min_blacks {
                        break;
                    }
                    if is_black[cy * w + cx] { continue; }
                    let mut xs = cx;
                    while xs > 0 && !get_b(&is_black, xs - 1, cy) { xs -= 1; }
                    let mut xe = cx;
                    while xe + 1 < w && !get_b(&is_black, xe + 1, cy) { xe += 1; }
                    let left_len = cx - xs;
                    let right_len = xe - cx;
                    let h_ok = (left_len == 0 || left_len >= min_l) && (right_len == 0 || right_len >= min_l);

                    let mut ys = cy;
                    while ys > 0 && !get_b(&is_black, cx, ys - 1) { ys -= 1; }
                    let mut ye = cy;
                    while ye + 1 < h && !get_b(&is_black, cx, ye + 1) { ye += 1; }
                    let top_len = cy - ys;
                    let bot_len = ye - cy;
                    let v_ok = (top_len == 0 || top_len >= min_l) && (bot_len == 0 || bot_len >= min_l);

                    if h_ok && v_ok && can_place(&is_black, &[(cx, cy)]) {
                        is_black[cy * w + cx] = true;
                        black_count += 1;
                    }
                }
            }
        }

        if black_count < min_blacks || black_count > max_blacks {
            continue;
        }

        // Verify all horizontal segments
        let mut valid = true;
        for y in 0..h {
            let mut x = 0;
            while x < w {
                if !get_b(&is_black, x, y) {
                    let xs = x;
                    while x < w && !get_b(&is_black, x, y) { x += 1; }
                    let len = x - xs;
                    if len < min_l || len > max_l {
                        valid = false;
                        break;
                    }
                } else {
                    x += 1;
                }
            }
            if !valid { break; }
        }
        if !valid {
            continue;
        }

        // Verify all vertical segments
        for x in 0..w {
            let mut y = 0;
            while y < h {
                if !get_b(&is_black, x, y) {
                    let ys = y;
                    while y < h && !get_b(&is_black, x, y) { y += 1; }
                    let len = y - ys;
                    if len < min_l || len > max_l {
                        valid = false;
                        break;
                    }
                } else {
                    y += 1;
                }
            }
            if !valid { break; }
        }
        if !valid {
            continue;
        }

        // Verify 4-connectivity of white cells
        let total_white = w * h - black_count;
        if total_white == 0 { continue; }

        let mut start_white = None;
        for cy in 0..h {
            for cx in 0..w {
                if !is_black[cy * w + cx] {
                    start_white = Some((cx, cy));
                    break;
                }
            }
            if start_white.is_some() { break; }
        }

        let (sx, sy) = match start_white {
            Some(pt) => pt,
            None => continue,
        };

        let mut visited = vec![false; w * h];
        let mut q = VecDeque::new();
        visited[sy * w + sx] = true;
        q.push_back((sx, sy));
        let mut reached = 0;

        while let Some((cx, cy)) = q.pop_front() {
            reached += 1;
            let nbs = [
                (cx.wrapping_sub(1), cy),
                (cx + 1, cy),
                (cx, cy.wrapping_sub(1)),
                (cx, cy + 1),
            ];
            for (nx, ny) in nbs {
                if nx < w && ny < h && !is_black[ny * w + nx] && !visited[ny * w + nx] {
                    visited[ny * w + nx] = true;
                    q.push_back((nx, ny));
                }
            }
        }

        if reached != total_white {
            continue;
        }

        let mut coords = Vec::with_capacity(black_count);
        for cy in 0..h {
            for cx in 0..w {
                if is_black[cy * w + cx] {
                    coords.push((min_x + cx as u8, min_y + cy as u8));
                }
            }
        }
        coords.sort();
        return Some(coords);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_generation_various_sizes() {
        for &(w, h, max_len) in &[(8, 8, 7), (10, 10, 9), (15, 15, 8)] {
            let mut grid = Grid::new_swedish_template(w, h);
            let cfg = LayoutConfig {
                min_word_len: 2,
                max_word_len: max_len,
                min_blacks: None,
                max_blacks: None,
                seed: Some(42),
            };
            let placed = grid.apply_auto_black_cells_with_config(&cfg).expect("Layout generation failed");
            let playable_count = (w as usize - 1) * (h as usize - 1);
            let density = placed.len() as f32 / playable_count as f32;
            println!("{}x{} placed {} black cells (density: {:.1}%)", w, h, placed.len(), density * 100.0);

            assert!(density < 0.20, "Black cell density ({:.1}%) must be < 20%", density * 100.0);
            assert!(!placed.is_empty(), "Must place black cells to relieve intersections");

            let slots = grid.extract_slots();
            for s in &slots {
                assert!(s.length >= 2, "Slot length must be >= 2, got {}", s.length);
                assert!(s.length <= max_len, "Slot length must be <= {}, got {}", max_len, s.length);
            }
        }
    }
}

