use notify_rust::{Notification, Timeout};
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command;
use x11rb::protocol::xproto::KeyButMask;

use anyhow::{Context, Result};
use x11rb::{atom_manager, connection::Connection};

use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, GetPropertyReply, Window};
use x11rb::rust_connection::RustConnection;

use x11rb::{
    connection::Connection as _,
    protocol::xproto::{ConnectionExt as _, KeyPressEvent, KEY_PRESS_EVENT, KEY_RELEASE_EVENT},
    wrapper::ConnectionExt as _,
};

// KeyPressEvent and KeyReleaseEvent are the same type, but have different response_type
type KeyEvent = KeyPressEvent;

fn get_or_intern_atom(conn: &RustConnection, name: &[u8]) -> Atom {
    let result = conn
        .intern_atom(false, name)
        .expect("Failed to intern atom")
        .reply()
        .expect("Failed receive interned atom");

    result.atom
}

fn find_active_window(
    conn: &impl Connection,
    root: Window,
    net_active_window: Atom,
) -> Option<Window> {
    let window: Atom = AtomEnum::WINDOW.into();
    let active_window = conn
        .get_property(false, root, net_active_window, window, 0, 1)
        .expect("Failed to get X11 property")
        .reply()
        .expect("Failed to receive X11 property reply");

    if active_window.format == 32 && active_window.length == 1 {
        active_window
            .value32()
            .expect("Invalid message. Expected value with format = 32")
            .next()
    } else {
        // Query the input focus
        Some(
            conn.get_input_focus()
                .expect("Failed to get input focus")
                .reply()
                .expect("Failed to receive X11 input focus")
                .focus,
        )
    }
}

fn parse_string_property(property: &GetPropertyReply) -> &str {
    std::str::from_utf8(&property.value).unwrap_or("Invalid utf8")
}

fn parse_wm_class(property: &GetPropertyReply) -> (&str, &str) {
    if property.format != 8 {
        return (
            "Malformed property: wrong format",
            "Malformed property: wrong format",
        );
    }
    let value = &property.value;
    // The property should contain two null-terminated strings. Find them.
    if let Some(middle) = value.iter().position(|&b| b == 0) {
        let (instance, class) = value.split_at(middle);
        // Skip the null byte at the beginning
        let mut class = &class[1..];
        // Remove the last null byte from the class, if it is there.
        if class.last() == Some(&0) {
            class = &class[..class.len() - 1];
        }
        let instance = std::str::from_utf8(instance);
        let class = std::str::from_utf8(class);
        (
            instance.unwrap_or("Invalid utf8"),
            class.unwrap_or("Invalid utf8"),
        )
    } else {
        ("Missing null byte", "Missing null byte")
    }
}

struct ActiveWindowLookupper {
    conn: RustConnection,
    root: u32,
    net_active_window: Atom,
    net_wm_name: Atom,
    utf8_string: Atom,
}

impl ActiveWindowLookupper {
    fn press_key(&self, modifiers: KeyButMask, keycode: u8) -> Result<()> {
        let focus = self.conn.get_input_focus()?.reply()?.focus;
        // println!("Input focus is {focus:x}");
        let send_event = |response_type| {
            let event = KeyEvent {
                sequence: 0,
                response_type,
                time: x11rb::CURRENT_TIME,
                root: self.root,
                event: focus,
                child: x11rb::NONE,
                // No modifiers pressed
                state: modifiers,
                // They key that is being toggled
                detail: keycode,
                // All of the following are technically wrong?!
                same_screen: false,
                root_x: 0,
                root_y: 0,
                event_x: 0,
                event_y: 0,
            };
            self.conn.send_event(true, focus, Default::default(), event)
        };
        send_event(KEY_PRESS_EVENT)?;
        send_event(KEY_RELEASE_EVENT)?;
        // self.conn.sync()?;

        Ok(())
    }

    fn lookup(&self) -> Result<(String, String, String)> {
        let focus = match find_active_window(&self.conn, self.root, self.net_active_window) {
            None => {
                anyhow::bail!("No active window selected")
            }
            Some(x) => x,
        };

        // Collect the replies to the atoms
        let (wm_class, string): (Atom, Atom) = (AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into());

        // Get the property from the window that we need
        let name = self
            .conn
            .get_property(
                false,
                focus,
                self.net_wm_name,
                self.utf8_string,
                0,
                u32::MAX,
            )
            .context("cound't get NET_WM_NAME property")?;
        let class = self
            .conn
            .get_property(false, focus, wm_class, string, 0, u32::MAX)
            .context("couldn't get the WM_CLASS")?;

        let name = name
            .reply()
            .map(|r| parse_string_property(&r).to_string())?;

        let class = class.reply().context("failed to get the WM_CLASS reply")?;

        // Print out the result
        // println!("Window name: {:?}", name);
        let (instance, class) = parse_wm_class(&class);
        // println!("Window instance: {:?}", instance);
        // println!("Window class: {:?}", class);

        Ok((class.to_string(), instance.to_string(), name))
    }
}

