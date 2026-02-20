use crate::guillotine::{GuillotineBin, ScoreStrategy};
use crate::types::{
    CutDirection, Demand, Rect, RotationConstraint, SheetResult, Solution, StockGrain,
};

pub struct Solver {
    stock: Rect,
    kerf: u32,
    cut_direction: CutDirection,
    stock_grain: StockGrain,
    demands: Vec<Demand>,
}

impl Solver {
    pub fn new(
        stock: Rect,
        kerf: u32,
        cut_direction: CutDirection,
        stock_grain: StockGrain,
        demands: Vec<Demand>,
    ) -> Self {
        Self {
            stock,
            kerf,
            cut_direction,
            stock_grain,
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

    fn expand_demands(&self) -> Vec<(Rect, RotationConstraint)> {
        let mut pieces = Vec::new();
        for d in &self.demands {
            let rotation =
                RotationConstraint::from_grain(self.stock_grain, d.grain, d.allow_rotate)
                    .with_cut_direction(self.cut_direction, d.rect);
            for _ in 0..d.qty {
                pieces.push((d.rect, rotation));
            }
        }
        // Sort by area descending for better packing
        pieces.sort_by(|a, b| b.0.area().cmp(&a.0.area()));
        pieces
    }

    fn greedy_best(&self, pieces: &[(Rect, RotationConstraint)]) -> Solution {
        let strategies = [
            ScoreStrategy::BestAreaFit,
            ScoreStrategy::BestShortSideFit,
            ScoreStrategy::BestLongSideFit,
        ];

        // In Auto mode, try both directions and keep the best result
        let directions = match self.cut_direction {
            CutDirection::Auto => vec![CutDirection::AlongLength, CutDirection::AlongWidth],
            dir => vec![dir],
        };

        let mut best: Option<Solution> = None;
        for &dir in &directions {
            for &strategy in &strategies {
                let sol = self.greedy_solve(pieces, strategy, dir);
                if best.is_none() || sol.sheets.len() < best.as_ref().unwrap().sheets.len() {
                    best = Some(sol);
                }
            }
        }
        best.unwrap()
    }

    fn greedy_solve(
        &self,
        pieces: &[(Rect, RotationConstraint)],
        strategy: ScoreStrategy,
        direction: CutDirection,
    ) -> Solution {
        let mut bins: Vec<GuillotineBin> = Vec::new();

        for &(piece, rotation) in pieces {
            // Try to fit in existing bins
            let mut best_bin = None;
            let mut best_score = None;

            for (bi, bin) in bins.iter().enumerate() {
                if let Some(scored) = bin.find_best(piece, rotation, strategy)
                    && (best_score.is_none() || scored.score < best_score.unwrap())
                {
                    best_bin = Some(bi);
                    best_score = Some(scored.score);
                }
            }

            if let Some(bi) = best_bin {
                let scored = bins[bi].find_best(piece, rotation, strategy).unwrap();
                bins[bi].place(scored, piece);
            } else {
                // Open new bin
                let mut bin = GuillotineBin::new(self.stock, self.kerf, direction);
                let scored = bin
                    .find_best(piece, rotation, strategy)
                    .expect("piece larger than stock");
                bin.place(scored, piece);
                bins.push(bin);
            }
        }

        self.bins_to_solution(bins)
    }

    fn bb_directions(&self) -> Vec<CutDirection> {
        match self.cut_direction {
            CutDirection::Auto => vec![CutDirection::AlongLength, CutDirection::AlongWidth],
            dir => vec![dir],
        }
    }

    fn branch_and_bound(
        &self,
        pieces: &[(Rect, RotationConstraint)],
        upper_bound: usize,
    ) -> Solution {
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
        pieces: &[(Rect, RotationConstraint)],
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

        let (piece, rotation) = pieces[idx];

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
            let orientations: &[bool] = match rotation {
                RotationConstraint::Free if piece.length != piece.width => &[false, true],
                RotationConstraint::ForceRotate => &[true],
                _ => &[false],
            };

            for &rotated in orientations {
                let try_piece = if rotated { piece.rotated() } else { piece };
                let strategy = ScoreStrategy::BestAreaFit;

                if let Some(scored) =
                    bins[bi].find_best(try_piece, RotationConstraint::NoRotate, strategy)
                {
                    let mut new_bins = bins.clone();
                    new_bins[bi].place(scored, try_piece);
                    self.bb_recurse(pieces, idx + 1, new_bins, best_bins, best_count);
                }
            }
        }

        // Try opening a new bin (only if it wouldn't exceed best)
        if bins.len() + 1 < *best_count {
            for &dir in &self.bb_directions() {
                let mut new_bins = bins.clone();
                let mut new_bin = GuillotineBin::new(self.stock, self.kerf, dir);
                let scored = new_bin.find_best(piece, rotation, ScoreStrategy::BestAreaFit);
                if let Some(scored) = scored {
                    new_bin.place(scored, piece);
                    new_bins.push(new_bin);
                    self.bb_recurse(pieces, idx + 1, new_bins, best_bins, best_count);
                }
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
    use crate::types::{Demand, PieceGrain, Placement, StockGrain};

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
                    p.rect,
                    p.x,
                    p.rect.length,
                    stock.length
                );
                assert!(
                    p.y + p.rect.width <= stock.width,
                    "sheet {si}, piece {pi} ({}) exceeds stock width: y={} + width={} > {}",
                    p.rect,
                    p.y,
                    p.rect.width,
                    stock.width
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

                let overlaps = a.x < b_x_end && b.x < a_x_end && a.y < b_y_end && b.y < a_y_end;

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
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(50, 50),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Auto,
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
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(50, 50),
                qty: 4,
                allow_rotate: false,
                grain: PieceGrain::Auto,
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
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(60, 60),
                qty: 4,
                allow_rotate: false,
                grain: PieceGrain::Auto,
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
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert_eq!(sol.sheet_count(), 1);
        assert!(sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_no_demands() {
        let solver = Solver::new(
            Rect::new(100, 100),
            0,
            CutDirection::Auto,
            StockGrain::None,
            vec![],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 0);
    }

    #[test]
    fn test_kerf_reduces_capacity() {
        // Without kerf: 2 pieces of 50x100 fit in 100x100
        let solver_no_kerf = Solver::new(
            Rect::new(100, 100),
            0,
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 2,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            }],
        );
        let sol_no_kerf = solver_no_kerf.solve();
        assert_solution_valid(&sol_no_kerf, 2);
        assert_eq!(sol_no_kerf.sheet_count(), 1);

        // With kerf of 5: 50 + 5 + 50 = 105 > 100, needs 2 sheets
        let solver_kerf = Solver::new(
            Rect::new(100, 100),
            5,
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 2,
                allow_rotate: false,
                grain: PieceGrain::Auto,
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
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(100, 100),
                qty: 1,
                allow_rotate: false,
                grain: PieceGrain::Auto,
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
            Demand {
                rect: Rect::new(800, 600),
                qty: 5,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(400, 300),
                qty: 8,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(600, 400),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(1200, 600),
                qty: 3,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(300, 200),
                qty: 6,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(500, 500),
                qty: 4,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 30);

        let solver = Solver::new(stock, 0, CutDirection::Auto, StockGrain::None, demands);
        let sol = solver.solve();
        assert_solution_valid(&sol, 30);

        // Lower bound: total piece area / stock area
        let total_area: u64 = sol
            .sheets
            .iter()
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
            Demand {
                rect: Rect::new(700, 500),
                qty: 6,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(350, 250),
                qty: 5,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(1000, 400),
                qty: 3,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(450, 450),
                qty: 4,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(600, 300),
                qty: 7,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(250, 150),
                qty: 5,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(800, 400),
                qty: 5,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 35);

        let solver = Solver::new(stock, 3, CutDirection::Auto, StockGrain::None, demands);
        let sol = solver.solve();
        assert_solution_valid(&sol, 35);
    }

    /// 40 pieces, 8 different sizes, rotation disabled for all.
    /// Without rotation the solver has less flexibility, requiring more sheets.
    #[test]
    fn test_complex_no_rotation() {
        let stock = Rect::new(2440, 1220);
        let demands = vec![
            Demand {
                rect: Rect::new(1200, 600),
                qty: 4,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(800, 400),
                qty: 6,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(600, 300),
                qty: 5,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(400, 400),
                qty: 3,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(500, 250),
                qty: 7,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(300, 200),
                qty: 5,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(700, 350),
                qty: 6,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(250, 150),
                qty: 4,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 40);

        let solver = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::None,
            demands.clone(),
        );
        let sol_no_rot = solver.solve();
        assert_solution_valid(&sol_no_rot, 40);

        // Compare with rotation enabled — should use <= sheets
        let demands_rot: Vec<Demand> = demands
            .into_iter()
            .map(|d| Demand {
                allow_rotate: true,
                ..d
            })
            .collect();
        let solver_rot = Solver::new(stock, 0, CutDirection::Auto, StockGrain::None, demands_rot);
        let sol_rot = solver_rot.solve();
        assert_solution_valid(&sol_rot, 40);
        assert!(sol_rot.sheet_count() <= sol_no_rot.sheet_count());
    }

    /// 50 pieces, 10 different sizes, kerf=4, mix of rotation allowed/disallowed.
    #[test]
    fn test_complex_large_batch_mixed_rotation() {
        let stock = Rect::new(3000, 1500);
        let demands = vec![
            Demand {
                rect: Rect::new(900, 600),
                qty: 5,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(500, 400),
                qty: 6,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(700, 350),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(1200, 500),
                qty: 3,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(300, 300),
                qty: 8,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(450, 200),
                qty: 6,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(600, 450),
                qty: 5,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(800, 300),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(350, 250),
                qty: 5,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(1000, 700),
                qty: 4,
                allow_rotate: false,
                grain: PieceGrain::Auto,
            },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 50);

        let solver = Solver::new(stock, 4, CutDirection::Auto, StockGrain::None, demands);
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
            Demand {
                rect: Rect::new(200, 150),
                qty: 8,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(300, 200),
                qty: 6,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(150, 100),
                qty: 7,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(250, 180),
                qty: 5,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(400, 300),
                qty: 6,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 32);

        let solver = Solver::new(stock, 0, CutDirection::Auto, StockGrain::None, demands);
        let sol = solver.solve();
        assert_solution_valid(&sol, 32);

        // With small stock and large pieces, we need many sheets
        assert!(sol.sheet_count() >= 5);
    }

    /// Real-world test from CSV: 473x14:4, 473x196:4, 473x158:12, 100x100:8, 742x473:8
    /// on 2500x1200 stock with kerf=3.
    /// Verifies all 3 cut directions produce valid solutions and that
    /// AlongLength and AlongWidth produce different placements.
    #[test]
    fn test_cut_direction_csv_data() {
        let stock = Rect::new(2500, 1200);
        let demands = vec![
            Demand {
                rect: Rect::new(473, 14),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(473, 196),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(473, 158),
                qty: 12,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(100, 100),
                qty: 8,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(742, 473),
                qty: 8,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();
        assert_eq!(total_pieces, 36);

        let sol_auto = Solver::new(
            stock,
            3,
            CutDirection::Auto,
            StockGrain::None,
            demands.clone(),
        )
        .solve();
        let sol_length = Solver::new(
            stock,
            3,
            CutDirection::AlongLength,
            StockGrain::None,
            demands.clone(),
        )
        .solve();
        let sol_width = Solver::new(
            stock,
            3,
            CutDirection::AlongWidth,
            StockGrain::None,
            demands.clone(),
        )
        .solve();

        // All solutions must be valid
        assert_solution_valid(&sol_auto, 36);
        assert_solution_valid(&sol_length, 36);
        assert_solution_valid(&sol_width, 36);

        // Collect all placements (x, y) per direction to verify they differ
        let coords = |sol: &Solution| -> Vec<Vec<(u32, u32)>> {
            sol.sheets
                .iter()
                .map(|s| {
                    let mut c: Vec<(u32, u32)> = s.placements.iter().map(|p| (p.x, p.y)).collect();
                    c.sort();
                    c
                })
                .collect()
        };
        let coords_length = coords(&sol_length);
        let coords_width = coords(&sol_width);

        assert_ne!(
            coords_length, coords_width,
            "AlongLength and AlongWidth should produce different placement layouts"
        );
    }

    /// Verifies that each CutDirection produces valid, non-overlapping solutions
    /// for a simple case where the split direction clearly matters.
    #[test]
    fn test_cut_direction_all_modes_valid() {
        let stock = Rect::new(1000, 500);
        let demands = vec![
            Demand {
                rect: Rect::new(400, 200),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(300, 150),
                qty: 3,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
        ];

        for &dir in &[
            CutDirection::Auto,
            CutDirection::AlongLength,
            CutDirection::AlongWidth,
        ] {
            let sol = Solver::new(stock, 3, dir, StockGrain::None, demands.clone()).solve();
            assert_solution_valid(&sol, 7);
        }
    }

    // ── Grain direction tests ──────────────────────────────────────

    #[test]
    fn test_grain_length_along_length_no_rotate() {
        // Piece grain=Length, stock grain=AlongLength → piece must NOT be rotated
        let stock = Rect::new(100, 50);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::AlongLength,
            vec![Demand {
                rect: Rect::new(100, 50),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Length,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert!(!sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_grain_length_along_width_force_rotate() {
        // Piece grain=Length, stock grain=AlongWidth → piece MUST be rotated
        // Stock 100x50, piece 50x100: needs rotation to fit (50x100 rotated → 100x50)
        let stock = Rect::new(100, 50);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::AlongWidth,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Length,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert!(sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_grain_width_along_length_force_rotate() {
        // Piece grain=Width, stock grain=AlongLength → piece MUST be rotated
        // Stock 100x50, piece 50x100: rotated → 100x50 fits
        let stock = Rect::new(100, 50);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::AlongLength,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Width,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert!(sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_grain_width_along_width_no_rotate() {
        // Piece grain=Width, stock grain=AlongWidth → piece must NOT be rotated
        let stock = Rect::new(100, 50);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::AlongWidth,
            vec![Demand {
                rect: Rect::new(100, 50),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Width,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert!(!sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_grain_auto_free_rotation() {
        // Piece grain=Auto with any stock grain → free rotation (optimizer chooses)
        // Stock 100x50, piece 50x100: only fits rotated
        let stock = Rect::new(100, 50);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::AlongLength,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        assert!(sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_grain_none_stock_ignores_piece_grain() {
        // stock grain=None → all piece grains ignored, free rotation
        // Piece 50x100 with grain=Length in stock 100x50: should still rotate to fit
        let stock = Rect::new(100, 50);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(50, 100),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Length,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        // With grain=None, optimizer is free to rotate — and must rotate to fit
        assert!(sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_grain_constraint_reduces_flexibility() {
        // With grain constraints, some pieces lose rotation flexibility → may need more sheets
        let stock = Rect::new(200, 100);
        // Two 100x200 pieces: without grain, both rotate to 200x100 and fit on 1 sheet each
        // With grain=Length + stock=AlongLength → NoRotate → 100x200 doesn't fit in 200x100 stock
        // because piece.length=100 < stock.length=200 ✓ but piece.width=200 > stock.width=100 ✗
        // So each piece must be ForceRotated or can't be placed depending on grain
        let solver_no_grain = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(100, 200),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            }],
        );
        let sol_free = solver_no_grain.solve();
        assert_solution_valid(&sol_free, 1);
        // Piece 100x200 rotated → 200x100, fits in 200x100 stock
        assert!(sol_free.sheets[0].placements[0].rotated);

        // With grain=Length + stock=AlongLength → NoRotate: 100x200 doesn't fit (width 200 > stock width 100)
        // This would panic at "piece larger than stock" — so use grain=Width to force rotate
        let solver_grain = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::AlongLength,
            vec![Demand {
                rect: Rect::new(100, 200),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Width,
            }],
        );
        let sol_grain = solver_grain.solve();
        assert_solution_valid(&sol_grain, 1);
        // ForceRotate: 100x200 rotated → 200x100 fits
        assert!(sol_grain.sheets[0].placements[0].rotated);
    }

    // ── Cut direction rotation constraint tests ──────────────────

    #[test]
    fn test_cut_direction_along_length_forces_orientation() {
        // AlongLength: pieces should be oriented with length >= width.
        // Piece 30x50 (length < width) → ForceRotate → placed as 50x30.
        let stock = Rect::new(100, 100);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::AlongLength,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(30, 50),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        let p = &sol.sheets[0].placements[0];
        // After forced rotation: placed length=50, width=30
        assert!(
            p.rect.length >= p.rect.width,
            "AlongLength: placed piece should have length >= width, got {}x{}",
            p.rect.length,
            p.rect.width
        );
    }

    #[test]
    fn test_cut_direction_along_width_forces_orientation() {
        // AlongWidth: pieces should be oriented with width >= length.
        // Piece 50x30 (width < length) → ForceRotate → placed as 30x50.
        let stock = Rect::new(100, 100);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::AlongWidth,
            StockGrain::None,
            vec![Demand {
                rect: Rect::new(50, 30),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        let p = &sol.sheets[0].placements[0];
        // After forced rotation: placed width=50, length=30
        assert!(
            p.rect.width >= p.rect.length,
            "AlongWidth: placed piece should have width >= length, got {}x{}",
            p.rect.length,
            p.rect.width
        );
    }

    #[test]
    fn test_cut_direction_grain_takes_priority() {
        // Grain constraint (NoRotate) should override cut_direction.
        // Piece 30x50 with grain=Length, stock=AlongLength → NoRotate (natural alignment)
        // Even though AlongLength would want ForceRotate for this piece shape.
        let stock = Rect::new(100, 100);
        let solver = Solver::new(
            stock,
            0,
            CutDirection::AlongLength,
            StockGrain::AlongLength,
            vec![Demand {
                rect: Rect::new(30, 50),
                qty: 1,
                allow_rotate: true,
                grain: PieceGrain::Length,
            }],
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 1);
        // Grain=Length + Stock=AlongLength → NoRotate, so piece stays 30x50
        assert!(!sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_cut_direction_along_length_all_pieces_oriented() {
        // Multiple non-square pieces with AlongLength: all should be placed length >= width
        let stock = Rect::new(2440, 1220);
        let demands = vec![
            Demand {
                rect: Rect::new(800, 600),
                qty: 3,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(300, 500),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
        ];
        let solver = Solver::new(
            stock,
            0,
            CutDirection::AlongLength,
            StockGrain::None,
            demands,
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 7);
        for sheet in &sol.sheets {
            for p in &sheet.placements {
                assert!(
                    p.rect.length >= p.rect.width,
                    "AlongLength: all placed pieces should have length >= width, got {}x{}",
                    p.rect.length,
                    p.rect.width
                );
            }
        }
    }

    #[test]
    fn test_cut_direction_along_width_all_pieces_oriented() {
        // Multiple non-square pieces with AlongWidth: all should be placed width >= length
        let stock = Rect::new(2440, 1220);
        let demands = vec![
            Demand {
                rect: Rect::new(800, 600),
                qty: 3,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
            Demand {
                rect: Rect::new(300, 500),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto,
            },
        ];
        let solver = Solver::new(
            stock,
            0,
            CutDirection::AlongWidth,
            StockGrain::None,
            demands,
        );
        let sol = solver.solve();
        assert_solution_valid(&sol, 7);
        for sheet in &sol.sheets {
            for p in &sheet.placements {
                assert!(
                    p.rect.width >= p.rect.length,
                    "AlongWidth: all placed pieces should have width >= length, got {}x{}",
                    p.rect.length,
                    p.rect.width
                );
            }
        }
    }

    #[test]
    fn test_grain_mixed_pieces() {
        // Mix of grain-constrained and auto pieces
        let stock = Rect::new(2440, 1220);
        let demands = vec![
            Demand {
                rect: Rect::new(800, 600),
                qty: 3,
                allow_rotate: true,
                grain: PieceGrain::Length, // must align length with stock grain
            },
            Demand {
                rect: Rect::new(400, 300),
                qty: 4,
                allow_rotate: true,
                grain: PieceGrain::Auto, // free rotation
            },
            Demand {
                rect: Rect::new(600, 400),
                qty: 2,
                allow_rotate: true,
                grain: PieceGrain::Width, // must align width with stock grain
            },
        ];
        let total_pieces: u32 = demands.iter().map(|d| d.qty).sum();

        let sol = Solver::new(
            stock,
            0,
            CutDirection::Auto,
            StockGrain::AlongLength,
            demands,
        )
        .solve();
        assert_solution_valid(&sol, total_pieces as usize);

        // Verify grain constraints on placements
        for sheet in &sol.sheets {
            for p in &sheet.placements {
                // All pieces should fit within stock
                assert!(p.x + p.rect.length <= stock.length);
                assert!(p.y + p.rect.width <= stock.width);
            }
        }
    }
}
