@echo off
setlocal

set "FOCO_DEV_BACKEND_PORT=33210"
set "FOCO_DEV_CONFIG_DIR=%USERPROFILE%\.foco-dev"
set "FOCO_DEV_FRONTEND_PORT=16000"

pushd "%~dp0" || exit /b 1

echo Starting Foco backend on port %FOCO_DEV_BACKEND_PORT% with config %FOCO_DEV_CONFIG_DIR%...
start "Foco backend" /D "%~dp0" cmd /k npm.cmd run backend -- %FOCO_DEV_BACKEND_PORT% "%FOCO_DEV_CONFIG_DIR%"

echo Starting Foco frontend on port %FOCO_DEV_FRONTEND_PORT%...
start "Foco frontend" /D "%~dp0" cmd /k npm.cmd run frontend -- %FOCO_DEV_BACKEND_PORT% "%FOCO_DEV_CONFIG_DIR%" %FOCO_DEV_FRONTEND_PORT%

popd
endlocal
