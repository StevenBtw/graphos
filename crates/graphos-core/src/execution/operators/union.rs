//! Union operator for combining multiple result sets.
//!
//! The union operator concatenates results from multiple input operators,
//! producing all rows from each input in sequence.

use graphos_common::types::LogicalType;

use super::{Operator, OperatorResult};

/// Union operator that combines results from multiple inputs.
///
/// This produces all rows from all inputs, in order. It does not
/// remove duplicates (use DISTINCT after UNION for UNION DISTINCT).
pub struct UnionOperator {
    /// Input operators.
    inputs: Vec<Box<dyn Operator>>,
    /// Current input index.
    current_input: usize,
    /// Output schema.
    output_schema: Vec<LogicalType>,
}

impl UnionOperator {
    /// Creates a new union operator.
    ///
    /// # Arguments
    /// * `inputs` - The input operators to union.
    /// * `output_schema` - The schema of the output (should match all inputs).
    pub fn new(inputs: Vec<Box<dyn Operator>>, output_schema: Vec<LogicalType>) -> Self {
        Self {
            inputs,
            current_input: 0,
            output_schema,
        }
    }

    /// Returns the output schema.
    #[must_use]
    pub fn output_schema(&self) -> &[LogicalType] {
        &self.output_schema
    }
}

impl Operator for UnionOperator {
    fn next(&mut self) -> OperatorResult {
        // Process inputs in order
        while self.current_input < self.inputs.len() {
            if let Some(chunk) = self.inputs[self.current_input].next()? {
                return Ok(Some(chunk));
            }
            // Move to next input when current is exhausted
            self.current_input += 1;
        }

        Ok(None)
    }

    fn reset(&mut self) {
        for input in &mut self.inputs {
            input.reset();
        }
        self.current_input = 0;
    }

    fn name(&self) -> &'static str {
        "Union"
    }
}

/// Union all operator that includes duplicates.
///
/// This is the same as UnionOperator - kept for semantic clarity.
#[allow(dead_code)]
pub type UnionAllOperator = UnionOperator;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::chunk::DataChunkBuilder;
    use crate::execution::DataChunk;

    /// Mock operator for testing.
    struct MockOperator {
        chunks: Vec<DataChunk>,
        position: usize,
    }

    impl MockOperator {
        fn new(chunks: Vec<DataChunk>) -> Self {
            Self {
                chunks,
                position: 0,
            }
        }
    }

    impl Operator for MockOperator {
        fn next(&mut self) -> OperatorResult {
            if self.position < self.chunks.len() {
                let chunk =
                    std::mem::replace(&mut self.chunks[self.position], DataChunk::empty());
                self.position += 1;
                Ok(Some(chunk))
            } else {
                Ok(None)
            }
        }

        fn reset(&mut self) {
            self.position = 0;
        }

        fn name(&self) -> &'static str {
            "Mock"
        }
    }

    fn create_int_chunk(values: &[i64]) -> DataChunk {
        let mut builder = DataChunkBuilder::new(&[LogicalType::Int64]);
        for &v in values {
            builder.column_mut(0).unwrap().push_int64(v);
            builder.advance_row();
        }
        builder.finish()
    }

    #[test]
    fn test_union_two_inputs() {
        let input1 = MockOperator::new(vec![create_int_chunk(&[1, 2])]);
        let input2 = MockOperator::new(vec![create_int_chunk(&[3, 4])]);

        let mut union = UnionOperator::new(
            vec![Box::new(input1), Box::new(input2)],
            vec![LogicalType::Int64],
        );

        let mut results = Vec::new();
        while let Some(chunk) = union.next().unwrap() {
            for row in chunk.selected_indices() {
                let val = chunk.column(0).unwrap().get_int64(row).unwrap();
                results.push(val);
            }
        }

        assert_eq!(results, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_union_three_inputs() {
        let input1 = MockOperator::new(vec![create_int_chunk(&[1])]);
        let input2 = MockOperator::new(vec![create_int_chunk(&[2])]);
        let input3 = MockOperator::new(vec![create_int_chunk(&[3])]);

        let mut union = UnionOperator::new(
            vec![Box::new(input1), Box::new(input2), Box::new(input3)],
            vec![LogicalType::Int64],
        );

        let mut results = Vec::new();
        while let Some(chunk) = union.next().unwrap() {
            for row in chunk.selected_indices() {
                let val = chunk.column(0).unwrap().get_int64(row).unwrap();
                results.push(val);
            }
        }

        assert_eq!(results, vec![1, 2, 3]);
    }

    #[test]
    fn test_union_empty_input() {
        let input1 = MockOperator::new(vec![create_int_chunk(&[1, 2])]);
        let input2 = MockOperator::new(vec![]); // Empty
        let input3 = MockOperator::new(vec![create_int_chunk(&[3])]);

        let mut union = UnionOperator::new(
            vec![Box::new(input1), Box::new(input2), Box::new(input3)],
            vec![LogicalType::Int64],
        );

        let mut results = Vec::new();
        while let Some(chunk) = union.next().unwrap() {
            for row in chunk.selected_indices() {
                let val = chunk.column(0).unwrap().get_int64(row).unwrap();
                results.push(val);
            }
        }

        assert_eq!(results, vec![1, 2, 3]);
    }

    #[test]
    fn test_union_reset() {
        let input1 = MockOperator::new(vec![create_int_chunk(&[1])]);
        let input2 = MockOperator::new(vec![create_int_chunk(&[2])]);

        let mut union = UnionOperator::new(
            vec![Box::new(input1), Box::new(input2)],
            vec![LogicalType::Int64],
        );

        // First pass
        let mut count = 0;
        while union.next().unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 2);

        // Reset and second pass
        union.reset();
        count = 0;
        while union.next().unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 2);
    }
}
