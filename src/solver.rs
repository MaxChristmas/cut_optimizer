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
            let orientations: &[bool] = if allow_rotate && piece.w != piece.h {
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
    use crate::types::Demand;

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
        assert_eq!(sol.sheet_count(), 1);
        assert_eq!(sol.sheets[0].placements.len(), 1);
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
        assert_eq!(sol.sheet_count(), 1);
        assert!(sol.sheets[0].placements[0].rotated);
    }

    #[test]
    fn test_no_demands() {
        let solver = Solver::new(Rect::new(100, 100), 0, vec![]);
        let sol = solver.solve();
        assert_eq!(sol.sheet_count(), 0);
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
        assert_eq!(solver_no_kerf.solve().sheet_count(), 1);

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
        assert_eq!(solver_kerf.solve().sheet_count(), 2);
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
        assert!((sol.total_waste_percent() - 0.0).abs() < 0.01);
    }
}
