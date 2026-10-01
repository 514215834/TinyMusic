# One-time permanent setup: write G:\DeskTop Rust env into current-user env vars (idempotent).
# See docs/TECH section 13 for machine conventions.
$rustupHome = 'G:\DeskTop\rustup'
$cargoHome  = 'G:\DeskTop\cargo'

[Environment]::SetEnvironmentVariable('RUSTUP_HOME', $rustupHome, 'User')
[Environment]::SetEnvironmentVariable('CARGO_HOME',  $cargoHome,  'User')

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$cargoHome\bin*") {
    $newPath = $userPath.TrimEnd(';') + ";$cargoHome\bin"
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    Write-Host "PATH appended: $cargoHome\bin"
} else {
    Write-Host 'PATH already contains cargo\bin, skipped'
}

Write-Host ''
Write-Host '--- Written values (User scope, effective in NEW terminals) ---'
"RUSTUP_HOME = " + [Environment]::GetEnvironmentVariable('RUSTUP_HOME', 'User')
"CARGO_HOME  = " + [Environment]::GetEnvironmentVariable('CARGO_HOME',  'User')
"PATH contains cargo\bin: " + ([Environment]::GetEnvironmentVariable('Path', 'User') -like "*$cargoHome\bin*")

# Simulate a freshly opened terminal: use only registry Machine+User PATH and user env vars.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
            [Environment]::GetEnvironmentVariable('Path', 'User')
$env:RUSTUP_HOME = [Environment]::GetEnvironmentVariable('RUSTUP_HOME', 'User')
$env:CARGO_HOME  = [Environment]::GetEnvironmentVariable('CARGO_HOME',  'User')

Write-Host ''
Write-Host '--- Simulated new-terminal check ---'
cargo --version
rustc --version
