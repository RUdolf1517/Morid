//! `morid-gui`: the Slint binary.
//!
//! Phase 1 proves that Slint links and that the binary can be measured: one
//! label, one button, and the button closes the window.

#![forbid(unsafe_code)]

slint::slint! {
    import { Button, VerticalBox } from "std-widgets.slint";

    export component MoridWindow inherits Window {
        in property <string> version: "0.1.0";
        callback quit();

        title: "Morid";
        preferred-width: 360px;
        preferred-height: 180px;

        VerticalBox {
            alignment: center;
            spacing: 10px;

            Text {
                text: "Morid " + root.version;
                horizontal-alignment: center;
            }

            Button {
                text: "Close";
                clicked => { root.quit(); }
            }
        }
    }
}

use std::time::Duration;

use slint::{ComponentHandle, Timer, Weak};

fn main() -> Result<(), slint::PlatformError> {
    let window = MoridWindow::new()?;
    window.set_version(env!("CARGO_PKG_VERSION").into());

    let handle = window.as_weak();
    window.on_quit(move || close(handle.clone()));

    // CI proof that the window opens and closes without a crash: when
    // MORID_GUI_SMOKE_MS is set the Close button's own code path runs after that
    // many milliseconds, so the process exits on its own with a real exit code.
    // The variable is unset in a normal run, and nothing else is added.
    if let Some(milliseconds) = smoke_timeout() {
        let handle = window.as_weak();
        Timer::single_shot(Duration::from_millis(milliseconds), move || {
            close(handle.clone());
        });
    }

    window.run()
}

fn close(handle: Weak<MoridWindow>) {
    if let Some(window) = handle.upgrade() {
        let _ = window.hide();
        let _ = slint::quit_event_loop();
    }
}

fn smoke_timeout() -> Option<u64> {
    std::env::var("MORID_GUI_SMOKE_MS").ok()?.parse().ok()
}
