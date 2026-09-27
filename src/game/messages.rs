use poker_engine::{Action, Card, LegalActions};
use serde::{Deserialize, Serialize};

/// Mode of the poker game / room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GameMode {
    #[default]
    #[serde(alias = "cash", alias = "CASH")]
    Cash,
    #[serde(alias = "tournament", alias = "TOURNAMENT")]
    Tournament,
}

// ─── Client → Server ────────────────────────────────────────────────────────

/// Messages sent by clients over WebSocket.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// List all available tables
    ListTables,

    /// Create a new table with the given config and optional game mode
    CreateTable {
        small_blind: u64,
        big_blind: u64,
        #[serde(default)]
        ante: u64,
        #[serde(default = "default_max_players")]
        max_players: usize,
        #[serde(default)]
        game_mode: GameMode,
        #[serde(default)]
        starting_chips: Option<u64>,
    },

    /// Create a tournament room with equal starting chips for all players
    CreateTournament {
        small_blind: u64,
        big_blind: u64,
        #[serde(default)]
        ante: u64,
        #[serde(default = "default_max_players")]
        max_players: usize,
        starting_chips: u64,
    },

    /// Join a table at a specific seat with a buy-in
    JoinTable {
        table_id: String,
        seat: usize,
        buy_in: u64,
    },

    /// Leave the current table
    LeaveTable {
        table_id: String,
    },

    /// Request to start a new hand (only works if enough players)
    StartHand {
        table_id: String,
    },

    /// Submit a game action (fold, check, call, bet, raise, all-in)
    PlayerAction {
        table_id: String,
        action: ActionPayload,
    },
}

fn default_max_players() -> usize {
    6
}

/// Simplified action payload for JSON deserialization.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action")]
pub enum ActionPayload {
    Fold,
    Check,
    Call,
    Bet { amount: u64 },
    Raise { amount: u64 },
    AllIn,
}

impl From<ActionPayload> for Action {
    fn from(payload: ActionPayload) -> Self {
        match payload {
            ActionPayload::Fold => Action::Fold,
            ActionPayload::Check => Action::Check,
            ActionPayload::Call => Action::Call,
            ActionPayload::Bet { amount } => Action::Bet(amount),
            ActionPayload::Raise { amount } => Action::Raise(amount),
            ActionPayload::AllIn => Action::AllIn,
        }
    }
}

// ─── Server → Client ────────────────────────────────────────────────────────

/// Messages sent by the server to clients over WebSocket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Response to ListTables
    TableList {
        tables: Vec<TableInfo>,
    },

    /// Confirmation of joining a table
    JoinedTable {
        table_id: String,
        seat: usize,
    },

    /// A player left the table
    PlayerLeft {
        table_id: String,
        seat: usize,
        username: String,
    },

    /// A player joined the table
    PlayerJoined {
        table_id: String,
        seat: usize,
        username: String,
        chips: u64,
    },

    /// A player was eliminated from the tournament
    PlayerEliminated {
        table_id: String,
        seat: usize,
        username: String,
        rank: usize,
    },

    /// The tournament has ended and a winner has been crowned
    TournamentEnded {
        table_id: String,
        winner_username: String,
        prize: u64,
    },

    /// Forwarded game event from the poker engine
    GameEvent {
        table_id: String,
        event: serde_json::Value,
    },

    /// Private: your hole cards
    HoleCards {
        table_id: String,
        cards: [Card; 2],
    },

    /// It's your turn — here are your legal actions
    YourTurn {
        table_id: String,
        legal_actions: LegalActions,
    },

    /// Full table state snapshot (sent on join)
    TableState {
        table_id: String,
        seats: Vec<SeatInfo>,
        stage: String,
        board: Vec<String>,
        pot: u64,
        /// Individual pots (main + side pots). Always at least one entry when a hand is running.
        /// Clients should display these so all-in players can see their eligible winnings.
        side_pots: Vec<SidePotInfo>,
        current_player: Option<usize>,
        game_mode: GameMode,
        is_started: bool,
    },

    /// Error message
    Error {
        message: String,
    },
}

/// Public table metadata for lobby listing.
#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub id: String,
    pub name: String,
    pub player_count: usize,
    pub max_players: usize,
    pub small_blind: u64,
    pub big_blind: u64,
    pub stage: String,
    pub game_mode: GameMode,
    pub is_started: bool,
    pub starting_chips: Option<u64>,
}

/// Public seat information (no hole cards).
#[derive(Debug, Clone, Serialize)]
pub struct SeatInfo {
    pub seat: usize,
    pub username: Option<String>,
    pub chips: Option<u64>,
    pub status: Option<String>,
    pub current_bet: Option<u64>,
}

/// One pot entry in the side-pot breakdown sent with `TableState`.
#[derive(Debug, Clone, Serialize)]
pub struct SidePotInfo {
    /// Total chips in this pot.
    pub amount: u64,
    /// Seat indices eligible to win this pot (i.e. have not folded and contributed enough).
    pub eligible_seats: Vec<usize>,
}
