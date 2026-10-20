use super::oldest_cell;

#[test]
fn ties_evict_the_lowest_index() {
    assert_eq!(oldest_cell(&[3, 7, 7, 1]), 1);
    assert_eq!(oldest_cell(&[5, 5, 5]), 0);
}

#[test]
fn a_single_cell_is_its_own_victim() {
    assert_eq!(oldest_cell(&[0]), 0);
}

/// `jle` is a signed compare: an age that wrapped past `i32::MAX` is negative and therefore
/// the least recent, even though its unsigned value is the largest.
#[test]
fn ages_compare_as_signed_32_bit_values_after_wrapping() {
    let wrapped = i32::MAX as u32 + 1;
    assert_eq!(oldest_cell(&[i32::MAX as u32, wrapped]), 0);
    assert_eq!(oldest_cell(&[wrapped, 0]), 1);
    assert_eq!(oldest_cell(&[u32::MAX, 0]), 1);
}
