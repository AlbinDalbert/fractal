#[cfg(test)]
mod tests {
    use std::string::ParseError;

    use mark_mrk::{MarkMrk, IntermediateRep};
    // A simple valid test case
    #[test]
    fn test_parse_mark_valid_input() {
        let input = "# Header\nThis is a paragraph.\n* List item 1\n* List item 2";
        let result = MarkMrk::parse_mark(input);

        match result {
            Ok(ir) => {
                assert_eq!(ir.elements.len(), 4);  // Check if the correct number of elements were parsed
                assert_eq!(ir.count, 5);    // Check the number of items in all the elements
            }
            Err(e) => panic!("Expected Ok, but got error: {:?}", e),
        }
    }

    // Test case with an empty input (edge case)
    #[test]
    fn test_parse_mark_empty_input() {
        let input = "";
        let result = MarkMrk::parse_mark(input);

        match result {
            Ok(ir) => {
                assert_eq!(ir.elements.len(), 0);  // No elements should be parsed from empty input
                assert_eq!(ir.count, 0);    // No elements in total
            }
            Err(e) => panic!("Expected Ok, but got error: {:?}", e),
        }
    }

    // Test case with multiple empty lines (edge case)
    #[test]
    fn test_parse_mark_multiple_empty_lines() {
        let input = "\n\n\n";
        let result = MarkMrk::parse_mark(input);

        match result {
            Ok(ir) => {
                assert_eq!(ir.elements.len(), 0);  // Should not parse any elements
                assert_eq!(ir.count, 0);    // No elements parsed
            }
            Err(e) => panic!("Expected Ok, but got error: {:?}", e),
        }
    }

    // Test case where the input contains a valid chunk and an invalid chunk
    #[test]
    fn test_parse_mark_mixed_valid_invalid() {
        let input = "# Header\nThis is a valid paragraph.\n**Invalid line";
        let result = MarkMrk::parse_mark(input);

        match result {
            Ok(_) => panic!("Expected error, but got Ok!"),
            Err(_) => {
                assert!(false);  // Make sure the error is as expected
            }
        }
    }
}