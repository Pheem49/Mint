use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StockError {
    #[error("stock symbol or name is required")]
    MissingSymbol,
    #[error("stock request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("stock or crypto symbol not found for: {0}")]
    NotFound(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockReport {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub change_percent: f64,
    pub currency: String,
    pub data: String,
    pub json_payload: Value,
}

fn resolve_ticker(query: &str) -> String {
    let clean = query.trim().to_lowercase();
    match clean.as_str() {
        "apple" | "aapl" => "AAPL".into(),
        "nvidia" | "nvda" => "NVDA".into(),
        "tesla" | "tsla" => "TSLA".into(),
        "google" | "alphabet" | "googl" | "goog" => "GOOGL".into(),
        "microsoft" | "msft" => "MSFT".into(),
        "amazon" | "amzn" => "AMZN".into(),
        "meta" | "facebook" => "META".into(),
        "bitcoin" | "btc" => "BTC-USD".into(),
        "ethereum" | "eth" => "ETH-USD".into(),
        "solana" | "sol" => "SOL-USD".into(),
        "dogecoin" | "doge" => "DOGE-USD".into(),
        _ => query.trim().to_uppercase(),
    }
}

pub async fn stock(query: &str) -> Result<StockReport, StockError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(StockError::MissingSymbol);
    }

    let client = crate::HTTP_CLIENT.clone();
    let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

    // 1. Resolve ticker symbol directly if known
    let target_ticker = resolve_ticker(query);

    // Try Yahoo Finance v8 chart API directly (most reliable)
    if let Ok(report) = fetch_v8_chart(&client, user_agent, &target_ticker).await {
        return Ok(report);
    }

    // 2. If direct chart fetch fails, search via Yahoo Search API
    if let Ok(search_res) = client
        .get("https://query2.finance.yahoo.com/1/finance/search")
        .query(&[("q", query), ("quotesCount", "1"), ("newsCount", "0")])
        .header("User-Agent", user_agent)
        .send()
        .await
    {
        if let Ok(json_data) = search_res.json::<Value>().await {
            if let Some(quotes) = json_data["quotes"].as_array() {
                if let Some(first) = quotes.first() {
                    if let Some(found_symbol) = first["symbol"].as_str() {
                        if let Ok(report) = fetch_v8_chart(&client, user_agent, found_symbol).await {
                            return Ok(report);
                        }
                    }
                }
            }
        }
    }

    Err(StockError::NotFound(query.into()))
}

async fn fetch_v8_chart(client: &reqwest::Client, user_agent: &str, symbol: &str) -> Result<StockReport, StockError> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbol}");
    let res: Value = client
        .get(&url)
        .query(&[("interval", "1d"), ("range", "1d")])
        .header("User-Agent", user_agent)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let meta = &res["chart"]["result"][0]["meta"];
    if meta.is_null() {
        return Err(StockError::NotFound(symbol.into()));
    }

    let price = meta["regularMarketPrice"].as_f64().unwrap_or(0.0);
    let prev_close = meta["chartPreviousClose"].as_f64().or_else(|| meta["previousClose"].as_f64()).unwrap_or(price);
    let change = price - prev_close;
    let change_percent = if prev_close != 0.0 { (change / prev_close) * 100.0 } else { 0.0 };

    let day_high = meta["regularMarketDayHigh"].as_f64().unwrap_or(price);
    let day_low = meta["regularMarketDayLow"].as_f64().unwrap_or(price);
    let currency = meta["currency"].as_str().unwrap_or("USD").to_string();
    let volume = meta["regularMarketVolume"].as_f64().unwrap_or(0.0);
    let short_name = meta["shortName"]
        .as_str()
        .or_else(|| meta["longName"].as_str())
        .unwrap_or(symbol);

    let json_payload = json!({
        "symbol": symbol,
        "name": short_name,
        "price": price,
        "change": change,
        "changePercent": change_percent,
        "dayHigh": day_high,
        "dayLow": day_low,
        "currency": currency,
        "marketCap": 0.0,
        "volume": volume
    });

    let json_str = serde_json::to_string(&json_payload).unwrap_or_default();
    let text_summary = format!(
        "{short_name} ({symbol}): {:.2} {currency} ({:+.2} / {:+.2}%)",
        price, change, change_percent
    );
    let full_data = format!("{text_summary}\n\n```stock_json\n{json_str}\n```");

    Ok(StockReport {
        symbol: symbol.to_string(),
        name: short_name.to_string(),
        price,
        change,
        change_percent,
        currency,
        data: full_data,
        json_payload,
    })
}

