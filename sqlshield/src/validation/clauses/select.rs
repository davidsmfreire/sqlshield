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
        if let Some(r) = VisibleRelation::from_factor(&t.relation) {
            out.push(r);
        }
        for j in &t.joins {
            if let Some(r) = VisibleRelation::from_factor(&j.relation) {
                out.push(r);
            }
        }
    }
    out
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

fn resolve_unqualified(
    col: &str,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    extras: &HashMap<&str, HashSet<&str>>,
) -> Option<String> {
    let mut any_known = false;
    for rel in relations {
        match column_in_relation(col, rel, schema, extras) {
            Some(true) => return None,
            Some(false) => any_known = true,
            None => {}
        }
    }
    if !any_known {
        // None of the visible relations are in the schema: table-not-found
        // errors from the FROM check already covered this; don't pile on.
        return None;
    }
    Some(format!("Column `{col}` not found in any visible table"))
}

/// Like [`resolve_unqualified`] but carries a richer error message that
/// names the specific table(s) the column was searched in. Used by the
/// projection check, which historically reported "not found in table X"
/// rather than the more generic "not found in any visible table".
fn resolve_unqualified_for_projection(
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
            if let Some(relation_name) =
                asserts::is_relation_in_schema(&item.relation, schema, extras)
            {
                errors.push(format!(
                    "Table `{relation_name}` not found in schema nor subqueries"
                ))
            }

            for join in &item.joins {
                if let Some(relation_name) =
                    asserts::is_relation_in_schema(&join.relation, schema, extras)
                {
                    errors.push(format!(
                        "Table `{relation_name}` not found in schema nor subqueries"
                    ))
                }

                // ON / USING constraint column checks.
                if let Some(constraint) = join_constraint(&join.join_operator) {
                    match constraint {
                        JoinConstraint::On(expr) => {
                            validate_expr_column_refs(
                                expr,
                                &visible,
                                schema,
                                extras,
                                &no_aliases,
                                &mut errors,
                            );
                        }
                        JoinConstraint::Using(cols) => {
                            for col in cols {
                                if let Some(err) = resolve_unqualified(
                                    col.value.as_str(),
                                    &visible,
                                    schema,
                                    extras,
                                ) {
                                    errors.push(err);
                                }
                            }
                        }
                        JoinConstraint::Natural | JoinConstraint::None => {}
                    }
                }
            }
        }

        for item in &select.projection {
            let expr = match item {
                SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => e,
                _ => continue,
            };
            let (col_name, col_qualifier) = direct_col_ref(expr);
            let Some(col_name) = col_name else { continue };

            let err = match col_qualifier {
                Some(qual) => resolve_qualified(qual, col_name, &visible, schema, extras),
                None => resolve_unqualified_for_projection(col_name, &visible, schema, extras),
            };
            if let Some(err) = err {
                errors.push(err);
            }
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

/// Pull a direct column reference out of an expression, ignoring wrappers
/// we don't yet drill into (function calls, CASE, casts, etc.). Returns
/// `(column, qualifier)` — the qualifier is the table-or-alias prefix in a
/// 2-segment compound identifier.
fn direct_col_ref(expr: &Expr) -> (Option<&str>, Option<&str>) {
    match expr {
        Expr::Identifier(identifier) => (Some(identifier.value.as_str()), None),
        Expr::CompoundIdentifier(identifier) if identifier.len() == 2 => (
            Some(identifier[1].value.as_str()),
            Some(identifier[0].value.as_str()),
        ),
        _ => (None, None),
    }
}
