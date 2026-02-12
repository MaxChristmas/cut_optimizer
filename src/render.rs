use crate::types::{Placement, Rect};

const MAX_WIDTH: f64 = 80.0;
const MAX_HEIGHT: f64 = 40.0;

pub fn render_sheet(stock: Rect, placements: &[Placement]) -> String {
    let scale = f64::min(
        MAX_WIDTH / stock.length as f64,
        MAX_HEIGHT / stock.width as f64,
    );
    let grid_w = (stock.length as f64 * scale).round() as usize;
    let grid_h = (stock.width as f64 * scale).round() as usize;

    if grid_w == 0 || grid_h == 0 {
        return String::new();
    }

    let mut grid = vec![vec![' '; grid_w + 1]; grid_h + 1];

    // Draw stock border first
    draw_rect(&mut grid, 0, 0, grid_w, grid_h);

    // Draw each placement
    for p in placements {
        let sx = (p.x as f64 * scale).round() as usize;
        let sy = (p.y as f64 * scale).round() as usize;
        let sw = (p.rect.length as f64 * scale).round() as usize;
        let sh = (p.rect.width as f64 * scale).round() as usize;

        if sw == 0 || sh == 0 {
            continue;
        }

        draw_rect(&mut grid, sx, sy, sw, sh);

        // Label
        let label = format!("{}x{}", p.rect.length, p.rect.width);
        let label_chars: Vec<char> = label.chars().collect();

        if sw > 2 && sh > 0 {
            let cx = sx + sw / 2;
            let cy = sy + sh / 2;
            let half = label_chars.len() / 2;
            let start_x = cx.saturating_sub(half);

            for (i, &ch) in label_chars.iter().enumerate() {
                let x = start_x + i;
                if x > sx && x < sx + sw && cy > sy && cy < sy + sh {
                    grid[cy][x] = ch;
                }
            }
        }
    }

    let mut result = String::new();
    for row in &grid {
        let line: String = row.iter().collect();
        result.push_str(line.trim_end());
        result.push('\n');
    }
    result
}

#[allow(clippy::needless_range_loop)]
fn draw_rect(grid: &mut [Vec<char>], x: usize, y: usize, w: usize, h: usize) {
    let rows = grid.len();
    let cols = if rows > 0 { grid[0].len() } else { return };

    // Horizontal edges
    for i in x..=x + w {
        if i < cols {
            if y < rows {
                grid[y][i] = if grid[y][i] == '|' || grid[y][i] == '+' {
                    '+'
                } else {
                    '-'
                };
            }
            if y + h < rows {
                grid[y + h][i] = if grid[y + h][i] == '|' || grid[y + h][i] == '+' {
                    '+'
                } else {
                    '-'
                };
            }
        }
    }

    // Vertical edges
    for j in y..=y + h {
        if j < rows {
            if x < cols {
                grid[j][x] = if grid[j][x] == '-' || grid[j][x] == '+' {
                    '+'
                } else {
                    '|'
                };
            }
            if x + w < cols {
                grid[j][x + w] = if grid[j][x + w] == '-' || grid[j][x + w] == '+' {
                    '+'
                } else {
                    '|'
                };
            }
        }
    }

    // Corners
    for &cx in &[x, x + w] {
        for &cy in &[y, y + h] {
            if cy < rows && cx < cols {
                grid[cy][cx] = '+';
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_single_piece() {
        let stock = Rect::new(100, 50);
        let placements = vec![Placement {
            rect: Rect::new(100, 50),
            x: 0,
            y: 0,
            rotated: false,
        }];
        let output = render_sheet(stock, &placements);
        assert!(output.contains('+'));
        assert!(output.contains('-'));
        assert!(output.contains('|'));
        assert!(output.contains("100x50"));
    }

    #[test]
    fn test_render_two_pieces() {
        let stock = Rect::new(100, 100);
        let placements = vec![
            Placement {
                rect: Rect::new(50, 100),
                x: 0,
                y: 0,
                rotated: false,
            },
            Placement {
                rect: Rect::new(50, 100),
                x: 50,
                y: 0,
                rotated: false,
            },
        ];
        let output = render_sheet(stock, &placements);
        assert!(output.contains("50x100"));
    }

    #[test]
    fn test_render_empty() {
        let stock = Rect::new(100, 100);
        let output = render_sheet(stock, &[]);
        // Should still draw the stock border
        assert!(output.contains('+'));
    }
}
