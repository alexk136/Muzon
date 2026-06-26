# Library Screen (data flow)

The Library screen consumes the IPC types from issue 0011
and the real search/facets/playback-log implementations from
issue 0013. The data flow is:

```
muzon-library (Rust)
  ├── search::fts              ── FTS5 query ──┐
  ├── search::facet_genres     ── facet       ├── Library::open(pool) ──┐
  ├── search::facet_years      ── facet       │                          │
  ├── search::facet_formats    ── facet       │                          │
  ├── search::tree_counts      ── sidebar     │                          │
  ├── search::all_facets       ── chip row    │                          │
  ├── search::overview         ── single call ─┘                          │
  └── playback_log::today_stats ── sidebar card ────────────────────────┘
                                          │
                                          ▼
muzon-ipc (typed wire contract)
  ├── LibraryRequest::Search { query, limit }
  ├── LibraryRequest::TrackCount
  ├── LibraryRequest::ListAlbums
  ├── LibraryRequest::ListArtists
  ├── LibraryRequest::ListGenres   ── 0013-future (album/artist/genre list)
  ├── LibraryRequest::GetAlbum { id } ── 0013-future
  └── ... 
                                          │
                                          ▼
muzon-ui::CoreHandle (in-process dispatcher; same path used by IPC server)
  dispatch_library(req) ── calls muzon_library functions
  dispatch_playback(req) ── status / set volume
  dispatch_queue(req)    ── placeholder snapshot
  dispatch_skin(req)     ── 4 built-ins + SetActive + GetActiveCss
                                          │
                                          ▼
muzon-ipc::IpcRequest / IpcResponse envelope
  (used by the headless Unix-socket server and muzonctl)
                                          │
                                          ▼
Tauri shell (app/, v0.2.0 hardening)
  Sidebar.tsx + TrackTable.tsx + SearchBar.tsx + FilterChips.tsx
```

The Library screen's mount sequence in v0.2.0 will be:

1. Tauri `app/src-tauri/src/main.rs` builds the in-process
   `CoreHandle` (or attaches to a running headless core via
   the Unix socket).
2. The React `LibraryScreen.tsx` calls
   `core.overview()` (a single round trip that returns
   `LibraryOverview { tree, facets, today }`).
3. The sidebar mounts with the tree counts.
4. The filter-chip row mounts with the facets.
5. The "Today" stat card mounts with the today stats.
6. The track table mounts with `core.search("")` (or the
   default "all music" view) and the IPC `TrackCount` result.
7. As the user types in the search bar, the React app
   debounces 150 ms and calls
   `core.search(query, 50)` (the FTS5 query).
8. As the user adds a filter chip, the app filters the
   in-memory list (v0.2.0 ships client-side filtering on
   the search result; v0.4.0 pushes the filter to the SQL
   layer).

## v0.2.0 Rust deliverables (this issue)

- `muzon-library::search` — `fts`, `facet_genres`, `facet_years`,
  `facet_formats`, `tree_counts`, `all_facets`, `overview`
  (7 functions, ~250 LOC).
- `muzon-library::playback_log` — `record`, `today_stats`,
  `DailyStat` (~150 LOC). Migration `0003_playback_log.sql`
  adds the table.
- `muzon-library` schema.rs — `TreeCounts`, `AllFacets`,
  `LibraryOverview` typed structs.

## v0.2.0 hardening (deferred)

- Tauri app/ scaffold (`app/src-tauri/src/main.rs`,
  `app/src/main.tsx`, `app/src/App.tsx`).
- React components: `Sidebar.tsx`, `TrackTable.tsx`,
  `SearchBar.tsx`, `FilterChips.tsx`, `StatCard.tsx`.
- i18n shim (`app/src/i18n.ts` + `en.json` + `ru.json`).
- The CSS for the new components in
  `crates/muzon-skin/skins/modern/theme.css` (uses the
  existing 10 CSS variables).
- The 50k-track FTS5 perf baseline (the test from issue
  0007; the new facets do not change the search cost).
- The `playback_log` row is written by the audio engine
  on every EndOfStream event; the call site in
  `muzon-audio::engine::play` is a v0.2.0 hardening addition.
- The `no_hardcoded_colors` static check (scans all
  `*.css` and `*.tsx` under `app/src` for hex colors).
- The `i18n_keys_complete` static check (asserts every
  `t("key")` call has a matching entry in `en.json`).

## Migration history

- `0001_initial.sql` — 8 entity tables, FTS5 virtual table,
  sync triggers. Issued with 0007.
- `0003_playback_log.sql` — playback log table with
  indexes on `(played_at)` and `(track_id, played_at)`.
  Issued with 0013. The album-art column that 0008
  documented as 0002 is reserved for a v0.2.0 hardening
  pass; the migration list is `[1, 3]` until then.
