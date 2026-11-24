use crate::config::AppConfig;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
struct ArgEntry {
    field: &'static str,
    text: String,
}

pub fn builtin_defaults() -> Value {
    serde_json::json!({
        "video": {
            "videoCodec": "", "videoEncoder": "", "hwdec": "", "bitrate": 0,
            "maxSize": 0, "maxFps": 0, "crop": "", "angle": "0",
            "renderFit": "letterbox", "videoBuffer": 0
        },
        "source": {
            "videoSource": "display", "displayMode": "main", "displayId": 0,
            "newDisplayRes": "", "newDisplayDpi": "", "noVdDestroy": true,
            "cameraFacing": "", "cameraSize": "", "cameraFps": "",
            "cameraZoom": 1, "cameraTorch": false
        },
        "audio": {
            "audioOff": true, "audioSource": "output", "audioCodec": "opus",
            "audioEncoder": "", "audioDup": false, "audioBitrate": 0,
            "audioBuffer": 50, "audioOutBuffer": 10
        },
        "control": {
            "noControl": false, "keyboard": "sdk", "noKeyRepeat": false,
            "mouse": "sdk", "gamepad": "disabled", "shortMod": [],
            "showTouches": false, "turnScreenOff": false, "screenOffTimeout": 0,
            "stayAwake": false, "keepActive": false, "powerOffOnClose": false,
            "clipboardOff": false
        },
        "record": {
            "recordOn": false, "recordPath": "D:/Recordings/session.mp4",
            "recordFormat": "mp4", "recordOrientation": "0", "noWindow": false,
            "timeLimit": 0, "pauseOnExit": "false"
        },
        "window": {
            "windowTitle": "", "borderless": false, "fullscreen": false,
            "alwaysOnTop": false, "aspectLockOff": false, "bgColor": "#222222",
            "windowWidth": 0, "windowHeight": 0, "windowX": 0, "windowY": 0
        },
        "apps": { "startApp": "" },
        "advanced": {
            "customArgs": "", "verbosity": "info", "printFps": false,
            "cleanupOff": false, "port": "27183:27199", "tunnelHost": "localhost"
        }
    })
}

pub fn resolve_args(config: &AppConfig, device_id: &str) -> Vec<String> {
    let mut defaults = config.defaults.clone();
    if !defaults.is_object() {
        defaults = Value::Object(Map::new());
    }
    migrate_toggle_keys(&mut defaults);
    if config.migrations.get("inputModeDefaults") != Some(&Value::Bool(true)) {
        migrate_input_modes(&mut defaults);
    }

    let mut params = merge_params(&builtin_defaults(), Some(&defaults));
    let mut device_keys = HashSet::new();
    if let Some(entry) = config.overrides.get(device_id) {
        if entry.enabled {
            let mut values = entry.values.clone();
            migrate_toggle_keys(&mut values);
            if config.migrations.get("inputModeDefaults") != Some(&Value::Bool(true)) {
                migrate_input_modes(&mut values);
            }
            collect_device_keys(&values, &mut device_keys);
            params = merge_params(&params, Some(&values));
        }
    }
    let errors = validate_params(&params);
    build_args(&params, &errors, &device_keys)
}

