//! Auto-next prompt generation and queueing.

use super::*;

fn sanitize_auto_next_goal_objective(objective: String) -> String {
    let objective = objective.trim();
    let objective = objective
        .strip_prefix("/goal ")
        .or_else(|| objective.strip_prefix("/goal"))
        .unwrap_or(objective)
        .trim();
    objective.trim_matches('"').trim().to_string()
}

impl ChatWidget {
    pub(super) fn maybe_auto_next(&mut self) {
        if !self.auto_next_steps && !self.auto_next_idea {
            return;
        }
        if self.auto_next_generation_in_flight {
            return;
        }
        if self.has_queued_follow_up_messages() {
            return;
        }

        if self.auto_next_steps && self.auto_next_done_file.exists() {
            let _ = std::fs::remove_file(&self.auto_next_done_file);
            self.auto_next_steps = false;
            self.auto_next_idea = true;
            self.auto_next_counter = 0;
            self.queue_user_message(
                "Review the completed work, run a focused final verification pass, and summarize any remaining risks.".into(),
            );
            self.maybe_send_next_queued_input();
            return;
        }

        let mode = if self.auto_next_idea {
            crate::legacy_core::auto_next_prompt::AutoNextMode::Idea
        } else {
            crate::legacy_core::auto_next_prompt::AutoNextMode::Steps
        };
        let fallback = self.auto_next_fallback_prompt();
        self.auto_next_counter += 1;
        self.spawn_auto_next_prompt_generation(mode, fallback);
    }

    fn auto_next_fallback_prompt(&self) -> String {
        let templates = if self.auto_next_idea {
            AUTO_NEXT_IDEA_META_TEMPLATES
        } else {
            AUTO_NEXT_STEPS_TEMPLATES
        };
        let base = templates[self.auto_next_counter % templates.len()];
        let mut prompt = base.to_string();
        if self.auto_next_steps {
            prompt.push_str(AUTO_NEXT_DONE_SUFFIX_STEPS);
            prompt.push_str(&self.auto_next_done_file.display().to_string());
        }
        prompt
    }

    fn spawn_auto_next_prompt_generation(
        &mut self,
        mode: crate::legacy_core::auto_next_prompt::AutoNextMode,
        fallback_prompt: String,
    ) {
        let Some(thread_id) = self.thread_id else {
            return;
        };
        self.auto_next_generation_in_flight = true;
        let app_event_tx = self.app_event_tx.clone();
        let config = self.config.clone();
        let rollout_path = self.current_rollout_path.clone();
        let examples: Vec<String> = if matches!(
            mode,
            crate::legacy_core::auto_next_prompt::AutoNextMode::Idea
        ) {
            AUTO_NEXT_IDEA_META_TEMPLATES
        } else {
            AUTO_NEXT_STEPS_TEMPLATES
        }
        .iter()
        .map(ToString::to_string)
        .collect();
        let append_done_suffix = self.auto_next_steps;
        let done_path = self.auto_next_done_file.display().to_string();
        tokio::spawn(async move {
            let generated = crate::legacy_core::auto_next_prompt::generate_auto_next_prompt(
                config,
                rollout_path,
                mode,
                examples,
            )
            .await;
            let mut prompt = generated.unwrap_or(fallback_prompt);
            if append_done_suffix && !prompt.contains(&done_path) {
                prompt.push_str(AUTO_NEXT_DONE_SUFFIX_STEPS);
                prompt.push_str(&done_path);
            }
            app_event_tx.send(AppEvent::AutoNextPromptGenerated { thread_id, prompt });
        });
    }

    pub(crate) fn handle_auto_next_prompt_generated(
        &mut self,
        thread_id: ThreadId,
        prompt: String,
    ) {
        self.auto_next_generation_in_flight = false;
        if self.thread_id != Some(thread_id) {
            return;
        }
        self.queue_user_message(prompt.into());
        self.maybe_send_next_queued_input();
    }

    pub(super) fn maybe_auto_next_goal(&mut self, goal: &AppThreadGoal, from_replay: bool) {
        if from_replay
            || !self.auto_next_goal
            || !self.config.features.enabled(Feature::Goals)
            || goal.status != AppThreadGoalStatus::Complete
            || self.auto_next_goal_generation_in_flight
        {
            return;
        }

        let completed_goal = (goal.thread_id.clone(), goal.objective.clone());
        if self.last_auto_next_completed_goal.as_ref() == Some(&completed_goal) {
            return;
        }
        let Ok(thread_id) = ThreadId::from_string(&goal.thread_id) else {
            return;
        };

        self.last_auto_next_completed_goal = Some(completed_goal);
        let fallback_objective = format!(
            "Identify and implement the highest-value follow-up after completing: {}",
            goal.objective
        );
        self.spawn_auto_next_goal_generation(thread_id, fallback_objective);
    }

    fn spawn_auto_next_goal_generation(&mut self, thread_id: ThreadId, fallback_objective: String) {
        self.auto_next_goal_generation_in_flight = true;
        let app_event_tx = self.app_event_tx.clone();
        let config = self.config.clone();
        let rollout_path = self.current_rollout_path.clone();
        let examples = vec![fallback_objective.clone()];
        tokio::spawn(async move {
            let generated = crate::legacy_core::auto_next_prompt::generate_auto_next_prompt(
                config,
                rollout_path,
                crate::legacy_core::auto_next_prompt::AutoNextMode::Goal,
                examples,
            )
            .await;
            let objective = generated
                .map(sanitize_auto_next_goal_objective)
                .filter(|objective| !objective.is_empty())
                .unwrap_or(fallback_objective);
            app_event_tx.send(AppEvent::AutoNextGoalGenerated {
                thread_id,
                objective,
            });
        });
    }

    pub(crate) fn handle_auto_next_goal_generated(&mut self) {
        self.auto_next_goal_generation_in_flight = false;
    }
}
