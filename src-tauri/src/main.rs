#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use serde_json::Map as JsonMap;
use serialport::{SerialPort, SerialPortType};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

const CONFIG_FILE_NAME: &str = "alder.config.json";
const LEGACY_CONFIG_FILE_NAME: &str = "sketchuploader.config.json";

#[derive(Debug, Serialize)]
struct CommandResult {
    command: String,
    success: bool,
    output: String,
}

#[derive(Debug, Serialize)]
struct BoardOption {
    name: String,
    fqbn: String,
}

#[derive(Debug, Serialize)]
struct PortOption {
    address: String,
    label: String,
    board_name: Option<String>,
    board_fqbn: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BoardConfigMenuOption {
    id: String,
    label: String,
    selected: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BoardConfigMenu {
    id: String,
    label: String,
    default_value_id: Option<String>,
    values: Vec<BoardConfigMenuOption>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledLibraryOption {
    name: String,
    version: String,
    latest_version: Option<String>,
    location: String,
    install_dir: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigResponse {
    config: AppConfig,
    source_path: Option<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartupCheckResult {
    ok: bool,
    arduino_cli_ok: bool,
    missing_cores: Vec<String>,
    notes: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum PickerMode {
    Path,
    Content,
    Both,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeFilePickerRequest {
    directory: bool,
    multiple: bool,
    mode: PickerMode,
    accept: Option<String>,
    max_files: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeFileSelection {
    path: String,
    name: String,
    text: Option<String>,
    bytes: Option<Vec<u8>>,
    size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
struct AppConfig {
    #[serde(rename = "$schemaVersion")]
    schema_version: u32,
    sketch_roots: Vec<String>,
    default_sketch_path: String,
    default_board_fqbn: String,
    default_port: String,
    default_baud: u32,
    preferences: PreferencesConfig,
    libraries: LibrariesConfig,
    tools: ToolsConfig,
    startup_checks: StartupChecksConfig,
    build: BuildConfig,
    about: AboutConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
struct PreferencesConfig {
    theme: String,
    verbose_compile: bool,
    verbose_upload: bool,
    warnings: String,
    verify_after_upload: bool,
    clean_build: bool,
    auto_open_serial_on_upload_success: bool,
    additional_board_manager_urls: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
struct LibrariesConfig {
    selected_paths: Vec<String>,
    allow_installed_fallback: bool,
    show_installed_from_cli: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
struct ToolsConfig {
    required_cores: Vec<String>,
    programmer: String,
    board_options: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
struct StartupChecksConfig {
    enabled: bool,
    check_arduino_cli: bool,
    check_core_index: bool,
    check_required_cores: bool,
    auto_run_core_update: bool,
    prompt_install_missing_cores: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
struct BuildConfig {
    build_root: String,
    extra_compile_args: Vec<String>,
    extra_upload_args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
struct AboutConfig {
    readme_path: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            sketch_roots: Vec::new(),
            default_sketch_path: String::new(),
            default_board_fqbn: String::new(),
            default_port: String::new(),
            default_baud: 115200,
            preferences: PreferencesConfig::default(),
            libraries: LibrariesConfig::default(),
            tools: ToolsConfig::default(),
            startup_checks: StartupChecksConfig::default(),
            build: BuildConfig::default(),
            about: AboutConfig::default(),
        }
    }
}

impl Default for PreferencesConfig {
    fn default() -> Self {
        Self {
            theme: String::from("system"),
            verbose_compile: false,
            verbose_upload: false,
            warnings: String::from("default"),
            verify_after_upload: false,
            clean_build: false,
            auto_open_serial_on_upload_success: true,
            additional_board_manager_urls: Vec::new(),
        }
    }
}

impl Default for LibrariesConfig {
    fn default() -> Self {
        Self {
            selected_paths: Vec::new(),
            allow_installed_fallback: true,
            show_installed_from_cli: true,
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            required_cores: Vec::new(),
            programmer: String::new(),
            board_options: serde_json::json!({}),
        }
    }
}

impl Default for StartupChecksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_arduino_cli: true,
            check_core_index: true,
            check_required_cores: true,
            auto_run_core_update: false,
            prompt_install_missing_cores: true,
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            build_root: String::from("build"),
            extra_compile_args: Vec::new(),
            extra_upload_args: Vec::new(),
        }
    }
}

impl Default for AboutConfig {
    fn default() -> Self {
        Self {
            readme_path: String::from("README.md"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ListAllBoardsResponse {
    boards: Vec<ListAllBoard>,
}

#[derive(Debug, Deserialize)]
struct ListAllBoard {
    name: String,
    fqbn: String,
}

#[derive(Debug, Deserialize)]
struct BoardDetailsResponse {
    #[serde(default)]
    config_options: Vec<BoardDetailsConfigOption>,
}

#[derive(Debug, Deserialize)]
struct BoardDetailsConfigOption {
    option: String,
    #[serde(default)]
    option_label: String,
    #[serde(default)]
    values: Vec<BoardDetailsConfigValue>,
}

#[derive(Debug, Deserialize)]
struct BoardDetailsConfigValue {
    value: String,
    #[serde(default)]
    value_label: String,
    #[serde(default)]
    selected: bool,
}

struct SerialSession {
    writer: Arc<Mutex<Box<dyn SerialPort>>>,
    stop_tx: Sender<()>,
    reader_thread: JoinHandle<()>,
}

struct AppState {
    serial_session: Mutex<Option<SerialSession>>,
}

struct CliExecution {
    command: String,
    success: bool,
    stdout: String,
    stderr: String,
}

fn format_cli_output(stdout: &str, stderr: &str) -> String {
    let merged = format!("{}{}", stdout, stderr).trim().to_string();
    if merged.is_empty() {
        String::from("(no output)")
    } else {
        merged
    }
}

fn run_arduino_cli_raw(args: &[String]) -> Result<CliExecution, String> {
    let output = Command::new("arduino-cli")
        .args(args)
        .output()
        .map_err(|err| {
            format!(
                "Failed to run arduino-cli (is it installed and in PATH?): {}",
                err
            )
        })?;

    Ok(CliExecution {
        command: format!("arduino-cli {}", args.join(" ")),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn run_arduino_cli(args: &[String]) -> Result<CommandResult, String> {
    let execution = run_arduino_cli_raw(args)?;

    Ok(CommandResult {
        command: execution.command,
        success: execution.success,
        output: format_cli_output(&execution.stdout, &execution.stderr),
    })
}

fn run_arduino_cli_static(args: &[&str]) -> Result<CommandResult, String> {
    let owned = args.iter().map(|s| String::from(*s)).collect::<Vec<_>>();
    run_arduino_cli(&owned)
}

fn run_arduino_cli_raw_static(args: &[&str]) -> Result<CliExecution, String> {
    let owned = args.iter().map(|s| String::from(*s)).collect::<Vec<_>>();
    run_arduino_cli_raw(&owned)
}

#[tauri::command]
async fn install_arduino_cli() -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(|| {
        if let Ok(result) = run_arduino_cli_static(&["version"]) {
            if result.success {
                return Ok(CommandResult {
                    command: String::from("arduino-cli version"),
                    success: true,
                    output: String::from("arduino-cli is already installed."),
                });
            }
        }

        #[cfg(target_os = "windows")]
        {
            let args = vec![
                "install",
                "-e",
                "--id",
                "ArduinoSA.CLI",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ];
            let output = Command::new("winget").args(&args).output().map_err(|err| {
                format!(
                    "Failed to launch winget. Install arduino-cli manually or ensure winget is available: {}",
                    err
                )
            })?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            return Ok(CommandResult {
                command: format!("winget {}", args.join(" ")),
                success: output.status.success(),
                output: format_cli_output(&stdout, &stderr),
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err(String::from(
                "Automatic install is currently supported on Windows only. Install arduino-cli manually and restart ALDER.",
            ))
        }
    })
    .await
    .map_err(|err| format!("arduino-cli install task failed: {}", err))?
}
fn config_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut push_candidates = |base_dir: &Path| {
        let primary = base_dir.join(CONFIG_FILE_NAME);
        if !paths.contains(&primary) {
            paths.push(primary);
        }

        let legacy = base_dir.join(LEGACY_CONFIG_FILE_NAME);
        if !paths.contains(&legacy) {
            paths.push(legacy);
        }
    };

    if let Ok(cwd) = std::env::current_dir() {
        if cwd
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("src-tauri"))
            .unwrap_or(false)
        {
            if let Some(parent) = cwd.parent() {
                push_candidates(parent);
            }
        }
        push_candidates(&cwd);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            push_candidates(exe_dir);
        }
    }

    paths
}

fn sanitize_board_options(raw: &serde_json::Value) -> (serde_json::Value, Vec<String>) {
    let mut warnings = Vec::new();
    let mut cleaned = JsonMap::new();

    if let Some(object) = raw.as_object() {
        for (menu_id, option_id) in object {
            let trimmed_menu = menu_id.trim();
            let trimmed_option = option_id.as_str().map(str::trim).unwrap_or_default();
            if trimmed_menu.is_empty() || trimmed_option.is_empty() {
                warnings.push(String::from(
                    "tools.boardOptions contained empty or non-string entries; invalid entries were removed.",
                ));
                continue;
            }
            cleaned.insert(
                trimmed_menu.to_string(),
                serde_json::Value::String(trimmed_option.to_string()),
            );
        }
    } else if !raw.is_null() {
        warnings.push(String::from(
            "tools.boardOptions was not an object and has been reset to {}.",
        ));
    }

    (serde_json::Value::Object(cleaned), warnings)
}

fn parse_fqbn_with_overrides(fqbn: &str) -> (String, BTreeMap<String, String>) {
    let trimmed = fqbn.trim();
    if trimmed.is_empty() {
        return (String::new(), BTreeMap::new());
    }

    let mut sections = trimmed.split(':');
    let Some(package) = sections.next() else {
        return (trimmed.to_string(), BTreeMap::new());
    };
    let Some(arch) = sections.next() else {
        return (trimmed.to_string(), BTreeMap::new());
    };
    let Some(board) = sections.next() else {
        return (trimmed.to_string(), BTreeMap::new());
    };

    let base = format!("{}:{}:{}", package, arch, board);
    let remaining = sections.collect::<Vec<_>>().join(":");
    if remaining.trim().is_empty() {
        return (base, BTreeMap::new());
    }

    let mut overrides = BTreeMap::new();
    for pair in remaining.split(',') {
        let mut parts = pair.splitn(2, '=');
        let menu_id = parts.next().map(str::trim).unwrap_or_default();
        let option_id = parts.next().map(str::trim).unwrap_or_default();
        if menu_id.is_empty() || option_id.is_empty() {
            continue;
        }
        overrides.insert(menu_id.to_string(), option_id.to_string());
    }

    (base, overrides)
}

fn normalize_config(config: &mut AppConfig) -> Vec<String> {
    let mut warnings = Vec::new();

    if config.schema_version == 0 {
        config.schema_version = 1;
        warnings.push(String::from("Invalid $schemaVersion. Defaulted to 1."));
    }

    if config.default_baud == 0 {
        config.default_baud = 115200;
        warnings.push(String::from("defaultBaud was 0 and has been reset to 115200."));
    }

    let warnings_level = config.preferences.warnings.trim().to_ascii_lowercase();
    let valid_warnings = ["none", "default", "more", "all"];
    if !valid_warnings.contains(&warnings_level.as_str()) {
        config.preferences.warnings = String::from("default");
        warnings.push(String::from(
            "preferences.warnings was invalid. Supported values: none, default, more, all.",
        ));
    } else {
        config.preferences.warnings = warnings_level;
    }

    let theme = config.preferences.theme.trim().to_ascii_lowercase();
    let valid_themes = ["system", "light", "dark"];
    if !valid_themes.contains(&theme.as_str()) {
        config.preferences.theme = String::from("system");
        warnings.push(String::from(
            "preferences.theme was invalid. Supported values: system, light, dark.",
        ));
    } else {
        config.preferences.theme = theme;
    }

    config.libraries.selected_paths = config
        .libraries
        .selected_paths
        .iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();

    config.tools.required_cores = config
        .tools
        .required_cores
        .iter()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();

    let (sanitized_board_options, board_option_warnings) = sanitize_board_options(&config.tools.board_options);
    config.tools.board_options = sanitized_board_options;
    warnings.extend(board_option_warnings);

    let (base_fqbn, parsed_overrides) = parse_fqbn_with_overrides(&config.default_board_fqbn);
    if base_fqbn != config.default_board_fqbn {
        config.default_board_fqbn = base_fqbn;
        warnings.push(String::from(
            "defaultBoardFqbn included option overrides. Stored only the base FQBN.",
        ));
    }

    if !parsed_overrides.is_empty() {
        let options_obj = config
            .tools
            .board_options
            .as_object_mut()
            .expect("board_options should be object after sanitize");
        for (menu_id, option_id) in parsed_overrides {
            options_obj
                .entry(menu_id)
                .or_insert_with(|| serde_json::Value::String(option_id));
        }
        warnings.push(String::from(
            "Moved board option overrides from defaultBoardFqbn into tools.boardOptions.",
        ));
    }

    config.build.extra_compile_args = config
        .build
        .extra_compile_args
        .iter()
        .map(|arg| arg.trim().to_string())
        .filter(|arg| !arg.is_empty())
        .collect();

    config.build.extra_upload_args = config
        .build
        .extra_upload_args
        .iter()
        .map(|arg| arg.trim().to_string())
        .filter(|arg| !arg.is_empty())
        .collect();

    warnings
}

fn load_app_config_internal() -> ConfigResponse {
    let mut last_error: Option<String> = None;

    for candidate in config_candidate_paths() {
        if !candidate.exists() {
            continue;
        }

        match fs::read_to_string(&candidate) {
            Ok(contents) => match serde_json::from_str::<AppConfig>(&contents) {
                Ok(mut config) => {
                    let warnings = normalize_config(&mut config);
                    return ConfigResponse {
                        config,
                        source_path: Some(candidate.to_string_lossy().to_string()),
                        warnings,
                    };
                }
                Err(err) => {
                    last_error = Some(format!(
                        "Failed to parse config at {}: {}",
                        candidate.to_string_lossy(),
                        err
                    ));
                }
            },
            Err(err) => {
                last_error = Some(format!(
                    "Failed to read config at {}: {}",
                    candidate.to_string_lossy(),
                    err
                ));
            }
        }
    }

    let mut fallback = AppConfig::default();
    let mut warnings = normalize_config(&mut fallback);
    if let Some(err) = last_error {
        warnings.push(err);
    }

    if let Ok(saved) = save_app_config_internal(fallback.clone()) {
        return ConfigResponse {
            config: saved.config,
            source_path: saved.source_path,
            warnings,
        };
    }

    ConfigResponse {
        config: fallback,
        source_path: None,
        warnings,
    }
}

fn resolve_config_write_path() -> Result<PathBuf, String> {
    let candidates = config_candidate_paths();
    
    if let Some(first) = candidates.first() {
        return Ok(first.clone());
    }

    Err(String::from(
        "Could not resolve a writable config path from current environment.",
    ))
}

fn save_app_config_internal(config: AppConfig) -> Result<ConfigResponse, String> {
    let mut normalized = config;
    let warnings = normalize_config(&mut normalized);
    let target_path = resolve_config_write_path()?;

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create config directory {}: {}",
                parent.to_string_lossy(),
                err
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(&normalized)
        .map_err(|err| format!("Failed to serialize app config: {}", err))?;

    fs::write(&target_path, payload).map_err(|err| {
        format!(
            "Failed to write config to {}: {}",
            target_path.to_string_lossy(),
            err
        )
    })?;

    Ok(ConfigResponse {
        config: normalized,
        source_path: Some(target_path.to_string_lossy().to_string()),
        warnings,
    })
}

fn close_serial_session_internal(state: &State<'_, AppState>) {
    let mut guard = match state.serial_session.lock() {
        Ok(lock) => lock,
        Err(_) => return,
    };

    if let Some(session) = guard.take() {
        let _ = session.stop_tx.send(());
        let _ = session.reader_thread.join();
    }
}

fn resolve_sketch_target(sketch_file: &str) -> Result<PathBuf, String> {
    let sketch_path = Path::new(sketch_file);
    if !sketch_path.exists() {
        return Err(format!("Sketch path does not exist: {}", sketch_file));
    }

    if sketch_path.is_dir() {
        return Ok(sketch_path.to_path_buf());
    }

    let extension = sketch_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if extension != "ino" {
        return Err(String::from("Please select an .ino sketch file."));
    }

    sketch_path
        .parent()
        .map(|parent| parent.to_path_buf())
        .ok_or_else(|| String::from("Sketch file has no parent directory."))
}

fn build_compile_args(sketch_arg: &str, fqbn: &str, config: &AppConfig) -> Vec<String> {
    let mut args = vec![
        String::from("compile"),
        String::from("--fqbn"),
        fqbn.to_string(),
        String::from("--warnings"),
        config.preferences.warnings.clone(),
    ];

    if config.preferences.verbose_compile {
        args.push(String::from("--verbose"));
    }
    if config.preferences.clean_build {
        args.push(String::from("--clean"));
    }

    for path in &config.libraries.selected_paths {
        args.push(String::from("--library"));
        args.push(path.clone());
    }

    args.extend(config.build.extra_compile_args.clone());
    args.push(sketch_arg.to_string());
    args
}

fn build_upload_args(
    sketch_arg: &str,
    fqbn: &str,
    port: &str,
    config: &AppConfig,
) -> Vec<String> {
    let mut args = vec![
        String::from("upload"),
        String::from("-p"),
        port.to_string(),
        String::from("--fqbn"),
        fqbn.to_string(),
    ];

    if config.preferences.verbose_upload {
        args.push(String::from("--verbose"));
    }
    if config.preferences.verify_after_upload {
        args.push(String::from("--verify"));
    }
    if !config.tools.programmer.trim().is_empty() {
        args.push(String::from("--programmer"));
        args.push(config.tools.programmer.trim().to_string());
    }

    args.extend(config.build.extra_upload_args.clone());
    args.push(sketch_arg.to_string());
    args
}

#[tauri::command]
fn get_app_config() -> ConfigResponse {
    load_app_config_internal()
}

#[tauri::command]
fn save_app_config(config: AppConfig) -> Result<ConfigResponse, String> {
    save_app_config_internal(config)
}

#[tauri::command]
fn run_startup_checks() -> StartupCheckResult {
    let config_response = load_app_config_internal();
    let config = config_response.config;

    if !config.startup_checks.enabled {
        return StartupCheckResult {
            ok: true,
            arduino_cli_ok: true,
            missing_cores: Vec::new(),
            notes: vec![String::from("Startup checks are disabled by config.")],
        };
    }

    let mut notes = Vec::new();
    let mut missing_cores = Vec::new();

    let arduino_cli_ok = if config.startup_checks.check_arduino_cli {
        match run_arduino_cli_static(&["version"]) {
            Ok(result) if result.success => true,
            Ok(result) => {
                notes.push(format!("arduino-cli version failed: {}", result.output));
                false
            }
            Err(err) => {
                notes.push(err);
                false
            }
        }
    } else {
        true
    };

    if arduino_cli_ok
        && config.startup_checks.check_core_index
        && config.startup_checks.auto_run_core_update
    {
        match run_arduino_cli_static(&["core", "update-index"]) {
            Ok(result) if result.success => notes.push(String::from("Core index updated.")),
            Ok(result) => notes.push(format!("Core index update failed: {}", result.output)),
            Err(err) => notes.push(format!("Core index update failed: {}", err)),
        }
    }

    if arduino_cli_ok
        && config.startup_checks.check_required_cores
        && !config.tools.required_cores.is_empty()
    {
        match run_arduino_cli_static(&["core", "list"]) {
            Ok(result) if result.success => {
                for core in &config.tools.required_cores {
                    if !result.output.contains(core) {
                        missing_cores.push(core.clone());
                    }
                }
            }
            Ok(result) => notes.push(format!("Unable to list installed cores: {}", result.output)),
            Err(err) => notes.push(format!("Unable to list installed cores: {}", err)),
        }
    }

    let ok = arduino_cli_ok && missing_cores.is_empty();
    StartupCheckResult {
        ok,
        arduino_cli_ok,
        missing_cores,
        notes,
    }
}

#[tauri::command]
async fn list_arduino_boards() -> Result<Vec<BoardOption>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let result = run_arduino_cli_raw_static(&["board", "listall", "--format", "json"])?;
        if !result.success {
            return Err(format_cli_output(&result.stdout, &result.stderr));
        }

        let parsed: ListAllBoardsResponse =
            serde_json::from_str(&result.stdout).map_err(|err| format!("Invalid board JSON: {}", err))?;

        let mut boards: Vec<BoardOption> = parsed
            .boards
            .into_iter()
            .map(|board| BoardOption {
                name: board.name,
                fqbn: board.fqbn,
            })
            .collect();

        boards.sort_by(|a, b| a.name.cmp(&b.name).then(a.fqbn.cmp(&b.fqbn)));
        Ok(boards)
    })
    .await
    .map_err(|err| format!("Board refresh task failed: {}", err))?
}

#[tauri::command]
async fn list_arduino_ports() -> Result<Vec<PortOption>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let available = serialport::available_ports()
            .map_err(|err| format!("Failed to enumerate serial ports: {}", err))?;

        let mut ports: Vec<PortOption> = available
            .into_iter()
            .map(|info| {
                let label = match info.port_type {
                    SerialPortType::UsbPort(usb) => {
                        let clean = |raw: Option<String>| -> Option<String> {
                            raw.map(|value| {
                                value
                                    .trim()
                                    .trim_matches(|ch| matches!(ch, '[' | ']'))
                                    .trim()
                                    .to_string()
                            })
                            .filter(|value| !value.is_empty())
                        };

                        let product = clean(usb.product);
                        let manufacturer = clean(usb.manufacturer);

                        let append_port_if_missing = |name: &str, port_name: &str| -> String {
                            let name_lower = name.to_ascii_lowercase();
                            let port_lower = port_name.to_ascii_lowercase();
                            if name_lower.contains(&port_lower) {
                                name.to_string()
                            } else {
                                format!("{} ({})", name, port_name)
                            }
                        };

                        if let Some(product_name) = product {
                            append_port_if_missing(&product_name, &info.port_name)
                        } else {
                            let _ = manufacturer;
                            info.port_name.clone()
                        }
                    }
                    SerialPortType::BluetoothPort => format!("{} [Bluetooth]", info.port_name),
                    SerialPortType::PciPort => format!("{} [PCI]", info.port_name),
                    SerialPortType::Unknown => info.port_name.clone(),
                };

                PortOption {
                    address: info.port_name,
                    label,
                    board_name: None,
                    board_fqbn: None,
                }
            })
            .collect();

        ports.sort_by(|a, b| a.address.cmp(&b.address));
        Ok(ports)
    })
    .await
    .map_err(|err| format!("Port refresh task failed: {}", err))?
}

#[tauri::command]
async fn get_board_option_menus(fqbn: String) -> Result<Vec<BoardConfigMenu>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let trimmed_fqbn = fqbn.trim().to_string();
        if trimmed_fqbn.is_empty() {
            return Ok(Vec::new());
        }

        let args = vec![
            String::from("board"),
            String::from("details"),
            String::from("-b"),
            trimmed_fqbn,
            String::from("--format"),
            String::from("json"),
        ];

        let result = run_arduino_cli_raw(&args)?;
        if !result.success {
            return Err(format_cli_output(&result.stdout, &result.stderr));
        }

        let parsed: BoardDetailsResponse =
            serde_json::from_str(&result.stdout).map_err(|err| format!("Invalid board details JSON: {}", err))?;

        let menus = parsed
            .config_options
            .into_iter()
            .map(|option| {
                let values: Vec<BoardConfigMenuOption> = option
                    .values
                    .into_iter()
                    .map(|value| BoardConfigMenuOption {
                        id: value.value.clone(),
                        label: if value.value_label.trim().is_empty() {
                            value.value
                        } else {
                            value.value_label
                        },
                        selected: value.selected,
                    })
                    .collect();

                let default_value_id = values
                    .iter()
                    .find(|value| value.selected)
                    .map(|value| value.id.clone());

                BoardConfigMenu {
                    id: option.option.clone(),
                    label: if option.option_label.trim().is_empty() {
                        option.option
                    } else {
                        option.option_label
                    },
                    default_value_id,
                    values,
                }
            })
            .collect();

        Ok(menus)
    })
    .await
    .map_err(|err| format!("Board details task failed: {}", err))?
}

#[tauri::command]
async fn list_installed_libraries() -> Result<Vec<InstalledLibraryOption>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let result = run_arduino_cli_raw_static(&["lib", "list", "--format", "json"])?;
        if !result.success {
            return Err(format_cli_output(&result.stdout, &result.stderr));
        }

        let parsed: serde_json::Value = serde_json::from_str(&result.stdout)
            .map_err(|err| format!("Invalid library JSON: {}", err))?;
        let entries = parsed
            .get("installed_libraries")
            .and_then(|value| value.as_array())
            .ok_or_else(|| String::from("Unexpected library list format from arduino-cli."))?;

        let mut libraries = Vec::new();

        for entry in entries {
            let library = entry.get("library").unwrap_or(entry);
            let name = library
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("Unknown")
                .trim()
                .to_string();
            let version = library
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let location = library
                .get("location")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .trim()
                .to_string();
            let install_dir = library
                .get("install_dir")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let latest_version = entry
                .get("release")
                .and_then(|release| release.get("version"))
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            libraries.push(InstalledLibraryOption {
                name,
                version,
                latest_version,
                location,
                install_dir,
            });
        }

        libraries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(libraries)
    })
    .await
    .map_err(|err| format!("Installed library refresh task failed: {}", err))?
}

#[tauri::command]
async fn compile_sketch(sketch_file: String, fqbn: String) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if fqbn.trim().is_empty() {
            return Err(String::from("Board FQBN is required for compile."));
        }

        let config = load_app_config_internal().config;
        let sketch_target = resolve_sketch_target(&sketch_file)?;
        let sketch_arg = sketch_target.to_string_lossy().to_string();
        let args = build_compile_args(sketch_arg.as_str(), fqbn.trim(), &config);
        run_arduino_cli(&args)
    })
    .await
    .map_err(|err| format!("Compile task failed: {}", err))?
}

