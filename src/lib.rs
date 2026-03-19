pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
pub mod core;
pub mod solver;
pub mod engine;
pub mod event;
pub mod discrete;
pub mod algebra;
pub mod math;
pub mod coordinate;
pub mod unit;
pub mod database;
pub mod modules;
pub mod physics;
pub mod coupling;
pub mod visualization;
pub mod platform;
pub mod scripting;
pub mod ext;
pub mod utils;
