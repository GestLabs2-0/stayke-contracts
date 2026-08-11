use anchor_lang::prelude::*;

use crate::{error::StaykeConfigError, state::GlobalConfig, CPI_AUTHORITY_SEED};

/// Which Stayke programs may invoke a given core CPI mutator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllowedCaller {
    Treasury,
    Escrow,
    Disputes,
    Core,
}

impl AllowedCaller {
    pub fn program_id(self, global_config: &GlobalConfig) -> Pubkey {
        match self {
            AllowedCaller::Treasury => global_config.treasury_program,
            AllowedCaller::Escrow => global_config.escrow_program,
            AllowedCaller::Disputes => global_config.disputes_program,
            AllowedCaller::Core => global_config.core_program,
        }
    }
}

/// Returns the CPI authority PDA for `program_id` using seed `cpi_authority`.
pub fn cpi_authority_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], program_id)
}

/// Resolve which registered Stayke program signed via its CPI PDA, if any.
pub fn resolve_cpi_caller(global_config: &GlobalConfig, cpi_authority: &Pubkey) -> Result<Pubkey> {
    for program_id in [
        global_config.treasury_program,
        global_config.escrow_program,
        global_config.disputes_program,
        global_config.core_program,
    ] {
        if program_id == Pubkey::default() {
            continue;
        }
        let (pda, _) = cpi_authority_pda(&program_id);
        if pda == *cpi_authority {
            return Ok(program_id);
        }
    }
    err!(StaykeConfigError::Unauthorized)
}

/// Verify the allowlist: `cpi_authority` must be the CPI PDA of an allowlisted registry program.
pub fn assert_cpi_authority(
    global_config: &GlobalConfig,
    cpi_authority: &Pubkey,
    allowed: &[AllowedCaller],
) -> Result<()> {
    let caller = resolve_cpi_caller(global_config, cpi_authority)?;
    let allowed_ids: Vec<Pubkey> = allowed
        .iter()
        .map(|a| a.program_id(global_config))
        .collect();
    require!(
        allowed_ids.contains(&caller),
        StaykeConfigError::Unauthorized
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config(treasury: Pubkey, escrow: Pubkey, disputes: Pubkey) -> GlobalConfig {
        GlobalConfig {
            authority: Pubkey::new_unique(),
            minimum_deposit: 0,
            fee_bps: 0,
            usdc_mint: Pubkey::new_unique(),
            is_initialized: true,
            platform_vault: Pubkey::new_unique(),
            platform_vault_bump: 0,
            core_program: Pubkey::new_unique(),
            escrow_program: escrow,
            disputes_program: disputes,
            treasury_program: treasury,
            max_operations: 0,
            bump: 255,
        }
    }

    #[test]
    fn treasury_pda_resolves_and_passes_update_deposit_allowlist() {
        let treasury = Pubkey::new_unique();
        let escrow = Pubkey::new_unique();
        let disputes = Pubkey::new_unique();
        let cfg = sample_config(treasury, escrow, disputes);
        let (pda, _) = cpi_authority_pda(&treasury);
        assert_cpi_authority(
            &cfg,
            &pda,
            &[AllowedCaller::Treasury, AllowedCaller::Disputes],
        )
        .unwrap();
    }

    #[test]
    fn escrow_pda_rejected_for_update_deposit_allowlist() {
        let treasury = Pubkey::new_unique();
        let escrow = Pubkey::new_unique();
        let disputes = Pubkey::new_unique();
        let cfg = sample_config(treasury, escrow, disputes);
        let (pda, _) = cpi_authority_pda(&escrow);
        assert!(assert_cpi_authority(
            &cfg,
            &pda,
            &[AllowedCaller::Treasury, AllowedCaller::Disputes],
        )
        .is_err());
    }

    #[test]
    fn random_wallet_rejected() {
        let cfg = sample_config(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        let wallet = Pubkey::new_unique();
        assert!(assert_cpi_authority(&cfg, &wallet, &[AllowedCaller::Escrow]).is_err());
    }

    #[test]
    fn disputes_pda_passes_add_infraction_allowlist() {
        let treasury = Pubkey::new_unique();
        let escrow = Pubkey::new_unique();
        let disputes = Pubkey::new_unique();
        let cfg = sample_config(treasury, escrow, disputes);
        let (pda, _) = cpi_authority_pda(&disputes);
        assert_cpi_authority(&cfg, &pda, &[AllowedCaller::Disputes]).unwrap();
    }

    #[test]
    fn escrow_pda_passes_set_occupied_and_host_review_allowlist() {
        let treasury = Pubkey::new_unique();
        let escrow = Pubkey::new_unique();
        let disputes = Pubkey::new_unique();
        let cfg = sample_config(treasury, escrow, disputes);
        let (pda, _) = cpi_authority_pda(&escrow);
        assert_cpi_authority(&cfg, &pda, &[AllowedCaller::Escrow]).unwrap();
    }

    #[test]
    fn treasury_pda_rejected_for_escrow_only_mutators() {
        let treasury = Pubkey::new_unique();
        let escrow = Pubkey::new_unique();
        let disputes = Pubkey::new_unique();
        let cfg = sample_config(treasury, escrow, disputes);
        let (pda, _) = cpi_authority_pda(&treasury);
        assert!(assert_cpi_authority(&cfg, &pda, &[AllowedCaller::Escrow]).is_err());
    }
}
