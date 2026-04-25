use std::collections::HashSet;

use sqlparser::ast::{
    Expr, FunctionArg, FunctionArgExpr, GroupByExpr, Ident, JoinConstraint, JoinOperator, Select,
    SelectItem, TableFactor, TableWithJoins,
};

use crate::dialect::Dialect;
use crate::schema::sql::{fold_ident, fold_str};
use crate::validation::{asserts, Extras};
use crate::{schema, validation::ClauseValidation};

/// A table (or CTE-derived relation) visible to the current Select scope.
pub(crate) struct VisibleRelation<'a> {
    /// Last segment of the table name (`users` in `public.users`).
    name: &'a Ident,
    /// Alias if one was given (`u` in `users u`).
    alias: Option<&'a Ident>,
}

impl<'a> VisibleRelation<'a> {
    /// The Ident the caller should use when referring to this relation with
    /// a qualifier (the alias if present, otherwise the name).
    fn qualifier(&self) -> &'a Ident {
        self.alias.unwrap_or(self.name)
    }

    /// Display string of the table name (without alias). Used for error
    /// messages — preserves the user's casing.
    pub(crate) fn name_display(&self) -> &'a str {
        self.name.value.as_str()
    }

    fn from_factor(factor: &'a TableFactor) -> Option<Self> {
        match factor {
            TableFactor::Table { name, alias, .. } => Some(Self {
                name: name.0.last()?,
                alias: alias.as_ref().map(|a| &a.name),
            }),
            // A derived table `(SELECT …) alias` — its alias doubles as the
            // relation name. Its projected columns are tracked in `extras`.
            TableFactor::Derived { alias, .. } => {
                let alias_ref = alias.as_ref()?;
                Some(Self {
                    name: &alias_ref.name,
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
    dialect: Dialect,
    extras: &Extras,
) -> Vec<String> {
    // Make derived tables in the FROM clause visible to expressions
    // evaluated outside the regular Select::validate pass (ORDER BY).
    let mut local_extras = extras.clone();
    for tw in &select.from {
        crate::validation::publish_derived(&tw.relation, schema, dialect, &mut local_extras);
        for join in &tw.joins {
            crate::validation::publish_derived(&join.relation, schema, dialect, &mut local_extras);
        }
    }
    let visible = collect_visible_relations(&select.from);
    let aliases = collect_projection_aliases(select);
    let mut errors = Vec::new();
    for expr in exprs {
        validate_expr_column_refs(
            expr,
            &visible,
            schema,
            dialect,
            &local_extras,
            &aliases,
            &mut errors,
        );
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
    dialect: Dialect,
    extras: &Extras,
    errors: &mut Vec<String>,
) {
    if let Some(name) = asserts::is_relation_in_schema(factor, schema, dialect, extras) {
        errors.push(format!("Table `{name}` not found in schema nor subqueries"));
    }
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = factor
    {
        validate_from_factor(
            &table_with_joins.relation,
            visible,
            schema,
            dialect,
            extras,
            errors,
        );
        for join in &table_with_joins.joins {
            validate_from_factor(&join.relation, visible, schema, dialect, extras, errors);
            validate_join_op(
                &join.join_operator,
                &join.relation,
                visible,
                schema,
                dialect,
                extras,
                errors,
            );
        }
    }
}

/// Validate a join's ON / USING / NATURAL constraint columns against the
/// outer visible relations. `right` is the right-hand `TableFactor` of the
/// join; needed to identify which side is "new" so NATURAL JOIN can compare
/// its column set against the accumulated left-hand side.
fn validate_join_op(
    op: &JoinOperator,
    right: &TableFactor,
    visible: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
    errors: &mut Vec<String>,
) {
    let Some(constraint) = join_constraint(op) else {
        return;
    };
    let no_aliases: HashSet<&str> = HashSet::new();
    match constraint {
        JoinConstraint::On(expr) => {
            validate_expr_column_refs(expr, visible, schema, dialect, extras, &no_aliases, errors);
        }
        JoinConstraint::Using(cols) => {
            for col in cols {
                if let Some(err) =
                    resolve_unqualified_for_using(col, visible, schema, dialect, extras)
                {
                    errors.push(err);
                }
            }
        }
        JoinConstraint::Natural => {
            if let Some(err) = validate_natural_join(right, visible, schema, dialect, extras) {
                errors.push(err);
            }
        }
        JoinConstraint::None => {}
    }
}

/// Resolve the column set of a `TableFactor` against the schema or CTE/
/// derived-table extras. Returns `None` if the factor isn't statically known
/// (subquery without alias, table function, etc.) — caller should silently
/// skip those.
fn factor_cols(
    factor: &TableFactor,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Option<HashSet<String>> {
    match factor {
        TableFactor::Table { name, .. } => {
            let key = if name.0.len() > 1 {
                crate::schema::sql::qualified_key(name, dialect)
            } else {
                fold_ident(name.0.last()?, dialect)
            };
            schema
                .get(&key)
                .cloned()
                .or_else(|| extras.get(&key).cloned())
        }
        TableFactor::Derived { alias, .. } => {
            let alias = alias.as_ref()?;
            let key = fold_ident(&alias.name, dialect);
            extras.get(&key).cloned()
        }
        _ => None,
    }
}

/// NATURAL JOIN: implicit equi-join on every column whose name appears in
/// both sides. Flag a query that uses NATURAL JOIN with no shared columns —
/// most engines silently degrade to a Cartesian product, which is almost
/// certainly not what the author meant.
fn validate_natural_join(
    right: &TableFactor,
    visible: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Option<String> {
    let right_cols = factor_cols(right, schema, dialect, extras)?;
    let right_qualifier_key = match right {
        TableFactor::Table { name, alias, .. } => {
            if let Some(a) = alias {
                fold_ident(&a.name, dialect)
            } else {
                fold_ident(name.0.last()?, dialect)
            }
        }
        TableFactor::Derived { alias, .. } => fold_ident(&alias.as_ref()?.name, dialect),
        _ => return None,
    };

    // Union of every other visible relation's columns. Skip relations whose
    // own column set we can't resolve — we don't want to false-positive when
    // schema is incomplete.
    let mut left_cols: HashSet<String> = HashSet::new();
    for rel in visible {
        if fold_ident(rel.qualifier(), dialect) == right_qualifier_key {
            continue;
        }
        let key = fold_ident(rel.name, dialect);
        let cols = schema.get(&key).or_else(|| extras.get(&key));
        if let Some(cols) = cols {
            left_cols.extend(cols.iter().cloned());
        }
    }
    if left_cols.is_empty() {
        return None;
    }
    if right_cols.is_disjoint(&left_cols) {
        let display = match right {
            TableFactor::Table { name, .. } => name
                .0
                .iter()
                .map(|p| p.value.as_str())
                .collect::<Vec<_>>()
                .join("."),
            TableFactor::Derived { alias, .. } => alias
                .as_ref()
                .map(|a| a.name.value.clone())
                .unwrap_or_default(),
            _ => String::new(),
        };
        return Some(format!(
            "NATURAL JOIN of `{display}` shares no column with the left-hand relations"
        ));
    }
    None
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
/// All identifier comparisons honor the dialect's folding rules.
fn column_in_relation(
    col: &Ident,
    rel: &VisibleRelation<'_>,
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Option<bool> {
    let rel_key = fold_ident(rel.name, dialect);
    let col_key = fold_ident(col, dialect);
    if let Some(cols) = schema.get(&rel_key) {
        return Some(cols.contains(&col_key));
    }
    if let Some(cols) = asserts::extras_get(extras, &rel_key) {
        return Some(cols.contains(&col_key));
    }
    None
}

/// Resolve an unqualified column reference against the visible relations.
/// The error message names the specific table(s) the column was missing
/// from when at least one visible relation is known to the schema; if no
/// known relation contains this column, table-not-found errors emitted by
/// the FROM walk already covered the situation, so we stay quiet. Columns
/// found in two or more visible relations are reported as ambiguous.
fn resolve_unqualified(
    col: &Ident,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Option<String> {
    resolve_unqualified_inner(col, relations, schema, dialect, extras, true)
}

/// Variant used by `JOIN ... USING (cols)` where the column is *expected* to
/// exist in two or more relations (that's the whole point of USING). Skips
/// the ambiguity check but keeps the missing-everywhere check.
fn resolve_unqualified_for_using(
    col: &Ident,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Option<String> {
    resolve_unqualified_inner(col, relations, schema, dialect, extras, false)
}

fn resolve_unqualified_inner(
    col: &Ident,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
    flag_ambiguity: bool,
) -> Option<String> {
    let mut found_in: Vec<&str> = Vec::new();
    let mut not_found_in: Vec<&str> = Vec::new();
    for rel in relations {
        match column_in_relation(col, rel, schema, dialect, extras) {
            Some(true) => found_in.push(rel.name_display()),
            Some(false) => not_found_in.push(rel.name_display()),
            None => {}
        }
    }
    if flag_ambiguity && found_in.len() >= 2 {
        let names = found_in.join(",");
        return Some(format!(
            "Column `{}` is ambiguous; appears in: {names}",
            col.value
        ));
    }
    if !found_in.is_empty() {
        return None;
    }
    if let [table] = not_found_in.as_slice() {
        Some(format!(
            "Column `{}` not found in table `{table}`",
            col.value
        ))
    } else if !not_found_in.is_empty() {
        let names = not_found_in.join(",");
        Some(format!(
            "Column `{}` not found in none of the tables: {names}",
            col.value
        ))
    } else {
        None
    }
}

fn resolve_qualified(
    qualifier: &Ident,
    col: &Ident,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
) -> Option<String> {
    let qualifier_key = fold_ident(qualifier, dialect);
    let matched = relations
        .iter()
        .find(|r| fold_ident(r.qualifier(), dialect) == qualifier_key)?;
    match column_in_relation(col, matched, schema, dialect, extras) {
        Some(false) => Some(format!(
            "Column `{}` not found in table `{}`",
            col.value,
            matched.name_display()
        )),
        _ => None,
    }
}

pub(crate) fn validate_expr_column_refs(
    root: &Expr,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
    aliases: &HashSet<&str>,
    errors: &mut Vec<String>,
) {
    walk_expr(root, relations, schema, dialect, extras, aliases, errors);
}

/// Manual recursion over `Expr`. Unlike `sqlparser::ast::visit_expressions`,
/// this does NOT descend blindly into subqueries — those are handed off to
/// `validate_query_with_scope` with a fresh scope so their identifiers
/// resolve against their own FROM, not the enclosing one.
fn walk_expr(
    expr: &Expr,
    relations: &[VisibleRelation<'_>],
    schema: &schema::TablesAndColumns,
    dialect: Dialect,
    extras: &Extras,
    aliases: &HashSet<&str>,
    errors: &mut Vec<String>,
) {
    match expr {
        Expr::Identifier(ident) => {
            // Projection aliases are matched against the original casing
            // currently surfaced by sqlparser; mirror that here.
            if aliases.contains(ident.value.as_str()) {
                return;
            }
            // Also accept aliases under dialect-aware folding so quoted
            // aliases compare correctly in PG mode.
            let folded = fold_str(ident.value.as_str(), dialect);
            if aliases.iter().any(|a| fold_str(a, dialect) == folded) {
                return;
            }
            if let Some(err) = resolve_unqualified(ident, relations, schema, dialect, extras) {
                errors.push(err);
            }
        }
        Expr::CompoundIdentifier(idents) if idents.len() == 2 => {
            if let Some(err) =
                resolve_qualified(&idents[0], &idents[1], relations, schema, dialect, extras)
            {
                errors.push(err);
            }
        }
        // `schema.table.col`: the table-qualifier is `idents[1]`. Match against
        // visible relations the same way as a 2-part reference; the FROM walk
        // already validated whether `schema.table` resolves at all.
        Expr::CompoundIdentifier(idents) if idents.len() == 3 => {
            if let Some(err) =
                resolve_qualified(&idents[1], &idents[2], relations, schema, dialect, extras)
            {
                errors.push(err);
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            walk_expr(left, relations, schema, dialect, extras, aliases, errors);
            walk_expr(right, relations, schema, dialect, extras, aliases, errors);
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
            walk_expr(expr, relations, schema, dialect, extras, aliases, errors);
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            walk_expr(expr, relations, schema, dialect, extras, aliases, errors);
            walk_expr(low, relations, schema, dialect, extras, aliases, errors);
            walk_expr(high, relations, schema, dialect, extras, aliases, errors);
        }
        Expr::InList { expr, list, .. } => {
            walk_expr(expr, relations, schema, dialect, extras, aliases, errors);
            for item in list {
                walk_expr(item, relations, schema, dialect, extras, aliases, errors);
            }
        }
        Expr::Like { expr, pattern, .. }
        | Expr::ILike { expr, pattern, .. }
        | Expr::SimilarTo { expr, pattern, .. }
        | Expr::RLike { expr, pattern, .. } => {
            walk_expr(expr, relations, schema, dialect, extras, aliases, errors);
            walk_expr(pattern, relations, schema, dialect, extras, aliases, errors);
        }
        Expr::Case {
            operand,
            conditions,
            results,
            else_result,
        } => {
            if let Some(op) = operand {
                walk_expr(op, relations, schema, dialect, extras, aliases, errors);
            }
            for c in conditions {
                walk_expr(c, relations, schema, dialect, extras, aliases, errors);
            }
            for r in results {
                walk_expr(r, relations, schema, dialect, extras, aliases, errors);
            }
            if let Some(e) = else_result {
                walk_expr(e, relations, schema, dialect, extras, aliases, errors);
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
                        walk_expr(e, relations, schema, dialect, extras, aliases, errors);
                    }
                    _ => {}
                }
            }
        }
        Expr::AnyOp { left, right, .. } | Expr::AllOp { left, right, .. } => {
            walk_expr(left, relations, schema, dialect, extras, aliases, errors);
            walk_expr(right, relations, schema, dialect, extras, aliases, errors);
        }
        // Subquery boundaries: hand off to the query validator with a fresh
        // scope. The enclosing `extras` (CTEs, derived tables visible at this
        // level) are threaded down so the inner query can see them.
        Expr::Subquery(q) => {
            errors.extend(crate::validation::validate_query_with_scope(
                q.as_ref(),
                schema,
                dialect,
                extras,
            ));
        }
        Expr::Exists { subquery, .. } => {
            errors.extend(crate::validation::validate_query_with_scope(
                subquery.as_ref(),
                schema,
                dialect,
                extras,
            ));
        }
        Expr::InSubquery { expr, subquery, .. } => {
            walk_expr(expr, relations, schema, dialect, extras, aliases, errors);
            errors.extend(crate::validation::validate_query_with_scope(
                subquery.as_ref(),
                schema,
                dialect,
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
        dialect: Dialect,
        extras: &Extras,
    ) -> Vec<String> {
        let select = self;
        let mut errors = vec![];

        // Visible relations are needed both for FROM/JOIN constraint walks
        // and for WHERE/HAVING/GROUP BY below, so build them once up front.
        let visible = collect_visible_relations(&select.from);
        let no_aliases: HashSet<&str> = HashSet::new();

        for item in &select.from {
            validate_from_factor(
                &item.relation,
                &visible,
                schema,
                dialect,
                extras,
                &mut errors,
            );
            for join in &item.joins {
                validate_from_factor(
                    &join.relation,
                    &visible,
                    schema,
                    dialect,
                    extras,
                    &mut errors,
                );
                validate_join_op(
                    &join.join_operator,
                    &join.relation,
                    &visible,
                    schema,
                    dialect,
                    extras,
                    &mut errors,
                );
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
            validate_expr_column_refs(
                expr,
                &visible,
                schema,
                dialect,
                extras,
                &no_aliases,
                &mut errors,
            );
        }

        // WHERE / HAVING / GROUP BY column references. `visible` and
        // `no_aliases` were built above for the JOIN pass.
        let aliases = collect_projection_aliases(select);

        if let Some(where_expr) = &select.selection {
            validate_expr_column_refs(
                where_expr,
                &visible,
                schema,
                dialect,
                extras,
                &no_aliases,
                &mut errors,
            );
        }
        if let Some(having_expr) = &select.having {
            validate_expr_column_refs(
                having_expr,
                &visible,
                schema,
                dialect,
                extras,
                &aliases,
                &mut errors,
            );
        }
        if let GroupByExpr::Expressions(exprs) = &select.group_by {
            for expr in exprs {
                validate_expr_column_refs(
                    expr,
                    &visible,
                    schema,
                    dialect,
                    extras,
                    &aliases,
                    &mut errors,
                );
            }
        }

        errors
    }
}
