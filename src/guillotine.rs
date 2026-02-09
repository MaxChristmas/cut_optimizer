use crate::types::{Placement, Rect};

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
    pub fn new(stock: Rect, kerf: u32) -> Self {
        Self {
            stock,
            kerf,
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
        allow_rotate: bool,
        score_strategy: ScoreStrategy,
    ) -> Option<ScoredPlacement> {
        let mut best: Option<ScoredPlacement> = None;

        for (idx, free) in self.free_rects.iter().enumerate() {
            // Try normal orientation
            if piece.fits_in(&free.rect) {
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
            if allow_rotate {
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
                let short_side = std::cmp::min(free.w - piece.w, free.h - piece.h) as u64;
                (area_diff, short_side)
            }
            ScoreStrategy::BestShortSideFit => {
                let short = std::cmp::min(free.w - piece.w, free.h - piece.h) as u64;
                let long = std::cmp::max(free.w - piece.w, free.h - piece.h) as u64;
                (short, long)
            }
            ScoreStrategy::BestLongSideFit => {
                let long = std::cmp::max(free.w - piece.w, free.h - piece.h) as u64;
                let short = std::cmp::min(free.w - piece.w, free.h - piece.h) as u64;
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
        let right_w = free.rect.w.saturating_sub(placed.w + self.kerf);
        let bottom_h = free.rect.h.saturating_sub(placed.h + self.kerf);

        // Use shorter leftover axis split
        if right_w > 0 && bottom_h > 0 {
            // Decide split direction: shorter leftover axis
            if free.rect.w - placed.w < free.rect.h - placed.h {
                // Split horizontally: right rect is narrow, bottom rect spans full width
                // Right remainder
                self.free_rects.push(FreeRect {
                    x: free.x + placed.w + self.kerf,
                    y: free.y,
                    rect: Rect::new(right_w, placed.h),
                });
                // Bottom remainder
                self.free_rects.push(FreeRect {
                    x: free.x,
                    y: free.y + placed.h + self.kerf,
                    rect: Rect::new(free.rect.w, bottom_h),
                });
            } else {
                // Split vertically: bottom rect is narrow, right rect spans full height
                // Right remainder
                self.free_rects.push(FreeRect {
                    x: free.x + placed.w + self.kerf,
                    y: free.y,
                    rect: Rect::new(right_w, free.rect.h),
                });
                // Bottom remainder
                self.free_rects.push(FreeRect {
                    x: free.x,
                    y: free.y + placed.h + self.kerf,
                    rect: Rect::new(placed.w, bottom_h),
                });
            }
        } else if right_w > 0 {
            self.free_rects.push(FreeRect {
                x: free.x + placed.w + self.kerf,
                y: free.y,
                rect: Rect::new(right_w, free.rect.h),
            });
        } else if bottom_h > 0 {
            self.free_rects.push(FreeRect {
                x: free.x,
                y: free.y + placed.h + self.kerf,
                rect: Rect::new(free.rect.w, bottom_h),
            });
        }
    }

    fn merge_free_rects(&mut self) {
        let mut merged = true;
        while merged {
            merged = false;
            'outer: for i in 0..self.free_rects.len() {
                for j in (i + 1)..self.free_rects.len() {
                    if let Some(m) = Self::try_merge(self.free_rects[i], self.free_rects[j]) {
                        self.free_rects[i] = m;
                        self.free_rects.swap_remove(j);
                        merged = true;
                        break 'outer;
                    }
                }
            }
        }
    }

    fn try_merge(a: FreeRect, b: FreeRect) -> Option<FreeRect> {
        // Merge horizontally: same y, same height, adjacent x
        if a.y == b.y && a.rect.h == b.rect.h {
            if a.x + a.rect.w == b.x {
                return Some(FreeRect {
                    x: a.x,
                    y: a.y,
                    rect: Rect::new(a.rect.w + b.rect.w, a.rect.h),
                });
            }
            if b.x + b.rect.w == a.x {
                return Some(FreeRect {
                    x: b.x,
                    y: b.y,
                    rect: Rect::new(a.rect.w + b.rect.w, a.rect.h),
                });
            }
        }
        // Merge vertically: same x, same width, adjacent y
        if a.x == b.x && a.rect.w == b.rect.w {
            if a.y + a.rect.h == b.y {
                return Some(FreeRect {
                    x: a.x,
                    y: a.y,
                    rect: Rect::new(a.rect.w, a.rect.h + b.rect.h),
                });
            }
            if b.y + b.rect.h == a.y {
                return Some(FreeRect {
                    x: b.x,
                    y: b.y,
                    rect: Rect::new(a.rect.w, a.rect.h + b.rect.h),
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
        let mut bin = GuillotineBin::new(Rect::new(100, 100), 0);
        let piece = Rect::new(50, 30);
        let scored = bin
            .find_best(piece, false, ScoreStrategy::BestAreaFit)
            .unwrap();
        let p = bin.place(scored, piece);
        assert_eq!(p.x, 0);
        assert_eq!(p.y, 0);
        assert_eq!(p.rect.w, 50);
        assert_eq!(p.rect.h, 30);
        assert!(!bin.free_rects.is_empty());
    }

    #[test]
    fn test_piece_too_large() {
        let bin = GuillotineBin::new(Rect::new(100, 100), 0);
        let piece = Rect::new(200, 50);
        assert!(
            bin.find_best(piece, false, ScoreStrategy::BestAreaFit)
                .is_none()
        );
    }

    #[test]
    fn test_rotation_fit() {
        let bin = GuillotineBin::new(Rect::new(100, 50), 0);
        let piece = Rect::new(50, 100);
        // Doesn't fit without rotation
        assert!(
            bin.find_best(piece, false, ScoreStrategy::BestAreaFit)
                .is_none()
        );
        // Fits with rotation
        let scored = bin
            .find_best(piece, true, ScoreStrategy::BestAreaFit)
            .unwrap();
        assert!(scored.rotated);
    }

    #[test]
    fn test_kerf() {
        let mut bin = GuillotineBin::new(Rect::new(100, 100), 5);
        let piece = Rect::new(50, 100);
        let scored = bin
            .find_best(piece, false, ScoreStrategy::BestAreaFit)
            .unwrap();
        bin.place(scored, piece);
        // Remaining width should be 100 - 50 - 5 = 45
        let has_45_wide = bin.free_rects.iter().any(|f| f.rect.w == 45);
        assert!(has_45_wide);
    }

    #[test]
    fn test_fill_exact() {
        let mut bin = GuillotineBin::new(Rect::new(100, 100), 0);
        let piece = Rect::new(100, 100);
        let scored = bin
            .find_best(piece, false, ScoreStrategy::BestAreaFit)
            .unwrap();
        bin.place(scored, piece);
        assert!(bin.free_rects.is_empty());
    }
}