pub fn merge_params(base: &Value, over: Option<&Value>) -> Value {
    let mut out = base.clone();
    let Some(over) = over.filter(|v| v.is_object()) else {
        return out;
    };
    if !out.is_object() {
        out = Value::Object(Map::new());
    }
    let out_map = out.as_object_mut().unwrap();
    for (group, fields) in over.as_object().unwrap() {
        let Some(fields) = fields.as_object() else {
            continue;
        };
        let target = out_map
            .entry(group.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        if !target.is_object() {
            *target = Value::Object(Map::new());
        }
        let target = target.as_object_mut().unwrap();
        for (key, value) in fields {
            target.insert(key.clone(), value.clone());
        }
    }
    out
}

fn collect_device_keys(values: &Value, out: &mut HashSet<String>) {
    let Some(groups) = values.as_object() else {
        return;
    };
    for (group, fields) in groups {
        let Some(fields) = fields.as_object() else {
            continue;
        };
        for key in fields.keys() {
            out.insert(format!("{group}.{key}"));
        }
    }
}

fn migrate_toggle_keys(values: &mut Value) {
    let Some(groups) = values.as_object_mut() else {
        return;
    };
    for (group, from, to) in [
        ("control", "clipboard", "clipboardOff"),
        ("window", "aspectLock", "aspectLockOff"),
        ("advanced", "cleanup", "cleanupOff"),
    ] {
        let Some(fields) = groups.get_mut(group).and_then(Value::as_object_mut) else {
            continue;
        };
        if fields.contains_key(to) {
            continue;
        }
        if let Some(old) = fields.remove(from) {
            fields.insert(to.to_string(), Value::Bool(old == Value::Bool(false)));
        }
    }
}

fn migrate_input_modes(values: &mut Value) {
    let Some(fields) = values.get_mut("control").and_then(Value::as_object_mut) else {
        return;
    };
    for (key, default) in [
        ("keyboard", "sdk"),
        ("mouse", "sdk"),
        ("gamepad", "disabled"),
    ] {
        if fields.get(key) == Some(&Value::String("uhid".to_string())) {
            fields.insert(key.to_string(), Value::String(default.to_string()));
        }
    }
}

pub fn build_args(
    params: &Value,
    errors: &HashSet<String>,
    device_keys: &HashSet<String>,
) -> Vec<String> {
    let builtin = builtin_defaults();
    let video = group(params, "video");
    let source = group(params, "source");
    let audio = group(params, "audio");
    let control = group(params, "control");
    let record = group(params, "record");
    let window = group(params, "window");
    let apps = group(params, "apps");
    let advanced = group(params, "advanced");
    let mut entries = Vec::new();

    let is_flex = text(source, "videoSource") != "camera" && text(source, "displayMode") == "flex";
    let is_camera = text(source, "videoSource") == "camera";

    let video_codec = text(video, "videoCodec");
    if !video_codec.is_empty() && !bad(errors, "video", "videoCodec") {
        push(
            &mut entries,
            "video.videoCodec",
            format!("--video-codec={video_codec}"),
            true,
        );
    }
    let video_encoder = text(video, "videoEncoder");
    if !video_encoder.is_empty() && !bad(errors, "video", "videoEncoder") {
        push(
            &mut entries,
            "video.videoEncoder",
            format!("--video-encoder={video_encoder}"),
            true,
        );
    }
    let hwdec = text(video, "hwdec");
    if !hwdec.is_empty()
        && !bad(errors, "video", "hwdec")
        && (hwdec == "disabled" || !flag(record, "noWindow"))
    {
        push(
            &mut entries,
            "video.hwdec",
            format!("--hwdec={hwdec}"),
            true,
        );
    }
    push_number(
        &mut entries,
        "video.bitrate",
        number(video, "bitrate"),
        |n| format!("-b {n}M"),
        |n| n > 0.0,
        errors,
        "video",
        "bitrate",
    );
    push_number(
        &mut entries,
        "video.maxSize",
        number(video, "maxSize"),
        |n| format!("-m {n}"),
        |n| n > 0.0,
        errors,
        "video",
        "maxSize",
    );
    push_number(
        &mut entries,
        "video.maxFps",
        number(video, "maxFps"),
        |n| format!("--max-fps={n}"),
        |n| n > 0.0,
        errors,
        "video",
        "maxFps",
    );
    let crop = text(video, "crop");
    if !crop.is_empty() && !is_flex && !bad(errors, "video", "crop") {
        push(&mut entries, "video.crop", format!("--crop={crop}"), true);
    }
    let angle = text(video, "angle");
    if angle != "0" && !bad(errors, "video", "angle") {
        push(
            &mut entries,
            "video.angle",
            format!("--angle={angle}"),
            true,
        );
    }
    let render_fit = text(video, "renderFit");
    if !render_fit.is_empty() && render_fit != "letterbox" && !bad(errors, "video", "renderFit") {
        push(
            &mut entries,
            "video.renderFit",
            format!("--render-fit={render_fit}"),
            true,
        );
    }
    push_number(
        &mut entries,
        "video.videoBuffer",
        number(video, "videoBuffer"),
        |n| format!("--video-buffer={n}"),
        |n| n > 0.0,
        errors,
        "video",
        "videoBuffer",
    );

    if is_camera {
        push(
            &mut entries,
            "source.videoSource",
            "--video-source=camera".to_string(),
            true,
        );
        let facing = text(source, "cameraFacing");
        if !facing.is_empty() {
            push(
                &mut entries,
                "source.cameraFacing",
                format!("--camera-facing={facing}"),
                true,
            );
        }
        let size = text(source, "cameraSize");
        if !size.is_empty() && !bad(errors, "source", "cameraSize") {
            push(
                &mut entries,
                "source.cameraSize",
                format!("--camera-size={size}"),
                true,
            );
        }
        push_number(
            &mut entries,
            "source.cameraFps",
            number(source, "cameraFps"),
            |n| format!("--camera-fps={n}"),
            |n| n > 0.0,
            errors,
            "source",
            "cameraFps",
        );
        let zoom = number(source, "cameraZoom");
        if zoom != 1.0 && zoom > 0.0 && !bad(errors, "source", "cameraZoom") {
            push(
                &mut entries,
                "source.cameraZoom",
                format!("--camera-zoom={zoom}"),
                true,
            );
        }
        if flag(source, "cameraTorch") {
            push(
                &mut entries,
                "source.cameraTorch",
                "--camera-torch".to_string(),
                true,
            );
        }
    } else if text(source, "displayMode") == "new" || is_flex {
        let res = if bad(errors, "source", "newDisplayRes") {
            String::new()
        } else {
            text(source, "newDisplayRes")
        };
        let dpi = if bad(errors, "source", "newDisplayDpi") {
            String::new()
        } else {
            text(source, "newDisplayDpi")
        };
        let spec = if res.is_empty() && dpi.is_empty() {
            String::new()
        } else if dpi.is_empty() {
            res
        } else {
            format!("{res}/{dpi}")
        };
        push(
            &mut entries,
            "source.displayMode",
            if spec.is_empty() {
                "--new-display".to_string()
            } else {
                format!("--new-display={spec}")
            },
            true,
        );
        if flag(source, "noVdDestroy") {
            push(
                &mut entries,
                "source.noVdDestroy",
                "--no-vd-destroy-content".to_string(),
                true,
            );
        }
        if is_flex {
            push(
                &mut entries,
                "source.displayMode",
                "--flex-display".to_string(),
                true,
            );
        }
    } else {
        push_number(
            &mut entries,
            "source.displayId",
            number(source, "displayId"),
            |n| format!("--display-id={n}"),
            |n| n > 0.0,
            errors,
            "source",
            "displayId",
        );
    }

    if flag(audio, "audioOff") {
        push(
            &mut entries,
            "audio.audioOff",
            "--no-audio".to_string(),
            true,
        );
    } else {
        let audio_source = text(audio, "audioSource");
        if !audio_source.is_empty() && audio_source != "output" {
            push(
                &mut entries,
                "audio.audioSource",
                format!("--audio-source={audio_source}"),
                true,
            );
        }
        let audio_codec = text(audio, "audioCodec");
        if !audio_codec.is_empty() && audio_codec != "opus" {
            push(
                &mut entries,
                "audio.audioCodec",
                format!("--audio-codec={audio_codec}"),
                true,
            );
        }
        let audio_encoder = text(audio, "audioEncoder");
        if !audio_encoder.is_empty() && !bad(errors, "audio", "audioEncoder") {
            push(
                &mut entries,
                "audio.audioEncoder",
                format!("--audio-encoder={audio_encoder}"),
                true,
            );
        }
        if flag(audio, "audioDup") {
            push(
                &mut entries,
                "audio.audioDup",
                "--audio-dup".to_string(),
                true,
            );
        }
        push_number(
            &mut entries,
            "audio.audioBitrate",
            number(audio, "audioBitrate"),
            |n| format!("--audio-bit-rate={n}K"),
            |n| n > 0.0,
            errors,
            "audio",
            "audioBitrate",
        );
        let audio_buffer = number(audio, "audioBuffer");
        if audio_buffer > 0.0
            && audio_buffer != number(group(&builtin, "audio"), "audioBuffer")
            && !bad(errors, "audio", "audioBuffer")
        {
            push(
                &mut entries,
                "audio.audioBuffer",
                format!("--audio-buffer={audio_buffer}"),
                true,
            );
        }
        let audio_out_buffer = number(audio, "audioOutBuffer");
        if audio_out_buffer > 0.0
            && audio_out_buffer != number(group(&builtin, "audio"), "audioOutBuffer")
            && !bad(errors, "audio", "audioOutBuffer")
        {
            push(
                &mut entries,
                "audio.audioOutBuffer",
                format!("--audio-output-buffer={audio_out_buffer}"),
                true,
            );
        }
    }

    if flag(control, "noControl") && !is_flex {
        push(
            &mut entries,
            "control.noControl",
            "--no-control".to_string(),
            true,
        );
    }
    let keyboard = text(control, "keyboard");
    if !keyboard.is_empty() && keyboard != "sdk" {
        push(
            &mut entries,
            "control.keyboard",
            format!("--keyboard={keyboard}"),
            true,
        );
    }
    if flag(control, "noKeyRepeat") {
        push(
            &mut entries,
            "control.noKeyRepeat",
            "--no-key-repeat".to_string(),
            true,
        );
    }
    for (key, default, flag_name) in [
        ("mouse", "sdk", "--mouse"),
        ("gamepad", "disabled", "--gamepad"),
    ] {
        let value = text(control, key);
        if !value.is_empty() && value != default {
            push(
                &mut entries,
                field("control", key),
                format!("{flag_name}={value}"),
                true,
            );
        }
    }
    if let Some(mods) = control.get("shortMod").and_then(Value::as_array) {
        let mods: Vec<String> = mods.iter().map(string).filter(|v| !v.is_empty()).collect();
        if !mods.is_empty() {
            push(
                &mut entries,
                "control.shortMod",
                format!("--shortcut-mod={}", mods.join(",")),
                true,
            );
        }
    }
    if flag(control, "showTouches") {
        push(
            &mut entries,
            "control.showTouches",
            "--show-touches".to_string(),
            true,
        );
    }
    if flag(control, "turnScreenOff") && !is_camera {
        push(
            &mut entries,
            "control.turnScreenOff",
            "--turn-screen-off".to_string(),
            true,
        );
    }
    push_number(
        &mut entries,
        "control.screenOffTimeout",
        number(control, "screenOffTimeout"),
        |n| format!("--screen-off-timeout={n}"),
        |n| n > 0.0,
        errors,
        "control",
        "screenOffTimeout",
    );
    for (key, flag_name) in [
        ("stayAwake", "--stay-awake"),
        ("keepActive", "--keep-active"),
        ("powerOffOnClose", "--power-off-on-close"),
        ("clipboardOff", "--no-clipboard-autosync"),
    ] {
        if flag(control, key) {
            push(
                &mut entries,
                field("control", key),
                flag_name.to_string(),
                true,
            );
        }
    }

    if flag(record, "recordOn") {
        let path = text(record, "recordPath");
        if !path.is_empty() {
            push(
                &mut entries,
                "record.recordPath",
                format!("--record={path}"),
                true,
            );
        }
        let format = text(record, "recordFormat");
        if !format.is_empty() {
            push(
                &mut entries,
                "record.recordFormat",
                format!("--record-format={format}"),
                true,
            );
        }
    }
    let orientation = text(record, "recordOrientation");
    if !orientation.is_empty() && orientation != "0" {
        push(
            &mut entries,
            "record.recordOrientation",
            format!("--record-orientation={orientation}"),
            true,
        );
    }
    if flag(record, "noWindow") {
        push(
            &mut entries,
            "record.noWindow",
            "--no-window".to_string(),
            true,
        );
    }
    push_number(
        &mut entries,
        "record.timeLimit",
        number(record, "timeLimit"),
        |n| format!("--time-limit={n}"),
        |n| n > 0.0,
        errors,
        "record",
        "timeLimit",
    );
    let pause = text(record, "pauseOnExit");
    if !pause.is_empty() && pause != "false" {
        push(
            &mut entries,
            "record.pauseOnExit",
            format!("--pause-on-exit={pause}"),
            true,
        );
    }

    let title = text(window, "windowTitle");
    if !title.is_empty() {
        push(
            &mut entries,
            "window.windowTitle",
            format!("--window-title={title}"),
            true,
        );
    }
    for (key, flag_name) in [
        ("borderless", "--window-borderless"),
        ("fullscreen", "--fullscreen"),
        ("alwaysOnTop", "--always-on-top"),
        ("aspectLockOff", "--no-window-aspect-ratio-lock"),
    ] {
        if flag(window, key) {
            push(
                &mut entries,
                field("window", key),
                flag_name.to_string(),
                true,
            );
        }
    }
    let bg = text(window, "bgColor");
    if !bg.is_empty() && bg != "#222222" {
        push(
            &mut entries,
            "window.bgColor",
            format!("--background-color={bg}"),
            true,
        );
    }
    push_number(
        &mut entries,
        "window.windowWidth",
        number(window, "windowWidth"),
        |n| format!("--window-width={n}"),
        |n| n > 0.0 && !is_flex,
        errors,
        "window",
        "windowWidth",
    );
    push_number(
        &mut entries,
        "window.windowHeight",
        number(window, "windowHeight"),
        |n| format!("--window-height={n}"),
        |n| n > 0.0 && !is_flex,
        errors,
        "window",
        "windowHeight",
    );
    push_number(
        &mut entries,
        "window.windowX",
        number(window, "windowX"),
        |n| format!("--window-x={n}"),
        |n| n > 0.0,
        errors,
        "window",
        "windowX",
    );
    push_number(
        &mut entries,
        "window.windowY",
        number(window, "windowY"),
        |n| format!("--window-y={n}"),
        |n| n > 0.0,
        errors,
        "window",
        "windowY",
    );

    let start_app = text(apps, "startApp");
    if !start_app.is_empty() {
        push(
            &mut entries,
            "apps.startApp",
            format!("--start-app={start_app}"),
            true,
        );
    }

    let verbosity = text(advanced, "verbosity");
    if !verbosity.is_empty() && verbosity != "info" {
        push(
            &mut entries,
            "advanced.verbosity",
            format!("--verbosity={verbosity}"),
            true,
        );
    }
    if flag(advanced, "printFps") {
        push(
            &mut entries,
            "advanced.printFps",
            "--print-fps".to_string(),
            true,
        );
    }
    if flag(advanced, "cleanupOff") {
        push(
            &mut entries,
            "advanced.cleanupOff",
            "--no-cleanup".to_string(),
            true,
        );
    }
    let port = text(advanced, "port");
    if !port.is_empty() && port != "27183:27199" && !bad(errors, "advanced", "port") {
        push(
            &mut entries,
            "advanced.port",
            format!("--port={port}"),
            true,
        );
    }
    let tunnel_host = text(advanced, "tunnelHost");
    if !tunnel_host.is_empty()
        && tunnel_host != "localhost"
        && !bad(errors, "advanced", "tunnelHost")
    {
        push(
            &mut entries,
            "advanced.tunnelHost",
            format!("--tunnel-host={tunnel_host}"),
            true,
        );
    }
    let custom = text(advanced, "customArgs");
    if !custom.is_empty() && !bad(errors, "advanced", "customArgs") {
        for token in split_custom_args(&custom) {
            entries.push(ArgEntry {
                field: "advanced.customArgs",
                text: token,
            });
        }
    }

    dedupe_args(entries, device_keys)
}

fn group<'a>(params: &'a Value, name: &str) -> &'a Map<String, Value> {
    static EMPTY: OnceLock<Map<String, Value>> = OnceLock::new();
    params
        .get(name)
        .and_then(Value::as_object)
        .unwrap_or_else(|| EMPTY.get_or_init(Map::new))
}

