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
| Panel: version retention | Implemented, tested | Keep last N (0 = all), impact preview, Version history window |
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

Correction of 2026-10-29 (replaces the earlier autosave rules): **autosave writes only a local
recovery checkpoint**, in the app's data folder, outside any synced folder. It never changes the
design's file, never makes a permanent version and never reaches the cloud. **Save (Ctrl+S)**
captures the current design, writes it to its file in the chosen folder, checks it and records a
permanent version under the version policy.

| Requirement | Status | Evidence / notes |
|---|---|---|
| Autosave = local checkpoint in the app data folder (`%LOCALAPPDATA%\SolidCraft\recovery`), not temp, not RAM | Upstream, verified | `recovery::default_dir`; files survive a crash and a reboot |
| Autosave never writes the design file or its (synced) folder | Implemented, tested | `checkpoints_rotate_once_fall_back_when_damaged_and_never_touch_the_design_file` |
| Autosave makes no permanent version | Implemented, tested | `save_makes_a_version_autosave_does_not_and_lowering_retention_shows_its_cost` |
| Checkpoint validated before it replaces the last one; small rotation | Implemented, tested | The new checkpoint is parsed first; the previous one is kept as `<id>.sc.bak` (one step, never more) |
| Damaged checkpoint falls back to the previous one | Implemented, tested | `recovery::load` |
| Autosave only when there are changes; background write | Upstream | `Autosaver::begin` / `tick` (worker thread) |
| Ctrl+S saves everything, including changes after the last checkpoint | Upstream, verified | Save writes the session's current design |
| Ctrl+S records a permanent version, checked by hash and a full parse before pruning | Implemented, tested | `versions::History::publish`, `file.save {version: manual}` from the app's Save |
| Saving without changes adds no duplicate version | Implemented, tested | Same bytes as the newest version → no version |
| Successful save clears only the recovery it covers; later edits keep theirs | Upstream, verified | `adopt_save` marks saved only what was written; the entry goes when nothing is unsaved |
| Failed save keeps recovery | Implemented, tested | Same recovery test (a save into a blocked folder) |
| Keep all / last N / protected versions | Implemented, tested | `Retention`, `file.version_protect`; N=5 after V6 keeps V2–V6; protected V1 kept besides |
| Numbers never reused; versions never rewritten | Implemented, tested | `numbers_are_never_reused_and_saving_the_same_bytes_adds_nothing` |
| Lowering retention shows the impact first | Implemented, tested | Storage window shows which versions the next save removes |
| Open an old version read-only | Implemented, tested | `file.version_open` (Save refuses, autosave leaves it) |
| Restore makes a new version | Implemented, tested | `file.version_restore`: V3 restored when V8 is newest makes V9 |
| Version retention independent of checkpoint rotation | Implemented | Different folders, different rules |
| Versions in a synced folder may be deleted in the cloud by pruning | Documented | Storage window note; no claim that a version reached the cloud |
| Close with unsaved changes: Save / Don't Save / Cancel | Upstream | Existing close prompt; Don't Save discards the recovery entry (`Autosaver::discard`) |
| Recovery after a crash: name, time, recover without overwriting the original | Upstream, verified | Recover opens it unsaved with its path (`doc.recover`) |
| Recovery: "open as copy", "keep for later" | Implemented, tested | Open as Copy recovers as an untitled design (`recovery_ui::apply_copy`); Later keeps entries for the next launch; Discard asks first (`open_as_copy_leaves_the_original_and_discard_asks_first`) |
| Abrupt exit detection without a crash handler; two instances | Upstream | Locked journal and per-app locks |
| Conflict when the file changed elsewhere | Upstream, partial | Save refuses to overwrite a file changed on disk (`disk_changed`); a dedicated conflict copy is still to do |
| Untitled designs get a stable identity in the library | Missing | Needs a stable document id in the format |

## Audit of 2026-10-31 (combined build `nuclear-integration`)

