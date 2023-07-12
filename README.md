Make ctrl+w (delete chunk), ctrl+h (backspace), ctrl+u (delete up until start of line), ctrl+[ (escape) etc. work as expected in any app. With the configurability to ignore specific apps too.

This currently uses x11, tested to work in the GNOME desktop environment.

# Mechanism

There is a server binary, and a client binary. At the press of a shortcut, GNOME calls the client, which communicates with the server through a unix socket. The server then simulates the correct key presses according to the currently focused window.

In short, the shortcut is intercepted by GNOME, and translated by the server. For example, ctrl+h is literally translated to backspace keypresses. ctrl+u is translated as shift+home followed by a backspace. etc.

# Installation

- `cargo build --release`
- `sudo ln -s $(realpath target/release/server) /usr/local/bin/unikey-server`
- `sudo ln -s $(realpath target/release/client) /usr/local/bin/unikey-client`

Now create a desktop entry for the server at `$HOME/.config/autostart/unikey.desktop`
```desktop
[Desktop Entry]
Name=Unikey
Comment=the unikey server that executes key presses according to the app
Exec=unikey-server
Terminal=false
Type=Application
X-GNOME-Autostart-enabled=true
```

Now in GNOME settings, register each hotkey like this:

![gnome shortcut](gnome-custom-shortcut.png)

# Contact me

I'm meowtuwu on discord. Contact me if you have questions.