#[tauri::command]
async fn upload_sketch(sketch_file: String, fqbn: String, port: String) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if fqbn.trim().is_empty() {
            return Err(String::from("Board FQBN is required for upload."));
        }
        if port.trim().is_empty() {
            return Err(String::from("Port is required for upload."));
        }

        let config = load_app_config_internal().config;
        let sketch_target = resolve_sketch_target(&sketch_file)?;
        let sketch_arg = sketch_target.to_string_lossy().to_string();
        let args = build_upload_args(sketch_arg.as_str(), fqbn.trim(), port.trim(), &config);
        run_arduino_cli(&args)
    })
    .await
    .map_err(|err| format!("Upload task failed: {}", err))?
}

#[tauri::command]
fn open_serial_monitor(
    app: AppHandle,
    state: State<'_, AppState>,
    port: String,
    baud_rate: u32,
) -> Result<(), String> {
    if port.trim().is_empty() {
        return Err(String::from("Port is required to open serial monitor."));
    }

    close_serial_session_internal(&state);

    let mut serial_port = serialport::new(port.trim(), baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|err| format!("Failed to open serial port: {}", err))?;

    // Mirror Arduino IDE behavior: pulse DTR/RTS on connect so boards that
    // implement auto-reset reboot when the serial monitor opens.
    let _ = serial_port.write_data_terminal_ready(false);
    let _ = serial_port.write_request_to_send(false);
    thread::sleep(Duration::from_millis(50));
    let _ = serial_port.write_data_terminal_ready(true);
    let _ = serial_port.write_request_to_send(true);
    thread::sleep(Duration::from_millis(50));
    let _ = serial_port.clear(serialport::ClearBuffer::All);

    let reader_port = serial_port
        .try_clone()
        .map_err(|err| format!("Failed to clone serial port: {}", err))?;

    let writer = Arc::new(Mutex::new(serial_port));
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let app_handle = app.clone();

    let reader_thread = thread::spawn(move || {
        let mut port_reader = reader_port;
        let mut raw = [0_u8; 512];
        let mut line_buffer = String::new();

        loop {
            match stop_rx.try_recv() {
                Ok(()) | Err(TryRecvError::Disconnected) => break,
                Err(TryRecvError::Empty) => {}
            }

            match port_reader.read(&mut raw) {
                Ok(bytes_read) if bytes_read > 0 => {
                    let chunk = String::from_utf8_lossy(&raw[..bytes_read]);
                    line_buffer.push_str(&chunk);

                    while let Some(idx) = line_buffer.find('\n') {
                        let line = line_buffer[..idx].trim_end_matches('\r').to_string();
                        let _ = app_handle.emit("serial-data", line);
                        line_buffer = line_buffer[idx + 1..].to_string();
                    }
                }
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(err) => {
                    let _ = app_handle.emit("serial-error", format!("Serial read error: {}", err));
                    break;
                }
            }
        }

        if !line_buffer.trim().is_empty() {
            let _ = app_handle.emit("serial-data", line_buffer.trim().to_string());
        }
    });

    let session = SerialSession {
        writer,
        stop_tx,
        reader_thread,
    };

    let mut guard = state
        .serial_session
        .lock()
        .map_err(|_| String::from("Failed to store serial session."))?;
    *guard = Some(session);

    Ok(())
}

