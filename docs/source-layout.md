# Source layout

`src/main.rs` is the application entry point. Everything else is grouped by responsibility:

| Directory | Responsibility |
| --- | --- |
| `src/app/` | Application state and orchestration, background jobs, settings persistence and updates |
| `src/ui/` | Capsule and chat views, account/composer UI, typography and formatting, motion and native window geometry |
| `src/music/` | Audio playback and spectrum, tracks, local files, lyrics, artwork, playback order and queues |
| `src/live/` | Live-room sessions, bounded chat history/media, Bilibili protocol, account login and encrypted session storage |
| `src/providers/` | Remote music catalogues, media transport and streaming decoder |

Each directory uses normal Rust module declarations in `mod.rs`. Cross-module imports name their owning area (for example, `crate::music::track::Track`), without compatibility exports at the crate root. Existing application and view implementations retain their behavior and internal APIs; this refactor changes module ownership and paths.

The examples compile the small portions they need directly. Their `#[path]` attributes are confined to example entry points; the application module tree uses standard Rust file lookup. The music example supplies a minimal track namespace without compiling the application or UI.

Current architecture notes and example paths follow this layout. Historical release notes retain the paths that existed in those releases.

Validation: compile the application and both examples, run the existing binary test suite (including Windows DPAPI tests in the user context), and build the release. No new network operations or sending tests are required by this organizational change.
