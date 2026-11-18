//! Database functions (DSUM, DCOUNT, DGET…): aggregate one field of the records of a table
//! that satisfy a criteria table.
//!
//! The database and the criteria are arrays whose first row holds column labels. Each criteria
//! row is a set of conditions that must all hold (AND); a record matches when any criteria row
//! holds (OR). An empty criteria cell matches anything. Plain text conditions match values that
//! begin with the text, as in Excel's advanced filter, so an empty string matches any text. A
//! formula under a label that names no field is a computed criterion, which the host evaluates
//! for each record ([`Ctx::computed_criterion`]).

mod records;
use crate::{FnSpec, util::A};
use records::{Operation, evaluate};

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![
        f!(
            "DSUM",
            3,
            3,
            Database,
            A,
            "DSUM(database, field, criteria)",
            "Adds the numbers in a field of the records that meet the criteria.",
            |args, ctx| evaluate(Operation::Sum, args, ctx)
        ),
        f!(
            "DAVERAGE",
            3,
            3,
            Database,
            A,
            "DAVERAGE(database, field, criteria)",
            "Averages the numbers in a field of the records that meet the criteria.",
            |args, ctx| evaluate(Operation::Average, args, ctx)
        ),
        f!(
            "DCOUNT",
            3,
            3,
            Database,
            A,
            "DCOUNT(database, [field], criteria)",
            "Counts numeric cells in a field of the matching records (or the matching records).",
            |args, ctx| evaluate(Operation::Count, args, ctx)
        ),
        f!(
            "DCOUNTA",
            3,
            3,
            Database,
            A,
            "DCOUNTA(database, [field], criteria)",
            "Counts non-blank cells in a field of the matching records (or the matching records).",
            |args, ctx| evaluate(Operation::CountA, args, ctx)
        ),
        f!(
            "DGET",
            3,
            3,
            Database,
            A,
            "DGET(database, field, criteria)",
            "Returns the field value of the single record that meets the criteria.",
            |args, ctx| evaluate(Operation::Get, args, ctx)
        ),
        f!("DMAX", 3, 3, Database, A, "DMAX(database, field, criteria)", "Largest number in a field of the matching records.", |args, ctx| evaluate(
            Operation::Max,
            args,
            ctx
        )),
        f!(
            "DMIN",
            3,
            3,
            Database,
            A,
            "DMIN(database, field, criteria)",
            "Smallest number in a field of the matching records.",
            |args, ctx| evaluate(Operation::Min, args, ctx)
        ),
        f!(
            "DPRODUCT",
            3,
            3,
            Database,
            A,
            "DPRODUCT(database, field, criteria)",
            "Multiplies the numbers in a field of the matching records.",
            |args, ctx| evaluate(Operation::Product, args, ctx)
        ),
        f!(
            "DSTDEV",
            3,
            3,
            Database,
            A,
            "DSTDEV(database, field, criteria)",
            "Sample standard deviation of a field over the matching records.",
            |args, ctx| evaluate(Operation::StdDev, args, ctx)
        ),
        f!(
            "DSTDEVP",
            3,
            3,
            Database,
            A,
            "DSTDEVP(database, field, criteria)",
            "Population standard deviation of a field over the matching records.",
            |args, ctx| evaluate(Operation::StdDevP, args, ctx)
        ),
        f!("DVAR", 3, 3, Database, A, "DVAR(database, field, criteria)", "Sample variance of a field over the matching records.", |args, ctx| {
            evaluate(Operation::Var, args, ctx)
        }),
        f!(
            "DVARP",
            3,
            3,
            Database,
            A,
            "DVARP(database, field, criteria)",
            "Population variance of a field over the matching records.",
            |args, ctx| evaluate(Operation::VarP, args, ctx)
        ),
    ]
}

#[cfg(test)]
#[path = "tests/database_tests.rs"]
mod tests;
