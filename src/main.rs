use axum::{
    extract::Query,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine};
use dotenvy::dotenv;
use image::{ImageBuffer, Rgb};
use qrcode::{EcLevel, QrCode};
use serde::{Deserialize, Serialize};
use std::env;
use tower_http::services::ServeDir;

// ── Request/response types ───────────────────────────────────────────

#[derive(Deserialize)]
struct GenerateRequest {
    url: String,
    logo: bool,
    print_mode: bool,
}

#[derive(Serialize)]
struct GenerateResponse {
    svg: String,
}

#[derive(Deserialize)]
struct PngQuery {
    url: String,
    logo: Option<bool>,
    print_mode: Option<bool>,
}

// ── App state ────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    shortener_url: String,
    logo_b64: String,
}

// ── Main ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let shortener_url = env::var("SHORTENER_URL").expect("SHORTENER_URL must be set");

    // Load favicon as base64 for logo embedding
    let logo_b64 = std::fs::read("static/qr-favicon.png")
        .map(|bytes| general_purpose::STANDARD.encode(&bytes))
        .unwrap_or_default();

    let state = AppState { shortener_url, logo_b64 };
    
    let app = Router::new()
        .route("/", get(index))
        .route("/api/generate", post(generate))
        .route("/api/png", get(generate_png))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001")
        .await
        .unwrap();
    tracing::info!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// ── Handlers ─────────────────────────────────────────────────────────

async fn index(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Html<String> {
    Html(format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>seraph / qr</title>
  <link rel="icon" type="image/png" href="/favicon.png">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/hack-font@3/build/web/hack.css">
  <link rel="stylesheet" href="/static/index.css">
</head>
<body>
  <header>
    <a class="logo" href="https://seraph.ws">seraph</a>
    <div class="barcode"></div>
  </header>
  <main>
    <h1>qr.seraph.ws</h1>
    <p class="prompt">personal qr code generator</p>
    <div class="generator">
      <div class="controls">
        <input type="url" id="url" placeholder="https://example.com" autocomplete="off" />
        <div class="toggle-row">
          <input type="checkbox" id="print-mode" />
          <label for="print-mode">print mode (black on white)</label>
        </div>
        <div class="btn-row">
          <button id="dl-png" onclick="downloadPng()" disabled>download png</button>
          <button id="dl-svg" class="secondary" onclick="downloadSvg()" disabled>download svg</button>
        </div>
        <p class="shortener-link">shorten a url first? <a href="{shortener_url}">{shortener_url}</a></p>
      </div>
      <div class="preview">
        <div id="qr-container"><span>enter a url to preview</span></div>
        <div class="mode-label" id="mode-label"></div>
      </div>
    </div>
  </main>
  <footer><a href="https://seraph.ws">seraph.ws</a></footer>
  <script src="/static/index.js"></script>
</body>
</html>"#, shortener_url = state.shortener_url))
}

async fn generate(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<GenerateRequest>,
) -> impl IntoResponse {
    let url = normalize_url(&payload.url);
    match build_svg(&url, payload.logo, payload.print_mode, &state.logo_b64) {
        Ok(svg) => Json(GenerateResponse { svg }).into_response(),
        Err(_) => (StatusCode::BAD_REQUEST, "Failed to generate QR code").into_response(),
    }
}

async fn generate_png(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(params): Query<PngQuery>,
) -> impl IntoResponse {
    let url = normalize_url(&params.url);
    let logo = params.logo.unwrap_or(false);
    let print_mode = params.print_mode.unwrap_or(false);

    match build_png(&url, logo, print_mode, &state.logo_b64) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CONTENT_DISPOSITION, "attachment; filename=\"qr.png\"")
            .body(axum::body::Body::from(bytes))
            .unwrap()
            .into_response(),
        Err(_) => (StatusCode::BAD_REQUEST, "Failed to generate QR code").into_response(),
    }
}

// ── QR generation ────────────────────────────────────────────────────

fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        format!("https://{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

fn build_svg(url: &str, logo: bool, print_mode: bool, logo_b64: &str) -> Result<String, ()> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::H)
        .map_err(|_| ())?;

    let colors = if print_mode {
        ("#ffffff", "#000000") // bg, fg
    } else {
        ("#0a0a0a", "#ff2d78")
    };

    let (bg, fg) = colors;
    let matrix = code.to_colors();
    let width = code.width();
    let quiet = 4;
    let cell = 10;
    let size = (width + quiet * 2) * cell;

    let mut rects = String::new();
    for y in 0..width {
        for x in 0..width {
            if matrix[y * width + x] == qrcode::Color::Dark {
                let px = (x + quiet) * cell;
                let py = (y + quiet) * cell;
                rects.push_str(&format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                    px, py, cell, cell, fg
                ));
            }
        }
    }

    let logo_svg = if logo && !logo_b64.is_empty() {
        let logo_size = size / 5;
        let logo_x = (size - logo_size) / 2;
        let logo_y = (size - logo_size) / 2;
        let pad = 6;
        format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>
<image href="data:image/png;base64,{}" x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="xMidYMid meet"/>"#,
            logo_x - pad, logo_y - pad,
            logo_size + pad * 2, logo_size + pad * 2,
            bg,
            logo_b64,
            logo_x, logo_y, logo_size, logo_size
        )
    } else {
        String::new()
    };

    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {size} {size}" width="{size}" height="{size}">
<rect width="{size}" height="{size}" fill="{bg}"/>
{rects}
{logo_svg}
</svg>"#,
        size = size, bg = bg, rects = rects, logo_svg = logo_svg
    ))
}

fn build_png(url: &str, logo: bool, print_mode: bool, logo_b64: &str) -> Result<Vec<u8>, ()> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::H)
        .map_err(|_| ())?;

    let (bg_color, fg_color): (Rgb<u8>, Rgb<u8>) = if print_mode {
        (Rgb([255, 255, 255]), Rgb([0, 0, 0]))
    } else {
        (Rgb([10, 10, 10]), Rgb([255, 45, 120]))
    };

    let matrix = code.to_colors();
    let width = code.width();
    let quiet = 4usize;
    let cell = 40usize;
    let size = ((width + quiet * 2) * cell) as u32;

    let mut img = ImageBuffer::from_pixel(size, size, bg_color);

    for y in 0..width {
        for x in 0..width {
            if matrix[y * width + x] == qrcode::Color::Dark {
                let px = ((x + quiet) * cell) as u32;
                let py = ((y + quiet) * cell) as u32;
                for dy in 0..cell as u32 {
                    for dx in 0..cell as u32 {
                        img.put_pixel(px + dx, py + dy, fg_color);
                    }
                }
            }
        }
    }

    // Overlay logo if requested
    if logo && !logo_b64.is_empty() {
        if let Ok(logo_bytes) = general_purpose::STANDARD.decode(logo_b64) {
            if let Ok(logo_img) = image::load_from_memory(&logo_bytes) {
                let logo_size = size / 5;
                let logo_resized = logo_img.resize(
                    logo_size, logo_size,
                    image::imageops::FilterType::Lanczos3
                ).to_rgb8();

                let offset_x = (size - logo_size) / 2;
                let offset_y = (size - logo_size) / 2;
                let pad = 10u32;

                // White/dark backing pad
                for dy in 0..(logo_size + pad * 2) {
                    for dx in 0..(logo_size + pad * 2) {
                        let px = offset_x + dx - pad;
                        let py = offset_y + dy - pad;
                        if px < size && py < size {
                            img.put_pixel(px, py, bg_color);
                        }
                    }
                }

                // Paste logo
                for dy in 0..logo_size {
                    for dx in 0..logo_size {
                        let px = offset_x + dx;
                        let py = offset_y + dy;
                        if px < size && py < size {
                            img.put_pixel(px, py, *logo_resized.get_pixel(dx, dy));
                        }
                    }
                }
            }
        }
    }

    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|_| ())?;
    Ok(bytes)
}
