@echo off
rem Avvia la dashboard delle fasce di Van Allen e non lascia nulla in esecuzione
rem alla chiusura della finestra.
rem
rem Il server HTTP locale (http://127.0.0.1:8787) gira DENTRO l'app: parte con lei
rem e si spegne con lei. Questo BAT fa da supervisore: aspetta la chiusura della
rem finestra e, per sicurezza, termina eventuali processi rimasti appesi.

setlocal
set "ROOT=%~dp0"
set "APP=%ROOT%dist\vanallen.exe"
if not exist "%APP%" set "APP=%ROOT%target\release\vanallen.exe"
if not exist "%APP%" set "APP=%ROOT%target\debug\vanallen.exe"

if not exist "%APP%" (
    echo Eseguibile non trovato.
    echo Compila prima con:  cargo build -p vanallen --release
    pause
    exit /b 1
)

echo Avvio delle fasce di Van Allen...
echo Chiudi la finestra per fermare app e server locale.
start "" /wait "%APP%"

rem nessun processo deve sopravvivere alla finestra
taskkill /im vanallen.exe /f >nul 2>&1

exit /b 0
