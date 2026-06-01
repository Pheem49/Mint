use mint_core::parse_data_uri;

#[test]
fn parses_supported_image_data_uri() {
    let parsed = parse_data_uri("data:image/png;base64,aGk=").unwrap();
    assert_eq!(parsed.0, "image/png");
    assert_eq!(parsed.1, "png");
    assert_eq!(parsed.2, b"hi");
}

#[test]
fn rejects_unsupported_picture_data_uri() {
    assert!(parse_data_uri("data:image/bmp;base64,aGk=").is_none());
}
