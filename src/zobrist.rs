use crate::{Square, Piece, CastlingSide, Color, Setup, Position, MoveList, Move, Outcome, Castles, RemainingChecks, Board, ByColor, Material, Bitboard, Role, File, FromSetup, CastlingMode, PositionError};
use std::num::NonZeroU32;


include!(concat!(env!("OUT_DIR"), "/zobrist.rs")); // generated by build.rs

/// Used to discriminate which variants support Zobrist hashing. See [`Zobrist`].
pub trait ZobristHashable {}


/// An extension of [`Position`] that includes an zobrist hash updated at every move.
///
/// It can be used with every variant that implements the [`ZobristHashable`] trait.
/// Updating the hash includes some overhead so only use it if needed.
/// [`hash_from_pos`] can be an alternative when needing an hash sporadically.
#[derive(Debug)]
pub struct Zobrist<P: Position + ZobristHashable> {
    pos: P,
    zobrist: u64
}

impl <P:Position + ZobristHashable> ZobristHashable for Zobrist<P> {}

impl <P:Position + ZobristHashable> Zobrist<P> {
    /// Get the zobrist hash of the current game state.
    pub fn hash(&self) -> u64 {
        self.zobrist
    }
}

impl <P:Default + Position + ZobristHashable> Default for Zobrist<P> {
    fn default() -> Self {
        let pos = P::default();
        let board = pos.board();

        // compute the zobrist hash from the pieces on the board
        let mut zobrist = zobrist_from_board(board);

        // add in all the castling
        zobrist ^= castle(Color::White, CastlingSide::KingSide);
        zobrist ^= castle(Color::White, CastlingSide::QueenSide);
        zobrist ^= castle(Color::Black, CastlingSide::KingSide);
        zobrist ^= castle(Color::Black, CastlingSide::QueenSide);

        Zobrist { pos, zobrist }
    }
}

impl <P:FromSetup + Position + ZobristHashable> FromSetup for Zobrist<P> {
    fn from_setup(setup: &dyn Setup, mode: CastlingMode) -> Result<Self, PositionError<Self>> {
        // create the underlying from the setup
        let pos = match P::from_setup(setup, mode) {
            Err(e) => return Err(PositionError { pos: Zobrist { pos: e.pos, zobrist: 0 }, errors: e.errors }), // Note, returning an hash not corresponding to the position
            Ok(p) => p
        };
        let zobrist = hash_from_pos(&pos);
        Ok(Zobrist { pos, zobrist })
    }
}

// Simply call through to the underlying methods
impl <P: Position + ZobristHashable> Setup for Zobrist<P> {
    #[inline(always)]
    fn board(&self) -> &Board {
        self.pos.board()
    }

    #[inline(always)]
    fn promoted(&self) -> Bitboard {
        self.pos.promoted()
    }

    #[inline(always)]
    fn pockets(&self) -> Option<&Material> {
        self.pos.pockets()
    }

    #[inline(always)]
    fn turn(&self) -> Color {
        self.pos.turn()
    }

    #[inline(always)]
    fn castling_rights(&self) -> Bitboard {
        self.pos.castling_rights()
    }

    #[inline(always)]
    fn ep_square(&self) -> Option<Square> {
        self.pos.ep_square()
    }

    #[inline(always)]
    fn remaining_checks(&self) -> Option<&ByColor<RemainingChecks>> {
        self.pos.remaining_checks()
    }

    #[inline(always)]
    fn halfmoves(&self) -> u32 {
        self.pos.halfmoves()
    }

    #[inline(always)]
    fn fullmoves(&self) -> NonZeroU32 {
        self.pos.fullmoves()
    }
}

// call through to the underlying methods for everything except `play_unchecked`
impl <P: Position + ZobristHashable> Position for Zobrist<P> {
    #[inline(always)]
    fn legal_moves(&self) -> MoveList {
        self.pos.legal_moves()
    }

    #[inline(always)]
    fn castles(&self) -> &Castles {
        self.pos.castles()
    }

    #[inline(always)]
    fn is_variant_end(&self) -> bool {
        self.pos.is_variant_end()
    }

    #[inline(always)]
    fn has_insufficient_material(&self, color: Color) -> bool {
        self.pos.has_insufficient_material(color)
    }

    #[inline(always)]
    fn variant_outcome(&self) -> Option<Outcome> {
        self.pos.variant_outcome()
    }

