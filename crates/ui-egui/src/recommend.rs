//! Recommended tools: up to [`SLOTS`] commands of the open ribbon tab, shown after the panels,
//! learned from this computer's own use of that tab. Local personalisation, not telemetry:
//! the preferences keep a use count and a score per `<tab>/<command id>` and nothing else (no
//! geometry, file names, typed values or design contents), and nothing is sent anywhere.
//!
//! - Only deliberate starts count: a toolbar button, a menu, a shortcut, the command search.
//!   Each start of a tool or dialog counts once however many times its values change. Commands
//!   the control channel (scripts, agents, MCP) starts are not counted, nor are hovers or redraws.
//! - Nothing is recommended before [`MIN_SESSIONS`] launches and [`MIN_TAB_USES`] uses in the tab,
//!   and a command needs [`MIN_COMMAND_USES`] uses of its own.
//! - A score adds one per use, at most [`SESSION_CAP`] per launch, and fades by [`DECAY`] each
//!   launch, so frequent and recent use both count and a short burst does not stay on top.
//! - Recommendations change only at an idle boundary (a launch, or entering a tab) while no
//!   tool, dialog or pointer press is active; a newcomer replaces a shown one only when its score
//!   beats it by [`REPLACE_MARGIN`] and one more use, and takes its place without moving the others.
//! - A pinned (shown) toolbar button is never recommended; a hidden one never comes back until
//!   learned use is reset; Freeze keeps the shown ones; turning the feature off hides the area.
//!
//! The thresholds are tunable defaults, documented in `docs/nuclear/interface.md`.

use serde::{Deserialize, Serialize};
use solidcraft_engine::CommandSpec;

use crate::SolveApp;

/// The most recommendations shown per tab (fewer when the ribbon is narrow).
pub const SLOTS: usize = 3;
/// Launches observed before anything is recommended.
pub const MIN_SESSIONS: u32 = 2;
/// Deliberate uses in a tab before it gets recommendations.
pub const MIN_TAB_USES: u32 = 15;
/// Uses a command needs before it can be recommended.
pub const MIN_COMMAND_USES: u32 = 3;
/// The most one launch adds to a command's score.
pub const SESSION_CAP: f32 = 4.0;
/// How much of a score is kept from one launch to the next.
pub const DECAY: f32 = 0.8;
/// A newcomer must score this many times the weakest shown one, plus one use, to replace it.
pub const REPLACE_MARGIN: f32 = 1.25;
/// Seconds after a control-channel request during which starts are not counted.
pub const SCRIPTED_QUIET: f64 = 2.0;
/// Hostile-input cap on kept counters.
const MAX_ENTRIES: usize = 2000;

/// The panel name the toolbar draws the recommendations under.
pub const PANEL: &str = "RECOMMENDED";

/// One command's use in one tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    /// Deliberate starts, ever (until reset).
    pub uses: u32,
    /// Frequency and recency: +1 a use (capped per launch), faded each launch.
    pub score: f32,
    /// The launch the score was last brought up to date in.
    pub session: u32,
    /// What this launch has added so far.
    pub this_session: f32,
}

impl Usage {
    /// The score as of launch `now`.
    fn score_at(&self, now: u32) -> f32 {
        let age = now.saturating_sub(self.session).min(64);
        let score = if self.score.is_finite() { self.score.max(0.0) } else { 0.0 };
        score * DECAY.powi(age as i32)
    }
}

/// This run's recommendation state (not kept).
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The launch has been counted.
    pub started: bool,
    /// The tab the shown recommendations were last brought up to date for.
    pub applied_tab: Option<String>,
}

fn key(tab: &str, id: &str) -> String {
    format!("{tab}/{id}")
}

/// Was the app driven by the control channel a moment ago (a script, an agent, MCP)?
fn scripted(app: &SolveApp) -> bool {
    app.control_at.is_some_and(|at| app.now - at < SCRIPTED_QUIET && app.now >= at)
}

