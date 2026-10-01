# TinyMusic dev launcher (PowerShell): sets machine-local Rust env (non-default G:\DeskTop paths).
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\dev.ps1
# After running scripts\setup-rust-env.ps1 and reopening the terminal, plain "npm run tauri dev" works.
$env:RUSTUP_HOME = "G:\DeskTop\rustup"
$env:CARGO_HOME   = "G:\DeskTop\cargo"
$env:Path         = "G:\DeskTop\cargo\bin;" + $env:Path

npm run tauri dev
