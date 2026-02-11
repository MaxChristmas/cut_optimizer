use crate::guillotine::{GuillotineBin, ScoreStrategy};
use crate::types::{Demand, Rect, SheetResult, Solution};

pub struct Solver {
    stock: Rect,
    kerf: u32,
    demands: Vec<Demand>,
}

impl Solver {
    pub fn new(stock: Rect, kerf: u32, demands: Vec<Demand>) -> Self {
        Self {
            stock,
            kerf,
            demands,
        }
    }

    pub fn solve(&self) -> Solution {
        let pieces = self.expand_demands();
        if pieces.is_empty() {
            return Solution {
                sheets: vec![],
                stock: self.stock,
            };
        }

        // Greedy phase: try multiple strategies, keep best
        let greedy = self.greedy_best(&pieces);

        // B&B phase: try to improve on greedy
        let bb = self.branch_and_bound(&pieces, greedy.sheets.len());

        if !bb.sheets.is_empty() && bb.sheets.len() < greedy.sheets.len() {
            bb
        } else {
            greedy
        }
    }

    fn expand_demands(&self) -> Vec<(Rect, bool)> {
        let mut pieces = Vec::new();
        for d in &self.demands {
            for _ in 0..d.qty {
                pieces.push((d.rect, d.allow_rotate));
            }
        }
        // Sort by area descending for better packing
        pieces.sort_by(|a, b| b.0.area().cmp(&a.0.area()));
        pieces
    }

    fn greedy_best(&self, pieces: &[(Rect, bool)]) -> Solution {
        let strategies = [
            ScoreStrategy::BestAreaFit,
            ScoreStrategy::BestShortSideFit,
            ScoreStrategy::BestLongSideFit,
        ];

        let mut best: Option<Solution> = None;
        for &strategy in &strategies {
            let sol = self.greedy_solve(pieces, strategy);
            if best.is_none() || sol.sheets.len() < best.as_ref().unwrap().sheets.len() {
                best = Some(sol);
            }
        }
        best.unwrap()
    }

    fn greedy_solve(&self, pieces: &[(Rect, bool)], strategy: ScoreStrategy) -> Solution {
        let mut bins: Vec<GuillotineBin> = Vec::new();

        for &(piece, allow_rotate) in pieces {
            // Try to fit in existing bins
            let mut best_bin = None;
            let mut best_score = None;

            for (bi, bin) in bins.iter().enumerate() {
                if let Some(scored) = bin.find_best(piece, allow_rotate, strategy)
                    && (best_score.is_none() || scored.score < best_score.unwrap())
                {
                    best_bin = Some(bi);
                    best_score = Some(scored.score);
                }
            }

            if let Some(bi) = best_bin {
                let scored = bins[bi].find_best(piece, allow_rotate, strategy).unwrap();
                bins[bi].place(scored, piece);
            } else {
                // Open new bin
                let mut bin = GuillotineBin::new(self.stock, self.kerf);
                let scored = bin
                    .find_best(piece, allow_rotate, strategy)
                    .expect("piece larger than stock");
                bin.place(scored, piece);
                bins.push(bin);
            }
        }

        self.bins_to_solution(bins)
    }

    fn branch_and_bound(&self, pieces: &[(Rect, bool)], upper_bound: usize) -> Solution {
        // Skip B&B for large inputs (too slow)
        if pieces.len() > 20 {
            return Solution {
                sheets: vec![],
                stock: self.stock,
            };
        }

        let mut best_bins: Option<Vec<GuillotineBin>> = None;
        let mut best_count = upper_bound;

        let bins: Vec<GuillotineBin> = vec![];
        self.bb_recurse(pieces, 0, bins, &mut best_bins, &mut best_count);

        match best_bins {
            Some(bins) => self.bins_to_solution(bins),
            None => Solution {
                sheets: vec![],
                stock: self.stock,
            },
        }
    }

