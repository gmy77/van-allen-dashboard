# Crea (o ricrea) il collegamento sul desktop verso launch.bat.
#
#   powershell -ExecutionPolicy Bypass -File tools\create-shortcut.ps1
#   powershell -ExecutionPolicy Bypass -File tools\create-shortcut.ps1 -Name "Van Allen"

param([string]$Name = 'Fasce di Van Allen')

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$desktop = [Environment]::GetFolderPath('Desktop')
$lnk = Join-Path $desktop "$Name.lnk"
$target = Join-Path $root 'launch.bat'
$icon = Join-Path $root 'assets\vanallen.ico'

if (-not (Test-Path $target)) { throw "manca $target" }

$shell = New-Object -ComObject WScript.Shell
$s = $shell.CreateShortcut($lnk)
$s.TargetPath = $target
$s.WorkingDirectory = $root
$s.Description = 'Dashboard live delle fasce di Van Allen'
if (Test-Path $icon) { $s.IconLocation = "$icon,0" }
# 7 = finestra ridotta a icona: la console del BAT non ruba lo schermo
$s.WindowStyle = 7
$s.Save()

"collegamento creato: $lnk"
