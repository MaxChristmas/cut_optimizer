use axum::{Json, Router, http::StatusCode, routing::{get, post}};
use cut_optimizer::solver::Solver;
use cut_optimizer::types::{Demand, Rect, Solution, deserialize_u32_from_number};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct OptimizeRequest {
    stock: Rect,
    cuts: Vec<CutRequest>,
    #[serde(default, deserialize_with = "deserialize_u32_from_number")]
    kerf: u32,
}

#[derive(Deserialize)]
struct CutRequest {
    rect: Rect,
    #[serde(deserialize_with = "deserialize_u32_from_number")]
    qty: u32,
    #[serde(default = "default_true")]
    allow_rotate: bool,
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
    if req.stock.w == 0 || req.stock.h == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "stock dimensions must be non-zero".to_string(),
        ));
    }

    let demands: Vec<Demand> = req
        .cuts
        .into_iter()
        .map(|c| {
            if c.rect.w == 0 || c.rect.h == 0 {
                return Err("cut dimensions must be non-zero".to_string());
            }
            if c.qty == 0 {
                return Err("cut quantity must be non-zero".to_string());
            }
            let fits_normal = c.rect.fits_in(&req.stock);
            let fits_rotated = c.allow_rotate && c.rect.rotated().fits_in(&req.stock);
            if !fits_normal && !fits_rotated {
                return Err(format!(
                    "piece {}x{} does not fit in stock {}x{}",
                    c.rect.w, c.rect.h, req.stock.w, req.stock.h
                ));
            }
            Ok(Demand {
                rect: c.rect,
                qty: c.qty,
                allow_rotate: c.allow_rotate,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let solver = Solver::new(req.stock, req.kerf, demands);
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
    let port = std::env::var("PORT").unwrap_or_else(|_| "3001".to_string());
    let addr = format!("0.0.0.0:{port}");

    let app = Router::new()
        .route("/up", get(|| async { "ok" }))
        .route("/optimize", post(optimize));

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    eprintln!("Listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}
