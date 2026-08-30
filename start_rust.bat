@echo off
title O'Turkish Kebab - Rust Backend
cd /d "%~dp0"
echo Lancement backend Rust (Axum + Stripe)...
echo Port par defaut 3000 (8080 bloque par Steam CEF) - modifiez .env si besoin
cargo run
pause
