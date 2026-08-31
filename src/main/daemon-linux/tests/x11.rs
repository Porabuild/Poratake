#![cfg(target_os = "linux")]

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine as _;
use xcb::{Xid as _, x};

fn atom(connection: &xcb::Connection, name: &str) -> x::Atom {
    connection
        .wait_for_reply(connection.send_request(&x::InternAtom {
            only_if_exists: false,
            name: name.as_bytes(),
        }))
        .expect("intern X11 atom")
        .atom()
}

fn keycode(connection: &xcb::Connection, keysym: x::Keysym) -> x::Keycode {
    let setup = connection.get_setup();
    let first = setup.min_keycode();
    let count = setup.max_keycode() - first + 1;
    let mapping = connection
        .wait_for_reply(connection.send_request(&x::GetKeyboardMapping {
            first_keycode: first,
            count,
        }))
        .expect("keyboard mapping");
    let per_keycode = usize::from(mapping.keysyms_per_keycode());
    let offset = mapping
        .keysyms()
        .chunks(per_keycode)
        .position(|values| values.contains(&keysym))
        .expect("keysym keycode");
    first + u8::try_from(offset).expect("keycode offset")
}

fn press_key(connection: &xcb::Connection, root: x::Window, keycode: x::Keycode) {
    for event_type in [2, 3] {
        connection.send_request(&xcb::xtest::FakeInput {
            r#type: event_type,
            detail: keycode,
            time: 0,
            root,
            root_x: 0,
            root_y: 0,
            deviceid: 0,
        });
    }
    connection.flush().expect("flush fake key");
}

fn click_at(connection: &xcb::Connection, root: x::Window, x: i16, y: i16) {
    connection.send_request(&xcb::xtest::FakeInput {
        r#type: 6,
        detail: 0,
        time: 0,
        root,
        root_x: x,
        root_y: y,
        deviceid: 0,
    });
    for event_type in [4, 5] {
        connection.send_request(&xcb::xtest::FakeInput {
            r#type: event_type,
            detail: 1,
            time: 0,
            root,
            root_x: x,
            root_y: y,
            deviceid: 0,
        });
    }
    connection.flush().expect("flush fake click");
}

fn find_named_window(
    connection: &xcb::Connection,
    root: x::Window,
    name: &[u8],
) -> Option<x::Window> {
    let tree = connection
        .wait_for_reply(connection.send_request(&x::QueryTree { window: root }))
        .expect("query X11 windows");
    tree.children().iter().copied().find(|window| {
        connection
            .wait_for_reply(connection.send_request(&x::GetProperty {
                delete: false,
                window: *window,
                property: x::ATOM_WM_NAME,
                r#type: x::ATOM_STRING,
                long_offset: 0,
                long_length: 256,
            }))
            .map(|reply| reply.value::<u8>() == name)
            .unwrap_or(false)
    })
}

