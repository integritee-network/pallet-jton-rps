use super::*;
use crate::{Error, mock::*};

use frame_support::{assert_ok, assert_noop};

#[test]
fn it_works_for_default_value() {
	new_test_ext().execute_with(|| {
		// Dispatch a signed extrinsic.
		assert_ok!(RockPaperScissor::do_something(Origin::signed(1), 42));
		// Read pallet storage and assert an expected result.
		assert_eq!(RockPaperScissor::something(), Some(42));
	});
}

#[test]
fn correct_error_for_none_value() {
	new_test_ext().execute_with(|| {
		// Ensure the expected error is thrown when no value is present.
		assert_noop!(
			RockPaperScissor::cause_error(Origin::signed(1)),
			Error::<Test>::NoneValue
		);
	});
}

#[test]
fn test_game_creation() {
	new_test_ext().execute_with(|| {

		let player_1:u64 = 1;
		let player_2:u64 = 2;
		let player_3:u64 = 3;

		// Test player can not play against himself
		assert_noop!(
			RockPaperScissor::new_game(Origin::signed(player_1), player_1),
			Error::<Test>::NoFakePlay
		);

		// Test game creation between to different players
		assert_ok!(RockPaperScissor::new_game(Origin::signed(player_1), player_2));
		run_to_block(1);

		let game_id_1 = RockPaperScissor::player_game(player_1);
		let game_id_2 = RockPaperScissor::player_game(player_2);

		assert_eq!(game_id_1, game_id_2);

		assert_noop!(
			RockPaperScissor::new_game(Origin::signed(player_1), player_3),
			Error::<Test>::PlayerHasGame
		);

		assert_noop!(
			RockPaperScissor::new_game(Origin::signed(player_3), player_2),
			Error::<Test>::PlayerHasGame
		);

		let game = RockPaperScissor::games(game_id_1);

		assert_eq!(game.last_action, 0);

	});
}

#[test]
fn try_simple_rps_game() {
	new_test_ext().execute_with(|| {

		let player_1:u64 = 1;
		let salt_1: [u8; 32] = [1u8;32];

		let player_2:u64 = 2;
		let salt_2: [u8; 32] = [2u8;32];

		let mut current_block:u64 = 100;

		// start from block 100
		run_to_block(current_block);

		// Create game
		assert_ok!(RockPaperScissor::new_game(Origin::signed(player_1), player_2));
		let game_id = RockPaperScissor::player_game(player_1);
		let game = RockPaperScissor::games(game_id);
		matches!(game.match_state, MatchState::Initiate(_));
		assert_eq!(game.last_action, current_block);

		run_next_block();
		current_block = current_block + 1;

		// Initiate phase
		assert_ok!(RockPaperScissor::initiate(Origin::signed(player_1)));
		let game = RockPaperScissor::games(game_id);
		matches!(game.match_state, MatchState::Initiate(_));
		assert_eq!(game.last_action, current_block);

		run_next_block();
		current_block = current_block + 1;

		assert_ok!(RockPaperScissor::initiate(Origin::signed(player_2)));
		let game = RockPaperScissor::games(game_id);
		matches!(game.match_state, MatchState::Choose(_));
		assert_eq!(game.last_action, current_block);
		
		run_next_block();
		current_block = current_block + 1;

		// Choose phase
		assert_ok!(RockPaperScissor::choose(Origin::signed(player_2), WeaponType::Paper, salt_2));
		let game = RockPaperScissor::games(game_id);
		matches!(game.match_state, MatchState::Choose(_));
		assert_eq!(game.last_action, current_block);

		run_next_block();
		current_block = current_block + 1;

		assert_ok!(RockPaperScissor::choose(Origin::signed(player_1), WeaponType::Scissor, salt_1));
		let game = RockPaperScissor::games(game_id);
		matches!(game.match_state, MatchState::Reveal(_));
		assert_eq!(game.last_action, current_block);

		run_next_block();
		current_block = current_block + 1;

		// Reveal phase
		assert_ok!(RockPaperScissor::reveal(Origin::signed(player_1), WeaponType::Scissor, salt_1));
		let game = RockPaperScissor::games(game_id);
		matches!(game.match_state, MatchState::Reveal(_));
		assert_eq!(game.last_action, current_block);

		run_next_block();
		current_block = current_block + 1;

		// trying to cheat !!!
		assert_noop!(RockPaperScissor::reveal(Origin::signed(player_2), WeaponType::Rock, salt_2),
			Error::<Test>::BadReveal);
		assert_noop!(RockPaperScissor::reveal(Origin::signed(player_2), WeaponType::Paper, salt_1),
			Error::<Test>::BadReveal);

		assert_ok!(RockPaperScissor::reveal(Origin::signed(player_2), WeaponType::Paper, salt_2));
		let game = RockPaperScissor::games(game_id);
		matches!(game.match_state, MatchState::Finished(_));
		assert_eq!(game.last_action, current_block);

		// finished phase
		if let MatchState::Finished(winner) = game.match_state {
			assert_eq!(winner, player_1);
		} else {
			assert!(false);
		}
	});
}