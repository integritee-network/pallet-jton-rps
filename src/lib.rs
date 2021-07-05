#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

pub use pallet::*;

use codec::{Decode, Encode};
use frame_support::{
	log,
	traits::{Randomness, LockIdentifier, schedule::{Named, DispatchTime}},
};
use frame_system::{
	WeightInfo
};
use sp_runtime::{
	traits::{Hash, Dispatchable, TrailingZeroInput}
};
use sp_std::vec::{
	Vec
};
use sp_io::hashing::blake2_256;

use pallet_matchmaker::MatchFunc;

use log::info;

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
	Running(Vec<AccountId>),
	Finished(AccountId),
}
impl<AccountId> Default for MatchState<AccountId> { fn default() -> Self { Self::None } }

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub enum GameState<AccountId> {
	None,
	Choose(Vec<AccountId>),
	Reveal(Vec<AccountId>),
}
impl<AccountId> Default for GameState<AccountId> { fn default() -> Self { Self::None } }

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub enum WeaponType {
	None,
	Rock,
	Paper,
	Scissor,
}
impl Default for WeaponType { fn default() -> Self { Self::None } }

/// Connect four board structure containing two players and the board
#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Game<Hash, AccountId, Blocknumber> {
	id: Hash,
	players: Vec<AccountId>,
	last_action: Blocknumber,
	game_state: GameState<AccountId>,
	match_state: MatchState<AccountId>,
}

const MAX_GAMES_PER_BLOCK: u8 = 10;

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

		/// Jton matchmaker pallet for match making.
		type MatchMaker: MatchFunc<Self::AccountId>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// The pallet's runtime storage items.
	// https://substrate.dev/docs/en/knowledgebase/runtime/storage
	#[pallet::storage]
	#[pallet::getter(fn something)]
	// Learn more about declaring storage items:
	// https://substrate.dev/docs/en/knowledgebase/runtime/storage#declaring-storage-items
	pub type Something<T> = StorageValue<_, u32>;

	// Default value for Nonce
	#[pallet::type_value]
	pub fn NonceDefault<T: Config>() -> u64 { 0 }
	// Nonce used for generating a different seed each time.
	#[pallet::storage]
	pub type Nonce<T: Config> = StorageValue<_, u64, ValueQuery, NonceDefault<T>>;

	#[pallet::storage]
	#[pallet::getter(fn games)]
	/// Store all games that are currently being played.
	pub type Games<T: Config> = StorageMap<_, Identity, T::Hash, Game<T::Hash, T::AccountId, T::BlockNumber>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn player_game)]
	/// Store players active games, currently only one game per player allowed.
	pub type PlayerGame<T: Config> = StorageMap<_, Identity, T::AccountId, T::Hash, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn player_choice)]
	/// Player choices of each game.
	pub type PlayerChoice<T: Config> = StorageDoubleMap<_, Blake2_128Concat, T::Hash, Blake2_128Concat, T::AccountId, T::Hash, ValueQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		SomethingStored(u32, T::AccountId),
		/// Player choosed his attack.
		PlayerChoosed(T::AccountId),
		/// A new board got created.
		NewGame(T::Hash),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		/// Player has no active game or there is no such game.
		GameDoesntExist,
		/// Player has already choosen, can't undo.
		PlayerChoiceFinish,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		// `on_initialize` is executed at the beginning of the block before any extrinsic are
		// dispatched.
		//
		// This function must return the weight consumed by `on_initialize` and `on_finalize`.
		fn on_initialize(_: T::BlockNumber) -> Weight {
			// Anything that needs to be done at the start of the block.
			// We don't do anything here.
			
			// initial weights
			let mut tot_weights = 10_000;
			for _i in 0..MAX_GAMES_PER_BLOCK {
				// try to create a match till we reached max games or no more matches available
				let result = T::MatchMaker::try_match();
				// if result is not empty we have a valid match
				if !result.is_empty() {
					// Create new game
					let _board_id = Self::create_game(result);
					// weights need to be adjusted
					tot_weights = tot_weights + T::DbWeight::get().reads_writes(1,1);
					continue;
				}
				break;
			}

			// return standard weigth for trying to fiond a match
			return tot_weights
		}

		// `on_finalize` is executed at the end of block after all extrinsic are dispatched.
		fn on_finalize(_n: BlockNumberFor<T>) {
			// Perform necessary data/state clean up here.
		}

		// A runtime code run after every block and have access to extended set of APIs.
		//
		// For instance you can generate extrinsics for the upcoming produced block.
		fn offchain_worker(_n: T::BlockNumber) {
			// We don't do anything here.
			// but we could dispatch extrinsic (transaction/unsigned/inherent) using
			// sp_io::submit_extrinsic.
			// To see example on offchain worker, please refer to example-offchain-worker pallet
		 	// accompanied in this repository.
		}
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T:Config> Pallet<T> {
		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn do_something(origin: OriginFor<T>, something: u32) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://substrate.dev/docs/en/knowledgebase/runtime/origin
			let who = ensure_signed(origin)?;

			// Update storage.
			<Something<T>>::put(something);

			// Emit an event.
			Self::deposit_event(Event::SomethingStored(something, who));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		/// An example dispatchable that may throw a custom error.
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn cause_error(origin: OriginFor<T>) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			// Read a value from storage.
			match <Something<T>>::get() {
				// Return an error if the value has not been set.
				None => Err(Error::<T>::NoneValue)?,
				Some(old) => {
					// Increment the value read from storage; will error in the event of overflow.
					let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
					// Update the value in storage with the incremented result.
					<Something<T>>::put(new);
					Ok(())
				},
			}
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn make_choice(origin: OriginFor<T>, choice: WeaponType, salt: [u8; 32]) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Make sure player has a running game.
			ensure!(PlayerGame::<T>::contains_key(&sender), Error::<T>::GameDoesntExist);
			let game_id = Self::player_game(&sender);

			// Make sure game exists.
			ensure!(Games::<T>::contains_key(&game_id), Error::<T>::GameDoesntExist);
			let game = Self::games(&game_id);

			// Make sure player has a running game.
			ensure!(!PlayerChoice::<T>::contains_key(&game_id, &sender), Error::<T>::PlayerChoiceFinish);

			let mut choice_value = salt;
			choice_value[31] = choice as u8;
			let choice_hashed = blake2_256(&choice_value);

			// insert choice into the double map.
			<PlayerChoice<T>>::insert(game_id, &sender, choice_hashed.using_encoded(T::Hashing::hash));

			// Emit an event.
			Self::deposit_event(Event::PlayerChoosed(sender));

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn reveal(origin: OriginFor<T>, choice: WeaponType, salt: T::Hash) -> DispatchResult {
			let player = ensure_signed(origin)?;


			// Emit an event.
			Self::deposit_event(Event::PlayerChoosed(player));

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

		// get current blocknumber
		let block_number = <frame_system::Pallet<T>>::block_number();

		// create a new empty game
		let game = Game {
			id: game_id,
			players: players.clone(),
			last_action: block_number,
			game_state: GameState::None,
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

	fn evaluate(
		game_id: T::Hash
	) -> bool {
		true
	}
}

