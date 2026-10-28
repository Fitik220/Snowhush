# Cursor Snow (Windows)

Enable **Cursor Snow** from the tray menu. The submenu controls accumulation
amount, adhesion, shake removal and melting. The feature starts disabled so an
existing installation's appearance does not change until it is enabled. Its
five values are stored as DWORDs under `HKCU\Software\Snowhush`.

## Build locally

On Windows 10 or 11, install a current Rust MSVC toolchain and Visual Studio
Build Tools with the C++ workload and Windows SDK. From `windows/` run:

```powershell
cargo test --lib
cargo test --bin Snowhush
cargo build --release --bin Snowhush
$env:SNOWHUSH_APP = (Resolve-Path target/release/Snowhush.exe).Path
cargo build --release --features setup --bin Snowhush-Setup
```

The last command embeds `Snowhush.exe` in `Snowhush-Setup.exe`. Building or
testing does not run either executable and does not touch an installed copy.
The app has a single-instance mutex, so a separately running installed copy
must be closed before manually launching this build for visual inspection.

## Implementation

`src/cursor_snow.rs` models ten small deposition sites along the upper sloping
edge of the standard arrow. The site heights cap at 1.7, 3.2 or 5.0 DIPs.
Native `GetCursorInfo` supplies visibility, position and cursor identity;
`GetIconInfo` supplies the hotspot when the handle changes. The existing
per-monitor DPI conversion maps it into overlay coordinates. A different
cursor handle or hidden cursor clears the attached snow. Detached snow enters
the existing gravity and wind particle path.

The overlay is still the application's click-through layered window. No
system cursor is replaced and no input hooks are installed. Existing screen
coverage, screenshot and power-saving behavior also applies to Cursor Snow.

## Changed files

- `src/cursor_snow.rs`: fixed-size cursor surface, accumulation, motion and detachment.
- `src/simulation.rs`, `src/lib.rs`: feed falling flakes into the cursor model and return detached snow to the ordinary particle simulation and renderer.
- `src/screen.rs`: identify the visible system arrow and its hotspot on each display.
- `src/app.rs`, `src/snow.rs`, `src/prefs.rs`: tray controls, live commands and registry persistence.
- `CURSOR_SNOW.md`: local build steps, design and limitations.

## Performance and limits

The model uses a fixed ten-site array, at most ten attached sprites and at
most ten detachment events in a simulation step. The existing maximum of
12,000 loose grains remains in force. Cursor icon information is queried only
when its handle changes; the arrow handle is loaded once per display. No
new per-frame heap allocation is required by the cursor model.

An optimized local microbenchmark of two million model steps with all ten
sites occupied and a slowly moving cursor measured **39.2 ns per step** on
the build machine. This measures only `CursorSnow::advance`, not cursor API
calls, full simulation, Direct3D rendering or power use; those need an
interactive profiling run on target displays.

The standard arrow outline is a visual approximation, not a bitmap collision
mask. Only the standard system arrow handle is supported initially; custom,
animated and other cursor shapes are ignored. Cursor scaling provided by
accessibility themes may not perfectly match the approximated edge. A snow
overlay is composited separately from the hardware cursor, so exact visual
alignment depends on the Windows compositor and display latency. Fullscreen
apps hide the overlay as before. Attached snow clears when the cursor crosses
to another monitor instead of being transferred between per-monitor
simulations.
