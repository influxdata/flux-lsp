extern crate flux_lsp;
extern crate speculate;

use flux_lsp::shared::all_combos;
use speculate::speculate;

speculate! {
    describe "All Combinations" {
        it "returns correct result" {
            let array = vec!["1", "2", "3"];
            let result = all_combos(array);

            assert_eq!(result.len(), 7);
        }
    }
}