    fn play_unchecked(&mut self, m: &Move) {
        let color = self.pos.turn();

        // we need to "remove" the old EP square if there is one
        if let Some(sq) = self.pos.ep_square() {
            self.zobrist ^= ENPASSANT[sq.file() as usize];
        }

        match *m {
            Move::Normal { role, from, capture, to, promotion } => {
                // if we have an enpassant square, add it to the hash
                if let Some(sq) = self.pos.ep_square() {
                    self.zobrist ^= ENPASSANT[sq.file() as usize];
                }

                if role == Role::King {
                    // if we have the castling ability, then need to "remove" it
                    if self.castles().has(color, CastlingSide::KingSide) {
                        self.zobrist ^= castle(color, CastlingSide::KingSide);
                    }

                    if self.castles().has(color, CastlingSide::QueenSide) {
                        self.zobrist ^= castle(color, CastlingSide::QueenSide);
                    }
                } else if role == Role::Rook {
                    let side = CastlingSide::from_queen_side(from.file() == File::A);

                    if self.castles().has(color, side) {
                        self.zobrist ^= castle(color, side);
                    }
                }

                if capture == Some(Role::Rook) {
                    let side = CastlingSide::from_queen_side(to.file() == File::A);

                    if self.castles().has(color, side) {
                        self.zobrist ^= castle(color, side);
                    }
                }

                // remove the piece at the from square
                self.zobrist ^= square(from, self.board().piece_at(from).unwrap());

                // remove the piece at the to square if there is one
                if let Some(to_piece) = self.board().piece_at(to) {
                    self.zobrist ^= square(to, to_piece);
                }

                let to_piece = promotion.map_or(role.of(color), |p| p.of(color));
                self.zobrist ^= square(to, to_piece); // add in the moving piece or promotion
            }
            Move::Castle { king, rook } => {
                let side = CastlingSide::from_queen_side(rook < king);

                self.zobrist ^= square(king, color.king());
                self.zobrist ^= square(rook, color.rook());

                self.zobrist ^= square(Square::from_coords(side.rook_to_file(), rook.rank()), color.rook());
                self.zobrist ^= square(Square::from_coords(side.king_to_file(), king.rank()), color.king());

                if self.castles().has(color, CastlingSide::KingSide) {
                    self.zobrist ^= castle(color, CastlingSide::KingSide);
                }

                if self.castles().has(color, CastlingSide::QueenSide) {
                    self.zobrist ^= castle(color, CastlingSide::QueenSide);
                }
            }
            Move::EnPassant { from, to } => {
                self.zobrist ^= square(Square::from_coords(to.file(), from.rank()), (!color).pawn());
                self.zobrist ^= square(from, color.pawn());
                self.zobrist ^= square(to, color.pawn());
            }
            Move::Put { role, to } => {
                self.zobrist ^= square(to, Piece { color, role });
            }
        }

        self.zobrist ^= 0x01;  // flip the side
    }
}

/// Computes the Zobrist hash given a board
/// This is NOT the complete hash... castling and en passant are not included
fn zobrist_from_board(board: &Board) -> u64 {
    // compute the zobrist hash from the pieces on the board
    let mut zobrist = 0u64;

    for sq in (0..64).into_iter().map(|i| Square::new(i)) {
        if let Some(piece) = board.piece_at(sq) {
            zobrist ^= square(sq, piece);
        }
    }

    zobrist
}

/// Computes the Zobrist hash for given a position.
pub fn hash_from_pos<T: Position + ZobristHashable>(pos: &T) -> u64 {
    // compute the zobrist hash from the pieces on the board
    let mut zobrist = zobrist_from_board(&pos.board());

    let castles = pos.castles();

    // set castling
    if castles.has(Color::White, CastlingSide::KingSide) {
        zobrist ^= castle(Color::White, CastlingSide::KingSide);
    }

    if castles.has(Color::White, CastlingSide::QueenSide) {
        zobrist ^= castle(Color::White, CastlingSide::QueenSide);
    }

    if castles.has(Color::Black, CastlingSide::KingSide) {
        zobrist ^= castle(Color::Black, CastlingSide::KingSide);
    }

    if castles.has(Color::Black, CastlingSide::QueenSide) {
        zobrist ^= castle(Color::Black, CastlingSide::QueenSide);
    }

    if let Some(sq) = pos.ep_square() {
        zobrist ^= ENPASSANT[sq.file() as usize];
    }

    if pos.turn() == Color::Black {
        zobrist ^= SIDE;
    }
    zobrist
}

#[inline(always)]
fn square(sq: Square, piece: Piece) -> u64 {
    PIECE_SQUARE[sq as usize][<Piece as Into<usize>>::into(piece)]
}

#[inline(always)]
fn castle(color :Color, castle: CastlingSide) -> u64 {
    // there are 4 values in CASTLE: WHITE_KING[0], WHITE_QUEEN[1], BLACK_KING[2], BLACK_QUEEN[3]
    match (color, castle) {
        (Color::White, CastlingSide::KingSide) => CASTLE[0],
        (Color::White, CastlingSide::QueenSide) => CASTLE[1],
        (Color::Black, CastlingSide::KingSide) => CASTLE[2],
        (Color::Black, CastlingSide::QueenSide) => CASTLE[3]
    }
}

#[cfg(test)]
mod zobrist_tests {
    use crate::{Square, Piece, Chess, Position, CastlingMode, Move};
    use crate::fen::{epd, Fen};
    use crate::zobrist::{square, Zobrist};
    use std::collections::{HashSet, HashMap};
    use rand::prelude::*;

