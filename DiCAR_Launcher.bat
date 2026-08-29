@echo off
cd /d "%~dp0"
python DiCAR_Launcher.py
if errorlevel 1 pause
