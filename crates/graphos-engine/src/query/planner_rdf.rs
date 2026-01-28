//! RDF Query Planner.
//!
//! Converts logical plans with RDF operators (TripleScan, etc.) to physical
//! operators that execute against an RDF store.
//!
//! This planner follows the same push-based, vectorized execution model as
//! the LPG planner for consistent performance characteristics.

use std::collections::HashMap;
use std::sync::Arc;

use graphos_common::types::{LogicalType, Value};
use graphos_common::utils::error::{Error, Result};
use graphos_core::execution::operators::{
    BinaryFilterOp, FilterExpression, FilterOperator, HashAggregateOperator, LimitOperator,
    NestedLoopJoinOperator, Operator, OperatorError, Predicate, SimpleAggregateOperator,
    SkipOperator, SortOperator, UnaryFilterOp,
};
use graphos_core::execution::DataChunk;
use graphos_core::execution::operators::JoinType;
use graphos_core::graph::rdf::{Literal, RdfStore, Term, Triple, TriplePattern};

use crate::query::plan::{
    AggregateFunction as LogicalAggregateFunction, AggregateOp, FilterOp, LimitOp,
    LogicalExpression, LogicalOperator, LogicalPlan, SkipOp, SortOp, TripleComponent, TripleScanOp,
};
use crate::query::planner::{convert_aggregate_function, convert_filter_expression, PhysicalPlan};

/// Default chunk size for morsel-driven execution.
const DEFAULT_CHUNK_SIZE: usize = 1024;

/// Converts logical plans with RDF operators to physical operators.
///
/// This planner produces push-based operators that process data in chunks
/// (morsels) for cache efficiency and parallelism compatibility.
pub struct RdfPlanner {
    /// The RDF store to query.
    store: Arc<RdfStore>,
    /// Chunk size for vectorized execution.
    chunk_size: usize,
}

impl RdfPlanner {
    /// Creates a new RDF planner with the given store.
    #[must_use]
    pub fn new(store: Arc<RdfStore>) -> Self {
        Self {
            store,
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    /// Sets the chunk size for vectorized execution.
    #[must_use]
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    /// Plans a logical plan into a physical operator tree.
    ///
    /// # Errors
    ///
    /// Returns an error if planning fails.
    pub fn plan(&self, logical_plan: &LogicalPlan) -> Result<PhysicalPlan> {
        let (operator, columns) = self.plan_operator(&logical_plan.root)?;
        Ok(PhysicalPlan { operator, columns })
    }

    /// Plans a single logical operator.
    fn plan_operator(&self, op: &LogicalOperator) -> Result<(Box<dyn Operator>, Vec<String>)> {
        match op {
            LogicalOperator::TripleScan(scan) => self.plan_triple_scan(scan),
            LogicalOperator::Filter(filter) => self.plan_filter(filter),
            LogicalOperator::Project(project) => self.plan_operator(&project.input),
            LogicalOperator::Limit(limit) => self.plan_limit(limit),
            LogicalOperator::Skip(skip) => self.plan_skip(skip),
            LogicalOperator::Sort(sort) => self.plan_sort(sort),
            LogicalOperator::Aggregate(agg) => self.plan_aggregate(agg),
            LogicalOperator::Return(ret) => self.plan_return(ret),
            LogicalOperator::Join(join) => self.plan_join(join),
            LogicalOperator::Union(union) => self.plan_union(union),
            LogicalOperator::Distinct(distinct) => self.plan_operator(&distinct.input),
            LogicalOperator::Empty => Err(Error::Internal("Empty plan".to_string())),
            _ => Err(Error::Internal(format!(
                "Unsupported RDF operator: {:?}",
                std::mem::discriminant(op)
            ))),
        }
    }

    /// Plans a triple scan operator.
    ///
    /// Creates a lazy scanning operator that reads triples in chunks
    /// for cache-efficient, vectorized processing.
    fn plan_triple_scan(&self, scan: &TripleScanOp) -> Result<(Box<dyn Operator>, Vec<String>)> {
        // Build the triple pattern for querying the store
        let pattern = self.build_triple_pattern(scan);

        // Determine which columns are variables (and thus in output)
        let mut columns = Vec::new();
        let mut output_mask = [false, false, false, false]; // s, p, o, g

        if let TripleComponent::Variable(name) = &scan.subject {
            columns.push(name.clone());
            output_mask[0] = true;
        }
        if let TripleComponent::Variable(name) = &scan.predicate {
            columns.push(name.clone());
            output_mask[1] = true;
        }
        if let TripleComponent::Variable(name) = &scan.object {
            columns.push(name.clone());
            output_mask[2] = true;
        }
        if let Some(TripleComponent::Variable(name)) = &scan.graph {
            columns.push(name.clone());
            output_mask[3] = true;
        }

        // Create the lazy scanning operator
        let operator = Box::new(RdfTripleScanOperator::new(
            Arc::clone(&self.store),
            pattern,
            output_mask,
            self.chunk_size,
        ));

        Ok((operator, columns))
    }

    /// Builds a TriplePattern from a TripleScanOp.
    fn build_triple_pattern(&self, scan: &TripleScanOp) -> TriplePattern {
        TriplePattern {
            subject: component_to_term(&scan.subject),
            predicate: component_to_term(&scan.predicate),
            object: component_to_term(&scan.object),
        }
    }

    /// Plans a RETURN clause.
    fn plan_return(
        &self,
        ret: &crate::query::plan::ReturnOp,
    ) -> Result<(Box<dyn Operator>, Vec<String>)> {
        let (input_op, _input_columns) = self.plan_operator(&ret.input)?;

        // Extract output column names
        let columns: Vec<String> = ret
            .items
            .iter()
            .map(|item| {
                item.alias
                    .clone()
                    .unwrap_or_else(|| expression_to_string(&item.expression))
            })
            .collect();

        Ok((input_op, columns))
    }

    /// Plans a filter operator.
    fn plan_filter(&self, filter: &FilterOp) -> Result<(Box<dyn Operator>, Vec<String>)> {
        let (input_op, columns) = self.plan_operator(&filter.input)?;

        // Build variable to column index mapping
        let variable_columns: HashMap<String, usize> = columns
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), i))
            .collect();