#[tauri::command]
fn close_serial_monitor(state: State<'_, AppState>) {
    close_serial_session_internal(&state);
}

#[tauri::command]
fn write_serial_monitor(state: State<'_, AppState>, payload: String) -> Result<(), String> {
    let guard = state
        .serial_session
        .lock()
        .map_err(|_| String::from("Failed to access serial monitor state."))?;

    let session = guard
        .as_ref()
        .ok_or_else(|| String::from("Serial monitor is not open."))?;

    let mut writer = session
        .writer
        .lock()
        .map_err(|_| String::from("Failed to lock serial writer."))?;

    writer
        .write_all(payload.as_bytes())
        .map_err(|err| format!("Serial write failed: {}", err))?;
    writer
        .flush()
        .map_err(|err| format!("Serial flush failed: {}", err))?;

    Ok(())
}

fn parse_accept_extensions(accept: Option<&str>) -> Vec<String> {
    let Some(raw) = accept else {
        return Vec::new();
    };

    raw.split(',')
        .map(|token| token.trim().to_lowercase())
        .filter_map(|token| {
            if token.is_empty() {
                return None;
            }

            if token.starts_with('.') && token.len() > 1 {
                return Some(token.trim_start_matches('.').to_string());
            }

            if token.ends_with("/*") {
                return None;
            }

            if let Some((_, subtype)) = token.split_once('/') {
                if !subtype.is_empty() && subtype != "*" {
                    return Some(subtype.to_string());
                }
            }

            None
        })
        .collect()
}

