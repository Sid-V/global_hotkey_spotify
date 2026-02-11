fn main() {
    // Read CLIENT_ID from environment at build time
    // Falls back to default if not set (for backwards compatibility)
    let client_id = std::env::var("SPOTIFY_CLIENT_ID")
        .unwrap_or_else(|_| {
            println!("cargo:warning=SPOTIFY_CLIENT_ID not set, using default (consider setting for production)");
            "919cdcc0a45d420d80f372105f5b96a0".to_string()
        });
    
    println!("cargo:rustc-env=SPOTIFY_CLIENT_ID={}", client_id);
    
    tauri_build::build()
}
