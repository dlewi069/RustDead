use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestType {
    KillZombies,
    KillBandits,
    ScavengeFood,
    LockpickSafes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub quest_type: QuestType,
    pub target_count: u32,
    pub current_progress: u32,
    pub reward_credits: u32,
    pub reward_xp: u64,
    pub is_completed: bool,
    pub is_claimed: bool,
}

impl Quest {
    pub fn new(
        id: u32,
        title: impl Into<String>,
        description: impl Into<String>,
        quest_type: QuestType,
        target_count: u32,
        reward_credits: u32,
        reward_xp: u64,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            quest_type,
            target_count,
            current_progress: 0,
            reward_credits,
            reward_xp,
            is_completed: false,
            is_claimed: false,
        }
    }

    pub fn advance(&mut self, amount: u32) -> bool {
        if self.is_completed {
            return false;
        }

        self.current_progress = (self.current_progress + amount).min(self.target_count);
        if self.current_progress >= self.target_count {
            self.is_completed = true;
            true
        } else {
            false
        }
    }
}

pub fn generate_starting_bounties() -> Vec<Quest> {
    vec![
        Quest::new(
            1,
            "Cull the Herd",
            "Eliminate 8 roaming infected walkers in the valley.",
            QuestType::KillZombies,
            8,
            200,
            80,
        ),
        Quest::new(
            2,
            "Rogue Bounty",
            "Eliminate 2 hostile bandits terrorizing the trade route.",
            QuestType::KillBandits,
            2,
            450,
            150,
        ),
        Quest::new(
            3,
            "Safecracker Operation",
            "Pick the lock of 1 secure safe or military cache.",
            QuestType::LockpickSafes,
            1,
            300,
            100,
        ),
    ]
}
