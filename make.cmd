@echo off
rem Windows alternative to the Makefile. See scripts\make.ps1 for the targets.
rem Runs the script with -ExecutionPolicy Bypass so it works on a fresh machine.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\make.ps1" %*
