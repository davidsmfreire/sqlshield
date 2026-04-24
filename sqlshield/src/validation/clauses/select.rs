use std::collections::{HashMap, HashSet};

use sqlparser::ast::{
    Expr, FunctionArg, FunctionArgExpr, GroupByExpr, JoinConstraint, JoinOperator, Select,
    SelectItem, TableFactor, TableWithJoins,
};

use crate::{schema, validation::asserts};

use super::ClauseValidation;

/// A table (or CTE-derived relation) visible to the current Select scope.
pub(crate) struct VisibleRelation<'a> {
    /// Last segment of the table name (`users` in `public.users`).
    name: &'a str,
    /// Alias if one was given (`u` in `users u`).
    alias: Option<&'a str>,
}

impl<'a> VisibleRelation<'a> {
    /// The name the caller should use when referring to this relation with
    /// a qualifier (the alias if present, otherwise the name).
    fn qualifier(&self) -> &'a str {
        self.alias.unwrap_or(self.name)
    }

    fn from_factor(factor: &'a TableFactor) -> Option<Self> {
        match factor {
            TableFactor::Table { name, alias, .. } => Some(Self {
                name: name.0.last()?.value.as_str(),
                alias: alias.as_ref().map(|a| a.name.value.as_str()),
            }),
            // A derived table `(SELECT …) alias` — its alias doubles as the
            // relation name. Its projected columns are tracked in `extras`.
            TableFactor::Derived { alias, .. } => {
                let alias_ref = alias.as_ref()?;
                Some(Self {
                    name: alias_ref.name.value.as_str(),
                    alias: None,
                })
            }
            _ => None,
        }
    }
}

pub(crate) fn validate_exprs_in_select_scope(
    exprs: &[&Expr],
    select: &Select,
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
) -> Vec<String> {
    let visible = collect_visible_relations(&select.from);
    let aliases = collect_projection_aliases(select);
    let mut errors = Vec::new();
    for expr in exprs {
        validate_expr_column_refs(expr, &visible, schema, extras, &aliases, &mut errors);
    }
    errors
}

/// Collect the output names of `SELECT expr AS alias` projection items.
/// Referenced by HAVING / GROUP BY / ORDER BY per Postgres/MySQL extensions to
/// standard SQL.
fn collect_projection_aliases(select: &Select) -> HashSet<&str> {
    let mut out = HashSet::new();
    for item in &select.projection {
        if let SelectItem::ExprWithAlias { alias, .. } = item {
            out.insert(alias.value.as_str());
        }
    }
    out
}

/// Validate a single `TableFactor` from a FROM clause: check table
/// existence, and recurse into NestedJoin so ON/USING and inner table
/// existence are also covered.
fn validate_from_factor(
    factor: &TableFactor,
    visible: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
    errors: &mut Vec<String>,
) {
    if let Some(name) = asserts::is_relation_in_schema(factor, schema, extras) {
        errors.push(format!("Table `{name}` not found in schema nor subqueries"));
    }
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = factor
    {
        validate_from_factor(&table_with_joins.relation, visible, schema, extras, errors);
        for join in &table_with_joins.joins {
            validate_from_factor(&join.relation, visible, schema, extras, errors);
            validate_join_op(&join.join_operator, visible, schema, extras, errors);
        }
    }
}

/// Validate a join's ON / USING constraint columns against the outer
/// visible relations.
fn validate_join_op(
    op: &JoinOperator,
    visible: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
    errors: &mut Vec<String>,
) {
    let Some(constraint) = join_constraint(op) else {
        return;
    };
    let no_aliases: HashSet<&str> = HashSet::new();
    match constraint {
        JoinConstraint::On(expr) => {
            validate_expr_column_refs(expr, visible, schema, extras, &no_aliases, errors);
        }
        JoinConstraint::Using(cols) => {
            for col in cols {
                if let Some(err) = resolve_unqualified(col.value.as_str(), visible, schema, extras)
                {
                    errors.push(err);
                }
            }
        }
        JoinConstraint::Natural | JoinConstraint::None => {}
    }
}

/// Borrow the `JoinConstraint` out of any `JoinOperator` that carries one.
/// CrossJoin / CrossApply / OuterApply have no constraint.
fn join_constraint(op: &JoinOperator) -> Option<&JoinConstraint> {
    match op {
        JoinOperator::Inner(c)
        | JoinOperator::LeftOuter(c)
        | JoinOperator::RightOuter(c)
        | JoinOperator::FullOuter(c)
        | JoinOperator::LeftSemi(c)
        | JoinOperator::RightSemi(c)
        | JoinOperator::LeftAnti(c)
        | JoinOperator::RightAnti(c) => Some(c),
        JoinOperator::CrossJoin | JoinOperator::CrossApply | JoinOperator::OuterApply => None,
    }
}

