use crate::api::sector_builder::metadata::PieceMetadata;
use sector_base::api::bytes_amount::{UnpaddedByteIndex, UnpaddedBytesAmount};
use std::cmp::max;
use std::io::Cursor;
use std::io::Read;
use std::iter::Iterator;

pub struct PieceAlignment {
    pub left_bytes: UnpaddedBytesAmount,
    pub right_bytes: UnpaddedBytesAmount,
}

pub fn sum_piece_bytes_with_alignment(pieces: &[PieceMetadata]) -> UnpaddedBytesAmount {
    pieces.iter().fold(UnpaddedBytesAmount(0), |acc, p| {
        let PieceAlignment {
            left_bytes,
            right_bytes,
        } = get_piece_alignment(acc, p.num_bytes);

        acc + left_bytes + p.num_bytes + right_bytes
    })
}

pub fn get_piece_by_key(pieces: &[PieceMetadata], piece_key: &str) -> Option<PieceMetadata> {
    pieces
        .iter()
        .find(|p| p.piece_key == piece_key)
        .map(|p| p.clone())
}

pub fn get_piece_start_byte(pieces: &[PieceMetadata], piece: &PieceMetadata) -> UnpaddedByteIndex {
    let pieces: Vec<PieceMetadata> = pieces
        .into_iter()
        .take_while(|p| p.piece_key != piece.piece_key)
        .map(PieceMetadata::clone)
        .collect();
    let last_byte = sum_piece_bytes_with_alignment(&pieces);
    let alignment = get_piece_alignment(last_byte, piece.num_bytes);

    UnpaddedByteIndex::from(last_byte + alignment.left_bytes)
}

pub fn get_piece_alignment(
    written_bytes: UnpaddedBytesAmount,
    piece_bytes: UnpaddedBytesAmount,
) -> PieceAlignment {
    let minimum_piece_bytes = (4 * 32) - 1;
    let adjusted_piece_bytes = max(minimum_piece_bytes, u64::from(piece_bytes));

    let mut piece_bytes_needed = minimum_piece_bytes;

    while piece_bytes_needed < adjusted_piece_bytes {
        piece_bytes_needed *= 2;
    }

    let encroaching = u64::from(written_bytes) % piece_bytes_needed;

    let left_bytes = if encroaching > 0 {
        piece_bytes_needed - encroaching
    } else {
        0
    };

    let right_bytes = piece_bytes_needed - u64::from(piece_bytes);

    PieceAlignment {
        left_bytes: UnpaddedBytesAmount(left_bytes),
        right_bytes: UnpaddedBytesAmount(right_bytes),
    }
}

fn with_alignment(source: impl Read, piece_alignment: PieceAlignment) -> impl Read {
    let PieceAlignment {
        left_bytes,
        right_bytes,
    } = piece_alignment;

    let left_padding = Cursor::new(vec![0; left_bytes.into()]);
    let right_padding = Cursor::new(vec![0; right_bytes.into()]);

    left_padding.chain(source).chain(right_padding)
}

pub fn get_aligned_source(
    source: impl Read,
    pieces: &[PieceMetadata],
    piece_bytes: UnpaddedBytesAmount,
) -> (UnpaddedBytesAmount, impl Read) {
    let written_bytes = sum_piece_bytes_with_alignment(pieces);
    let piece_alignment = get_piece_alignment(written_bytes, piece_bytes);
    let expected_num_bytes_written =
        piece_alignment.left_bytes + piece_bytes + piece_alignment.right_bytes;

    (
        expected_num_bytes_written,
        with_alignment(source, piece_alignment),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_piece_alignment() {
        let table = vec![
            (0, 0, (0, 127)),
            (0, 127, (0, 0)),
            (0, 254, (0, 0)),
            (0, 508, (0, 0)),
            (0, 1016, (0, 0)),
            (127, 127, (0, 0)),
            (127, 254, (127, 0)),
            (127, 508, (381, 0)),
            (100, 100, (27, 27)),
            (200, 200, (54, 54)),
            (300, 300, (208, 208)),
        ];

        for (bytes_in_sector, bytes_in_piece, (expected_left_align, expected_right_align)) in
            table.clone()
        {
            let PieceAlignment {
                left_bytes: UnpaddedBytesAmount(actual_left_align),
                right_bytes: UnpaddedBytesAmount(actual_right_align),
            } = get_piece_alignment(
                UnpaddedBytesAmount(bytes_in_sector),
                UnpaddedBytesAmount(bytes_in_piece),
            );
            assert_eq!(
                (expected_left_align, expected_right_align),
                (actual_left_align, actual_right_align)
            );
        }
    }

    #[test]
    fn test_get_piece_start_byte() {
        let mut pieces: Vec<PieceMetadata> = Default::default();

        let piece_a = PieceMetadata {
            piece_key: String::from("a"),
            num_bytes: UnpaddedBytesAmount(31),
        };

        let piece_b = PieceMetadata {
            piece_key: String::from("b"),
            num_bytes: UnpaddedBytesAmount(32),
        };

        let piece_c = PieceMetadata {
            piece_key: String::from("c"),
            num_bytes: UnpaddedBytesAmount(33),
        };

        pieces.push(piece_a);
        pieces.push(piece_b);
        pieces.push(piece_c);

        assert_eq!(
            get_piece_start_byte(&pieces, &pieces[0]),
            UnpaddedByteIndex(0)
        );
        assert_eq!(
            get_piece_start_byte(&pieces, &pieces[1]),
            UnpaddedByteIndex(127)
        );
        assert_eq!(
            get_piece_start_byte(&pieces, &pieces[2]),
            UnpaddedByteIndex(254)
        );
    }
}
