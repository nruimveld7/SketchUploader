# ALDER App Help

ALDER is a focused Arduino CLI companion for library development workflows.

## What This App Does

- Select a sketch file and compile/upload it with Arduino CLI.
- Choose local library folders to pass to compile via `--library`.
- Manage board, port, and baud settings.
- View compile/upload logs and serial monitor output.

## Libraries Tab Behavior

- `Show installed libraries from arduino-cli` controls whether the installed-library list is shown.
- The list is informational and is refreshed from `arduino-cli lib list`.

## Notes

- App settings are loaded from `alder.config.json`.
- This About content is bundled at build time and is not loaded from the repo README at runtime.