pub(crate) fn collect_visible_relations<'a>(
    tables: &'a [TableWithJoins],
) -> Vec<VisibleRelation<'a>> {
    let mut out = Vec::new();
    for t in tables {
        collect_from_factor(&t.relation, &mut out);
        for j in &t.joins {
            collect_from_factor(&j.relation, &mut out);
        }
    }
    out
}

/// Recurse into `TableFactor::NestedJoin` so the relations inside a
/// parenthesized join group still appear in the visible set.
fn collect_from_factor<'a>(factor: &'a TableFactor, out: &mut Vec<VisibleRelation<'a>>) {
    if let Some(r) = VisibleRelation::from_factor(factor) {
        out.push(r);
        return;
    }
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = factor
    {
        collect_from_factor(&table_with_joins.relation, out);
        for j in &table_with_joins.joins {
            collect_from_factor(&j.relation, out);
        }
    }
}

/// Look up whether `col` exists in the column set for a relation (either in
/// the real schema or CTE-derived extras). Returns `Some(true)` if yes,
/// `Some(false)` if the relation is known but the column isn't, and `None`
/// if the relation is entirely unknown (caller should not over-report).
fn column_in_relation(
    col: &str,
    rel: &VisibleRelation<'_>,
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
) -> Option<bool> {
    if let Some(cols) = schema.get(rel.name) {
        return Some(cols.contains(col));
    }
    if let Some(cols) = extras.get(rel.name) {
        return Some(cols.contains(col));
    }
    None
}

/// Resolve an unqualified column reference against the visible relations.
/// The error message names the specific table(s) the column was missing
/// from when at least one visible relation is known to the schema; if no
/// known relation contains this column, table-not-found errors emitted by
/// the FROM walk already covered the situation, so we stay quiet.
fn resolve_unqualified(
    col: &str,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
) -> Option<String> {
    let mut not_found_in: Vec<&str> = Vec::new();
    for rel in relations {
        match column_in_relation(col, rel, schema, extras) {
            Some(true) => return None,
            Some(false) => not_found_in.push(rel.name),
            None => {}
        }
    }
    if let [table] = not_found_in.as_slice() {
        Some(format!("Column `{col}` not found in table `{table}`"))
    } else if !not_found_in.is_empty() {
        let names = not_found_in.join(",");
        Some(format!(
            "Column `{col}` not found in none of the tables: {names}"
        ))
    } else {
        None
    }
}

fn resolve_qualified(
    qualifier: &str,
    col: &str,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
) -> Option<String> {
    let matched = relations.iter().find(|r| r.qualifier() == qualifier)?;
    match column_in_relation(col, matched, schema, extras) {
        Some(false) => Some(format!(
            "Column `{col}` not found in table `{}`",
            matched.name
        )),
        _ => None,
    }
}

pub(crate) fn validate_expr_column_refs(
    root: &Expr,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
    aliases: &HashSet<&str>,
    errors: &mut Vec<String>,
) {
    walk_expr(root, relations, schema, extras, aliases, errors);
}