    fn bb_recurse(
        &self,
        pieces: &[(Rect, bool)],
        idx: usize,
        bins: Vec<GuillotineBin>,
        best_bins: &mut Option<Vec<GuillotineBin>>,
        best_count: &mut usize,
    ) {
        if idx == pieces.len() {
            if bins.len() < *best_count {
                *best_count = bins.len();
                *best_bins = Some(bins);
            }
            return;
        }

        // Pruning: if current bins already >= best, no point continuing
        if bins.len() >= *best_count {
            return;
        }

        let (piece, allow_rotate) = pieces[idx];

        // Lower bound: remaining area / stock area
        let remaining_area: u64 = pieces[idx..].iter().map(|(r, _)| r.area()).sum();
        let stock_area = self.stock.area();
        let min_extra_bins = if remaining_area > 0 {
            remaining_area.div_ceil(stock_area) as usize
        } else {
            0
        };
        let open_free_area: u64 = bins
            .iter()
            .flat_map(|b| &b.free_rects)
            .map(|f| f.rect.area())
            .sum();
        let needed = if remaining_area > open_free_area {
            bins.len() + (remaining_area - open_free_area).div_ceil(stock_area) as usize
        } else {
            bins.len()
        };
        let lower_bound = std::cmp::max(
            needed,
            bins.len()
                .saturating_add(min_extra_bins.saturating_sub(bins.len())),
        );

        if lower_bound >= *best_count {
            return;
        }

        // Try placing in each existing bin
        for bi in 0..bins.len() {
            let orientations: &[bool] = if allow_rotate && piece.length != piece.width {
                &[false, true]
            } else {
                &[false]
            };

            for &rotated in orientations {
                let try_piece = if rotated { piece.rotated() } else { piece };
                let strategy = ScoreStrategy::BestAreaFit;

                if let Some(scored) = bins[bi].find_best(try_piece, false, strategy) {
                    let mut new_bins = bins.clone();
                    new_bins[bi].place(scored, try_piece);
                    self.bb_recurse(pieces, idx + 1, new_bins, best_bins, best_count);
                }
            }
        }

        // Try opening a new bin (only if it wouldn't exceed best)
        if bins.len() + 1 < *best_count {
            let mut new_bins = bins;
            let mut new_bin = GuillotineBin::new(self.stock, self.kerf);
            let scored = new_bin.find_best(piece, allow_rotate, ScoreStrategy::BestAreaFit);
            if let Some(scored) = scored {
                new_bin.place(scored, piece);
                new_bins.push(new_bin);
                self.bb_recurse(pieces, idx + 1, new_bins, best_bins, best_count);
            }
        }
    }

