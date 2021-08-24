pub mod ast;
pub mod callbacks;
pub mod conversion;
pub mod signatures;
pub mod structs;

use combinations::Combinations;

pub use structs::Function;

pub fn all_combos<T>(l: Vec<T>) -> Vec<Vec<T>>
where
    T: std::cmp::Ord + Clone,
{
    let mut result = vec![];
    let length = l.len();

    for i in 1..length {
        let c: Vec<Vec<T>> =
            Combinations::new(l.clone(), i).collect();
        result.extend(c);
    }

    result.push(l);

    result
}

pub fn get_package_name(name: String) -> Option<String> {
    let items = name.split('/');
    items.last().map(|n| n.to_string())
}

#[cfg(test)]
mod tests {
    use super::all_combos;

    #[test]
    fn test_all_combos() {
        let array = vec!["1", "2", "3"];
        let result = all_combos(array);

        assert_eq!(result.len(), 7);
    }
}