/// Count a deliberate start of `id` in the open tab. One call per tool or dialog session.
pub fn record(app: &mut SolveApp, id: &str) {
    if !app.preferences.adaptive_tools || scripted(app) {
        return;
    }
    let now = app.preferences.usage_sessions;
    let key = key(&app.ui.tab, id);
    if !app.preferences.tool_usage.contains_key(&key) && app.preferences.tool_usage.len() >= MAX_ENTRIES {
        return;
    }
    let usage = app.preferences.tool_usage.entry(key).or_default();
    if usage.session != now {
        usage.score = usage.score_at(now);
        usage.session = now;
        usage.this_session = 0.0;
    }
    usage.uses = usage.uses.saturating_add(1);
    if usage.this_session < SESSION_CAP {
        usage.score += 1.0;
        usage.this_session += 1.0;
    }
}

/// Forget learned use: counts, scores, shown and hidden recommendations. Pins stay.
pub fn reset(app: &mut SolveApp) {
    app.preferences.tool_usage.clear();
    app.preferences.adaptive_shown.clear();
    app.preferences.adaptive_hidden.clear();
    app.preferences.usage_sessions = 0;
    app.recommend.applied_tab = None;
}

/// Hide one recommendation in the open tab (until learned use is reset).
pub fn hide(app: &mut SolveApp, id: &str) {
    let key = key(&app.ui.tab, id);
    if !app.preferences.adaptive_hidden.contains(&key) && app.preferences.adaptive_hidden.len() < MAX_ENTRIES {
        app.preferences.adaptive_hidden.push(key);
    }
    if let Some(shown) = app.preferences.adaptive_shown.get_mut(&app.ui.tab) {
        shown.retain(|c| c != id);
    }
}

/// Is `id` one of the open tab's recommendations now?
pub fn is_shown(app: &SolveApp, id: &str) -> bool {
    app.preferences.adaptive_tools && app.preferences.adaptive_shown.get(&app.ui.tab).is_some_and(|ids| ids.iter().any(|c| c == id))
}

