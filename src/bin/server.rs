use axum::{
    Json, Router,
    http::StatusCode,
    routing::{get, post},
};
use cut_optimizer::solver::Solver;
use cut_optimizer::types::{
    CutDirection, Demand, PieceGrain, Rect, RotationConstraint, Solution, StockGrain,
    deserialize_u32_from_number,
};
use serde::{Deserialize, Serialize};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

#[derive(Deserialize, Serialize)]
struct StockRequest {
    #[serde(deserialize_with = "deserialize_u32_from_number")]
    length: u32,
    #[serde(deserialize_with = "deserialize_u32_from_number")]
    width: u32,
    #[serde(default)]
    grain: StockGrain,
}

#[derive(Deserialize, Serialize)]
struct OptimizeRequest {
    stock: StockRequest,
    cuts: Vec<CutRequest>,
    #[serde(default, deserialize_with = "deserialize_u32_from_number")]
    kerf: u32,
    #[serde(default)]
    cut_direction: CutDirection,
    #[serde(default = "default_true")]
    allow_rotate: bool,
}

#[derive(Deserialize, Serialize)]
struct CutRequest {
    rect: Rect,
    #[serde(deserialize_with = "deserialize_u32_from_number")]
    qty: u32,
    #[serde(default)]
    grain: PieceGrain,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct OptimizeResponse {
    sheets: Vec<SheetResponse>,
    stock: Rect,
    sheet_count: usize,
    waste_percent: f64,
}

#[derive(Serialize)]
struct SheetResponse {
    placements: Vec<cut_optimizer::types::Placement>,
    waste_area: u64,
}

async fn optimize(
    Json(req): Json<OptimizeRequest>,
) -> Result<Json<OptimizeResponse>, (StatusCode, String)> {
    tracing::info!(
        body = serde_json::to_string(&req).unwrap_or_default(),
        "POST /optimize"
    );

    let stock = Rect::new(req.stock.length, req.stock.width);
    let stock_grain = req.stock.grain;

    if stock.length == 0 || stock.width == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "stock dimensions must be non-zero".to_string(),
        ));
    }

    let demands: Vec<Demand> = req
        .cuts
        .into_iter()
        .map(|c| {
            if c.rect.length == 0 || c.rect.width == 0 {
                return Err("cut dimensions must be non-zero".to_string());
            }
            if c.qty == 0 {
                return Err("cut quantity must be non-zero".to_string());
            }
            let rotation = RotationConstraint::from_grain(stock_grain, c.grain, req.allow_rotate)
                .with_cut_direction(req.cut_direction, c.rect);
            let fits = match rotation {
                RotationConstraint::NoRotate => c.rect.fits_in(&stock),
                RotationConstraint::ForceRotate => c.rect.rotated().fits_in(&stock),
                RotationConstraint::Free => {
                    c.rect.fits_in(&stock) || c.rect.rotated().fits_in(&stock)
                }
            };
            if !fits {
                return Err(format!(
                    "piece {}x{} does not fit in stock {}x{}",
                    c.rect.length, c.rect.width, stock.length, stock.width
                ));
            }
            Ok(Demand {
                rect: c.rect,
                qty: c.qty,
                allow_rotate: req.allow_rotate,
                grain: c.grain,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let solver = Solver::new(stock, req.kerf, req.cut_direction, stock_grain, demands);
    let solution: Solution = solver.solve();

    let response = OptimizeResponse {
        sheets: solution
            .sheets
            .iter()
            .map(|s| SheetResponse {
                placements: s.placements.clone(),
                waste_area: s.waste_area,
            })
            .collect(),
        stock: solution.stock,
        sheet_count: solution.sheet_count(),
        waste_percent: solution.total_waste_percent(),
    };

    Ok(Json(response))
}

#[tokio::main]
async fn main() {
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("development.log")
        .expect("failed to open development.log");

    tracing_subscriber::fmt()
        .with_writer(log_file)
        .with_target(false)
        .with_ansi(false)
        .with_max_level(Level::INFO)
        .init();

    let port = std::env::var("PORT").unwrap_or_else(|_| "3001".to_string());
    let addr = format!("0.0.0.0:{port}");

    let app = Router::new()
        .route("/up", get(|| async { "ok" }))
        .route("/optimize", post(optimize))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    eprintln!("Listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}
