//! Safety-buffering state for active turns.
//!
//! Keep-thinking is the default: no interstitial menu or status message is shown.
//! Active state only suppresses reasoning status headers until the agent responds.

use super::*;
use codex_app_server_protocol::ModelSafetyBufferingUpdatedNotification;

#[derive(Debug)]
struct ActiveSafetyBuffering {
    turn_id: String,
    agent_message_started: bool,
}

#[derive(Debug, Default)]
pub(super) struct SafetyBufferingState {
    active: Option<ActiveSafetyBuffering>,
}

impl ChatWidget {
    pub(super) fn reset_safety_buffering_for_turn_start(&mut self) {
        self.safety_buffering.active = None;
    }

    pub(crate) fn clear_safety_buffering(&mut self) {
        self.safety_buffering = SafetyBufferingState::default();
    }

    pub(super) fn mark_safety_buffering_agent_message_started(&mut self) {
        if let Some(active) = self.safety_buffering.active.as_mut() {
            active.agent_message_started = true;
        }
    }

    pub(super) fn safety_buffering_is_waiting(&self) -> bool {
        self.safety_buffering
            .active
            .as_ref()
            .is_some_and(|active| !active.agent_message_started)
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
            || self.turn_lifecycle.last_turn_id.as_deref() != Some(turn_id.as_str())
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
