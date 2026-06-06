final result: passed

# Design QA

Reference image:

- `E:/03_Projects/G-OpenSteamTool/outputs/moodboards/g-opensteamtool-ui/generated/d1f585fdc07e68388211bb590c6133ec.png`

Latest prototype screenshot:

- `E:/03_Projects/G-OpenSteamTool/outputs/g-opensteamtool-ui-screenshot-v7-window-fit.png`

Viewport:

- `1440 x 1024`

Checks:

- The UI now follows the reference's compact desktop-tool structure with a dark window shell, top title bar, left icon navigation rail, and dense work area.
- The intermediate context sidebar was removed per browser annotation feedback, returning the shell to `IconRail` plus `Workspace`.
- The left rail buttons use a fixed row width and centered active affordance for cleaner alignment.
- The left rail buttons now use square 44px by 44px button boxes, with the active marker and highlight centered on the button.
- The Tauri window and UI canvas now share the same `1380 x 860` size, so the EXE no longer shows a white outer gutter or clipped title area.
- The palette has been shifted away from purple/blue gradients toward graphite surfaces with green status/action accents.
- Panels, inputs, tables, and controls use tighter spacing, thinner dividers, and smaller radii to better match the Windows utility direction.
- No major text overlap, clipping, or layout collapse was visible in the captured overview screen.
- The about/logo image keeps its original color instead of receiving the green-tint treatment.

Remaining polish:

- The about/logo asset remains dark on the graphite surface because its original artwork is dark; this preserves the user's requested original color.
