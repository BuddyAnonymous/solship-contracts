use anchor_lang::{prelude::*, solana_program};
use solana_program::blake3::{hash, Hash};

declare_id!("8ud2dBF8N4f9eZwiWnYZ3TEXEaEvm4QHr6Tu6tYKkJ5T");

type BoardHash = [u8; 32];

const TURN_DURATION: u8 = 75; // 75 slots = 75 * 0.4s = 30s

#[program]
pub mod solship {
    use super::*;

    pub fn initialize_queue(ctx: Context<InitializeQueue>) -> Result<()> {
        let queue = &mut ctx.accounts.queue;
        queue.players = Vec::new();
        Ok(())
    }

    pub fn join_queue(ctx: Context<JoinQueue>, board_root: BoardHash) -> Result<()> {
        let queue = &mut ctx.accounts.queue;
        let player = *ctx.accounts.player.key;

        let game_player = GamePlayer {
            address: player,
            session_key: *ctx.accounts.session_key.key,
            board_root,
        };

        queue.players.push(game_player);
        Ok(())
    }

    pub fn create_game(
        ctx: Context<CreateGame>,
        enemy: Pubkey,
        board_root: BoardHash,
    ) -> Result<()> {
        let player1_board_root = board_root;
        let pos = ctx
            .accounts
            .queue
            .players
            .iter()
            .position(|p| p.address == enemy);

        if pos.is_none() {
            return err!(CustomError::PlayerNotPartOfGame);
        }

        let enemy_game_player = ctx.accounts.queue.players.remove(pos.unwrap());
        let enemy_board_root = enemy_game_player.board_root;

        let game = &mut ctx.accounts.game;
        game.player1 = *ctx.accounts.player.key;
        game.player2 = enemy;
        game.player1_board_hash = player1_board_root;
        game.player2_board_hash = enemy_board_root;
        game.current_turn = 1;
        game.player1_session_key = *ctx.accounts.session_key.key;
        game.player2_session_key = enemy_game_player.session_key;
        // game.player1_attacked_fields = [false; 100];
        // game.player2_attacked_fields = [false; 100];
        game.player1_attacked_this_turn = false;
        game.player2_attacked_this_turn = false;
        game.player1_tried_verifing_this_turn = false;
        game.player2_tried_verifing_this_turn = false;
        game.field_player1_attacked_this_turn = 255;
        game.field_player2_attacked_this_turn = 255;
        game.player1_remaining_ship_fields = 17;
        game.player2_remaining_ship_fields = 17;
        game.turn_start_slot = Clock::get()?.slot;
        game.winner = Pubkey::default();

        emit!(GameStarted {
            game: game.key(),
            player1: game.player1,
            player2: game.player2
        });

        Ok(())
    }

    pub fn attack(ctx: Context<VerifyProof>, field_to_attack: u8) -> Result<()> {
        let game = &mut ctx.accounts.game;

        let player = *ctx.accounts.player.key;
        check_time_expired(game)?;
        let player = check_if_player_is_part_of_game(player, game)?;

        if player != game.player1 && player != game.player2 {
            return Err(CustomError::PlayerNotPartOfGame.into());
        }

        if player == game.player1 && !game.player1_attacked_this_turn {
            // game.player1_attacked_fields[field_to_attack as usize] = true;
            game.field_player1_attacked_this_turn = field_to_attack;
            game.player1_attacked_this_turn = true;
        } else if player == game.player2 && !game.player2_attacked_this_turn {
            // game.player2_attacked_fields[field_to_attack as usize] = true;
            game.field_player2_attacked_this_turn = field_to_attack;
            game.player2_attacked_this_turn = true;
        } else {
            return err!(CustomError::AlreadyAttackedThisTurn);
        }

        emit!(FieldAttacked {
            game: game.key(),
            player,
            attacked_field: field_to_attack
        });

        Ok(())
    }

