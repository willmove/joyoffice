use super::*;

#[test]
fn spill_references() {
    assert_eq!(to_file(&Formula::new("SUM(Sheet1!$D$1#)+B2#")), "SUM(_xlfn.ANCHORARRAY(Sheet1!$D$1))+_xlfn.ANCHORARRAY(B2)");
    assert_eq!(text_to_file("=Sheet1!$D$1#"), "_xlfn.ANCHORARRAY(Sheet1!$D$1)");
    assert_eq!(from_file("SUM(_xlfn.ANCHORARRAY(Sheet1!$D$1))+_xlfn.ANCHORARRAY(B2)"), "SUM(Sheet1!$D$1#)+B2#");
    // Found by the command-sequence fuzzer: deleting the anchor of `H5#` left `=#REF!` in
    // the workbook but `ANCHORARRAY(#REF!)` in the file, which read back differently.
    let deleted = Formula::from_expr(Expr::Unary(joysheet_formula::UnOp::Spill, Box::new(Expr::Error(joysheet_core::CellError::Ref))));
    assert_eq!(deleted.text, "#REF!");
    assert_eq!(to_file(&deleted), "#REF!");
}

#[test]
fn implicit_intersection() {
    assert_eq!(to_file(&Formula::new("SUM(@A1:A3*B1:B3)")), "SUM(_xlfn.SINGLE(A1:A3)*B1:B3)");
    assert_eq!(to_file(&Formula::new("@A1:A3*2")), "_xlfn.SINGLE(A1:A3)*2");
    assert_eq!(text_to_file("=@Sheet1!$A$1:$A$3"), "_xlfn.SINGLE(Sheet1!$A$1:$A$3)");
    assert_eq!(from_file("_xlfn.SINGLE(A1:A3)*2"), "@A1:A3*2");
    assert_eq!(from_file("SUM(_xlfn.SINGLE(A1:A3)*B1:B3)"), "SUM(@A1:A3*B1:B3)");
}

#[test]
fn trim_references() {
    assert_eq!(to_file(&Formula::new("SUM(A1:.E10)+SUM(Data!A.:A)")), "SUM(_xlfn.TRIMRANGE(A1:E10,2,2))+SUM(_xlfn.TRIMRANGE(Data!A:A,1,1))");
    assert_eq!(text_to_file("=ROWS(1.:.5)"), "ROWS(_xlfn.TRIMRANGE(1:5,3,3))");
    assert_eq!(from_file("SUM(_xlfn._TRO_TRAILING(A1:E10))+_xlfn._TRO_ALL($A:$B)"), "SUM(A1:.E10)+$A.:.$B");
}

#[test]
fn unbracketed_this_row_column() {
    assert_eq!(from_file("Sales[[#This Row],Units]*[[#This Row],Price]"), "Sales[@Units]*[@Price]");
}

#[test]
fn this_row_long_form() {
    // `[@Col]` is only Excel's display shorthand; a file holding it is repaired by Excel,
    // which drops the table.
    assert_eq!(to_file(&Formula::new("Sales[@Units]*Sales[@[Unit Price]]")), "Sales[[#This Row],[Units]]*Sales[[#This Row],[Unit Price]]");
    assert_eq!(text_to_file("=[@Units]&COUNTA(Sales[@])"), "[[#This Row],[Units]]&COUNTA(Sales[#This Row])");
    assert_eq!(text_to_file("=SUM(Sales[Units])+[#Totals]"), "SUM(Sales[Units])+[#Totals]");
    for f in ["Sales[@Units]*[@[Unit Price]]", "Sales[@]", "[@[Unit Price]]+1"] {
        let file = to_file(&Formula::new(f));
        assert!(!file.contains("[@"), "{file}");
        assert_eq!(from_file(&file), f);
    }
}

#[test]
fn struct_ref_column_with_bang() {
    // A `!` inside the brackets is part of the column name, not a sheet separator.
    assert_eq!(to_file(&Formula::new("SUM(Table1[Done!])")), "SUM(Table1[Done!])");
    assert_eq!(text_to_file("=Table1[[#This Row],[a!b]]"), "Table1[[#This Row],[a!b]]");
    // `Table1[#REF!]` was found by the command-sequence fuzzer (seed 40).
    for text in ["SUM(Table1[Done!])", "Table1[#REF!]", "Table1[@[x!]]", "Sheet2!A1+Table1[[#All],[a!b]]"] {
        let f = Formula::new(text);
        assert_eq!(from_file(&to_file(&f)), f.text);
    }
}

#[test]
fn prefixes() {
    assert_eq!(to_file(&Formula::new("XLOOKUP(1,A:A,B:B)+SUM(A1)")), "_xlfn.XLOOKUP(1,A:A,B:B)+SUM(A1)");
    assert_eq!(to_file(&Formula::new("FILTER(A1:A3,B1:B3)")), "_xlfn._xlws.FILTER(A1:A3,B1:B3)");
    assert_eq!(from_file("_xlfn.XLOOKUP(1,A:A,B:B)"), "XLOOKUP(1,A:A,B:B)");
    assert_eq!(text_to_file("=IFS(A1>0,1)"), "_xlfn.IFS(A1>0,1)");
    assert_eq!(to_file(&Formula::new("REGEXTEST(A1,\"x\")&ENCODEURL(B1)")), "_xlfn.REGEXTEST(A1,\"x\")&_xlfn.ENCODEURL(B1)");
    assert_eq!(to_file(&Formula::new("GROUPBY(A1:A3,B1:B3,\"SUM\")")), "_xlfn.GROUPBY(A1:A3,B1:B3,\"SUM\")");
}

#[test]
fn let_lambda_params() {
    assert_eq!(to_file(&Formula::new("LET(x,1,x+1)")), "_xlfn.LET(_xlpm.x,1,_xlpm.x+1)");
    assert_eq!(to_file(&Formula::new("LET(x,A1*2,y,x+1,x*y)")), "_xlfn.LET(_xlpm.x,A1*2,_xlpm.y,_xlpm.x+1,_xlpm.x*_xlpm.y)");
    assert_eq!(to_file(&Formula::new("LAMBDA(a,b,a^2+b)(3,4)")), "_xlfn.LAMBDA(_xlpm.a,_xlpm.b,_xlpm.a^2+_xlpm.b)(3,4)");
    // A name outside the scope stays a plain defined name.
    assert_eq!(to_file(&Formula::new("LET(x,1,x)+x")), "_xlfn.LET(_xlpm.x,1,_xlpm.x)+x");
    assert_eq!(from_file("_xlfn.LET(_xlpm.x,1,_xlpm.x+1)"), "LET(x,1,x+1)");
    assert_eq!(from_file("_xlfn.LAMBDA(_xlpm.a,_xlpm.a*2)(3)"), "LAMBDA(a,a*2)(3)");
    // A LET-bound LAMBDA called by name.
    assert_eq!(to_file(&Formula::new("LET(f,LAMBDA(x,x*2),f(3))")), "_xlfn.LET(_xlpm.f,_xlfn.LAMBDA(_xlpm.x,_xlpm.x*2),_xlpm.F(3))");
    assert_eq!(from_file("_xlfn.LET(_xlpm.f,_xlfn.LAMBDA(_xlpm.x,_xlpm.x*2),_xlpm.f(3))"), "LET(f,LAMBDA(x,x*2),F(3))");
}
