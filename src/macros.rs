/// Creates a new world.
///
/// See [`World::new`] for more information.
///
/// # Syntax
///
/// ```ignored
/// $io_handler, $($items_per_chunk | $dimension_value_range),+
/// ```
///
/// # Example
///
/// ```
/// # use dmds::{*, mem_io_handle::*};
/// let world: World<[u64; 2], 2, _> = world! {
///     MemStorage::new(), 16 | ..1024, 8 | ..128
/// };
/// # let _ = world;
/// ```
///
/// # Panics
///
/// Panics when count of given dimensions and the
/// dimension count of data are different.
#[macro_export]
macro_rules! world {
    ($io:expr, $($ipc:literal | $dr:expr),+) => {
        $crate::World::new([$($crate::Dim{range:$dr,items_per_chunk:$ipc},)+], $io)
    };
}