    pub fn verify_proof(
        ctx: Context<VerifyProof>,
        proof: [BoardHash; 7],
        leaf: GameField,
    ) -> Result<()> {
        // let player = *ctx.accounts.player.key;
        // let game: &mut Account<'_, Game> = &mut ctx.accounts.game;
        // check_time_expired(game)?;
        // check_if_player_is_part_of_game(player, game)?;

        // let proving_field_index = leaf.index;

        // Double hash the leaf to prevent second preimage attack "https://www.rareskills.io/post/merkle-tree-second-preimage-attack"
        let hashed_leaf = hash_leaf(&leaf);

        let (root, player) =
            get_player_board_hash(*ctx.accounts.player.key, &mut ctx.accounts.game)?;

        let is_proof_valid = verify_merkle_proof(
            hashed_leaf,
            proof,
            root,
            leaf.index,
            &mut ctx.accounts.game,
            player,
        )?;

        if !is_proof_valid {
            // ctx.accounts.game.winner = *ctx.accounts.player.key;
            return err!(CustomError::InvalidProof);
        }

        if player == ctx.accounts.game.player1 {
            ctx.accounts.game.player1_verified_proof_this_turn = true;
        } else if player == ctx.accounts.game.player2 {
            ctx.accounts.game.player2_verified_proof_this_turn = true;
        } else {
            return err!(CustomError::PlayerNotPartOfGame);
        }

        emit!(ProofVerified {
            game: ctx.accounts.game.key(),
            player: player,
            attacked_field: leaf.index,
            ship_placed: leaf.ship_placed
        });

        msg!("{:?} is proving", player);
        msg!("Game: {:?}", ctx.accounts.game);

        update_game_state(&mut ctx.accounts.game, leaf.ship_placed, player);

        Ok(())
    }

    pub fn claim_win(ctx: Context<ClaimWin>, table: [ProofField; 128]) -> Result<()> {
        let player = *ctx.accounts.player.key;
        let game: &mut Account<'_, Game> = &mut ctx.accounts.game;

        let player = check_if_player_is_part_of_game(player, game)?;

        let current_slot = Clock::get()?.slot;
        let turn_duration = TURN_DURATION as u64;

        // Ignore turn expiration if the enemy has no remaining ship fields
        msg!("Player1 remaining ship fields: {}", game.player1_remaining_ship_fields);
        msg!("Player2 remaining ship fields: {}", game.player2_remaining_ship_fields);
        msg!("Player1: {:?}", game.player1);
        msg!("Player2: {:?}", game.player2);
        msg!("Player: {:?}", player);
        if !((player == game.player1 && game.player2_remaining_ship_fields == 0)
            || (player == game.player2 && game.player1_remaining_ship_fields == 0))
        {
            if current_slot < game.turn_start_slot + turn_duration {
                return err!(CustomError::TurnNotExpired);
            }
        }

        // if game.winner == Pubkey::default() {
        //     return err!(CustomError::GameFinished);
        // }

        if player == game.player1
            && ((!game.player2_attacked_this_turn || !game.player2_verified_proof_this_turn)
                || game.player2_remaining_ship_fields == 0)
            && game.player1_remaining_ship_fields > 0
        {
            verify_table(table, player, &game)?;
            game.winner = game.player1;
            emit!(GameFinished {
                game: game.key(),
                winner: game.player1
            });

            return Ok(());
        } else if player == game.player2
            && ((!game.player1_attacked_this_turn || !game.player1_verified_proof_this_turn)
                || game.player1_remaining_ship_fields == 0)
            && game.player2_remaining_ship_fields > 0
        {
            verify_table(table, player, &game)?;
            game.winner = game.player2;
            emit!(GameFinished {
                game: game.key(),
                winner: game.player2
            });

            return Ok(());
        } else {
            return err!(CustomError::EnemyPlayedTurn);
        }
    }
}

fn check_time_expired(game: &Game) -> Result<()> {
    let current_slot = Clock::get()?.slot;
    let turn_duration = TURN_DURATION as u64;

    if current_slot > game.turn_start_slot + turn_duration {
        return err!(CustomError::TimeExpired);
    }
    Ok(())
}

