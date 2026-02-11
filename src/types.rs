use serde::{Deserialize, Deserializer, Serialize};

pub fn deserialize_u32_from_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            if let Some(v) = n.as_u64() {
                u32::try_from(v).map_err(serde::de::Error::custom)
            } else if let Some(v) = n.as_f64() {
                if v >= 0.0 && v <= u32::MAX as f64 && v.fract() == 0.0 {
                    Ok(v as u32)
                } else {
                    Err(serde::de::Error::custom(format!(
                        "expected a non-negative whole number, got {v}"
                    )))
                }
            } else {
                Err(serde::de::Error::custom("invalid number"))
            }
        }
        _ => Err(serde::de::Error::custom("expected a number")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    #[serde(deserialize_with = "deserialize_u32_from_number")]
    pub length: u32,
    #[serde(deserialize_with = "deserialize_u32_from_number")]
    pub width: u32,
}

impl Rect {
    pub fn new(length: u32, width: u32) -> Self {
        Self { length, width }
    }

    pub fn area(&self) -> u64 {
        self.length as u64 * self.width as u64
    }

    pub fn rotated(&self) -> Self {
        Self {
            length: self.width,
            width: self.length,
        }
    }

    pub fn fits_in(&self, other: &Rect) -> bool {
        self.length <= other.length && self.width <= other.width
    }
}

impl std::fmt::Display for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.length, self.width)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Demand {
    pub rect: Rect,
    pub qty: u32,
    pub allow_rotate: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Placement {
    pub rect: Rect,
    pub x: u32,
    pub y: u32,
    pub rotated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetResult {
    pub placements: Vec<Placement>,
    #[allow(dead_code)]
    pub waste_area: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    pub sheets: Vec<SheetResult>,
    pub stock: Rect,
}

impl Solution {
    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    pub fn total_waste_percent(&self) -> f64 {
        let stock_area = self.stock.area();
        let total_stock_area = stock_area * self.sheets.len() as u64;
        let total_used: u64 = self
            .sheets
            .iter()
            .flat_map(|s| &s.placements)
            .map(|p| p.rect.area())
            .sum();
        if total_stock_area == 0 {
            return 0.0;
        }
        (total_stock_area - total_used) as f64 / total_stock_area as f64 * 100.0
    }
}