        // Convert logical expression to filter expression
        let filter_expr = convert_filter_expression(&filter.predicate)?;

        // Create RDF-specific predicate (doesn't need LpgStore)
        let predicate = RdfExpressionPredicate::new(filter_expr, variable_columns);

        let operator = Box::new(FilterOperator::new(input_op, Box::new(predicate)));
        Ok((operator, columns))
    }

    /// Plans a LIMIT operator.
    fn plan_limit(&self, limit: &LimitOp) -> Result<(Box<dyn Operator>, Vec<String>)> {
        let (input_op, columns) = self.plan_operator(&limit.input)?;
        let output_schema = derive_rdf_schema(&columns);
        let operator = Box::new(LimitOperator::new(input_op, limit.count, output_schema));
        Ok((operator, columns))
    }

    /// Plans a SKIP operator.
    fn plan_skip(&self, skip: &SkipOp) -> Result<(Box<dyn Operator>, Vec<String>)> {
        let (input_op, columns) = self.plan_operator(&skip.input)?;
        let output_schema = derive_rdf_schema(&columns);
        let operator = Box::new(SkipOperator::new(input_op, skip.count, output_schema));
        Ok((operator, columns))
    }

    /// Plans a SORT operator.
    fn plan_sort(&self, sort: &SortOp) -> Result<(Box<dyn Operator>, Vec<String>)> {
        use crate::query::plan::SortOrder;
        use graphos_core::execution::operators::{NullOrder, SortDirection, SortKey};

        let (input_op, columns) = self.plan_operator(&sort.input)?;

        let variable_columns: HashMap<String, usize> = columns
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), i))
            .collect();

        let physical_keys: Vec<SortKey> = sort
            .keys
            .iter()
            .map(|key| {
                let col_idx = resolve_expression(&key.expression, &variable_columns)?;
                Ok(SortKey {
                    column: col_idx,
                    direction: match key.order {
                        SortOrder::Ascending => SortDirection::Ascending,
                        SortOrder::Descending => SortDirection::Descending,
                    },
                    null_order: NullOrder::NullsLast,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let output_schema = derive_rdf_schema(&columns);
        let operator = Box::new(SortOperator::new(input_op, physical_keys, output_schema));
        Ok((operator, columns))
    }

    /// Plans an AGGREGATE operator.
    fn plan_aggregate(&self, agg: &AggregateOp) -> Result<(Box<dyn Operator>, Vec<String>)> {
        use graphos_core::execution::operators::AggregateExpr as PhysicalAggregateExpr;

        let (input_op, input_columns) = self.plan_operator(&agg.input)?;

        let variable_columns: HashMap<String, usize> = input_columns
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), i))
            .collect();

        let group_columns: Vec<usize> = agg
            .group_by
            .iter()
            .map(|expr| resolve_expression(expr, &variable_columns))
            .collect::<Result<Vec<_>>>()?;

        let physical_aggregates: Vec<PhysicalAggregateExpr> = agg
            .aggregates
            .iter()
            .map(|agg_expr| {
                let column = agg_expr
                    .expression
                    .as_ref()
                    .map(|e| resolve_expression(e, &variable_columns))
                    .transpose()?;

                Ok(PhysicalAggregateExpr {
                    function: convert_aggregate_function(agg_expr.function),
                    column,
                    distinct: agg_expr.distinct,
                    alias: agg_expr.alias.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let mut output_schema = Vec::new();
        let mut output_columns = Vec::new();

        for expr in &agg.group_by {
            output_schema.push(LogicalType::String);
            output_columns.push(expression_to_string(expr));
        }

        for agg_expr in &agg.aggregates {
            let result_type = match agg_expr.function {
                LogicalAggregateFunction::Count => LogicalType::Int64,
                LogicalAggregateFunction::Sum => LogicalType::Int64,
                LogicalAggregateFunction::Avg => LogicalType::Float64,
                _ => LogicalType::String,
            };
            output_schema.push(result_type);
            output_columns.push(
                agg_expr
                    .alias
                    .clone()
                    .unwrap_or_else(|| format!("{:?}(...)", agg_expr.function).to_lowercase()),
            );
        }

        let operator: Box<dyn Operator> = if group_columns.is_empty() {
            Box::new(SimpleAggregateOperator::new(
                input_op,
                physical_aggregates,
                output_schema,
            ))
        } else {
            Box::new(HashAggregateOperator::new(
                input_op,
                group_columns,
                physical_aggregates,
                output_schema,
            ))
        };

        Ok((operator, output_columns))
    }

    /// Plans a JOIN operator.
    fn plan_join(
        &self,
        join: &crate::query::plan::JoinOp,
    ) -> Result<(Box<dyn Operator>, Vec<String>)> {
        let (left_op, left_columns) = self.plan_operator(&join.left)?;
        let (right_op, right_columns) = self.plan_operator(&join.right)?;

        let mut columns = left_columns;
        columns.extend(right_columns);

        let output_schema = derive_rdf_schema(&columns);
        let operator = Box::new(NestedLoopJoinOperator::new(
            left_op,
            right_op,
            None, // No join condition
            JoinType::Cross,
            output_schema,
        ));

        Ok((operator, columns))
    }

    /// Plans a UNION operator.
    fn plan_union(
        &self,
        union: &crate::query::plan::UnionOp,
    ) -> Result<(Box<dyn Operator>, Vec<String>)> {
        if union.inputs.is_empty() {
            return Err(Error::Internal("Empty UNION".to_string()));
        }

        let (first_op, columns) = self.plan_operator(&union.inputs[0])?;

        if union.inputs.len() == 1 {
            return Ok((first_op, columns));
        }

        Err(Error::Internal(
            "UNION with multiple inputs not yet implemented".to_string(),
        ))
    }
}

// ============================================================================
// RDF Triple Scan Operator
// ============================================================================

/// Lazy triple scan operator that processes triples in chunks.
///
/// This operator queries the RDF store and emits results in DataChunks
/// for efficient vectorized processing.
struct RdfTripleScanOperator {
    /// The RDF store to scan.
    store: Arc<RdfStore>,
    /// The pattern to match.
    pattern: TriplePattern,
    /// Which components to include in output [s, p, o, g].
    output_mask: [bool; 4],
    /// Chunk size for batching.
    chunk_size: usize,
    /// Cached matching triples (lazily populated).
    triples: Option<Vec<Arc<Triple>>>,
    /// Current position in the triples.
    position: usize,
}

impl RdfTripleScanOperator {
    fn new(
        store: Arc<RdfStore>,
        pattern: TriplePattern,
        output_mask: [bool; 4],
        chunk_size: usize,
    ) -> Self {
        Self {
            store,
            pattern,
            output_mask,
            chunk_size,
            triples: None,
            position: 0,
        }
    }

    /// Lazily load matching triples on first access.
    fn ensure_triples(&mut self) {
        if self.triples.is_none() {
            self.triples = Some(self.store.find(&self.pattern));
        }
    }

    /// Count how many output columns we have.
    fn output_column_count(&self) -> usize {
        self.output_mask.iter().filter(|&&b| b).count()
    }
}

impl Operator for RdfTripleScanOperator {
    fn next(&mut self) -> std::result::Result<Option<DataChunk>, OperatorError> {
        self.ensure_triples();

        let triples = self.triples.as_ref().unwrap();

        if self.position >= triples.len() {
            return Ok(None);
        }

        let end = (self.position + self.chunk_size).min(triples.len());
        let batch_size = end - self.position;
        let col_count = self.output_column_count();

        // Create output schema (all String for RDF)
        let schema: Vec<LogicalType> = (0..col_count).map(|_| LogicalType::String).collect();
        let mut chunk = DataChunk::with_capacity(&schema, batch_size);

        // Fill the chunk
        for i in self.position..end {
            let triple = &triples[i];
            let mut col_idx = 0;

            if self.output_mask[0] {
                // Subject
                if let Some(col) = chunk.column_mut(col_idx) {
                    col.push_string(term_to_string(triple.subject()));
                }
                col_idx += 1;
            }
            if self.output_mask[1] {
                // Predicate
                if let Some(col) = chunk.column_mut(col_idx) {
                    col.push_string(term_to_string(triple.predicate()));
                }
                col_idx += 1;
            }
            if self.output_mask[2] {
                // Object
                if let Some(col) = chunk.column_mut(col_idx) {
                    push_term_value(col, triple.object());
                }
                col_idx += 1;
            }
            if self.output_mask[3] {
                // Graph (always null for now - named graphs not yet supported)
                if let Some(col) = chunk.column_mut(col_idx) {
                    col.push_value(Value::Null);
                }
            }
        }

        chunk.set_count(batch_size);
        self.position = end;

        Ok(Some(chunk))
    }

    fn reset(&mut self) {
        self.position = 0;
        // Keep triples cached for re-execution
    }

    fn name(&self) -> &'static str {
        "RdfTripleScan"
    }
}

// ============================================================================
// RDF Expression Predicate
// ============================================================================

/// Expression predicate for RDF queries.
///
/// Unlike the LPG predicate, this doesn't need a store reference because
/// RDF values are already materialized in the DataChunk columns.
struct RdfExpressionPredicate {
    expression: FilterExpression,
    variable_columns: HashMap<String, usize>,
}

impl RdfExpressionPredicate {
    fn new(expression: FilterExpression, variable_columns: HashMap<String, usize>) -> Self {
        Self {
            expression,
            variable_columns,
        }
    }

    fn eval(&self, chunk: &DataChunk, row: usize) -> Option<Value> {
        self.eval_expr(&self.expression, chunk, row)
    }

    fn eval_expr(&self, expr: &FilterExpression, chunk: &DataChunk, row: usize) -> Option<Value> {
        match expr {
            FilterExpression::Literal(v) => Some(v.clone()),
            FilterExpression::Variable(name) => {
                let col_idx = *self.variable_columns.get(name)?;
                chunk.column(col_idx)?.get_value(row)
            }
            FilterExpression::Property { variable, .. } => {
                // For RDF, treat property access as variable access
                let col_idx = *self.variable_columns.get(variable)?;
                chunk.column(col_idx)?.get_value(row)
            }
            FilterExpression::Binary { left, op, right } => {
                let left_val = self.eval_expr(left, chunk, row)?;
                let right_val = self.eval_expr(right, chunk, row)?;
                self.eval_binary_op(&left_val, *op, &right_val)
            }
            FilterExpression::Unary { op, operand } => {
                let val = self.eval_expr(operand, chunk, row);
                self.eval_unary_op(*op, val)
            }
            FilterExpression::Id(var) | FilterExpression::Labels(var) | FilterExpression::Type(var) => {
                // Treat Id/Labels/Type access as variable lookup for RDF
                let col_idx = *self.variable_columns.get(var)?;
                chunk.column(col_idx)?.get_value(row)
            }
            // These expression types are not commonly used in RDF FILTER clauses
            FilterExpression::FunctionCall { .. }
            | FilterExpression::List(_)
            | FilterExpression::Case { .. }
            | FilterExpression::Map(_)
            | FilterExpression::IndexAccess { .. }
            | FilterExpression::SliceAccess { .. }
            | FilterExpression::ListComprehension { .. }
            | FilterExpression::ExistsSubquery { .. } => None,
        }
    }

    fn eval_binary_op(&self, left: &Value, op: BinaryFilterOp, right: &Value) -> Option<Value> {
        match op {
            BinaryFilterOp::And => Some(Value::Bool(left.as_bool()? && right.as_bool()?)),
            BinaryFilterOp::Or => Some(Value::Bool(left.as_bool()? || right.as_bool()?)),
            BinaryFilterOp::Xor => {
                Some(Value::Bool(left.as_bool()? != right.as_bool()?))
            }
            BinaryFilterOp::Eq => Some(Value::Bool(left == right)),
            BinaryFilterOp::Ne => Some(Value::Bool(left != right)),
            BinaryFilterOp::Lt => compare_values(left, right, |o| o.is_lt()),
            BinaryFilterOp::Le => compare_values(left, right, |o| o.is_le()),
            BinaryFilterOp::Gt => compare_values(left, right, |o| o.is_gt()),
            BinaryFilterOp::Ge => compare_values(left, right, |o| o.is_ge()),
            BinaryFilterOp::Add => {
                match (left, right) {
                    (Value::Int64(l), Value::Int64(r)) => Some(Value::Int64(l + r)),
                    (Value::Float64(l), Value::Float64(r)) => Some(Value::Float64(l + r)),
                    (Value::Int64(l), Value::Float64(r)) => Some(Value::Float64(*l as f64 + r)),
                    (Value::Float64(l), Value::Int64(r)) => Some(Value::Float64(l + *r as f64)),
                    _ => None,
                }
            }
            BinaryFilterOp::Sub => {
                match (left, right) {
                    (Value::Int64(l), Value::Int64(r)) => Some(Value::Int64(l - r)),
                    (Value::Float64(l), Value::Float64(r)) => Some(Value::Float64(l - r)),
                    (Value::Int64(l), Value::Float64(r)) => Some(Value::Float64(*l as f64 - r)),
                    (Value::Float64(l), Value::Int64(r)) => Some(Value::Float64(l - *r as f64)),
                    _ => None,
                }
            }
            BinaryFilterOp::Mul => {
                match (left, right) {
                    (Value::Int64(l), Value::Int64(r)) => Some(Value::Int64(l * r)),
                    (Value::Float64(l), Value::Float64(r)) => Some(Value::Float64(l * r)),
                    (Value::Int64(l), Value::Float64(r)) => Some(Value::Float64(*l as f64 * r)),
                    (Value::Float64(l), Value::Int64(r)) => Some(Value::Float64(l * *r as f64)),
                    _ => None,
                }
            }
            BinaryFilterOp::Div => {
                match (left, right) {
                    (Value::Int64(l), Value::Int64(r)) if *r != 0 => Some(Value::Int64(l / r)),
                    (Value::Float64(l), Value::Float64(r)) if *r != 0.0 => {
                        Some(Value::Float64(l / r))
                    }
                    (Value::Int64(l), Value::Float64(r)) if *r != 0.0 => {
                        Some(Value::Float64(*l as f64 / r))
                    }
                    (Value::Float64(l), Value::Int64(r)) if *r != 0 => {
                        Some(Value::Float64(l / *r as f64))
                    }
                    _ => None,
                }
            }
            BinaryFilterOp::Mod => {
                match (left, right) {
                    (Value::Int64(l), Value::Int64(r)) if *r != 0 => Some(Value::Int64(l % r)),
                    _ => None,
                }
            }
            BinaryFilterOp::Contains => {
                match (left, right) {
                    (Value::String(l), Value::String(r)) => Some(Value::Bool(l.contains(&**r))),
                    _ => None,
                }
            }
            BinaryFilterOp::StartsWith => {
                match (left, right) {
                    (Value::String(l), Value::String(r)) => {
                        Some(Value::Bool(l.starts_with(&**r)))
                    }
                    _ => None,
                }
            }
            BinaryFilterOp::EndsWith => {
                match (left, right) {
                    (Value::String(l), Value::String(r)) => {
                        Some(Value::Bool(l.ends_with(&**r)))
                    }
                    _ => None,
                }
            }
            BinaryFilterOp::In => {
                // Not implemented for RDF filter evaluation
                None
            }
            BinaryFilterOp::Regex => {
                // Regex matching - not yet implemented for RDF
                // Would need regex crate for full support
                None
            }
            BinaryFilterOp::Pow => {
                // Power operation
                match (left, right) {
                    (Value::Int64(base), Value::Int64(exp)) => {
                        Some(Value::Float64((*base as f64).powf(*exp as f64)))
                    }
                    (Value::Float64(base), Value::Float64(exp)) => {
                        Some(Value::Float64(base.powf(*exp)))
                    }
                    (Value::Int64(base), Value::Float64(exp)) => {
                        Some(Value::Float64((*base as f64).powf(*exp)))
                    }
                    (Value::Float64(base), Value::Int64(exp)) => {
                        Some(Value::Float64(base.powf(*exp as f64)))
                    }
                    _ => None,
                }
            }
        }
    }

    fn eval_unary_op(&self, op: UnaryFilterOp, val: Option<Value>) -> Option<Value> {
        match op {
            UnaryFilterOp::Not => Some(Value::Bool(!val?.as_bool()?)),
            UnaryFilterOp::IsNull => Some(Value::Bool(val.is_none())),
            UnaryFilterOp::IsNotNull => Some(Value::Bool(val.is_some())),
            UnaryFilterOp::Neg => {
                match val? {
                    Value::Int64(v) => Some(Value::Int64(-v)),
                    Value::Float64(v) => Some(Value::Float64(-v)),
                    _ => None,
                }
            }
        }
    }
}

impl Predicate for RdfExpressionPredicate {
    fn evaluate(&self, chunk: &DataChunk, row: usize) -> bool {
        matches!(self.eval(chunk, row), Some(Value::Bool(true)))
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Converts an RDF Term to a string for IRI/blank node representation.
fn term_to_string(term: &Term) -> String {
    match term {
        Term::Iri(iri) => iri.as_str().to_string(),
        Term::BlankNode(bnode) => format!("_:{}", bnode.id()),
        Term::Literal(lit) => lit.value().to_string(),
    }
}

/// Pushes an RDF term value to a column, preserving type where possible.
fn push_term_value(col: &mut graphos_core::execution::ValueVector, term: &Term) {
    match term {
        Term::Iri(iri) => col.push_string(iri.as_str().to_string()),
        Term::BlankNode(bnode) => col.push_string(format!("_:{}", bnode.id())),
        Term::Literal(lit) => {
            // Try to preserve typed literals
            let datatype = lit.datatype();
            match datatype {
                Literal::XSD_INTEGER | "http://www.w3.org/2001/XMLSchema#int" => {
                    if let Ok(i) = lit.value().parse::<i64>() {
                        col.push_int64(i);
                        return;
                    }
                }
                Literal::XSD_DOUBLE | "http://www.w3.org/2001/XMLSchema#float" => {
                    if let Ok(f) = lit.value().parse::<f64>() {
                        col.push_float64(f);
                        return;
                    }
                }
                Literal::XSD_BOOLEAN => {
                    if let Ok(b) = lit.value().parse::<bool>() {
                        col.push_bool(b);
                        return;
                    }
                }
                _ => {}
            }
            col.push_string(lit.value().to_string());
        }
    }
}

/// Converts a TripleComponent to an Option<Term> for pattern matching.
fn component_to_term(component: &TripleComponent) -> Option<Term> {
    match component {
        TripleComponent::Variable(_) => None,
        TripleComponent::Iri(iri) => Some(Term::iri(iri.clone())),
        TripleComponent::Literal(value) => match value {
            Value::String(s) => Some(Term::literal(Arc::clone(s))),
            Value::Int64(i) => Some(Term::typed_literal(
                i.to_string(),
                Literal::XSD_INTEGER,
            )),
            Value::Float64(f) => Some(Term::typed_literal(
                f.to_string(),
                Literal::XSD_DOUBLE,
            )),
            Value::Bool(b) => Some(Term::typed_literal(
                b.to_string(),
                Literal::XSD_BOOLEAN,
            )),
            _ => Some(Term::literal(value.to_string())),
        },
    }
}

/// Derives RDF schema (all String type for simplicity).
fn derive_rdf_schema(columns: &[String]) -> Vec<LogicalType> {
    columns.iter().map(|_| LogicalType::String).collect()
}

/// Resolves an expression to a column index.
fn resolve_expression(
    expr: &LogicalExpression,
    variable_columns: &HashMap<String, usize>,
) -> Result<usize> {
    match expr {
        LogicalExpression::Variable(name) => variable_columns
            .get(name)
            .copied()
            .ok_or_else(|| Error::Internal(format!("Variable '{}' not found", name))),
        _ => Err(Error::Internal(format!(
            "Cannot resolve expression to column: {:?}",
            expr
        ))),
    }
}

/// Converts an expression to a string for column naming.
fn expression_to_string(expr: &LogicalExpression) -> String {
    match expr {
        LogicalExpression::Variable(name) => name.clone(),
        LogicalExpression::Property { variable, property } => format!("{variable}.{property}"),
        LogicalExpression::Literal(value) => format!("{value:?}"),
        _ => "expr".to_string(),
    }
}

/// Compares two values and returns a boolean result.
fn compare_values<F>(left: &Value, right: &Value, cmp: F) -> Option<Value>
where
    F: Fn(std::cmp::Ordering) -> bool,
{
    let ordering = match (left, right) {
        (Value::Int64(l), Value::Int64(r)) => l.cmp(r),
        (Value::Float64(l), Value::Float64(r)) => l.partial_cmp(r)?,
        (Value::String(l), Value::String(r)) => l.cmp(r),
        (Value::Int64(l), Value::Float64(r)) => (*l as f64).partial_cmp(r)?,
        (Value::Float64(l), Value::Int64(r)) => l.partial_cmp(&(*r as f64))?,
        _ => return None,
    };
    Some(Value::Bool(cmp(ordering)))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::plan::LogicalPlan;

    #[test]
    fn test_rdf_planner_simple_scan() {
        let store = Arc::new(RdfStore::new());

        store.insert(Triple::new(
            Term::iri("http://example.org/alice"),
            Term::iri("http://xmlns.com/foaf/0.1/name"),
            Term::literal("Alice"),
        ));

        let planner = RdfPlanner::new(store);

        let scan = TripleScanOp {
            subject: TripleComponent::Variable("s".to_string()),
            predicate: TripleComponent::Variable("p".to_string()),
            object: TripleComponent::Variable("o".to_string()),
            graph: None,
            input: None,
        };

        let plan = LogicalPlan::new(LogicalOperator::TripleScan(scan));
        let physical = planner.plan(&plan).unwrap();

        assert_eq!(physical.columns, vec!["s", "p", "o"]);
    }

    #[test]
    fn test_rdf_planner_with_pattern() {
        let store = Arc::new(RdfStore::new());

        store.insert(Triple::new(
            Term::iri("http://example.org/alice"),
            Term::iri("http://xmlns.com/foaf/0.1/name"),
            Term::literal("Alice"),
        ));
        store.insert(Triple::new(
            Term::iri("http://example.org/bob"),
            Term::iri("http://xmlns.com/foaf/0.1/name"),
            Term::literal("Bob"),
        ));
        store.insert(Triple::new(
            Term::iri("http://example.org/alice"),
            Term::iri("http://xmlns.com/foaf/0.1/age"),
            Term::typed_literal("30", "http://www.w3.org/2001/XMLSchema#integer"),
        ));

        let planner = RdfPlanner::new(store);

        let scan = TripleScanOp {
            subject: TripleComponent::Variable("s".to_string()),
            predicate: TripleComponent::Iri("http://xmlns.com/foaf/0.1/name".to_string()),
            object: TripleComponent::Variable("o".to_string()),
            graph: None,
            input: None,
        };

        let plan = LogicalPlan::new(LogicalOperator::TripleScan(scan));
        let physical = planner.plan(&plan).unwrap();

        // Only s and o are variables (predicate is fixed)
        assert_eq!(physical.columns, vec!["s", "o"]);
    }

    #[test]
    fn test_rdf_scan_operator_chunking() {
        let store = Arc::new(RdfStore::new());

        // Insert 100 triples
        for i in 0..100 {
            store.insert(Triple::new(
                Term::iri(format!("http://example.org/item{}", i)),
                Term::iri("http://example.org/value"),
                Term::literal(i.to_string()),
            ));
        }

        let pattern = TriplePattern {
            subject: None,
            predicate: None,
            object: None,
        };

        let mut operator =
            RdfTripleScanOperator::new(Arc::clone(&store), pattern, [true, true, true, false], 30);

        let mut total_rows = 0;
        while let Ok(Some(chunk)) = operator.next() {
            total_rows += chunk.row_count();
            assert!(chunk.row_count() <= 30); // Respects chunk size
        }

        assert_eq!(total_rows, 100);
    }
}