| Requirement | Status | Evidence / notes |
|---|---|---|
| M on a body or component: Move/Copy, move the occurrence | Upstream, verified | `solid.move`, `ui.moveOccurrence` |
| M on a face never moves the whole body | Implemented, tested | `SolveApp::contextual_move` opens a Move Face menu: Move Along the Normal (`solid.offset_face`), Rotate About an Edge (`solid.draft` with an edge as hinge), Press Pull; `m_on_a_face_edits_the_face_and_never_moves_its_body`; checked in the app |
| Move a face (direct edit, adjacent faces rebuilt, valid solid) | Implemented, tested | Offset Face moves a face along its normal; for a flat face any translation is its normal component, which the dialog sets directly |
| Rotate a face about a straight edge, edge stays fixed | Implemented, tested | Draft accepts a straight edge: pull = face normal × edge, neutral plane through the edge; planar faces parallel to the edge; anything else is refused with the reason |
| Edits are timeline features with editable parameters, undo/redo, save/reopen | Upstream, verified | Offset Face and Draft are ordinary features |
| M on an edge alone | Implemented, tested | M on one straight edge between two planar faces opens Move Edge: Both Faces, Slide Along First Face or Slide Along Second Face, and a distance. Each face that changes turns about its far side (one `solid.draft` per face, so it is an editable timeline feature with undo/redo). Curved edges, edges without exactly two planar faces, coplanar faces and distances that would fold a face are refused with the reason and nothing moves (`m_on_an_edge_moves_the_edge_by_rebuilding_its_faces`) |
| Step control for arrows (linear and angular, custom with units, arrow keys) | Implemented | Step menu beside the Browser title (`manipulator-step-menu`) |
| Extrude: selected face first, flip, one side, two sides, symmetric, New/Join/Cut/Intersect | Upstream | Extrude dialog directions and operations |
| Shell: add and remove single faces; continuous inner walls on revolved bodies | Upstream + implemented | Shell thread (`095e949`, `e5cc797`) |
| Sketch on a selected face | Upstream | Create Sketch on the selected planar face |
| Quick measure of a face: area, size, type | Implemented, tested | Measure shows Area, Size (Bounds when not a rectangle) and Type; 35 × 25 face gives 35 × 25 and 875 mm², unchanged when the body is turned (`a_flat_face_measures_its_own_size_however_the_body_is_turned`) |
| Undo/redo, close with Save / Don't Save / Cancel | Upstream | |
| Interface: Nuclear identity, three languages, HUD like the concept | Implemented | Reports `nuclear-identity-migration.md`, `nuclear-hud-layout.md` |
| Groups, motion, exploded views | Upstream | Rigid groups, joints, motion studies, exploded views |
| Portable Windows build | Implemented | The CI artifact is a folder with `nuclear.exe` and the licence files; no installer needed |
| Untitled designs get a stable identity in the library | Implemented, tested | A new design gets a random id in its metadata (`io.nuclear.document_id`, no format change) and a name no open tab and no library file has (Untitled, Untitled 2, ...); a design saved for the first time gets one too. Rename, move and Save keep it; Duplicate gives the copy a new one (`new_designs_get_an_identity_and_a_name_no_other_design_has`) |
| Conflict copy when the file changed elsewhere | Implemented, tested | When the file changed on disk (another computer, a sync app), Save asks: Keep Both (default) writes this version as `Name (conflict, COMPUTER, YYYY-MM-DD HH.MM).sc` beside it and leaves the other version untouched; Overwrite and Cancel stay. Decided by the file's content on disk, not by clocks (`keep_both_writes_a_conflict_copy_and_leaves_the_other_version`) |
| Library: folders, search, rename/move/duplicate, copy/migrate projects, portable project export | Implemented, tested | File view → Projects: the library's real folders and designs (with pictures), breadcrumb, search over the whole library, New Folder, and per item Open, Rename, Move to Folder, Duplicate, Export Portable Copy, Show in folder. Rename and move take the version history along and rewrite links to and from the design so references never break; names Windows refuses are rejected; nothing is ever overwritten. Portable export copies the design and every linked design into `Name (portable)/Linked designs` with links pointing at the copies (`solidcraft_engine::library` tests, `projects_page_lists_searches_renames_and_moves_real_files`) |

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
| 5 | No changes, no redundant versions | `numbers_are_never_reused_and_saving_the_same_bytes_adds_nothing` |
| 6 | Failed write keeps state and recovery | `checkpoints_rotate_once_fall_back_when_damaged_and_never_touch_the_design_file` |
| 8 | Unavailable synced folder does not destroy work | `a_picked_folder_that_is_gone_is_unavailable_and_not_recreated`; checkpoints stay local |
| 9 | Retention N=5 keeps the right set | `keeping_five_after_six_saves_keeps_two_to_six` |
| 10 | Protected version survives pruning | `a_protected_version_survives_and_does_not_count` |
| 11 | Restore makes a new revision | `restoring_makes_a_new_version_and_leaves_the_old_ones_as_they_were`, `saves_record_versions_and_restoring_makes_a_new_one` |
| 12 | Forced exit recovers a valid checkpoint | `autosave_only_unsaved_changes_and_recover_after_a_crash` (upstream) |
| 13 | Crash while saving keeps the last valid state | Atomic writes (`safe_write.rs`), damaged checkpoint fallback |
| 14 | Recovery does not overwrite the original | `doc.recover` opens unsaved (upstream) |
| 16 | Recovery works offline | All local files |
