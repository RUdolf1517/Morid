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

use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let window = MoridWindow::new()?;
    window.set_version(env!("CARGO_PKG_VERSION").into());

    let handle = window.as_weak();
    window.on_quit(move || {
        if let Some(window) = handle.upgrade() {
            let _ = window.hide();
        }
    });

    window.run()
}
