use serde::{Deserialize, Serialize};

use rose_data::{ClanMemberPosition, SkillId};
use rose_game_common::components::{ClanLevel, ClanMark, ClanPoints, Money};

#[derive(Deserialize, Serialize, Clone)]
pub struct ClanStorageMember {
    pub name: String,
    pub position: ClanMemberPosition,
    pub contribution: ClanPoints,
}

impl ClanStorageMember {
    pub fn new(name: String, position: ClanMemberPosition) -> Self {
        Self {
            name,
            position,
            contribution: ClanPoints(0),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ClanStorage {
    pub name: String,
    pub description: String,
    pub mark: ClanMark,
    pub money: Money,
    pub points: ClanPoints,
    pub level: ClanLevel,
    pub members: Vec<ClanStorageMember>,
    pub skills: Vec<SkillId>,
}

impl ClanStorage {
    pub fn new(name: String, description: String, mark: ClanMark) -> Self {
        Self {
            name,
            description,
            mark,
            money: Money(0),
            points: ClanPoints(0),
            level: ClanLevel::new(1).unwrap(),
            members: Vec::default(),
            skills: Vec::default(),
        }
    }
}