    fn bins_to_solution(&self, bins: Vec<GuillotineBin>) -> Solution {
        let stock_area = self.stock.area();
        let sheets = bins
            .into_iter()
            .map(|bin| {
                let used = bin.used_area();
                SheetResult {
                    placements: bin.placements,
                    waste_area: stock_area - used,
                }
            })
            .collect();

        Solution {
            sheets,
            stock: self.stock,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Demand, Placement};

    /// Validates a complete solution:
    /// 1. Every placement fits within the stock dimensions
    /// 2. No two placements on the same sheet overlap
    /// 3. The total number of placed pieces matches expectations
    fn assert_solution_valid(sol: &Solution, expected_pieces: usize) {
        let stock = sol.stock;
        let total_placed: usize = sol.sheets.iter().map(|s| s.placements.len()).sum();
        assert_eq!(
            total_placed, expected_pieces,
            "expected {} pieces placed, got {}",
            expected_pieces, total_placed
        );

        for (si, sheet) in sol.sheets.iter().enumerate() {
            for (pi, p) in sheet.placements.iter().enumerate() {
                // Check bounds
                assert!(
                    p.x + p.rect.length <= stock.length,
                    "sheet {si}, piece {pi} ({}) exceeds stock length: x={} + length={} > {}",
                    p.rect, p.x, p.rect.length, stock.length
                );
                assert!(
                    p.y + p.rect.width <= stock.width,
                    "sheet {si}, piece {pi} ({}) exceeds stock width: y={} + width={} > {}",
                    p.rect, p.y, p.rect.width, stock.width
                );
            }

            // Check no overlaps between any pair of placements
            assert_no_overlaps(si, &sheet.placements);
        }
    }

    fn assert_no_overlaps(sheet_idx: usize, placements: &[Placement]) {
        for i in 0..placements.len() {
            for j in (i + 1)..placements.len() {
                let a = &placements[i];
                let b = &placements[j];

                let a_x_end = a.x + a.rect.length;
                let a_y_end = a.y + a.rect.width;
                let b_x_end = b.x + b.rect.length;
                let b_y_end = b.y + b.rect.width;

                let overlaps = a.x < b_x_end && b.x < a_x_end
                    && a.y < b_y_end && b.y < a_y_end;

                assert!(
                    !overlaps,
                    "sheet {sheet_idx}: piece {i} ({} @ ({},{})) overlaps piece {j} ({} @ ({},{}))",
                    a.rect, a.x, a.y, b.rect, b.x, b.y
                );
            }
        }
    }

    #[test]
    fn test_single_piece() {
        let solver = Solver::new(
            Rect::new(100, 100),
            0,
            vec![Demand {
                rect: Rect::new(50, 50),
                qty: 1,
                allow_rotate: true,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert_eq!(sol.sheet_count(), 1);
    }

    #[test]
    fn test_exact_fit_four_pieces() {
        let solver = Solver::new(
            Rect::new(100, 100),
            0,
            vec![Demand {
                rect: Rect::new(50, 50),
                qty: 4,
                allow_rotate: false,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 4);
        assert_eq!(sol.sheet_count(), 1);
    }

    #[test]
    fn test_needs_two_sheets() {
        let solver = Solver::new(
            Rect::new(100, 100),
            0,
            vec![Demand {
                rect: Rect::new(60, 60),
                qty: 4,
                allow_rotate: false,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 4);
        // Each sheet can fit at most 1 piece (60x60 leaves 40x100 and 60x40 — no room for another 60x60)
        assert!(sol.sheet_count() >= 4);
    }

    #[test]
    fn test_rotation_helps() {
        // Stock 100x50, piece 50x100 — only fits if rotated
        let solver = Solver::new(
            Rect::new(100, 50),
            0,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 1,
                allow_rotate: true,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert_eq!(sol.sheet_count(), 1);
        assert!(sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_no_demands() {
        let solver = Solver::new(Rect::new(100, 100), 0, vec![]);
        let sol = solver.solve();
        assert_solution_valid(&sol, 0);
    }

    #[test]
    fn test_kerf_reduces_capacity() {
        // Without kerf: 2 pieces of 50x100 fit in 100x100
        let solver_no_kerf = Solver::new(
            Rect::new(100, 100),
            0,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 2,
                allow_rotate: false,
            }],
        );
        let sol_no_kerf = solver_no_kerf.solve();
        assert_solution_valid(&sol_no_kerf, 2);
        assert_eq!(sol_no_kerf.sheet_count(), 1);

        // With kerf of 5: 50 + 5 + 50 = 105 > 100, needs 2 sheets
        let solver_kerf = Solver::new(
            Rect::new(100, 100),
            5,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 2,
                allow_rotate: false,
            }],
        );
        let sol_kerf = solver_kerf.solve();
        assert_solution_valid(&sol_kerf, 2);
        assert_eq!(sol_kerf.sheet_count(), 2);
    }

    #[test]
    fn test_waste_percent() {
        let solver = Solver::new(
            Rect::new(100, 100),
            0,
            vec![Demand {
                rect: Rect::new(100, 100),
                qty: 1,
                allow_rotate: false,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert!((sol.total_waste_percent() - 0.0).abs() < 0.01);
    }

    /// 30 pieces, 6 different sizes, standard plywood sheet 2440x1220, no kerf.
    /// Verifies all pieces are placed and no placement overlaps or exceeds the stock.
    #[test]
    fn test_complex_mixed_sizes_no_kerf() {
        let stock = Rect::new(2440, 1220);
        let demands = vec![
            Demand { rect: Rect::new(800, 600), qty: 5, allow_rotate: true },
            Demand { rect: Rect::new(400, 300), qty: 8, allow_rotate: true },
            Demand { rect: Rect::new(600, 400), qty: 4, allow_rotate: true },
            Demand { rect: Rect::new(1200, 600), qty: 3, allow_rotate: true },
            Demand { rect: Rect::new(300, 200), qty: 6, allow_rotate: true },
            Demand { rect: Rect::new(500, 500), qty: 4, allow_rotate: false },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 30);

        let solver = Solver::new(stock, 0, demands);
        let sol = solver.solve();
        assert_solution_valid(&sol, 30);

        // Lower bound: total piece area / stock area
        let total_area: u64 = sol.sheets.iter()
            .flat_map(|s| &s.placements)
            .map(|p| p.rect.area())
            .sum();
        let min_sheets = total_area.div_ceil(stock.area()) as usize;
        assert!(sol.sheet_count() >= min_sheets);
    }

    /// 35 pieces, 7 different sizes, with kerf=3.
    /// Kerf eats into available space, so more sheets are needed.
    #[test]
    fn test_complex_mixed_sizes_with_kerf() {
        let stock = Rect::new(2440, 1220);
        let demands = vec![
            Demand { rect: Rect::new(700, 500), qty: 6, allow_rotate: true },
            Demand { rect: Rect::new(350, 250), qty: 5, allow_rotate: true },
            Demand { rect: Rect::new(1000, 400), qty: 3, allow_rotate: true },
            Demand { rect: Rect::new(450, 450), qty: 4, allow_rotate: false },
            Demand { rect: Rect::new(600, 300), qty: 7, allow_rotate: true },
            Demand { rect: Rect::new(250, 150), qty: 5, allow_rotate: true },
            Demand { rect: Rect::new(800, 400), qty: 5, allow_rotate: true },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 35);

        let solver = Solver::new(stock, 3, demands);
        let sol = solver.solve();
        assert_solution_valid(&sol, 35);
    }

    /// 40 pieces, 8 different sizes, rotation disabled for all.
    /// Without rotation the solver has less flexibility, requiring more sheets.
    #[test]
    fn test_complex_no_rotation() {
        let stock = Rect::new(2440, 1220);
        let demands = vec![
            Demand { rect: Rect::new(1200, 600), qty: 4, allow_rotate: false },
            Demand { rect: Rect::new(800, 400), qty: 6, allow_rotate: false },
            Demand { rect: Rect::new(600, 300), qty: 5, allow_rotate: false },
            Demand { rect: Rect::new(400, 400), qty: 3, allow_rotate: false },
            Demand { rect: Rect::new(500, 250), qty: 7, allow_rotate: false },
            Demand { rect: Rect::new(300, 200), qty: 5, allow_rotate: false },
            Demand { rect: Rect::new(700, 350), qty: 6, allow_rotate: false },
            Demand { rect: Rect::new(250, 150), qty: 4, allow_rotate: false },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 40);

        let solver = Solver::new(stock, 0, demands.clone());
        let sol_no_rot = solver.solve();
        assert_solution_valid(&sol_no_rot, 40);

        // Compare with rotation enabled — should use <= sheets
        let demands_rot: Vec<Demand> = demands.into_iter()
            .map(|d| Demand { allow_rotate: true, ..d })
            .collect();
        let solver_rot = Solver::new(stock, 0, demands_rot);
        let sol_rot = solver_rot.solve();
        assert_solution_valid(&sol_rot, 40);
        assert!(sol_rot.sheet_count() <= sol_no_rot.sheet_count());
    }

    /// 50 pieces, 10 different sizes, kerf=4, mix of rotation allowed/disallowed.
    #[test]
    fn test_complex_large_batch_mixed_rotation() {
        let stock = Rect::new(3000, 1500);
        let demands = vec![
            Demand { rect: Rect::new(900, 600), qty: 5, allow_rotate: true },
            Demand { rect: Rect::new(500, 400), qty: 6, allow_rotate: false },
            Demand { rect: Rect::new(700, 350), qty: 4, allow_rotate: true },
            Demand { rect: Rect::new(1200, 500), qty: 3, allow_rotate: true },
            Demand { rect: Rect::new(300, 300), qty: 8, allow_rotate: false },
            Demand { rect: Rect::new(450, 200), qty: 6, allow_rotate: true },
            Demand { rect: Rect::new(600, 450), qty: 5, allow_rotate: false },
            Demand { rect: Rect::new(800, 300), qty: 4, allow_rotate: true },
            Demand { rect: Rect::new(350, 250), qty: 5, allow_rotate: true },
            Demand { rect: Rect::new(1000, 700), qty: 4, allow_rotate: false },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 50);

        let solver = Solver::new(stock, 4, demands);
        let sol = solver.solve();
        assert_solution_valid(&sol, 50);

        assert!(sol.total_waste_percent() < 100.0);
        assert!(sol.total_waste_percent() >= 0.0);
    }

    /// 32 pieces, 5 different sizes, small stock forcing many sheets.
    #[test]
    fn test_complex_small_stock_many_sheets() {
        let stock = Rect::new(500, 400);
        let demands = vec![
            Demand { rect: Rect::new(200, 150), qty: 8, allow_rotate: true },
            Demand { rect: Rect::new(300, 200), qty: 6, allow_rotate: true },
            Demand { rect: Rect::new(150, 100), qty: 7, allow_rotate: true },
            Demand { rect: Rect::new(250, 180), qty: 5, allow_rotate: true },
            Demand { rect: Rect::new(400, 300), qty: 6, allow_rotate: true },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 32);

        let solver = Solver::new(stock, 0, demands);
        let sol = solver.solve();
        assert_solution_valid(&sol, 32);

        // With small stock and large pieces, we need many sheets
        assert!(sol.sheet_count() >= 5);
    }
}
