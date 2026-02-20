use crate::types::{CutDirection, Placement, Rect, RotationConstraint};

#[derive(Debug, Clone, Copy)]
pub struct FreeRect {
    pub x: u32,
    pub y: u32,
    pub rect: Rect,
}

#[derive(Debug, Clone)]
pub struct GuillotineBin {
    #[allow(dead_code)]
    stock: Rect,
    kerf: u32,
    cut_direction: CutDirection,
    pub free_rects: Vec<FreeRect>,
    pub placements: Vec<Placement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ScoreStrategy {
    BestAreaFit,
    BestShortSideFit,
    BestLongSideFit,
}

#[derive(Debug, Clone, Copy)]
pub struct ScoredPlacement {
    pub free_idx: usize,
    pub rotated: bool,
    pub score: (u64, u64),
}

impl GuillotineBin {
    pub fn new(stock: Rect, kerf: u32, cut_direction: CutDirection) -> Self {
        Self {
            stock,
            kerf,
            cut_direction,
            free_rects: vec![FreeRect {
                x: 0,
                y: 0,
                rect: stock,
            }],
            placements: Vec::new(),
        }
    }

    pub fn used_area(&self) -> u64 {
        self.placements.iter().map(|p| p.rect.area()).sum()
    }

    pub fn find_best(
        &self,
        piece: Rect,
        rotation: RotationConstraint,
        score_strategy: ScoreStrategy,
    ) -> Option<ScoredPlacement> {
        let try_normal = rotation != RotationConstraint::ForceRotate;
        let try_rotated = rotation != RotationConstraint::NoRotate;

        let mut best: Option<ScoredPlacement> = None;

        for (idx, free) in self.free_rects.iter().enumerate() {
            // Try normal orientation
            if try_normal && piece.fits_in(&free.rect) {
                let score = Self::score(piece, free.rect, score_strategy);
                if best.is_none() || score < best.unwrap().score {
                    best = Some(ScoredPlacement {
                        free_idx: idx,
                        rotated: false,
                        score,
                    });
                }
            }
            // Try rotated
            if try_rotated {
                let rotated = piece.rotated();
                if rotated.fits_in(&free.rect) {
                    let score = Self::score(rotated, free.rect, score_strategy);
                    if best.is_none() || score < best.unwrap().score {
                        best = Some(ScoredPlacement {
                            free_idx: idx,
                            rotated: true,
                            score,
                        });
                    }
                }
            }
        }

        best
    }

    fn score(piece: Rect, free: Rect, strategy: ScoreStrategy) -> (u64, u64) {
        match strategy {
            ScoreStrategy::BestAreaFit => {
                let area_diff = free.area() - piece.area();
                let short_side =
                    std::cmp::min(free.length - piece.length, free.width - piece.width) as u64;
                (area_diff, short_side)
            }
            ScoreStrategy::BestShortSideFit => {
                let short =
                    std::cmp::min(free.length - piece.length, free.width - piece.width) as u64;
                let long =
                    std::cmp::max(free.length - piece.length, free.width - piece.width) as u64;
                (short, long)
            }
            ScoreStrategy::BestLongSideFit => {
                let long =
                    std::cmp::max(free.length - piece.length, free.width - piece.width) as u64;
                let short =
                    std::cmp::min(free.length - piece.length, free.width - piece.width) as u64;
                (long, short)
            }
        }
    }

    pub fn place(&mut self, scored: ScoredPlacement, piece: Rect) -> Placement {
        let free = self.free_rects[scored.free_idx];
        let placed = if scored.rotated {
            piece.rotated()
        } else {
            piece
        };

        let placement = Placement {
            rect: placed,
            x: free.x,
            y: free.y,
            rotated: scored.rotated,
        };

        // Remove the used free rect and split
        self.free_rects.swap_remove(scored.free_idx);
        self.split(free, placed);
        self.placements.push(placement);
        self.merge_free_rects();

        placement
    }

