//! Safety-buffering state for active turns.
//!
//! Keep-thinking is the default: no interstitial menu or status message is shown.
//! Active state only suppresses reasoning status headers until the agent responds.

use super::*;
use codex_app_server_protocol::ModelSafetyBufferingUpdatedNotification;

const SAFETY_BUFFERING_PROMPT_VIEW_ID: &str = "safety-buffering-prompt";
#[derive(Debug)]
struct ActiveSafetyBuffering {
    turn_id: String,
    agent_message_started: bool,
}

#[derive(Debug, Default)]
pub(super) struct SafetyBufferingState {
    submitted_turn: Option<(String, AppCommand)>,
    active: Option<ActiveSafetyBuffering>,
}

impl ChatWidget {
    /// Records the submitted turn so a later safety-buffering notification can retry it.
    pub(crate) fn record_safety_buffering_turn(&mut self, turn_id: String, turn: &AppCommand) {
        self.safety_buffering.submitted_turn = Some((turn_id, turn.clone()));
    }

    pub(super) fn reset_safety_buffering_for_turn_start(&mut self) {
        self.safety_buffering.active = None;
    }

    pub(crate) fn clear_safety_buffering(&mut self) {
        self.safety_buffering = SafetyBufferingState::default();
    }

    pub(super) fn mark_safety_buffering_agent_message_started(&mut self) {
        if let Some(active) = self.safety_buffering.active.as_mut()
            && !active.agent_message_started
        {
            active.agent_message_started = true;
            self.bottom_pane
                .dismiss_view_by_id(SAFETY_BUFFERING_PROMPT_VIEW_ID);
            self.restore_reasoning_status_header();
        }
    }

    pub(super) fn safety_buffering_is_waiting(&self) -> bool {
        self.safety_buffering
            .active
            .as_ref()
            .is_some_and(|active| !active.agent_message_started)
    }

    /// The fork shows no retry menu, but `App::retry_safety_buffered_turn` still forks the
    /// buffered turn on request. Keep the state checks those entry points rely on.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn can_retry_safety_buffered_turn(&self, turn_id: &str) -> bool {
        self.turn_lifecycle.agent_turn_running
            && self
                .safety_buffering
                .active
                .as_ref()
                .is_some_and(|active| active.turn_id == turn_id && !active.agent_message_started)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn prepare_safety_buffered_retry_submission(&mut self, prompt: UserMessage) {
        self.last_rendered_user_message_display = None;
        self.finalize_turn();
        self.safety_buffering_prompt = Some(prompt);
        self.input_queue.user_turn_pending_start = true;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn commit_safety_buffered_retry_submission(&mut self, display: UserMessageDisplay) {
        self.on_user_message_display(display);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn cancel_safety_buffered_retry_submission(&mut self) {
        self.input_queue.user_turn_pending_start = false;
        self.clear_safety_buffering();
    }

    pub(super) fn on_model_safety_buffering_updated(
        &mut self,
        notification: ModelSafetyBufferingUpdatedNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        let ModelSafetyBufferingUpdatedNotification {
            turn_id,
            show_buffering_ui,
            ..
        } = notification;
        if matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages))
            || !self.turn_lifecycle.agent_turn_running
            || self.safety_recourse.is_some()
            || self.turn_lifecycle.last_turn_id.as_deref() != Some(turn_id.as_str())
            || self
                .safety_buffering
                .active
                .as_ref()
                .is_some_and(|active| active.turn_id == turn_id && active.agent_message_started)
        {
            return;
        }
        if !show_buffering_ui {
            if self
                .safety_buffering
                .active
                .as_ref()
                .is_some_and(|active| active.turn_id == turn_id)
            {
                self.safety_buffering.active = None;
                self.restore_reasoning_status_header();
            }
            return;
        }

        let agent_message_started = self
            .safety_buffering
            .active
            .as_ref()
            .filter(|active| active.turn_id == turn_id)
            .is_some_and(|active| active.agent_message_started);
        self.safety_buffering.active = Some(ActiveSafetyBuffering {
            turn_id,
            agent_message_started,
        });
    }
}
