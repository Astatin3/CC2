use evdev::{
    AbsoluteAxisCode, Device, EventSummary, EventType, InputEvent, KeyCode, SynchronizationCode,
};
use std::env;
use std::error::Error;
use std::path::Path;
use std::time::UNIX_EPOCH;

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/dev/input/event0".to_string());
    let mut device = Device::open(Path::new(&path))?;
    let id = device.input_id();

    println!(
        "Input device name: {:?}",
        device.name().unwrap_or("unknown input device")
    );
    println!(
        "Input device ID: bus 0x{:x} vendor 0x{:x} product 0x{:x} version 0x{:x}",
        id.bus_type().0,
        id.vendor(),
        id.product(),
        id.version()
    );
    println!("Reading touch events from {path}. Press Ctrl-C to stop.\n");

    loop {
        for event in device.fetch_events()? {
            if is_touch_event(&event) {
                print_event(event);
            }
        }
    }
}

fn is_touch_event(event: &InputEvent) -> bool {
    match event.destructure() {
        EventSummary::AbsoluteAxis(_, axis, _) => matches!(
            axis,
            AbsoluteAxisCode::ABS_X
                | AbsoluteAxisCode::ABS_Y
                | AbsoluteAxisCode::ABS_PRESSURE
                | AbsoluteAxisCode::ABS_MT_SLOT
                | AbsoluteAxisCode::ABS_MT_TOUCH_MAJOR
                | AbsoluteAxisCode::ABS_MT_TOUCH_MINOR
                | AbsoluteAxisCode::ABS_MT_POSITION_X
                | AbsoluteAxisCode::ABS_MT_POSITION_Y
                | AbsoluteAxisCode::ABS_MT_TRACKING_ID
                | AbsoluteAxisCode::ABS_MT_PRESSURE
        ),
        EventSummary::Key(_, key, _) => matches!(
            key,
            KeyCode::BTN_TOUCH
                | KeyCode::BTN_TOOL_FINGER
                | KeyCode::BTN_TOOL_DOUBLETAP
                | KeyCode::BTN_TOOL_TRIPLETAP
                | KeyCode::BTN_TOOL_QUADTAP
                | KeyCode::BTN_TOOL_QUINTTAP
        ),
        EventSummary::Synchronization(_, code, _) => {
            matches!(
                code,
                SynchronizationCode::SYN_REPORT | SynchronizationCode::SYN_MT_REPORT
            )
        }
        _ => false,
    }
}

fn print_event(event: InputEvent) {
    let timestamp = event
        .timestamp()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let event_type = event.event_type();
    let (type_name, code_name) = event_names(event);

    println!(
        "Event: time {}.{:06}, type {} ({}), code {} ({}), value {}",
        timestamp.as_secs(),
        timestamp.subsec_micros(),
        event_type.0,
        type_name,
        event.code(),
        code_name,
        event.value()
    );
}

fn event_names(event: InputEvent) -> (&'static str, String) {
    match event.destructure() {
        EventSummary::Synchronization(_, code, _) => ("EV_SYN", format!("{code:?}")),
        EventSummary::Key(_, code, _) => ("EV_KEY", format!("{code:?}")),
        EventSummary::AbsoluteAxis(_, code, _) => ("EV_ABS", format!("{code:?}")),
        _ => (
            event_type_name(event.event_type()),
            format!("0x{:x}", event.code()),
        ),
    }
}

fn event_type_name(event_type: EventType) -> &'static str {
    match event_type {
        EventType::SYNCHRONIZATION => "EV_SYN",
        EventType::KEY => "EV_KEY",
        EventType::RELATIVE => "EV_REL",
        EventType::ABSOLUTE => "EV_ABS",
        EventType::MISC => "EV_MSC",
        EventType::SWITCH => "EV_SW",
        EventType::LED => "EV_LED",
        EventType::SOUND => "EV_SND",
        EventType::REPEAT => "EV_REP",
        EventType::FORCEFEEDBACK => "EV_FF",
        EventType::POWER => "EV_PWR",
        EventType::FORCEFEEDBACKSTATUS => "EV_FF_STATUS",
        EventType::UINPUT => "EV_UINPUT",
        _ => "EV_UNKNOWN",
    }
}