    fn split(&mut self, free: FreeRect, placed: Rect) {
        let right_l = free.rect.length.saturating_sub(placed.length + self.kerf);
        let bottom_w = free.rect.width.saturating_sub(placed.width + self.kerf);

        // Use shorter leftover axis split
        if right_l > 0 && bottom_w > 0 {
            // Decide split direction based on cut_direction preference
            let split_horizontally = match self.cut_direction {
                CutDirection::Auto => {
                    free.rect.length - placed.length < free.rect.width - placed.width
                }
                CutDirection::AlongLength => true,
                CutDirection::AlongWidth => false,
            };
            if split_horizontally {
                // Split horizontally: right rect is narrow, bottom rect spans full length
                // Right remainder
                self.free_rects.push(FreeRect {
                    x: free.x + placed.length + self.kerf,
                    y: free.y,
                    rect: Rect::new(right_l, placed.width),
                });
                // Bottom remainder
                self.free_rects.push(FreeRect {
                    x: free.x,
                    y: free.y + placed.width + self.kerf,
                    rect: Rect::new(free.rect.length, bottom_w),
                });
            } else {
                // Split vertically: bottom rect is narrow, right rect spans full width
                // Right remainder
                self.free_rects.push(FreeRect {
                    x: free.x + placed.length + self.kerf,
                    y: free.y,
                    rect: Rect::new(right_l, free.rect.width),
                });
                // Bottom remainder
                self.free_rects.push(FreeRect {
                    x: free.x,
                    y: free.y + placed.width + self.kerf,
                    rect: Rect::new(placed.length, bottom_w),
                });
            }
        } else if right_l > 0 {
            self.free_rects.push(FreeRect {
                x: free.x + placed.length + self.kerf,
                y: free.y,
                rect: Rect::new(right_l, free.rect.width),
            });
        } else if bottom_w > 0 {
            self.free_rects.push(FreeRect {
                x: free.x,
                y: free.y + placed.width + self.kerf,
                rect: Rect::new(free.rect.length, bottom_w),
            });
        }
    }

    fn merge_free_rects(&mut self) {
        let mut merged = true;
        while merged {
            merged = false;
            'outer: for i in 0..self.free_rects.len() {
                for j in (i + 1)..self.free_rects.len() {
                    if let Some(m) =
                        Self::try_merge(self.free_rects[i], self.free_rects[j], self.cut_direction)
                    {
                        self.free_rects[i] = m;
                        self.free_rects.swap_remove(j);
                        merged = true;
                        break 'outer;
                    }
                }
            }
        }
    }

    fn try_merge(a: FreeRect, b: FreeRect, cut_direction: CutDirection) -> Option<FreeRect> {
        // Merge horizontally: same y, same width, adjacent x
        // Disabled for AlongWidth to preserve column boundaries
        if cut_direction != CutDirection::AlongWidth && a.y == b.y && a.rect.width == b.rect.width {
            if a.x + a.rect.length == b.x {
                return Some(FreeRect {
                    x: a.x,
                    y: a.y,
                    rect: Rect::new(a.rect.length + b.rect.length, a.rect.width),
                });
            }
            if b.x + b.rect.length == a.x {
                return Some(FreeRect {
                    x: b.x,
                    y: b.y,
                    rect: Rect::new(a.rect.length + b.rect.length, a.rect.width),
                });
            }
        }
        // Merge vertically: same x, same length, adjacent y
        // Disabled for AlongLength to preserve row boundaries
        if cut_direction != CutDirection::AlongLength
            && a.x == b.x
            && a.rect.length == b.rect.length
        {
            if a.y + a.rect.width == b.y {
                return Some(FreeRect {
                    x: a.x,
                    y: a.y,
                    rect: Rect::new(a.rect.length, a.rect.width + b.rect.width),
                });
            }
            if b.y + b.rect.width == a.y {
                return Some(FreeRect {
                    x: b.x,
                    y: b.y,
                    rect: Rect::new(a.rect.length, a.rect.width + b.rect.width),
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_place_single_piece() {
        let mut bin = GuillotineBin::new(Rect::new(100, 100), 0, CutDirection::Auto);
        let piece = Rect::new(50, 30);
        let scored = bin
            .find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit,
            )
            .unwrap();
        let p = bin.place(scored, piece);
        assert_eq!(p.x, 0);
        assert_eq!(p.y, 0);
        assert_eq!(p.rect.length, 50);
        assert_eq!(p.rect.width, 30);
        assert!(!bin.free_rects.is_empty());
    }

    #[test]
    fn test_piece_too_large() {
        let bin = GuillotineBin::new(Rect::new(100, 100), 0, CutDirection::Auto);
        let piece = Rect::new(200, 50);
        assert!(
            bin.find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit
            )
            .is_none()
        );
    }

    #[test]
    fn test_rotation_fit() {
        let bin = GuillotineBin::new(Rect::new(100, 50), 0, CutDirection::Auto);
        let piece = Rect::new(50, 100);
        // Doesn't fit without rotation
        assert!(
            bin.find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit
            )
            .is_none()
        );
        // Fits with rotation
        let scored = bin
            .find_best(piece, RotationConstraint::Free, ScoreStrategy::BestAreaFit)
            .unwrap();
        assert!(scored.rotated);
    }

    #[test]
    fn test_kerf() {
        let mut bin = GuillotineBin::new(Rect::new(100, 100), 5, CutDirection::Auto);
        let piece = Rect::new(50, 100);
        let scored = bin
            .find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit,
            )
            .unwrap();
        bin.place(scored, piece);
        // Remaining width should be 100 - 50 - 5 = 45
        let has_45_wide = bin.free_rects.iter().any(|f| f.rect.length == 45);
        assert!(has_45_wide);
    }

    #[test]
    fn test_fill_exact() {
        let mut bin = GuillotineBin::new(Rect::new(100, 100), 0, CutDirection::Auto);
        let piece = Rect::new(100, 100);
        let scored = bin
            .find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit,
            )
            .unwrap();
        bin.place(scored, piece);
        assert!(bin.free_rects.is_empty());
    }