fn verify_table(table: [ProofField; 128], player: Pubkey, game: &Game) -> Result<()> {
    let (root, _) = get_player_board_hash(player, game)?;

    let mut ships_placed_counter = 0;
    let mut ship_lengths = vec![0; 4]; // Counters for ships of length 2, 3, 4, 5
    let mut visited = vec![false; 100]; // To track visited fields

    // Identify ships and their lengths
    for index in 0..100 {
        if table[index].ship_placed && !visited[index] {
            // Check all diagonally connected fields
            let curr_row = index / 10;
            let curr_col = index % 10;
            let diagonal_directions = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
            for (dr, dc) in diagonal_directions.iter() {
                let new_row = curr_row as isize + dr;
                let new_col = curr_col as isize + dc;
                if new_row >= 0 && new_row < 10 && new_col >= 0 && new_col < 10 {
                    let new_index = (new_row * 10 + new_col) as usize;
                    if table[new_index].ship_placed {
                        return err!(CustomError::InvalidTable);
                    }
                }
            }

            let mut length = 0;
            let mut stack = vec![index];

            while let Some(current) = stack.pop() {
                if visited[current] {
                    continue;
                }
                visited[current] = true;
                length += 1;
                let row = current / 10;
                let col = current % 10;
                let directions = [(-1, 0), (1, 0), (0, -1), (0, 1)];
                for (dr, dc) in directions.iter() {
                    let new_row = row as isize + dr;
                    let new_col = col as isize + dc;
                    if new_row >= 0 && new_row < 10 && new_col >= 0 && new_col < 10 {
                        let new_index = (new_row * 10 + new_col) as usize;
                        if table[new_index].ship_placed && !visited[new_index] {
                            stack.push(new_index);
                        }
                    }
                }
            }
            if length > 5 || length < 2 {
                return err!(CustomError::InvalidTable);
            }
            ship_lengths[length - 2] += 1;
            ships_placed_counter += length;
        }
    }

    // Validate ship counts
    if ship_lengths != vec![1, 2, 1, 1] {
        return err!(CustomError::InvalidTable);
    }

    let leaves: Result<Vec<Hash>> = table
        .iter()
        .enumerate()
        .map(|(index, field)| {
            Ok(hash_leaf(&GameField {
                index: index as u8,
                ship_placed: field.ship_placed,
            }))
        })
        .collect();

    if ships_placed_counter != 17 {
        return err!(CustomError::InvalidTable);
    }

    let mut leaves = leaves?;

    msg!("Leaves[0]: {:?}", to_hex_string(&leaves[0].to_bytes()));
    msg!("Leaves[127]: {:?}", to_hex_string(&leaves[127].to_bytes()));

    while leaves.len() > 1 {
        let mut next_level = Vec::new();
        for i in (0..leaves.len()).step_by(2) {
            if i + 1 < leaves.len() {
                next_level.push(hash(
                    &[leaves[i].to_bytes(), leaves[i + 1].to_bytes()].concat(),
                ));
            } else {
                next_level.push(leaves[i].clone());
            }
        }
        leaves = next_level;
    }

    msg!("Root hash: {:?}", to_hex_string(&root));
    msg!(
        "Calculated root hash: {:?}",
        to_hex_string(&leaves[0].to_bytes())
    );

    if root == leaves[0].to_bytes() {
        Ok(())
    } else {
        err!(CustomError::InvalidTable)
    }
}

#[inline(never)]
fn hash_leaf(leaf: &GameField) -> Hash {
    hash(&leaf.serialize())
}

#[inline(never)]
fn get_player_board_hash(player: Pubkey, game: &Game) -> Result<(BoardHash, Pubkey)> {
    if player == game.player1 || player == game.player1_session_key {
        return Ok((game.player1_board_hash, game.player1));
    }

    if player == game.player2 || player == game.player2_session_key {
        return Ok((game.player2_board_hash, game.player2));
    }

    return err!(CustomError::PlayerNotPartOfGame);
}

fn check_tried_verifying(tried_verifying: &mut bool) -> Result<()> {
    if *tried_verifying {
        return err!(CustomError::AlreadyTriedVerifing);
    }
    *tried_verifying = true;
    Ok(())
}

fn check_field_index(field_index: u8, expected_field_index: u8) -> Result<()> {
    msg!(
        "Field index: {}, Expected field index {}",
        field_index,
        expected_field_index
    );
    if field_index != expected_field_index {
        return Err(CustomError::WrongProvingFieldIndex.into());
    }
    Ok(())
}

