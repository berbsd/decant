# decant-dashboard

Terminal rendering for the `decant dashboard` savings view.

This crate is pure presentation. `render(frame, &DashboardData)` paints a
snapshot — a `decant_store::Summary` plus a slice of `decant_store::DailyBucket`
trend points — into a [ratatui](https://ratatui.rs) `Frame`. It performs no
I/O, owns no terminal state, and runs no event loop.

The `decant dashboard` subcommand (in `tools/decant`) owns the terminal
lifecycle, fetches the data from `decant-store`, and drives the snapshot /
`--watch` loop; this crate just turns that data into widgets. Keeping rendering
side-effect-free lets it be unit-tested against ratatui's in-memory
`TestBackend`.

Layout, top to bottom: a header of headline figures (runs, tokens in→out,
percent saved), a daily saved-tokens sparkline, a scrollable table of reduced
commands ranked by tokens saved, a table of recurring no-config "opportunity"
commands, and a key-hint footer. Colours follow the Catppuccin Frappé palette.