/// Manual recursion over `Expr`. Unlike `sqlparser::ast::visit_expressions`,
/// this does NOT descend blindly into subqueries — those are handed off to
/// `validate_query_with_scope` with a fresh scope so their identifiers
/// resolve against their own FROM, not the enclosing one.
fn walk_expr(
    expr: &Expr,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
    aliases: &HashSet<&str>,
    errors: &mut Vec<String>,
) {
    match expr {
        Expr::Identifier(ident) => {
            let col = ident.value.as_str();
            if aliases.contains(col) {
                return;
            }
            if let Some(err) = resolve_unqualified(col, relations, schema, extras) {
                errors.push(err);
            }
        }
        Expr::CompoundIdentifier(idents) if idents.len() == 2 => {
            let qualifier = idents[0].value.as_str();
            let col = idents[1].value.as_str();
            if let Some(err) = resolve_qualified(qualifier, col, relations, schema, extras) {
                errors.push(err);
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            walk_expr(left, relations, schema, extras, aliases, errors);
            walk_expr(right, relations, schema, extras, aliases, errors);
        }
        Expr::UnaryOp { expr, .. }
        | Expr::Nested(expr)
        | Expr::IsNull(expr)
        | Expr::IsNotNull(expr)
        | Expr::IsTrue(expr)
        | Expr::IsFalse(expr)
        | Expr::IsUnknown(expr)
        | Expr::IsNotTrue(expr)
        | Expr::IsNotFalse(expr)
        | Expr::IsNotUnknown(expr)
        | Expr::Cast { expr, .. }
        | Expr::TryCast { expr, .. }
        | Expr::SafeCast { expr, .. }
        | Expr::Collate { expr, .. } => {
            walk_expr(expr, relations, schema, extras, aliases, errors);
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            walk_expr(expr, relations, schema, extras, aliases, errors);
            walk_expr(low, relations, schema, extras, aliases, errors);
            walk_expr(high, relations, schema, extras, aliases, errors);
        }
        Expr::InList { expr, list, .. } => {
            walk_expr(expr, relations, schema, extras, aliases, errors);
            for item in list {
                walk_expr(item, relations, schema, extras, aliases, errors);
            }
        }
        Expr::Like { expr, pattern, .. }
        | Expr::ILike { expr, pattern, .. }
        | Expr::SimilarTo { expr, pattern, .. }
        | Expr::RLike { expr, pattern, .. } => {
            walk_expr(expr, relations, schema, extras, aliases, errors);
            walk_expr(pattern, relations, schema, extras, aliases, errors);
        }
        Expr::Case {
            operand,
            conditions,
            results,
            else_result,
        } => {
            if let Some(op) = operand {
                walk_expr(op, relations, schema, extras, aliases, errors);
            }
            for c in conditions {
                walk_expr(c, relations, schema, extras, aliases, errors);
            }
            for r in results {
                walk_expr(r, relations, schema, extras, aliases, errors);
            }
            if let Some(e) = else_result {
                walk_expr(e, relations, schema, extras, aliases, errors);
            }
        }
        Expr::Function(f) => {
            for arg in &f.args {
                match arg {
                    FunctionArg::Named {
                        arg: FunctionArgExpr::Expr(e),
                        ..
                    }
                    | FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => {
                        walk_expr(e, relations, schema, extras, aliases, errors);
                    }
                    _ => {}
                }
            }
        }
        Expr::AnyOp { left, right, .. } | Expr::AllOp { left, right, .. } => {
            walk_expr(left, relations, schema, extras, aliases, errors);
            walk_expr(right, relations, schema, extras, aliases, errors);
        }
        // Subquery boundaries: hand off to the query validator with a fresh
        // scope. The enclosing `extras` (CTEs, derived tables visible at this
        // level) are threaded down so the inner query can see them.
        Expr::Subquery(q) => {
            errors.extend(crate::validation::validate_query_with_scope(
                q.as_ref(),
                schema,
                extras,
            ));
        }
        Expr::Exists { subquery, .. } => {
            errors.extend(crate::validation::validate_query_with_scope(
                subquery.as_ref(),
                schema,
                extras,
            ));
        }
        Expr::InSubquery { expr, subquery, .. } => {
            walk_expr(expr, relations, schema, extras, aliases, errors);
            errors.extend(crate::validation::validate_query_with_scope(
                subquery.as_ref(),
                schema,
                extras,
            ));
        }
        // Leave literals, wildcards, typed strings, and uncommon variants
        // (Substring, Trim, Extract, Overlay, …) alone. Missing coverage here
        // is a false negative, not a false positive — safer default.
        _ => {}
    }
}

impl ClauseValidation for Select {
    fn validate(
        &self,
        schema: &schema::TablesAndColumns,
        extras: &HashMap<&str, HashSet<&str>>,
    ) -> Vec<String> {
        let select = self;
        let mut errors = vec![];

        // Visible relations are needed both for FROM/JOIN constraint walks
        // and for WHERE/HAVING/GROUP BY below, so build them once up front.
        let visible = collect_visible_relations(&select.from);
        let no_aliases: HashSet<&str> = HashSet::new();

        for item in &select.from {
            validate_from_factor(&item.relation, &visible, schema, extras, &mut errors);
            for join in &item.joins {
                validate_from_factor(&join.relation, &visible, schema, extras, &mut errors);
                validate_join_op(&join.join_operator, &visible, schema, extras, &mut errors);
            }
        }

        // Walk every projection expression with the same scope-aware visitor
        // used by WHERE/HAVING/etc. — catches column refs inside function
        // calls, CASE branches, CAST, arithmetic, and nested expressions
        // that the old `direct_col_ref` shortcut silently skipped.
        // Wildcard / QualifiedWildcard items have no column to validate.
        for item in &select.projection {
            let expr = match item {
                SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => e,
                _ => continue,
            };
            validate_expr_column_refs(expr, &visible, schema, extras, &no_aliases, &mut errors);
        }

        // WHERE / HAVING / GROUP BY column references. `visible` and
        // `no_aliases` were built above for the JOIN pass.
        let aliases = collect_projection_aliases(select);

        if let Some(where_expr) = &select.selection {
            validate_expr_column_refs(
                where_expr,
                &visible,
                schema,
                extras,
                &no_aliases,
                &mut errors,
            );
        }
        if let Some(having_expr) = &select.having {
            validate_expr_column_refs(having_expr, &visible, schema, extras, &aliases, &mut errors);
        }
        if let GroupByExpr::Expressions(exprs) = &select.group_by {
            for expr in exprs {
                validate_expr_column_refs(expr, &visible, schema, extras, &aliases, &mut errors);
            }
        }

        errors
    }
}
