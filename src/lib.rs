#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

pub use pallet::*;

use codec::{Decode, Encode};
use frame_support::{
	traits::{Randomness},
};

use sp_runtime::{
	traits::{Hash, TrailingZeroInput}
};
use sp_std::vec::{
	Vec
};
use sp_io::hashing::blake2_256;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub enum MatchState<AccountId> {
	None,
	Initiate(Vec<AccountId>),
	Choose(Vec<AccountId>),
	Reveal(Vec<AccountId>),
	Finished(AccountId),
}
impl<AccountId> Default for MatchState<AccountId> { fn default() -> Self { Self::None } }

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub enum WeaponType {
	None,
	Rock,
	Paper,
	Scissor,
}
impl Default for WeaponType { fn default() -> Self { Self::None } }

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub enum Choice<Hash> {
	None,
	Choose(Hash),
	Reveal(WeaponType),
}
impl<Hash> Default for Choice<Hash> { fn default() -> Self { Self::None } }

/// Connect four board structure containing two players and the board
#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Game<Hash, AccountId> {
	id: Hash,
	players: Vec<AccountId>,
	match_state: MatchState<AccountId>,
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
	use frame_system::pallet_prelude::*;

	// important to use outside structs and consts
	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The generator used to supply randomness to contracts through `seal_random`.
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// Default value for Nonce
	#[pallet::type_value]
	pub fn NonceDefault<T: Config>() -> u64 { 0 }
	// Nonce used for generating a different seed each time.
	#[pallet::storage]
	pub type Nonce<T: Config> = StorageValue<_, u64, ValueQuery, NonceDefault<T>>;

	#[pallet::storage]
	#[pallet::getter(fn games)]
	/// Store all games that are currently being played.
	pub type Games<T: Config> = StorageMap<_, Identity, T::Hash, Game<T::Hash, T::AccountId>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn player_game)]
	/// Store players active games, currently only one game per player allowed.
	pub type PlayerGame<T: Config> = StorageMap<_, Identity, T::AccountId, T::Hash, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn player_choice)]
	/// Player choices of each game.
	pub type PlayerChoice<T: Config> = StorageDoubleMap<_, Blake2_128Concat, T::Hash, Blake2_128Concat, T::AccountId, Choice<T::Hash>, ValueQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new game got created.
		NewGame(T::Hash),
		/// A games match state changed.
		MatchStateChange(T::Hash, MatchState<T::AccountId>),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Player can't play against them self.
		NoFakePlay,
		/// Player has already a game.
		PlayerHasGame,
		/// Player has no active game or there is no such game.
		GameDoesntExist,
		/// Player choice already exist.
		PlayerChoiceExist,
		/// Player choice doesn't exist.
		PlayerChoiceDoesntExist,
		/// Bad behaviour, trying to cheat?
		BadBehaviour,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T:Config> Pallet<T> {

		/// Create game for two players
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn new_game(origin: OriginFor<T>, opponent: T::AccountId) -> DispatchResult {
			
			let sender = ensure_signed(origin)?;

			// Don't allow playing against yourself.
			ensure!(sender != opponent, Error::<T>::NoFakePlay);

			// Make sure players have no board open.
			ensure!(!PlayerGame::<T>::contains_key(&sender), Error::<T>::PlayerHasGame);
			ensure!(!PlayerGame::<T>::contains_key(&opponent), Error::<T>::PlayerHasGame);
			
			let mut players = Vec::new();
			players.push(sender.clone());
			players.push(opponent.clone());

			// Create new game
			let _game_id = Self::create_game(players);

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn initiate(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Make sure player has a running game.
			ensure!(PlayerGame::<T>::contains_key(&sender), Error::<T>::GameDoesntExist);
			let game_id = Self::player_game(&sender);

			// Make sure game exists.
			ensure!(Games::<T>::contains_key(&game_id), Error::<T>::GameDoesntExist);

			// get players game
			let game = Self::games(&game_id);

			// check if we have correct state
			if let MatchState::Initiate(_) = game.match_state {
				// check we have the correct state
			} else {
				Err(Error::<T>::BadBehaviour)?
			}

			// match state change
			if !Self::match_state_change(sender, game) {
				Err(Error::<T>::BadBehaviour)?
			}
				
			Ok(())		
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn choose(origin: OriginFor<T>, choice: WeaponType, salt: [u8; 32]) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Make sure player has a running game.
			ensure!(PlayerGame::<T>::contains_key(&sender), Error::<T>::GameDoesntExist);
			let game_id = Self::player_game(&sender);
			
			// Make sure game exists.
			ensure!(Games::<T>::contains_key(&game_id), Error::<T>::GameDoesntExist);
			
			// Make sure player has not already choosen in this game.
			ensure!(!PlayerChoice::<T>::contains_key(&game_id, &sender), Error::<T>::PlayerChoiceExist);

			// get players game
			let game = Self::games(&game_id);

			// check if we have correct state
			if let MatchState::Choose(_) = game.match_state {
				// check we have the correct state
			} else {
				Err(Error::<T>::BadBehaviour)?
			}

			// insert choice into the double map.
			<PlayerChoice<T>>::insert(game_id, &sender, Choice::Choose(Self::hash_choice(salt, choice as u8)));

			// match state change
			if !Self::match_state_change(sender, game) {
				Err(Error::<T>::BadBehaviour)?
			}

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn reveal(origin: OriginFor<T>, choice: WeaponType, salt: [u8; 32]) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Make sure player has a running game.
			ensure!(PlayerGame::<T>::contains_key(&sender), Error::<T>::GameDoesntExist);
			let game_id = Self::player_game(&sender);

			// Make sure game exists.
			ensure!(Games::<T>::contains_key(&game_id), Error::<T>::GameDoesntExist);
			// Make sure player has already choosen in this game.
			ensure!(PlayerChoice::<T>::contains_key(&game_id, &sender), Error::<T>::PlayerChoiceDoesntExist);

			// get choice of player
			let player_choice = PlayerChoice::<T>::get(&game_id, &sender);

			// get players game
			let game = Self::games(&game_id);

			// check if we have correct state
			if let MatchState::Reveal(_) = game.match_state {
				// check we have the correct state
			} else {
				Err(Error::<T>::BadBehaviour)?
			}

			match player_choice {
				Choice::Choose(org_hash) => {
					// compare persisted hash with revealing value
					if org_hash == Self::hash_choice(salt, choice.clone() as u8)  {
						PlayerChoice::<T>::insert(&game_id, &sender, Choice::Reveal(choice));
					} else {
						Err(Error::<T>::BadBehaviour)?
					}
				},
				_ => Err(Error::<T>::BadBehaviour)?,
			}

			// match state change
			if !Self::match_state_change(sender, game) {
				Err(Error::<T>::BadBehaviour)?
			}

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {

	/// Update nonce once used. 
	fn encode_and_update_nonce(
	) -> Vec<u8> {
		let nonce = <Nonce<T>>::get();
		<Nonce<T>>::put(nonce.wrapping_add(1));
		nonce.encode()
	}

	/// Generates a random hash out of a seed.
	fn generate_random_hash(
		phrase: &[u8], 
		sender: T::AccountId
	) -> T::Hash {
		let (seed, _) = T::Randomness::random(phrase);
		let seed = <[u8; 32]>::decode(&mut TrailingZeroInput::new(seed.as_ref()))
			.expect("input is padded with zeroes; qed");
		return (seed, &sender, Self::encode_and_update_nonce()).using_encoded(T::Hashing::hash);
	}

	fn create_game(
		players: Vec<T::AccountId>
	) -> T::Hash {

		// get a random hash as board id
		let game_id = Self::generate_random_hash(b"create", players[0].clone());

		// create a new empty game
		let game = Game {
			id: game_id,
			players: players.clone(),
			match_state: MatchState::Initiate(players.clone()),
		};

		// insert the new board into the storage
		<Games<T>>::insert(game_id, game);

		// insert conenction for each player with the game
		for player in &players {
			<PlayerGame<T>>::insert(player, game_id);
		}
		
		// emit event for a new game creation
		Self::deposit_event(Event::NewGame(game_id));

		game_id
	}

	fn hash_choice(
		salt: [u8; 32],
		choice: u8
	) -> T::Hash {
		let mut choice_value = salt;
		choice_value[31] = choice as u8;
		let choice_hashed = blake2_256(&choice_value);
		// return hashed choice
		choice_hashed.using_encoded(T::Hashing::hash)
	}

	fn try_remove(
		player: T::AccountId,
		players: &mut Vec<T::AccountId>
	) -> bool {
		if let Some(p) = players.iter().position(|x| *x == player) {
			// remove player from vec
			players.swap_remove(p);
			return true;
		} 
		
		false
	}

	fn match_state_change(
		player: T::AccountId,
		mut game: Game<T::Hash, T::AccountId>
	) -> bool {

		match game.match_state.clone() {

			MatchState::Initiate(mut players) => {
				if !Self::try_remove(player, &mut players) {
					return false;
				}
				// check if all players have initiated
				if players.is_empty() {
					game.match_state = MatchState::Choose(game.players.clone());
				} else {
					game.match_state = MatchState::Initiate(players);
				}				
			},

			MatchState::Choose(mut players) => {
				if !Self::try_remove(player, &mut players) {
					return false;
				}
				// check if all players have choosen
				if players.is_empty() {
					game.match_state = MatchState::Reveal(game.players.clone());
				} else {
					game.match_state = MatchState::Choose(players);
				}
			},

			MatchState::Reveal(mut players) => {
				if !Self::try_remove(player, &mut players) {
					return false;
				}
				// check if all players have revealed
				if players.is_empty() {
					// do game evaluation here
					game.match_state = MatchState::Finished(Self::evaluate(game.clone()));
				} else {
					game.match_state = MatchState::Reveal(players);
				}
			},
			_ => return false,
		}
		
		Games::<T>::insert(game.id, game);
		
		true
	}

	fn evaluate(
		game: Game<T::Hash, T::AccountId>
	) -> T::AccountId {

		let mut last_choice: WeaponType = Default::default();
		let mut last_player: T::AccountId = Default::default();
		for player in &game.players {
			if PlayerChoice::<T>::contains_key(game.id, player) {
				if let Choice::Reveal(choice) = Self::player_choice(game.id, player) {
					match Self::game_logic(&choice, &last_choice) {
						1 => {
							last_choice = choice.clone();
							last_player = player.clone();
						},
						2 => {},
						_ => {
							last_player = Default::default();
						}
					}
				}
			}
		}

		last_player
	}

	fn game_logic(
		a: &WeaponType,
		b: &WeaponType
	) -> u8 {
		match a {
			WeaponType::None => {
				if a == b {
					return 0;
				} else {
					return 2;
				}
			},
			WeaponType::Rock => {
				if a == b {
					return 0; 
				} else if let WeaponType::Paper = b {
					return 2;
				} else {
					return 1;
				}
			},
			WeaponType::Paper => {
				if a == b {
					return 0; 
				} else if let WeaponType::Scissor = b {
					return 2;
				} else {
					return 1;
				}
			},
			WeaponType::Scissor => {
				if a == b {
					return 0; 
				} else if let WeaponType::Rock = b {
					return 2;
				} else {
					return 1;
				}
			},
		}
	}
}

