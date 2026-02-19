use clap::Parser;
use cut_optimizer::render;
use cut_optimizer::solver::Solver;
use cut_optimizer::types::{CutDirection, Demand, PieceGrain, Rect, StockGrain};

#[derive(Parser)]
#[command(
    name = "cut_optimizer",
    about = "2D rectangular cutting stock optimizer"
)]
struct Cli {
    /// Stock sheet dimensions (LxW, e.g. 2400x1200)
    #[arg(long)]
    stock: String,

    /// Cut pieces as LxW:qty (e.g. 800x600:3 400x300:5)
    #[arg(long = "cuts", num_args = 1..)]
    cuts: Vec<String>,

    /// Blade kerf width in mm (default: 0)
    #[arg(long, default_value_t = 0)]
    kerf: u32,

    /// Disable piece rotation
    #[arg(long)]
    no_rotate: bool,

    /// Cut direction: auto, along-length, or along-width
    #[arg(long, default_value = "auto", value_parser = parse_cut_direction)]
    cut_direction: CutDirection,

    /// Show ASCII layout of each sheet
    #[arg(long)]
    layout: bool,
}

fn parse_cut_direction(s: &str) -> Result<CutDirection, String> {
    match s {
        "auto" => Ok(CutDirection::Auto),
        "along-length" => Ok(CutDirection::AlongLength),
        "along-width" => Ok(CutDirection::AlongWidth),
        _ => Err(format!(
            "invalid cut direction '{}', expected: auto, along-length, or along-width",
            s
        )),
    }
}

fn parse_dimensions(s: &str) -> Result<Rect, String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err(format!("invalid dimensions '{}', expected LxW", s));
    }
    let length = parts[0]
        .parse::<u32>()
        .map_err(|_| format!("invalid length in '{}'", s))?;
    let width = parts[1]
        .parse::<u32>()
        .map_err(|_| format!("invalid width in '{}'", s))?;
    if length == 0 || width == 0 {
        return Err(format!("dimensions must be non-zero in '{}'", s));
    }
    Ok(Rect::new(length, width))
}

fn parse_cut(s: &str, allow_rotate: bool) -> Result<Demand, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(format!("invalid cut '{}', expected LxW:qty", s));
    }
    let rect = parse_dimensions(parts[0])?;
    let qty = parts[1]
        .parse::<u32>()
        .map_err(|_| format!("invalid quantity in '{}'", s))?;
    if qty == 0 {
        return Err(format!("quantity must be non-zero in '{}'", s));
    }
    Ok(Demand {
        rect,
        qty,
        allow_rotate,
        grain: PieceGrain::Auto,
    })
}

fn main() {
    let cli = Cli::parse();

    let stock = parse_dimensions(&cli.stock).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });

    let demands: Vec<Demand> = cli
        .cuts
        .iter()
        .map(|c| parse_cut(c, !cli.no_rotate))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        });

    // Validate all pieces fit in stock (considering rotation)
    for d in &demands {
        let fits_normal = d.rect.fits_in(&stock);
        let fits_rotated = d.allow_rotate && d.rect.rotated().fits_in(&stock);
        if !fits_normal && !fits_rotated {
            eprintln!("Error: piece {} does not fit in stock {}", d.rect, stock);
            std::process::exit(1);
        }
    }

    let solver = Solver::new(
        stock,
        cli.kerf,
        cli.cut_direction,
        StockGrain::None,
        demands,
    );
    let solution = solver.solve();

    // Output results
    for (i, sheet) in solution.sheets.iter().enumerate() {
        println!("Sheet {}:", i + 1);
        for p in &sheet.placements {
            let rot = if p.rotated { " [rotated]" } else { "" };
            println!("  {} @ ({}, {}){}", p.rect, p.x, p.y, rot);
        }
        if cli.layout {
            print!("{}", render::render_sheet(stock, &sheet.placements));
        }
        println!();
    }

    println!(
        "Summary: {} sheet{} used, {:.1}% waste",
        solution.sheet_count(),
        if solution.sheet_count() == 1 { "" } else { "s" },
        solution.total_waste_percent(),
    );
}
