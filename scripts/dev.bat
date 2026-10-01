@echo off
rem TinyMusic dev launcher (CMD / double-click): sets machine-local Rust env (G:\DeskTop).
cd /d "%~dp0.."
set "RUSTUP_HOME=G:\DeskTop\rustup"
set "CARGO_HOME=G:\DeskTop\cargo"
set "PATH=G:\DeskTop\cargo\bin;%PATH%"
npm run tauri dev
