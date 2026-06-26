# Skin Injection

The Tauri shell injects the active skin's `theme.css` into the
WebView at startup and on every skin switch. The injection is
local — no fonts or stylesheets are loaded over the network
(decision 0001-N4, TZ §3.9). The §3.9 enforcement is the
`no_remote_assets` static check in `crates/muzon-ui::no_remote_assets_check`,
which scans every `.html`, `.css`, `.ts`, `.tsx`, `.toml`, and
`.tmpl` under `app/src` and `crates/muzon-skin/skins/` and
fails the build if any `http://` or `https://` reference is
present.

## Flow

1. The Tauri shell calls `CoreHandle::new()` at startup, which
   loads the four built-in skin manifests and the active skin's
   `theme.css` + `index.html.tmpl` from `muzon-skin`.
2. The frontend's React mount sequence calls the `GetActiveCss`
   IPC command. The Tauri command handler delegates to
   `CoreHandle::dispatch_skin`, which returns the
   `SkinCssPayload { skin, css, main_template }`.
3. The frontend injects the CSS into a `<style id="muzon-skin">`
   element under `<head>` and renders the main template into
   the root container.
4. Switching skins (Settings → Skin picker, 0019) calls
   `SetActive { id }` and then `GetActiveCss` again; the
   frontend replaces the `<style>` element content and re-renders
   the main template.

## Code snippet (frontend)

```ts
import { invoke } from "@tauri-apps/api/core";
import type { SkinCssPayload } from "muzon-ipc";

const payload = await invoke<SkinCssPayload>({ op: "get_active_css" });
const styleEl = document.getElementById("muzon-skin") as HTMLStyleElement;
styleEl.textContent = payload.css;
document.getElementById("root")!.innerHTML = payload.main_template;
```

## Switching skins

```ts
await invoke({ op: "set_active", id: "winamp" });
const next = await invoke<SkinCssPayload>({ op: "get_active_css" });
styleEl.textContent = next.css;
document.getElementById("root")!.innerHTML = next.main_template;
```

## Enforced invariants

- No `http://` or `https://` reference in any skin asset
  (no-remote-assets check).
- Every skin's `theme.css` defines the 10 required CSS
  variables (muzon-skin validator).
- The default skin is `modern` (decision 0005).
- The active skin is set via the IPC `SetActive` command;
  the dispatcher rejects unknown ids.
