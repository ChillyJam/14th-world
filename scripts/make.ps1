# Build and run the world server and desktop client on Windows, without GNU
# make. Mirrors the Makefile targets. Usually called through make.cmd in the
# repository root:
#
#   .\make build                         build both binaries
#   .\make run                           build, then run the server and a client
#   .\make run-server                    run only the server
#   .\make run-client host:port          run only the client, against any server
#   .\make clean                         remove build output
#
# Builds are optimised by default. Pass -Profile dev for faster, unoptimised
# builds while iterating, e.g. .\make run -Profile dev.

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('build', 'run', 'run-server', 'run-client', 'clean')]
    [string]$Target = 'build',

    # Cargo profile. $Profile is reserved by PowerShell, so it is an alias.
    [Alias('Profile')]
    [string]$CargoProfile = 'release',

    # Server for run-client: host:port or a full WebSocket URL.
    [Parameter(Position = 1)]
    [string]$Server = ''
)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

$binDir = if ($CargoProfile -eq 'dev') { 'target\debug' } else { "target\$CargoProfile" }

function Invoke-Cargo {
    & cargo @args
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

switch ($Target) {
    'build' {
        Invoke-Cargo build --profile $CargoProfile --bin world-server --bin world-client
    }

    # The client starts in the background and reconnects until the server is
    # up. The server stays in the foreground so Ctrl+C reaches it and it saves
    # the world before exiting. Closing the client window leaves the server
    # running.
    'run' {
        Invoke-Cargo build --profile $CargoProfile --bin world-server --bin world-client
        Start-Process -FilePath "$binDir\world-client.exe" -NoNewWindow
        & "$binDir\world-server.exe"
        exit $LASTEXITCODE
    }

    'run-server' {
        Invoke-Cargo run --profile $CargoProfile --bin world-server
    }

    'run-client' {
        $clientArgs = @('run', '--profile', $CargoProfile, '--bin', 'world-client')
        if ($Server) { $clientArgs += @('--', $Server) }
        Invoke-Cargo @clientArgs
    }

    'clean' {
        Invoke-Cargo clean
    }
}
