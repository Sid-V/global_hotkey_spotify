@echo off
echo Testing Rust compilation...
cd src-tauri
cargo check --message-format=short 2>&1
echo.
echo Exit code: %ERRORLEVEL%