fn field(group: &str, key: &str) -> &'static str {
    match (group, key) {
        ("control", "keyboard") => "control.keyboard",
        ("control", "mouse") => "control.mouse",
        ("control", "gamepad") => "control.gamepad",
        ("control", "stayAwake") => "control.stayAwake",
        ("control", "keepActive") => "control.keepActive",
        ("control", "powerOffOnClose") => "control.powerOffOnClose",
        ("control", "clipboardOff") => "control.clipboardOff",
        ("window", "borderless") => "window.borderless",
        ("window", "fullscreen") => "window.fullscreen",
        ("window", "alwaysOnTop") => "window.alwaysOnTop",
        ("window", "aspectLockOff") => "window.aspectLockOff",
        _ => "",
    }
}

fn text(group: &Map<String, Value>, key: &str) -> String {
    group.get(key).map(string).unwrap_or_default()
}

fn number(group: &Map<String, Value>, key: &str) -> f64 {
    group.get(key).map(number_value).unwrap_or(0.0)
}

fn flag(group: &Map<String, Value>, key: &str) -> bool {
    group.get(key) == Some(&Value::Bool(true))
}

fn bad(errors: &HashSet<String>, group: &str, key: &str) -> bool {
    errors.contains(&format!("{group}.{key}"))
}