fn verify_merkle_proof(
    hashed_leaf: Hash,
    proof: [BoardHash; 7],
    root: BoardHash,
    proving_field_index: u8,
    game: &mut Account<'_, Game>,
    player: Pubkey,
) -> Result<bool> {
    // let field_player1_attacked_this_turn = game.field_player1_attacked_this_turn;
    // let field_player2_attacked_this_turn = game.field_player2_attacked_this_turn;
    // let mut player1_tried_verifing_this_turn = game.player1_tried_verifing_this_turn;
    // let mut player2_tried_verifing_this_turn = game.player2_tried_verifing_this_turn;

    // msg!("Game: {:?}", game);

    if player == game.player1 {
        check_tried_verifying(&mut game.player1_tried_verifing_this_turn)?;
        check_field_index(proving_field_index, game.field_player2_attacked_this_turn)?;
    }
    if player == game.player2 {
        check_tried_verifying(&mut game.player2_tried_verifing_this_turn)?;
        check_field_index(proving_field_index, game.field_player1_attacked_this_turn)?;
    }

    let mut last_hash = hashed_leaf;

    msg!("Last hash hex: {:?}", to_hex_string(&last_hash.to_bytes()));
    // msg!("Last hash hex: {:?}", to_hex_string(last_hash.to_bytes()));
    // msg!("Last hash hex: {:?}", to_hex_string(last_hash.to_bytes()));

    // msg!("Proof length: {}", proof.len());
    // for i in 0..proof.len() {
    //     sol_log_compute_units();
    //     let mut hasher = Hasher::default();
    //     hasher.hashv(&[&last_hash.to_bytes(), &proof[i]]);
    //     last_hash = hasher.result();
    // }

    // let mut i = 0;
    // for p in proof {
    //     msg!("Execution num: {} Proof field: {:?}", i, p);
    //     i += 1;
    // }

    let mut dir_array: Vec<u8> = Vec::new();
    let mut field_index = proving_field_index + 1; // Replace with your initial value

    while dir_array.len() < 7 {
        dir_array.push(field_index % 2);
        field_index = (field_index + 1) / 2; // Equivalent to `Math.ceil(fieldIndex / 2)`
    }

    for (i, dir) in dir_array.iter().enumerate() {
        if *dir == 0 {
            // let mut hasher = Hasher::default();
            last_hash = hash(&[proof[i], last_hash.to_bytes()].concat());
            // msg!("Last hash hex: {:?}", to_hex_string(&last_hash.to_bytes()));
        } else {
            // let mut hasher = Hasher::default();
            last_hash = hash(&[last_hash.to_bytes(), proof[i]].concat());
            // msg!("Last hash hex: {:?}", to_hex_string(&last_hash.to_bytes()));
        }
    }

    // if *player == game.player1 {
    //     msg!(
    //         "Root hash: {:?}",
    //         Hash::new_from_array(game.player1_board_hash)
    //     );
    //     Ok(last_hash == Hash::new_from_array(game.player1_board_hash))
    // } else if *player == game.player2 {
    //     msg!(
    //         "Root hash: {:?}",
    //         Hash::new_from_array(game.player2_board_hash)
    //     );
    //     Ok(last_hash == Hash::new_from_array(game.player2_board_hash))
    // } else {
    //     err!(CustomError::PlayerNotPartOfGame)
    // }

    // msg!("Root hash: {:?}", Hash::new_from_array(*root));
    Ok(last_hash == Hash::new_from_array(root))
}

fn check_if_player_is_part_of_game(player: Pubkey, game: &Game) -> Result<Pubkey> {
    if player == game.player1 || player == game.player1_session_key {
        return Ok(game.player1);
    }

    if player == game.player2 || player == game.player2_session_key {
        return Ok(game.player2);
    }

    msg!("Player: {:?}", player);
    msg!("Game: {:?}", game);

    return err!(CustomError::PlayerNotPartOfGame);
}

fn update_game_state(game: &mut Account<'_, Game>, ship_hit: bool, player: Pubkey) {
    msg!("Player: {}", player);
    msg!("Player {:?}", player);
    msg!("Player1: {:?}", game.player1);
    // msg!("game.field_player2_attacked_this_turn: {}", proving_field_index);
    // msg!("Proving field: {}", proving_field_index);

    msg!("Player1 {}", game.player1);
    msg!("game.field_player1_attacked_this_turn: {}", game.field_player1_attacked_this_turn);

    if player == game.player1 && ship_hit {
        game.player1_remaining_ship_fields -= 1;
    } else if player == game.player2 && ship_hit
    {
        game.player2_remaining_ship_fields -= 1;
    }

    if game.player1_attacked_this_turn
        && game.player2_attacked_this_turn
        && game.player1_tried_verifing_this_turn
        && game.player2_tried_verifing_this_turn
    {
        game.current_turn += 1;
        game.player1_attacked_this_turn = false;
        game.player2_attacked_this_turn = false;
        game.player1_tried_verifing_this_turn = false;
        game.player2_tried_verifing_this_turn = false;
        game.field_player1_attacked_this_turn = 255;
        game.field_player2_attacked_this_turn = 255;
        game.player1_verified_proof_this_turn = false;
        game.player2_verified_proof_this_turn = false;
        game.turn_start_slot = Clock::get().unwrap().slot;
        emit!(TurnFinished {
            game: game.key(),
            turn: game.current_turn - 1
        });

        // if game.player1_remaining_ship_fields == 0 && game.player2_remaining_ship_fields == 0 {
        //     game.winner = Pubkey::default();
        //     emit!(GameFinished {
        //         game: game.key(),
        //         winner: Pubkey::default()
        //     });
        // } else if game.player1_remaining_ship_fields == 0 {
        //     game.winner = game.player2;
        //     emit!(GameFinished {
        //         game: game.key(),
        //         winner: game.player2
        //     });
        // } else if game.player2_remaining_ship_fields == 0 {
        //     game.winner = game.player1;
        //     emit!(GameFinished {
        //         game: game.key(),
        //         winner: game.player1
        //     });
        // }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct ProofField {
    ship_placed: bool,
    // secret: u64,
    // salt: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct GameField {
    index: u8,
    ship_placed: bool,
    // secret: u64,
}

impl GameField {
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.index);
        buf.push(self.ship_placed as u8);
        // buf.extend_from_slice(&self.secret.to_le_bytes());
        buf
    }
}

