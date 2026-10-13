# Terminal and graphical programs

Test what a person sees by checking **facts** about it (the greeting is
shown, the selected row is highlighted, nothing is clipped, the scene is lit),
never by matching a stored screen or image. Save what was seen as an artifact
for a person or agent to look at.

## Four tiers

Prefer the lowest tier that can show the bug.

| Tier | What runs | Needs | For |
| --- | --- | --- | --- |
| 0 | State and update logic, no rendering | Nothing | Most behaviour: keys, commands, layout decisions |
| 1 | The real renderer, into an offscreen buffer | A GPU or software renderer; no display | What a frame looks like |
| 2 | A real window on a private display | A display server per test | Window, focus, IME, clipboard, DPI bugs |
| 3 | An agent playtest of the release build | A private terminal or compositor | UX, discoverability, long sessions |

Tier 0 works the same on every OS. Structure the program as
`update(state, input) -> state` and `view(state) -> description` (a ratatui
`Buffer`, an ECS world, a widget tree) and test the description. Feed input
as the framework's own events, not OS-level key injection: it is portable,
needs no permissions and cannot race a window manager.

## Terminal programs through fux

`tests/tui.rs` runs the real binary in a pane of a **private** fux server:

```rust
let Some(fux) = Fux::start(env!("CARGO_TARGET_TMPDIR"), "tui")? else {
    return Ok(()); // fux not installed; REQUIRE_FUX=1 makes this a failure
};
let pane = fux.run("app", 80, 24, BINARY, "Ready")?;   // waits for "Ready"
fux.type_line(&pane, "open notes.txt", "notes.txt —")?; // types, waits for output
let screen = fux.screen(&pane)?;
ensure(screen.contains("3 lines"), …)?;
ensure_eq(fux.wait_exit(&pane, 10)?.as_str(), "exited 0", "exit")?;
```

`Fux::words` runs any other step in fux's command language (`fux
help-agents` lists them: `run`, `send-keys` with a wait armed before the keys
go in, `wait`, `capture` with `marks`/`styles`/`json`, `get`).

Rules learned the hard way:

- **One private server per test.** `Fux::start` puts the socket and
  `XDG_RUNTIME_DIR` in a 0700 scratch directory and clears `FUX_PANE` and
  `FUX_SERVER`, so nothing can reach the person's own session. An agent once
  killed its owner's fux server; another split their pane to nine columns.
- **Wait for output only the program produces.** If the wait text is also in
  what you typed, the echo matches before the program answers. Compute a
  marker the input cannot contain (fux's random walks wait for
  `act-done-$((N+1000))` after typing the expression).
- **Never sleep.** Every step waits on screen text, idleness, exit or a bell,
  with a timeout that reports what the pane was doing.
- **Check facts, not screens.** Use `capture` with `marks` or `styles` to ask
  "is the selected row `delta`?" rather than comparing all 24 lines.
- **Skips must not pass silently.** Set `REQUIRE_FUX=1` wherever fux is
  installed; `FUX_BIN` points at a specific build.
- Test odd sizes (40×12, 132×43) and resizes; that is where layout breaks.

tmux works the same way (`tmux -L private-socket`, `send-keys`,
`capture-pane -p`), but has no waits: poll `capture-pane` with
`testkit::sandbox::wait_until`.

## Graphical programs

### Tier 1: offscreen rendering (preferred for pixels)

Render into a texture the program owns, read it back, and check rules with
`testkit::frame::Frame`. No window, no display server, no OS permissions.

- **Bevy:** `DefaultPlugins` with `primary_window: None`, the windowing plugin
  disabled and `ScheduleRunnerPlugin` driving frames; render to an `Image`
  render target and read it back (bevy_terminal's `gpu_readback.rs`, ratty's
  `headless_snapshot.rs`). Do not use a hidden window: "a merely invisible
  window still shows on compositors that map any surface presented to".
  Without rendering at all, `MinimalPlugins` and a fixed timestep give
  deterministic simulation.
- **wgpu directly:** request an adapter with `compatible_surface: None`.
- **No GPU:** a software renderer.
  - Linux: Mesa lavapipe (Vulkan) or llvmpipe (GL). Pin the Vulkan driver file
    (`VK_DRIVER_FILES`) to lavapipe so the host GPU does not change results.
  - Windows: WARP, built into D3D12, present on CI runners.
  - macOS: CI runners have a Metal device; use it.
  - SwiftShader works everywhere at the cost of shipping it.
- **2D or text UIs:** a CPU renderer (tiny-skia, or rendering a ratatui
  buffer to HTML or an image) removes the GPU question.

Wait for the program's state to say a frame is ready (assets loaded, N frames
after the last change), never for wall time; the first readbacks in
bevy_terminal and the first xterm captures were blank. Treat shader,
pipeline or validation errors in the log as failures: hyper-surf's
`render_smoke` once "passed" with a shader that failed to compile.

Rules that survived in practice (bevy_terminal, hyper-surf):

```rust
let stats = frame.stats();
ensure(stats.dark_per_mille() < 950, "the scene is black")?;
ensure(stats.bright_per_mille() < 300, "the scene is blown out")?;
let glyph_row = frame.region(0, 40, 200, 20).ok_or_else(|| fail("frame too small"))?;
ensure(glyph_row.stats().dark > 0, "no ink where text should be")?;
let path = artifacts.save("frame.ppm", &frame.to_ppm())?;
```

Others: no ink in the cells next to a glyph (nothing clipped); no seam
between tiles; the share of changed pixels between two frames of a still
scene is near zero. A single-frame check cannot catch flicker; for that,
compare statistics across a sequence of frames.

### Tier 2: a real window on a private display

Only for bugs in the windowing layer itself, or for third-party programs that
insist on a display.

| OS | Private display | Capture | Input |
| --- | --- | --- | --- |
| Linux, X11 | `Xvfb :N` or `xvfb-run -a` | `xwd`, `import -window root` | `xdotool`, XTEST |
| Linux, Wayland | `sway` or `cage` with `WLR_BACKENDS=headless` (and `WLR_RENDERER=pixman` without a GPU), or `weston --backend=headless`, each with its own 0700 `XDG_RUNTIME_DIR` | `grim` | `wtype` |
| macOS | No virtual display: a dedicated logged-in session (CI runners have one) | `screencapture -l WINDOW_ID`, ScreenCaptureKit (Screen Recording permission) | `CGEventPost` (Accessibility permission) |
| Windows | The CI runner's desktop session | `PrintWindow` with `PW_RENDERFULLCONTENT`, Windows.Graphics.Capture | `SendInput` (unlocked session, foreground window) |

Never open windows on the person's own desktop, and never borrow their
compositor (a headless output on their Hyprland shares their workspaces and
pointer). Xvfb could not give Bevy a working present mode here; render such
programs offscreen instead.

### Tier 3: playtests

An agent uses the release build in a private terminal or compositor for a
while, at realistic sizes, and writes a dated report. See
[agent-evals.md](agent-evals.md#playtests) and `prompts/playtest.md`.

## Artifacts

`Artifacts::new(env!("CARGO_TARGET_TMPDIR"), "suite")?.save(name, bytes)?`
writes to `target/tmp/artifacts/suite/` and returns the absolute path; put it
in the failure message. PPM needs no image crate and opens everywhere. For
pull requests, generate before/after images fresh; never commit them as
expected output.