fn main() -> Result<()> {
    // Set up our state
    let (conn, screen) = x11rb::connect(None).expect("Failed to connect");
    let root = conn.setup().roots[screen].root;
    let net_active_window = get_or_intern_atom(&conn, b"_NET_ACTIVE_WINDOW");
    let net_wm_name = get_or_intern_atom(&conn, b"_NET_WM_NAME");
    let utf8_string = get_or_intern_atom(&conn, b"UTF8_STRING");

    let l = ActiveWindowLookupper {
        conn,
        root,
        net_active_window,
        net_wm_name,
        utf8_string,
    };

    let socket_path = common::socket();

    if std::fs::metadata(&socket_path).is_ok() {
        println!("A socket is already present. Deleting...");
        std::fs::remove_file(&socket_path)
            .with_context(|| format!("could not delete previous socket at {:?}", socket_path))?;
    }

    let unix_listener =
        UnixListener::bind(socket_path).context("Could not create the unix socket")?;

    // put the server logic in a loop to accept several connections
    loop {
        let (unix_stream, _socket_address) = unix_listener
            .accept()
            .context("Failed at accepting a connection on the unix listener")?;
        handle_stream(unix_stream, &l)?;
    }
    // Ok(())
}

fn handle_stream(mut unix_stream: UnixStream, l: &ActiveWindowLookupper) -> Result<()> {
    let mut message = String::new();
    unix_stream
        .read_to_string(&mut message)
        .context("Failed at reading the unix stream")?;

    let combos = match message.as_str() {
        "ctrl u" => {
            if let Ok((class, instance, window)) = l.lookup() {
                if class == "neovide" || window == "/usr/bin/nvim" || (class == "Gnome-terminal" && instance == "gnome-terminal-server" && window=="python"){
                    vec![(KeyButMask::CONTROL, 30)]
                } else {
                    vec![(KeyButMask::SHIFT, 110), (Default::default(), 22)]
                }
            } else {
                vec![(KeyButMask::CONTROL, 30)]
            }
        }
        "ctrl h" => {
            vec![(Default::default(), 22)]
        }
        "ctrl [" => {
            vec![(Default::default(), 9)]
        }
        "ctrl ;" => {
            vec![(Default::default(), 9)]
        }
        "ctrl w" => {
            if let Ok((class, instance, window)) = l.lookup() {
                if class == "neovide" || window == "/usr/bin/nvim" || (class == "Gnome-terminal" && instance == "gnome-terminal-server") {
                    vec![(KeyButMask::CONTROL, 25)]
                } else {
                    vec![(KeyButMask::CONTROL, 22)]
                }
            } else {
                vec![(KeyButMask::CONTROL, 25)]
            }
        }
        "ctrl ins" => {
            if let Ok((class, instance, window)) = l.lookup() {
                Notification::new()
                    .summary("Unikey")
                    .body(
                        format!("class is: {class}\ninstance is {instance}\nwindow is: {window}")
                            .as_str(),
                    )
                    // .icon("firefox")
                    .timeout(Timeout::Milliseconds(6000)) //milliseconds
                    .show()
                    .unwrap();
            } else {
                println!("failed to get active window");
            };
            vec![]
        }
        "super q" => {
            Command::new("ibus")
                    .arg("engine")
                    .arg("xkb:us::eng")
                    .spawn()
                    .expect("ibus command failed to start");
            vec![]
        }
        "super w" => {
            Command::new("ibus")
                    .arg("engine")
                    .arg("libpinyin")
                    .spawn()
                    .expect("ibus command failed to start");
            vec![]
        }
        "super e" => {
            Command::new("ibus")
                    .arg("engine")
                    .arg("anthy")
                    .spawn()
                    .expect("ibus command failed to start");
            vec![]
        }
        _ => {
            panic!("unexpected message")
        }
    };

    combos.iter().for_each(|(modi, code)| {
        l.press_key(*modi, *code).ok();
    });

    l.conn.sync()?;

    // println!("We received this message: {message}");

    // unix_stream
    //     .write(b"I hear you!")
    //     .context("Failed at writing onto the unix stream")?;

    Ok(())
}