#[derive(Accounts)]
pub struct InitializeQueue<'info> {
    #[account(init, seeds = [b"queue"], bump,  payer = user, space = 8 + 32 * 100)]
    // Adjust space as needed
    pub queue: Account<'info, Queue>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct JoinQueue<'info> {
    #[account(mut, seeds = [b"queue"], bump)]
    pub queue: Account<'info, Queue>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub session_key: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(enemy: Pubkey)]
pub struct CreateGame<'info> {
    #[account(init, seeds = [b"game", player.key().as_ref() , enemy.as_ref()], bump, payer = player, space = 8 + 2 * 32 + 2 * 32 + 2 * 32 + 1  + 2 + 2 + 2 + 2 + 2 + 8 + 32)]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub session_key: Signer<'info>,
    #[account(mut, seeds = [b"queue"], bump)]
    pub queue: Account<'info, Queue>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyProof<'info> {
    pub player: Signer<'info>,
    #[account(mut)]
    pub game: Box<Account<'info, Game>>,
}

#[derive(Accounts)]
pub struct ClaimWin<'info> {
    pub player: Signer<'info>,
    #[account(mut)]
    pub game: Box<Account<'info, Game>>,
}

// #[account(zero_copy)]
#[account]
#[derive(Debug)]
pub struct Game {
    pub player1: Pubkey,
    pub player2: Pubkey,
    pub player1_session_key: Pubkey,
    pub player2_session_key: Pubkey,
    pub player1_board_hash: BoardHash,
    pub player2_board_hash: BoardHash,
    pub current_turn: u8,
    // pub player1_attacked_fields: [bool; 100],
    // pub player2_attacked_fields: [bool; 100],
    pub player1_attacked_this_turn: bool,
    pub player2_attacked_this_turn: bool,
    pub player1_tried_verifing_this_turn: bool,
    pub player2_tried_verifing_this_turn: bool,
    pub player1_verified_proof_this_turn: bool,
    pub player2_verified_proof_this_turn: bool,
    pub field_player1_attacked_this_turn: u8,
    pub field_player2_attacked_this_turn: u8,
    pub player1_remaining_ship_fields: u8,
    pub player2_remaining_ship_fields: u8,
    pub turn_start_slot: u64,
    pub winner: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct GamePlayer {
    address: Pubkey,
    session_key: Pubkey,
    board_root: BoardHash,
}

#[account]
pub struct Queue {
    pub players: Vec<GamePlayer>,
}

#[event]
pub struct GameStarted {
    game: Pubkey,
    pub player1: Pubkey,
    pub player2: Pubkey,
}

#[event]
pub struct TurnFinished {
    pub game: Pubkey,
    pub turn: u8,
}

#[event]
pub struct FieldAttacked {
    game: Pubkey,
    player: Pubkey,
    attacked_field: u8,
}

#[event]
pub struct ProofVerified {
    game: Pubkey,
    player: Pubkey,
    attacked_field: u8,
    ship_placed: bool,
}

#[event]
pub struct GameFinished {
    game: Pubkey,
    winner: Pubkey,
}

#[error_code]
pub enum CustomError {
    #[msg("Player is not part of the game")]
    PlayerNotPartOfGame,
    #[msg("Wrong proving field index")]
    WrongProvingFieldIndex,
    #[msg("Invalid proof")]
    InvalidProof,
    #[msg("Player already tried verifing this turn")]
    AlreadyTriedVerifing,
    #[msg("Turn duration has not expired")]
    TurnNotExpired,
    #[msg("Invalid table")]
    InvalidTable,
    #[msg("Time expired")]
    TimeExpired,
    #[msg("Game finished")]
    GameFinished,
    #[msg("Enemy played turn")]
    EnemyPlayedTurn,
    #[msg("Player already attacked this turn")]
    AlreadyAttackedThisTurn,
}

#[inline(never)]
fn to_hex_string(bytes: &[u8; 32]) -> String {
    (*bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>())
    .to_string()
}