fn wait_for_named_window(connection: &xcb::Connection, root: x::Window, name: &[u8]) -> x::Window {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(window) = find_named_window(connection, root, name) {
            return window;
        }
        assert!(Instant::now() < deadline, "named window did not appear");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_named_window_to_close(connection: &xcb::Connection, root: x::Window, name: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while find_named_window(connection, root, name).is_some() {
        assert!(Instant::now() < deadline, "named window did not close");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn send(stdin: &mut std::process::ChildStdin, value: serde_json::Value) {
    writeln!(stdin, "{value}").expect("write daemon request");
    stdin.flush().expect("flush daemon request");
}

fn read(reader: &mut BufReader<std::process::ChildStdout>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read daemon response");
    serde_json::from_str(&line).expect("parse daemon response")
}

fn named_window(connection: &xcb::Connection, root: x::Window, name: &[u8]) -> x::Window {
    find_named_window(connection, root, name).expect("named X11 window")
}

#[test]
#[ignore = "requires an X11 display"]
fn daemon_lists_and_captures_an_x11_window() {
    let (connection, screen_number) =
        xcb::Connection::connect_with_extensions(None, &[xcb::Extension::Test], &[])
            .expect("connect to X11");
    let screen = connection
        .get_setup()
        .roots()
        .nth(screen_number as usize)
        .expect("X11 screen");
    let wallpaper: x::Pixmap = connection.generate_id();
    connection.send_request(&x::CreatePixmap {
        depth: screen.root_depth(),
        pid: wallpaper,
        drawable: x::Drawable::Window(screen.root()),
        width: screen.width_in_pixels(),
        height: screen.height_in_pixels(),
    });
    let wallpaper_gc: x::Gcontext = connection.generate_id();
    connection.send_request(&x::CreateGc {
        cid: wallpaper_gc,
        drawable: x::Drawable::Pixmap(wallpaper),
        value_list: &[x::Gc::Foreground(0x0012_3456)],
    });
    connection.send_request(&x::PolyFillRectangle {
        drawable: x::Drawable::Pixmap(wallpaper),
        gc: wallpaper_gc,
        rectangles: &[x::Rectangle {
            x: 0,
            y: 0,
            width: screen.width_in_pixels(),
            height: screen.height_in_pixels(),
        }],
    });
    let stale_wallpaper: x::Pixmap = connection.generate_id();
    connection.send_request(&x::CreatePixmap {
        depth: screen.root_depth(),
        pid: stale_wallpaper,
        drawable: x::Drawable::Window(screen.root()),
        width: 1,
        height: 1,
    });
    connection.send_request(&x::FreePixmap {
        pixmap: stale_wallpaper,
    });
    connection.send_request(&x::ChangeProperty {
        mode: x::PropMode::Replace,
        window: screen.root(),
        property: atom(&connection, "_XROOTPMAP_ID"),
        r#type: x::ATOM_PIXMAP,
        data: &[stale_wallpaper.resource_id()],
    });
    connection.send_request(&x::ChangeProperty {
        mode: x::PropMode::Replace,
        window: screen.root(),
        property: atom(&connection, "ESETROOT_PMAP_ID"),
        r#type: x::ATOM_PIXMAP,
        data: &[wallpaper.resource_id()],
    });
    let window: x::Window = connection.generate_id();
    connection.send_request(&x::CreateWindow {
        depth: x::COPY_FROM_PARENT as u8,
        wid: window,
        parent: screen.root(),
        x: -40,
        y: -30,
        width: 320,
        height: 180,
        border_width: 0,
        class: x::WindowClass::InputOutput,
        visual: screen.root_visual(),
        value_list: &[x::Cw::BackPixel(0x00ff_0000)],
    });
    connection.send_request(&x::ChangeProperty {
        mode: x::PropMode::Replace,
        window,
        property: x::ATOM_WM_NAME,
        r#type: x::ATOM_STRING,
        data: b"PoratakeWindowTest",
    });
    connection.send_request(&x::ChangeProperty {
        mode: x::PropMode::Replace,
        window,
        property: x::ATOM_WM_CLASS,
        r#type: x::ATOM_STRING,
        data: b"poratake-test\0PoratakeTest\0",
    });
    connection.send_request(&x::MapWindow { window });
    let covering_window: x::Window = connection.generate_id();
    connection.send_request(&x::CreateWindow {
        depth: x::COPY_FROM_PARENT as u8,
        wid: covering_window,
        parent: screen.root(),
        x: 0,
        y: 0,
        width: 700,
        height: 300,
        border_width: 0,
        class: x::WindowClass::InputOutput,
        visual: screen.root_visual(),
        value_list: &[x::Cw::BackPixel(0x0000_00ff)],
    });
    connection.send_request(&x::MapWindow {
        window: covering_window,
    });
    connection.send_request(&x::SetInputFocus {
        revert_to: x::InputFocus::Parent,
        focus: covering_window,
        time: x::CURRENT_TIME,
    });
    connection.flush().expect("flush X11 window");
    connection
        .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
        .expect("synchronize X11 window");

    let capture_path = std::env::temp_dir().join(format!(
        "poratake-x11-window-test-{}.png",
        std::process::id()
    ));
    let frozen_live_path = std::env::temp_dir().join(format!(
        "poratake-x11-frozen-live-test-{}.png",
        std::process::id()
    ));
    let frozen_cached_path = std::env::temp_dir().join(format!(
        "poratake-x11-frozen-cached-test-{}.png",
        std::process::id()
    ));
    let unfrozen_live_path = std::env::temp_dir().join(format!(
        "poratake-x11-unfrozen-live-test-{}.png",
        std::process::id()
    ));
    let released_live_path = std::env::temp_dir().join(format!(
        "poratake-x11-released-live-test-{}.png",
        std::process::id()
    ));
    let scroll_path = std::env::temp_dir().join(format!(
        "poratake-x11-scroll-test-{}.png",
        std::process::id()
    ));
    let mut child = Command::new(env!("CARGO_BIN_EXE_poratake-daemon-linux"))
        .args(["--session", "x11"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn Linux daemon");
    let mut stdin = child.stdin.take().expect("daemon stdin");
    let mut reader = BufReader::new(child.stdout.take().expect("daemon stdout"));
    assert_eq!(read(&mut reader)["data"]["backend"], "x11");

    send(
        &mut stdin,
        serde_json::json!({
            "id": "wallpaper",
            "module": "desktop-wallpaper",
            "method": "get"
        }),
    );
    let wallpaper_result = read(&mut reader);
    assert_eq!(wallpaper_result["result"]["type"], "data");
    let wallpaper_data = wallpaper_result["result"]["value"]
        .as_str()
        .expect("wallpaper data URL")
        .strip_prefix("data:image/png;base64,")
        .expect("PNG wallpaper data URL");
    let wallpaper_image = image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(wallpaper_data)
            .expect("decode wallpaper PNG"),
    )
    .expect("wallpaper PNG")
    .to_rgba8();
    assert_eq!(
        (wallpaper_image.width(), wallpaper_image.height()),
        (
            u32::from(screen.width_in_pixels()),
            u32::from(screen.height_in_pixels())
        )
    );
    assert_eq!(wallpaper_image.get_pixel(0, 0).0, [0x12, 0x34, 0x56, 255]);

    send(
        &mut stdin,
        serde_json::json!({
            "id": "scroll-auto-without-session",
            "module": "scroll-capture",
            "method": "startAutoScroll"
        }),
    );
    let not_capturing = read(&mut reader);
    assert_eq!(not_capturing["error"]["code"], "NOT_CAPTURING");
    let ui_scale = std::env::var("GDK_SCALE")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(1);
    let capture_x = 10 * ui_scale;
    let capture_y = 10 * ui_scale;
    let capture_width = 320 * ui_scale;
    let capture_height = 80 * ui_scale;
    let conflicting_enter = keycode(&connection, 0xff0d);
    let conflict = connection.send_request_checked(&x::GrabKey {
        owner_events: false,
        grab_window: screen.root(),
        modifiers: x::ModMask::empty(),
        key: conflicting_enter,
        pointer_mode: x::GrabMode::Async,
        keyboard_mode: x::GrabMode::Async,
    });
    connection
        .check_request(conflict)
        .expect("register conflicting Enter");
    send(
        &mut stdin,
        serde_json::json!({
            "id": "scroll-shortcut-conflict",
            "module": "scroll-capture",
            "method": "start",
            "params": {
                "x": capture_x,
                "y": capture_y,
                "width": capture_width,
                "height": capture_height,
                "scaleFactor": ui_scale,
                "nativeControls": true
            }
        }),
    );
    assert_eq!(read(&mut reader)["error"]["code"], "UI_ERROR");
    assert!(
        find_named_window(
            &connection,
            screen.root(),
            b"Poratake Scroll Capture Boundary"
        )
        .is_none()
    );
    let release_conflict = connection.send_request_checked(&x::UngrabKey {
        key: conflicting_enter,
        grab_window: screen.root(),
        modifiers: x::ModMask::empty(),
    });
    connection
        .check_request(release_conflict)
        .expect("unregister conflicting Enter");
    connection.flush().expect("flush released Enter");

    send(
        &mut stdin,
        serde_json::json!({
            "id": "scroll-key-cancel",
            "module": "scroll-capture",
            "method": "start",
            "params": {
                "x": capture_x,
                "y": capture_y,
                "width": capture_width,
                "height": capture_height,
                "scaleFactor": ui_scale,
                "nativeControls": true
            }
        }),
    );
    assert_eq!(read(&mut reader)["result"]["started"], true);
    wait_for_named_window(
        &connection,
        screen.root(),
        b"Poratake Scroll Capture Controls",
    );
    assert_eq!(
        connection
            .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
            .expect("focus after scroll controls")
            .focus(),
        covering_window
    );
    press_key(&connection, screen.root(), keycode(&connection, 0xff1b));
    assert_eq!(read(&mut reader)["event"], "scroll-capture:cancelled");
    wait_for_named_window_to_close(
        &connection,
        screen.root(),
        b"Poratake Scroll Capture Controls",
    );
    connection.send_request(&x::SetInputFocus {
        revert_to: x::InputFocus::Parent,
        focus: covering_window,
        time: x::CURRENT_TIME,
    });
    connection.flush().expect("restore test window focus");

    send(
        &mut stdin,
        serde_json::json!({
            "id": "timer-cancel",
            "module": "timer-control",
            "method": "show",
            "params": {
                "x": 20,
                "y": 20,
                "duration": 30,
                "color": "#8892ef",
                "foregroundColor": "#ffffff"
            }
        }),
    );
    assert_eq!(read(&mut reader)["result"]["visible"], true);
    assert_eq!(
        connection
            .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
            .expect("timer focus")
            .focus(),
        covering_window
    );
    press_key(&connection, screen.root(), keycode(&connection, 0xff1b));
    assert_eq!(read(&mut reader)["event"], "timer-control:cancel");

    send(
        &mut stdin,
        serde_json::json!({
            "id": "timer-complete",
            "module": "timer-control",
            "method": "show",
            "params": {
                "x": 20,
                "y": 20,
                "duration": 1,
                "color": "#8892ef",
                "foregroundColor": "#ffffff"
            }
        }),
    );
    assert_eq!(read(&mut reader)["result"]["visible"], true);
    assert_eq!(read(&mut reader)["event"], "timer-control:completed");

    send(
        &mut stdin,
        serde_json::json!({
            "id": "freeze",
            "module": "freeze-screen",
            "method": "freeze"
        }),
    );
    assert_eq!(read(&mut reader)["result"]["frozen"], true);
    let freeze_window = named_window(&connection, screen.root(), b"Poratake Freeze Screen");
    let freeze_geometry = connection
        .wait_for_reply(connection.send_request(&x::GetGeometry {
            drawable: x::Drawable::Window(freeze_window),
        }))
        .expect("freeze geometry");
    assert_eq!((freeze_geometry.x(), freeze_geometry.y()), (0, 0));
    assert_eq!(
        (freeze_geometry.width(), freeze_geometry.height()),
        (screen.width_in_pixels(), screen.height_in_pixels())
    );
    connection.send_request(&x::ChangeWindowAttributes {
        window: covering_window,
        value_list: &[x::Cw::BackPixel(0x0000_ff00)],
    });
    connection.send_request(&x::ClearArea {
        exposures: false,
        window: covering_window,
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    });
    connection.flush().expect("paint changed live desktop");
    connection
        .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
        .expect("synchronize changed live desktop");

    send(
        &mut stdin,
        serde_json::json!({
            "id": "capture-frozen-live",
            "module": "screenshot",
            "method": "capture-area",
            "params": {
                "x": 0,
                "y": 0,
                "width": 20,
                "height": 20,
                "path": frozen_live_path,
                "cached": false,
                "displayOriginX": 0,
                "displayOriginY": 0
            }
        }),
    );
    assert_eq!(read(&mut reader)["success"], true);
    assert_eq!(
        image::open(&frozen_live_path)
            .expect("live frozen image")
            .to_rgba8()
            .get_pixel(10, 10)
            .0,
        [0, 0, 255, 255]
    );

    connection.send_request(&x::UnmapWindow {
        window: freeze_window,
    });
    connection.flush().expect("unmap freeze overlay");
    connection
        .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
        .expect("synchronize unmapped freeze overlay");
    send(
        &mut stdin,
        serde_json::json!({
            "id": "capture-unfrozen-live",
            "module": "screenshot",
            "method": "capture-area",
            "params": {
                "x": 0,
                "y": 0,
                "width": 20,
                "height": 20,
                "path": unfrozen_live_path,
                "cached": false,
                "displayOriginX": 0,
                "displayOriginY": 0
            }
        }),
    );
    assert_eq!(read(&mut reader)["success"], true);
    assert_eq!(
        image::open(&unfrozen_live_path)
            .expect("unfrozen live image")
            .to_rgba8()
            .get_pixel(10, 10)
            .0,
        [0, 255, 0, 255]
    );

    send(
        &mut stdin,
        serde_json::json!({
            "id": "capture-frozen-cached",
            "module": "screenshot",
            "method": "capture-area",
            "params": {
                "x": 0,
                "y": 0,
                "width": 20,
                "height": 20,
                "path": frozen_cached_path,
                "cached": true,
                "displayOriginX": 0,
                "displayOriginY": 0
            }
        }),
    );
    assert_eq!(read(&mut reader)["success"], true);
    assert_eq!(
        image::open(&frozen_cached_path)
            .expect("cached frozen image")
            .to_rgba8()
            .get_pixel(10, 10)
            .0,
        [0, 0, 255, 255]
    );

    send(
        &mut stdin,
        serde_json::json!({
            "id": "release-freeze",
            "module": "freeze-screen",
            "method": "release"
        }),
    );
    assert_eq!(read(&mut reader)["result"]["frozen"], false);
    send(
        &mut stdin,
        serde_json::json!({
            "id": "capture-released-live",
            "module": "screenshot",
            "method": "capture-area",
            "params": {
                "x": 0,
                "y": 0,
                "width": 20,
                "height": 20,
                "path": released_live_path,
                "cached": false,
                "displayOriginX": 0,
                "displayOriginY": 0
            }
        }),
    );
    assert_eq!(read(&mut reader)["success"], true);
    assert_eq!(
        image::open(&released_live_path)
            .expect("released live image")
            .to_rgba8()
            .get_pixel(10, 10)
            .0,
        [0, 255, 0, 255]
    );

    send(
        &mut stdin,
        serde_json::json!({
            "id": "list",
            "module": "window-selector",
            "method": "list"
        }),
    );
    let list = read(&mut reader);
    let listed = list["result"]["windows"]
        .as_array()
        .expect("window list")
        .iter()
        .find(|item| item["windowId"] == i64::from(window.resource_id()))
        .expect("test window listed");
    assert_eq!(listed["title"], "PoratakeWindowTest");
    assert_eq!(listed["ownerName"], "PoratakeTest");
    assert_eq!(listed["bounds"]["x"], -40.0);
    assert_eq!(listed["bounds"]["y"], -30.0);
    assert_eq!(listed["bounds"]["width"], 320.0);

    send(
        &mut stdin,
        serde_json::json!({
            "id": "capture",
            "module": "screenshot",
            "method": "capture-window",
            "params": {
                "windowId": window.resource_id(),
                "path": capture_path
            }
        }),
    );
    let captured = read(&mut reader);
    assert_eq!(captured["success"], true);
    let image = image::open(&capture_path).expect("captured window image");
    assert_eq!((image.width(), image.height()), (320, 180));
    let image = image.to_rgba8();
    assert_eq!(image.get_pixel(50, 50).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(10, 10).0, [255, 0, 0, 255]);

    send(
        &mut stdin,
        serde_json::json!({
            "id": "scroll-start",
            "module": "scroll-capture",
            "method": "start",
            "params": {
                "x": capture_x,
                "y": capture_y,
                "width": capture_width,
                "height": capture_height,
                "scaleFactor": ui_scale,
                "autoScrollSpeed": "fast",
                "maxHeight": 500,
                "nativeControls": true
            }
        }),
    );
    assert_eq!(read(&mut reader)["result"]["started"], true);
    let panel = wait_for_named_window(
        &connection,
        screen.root(),
        b"Poratake Scroll Capture Controls",
    );
    let panel_geometry = connection
        .wait_for_reply(connection.send_request(&x::GetGeometry {
            drawable: x::Drawable::Window(panel),
        }))
        .expect("scroll panel geometry");
    assert_eq!(
        (
            i32::from(panel_geometry.x()),
            i32::from(panel_geometry.y()),
            u32::from(panel_geometry.width()),
            u32::from(panel_geometry.height())
        ),
        (
            30 * ui_scale,
            102 * ui_scale,
            280 * ui_scale as u32,
            52 * ui_scale as u32
        )
    );
    assert_eq!(
        connection
            .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
            .expect("focus during scroll capture")
            .focus(),
        covering_window
    );
    click_at(
        &connection,
        screen.root(),
        i16::try_from((30 + 44) * ui_scale).expect("auto button x"),
        i16::try_from((102 + 26) * ui_scale).expect("auto button y"),
    );
    let ended = read(&mut reader);
    assert_eq!(ended["event"], "scroll-capture:scroll-ended");
    assert_eq!(ended["data"]["reason"], "duplicate");
    assert_eq!(
        connection
            .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
            .expect("focus after scroll button")
            .focus(),
        covering_window
    );
    press_key(&connection, screen.root(), keycode(&connection, 0xff0d));
    let done = read(&mut reader);
    assert_eq!(done["event"], "scroll-capture:done");
    send(
        &mut stdin,
        serde_json::json!({
            "id": "scroll-finish",
            "module": "scroll-capture",
            "method": "finish",
            "params": { "outputPath": scroll_path }
        }),
    );
    let stitched = read(&mut reader);
    assert_eq!(stitched["result"]["success"], true);
    assert_eq!(stitched["result"]["frameCount"], 1);
    let stitched_image = image::open(&scroll_path).expect("stitched scroll image");
    assert_eq!(
        (stitched_image.width(), stitched_image.height()),
        (capture_width as u32, capture_height as u32)
    );
    assert_eq!(
        stitched_image
            .to_rgba8()
            .get_pixel(capture_width as u32 / 2, capture_height as u32 / 2)
            .0,
        [0, 255, 0, 255]
    );
    send(
        &mut stdin,
        serde_json::json!({
            "id": "print",
            "module": "print",
            "method": "image",
            "params": {
                "imageBase64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
            }
        }),
    );
    assert_eq!(read(&mut reader)["result"]["success"], true);
    wait_for_named_window(&connection, screen.root(), b"Print");
    send(
        &mut stdin,
        serde_json::json!({"id": "after-print", "module": "system", "method": "ping"}),
    );
    assert_eq!(read(&mut reader)["result"]["pong"], true);

    send(
        &mut stdin,
        serde_json::json!({"id": "quit", "module": "system", "method": "quit"}),
    );
    assert_eq!(read(&mut reader)["success"], true);
    drop(stdin);
    assert!(child.wait().expect("wait for daemon").success());
    connection.send_request(&x::FreeGc { gc: wallpaper_gc });
    connection.send_request(&x::FreePixmap { pixmap: wallpaper });
    for path in [
        capture_path,
        frozen_live_path,
        frozen_cached_path,
        unfrozen_live_path,
        released_live_path,
        scroll_path,
    ] {
        std::fs::remove_file(path).expect("remove capture");
    }
}
