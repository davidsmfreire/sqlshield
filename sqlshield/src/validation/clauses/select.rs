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
            let result = is_select_item_in_relations(item, &select.from, schema, extras);

            if let Some((item_name, relations_not_found_in)) = result {
                if let [table] = relations_not_found_in.as_slice() {
                    errors.push(format!("Column `{item_name}` not found in table `{table}`"))
                } else {
                    let not_found_on = relations_not_found_in.join(",");
                    errors.push(format!(
                        "Column `{item_name}` not found in none of the tables: {not_found_on}"
                    ))
                }
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

fn is_select_item_in_relations<'a>(
    item: &'a SelectItem,
    tables: &'a [TableWithJoins],
    schema: &'a schema::TablesAndColumns,
    extras: &HashMap<&'a str, HashSet<&'a str>>,
) -> Option<(&'a str, Vec<&'a str>)> {
    let mut tables_searched_where_not_found: Vec<&str> = vec![];
    let mut item_name: Option<&str> = None;

    for relation in tables {
        if let Some((col_name, table_name)) =
            could_select_item_be_in_relation(item, &relation.relation, schema, extras)
        {
            tables_searched_where_not_found.push(table_name);
            if item_name.is_none() {
                item_name = Some(col_name);
            }
        }
        for join in &relation.joins {
            if let Some((col_name, table_name)) =
                could_select_item_be_in_relation(item, &join.relation, schema, extras)
            {
                tables_searched_where_not_found.push(table_name);
                if item_name.is_none() {
                    item_name = Some(col_name);
                }
            }
        }
    }
    if tables_searched_where_not_found.is_empty() {
        return None;
    }

    Some((item_name?, tables_searched_where_not_found))
}

/// Pull a direct column reference out of an expression, ignoring wrappers
/// we don't yet drill into (function calls, CASE, casts, etc.).
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

fn could_select_item_be_in_relation<'a>(
    item: &'a SelectItem,
    table: &'a TableFactor,
    schema: &'a schema::TablesAndColumns,
    extras: &HashMap<&'a str, HashSet<&'a str>>,
) -> Option<(&'a str, &'a str)> {
    // returns item_name, table_name if item could be in table but is not

    // `SELECT expr` and `SELECT expr AS alias` resolve the same underlying
    // column reference — the alias only renames the output.
    let (col_name, col_table_alias) = match item {
        SelectItem::UnnamedExpr(expression)
        | SelectItem::ExprWithAlias {
            expr: expression, ..
        } => direct_col_ref(expression),
        _ => (None, None),
    };

    let (table_name, alias) = match table {
        TableFactor::Table { name, alias, .. } => (
            name.0
                .last()
                .expect("sqlparser guarantees ObjectName has ≥1 ident")
                .value
                .as_str(),
            alias.as_ref(),
        ),
        TableFactor::Derived { alias, .. } => {
            let alias_ref = alias.as_ref()?;
            (alias_ref.name.value.as_str(), Some(alias_ref))
        }
        _ => return None,
    };

    let should_check = match (alias, col_table_alias) {
        (None, None) => true,
        (Some(table_alias), Some(col_alias)) => table_alias.name.value == col_alias,
        _ => false,
    };

    if !should_check {
        return None;
    }

    let col_name = col_name?;

    let column_present = if let Some(cols) = schema.get(table_name) {
        cols.contains(col_name)
    } else if let Some(cols) = extras.get(table_name) {
        cols.contains(col_name)
    } else {
        return None;
    };

    if column_present {
        None
    } else {
        Some((col_name, table_name))
    }
}