/// Recommendations ranked for `tab` from `eligible` (its commands that are not toolbar buttons
/// and not hidden): best first, at most [`SLOTS`], empty during the observation period.
fn ranked(app: &SolveApp, tab: &str, eligible: &[&'static CommandSpec]) -> Vec<(&'static str, f32)> {
    let now = app.preferences.usage_sessions;
    let prefix = format!("{tab}/");
    let tab_uses: u32 = app.preferences.tool_usage.iter().filter(|(k, _)| k.starts_with(&prefix)).map(|(_, u)| u.uses).fold(0, u32::saturating_add);
    if now < MIN_SESSIONS || tab_uses < MIN_TAB_USES {
        return Vec::new();
    }
    let mut scored: Vec<(&'static str, f32)> = eligible
        .iter()
        .filter_map(|c| {
            let usage = app.preferences.tool_usage.get(&key(tab, c.id))?;
            (usage.uses >= MIN_COMMAND_USES).then(|| (c.id, usage.score_at(now)))
        })
        .filter(|(_, score)| *score > 0.0)
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    scored
}

/// The shown recommendations after a boundary: the previous ones that are still eligible keep
/// their places, free slots take the best newcomers, and a newcomer replaces the weakest shown
/// one only past the replacement threshold.
fn merge(previous: &[String], ranked: &[(&'static str, f32)]) -> Vec<String> {
    let score = |id: &str| ranked.iter().find(|(c, _)| *c == id).map(|(_, s)| *s);
    let mut shown: Vec<String> = previous.iter().filter(|id| score(id).is_some()).take(SLOTS).cloned().collect();
    for (id, _) in ranked {
        if shown.len() >= SLOTS {
            break;
        }
        if !shown.iter().any(|s| s == id) {
            shown.push((*id).to_string());
        }
    }
    for (id, newcomer) in ranked {
        if shown.iter().any(|s| s == id) {
            continue;
        }
        let weakest = shown.iter().enumerate().filter_map(|(i, s)| score(s).map(|v| (i, v))).min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, weak)) = weakest
            && *newcomer > weak * REPLACE_MARGIN + 1.0
            && let Some(slot) = shown.get_mut(i)
        {
            *slot = (*id).to_string();
        }
    }
    shown
}

/// Bring the tab's recommendations up to date at an idle boundary (a launch or entering the
/// tab), never while a tool, a dialog or a pointer press is active. `commands` are the tab's
/// commands and `buttons` the ones already on its toolbar.
pub fn refresh(app: &mut SolveApp, ctx: &egui::Context, commands: &[&'static CommandSpec], buttons: &[&'static str]) {
    if !app.recommend.started {
        app.recommend.started = true;
        app.preferences.usage_sessions = app.preferences.usage_sessions.saturating_add(1);
    }
    let tab = app.ui.tab.clone();
    if app.recommend.applied_tab.as_deref() == Some(tab.as_str()) {
        return;
    }
    let busy = app.tool.is_some() || app.dialog.is_some() || ctx.input(|i| i.pointer.any_down()) || ctx.dragged_id().is_some();
    if busy {
        return;
    }
    app.recommend.applied_tab = Some(tab.clone());
    if !app.preferences.adaptive_tools || app.preferences.adaptive_frozen {
        return;
    }
    let hidden = |id: &str| app.preferences.adaptive_hidden.iter().any(|k| *k == key(&tab, id));
    let eligible: Vec<&'static CommandSpec> = commands.iter().copied().filter(|c| !buttons.contains(&c.id) && !hidden(c.id)).collect();
    let ranked = ranked(app, &tab, &eligible);
    let previous = app.preferences.adaptive_shown.get(&tab).cloned().unwrap_or_default();
    let shown = merge(&previous, &ranked);
    if shown.is_empty() {
        app.preferences.adaptive_shown.remove(&tab);
    } else {
        app.preferences.adaptive_shown.insert(tab, shown);
    }
}

/// The recommendations to draw now for the open tab: the applied ones, less any since pinned
/// to the toolbar or hidden. Nothing is added between boundaries.
pub fn shown(app: &SolveApp, commands: &[&'static CommandSpec], buttons: &[&'static str]) -> Vec<&'static CommandSpec> {
    if !app.preferences.adaptive_tools {
        return Vec::new();
    }
    let tab = &app.ui.tab;
    let Some(ids) = app.preferences.adaptive_shown.get(tab) else { return Vec::new() };
    ids.iter()
        .filter(|id| !buttons.contains(&id.as_str()) && !app.preferences.adaptive_hidden.iter().any(|k| *k == key(tab, id)))
        .filter_map(|id| commands.iter().copied().find(|c| c.id == id))
        .take(SLOTS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn harness() -> crate::scenario::Harness {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.app.ui.tab = "MODEL".into();
        h.app.control_at = None;
        h
    }

    fn uses(h: &crate::scenario::Harness, id: &str) -> u32 {
        h.app.preferences.tool_usage.get(&format!("MODEL/{id}")).map_or(0, |u| u.uses)
    }

    #[test]
    fn a_burst_counts_at_most_the_session_cap_and_fades() {
        let mut h = harness();
        h.app.preferences.usage_sessions = 1;
        for _ in 0..20 {
            record(&mut h.app, "solid.fillet");
        }
        let usage = h.app.preferences.tool_usage["MODEL/solid.fillet"];
        assert_eq!((usage.uses, usage.score), (20, SESSION_CAP));
        assert!((usage.score_at(3) - SESSION_CAP * DECAY * DECAY).abs() < 1e-4);
    }

    #[test]
    fn control_channel_starts_and_a_disabled_feature_do_not_count() {
        let mut h = harness();
        h.call("ui.start", json!({"command": "solid.chamfer"}));
        assert_eq!(uses(&h, "solid.chamfer"), 0, "an agent's start is not the user's use");
        h.app.control_at = None;
        h.app.preferences.adaptive_tools = false;
        record(&mut h.app, "solid.chamfer");
        assert_eq!(uses(&h, "solid.chamfer"), 0);
        h.app.preferences.adaptive_tools = true;
        h.app.start("solid.chamfer");
        assert_eq!(uses(&h, "solid.chamfer"), 1, "a deliberate start counts once");
    }

    #[test]
    fn nothing_is_recommended_during_the_observation_period() {
        let mut h = harness();
        let specs: Vec<&'static CommandSpec> =
            ["solid.fillet", "solid.chamfer"].iter().filter_map(|id| solidcraft_engine::find_command(id)).collect();
        h.app.preferences.usage_sessions = 1;
        for _ in 0..4 {
            record(&mut h.app, "solid.fillet");
        }
        assert!(ranked(&h.app, "MODEL", &specs).is_empty(), "one launch and four uses are not enough");
        for session in 2..6 {
            h.app.preferences.usage_sessions = session;
            for _ in 0..4 {
                record(&mut h.app, "solid.fillet");
            }
        }
        assert_eq!(ranked(&h.app, "MODEL", &specs).first().map(|r| r.0), Some("solid.fillet"));
    }

    #[test]
    fn small_differences_do_not_swap_shown_tools_and_places_are_kept() {
        let previous = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let close = [("d", 3.2), ("a", 3.0), ("b", 2.9), ("c", 2.8)];
        assert_eq!(merge(&previous, &close), previous, "d is not far enough ahead");
        let far = [("d", 9.0), ("a", 3.0), ("b", 2.9), ("c", 2.8)];
        assert_eq!(merge(&previous, &far), ["a", "b", "d"], "d takes the weakest one's place");
        assert_eq!(merge(&[], &far), ["d", "a", "b"]);
    }

    #[test]
    fn pinned_hidden_and_frozen_recommendations_follow_the_user() {
        let mut h = harness();
        let specs: Vec<&'static CommandSpec> =
            ["solid.fillet", "solid.chamfer", "solid.shell", "solid.draft"].iter().filter_map(|id| solidcraft_engine::find_command(id)).collect();
        for session in 1..5 {
            h.app.preferences.usage_sessions = session;
            for id in ["solid.fillet", "solid.chamfer", "solid.shell", "solid.draft"] {
                for _ in 0..2 {
                    record(&mut h.app, id);
                }
            }
        }
        h.app.recommend.started = true;
        h.app.recommend.applied_tab = None;
        refresh(&mut h.app, &h.ctx, &specs, &["solid.fillet"]);
        let shown_now: Vec<_> = shown(&h.app, &specs, &["solid.fillet"]).iter().map(|c| c.id).collect();
        assert_eq!(shown_now.len(), SLOTS);
        assert!(!shown_now.contains(&"solid.fillet"), "a toolbar button is never recommended too");
        let first = shown_now[0];
        hide(&mut h.app, first);
        assert!(!shown(&h.app, &specs, &["solid.fillet"]).iter().any(|c| c.id == first));
        // Frozen: a new boundary changes nothing.
        h.app.preferences.adaptive_frozen = true;
        let before = h.app.preferences.adaptive_shown.clone();
        h.app.recommend.applied_tab = None;
        refresh(&mut h.app, &h.ctx, &specs, &[]);
        assert_eq!(h.app.preferences.adaptive_shown, before);
        // Off: nothing shows; pins are untouched.
        h.app.preferences.adaptive_tools = false;
        assert!(shown(&h.app, &specs, &[]).is_empty());
        reset(&mut h.app);
        assert!(h.app.preferences.tool_usage.is_empty() && h.app.preferences.adaptive_hidden.is_empty());
    }

    #[test]
    fn recommendations_do_not_change_while_a_tool_is_active() {
        let mut h = harness();
        let specs: Vec<&'static CommandSpec> =
            ["solid.fillet", "solid.chamfer"].iter().filter_map(|id| solidcraft_engine::find_command(id)).collect();
        for session in 1..6 {
            h.app.preferences.usage_sessions = session;
            for _ in 0..4 {
                record(&mut h.app, "solid.chamfer");
            }
        }
        h.app.recommend.started = true;
        h.app.recommend.applied_tab = None;
        h.app.start("solid.fillet");
        assert!(h.app.tool.is_some() || h.app.dialog.is_some());
        refresh(&mut h.app, &h.ctx, &specs, &[]);
        assert!(h.app.preferences.adaptive_shown.is_empty(), "no change during an interaction");
        assert!(h.app.recommend.applied_tab.is_none(), "it waits for the next idle frame");
    }
}
