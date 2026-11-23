# Slint WinUI

Slint WinUI is a collection of WinUI-inspired controls for Slint. It was extracted from LumaTune as a standalone library. The controls are grouped by category, including basic input, collections, dialogs and flyouts, layout, navigation, scrolling, and text.

## Use the Slint files directly

For controls implemented entirely in Slint, import from `ui/lib.slint` with a path relative to your entry file, or configure your Slint host to resolve a library alias to that file. Set `SLINT_ENABLE_EXPERIMENTAL_FEATURES=1` in the compiler environment because some controls use named slots. Controls with Rust behavior or native Windows integration have additional setup in the matching examples.

## Use in a Rust app

Place this repository beside your app, or adjust the path in `Cargo.toml`. The examples use Slint 1.18.1.

```toml
[dependencies]
slint = "=1.18.1"

[build-dependencies]
slint-build = "=1.18.1"
slint-winui = { path = "../slint-winui", default-features = false }
```

Map `@winui` to the library in `build.rs`. Enable Slint's experimental features before compiling because some components use named slots.

```rust
use std::{collections::HashMap, env};

fn main() {
    // SAFETY: This runs before the build script starts any threads.
    unsafe { env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1") };

    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(HashMap::from([("winui".into(), slint_winui::library_path())]));
    slint_build::compile_with_config("ui/main.slint", config).unwrap();

    slint_winui::write_behavior_modules(env::var_os("OUT_DIR").unwrap(), &["number_box"])
        .unwrap();
}
```

Create `ui/main.slint`. Re-export the types needed by any Rust adapter from your entry file. This lets the adapter and the app use the same Slint-generated Rust types.

```slint
import { Button, NumberBox, NumberBoxMath, NumberBoxParseResult, NumberBoxValue } from "@winui";

export { NumberBoxMath, NumberBoxParseResult, NumberBoxValue }

export component MainWindow inherits Window {
    title: "Slint WinUI quick start";
    preferred-width: 360px;
    preferred-height: 180px;

    VerticalLayout {
        padding: 24px;
        spacing: 12px;

        amount := NumberBox {
            header: "Amount";
            value: { number: 24, has-value: true };
            spin-placement: inline;
        }

        Button {
            text: "Reset";
            clicked => { amount.set-value({ number: 24, has-value: true }); }
        }
    }
}
```

Install the adapter in `src/main.rs` before running the window.

```rust
use slint::ComponentHandle;

slint::include_modules!();
include!(concat!(env!("OUT_DIR"), "/slint_winui_behaviors.rs"));

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    number_box::install(ui.global::<NumberBoxMath>());
    ui.run()
}
```

For controls implemented entirely in Slint, such as `Button` and `Expander`, omit the adapter generation and registration. See the matching [Rust examples](examples/) for each control's imports, setup, and dependencies. Enable the `windows-platform` feature and add `slint-winui` under `[dependencies]` when a Rust adapter uses Windows services such as the system calendar, password reveal policy, or native rich text.

## Run the examples

From the library checkout on Windows, run an example using the file stem under [examples](examples/).

```sh
cargo run --example navigation_view
cargo run --example expander
cargo run --example number_box
cargo test --all-targets
```

The demos use bundled icons and generate their own sample media by default. Set `SLINT_WINUI_SAMPLE_MEDIA` to a folder with custom sample images or place them under `demo-assets/` to use them in the image demos. Optional SemanticZoom data can be supplied with `SLINT_WINUI_GALLERY_DATA`.
