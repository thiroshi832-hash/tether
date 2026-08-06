@echo off
rem Tether virtual audio driver — first-use install (run elevated).
rem Invoked by src/virtual_mic.rs::ensure_installed(). Adds the signed driver to
rem the driver store and creates the root-enumerated software devnode.
setlocal
set DIR=%~dp0
set INF=%DIR%tether_audio.inf
set HWID=Root\TetherAudio

echo Adding Tether virtual audio driver to the driver store...
pnputil /add-driver "%INF%" /install
if errorlevel 1 goto :fail

echo Creating Tether virtual audio device...
rem devgen ships with the WDK; bundle devgen.exe next to this script (or adapt
rem to devcon.exe: devcon install "%INF%" %HWID%).
"%DIR%devgen.exe" /add /instanceid TetherAudio0 /hardwareid "%HWID%"
if errorlevel 1 goto :fail

echo Tether virtual audio installed.
exit /b 0

:fail
echo Tether virtual audio install failed (errorlevel %errorlevel%).
exit /b 1
