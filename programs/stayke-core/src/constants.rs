use anchor_lang::prelude::*;

#[constant]
pub const LISTING_SEED: &str = "listing";

#[constant]
pub const USER_PROFILE_SEED: &str = "user_profile";

#[constant]
pub const REPUTATION_PROFILE_SEED: &str = "reputation_profile";

#[constant]
pub const IDENTITY_SEED: &str = "identity";

#[constant]
pub const CORE_CONFIG_SEED: &str = "config";

pub const MAX_HIGH_INFRACTIONS: u8 = 3;

pub const MAX_MID_INFRACTIONS: u8 = 7;

pub const MAX_LOW_INFRACTIONS: u8 = 12;
