# Build and run the world server on Windows, without GNU make. Mirrors the
# Makefile targets. Usually called through make.cmd in the repository root:
#
#   .\make build        build the server
#   .\make run          build and run the server, then open http://localhost:8080
#   .\make clean        remove build output
#
# Builds are optimised by default. Pass -Profile dev for faster, unoptimised
# builds while iterating, e.g. .\make run -Profile dev.

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('build', 'run', 'clean')]
    [string]$Target = 'build',

    # Cargo profile. $Profile is reserved by PowerShell, so it is an alias.
    [Alias('Profile')]
    [string]$CargoProfile = 'release'
)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

function Invoke-Cargo {
    & cargo @args
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

switch ($Target) {
    'build' { Invoke-Cargo build --profile $CargoProfile --bin world-server }
    'run'   { Invoke-Cargo run --profile $CargoProfile --bin world-server }
    'clean' { Invoke-Cargo clean }
}
