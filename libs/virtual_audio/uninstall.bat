@echo off
rem Tether virtual audio driver — uninstall (run elevated).
setlocal
set DIR=%~dp0
set HWID=Root\TetherAudio

echo Removing Tether virtual audio device...
"%DIR%devgen.exe" /remove /instanceid TetherAudio0 2>nul
rem Fallback if devgen isn't present:
rem devcon remove %HWID%

echo Removing driver package from the store...
for /f "tokens=*" %%i in ('pnputil /enum-drivers ^| findstr /i tether_audio.inf') do (
  rem Parse the Published Name (oemNN.inf) and delete it; adapt as needed.
)
echo Done.
exit /b 0
