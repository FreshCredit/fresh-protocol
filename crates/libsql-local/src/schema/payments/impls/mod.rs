// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
mod core;
mod helpers;
#[cfg(test)]
mod tests;

pub use core::initialize_payment_tables;