    #[test]
    fn square_test() {
        let mut hashes = HashSet::new();

        // go through each square and piece combo and make sure they're unique
        for sq in (0..64).into_iter().map(|i| Square::new(i)) {
            for piece in ['p','n','b','r','q','k','P','N','B','R','Q','K'].iter().map(|c| Piece::from_char(*c).unwrap()) {
                let h = square(sq, piece);

                if hashes.contains(&h) {
                    panic!("Zobrist square({}, {:?}) = {} already exists!!!", sq, piece, h);
                } else {
                    hashes.insert(h);
                }
            }
        }

        println!("LEN: {}", hashes.len());
    }

    #[test]
    fn fen_test() {
        let setup1 :Fen = "8/8/8/8/p7/P7/6k1/2K5 w - -".parse().expect("Error parsing FEN");
        let setup2 :Fen = "8/8/8/8/p7/P7/6k1/2K5 b - -".parse().expect("Error parsing FEN");

        let game1 :Zobrist<Chess> = setup1.position(CastlingMode::Standard).expect("Error setting up game");
        let game2 :Zobrist<Chess> = setup2.position(CastlingMode::Standard).expect("Error setting up game");

        println!("0x{:x} != 0x{:x}", game1.hash(), game2.hash());

        assert_ne!(game1.hash(), game2.hash());
    }

    #[test]
    fn moves_test() {
        // randomly move through a bunch of moves, ensuring we get different zobrist hashes
        const MAX_MOVES :usize = 10_000;
        let mut hash_fen :HashMap<u64, String> = HashMap::new();
        let mut hash_moves :HashMap<u64, Vec<Move>> = HashMap::new();
        let mut moves = Vec::new();
        let mut chess = Zobrist::<Chess>::default();
        let mut rnd = StdRng::seed_from_u64(0x30b3_1137_bb45_7b1b_u64);

        while hash_fen.len() < MAX_MOVES {
            // generate and collect all the moves
            let legal_moves = chess.legal_moves();
            let mv_i = rnd.gen_range(0..legal_moves.len());
            let mv = legal_moves[mv_i].clone();

            // play a random move
            chess.play_unchecked(&mv);

            // add to our current list of moves
            moves.push(mv);

            // get the zobrist hash value
            let z = chess.hash();
            let fen = epd(&chess);

            if let Some(existing_fen) = hash_fen.get(&z) {
                // found a collision!!!
                if fen != *existing_fen {
                    // check to see if the FENs are also the same
                    let setup1 :Fen = fen.parse().expect("Error parsing FEN");
                    let setup2 :Fen = existing_fen.parse().expect("Error parsing FEN");

                    let game1 :Zobrist<Chess> = setup1.position(CastlingMode::Standard).expect("Error setting up game");
                    let game2 :Zobrist<Chess> = setup2.position(CastlingMode::Standard).expect("Error setting up game");

                    if game1.hash() == game2.hash() {
                        panic!("COLLISION FOUND FOR 2 FENs: {} (0x{:x}) & {} (0x{:x})", fen, game1.hash(), existing_fen, game2.hash());
                    } else {
                        let mvs1 = hash_moves.get(&z).unwrap();
                        let mvs2 = moves;
                        let mut game = Zobrist::<Chess>::default();

                        let mut panic_str = format!("ZOBRIST COLLISION AFTER {}: 0x{:016x} ({} {})\n", hash_fen.len(), z, mvs1.len(), mvs2.len());

                        for (i, (mv1, mv2)) in mvs1.iter().zip(mvs2.iter()).enumerate() {
                            if mv1 == mv2 {
                                game.play_unchecked(mv1);
                                panic_str += format!("{:03}: {:?} -> {}\t0x{:08x}\n", i, mv1, epd(&game), game.hash()).as_str();
                            } else {
                                panic_str += format!("DIFF {:03}: {:?} {:?}", i, mv1, mv2).as_str();
                                break
                            }
                        }

                        if mvs1.len() > mvs2.len() {
                            for (i, mv1) in mvs1.iter().skip(mvs2.len()).enumerate() {
                                game.play_unchecked(mv1);
                                panic_str += format!("MV1 {:03}: {:?} -> {}\t0x{:08x}\n", i + mvs2.len(), mv1, epd(&game), game.hash()).as_str();
                            }
                        } else {
                            for (i, mv2) in mvs2.iter().skip(mvs1.len()).enumerate() {
                                game.play_unchecked(mv2);
                                panic_str += format!("MV2 {:03}: {:?} -> {}\t0x{:08x}\n", i + mvs1.len(), mv2, epd(&game), game.hash()).as_str();
                            }
                        }

                        panic!("{}", panic_str);
                    }
                }
            } else {
                // keep around the FEN of the board, and also the moves that got us there
                hash_fen.insert(z, fen);
                hash_moves.insert(z, moves.clone());
            }

            // check to see if the game is over, and if so restart it
            if chess.is_game_over() {
                chess = Zobrist::<Chess>::default();
                moves.clear();
                println!("{} of {}", hash_fen.len(), MAX_MOVES);
            }
        }

        println!("Found {} unique hashes for boards", hash_fen.len());
    }

}
