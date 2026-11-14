# Avvia la dashboard delle fasce di Van Allen.
#
#   .\run.ps1            build release + avvio
#   .\run.ps1 -Debug     build debug (piu' veloce da compilare, piu' lenta a schermo)
#   .\run.ps1 -Test      esegue i test del core invece di avviare l'app

param(
    [switch]$Debug,
    [switch]$Test
)

$ErrorActionPreference = 'Stop'
Set-Location -Path $PSScriptRoot

if ($Test) {
    cargo test -p vanallen-core
    exit $LASTEXITCODE
}

if ($Debug) {
    cargo run -p vanallen
} else {
    cargo run -p vanallen --release
}
exit $LASTEXITCODE
