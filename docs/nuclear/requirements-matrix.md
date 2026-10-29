# Nuclear 3D: baseline and requirements matrix

This is the acceptance matrix for the Nuclear 3D specification (2026-10-29). Each stage of work
updates it. Code, UI text and docs stay in English; the UI ships English and Portuguese (Brazil).

## Baseline

| Item | Value |
|---|---|
| Base | SolidCraft import on `main` (`93d0298`), rebrand in PR #1 (`claude/rebrand-nuclear`, `e95c5d4`) |
| Language and build | Rust workspace; `cargo build --profile ci -p solidcraft --bins`; Windows exe built by `.github/workflows/nuclear.yml` |
| Tests | `cargo test --profile ci -p <crate> --lib` (the dev profile trips egui debug checks in headless UI tests) |
| Kernel | `crates/kernel` (B-rep on truck, the only crate allowed to use it) |
| Sketch solver | `crates/sketch` |
| Document and history | `crates/doc` (design, feature timeline), `crates/engine` (`Session`: commands, undo/redo snapshots) |
| Persistence | `crates/io` (`.sc` container, atomic writes with `.bak`), `crates/engine/src/recovery.rs` (autosave for recovery), `crates/engine/src/crash.rs` (crash journal and reports) |
| Rendering | `crates/render` (scene), `crates/gpu` / `crates/ui-egui` (wgpu viewport) |
| UI | `crates/ui-egui` (egui), desktop app `apps/solidcraft` |

Status words: **Verified** (exists and was checked), **Partial**, **Missing**, **Implemented, untested**,
**Blocked** (with the reason). "Upstream" marks a feature found in the SolidCraft code that has not
yet been checked against the Nuclear criteria; it is not a claim that the criterion is met.

## 2. Identity and autonomy

| Requirement | Status | Evidence / notes |
|---|---|---|
| Modelling, opening, saving, export work offline | Upstream | Engine and IO are local; update check is optional (`crates/ui-egui/src/update.rs`) |
| No account, subscription or periodic validation | Upstream | No sign-in code in the desktop app |
| No telemetry, no automatic upload of designs | Upstream | Crash reports stay local (`crash.rs`: "Nothing is ever sent anywhere") |
| Windows native first; portable build | Partial | Windows exe from CI; a portable zip is not packaged yet |

## 3. Start page and Storage button

| Requirement | Status | Evidence / notes |
|---|---|---|
| Keep New, Open, Import on the Start page | Verified | `crates/ui-egui/src/home.rs` unchanged apart from the title row |
| Button beside "Start": "Storage: Local" / "Storage: Synced folder" | Implemented, tested | `storage_ui::button`; scenario handle `home:storage` |
| Panel: full path, local vs synced, choose folder, show in Explorer | Implemented, tested | `storage_ui::show` |
| Panel: autosave on/off and frequency | Implemented | Drives the existing recovery autosave interval (`autosave_minutes`) |
| Panel: version retention | Missing | Stage 3 (version history) |
| Panel: access to session recovery | Implemented | Opens the Start page's Recover section |
| Panel: who does the syncing | Implemented | `storage.explain_synced` / `storage.explain_local` |
| Advanced area | Implemented | Documents, recovery and crash folders |
| Choice persists between runs | Implemented, tested | `storage_kind`, `storage_folder` preferences |
| First run creates Documents/Nuclear Projects via the OS API | Implemented, tested (Linux); Windows built in CI | `known_folders.rs` (`SHGetKnownFolderPath`, follows OneDrive redirection); `storage::prepare` |
| Idempotent: reuse, never delete or overwrite | Implemented, tested | `storage` tests: second run, file in the way |
| Folder cannot be created: say so and offer another | Implemented, tested | `storage.prepare_failed` status, the window's error and Choose folder |
| Library is the default for new designs | Implemented | Save and Open dialogs start in the library until a folder is chosen |
| Changing the library does not move designs | Implemented, tested | `changing_the_library_moves_nothing_and_survives_a_restart` |
| Separate copy/migrate projects action | Missing | Later stage |

## 4. Synced folder

| Requirement | Status | Evidence / notes |
|---|---|---|
| Pick a provider folder with the system picker; no password, no OAuth | Implemented | `Services::pick_folder` (rfd) |
| No assumed provider paths | Implemented | Only the picked path is used |
| Destination unavailable does not destroy work | Partial | Status "not available"; recovery autosave stays local. Full save-state flow in stage 3 |
| True states (unsaved, saving, saved in this folder, sync not verified, unavailable, conflict) | Partial | Unavailable / ready shown; per-document states in stage 3 |
| No re-creation of a missing picked folder | Implemented, tested | `a_picked_folder_that_is_gone_is_unavailable_and_not_recreated` |
| Conflict detection on return | Missing | Stage 3 |
| Direct Google Drive integration | Deferred by spec | Not started; no login button |

## 5. Project organisation

| Requirement | Status | Evidence / notes |
|---|---|---|
| Recents, thumbnails, tabs for open documents | Upstream | `home.rs` recent list with pictures, `documents.rs` tabs |
| Native format reused (`.sc`) | Verified | No new extension |
| Folders, search, rename/move/duplicate, show in Explorer, portable export | Partial / Missing | Later stage |

## 6–9. Autosave, persistence, versions, crash recovery

| Requirement | Status | Evidence / notes |
|---|---|---|
| Autosave without interrupting modelling, background write | Upstream | `recovery::Autosaver::tick` writes on a worker thread |
| No write when nothing changed | Upstream | Autosaver writes only unsaved, unwritten changes |
| Trigger after N confirmed operations / 2 minutes | Partial | Interval in minutes plus "after a big operation"; op-count trigger in stage 3 |
| Autosave updates the current document (A) | Missing | Today autosave writes only the recovery copy; stage 3 |
| Version history with retention and protected versions (B) | Missing | Stage 3 |
| Crash recovery separate from the document and cloud (C) | Upstream | Per-user recovery folder (`%LOCALAPPDATA%`), locks per running app, offered at next launch |
| Atomic writes that keep the previous file | Upstream | `crates/io/src/safe_write.rs` (temp, flush, rename, `.bak`) |
| Abrupt exit detection without a crash handler | Upstream | Locked command journal (`crash.rs`) |
| Two instances | Upstream | One lock per running app's entry |
| Recover / open as copy / keep for later / discard with confirmation | Partial | Recover and discard exist (`recovery_ui.rs`); "open as copy" and "keep for later" in stage 3 |

## 10–19. Later stages

Contextual M, face and edge edits, step control, extrusion, shell, sketch on face, measurements,
undo performance, interface tokens, viewport, browser, groups and exploded views are assessed at the
start of their stage. Upstream already has `solid.move` (key M, bodies only), `solid.offset_face`,
`solid.replace_face`, `solid.press_pull`, `solid.shell`, measurement commands, motion studies and
exploded views; each will be checked against the spec's criteria before any change.

## 21. Acceptance tests covered so far

| # | Test | Status |
|---|---|---|
| 1 | First run creates Nuclear Projects in the real Documents | Engine and UI tests (with a test Documents folder); Windows API path built in CI |
| 2 | Second run keeps existing files | `a_second_run_keeps_everything_already_in_the_library` |
| 3 | Button beside Start shows and changes the setting | `storage_ui` tests (setting, persistence); screenshot in the PR |
| 7 | Changing destination does not move files | `changing_the_library_moves_nothing_and_survives_a_restart` |
| 8 | Unavailable synced folder does not destroy work | `a_picked_folder_that_is_gone_is_unavailable_and_not_recreated` (status only; save path in stage 3) |
