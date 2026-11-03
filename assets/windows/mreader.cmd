@echo off
setlocal EnableExtensions EnableDelayedExpansion
set "APP_DIR=%~dp0"
set "PATH=%APP_DIR%;%PATH%"
set "GTK_DATA_PREFIX=%APP_DIR%runtime"
set "GTK_EXE_PREFIX=%APP_DIR%runtime"
set "GTK_PATH=%APP_DIR%runtime\lib\gtk-4.0"
set "GSETTINGS_SCHEMA_DIR=%APP_DIR%runtime\share\glib-2.0\schemas"
set "GDK_PIXBUF_MODULEDIR=%APP_DIR%runtime\lib\gdk-pixbuf-2.0\2.10.0\loaders"
if exist "%APP_DIR%gdk-pixbuf-query-loaders.exe" (
  set "LOADERS="
  for %%F in ("%APP_DIR%runtime\lib\gdk-pixbuf-2.0\2.10.0\loaders\*.dll") do (
    if exist "%%~fF" set "LOADERS=!LOADERS! "%%~fF""
  )
  if defined LOADERS (
    set "GDK_PIXBUF_MODULE_FILE=%TEMP%\mreader-gdk-pixbuf-loaders.cache"
    "%APP_DIR%gdk-pixbuf-query-loaders.exe" !LOADERS! > "!GDK_PIXBUF_MODULE_FILE!"
  )
)
"%APP_DIR%mreader.exe" %*