    /// Place a 40x30 piece in a 100x100 stock. The leftover is asymmetric (60 vs 70),
    /// so AlongLength and AlongWidth must produce different free rects.
    ///
    /// AlongLength (split horizontally): bottom rect spans full length (100),
    ///   right rect is narrow (30 tall).
    /// AlongWidth (split vertically): right rect spans full width (100),
    ///   bottom rect is narrow (40 wide).
    #[test]
    fn test_cut_direction_along_length_split() {
        let stock = Rect::new(100, 100);
        let piece = Rect::new(40, 30);

        let mut bin = GuillotineBin::new(stock, 0, CutDirection::AlongLength);
        let scored = bin
            .find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit,
            )
            .unwrap();
        bin.place(scored, piece);

        // AlongLength => split horizontally: bottom rect gets full length
        assert!(
            bin.free_rects
                .iter()
                .any(|f| f.rect.length == 60 && f.rect.width == 30),
            "AlongLength should produce a 60x30 right rect, got: {:?}",
            bin.free_rects
        );
        assert!(
            bin.free_rects
                .iter()
                .any(|f| f.rect.length == 100 && f.rect.width == 70),
            "AlongLength should produce a 100x70 bottom rect spanning full length, got: {:?}",
            bin.free_rects
        );
    }

    #[test]
    fn test_cut_direction_along_width_split() {
        let stock = Rect::new(100, 100);
        let piece = Rect::new(40, 30);

        let mut bin = GuillotineBin::new(stock, 0, CutDirection::AlongWidth);
        let scored = bin
            .find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit,
            )
            .unwrap();
        bin.place(scored, piece);

        // AlongWidth => split vertically: right rect gets full width
        assert!(
            bin.free_rects
                .iter()
                .any(|f| f.rect.length == 60 && f.rect.width == 100),
            "AlongWidth should produce a 60x100 right rect spanning full width, got: {:?}",
            bin.free_rects
        );
        assert!(
            bin.free_rects
                .iter()
                .any(|f| f.rect.length == 40 && f.rect.width == 70),
            "AlongWidth should produce a 40x70 bottom rect, got: {:?}",
            bin.free_rects
        );
    }

    #[test]
    fn test_cut_direction_produces_different_splits() {
        let stock = Rect::new(100, 100);
        let piece = Rect::new(40, 30);

        let mut bin_length = GuillotineBin::new(stock, 0, CutDirection::AlongLength);
        let scored = bin_length
            .find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit,
            )
            .unwrap();
        bin_length.place(scored, piece);

        let mut bin_width = GuillotineBin::new(stock, 0, CutDirection::AlongWidth);
        let scored = bin_width
            .find_best(
                piece,
                RotationConstraint::NoRotate,
                ScoreStrategy::BestAreaFit,
            )
            .unwrap();
        bin_width.place(scored, piece);

        // The free rects must differ between the two directions
        let rects_length: Vec<(u32, u32)> = bin_length
            .free_rects
            .iter()
            .map(|f| (f.rect.length, f.rect.width))
            .collect();
        let rects_width: Vec<(u32, u32)> = bin_width
            .free_rects
            .iter()
            .map(|f| (f.rect.length, f.rect.width))
            .collect();

        assert_ne!(
            rects_length, rects_width,
            "AlongLength and AlongWidth must produce different free rect splits"
        );
    }

    #[test]
    fn test_force_rotate() {
        let bin = GuillotineBin::new(Rect::new(100, 50), 0, CutDirection::Auto);
        let piece = Rect::new(100, 50);
        // ForceRotate: only tries rotated orientation (50x100 doesn't fit in 100x50)
        assert!(
            bin.find_best(
                piece,
                RotationConstraint::ForceRotate,
                ScoreStrategy::BestAreaFit
            )
            .is_none()
        );
        // A piece that fits when rotated
        let piece2 = Rect::new(50, 100);
        let scored = bin
            .find_best(
                piece2,
                RotationConstraint::ForceRotate,
                ScoreStrategy::BestAreaFit,
            )
            .unwrap();
        assert!(scored.rotated);
    }
}