fn push(entries: &mut Vec<ArgEntry>, field: &'static str, text: String, enabled: bool) {
    if enabled {
        entries.push(ArgEntry { field, text });
    }
}

fn push_number<F, P>(
    entries: &mut Vec<ArgEntry>,
    field: &'static str,
    value: f64,
    render: F,
    predicate: P,
    errors: &HashSet<String>,
    group: &str,
    key: &str,
) where
    F: Fn(f64) -> String,
    P: Fn(f64) -> bool,
{
    if predicate(value) && !bad(errors, group, key) {
        entries.push(ArgEntry {
            field,
            text: render(value),
        });
    }
}

fn string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::String(value) => trim_js(value).to_string(),
        Value::Number(value) => value
            .as_f64()
            .map(|value| format!("{value}"))
            .unwrap_or_else(|| value.to_string()),
        Value::Array(values) => values.iter().map(string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn number_value(value: &Value) -> f64 {
    match value {
        Value::Number(value) => value.as_f64().unwrap_or(0.0),
        _ => parse_number(&string(value)).unwrap_or(0.0),
    }
}

fn parse_number(raw: &str) -> Option<f64> {
    if raw.is_empty() {
        return Some(0.0);
    }
    let value = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16)
            .map(|value| value as f64)
            .ok()?
    } else {
        raw.parse::<f64>().ok()?
    };
    value.is_finite().then_some(value)
}

