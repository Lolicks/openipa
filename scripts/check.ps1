$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
Write-Host "[*] cargo fmt --check"
cargo fmt --all -- --check
Write-Host "[*] cargo clippy"
cargo clippy --all-targets -- -D warnings
Write-Host "[*] cargo test"
cargo test --all-targets
Write-Host "[*] OK"