fn to_display_path(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();

    #[cfg(windows)]
    {
        if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{}", rest);
        }
        if let Some(rest) = raw.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }

    raw
}

#[tauri::command]
fn pick_native_files(
    request: NativeFilePickerRequest,
) -> Result<Option<Vec<NativeFileSelection>>, String> {
    let mut dialog = rfd::FileDialog::new();

    if !request.directory {
        let extensions = parse_accept_extensions(request.accept.as_deref());
        if !extensions.is_empty() {
            let ext_refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter("Allowed files", &ext_refs);
        }
    }

    let picked: Option<Vec<PathBuf>> = if request.directory {
        if request.multiple {
            dialog.pick_folders()
        } else {
            dialog.pick_folder().map(|path| vec![path])
        }
    } else if request.multiple {
        dialog.pick_files()
    } else {
        dialog.pick_file().map(|path| vec![path])
    };

    let Some(paths) = picked else {
        return Ok(None);
    };

    let limit = request.max_files.unwrap_or(paths.len()).max(1);
    let selected_paths: Vec<PathBuf> = paths.into_iter().take(limit).collect();
    let include_content = matches!(request.mode, PickerMode::Content | PickerMode::Both);

    let mut selections = Vec::with_capacity(selected_paths.len());
    for selected_path in selected_paths {
        let absolute_path = fs::canonicalize(&selected_path).unwrap_or_else(|_| selected_path.clone());
        let path_string = to_display_path(&absolute_path);
        let name = absolute_path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|| path_string.clone());
        let size = fs::metadata(&absolute_path).ok().map(|metadata| metadata.len());

        let (bytes, text) = if include_content && absolute_path.is_file() {
            let content = fs::read(&absolute_path)
                .map_err(|err| format!("Failed to read {}: {}", path_string, err))?;
            let utf8 = String::from_utf8(content.clone()).ok();
            (Some(content), utf8)
        } else {
            (None, None)
        };

        selections.push(NativeFileSelection {
            path: path_string,
            name,
            text,
            bytes,
            size,
        });
    }

    Ok(Some(selections))
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            serial_session: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_app_config,
            save_app_config,
            run_startup_checks,
            install_arduino_cli,
            list_arduino_boards,
            list_arduino_ports,
            get_board_option_menus,
            list_installed_libraries,
            compile_sketch,
            upload_sketch,
            open_serial_monitor,
            close_serial_monitor,
            write_serial_monitor,
            pick_native_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