fn trim_js(raw: &str) -> &str {
    raw.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

fn split_custom_args(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut quoted = false;
    for ch in raw.chars() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            quoted = true;
            continue;
        }
        if ch.is_whitespace() || ch == '\u{feff}' {
            if !current.is_empty() || quoted {
                out.push(std::mem::take(&mut current));
                quoted = false;
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() || quoted {
        out.push(current);
    }
    out
}

fn flag_of(text: &str) -> Option<String> {
    let head = trim_js(text).split_whitespace().next()?;
    let bytes = head.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'-' {
        return None;
    }
    let name_start = if bytes.len() > 2 && bytes[1] == b'-' {
        2
    } else {
        1
    };
    if bytes.len() <= name_start || !bytes[name_start].is_ascii_alphabetic() {
        return None;
    }
    let name = head.split('=').next()?;
    Some(match name {
        "-b" => "--video-bit-rate".to_string(),
        "-m" => "--max-size".to_string(),
        _ => name.to_string(),
    })
}

fn dedupe_args(entries: Vec<ArgEntry>, device_keys: &HashSet<String>) -> Vec<String> {
    if device_keys.is_empty() {
        return entries.into_iter().map(|entry| entry.text).collect();
    }
    let mut device_flags = HashSet::new();
    for entry in &entries {
        if device_keys.contains(entry.field) {
            if let Some(flag) = flag_of(&entry.text) {
                device_flags.insert(flag);
            }
        }
    }
    let mut kept = Vec::new();
    let mut index = 0;
    while index < entries.len() {
        let entry = &entries[index];
        if device_keys.contains(entry.field) {
            kept.push(entry.text.clone());
            index += 1;
            continue;
        }
        let flag = flag_of(&entry.text);
        if flag.is_none() || !device_flags.contains(flag.as_ref().unwrap()) {
            kept.push(entry.text.clone());
            index += 1;
            continue;
        }
        if entry.field == "advanced.customArgs" {
            let next = entries.get(index + 1);
            if next.is_some_and(|next| next.field == entry.field && flag_of(&next.text).is_none()) {
                index += 1;
            }
        }
        index += 1;
    }
    kept
}

pub fn validate_params(params: &Value) -> HashSet<String> {
    let mut errors = HashSet::new();
    for (group, key, min, max) in [
        ("source", "displayId", 0.0, 9999.0),
        ("source", "newDisplayDpi", 72.0, 1000.0),
        ("source", "cameraFps", 1.0, 240.0),
        ("source", "cameraZoom", 0.1, 10.0),
        ("video", "maxSize", 0.0, 2560.0),
        ("video", "bitrate", 0.0, 64.0),
        ("video", "maxFps", 0.0, 240.0),
        ("video", "videoBuffer", 0.0, 60000.0),
        ("audio", "audioBitrate", 0.0, 512.0),
        ("audio", "audioBuffer", 0.0, 10000.0),
        ("audio", "audioOutBuffer", 0.0, 10000.0),
        ("control", "screenOffTimeout", 0.0, 86400.0),
        ("record", "timeLimit", 0.0, f64::INFINITY),
        ("window", "windowWidth", 0.0, 16384.0),
        ("window", "windowHeight", 0.0, 16384.0),
        ("window", "windowX", 0.0, 32767.0),
        ("window", "windowY", 0.0, 32767.0),
    ] {
        let Some(value) = params.get(group).and_then(|v| v.get(key)) else {
            continue;
        };
        if is_blank(value) {
            continue;
        }
        let Some(value) = as_number(value) else {
            errors.insert(format!("{group}.{key}"));
            continue;
        };
        if value < min || value > max {
            errors.insert(format!("{group}.{key}"));
        }
    }

    for (group, key) in [
        ("video", "crop"),
        ("source", "newDisplayRes"),
        ("source", "cameraSize"),
        ("window", "bgColor"),
        ("advanced", "port"),
        ("advanced", "tunnelHost"),
        ("advanced", "customArgs"),
    ] {
        let Some(value) = params.get(group).and_then(|v| v.get(key)) else {
            continue;
        };
        let raw = match value {
            Value::String(value) => value.clone(),
            Value::Null => String::new(),
            _ => string(value),
        };
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let valid = match key {
            "crop" => valid_crop(raw),
            "newDisplayRes" => valid_resolution(raw, true),
            "cameraSize" => valid_resolution(raw, false),
            "bgColor" => valid_hex(raw),
            "port" => valid_port(raw),
            "tunnelHost" => !raw.chars().any(|c| c.is_whitespace() || c == '\u{feff}'),
            "customArgs" => {
                let single = raw.chars().filter(|c| *c == '\'').count();
                let double = raw.chars().filter(|c| *c == '"').count();
                single % 2 == 0 && double % 2 == 0
            }
            _ => true,
        };
        if !valid {
            errors.insert(format!("{group}.{key}"));
        }
    }

    for (group, key, options) in [
        (
            "video",
            "videoCodec",
            &["", "h264", "h265", "av1", "vp8", "vp9"][..],
        ),
        ("video", "hwdec", &["", "disabled", "vaapi", "d3d11va"][..]),
        (
            "video",
            "renderFit",
            &["letterbox", "stretched", "unscaled"][..],
        ),
        (
            "source",
            "cameraFacing",
            &["", "front", "back", "external"][..],
        ),
        (
            "audio",
            "audioSource",
            &[
                "output",
                "playback",
                "mic",
                "mic-unprocessed",
                "mic-camcorder",
                "mic-voice-communication",
                "voice-call",
                "voice-performance",
            ][..],
        ),
        ("audio", "audioCodec", &["opus", "aac", "flac", "raw"][..]),
        (
            "control",
            "keyboard",
            &["uhid", "sdk", "aoa", "disabled"][..],
        ),
        ("control", "mouse", &["uhid", "sdk", "aoa", "disabled"][..]),
        ("control", "gamepad", &["uhid", "aoa", "disabled"][..]),
        (
            "record",
            "recordFormat",
            &["mp4", "mkv", "m4a", "mka", "opus", "aac", "flac", "wav"][..],
        ),
        ("record", "pauseOnExit", &["false", "true", "if-error"][..]),
        (
            "advanced",
            "verbosity",
            &["info", "debug", "verbose", "warn", "error"][..],
        ),
    ] {
        let Some(value) = params.get(group).and_then(|v| v.get(key)) else {
            continue;
        };
        if is_blank(value) {
            continue;
        }
        let value = string(value);
        if !value.is_empty() && !options.contains(&value.as_str()) {
            errors.insert(format!("{group}.{key}"));
        }
    }
    errors
}

fn is_blank(value: &Value) -> bool {
    value.is_null()
        || value
            .as_str()
            .is_some_and(|value| trim_js(value).is_empty())
}

fn as_number(value: &Value) -> Option<f64> {
    if is_blank(value) {
        return None;
    }
    match value {
        Value::Number(value) => value.as_f64().filter(|value| value.is_finite()),
        _ => parse_number(&string(value)),
    }
}

fn all_digits(raw: &str) -> bool {
    !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit())
}

fn valid_resolution(raw: &str, allow_dpi: bool) -> bool {
    let mut parts = raw.split('/');
    let size = parts.next().unwrap_or_default();
    let dpi = parts.next();
    if parts.next().is_some() || (!allow_dpi && dpi.is_some()) {
        return false;
    }
    let mut size_parts = size.split('x');
    let width = size_parts.next().unwrap_or_default();
    let height = size_parts.next().unwrap_or_default();
    all_digits(width)
        && all_digits(height)
        && size_parts.next().is_none()
        && dpi.is_none_or(all_digits)
}

fn valid_crop(raw: &str) -> bool {
    let parts: Vec<&str> = raw.split(':').collect();
    parts.len() == 4
        && parts.iter().all(|part| all_digits(part))
        && parts[0].parse::<u64>().unwrap_or(0) > 0
        && parts[1].parse::<u64>().unwrap_or(0) > 0
}

fn valid_hex(raw: &str) -> bool {
    raw.len() == 7 && raw.starts_with('#') && raw[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_port(raw: &str) -> bool {
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() > 2 || !parts.iter().all(|part| all_digits(part)) {
        return false;
    }
    let from = parts[0].parse::<u32>().unwrap_or(0);
    let to = parts
        .get(1)
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .unwrap_or(from);
    (1..=65535).contains(&from) && (1..=65535).contains(&to) && from <= to
}

#[cfg(test)]
#[derive(serde::Deserialize)]
struct Fixture {
    version: u32,
    base: Value,
    cases: Vec<FixtureCase>,
}

#[cfg(test)]
#[derive(serde::Deserialize)]
struct FixtureCase {
    name: String,
    global: Value,
    #[serde(rename = "override")]
    overrides: Option<FixtureOverride>,
    errors: Vec<String>,
    expected: Vec<String>,
}

#[cfg(test)]
#[derive(serde::Deserialize)]
struct FixtureOverride {
    enabled: Option<bool>,
    values: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Fixture {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/local-control-argv.json"
        ))
        .unwrap()
    }

    #[test]
    fn rust_matches_shared_argv_fixture() {
        let fixture = fixture();
        assert_eq!(fixture.version, 1);
        assert_eq!(builtin_defaults(), fixture.base);
        for case in fixture.cases {
            let mut params = merge_params(&fixture.base, Some(&case.global));
            let mut device_keys = HashSet::new();
            if let Some(over) = case.overrides {
                if over.enabled != Some(false) {
                    collect_device_keys(&over.values, &mut device_keys);
                    params = merge_params(&params, Some(&over.values));
                }
            }
            let errors = case.errors.into_iter().collect();
            let actual = build_args(&params, &errors, &device_keys);
            assert_eq!(actual, case.expected, "fixture case: {}", case.name);
        }
    }

    #[test]
    fn resolve_args_uses_saved_defaults_and_enabled_override() {
        let mut config = AppConfig::fresh();
        config.defaults = serde_json::json!({
            "video": { "bitrate": 8 },
            "advanced": { "customArgs": "--max-size 720" }
        });
        config.overrides.insert(
            "device-a".to_string(),
            crate::config::OverrideEntry {
                enabled: true,
                values: serde_json::json!({ "video": { "maxSize": 1080 } }),
            },
        );
        assert_eq!(
            resolve_args(&config, "device-a"),
            vec!["-b 8M", "-m 1080", "--no-audio"]
        );
    }

    #[test]
    fn validation_rejects_ranges_formats_and_selects() {
        let mut params = builtin_defaults();
        params["video"]["bitrate"] = serde_json::json!(65);
        params["video"]["videoCodec"] = serde_json::json!("bad");
        params["window"]["bgColor"] = serde_json::json!("bad");
        params["advanced"]["customArgs"] = serde_json::json!("--foo \"unterminated");
        let errors = validate_params(&params);
        for key in [
            "video.bitrate",
            "video.videoCodec",
            "window.bgColor",
            "advanced.customArgs",
        ] {
            assert!(errors.contains(key), "missing validation error: {key}");
        }
    }
}
