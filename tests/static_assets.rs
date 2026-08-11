use llama_monitor::web::{gen_routes::static_routes, static_assets};

#[tokio::test]
async fn binary_brand_assets_are_served_as_bytes_with_mime() {
    let routes = static_routes();
    let response = warp::test::request()
        .path("/brand/token-ingot-192.png")
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.body(), static_assets::TOKEN_INGOT_192_PNG);
    assert!(response.body().starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[tokio::test]
async fn manifest_and_maskable_brand_assets_are_registered() {
    let routes = static_routes();
    let manifest = warp::test::request()
        .path("/manifest.json")
        .reply(&routes)
        .await;
    assert_eq!(manifest.status(), 200);
    let manifest_text = std::str::from_utf8(manifest.body()).expect("manifest is UTF-8");
    assert!(manifest_text.contains("token-ingot-maskable-512.png"));

    let maskable = warp::test::request()
        .path("/brand/token-ingot-maskable-512.png")
        .reply(&routes)
        .await;
    assert_eq!(maskable.status(), 200);
    assert_eq!(maskable.headers()["content-type"], "image/png");
    assert_eq!(maskable.body(), static_assets::TOKEN_INGOT_MASKABLE_512_PNG);
}

#[test]
fn production_svg_has_no_raster_or_executable_content() {
    let svg = static_assets::ICON_SVG;
    let lower = svg.to_ascii_lowercase();
    for forbidden in [
        "<script",
        "<image",
        "<filter",
        "foreignobject",
        "href=\"http",
    ] {
        assert!(
            !lower.contains(forbidden),
            "forbidden SVG content: {forbidden}"
        );
    }
    assert!(svg.contains("Token Ingot"));
}
