use anchor_lang::prelude::*;

use crate::{DocType, Identity, ReputationProfile, UserProfile};

#[derive(Accounts)]
pub struct Initialize {}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    msg!("Greetings from: {:?}", ctx.program_id);
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(init, payer = authority, space = 8 + 32 + 1, seeds = [b"config"], bump)]
    pub config: Account<'info, ConfigAcc>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.bump = *ctx.bumps.config;
    Ok(())
}

#[derive(Accounts)]
#[instruction(id: [u8; 32])]
pub struct InitializeUserProfile<'info> {
    #[account(init, payer = authority, space = 8 + UserProfile::INIT_SPACE, seeds = [b"user_profile", authority.key().as_ref()], bump)]
    pub user_profile: Account<'info, UserProfile>,
    #[account(init, payer = authority, space = 8 + ReputationProfile::INIT_SPACE, seeds = [b"reputation_profile", authority.key().as_ref()], bump)]
    pub reputation_profile: Account<'info, ReputationProfile>,
    #[account(
        init_if_needed, 
        payer = authority, 
        space = 8 + Identity::INIT_SPACE, 
        seeds = [b"identity", id], 
        bump
    )]
    pub identity: Account<'info, Identity>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_user_profile(ctx: Context<InitializeUserProfile>, id: [u8; 32], country_code: [u8; 2], doctype: DocType) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    user_profile.owner = ctx.accounts.authority.key();
    user_profile.identity = ctx.accounts.identity.key();
    user_profile.bump = *ctx.bumps.user_profile;

    let reputation_profile = &mut ctx.accounts.reputation_profile;
    reputation_profile.owner = ctx.accounts.authority.key();
    reputation_profile.bump = *ctx.bumps.reputation_profile;

    // This is just for initialization, we will update the identity data later when the user updates their profile.
    let identity = &mut ctx.accounts.identity;
    identity.owner = ctx.accounts.authority.key();
    identity.id = id;
    identity.doc_type = doctype;
    identity.country_code = country_code;
    identity.bump = *ctx.bumps.identity;

    Ok(())
}