//! Bounded orthogonal-array designs for Calibration.
//!
//! The generator is a native Rust port of the small prime-field construction
//! used by `bigattichouse/robust` (`optimize/taguchi/src/lib/arrays.c`, pinned
//! commit `a457b7f7f4a7a06b183fd55be4b8aced5d7f2541`, CC0-1.0). It is purposely
//! bounded to the 3-level L9 and 5-level L25 arrays needed by Calibration v1;
//! it does not link or execute the upstream C library.

use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrthogonalArray {
    L9,
    L25,
}

impl OrthogonalArray {
    pub const fn levels(self) -> u8 {
        match self {
            Self::L9 => 3,
            Self::L25 => 5,
        }
    }

    pub const fn rows(self) -> usize {
        match self {
            Self::L9 => 9,
            Self::L25 => 25,
        }
    }

    pub const fn max_columns(self) -> usize {
        match self {
            Self::L9 => 4,
            Self::L25 => 6,
        }
    }
}

/// Generate the first `columns` columns of a bounded Taguchi orthogonal array.
///
/// Each pair of columns contains every ordered level pair equally often. The
/// result is deterministic and uses zero-based levels for direct mapping onto
/// typed factor levels.
pub fn generate(array: OrthogonalArray, columns: usize) -> Result<Vec<Vec<u8>>> {
    if columns == 0 || columns > array.max_columns() {
        bail!(
            "{} supports between 1 and {} columns",
            array_name(array),
            array.max_columns()
        );
    }

    let levels = array.levels() as usize;
    let rows = array.rows();
    let dimensions = 2;
    let vectors = canonical_vectors(levels, dimensions, array.max_columns());
    let mut design = Vec::with_capacity(rows);

    for row_index in 0..rows {
        let coordinates = decode_base(row_index, levels, dimensions);
        let mut row = Vec::with_capacity(columns);
        for vector in vectors.iter().take(columns) {
            let value = vector
                .iter()
                .zip(coordinates.iter())
                .map(|(weight, coordinate)| weight * coordinate)
                .sum::<usize>()
                % levels;
            row.push(value as u8);
        }
        design.push(row);
    }
    Ok(design)
}

fn array_name(array: OrthogonalArray) -> &'static str {
    match array {
        OrthogonalArray::L9 => "L9",
        OrthogonalArray::L25 => "L25",
    }
}

fn decode_base(mut value: usize, base: usize, dimensions: usize) -> Vec<usize> {
    let mut coordinates = vec![0; dimensions];
    for coordinate in coordinates.iter_mut().rev() {
        *coordinate = value % base;
        value /= base;
    }
    coordinates
}

/// Return canonical non-zero vectors in a finite prime field. The first
/// non-zero component is normalized to one, so vectors differing only by a
/// non-zero scalar are represented once. Unit vectors are emitted first to
/// preserve the stable factor-column ordering used by the upstream generator.
fn canonical_vectors(base: usize, dimensions: usize, max_columns: usize) -> Vec<Vec<usize>> {
    let mut vectors = Vec::with_capacity(max_columns);
    for unit in 0..dimensions {
        let mut vector = vec![0; dimensions];
        vector[unit] = 1;
        vectors.push(vector);
    }

    for encoded in 1..base.pow(dimensions as u32) {
        let vector = decode_base(encoded, base, dimensions);
        let Some(first_nonzero) = vector.iter().find(|value| **value != 0) else {
            continue;
        };
        if *first_nonzero != 1 || vector.iter().filter(|value| **value != 0).count() <= 1 {
            continue;
        }
        vectors.push(vector);
        if vectors.len() == max_columns {
            break;
        }
    }
    vectors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn l9_matches_bounded_shape_and_level_balance() {
        let design = generate(OrthogonalArray::L9, 4).expect("L9");
        assert_eq!(design.len(), 9);
        assert!(design.iter().all(|row| row.len() == 4));
        assert!(design.iter().flatten().all(|level| *level < 3));
        assert_pairwise_balance(&design, 3);
    }

    #[test]
    fn l25_matches_bounded_shape_and_level_balance() {
        let design = generate(OrthogonalArray::L25, 6).expect("L25");
        assert_eq!(design.len(), 25);
        assert!(design.iter().all(|row| row.len() == 6));
        assert!(design.iter().flatten().all(|level| *level < 5));
        assert_pairwise_balance(&design, 5);
    }

    #[test]
    fn column_bounds_fail_closed() {
        assert!(generate(OrthogonalArray::L9, 0).is_err());
        assert!(generate(OrthogonalArray::L25, 7).is_err());
    }

    fn assert_pairwise_balance(design: &[Vec<u8>], levels: u8) {
        let expected = design.len() / usize::from(levels * levels);
        for left in 0..design[0].len() {
            for right in (left + 1)..design[0].len() {
                let mut counts = BTreeMap::new();
                for row in design {
                    *counts.entry((row[left], row[right])).or_insert(0usize) += 1;
                }
                assert_eq!(counts.len(), usize::from(levels * levels));
                assert!(counts.values().all(|count| *count == expected));
            }
        }
    }
}
