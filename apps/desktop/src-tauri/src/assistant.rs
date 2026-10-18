//! The built-in assistant's running turns, so Stop can end one. Everything else lives in the
//! `plumbq-assistant` crate.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct AssistantState {
    /// Per chat: the running turn's number and its stop token.
    running: Mutex<HashMap<String, (u64, CancellationToken)>>,
    turns: Mutex<u64>,
}

impl AssistantState {
    /// Registers a turn for `chat_id`, stopping one still running there. Returns the turn's
    /// number (for [`Self::finish`]) and its stop token.
    pub fn start(&self, chat_id: &str) -> (u64, CancellationToken) {
        let turn = {
            let mut turns = self.turns.lock().unwrap_or_else(|e| e.into_inner());
            *turns += 1;
            *turns
        };
        let token = CancellationToken::new();
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, previous)) = running.insert(chat_id.to_string(), (turn, token.clone())) {
            previous.cancel();
        }
        (turn, token)
    }

    pub fn stop(&self, chat_id: &str) {
        let running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, token)) = running.get(chat_id) {
            token.cancel();
        }
    }

    /// Stops every running turn, for "Turn AI off".
    pub fn stop_all(&self) {
        let running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        for (_, token) in running.values() {
            token.cancel();
        }
    }

    /// Forgets a finished turn, unless a newer one replaced it.
    pub fn finish(&self, chat_id: &str, turn: u64) {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if running.get(chat_id).is_some_and(|(t, _)| *t == turn) {
            running.remove(chat_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_turn_stops_the_old_one_and_only_its_own_finish_counts() {
        let state = AssistantState::default();
        let (first, old) = state.start("c");
        let (second, new) = state.start("c");
        assert!(old.is_cancelled());
        state.finish("c", first);
        state.stop("c");
        assert!(new.is_cancelled(), "the newer turn is still registered");
        state.finish("c", second);
        assert!(state.running.lock().unwrap().is_empty());
    }

    #[test]
    fn turning_ai_off_stops_every_chat() {
        let state = AssistantState::default();
        let (_, one) = state.start("one");
        let (_, two) = state.start("two");
        state.stop_all();
        assert!(one.is_cancelled() && two.is_cancelled());
    }
}
