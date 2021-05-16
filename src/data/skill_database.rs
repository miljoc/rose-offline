use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::item::AbilityType;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct SkillReference(pub usize);

#[derive(Clone, Copy, Debug)]
pub enum SkillPage {
    Basic,
    Active,
    Passive,
    Clan,
}

pub enum SkillAddAbility {
    Value(AbilityType, i32),
    Rate(AbilityType, i32),
}

#[derive(FromPrimitive)]
pub enum SkillType {
    Unknown = 0,
    Passive = 15,
}

pub struct SkillData {
    pub page: SkillPage,
    pub icon_number: u32,
    pub add_ability: Vec<SkillAddAbility>,
    pub skill_type: SkillType,
}

pub struct SkillDatabase {
    skills: HashMap<u16, SkillData>,
}

impl SkillDatabase {
    pub fn new(skills: HashMap<u16, SkillData>) -> Self {
        Self { skills }
    }

    pub fn get_skill(&self, id: &SkillReference) -> Option<&SkillData> {
        self.skills.get(&(id.0 as u16))
    }
}
